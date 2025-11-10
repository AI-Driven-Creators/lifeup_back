use rbatis::RBatis;
use crate::models::{Achievement, UserAchievement, Task, Skill, UserProfile, UserAttributes, TaskStatus, AchievementRequirementType};
use rbs::value;
use uuid::Uuid;
use chrono::Utc;
use log::{info, error};

pub struct AchievementService;

impl AchievementService {
    /// 檢查並可能解鎖使用者的成就
    pub async fn check_and_unlock_achievements(rb: &RBatis, user_id: &str) -> Result<Vec<Achievement>, anyhow::Error> {
        // 1. 獲取所有成就定義 和 使用者已解鎖的成就ID
        let all_achievements = Achievement::select_all(rb).await?;
        let user_unlocked_achievements: Vec<UserAchievement> = UserAchievement::select_by_map(rb, value!{"user_id": user_id}).await?;
        let unlocked_ids: std::collections::HashSet<Option<String>> = user_unlocked_achievements.into_iter().map(|ua| ua.achievement_id).collect();

        let mut newly_unlocked = Vec::new();

        // 2. 遍歷所有未解鎖的成就
        for achievement in all_achievements {
            if unlocked_ids.contains(&achievement.id) {
                continue; // 跳過已解鎖的
            }

            let requirement_value = achievement.requirement_value.unwrap_or(0);

            let should_unlock = match &achievement.requirement_type {
                Some(AchievementRequirementType::TaskComplete) => {
                    // 如果成就有 related_task_id，檢查該特定任務是否完成
                    if let Some(related_task_id) = &achievement.related_task_id {
                        let sql = "SELECT COUNT(*) FROM task WHERE id = ? AND user_id = ? AND status = ?";
                        let args = vec![
                            related_task_id.clone().into(),
                            user_id.into(),
                            TaskStatus::Completed.to_i32().into()
                        ];
                        let count: u64 = rb.query_decode(sql, args).await?;
                        count > 0
                    } else {
                        // 沒有 related_task_id，檢查完成任務總數
                        let sql = "SELECT COUNT(*) FROM task WHERE user_id = ? AND status = ?";
                        let args = vec![user_id.into(), TaskStatus::Completed.to_i32().into()];
                        let count: u64 = rb.query_decode(sql, args).await?;
                        count >= requirement_value as u64
                    }
                },
                Some(AchievementRequirementType::LearningTaskComplete) => {
                    let sql = "SELECT COUNT(*) FROM task WHERE user_id = ? AND status = ? AND skill_tags LIKE ?";
                    let args = vec![user_id.into(), TaskStatus::Completed.to_i32().into(), rbs::Value::String("%智慧%".to_string())];
                    let count: u64 = rb.query_decode(sql, args).await?;
                    count >= requirement_value as u64
                },
                Some(AchievementRequirementType::SkillLevel) => {
                    // 檢查是否有任何一個技能達到等級
                    let skills = Skill::select_by_map(rb, value!{"user_id": user_id}).await?;
                    skills.iter().any(|s| s.level.unwrap_or(0) >= requirement_value)
                },
                Some(AchievementRequirementType::ConsecutiveDays) => {
                    if let Some(profile) = UserProfile::select_by_map(rb, value!{"user_id": user_id}).await?.first() {
                        profile.consecutive_login_days.unwrap_or(0) >= requirement_value
                    } else { false }
                },
                Some(AchievementRequirementType::StreakRecovery) => {
                    // 定義：自上次取消任務(last_cancelled_at 最大值)之後，
                    // 用戶完成的任務數量達到 requirement_value 視為從低潮中恢復
                    // 1) 找出用戶最近一次取消時間
                    let tasks_with_cancel = Task::select_by_map(rb, value!{"user_id": user_id}).await?;
                    let latest_cancel_time = tasks_with_cancel
                        .iter()
                        .filter_map(|t| t.last_cancelled_at)
                        .max();

                    if let Some(latest_cancel_time) = latest_cancel_time {
                        // 2) 統計在該時間點之後完成(updated_at)的任務數量
                        let completed_tasks_after = tasks_with_cancel
                            .iter()
                            .filter(|t| t.status == Some(TaskStatus::Completed.to_i32()))
                            .filter_map(|t| t.updated_at)
                            .filter(|updated| *updated > latest_cancel_time)
                            .count() as i32;

                        completed_tasks_after >= requirement_value
                    } else {
                        // 沒有取消紀錄則不符合「恢復」定義
                        false
                    }
                },
                // 屬性相關成就
                Some(AchievementRequirementType::IntelligenceAttribute) => {
                    Self::check_attribute_requirement(rb, user_id, "intelligence", requirement_value).await?
                },
                Some(AchievementRequirementType::EnduranceAttribute) => {
                    Self::check_attribute_requirement(rb, user_id, "endurance", requirement_value).await?
                },
                Some(AchievementRequirementType::CreativityAttribute) => {
                    Self::check_attribute_requirement(rb, user_id, "creativity", requirement_value).await?
                },
                Some(AchievementRequirementType::SocialAttribute) => {
                    Self::check_attribute_requirement(rb, user_id, "social", requirement_value).await?
                },
                Some(AchievementRequirementType::FocusAttribute) => {
                    Self::check_attribute_requirement(rb, user_id, "focus", requirement_value).await?
                },
                Some(AchievementRequirementType::AdaptabilityAttribute) => {
                    Self::check_attribute_requirement(rb, user_id, "adaptability", requirement_value).await?
                },
                None => {
                    error!("成就 {} 沒有設置達成條件類型", achievement.name.as_deref().unwrap_or("未知"));
                    false
                },
            };

            // 3. 如果條件滿足，解鎖成就
            if should_unlock {
                info!("條件滿足，準備解鎖成就: {}", achievement.name.as_deref().unwrap_or("未知"));
                let user_achievement = UserAchievement {
                    id: Some(Uuid::new_v4().to_string()),
                    user_id: Some(user_id.to_string()),
                    achievement_id: achievement.id.clone(),
                    achieved_at: Some(Utc::now()),
                    progress: Some(requirement_value),
                };
                if UserAchievement::insert(rb, &user_achievement).await.is_ok() {
                    info!("成功解鎖成就: {}", achievement.name.as_deref().unwrap_or("未知"));
                    newly_unlocked.push(achievement);
                } else {
                    error!("解鎖成就 {} 失敗", achievement.name.as_deref().unwrap_or("未知"));
                }
            }
        }

        Ok(newly_unlocked)
    }

    /// 檢查用戶屬性是否達到要求
    async fn check_attribute_requirement(
        rb: &RBatis, 
        user_id: &str, 
        attribute_name: &str, 
        requirement_value: i32
    ) -> Result<bool, anyhow::Error> {
        if let Some(attributes) = UserAttributes::select_by_map(rb, value!{"user_id": user_id}).await?.first() {
            let current_value = match attribute_name {
                "intelligence" => attributes.intelligence,
                "endurance" => attributes.endurance,
                "creativity" => attributes.creativity,
                "social" => attributes.social,
                "focus" => attributes.focus,
                "adaptability" => attributes.adaptability,
                _ => {
                    error!("未知的屬性類型: {}", attribute_name);
                    return Ok(false);
                }
            };
            Ok(current_value.unwrap_or(0) >= requirement_value)
        } else {
            Ok(false)
        }
    }
}