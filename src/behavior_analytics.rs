use anyhow::Result;
use rbatis::RBatis;
use rbs::{value, Value};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

// ===== 資料结构定义 =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserBehaviorSummary {
    // 全局统计
    pub total_tasks_completed: i32,
    pub total_tasks_cancelled: i32,
    pub total_tasks_pending: i32,

    // 時間維度統計
    pub longest_streak: StreakInfo,
    pub current_streak: StreakInfo,
    pub active_days_last_30: i32,
    pub active_days_last_90: i32,

    // 分类统计
    pub top_categories: Vec<CategoryStats>,
    pub top_task_types: Vec<TaskTypeStats>,

    // 样本資料
    pub recent_completions: Vec<TaskSummary>,
    pub recent_cancellations: Vec<TaskSummary>,
    pub milestone_events: Vec<MilestoneEvent>,

    // 成就相关
    pub unlocked_achievements: Vec<String>,
    pub total_experience: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreakInfo {
    pub days: i32,
    pub task_title: String,
    pub category: String,
    pub start_date: String,
    pub end_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryStats {
    pub category: String,
    pub completed_count: i32,
    pub cancelled_count: i32,
    pub completion_rate: f64,
    pub avg_difficulty: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskTypeStats {
    pub task_type: String,
    pub count: i32,
    pub avg_experience: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSummary {
    pub title: String,
    pub category: String,
    pub completion_date: String,
    pub streak_context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MilestoneEvent {
    pub event_type: String, // "首次達成", "突破紀錄", "復出"
    pub description: String,
    pub date: String,
}

// ===== 行为分析核心实现 =====

pub struct BehaviorAnalytics;

impl BehaviorAnalytics {
    /// 生成用户行为摘要（主入口）
    pub async fn generate_summary(rb: &RBatis, user_id: &str) -> Result<UserBehaviorSummary> {
        Ok(UserBehaviorSummary {
            total_tasks_completed: Self::count_completed_tasks(rb, user_id).await?,
            total_tasks_cancelled: Self::count_cancelled_tasks(rb, user_id).await?,
            total_tasks_pending: Self::count_pending_tasks(rb, user_id).await?,

            longest_streak: Self::calculate_longest_streak(rb, user_id).await?,
            current_streak: Self::calculate_current_streak(rb, user_id).await?,

            active_days_last_30: Self::count_active_days(rb, user_id, 30).await?,
            active_days_last_90: Self::count_active_days(rb, user_id, 90).await?,

            top_categories: Self::analyze_categories(rb, user_id, 5).await?,
            top_task_types: Self::analyze_task_types(rb, user_id).await?,

            recent_completions: Self::get_recent_tasks(rb, user_id, 2, 20).await?, // status=2 完成
            recent_cancellations: Self::get_recent_tasks(rb, user_id, 3, 10).await?, // status=3 取消
            milestone_events: Self::detect_milestones(rb, user_id).await?,

            unlocked_achievements: Self::get_achievement_names(rb, user_id).await?,
            total_experience: Self::sum_total_experience(rb, user_id).await?,
        })
    }

    /// 统计完成任务数
    async fn count_completed_tasks(rb: &RBatis, user_id: &str) -> Result<i32> {
        let result: Option<i32> = rb
            .query_decode(
                "SELECT COUNT(*) as count FROM tasks WHERE user_id = ? AND status = 2",
                vec![value!(user_id)],
            )
            .await?;
        Ok(result.unwrap_or(0))
    }

    /// 统计取消任务数
    async fn count_cancelled_tasks(rb: &RBatis, user_id: &str) -> Result<i32> {
        let result: Option<i32> = rb
            .query_decode(
                "SELECT COUNT(*) as count FROM tasks WHERE user_id = ? AND status = 3",
                vec![value!(user_id)],
            )
            .await?;
        Ok(result.unwrap_or(0))
    }

    /// 统计待处理任务数
    async fn count_pending_tasks(rb: &RBatis, user_id: &str) -> Result<i32> {
        let result: Option<i32> = rb
            .query_decode(
                "SELECT COUNT(*) as count FROM tasks WHERE user_id = ? AND status IN (0, 1)",
                vec![value!(user_id)],
            )
            .await?;
        Ok(result.unwrap_or(0))
    }

    /// 計算活跃天数
    async fn count_active_days(rb: &RBatis, user_id: &str, days: i64) -> Result<i32> {
        let cutoff_date = (Utc::now() - Duration::days(days)).to_rfc3339();

        let result: Option<i32> = rb
            .query_decode(
                "SELECT COUNT(DISTINCT DATE(updated_at)) as count
                 FROM tasks
                 WHERE user_id = ?
                 AND status = 2
                 AND updated_at >= ?",
                vec![value!(user_id), value!(cutoff_date)],
            )
            .await?;
        Ok(result.unwrap_or(0))
    }

    /// 分析分类统计
    async fn analyze_categories(
        rb: &RBatis,
        user_id: &str,
        limit: usize,
    ) -> Result<Vec<CategoryStats>> {
        #[derive(Debug, Serialize, Deserialize)]
        struct CategoryRow {
            task_category: String,
            completed: i32,
            cancelled: i32,
            avg_difficulty: f64,
        }

        let rows: Vec<CategoryRow> = rb
            .query_decode(
                "SELECT
                    COALESCE(task_category, '未分类') as task_category,
                    SUM(CASE WHEN status = 2 THEN 1 ELSE 0 END) as completed,
                    SUM(CASE WHEN status = 3 THEN 1 ELSE 0 END) as cancelled,
                    AVG(CASE WHEN status = 2 AND difficulty IS NOT NULL THEN difficulty ELSE 0 END) as avg_difficulty
                FROM tasks
                WHERE user_id = ?
                GROUP BY task_category
                ORDER BY completed DESC
                LIMIT ?",
                vec![value!(user_id), value!(limit as i32)],
            )
            .await
            .unwrap_or_default();

        Ok(rows
            .into_iter()
            .map(|row| {
                let total = row.completed + row.cancelled;
                CategoryStats {
                    category: row.task_category,
                    completed_count: row.completed,
                    cancelled_count: row.cancelled,
                    completion_rate: if total > 0 {
                        row.completed as f64 / total as f64
                    } else {
                        0.0
                    },
                    avg_difficulty: row.avg_difficulty,
                }
            })
            .collect())
    }

    /// 分析任务类型统计
    async fn analyze_task_types(rb: &RBatis, user_id: &str) -> Result<Vec<TaskTypeStats>> {
        #[derive(Debug, Serialize, Deserialize)]
        struct TaskTypeRow {
            task_type: String,
            count: i32,
            avg_experience: f64,
        }

        let rows: Vec<TaskTypeRow> = rb
            .query_decode(
                "SELECT
                    COALESCE(task_type, 'side') as task_type,
                    COUNT(*) as count,
                    AVG(COALESCE(experience, 0)) as avg_experience
                FROM tasks
                WHERE user_id = ? AND status = 2
                GROUP BY task_type
                ORDER BY count DESC",
                vec![value!(user_id)],
            )
            .await
            .unwrap_or_default();

        Ok(rows
            .into_iter()
            .map(|row| TaskTypeStats {
                task_type: row.task_type,
                count: row.count,
                avg_experience: row.avg_experience,
            })
            .collect())
    }

    /// 獲取最近的任务样本
    async fn get_recent_tasks(
        rb: &RBatis,
        user_id: &str,
        status: i32,
        limit: usize,
    ) -> Result<Vec<TaskSummary>> {
        #[derive(Debug, Serialize, Deserialize)]
        struct TaskRow {
            title: String,
            task_category: Option<String>,
            updated_at: String,
        }

        let rows: Vec<TaskRow> = rb
            .query_decode(
                "SELECT title, task_category, updated_at
                FROM tasks
                WHERE user_id = ? AND status = ?
                ORDER BY updated_at DESC
                LIMIT ?",
                vec![value!(user_id), value!(status), value!(limit as i32)],
            )
            .await
            .unwrap_or_default();

        Ok(rows
            .into_iter()
            .map(|row| TaskSummary {
                title: row.title,
                category: row.task_category.unwrap_or_else(|| "未分类".to_string()),
                completion_date: row.updated_at,
                streak_context: None,
            })
            .collect())
    }

    /// 計算最長連續紀錄
    async fn calculate_longest_streak(rb: &RBatis, user_id: &str) -> Result<StreakInfo> {
        #[derive(Debug, Serialize, Deserialize)]
        struct StreakRow {
            start_date: String,
            end_date: String,
            days: i32,
            title: String,
            task_category: Option<String>,
        }

        // 使用窗口函数計算連續天数（SQLite 3.25.0+ 支持）
        let result: Option<StreakRow> = rb
            .query_decode(
                "WITH daily_completions AS (
                    SELECT
                        DATE(updated_at) as completion_date,
                        title,
                        task_category
                    FROM tasks
                    WHERE user_id = ? AND status = 2
                    ORDER BY completion_date
                ),
                date_groups AS (
                    SELECT
                        completion_date,
                        title,
                        task_category,
                        julianday(completion_date) - ROW_NUMBER() OVER (ORDER BY completion_date) as grp
                    FROM daily_completions
                ),
                streaks AS (
                    SELECT
                        MIN(completion_date) as start_date,
                        MAX(completion_date) as end_date,
                        COUNT(*) as days,
                        title,
                        task_category
                    FROM date_groups
                    GROUP BY grp
                    ORDER BY days DESC
                    LIMIT 1
                )
                SELECT start_date, end_date, days, title, task_category
                FROM streaks",
                vec![value!(user_id)],
            )
            .await
            .ok()
            .flatten();

        if let Some(row) = result {
            Ok(StreakInfo {
                days: row.days,
                task_title: row.title,
                category: row.task_category.unwrap_or_else(|| "未分类".to_string()),
                start_date: row.start_date,
                end_date: Some(row.end_date),
            })
        } else {
            Ok(StreakInfo {
                days: 0,
                task_title: "无".to_string(),
                category: "".to_string(),
                start_date: Utc::now().to_rfc3339(),
                end_date: None,
            })
        }
    }

    /// 計算当前連續记录
    async fn calculate_current_streak(rb: &RBatis, user_id: &str) -> Result<StreakInfo> {
        // 獲取最近完成的任务日期
        #[derive(Debug, Serialize, Deserialize)]
        struct RecentTask {
            completion_date: String,
            title: String,
            task_category: Option<String>,
        }

        let recent_tasks: Vec<RecentTask> = rb
            .query_decode(
                "SELECT
                    DATE(updated_at) as completion_date,
                    title,
                    task_category
                FROM tasks
                WHERE user_id = ? AND status = 2
                ORDER BY updated_at DESC
                LIMIT 30",
                vec![value!(user_id)],
            )
            .await
            .unwrap_or_default();

        if recent_tasks.is_empty() {
            return Ok(StreakInfo {
                days: 0,
                task_title: "无".to_string(),
                category: "".to_string(),
                start_date: Utc::now().to_rfc3339(),
                end_date: None,
            });
        }

        // 简化版本：計算从最新任务到今天/昨天的連續性
        let mut streak_days = 1;
        let first_task = &recent_tasks[0];

        Ok(StreakInfo {
            days: streak_days,
            task_title: first_task.title.clone(),
            category: first_task.task_category.clone().unwrap_or_else(|| "未分类".to_string()),
            start_date: first_task.completion_date.clone(),
            end_date: Some(Utc::now().format("%Y-%m-%d").to_string()),
        })
    }

    /// 檢測里程碑事件
    async fn detect_milestones(rb: &RBatis, user_id: &str) -> Result<Vec<MilestoneEvent>> {
        let mut milestones = Vec::new();

        // 檢測首次完成 100 次任务
        let total_completed = Self::count_completed_tasks(rb, user_id).await?;
        if total_completed >= 100 {
            milestones.push(MilestoneEvent {
                event_type: "突破紀錄".to_string(),
                description: format!("已完成 {} 个任务", total_completed),
                date: Utc::now().format("%Y-%m-%d").to_string(),
            });
        }

        // 檢測最長連續紀錄
        let longest = Self::calculate_longest_streak(rb, user_id).await?;
        if longest.days >= 7 {
            milestones.push(MilestoneEvent {
                event_type: "持续坚持".to_string(),
                description: format!("「{}」連續 {} 天", longest.task_title, longest.days),
                date: longest.end_date.unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string()),
            });
        }

        Ok(milestones)
    }

    /// 獲取已解鎖成就名称
    async fn get_achievement_names(rb: &RBatis, user_id: &str) -> Result<Vec<String>> {
        #[derive(Debug, Serialize, Deserialize)]
        struct AchievementName {
            name: String,
        }

        let rows: Vec<AchievementName> = rb
            .query_decode(
                "SELECT a.name
                FROM achievements a
                INNER JOIN user_achievements ua ON a.id = ua.achievement_id
                WHERE ua.user_id = ?",
                vec![value!(user_id)],
            )
            .await
            .unwrap_or_default();

        Ok(rows.into_iter().map(|row| row.name).collect())
    }

    /// 計算总经验值
    async fn sum_total_experience(rb: &RBatis, user_id: &str) -> Result<i32> {
        let result: Option<i32> = rb
            .query_decode(
                "SELECT COALESCE(SUM(experience), 0) as total FROM tasks WHERE user_id = ? AND status = 2",
                vec![value!(user_id)],
            )
            .await?;
        Ok(result.unwrap_or(0))
    }
}
