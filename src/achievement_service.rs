use rbatis::RBatis;
use crate::models::{Achievement, UserAchievement, Task, Skill, UserProfile, UserAttributes, TaskStatus};
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

            let should_unlock = match achievement.requirement_type.as_deref() {
                Some("task_complete") => {
                    let sql = "SELECT COUNT(*) FROM task WHERE user_id = ? AND status = ?";
                    let args = vec![user_id.into(), TaskStatus::Completed.to_i32().into()];
                    let count: u64 = rb.query_decode(sql, args).await?;
                    count >= requirement_value as u64
                },
                Some("learning_task_complete") => {
                    let sql = "SELECT COUNT(*) FROM task WHERE user_id = ? AND status = ? AND skill_tags LIKE ?";
                    let args = vec![user_id.into(), TaskStatus::Completed.to_i32().into(), rbs::Value::String("%智慧%".to_string())];
                    let count: u64 = rb.query_decode(sql, args).await?;
                    count >= requirement_value as u64
                },
                Some("skill_level") => {
                    // 檢查是否有任何一個技能達到等級
                    let skills = Skill::select_by_map(rb, value!{"user_id": user_id}).await?;
                    skills.iter().any(|s| s.level.unwrap_or(0) >= requirement_value)
                },
                Some("consecutive_days") => {
                    if let Some(profile) = UserProfile::select_by_map(rb, value!{"user_id": user_id}).await?.first() {
                        profile.consecutive_login_days.unwrap_or(0) >= requirement_value
                    } else { false }
                },
                Some(attr_type) if attr_type.ends_with("_attribute") => {
                    // 處理所有屬性相關的成就
                    if let Some(attributes) = UserAttributes::select_by_map(rb, value!{"user_id": user_id}).await?.first() {
                        let current_value = match attr_type {
                            "social_attribute" => attributes.social,
                            "focus_attribute" => attributes.focus,
                            "creativity_attribute" => attributes.creativity,
                            "intelligence_attribute" => attributes.intelligence,
                            "endurance_attribute" => attributes.endurance,
                            "adaptability_attribute" => attributes.adaptability,
                            _ => None,
                        };
                        current_value.unwrap_or(0) >= requirement_value
                    } else { false }
                },
                _ => false, // 未知的 requirement_type
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
}