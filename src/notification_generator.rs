use rbatis::RBatis;
use serde_json::json;

/// 通知內容生成器
pub struct NotificationGenerator;

impl NotificationGenerator {
    /// 生成早上任務提醒通知
    pub async fn generate_morning_notification(
        rb: &RBatis,
        user_id: &str,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        // 統計執行中的任務（InProgress=1, DailyInProgress=5）
        let in_progress_count: i64 = rb
            .query_decode(
                "SELECT COUNT(*) as count FROM task WHERE user_id = ? AND status IN (1, 5)",
                vec![rbs::to_value!(user_id)],
            )
            .await
            .unwrap_or(0);

        // 統計等待執行的任務（Pending=0）
        let pending_count: i64 = rb
            .query_decode(
                "SELECT COUNT(*) as count FROM task WHERE user_id = ? AND status = 0",
                vec![rbs::to_value!(user_id)],
            )
            .await
            .unwrap_or(0);

        let total_count = in_progress_count + pending_count;

        // 查詢優先級最高的任務
        let priority_tasks: Vec<serde_json::Value> = rb
            .query_decode(
                "SELECT title FROM task
                 WHERE user_id = ? AND status IN (0, 1, 5)
                 ORDER BY priority DESC, due_date ASC
                 LIMIT 2",
                vec![rbs::to_value!(user_id)],
            )
            .await
            .unwrap_or_default();

        // 生成通知內容
        let (title, body) = if total_count == 0 {
            (
                "早安！".to_string(),
                "今天還沒有待辦任務，享受輕鬆的一天吧 ☀️".to_string(),
            )
        } else {
            let task_list = priority_tasks
                .iter()
                .filter_map(|t| t.get("title").and_then(|v| v.as_str()))
                .collect::<Vec<_>>();

            let status_summary = format!(
                "有{}個任務執行中，有{}個任務等待執行",
                in_progress_count, pending_count
            );

            let body = if task_list.len() >= 2 {
                format!(
                    "{} 💪\n重點任務：\n• {}\n• {}",
                    status_summary, task_list[0], task_list[1]
                )
            } else if task_list.len() == 1 {
                format!(
                    "{} 💪\n重點任務：{}",
                    status_summary, task_list[0]
                )
            } else {
                format!("{} 💪", status_summary)
            };

            ("早安！開始新的一天".to_string(), body)
        };

        Ok(json!({
            "title": title,
            "body": body,
            "icon": "/icon.svg",
            "badge": "/icon.svg",
            "tag": "morning-notification",
            "data": {
                "url": "/mission",
                "type": "morning",
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }
        }))
    }

    /// 生成晚上進度總結通知
    pub async fn generate_evening_notification(
        rb: &RBatis,
        user_id: &str,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        // 查詢今天完成的任務數量
        let completed_today: i64 = rb
            .query_decode(
                "SELECT COUNT(*) as count FROM task
                 WHERE user_id = ?
                 AND status IN (2, 6)
                 AND date(updated_at) = date('now')",
                vec![rbs::to_value!(user_id)],
            )
            .await
            .unwrap_or(0);

        // 查詢今天獲得的總經驗值
        let total_exp: i64 = rb
            .query_decode(
                "SELECT COALESCE(SUM(experience), 0) as total_exp FROM task
                 WHERE user_id = ?
                 AND status IN (2, 6)
                 AND date(updated_at) = date('now')",
                vec![rbs::to_value!(user_id)],
            )
            .await
            .unwrap_or(0);

        // 查詢進行中的任務數量（InProgress=1, DailyInProgress=5）
        let in_progress_count: i64 = rb
            .query_decode(
                "SELECT COUNT(*) as count FROM task WHERE user_id = ? AND status IN (1, 5)",
                vec![rbs::to_value!(user_id)],
            )
            .await
            .unwrap_or(0);

        // 生成通知內容
        let (title, body) = if completed_today == 0 {
            // 沒有完成任何任務
            if in_progress_count > 0 {
                (
                    "今天辛苦了！".to_string(),
                    format!("今天還沒有完成任務，有{}個任務進行中，明天繼續加油！💪", in_progress_count),
                )
            } else {
                (
                    "今天辛苦了！".to_string(),
                    "今天是休息日，好好放鬆一下 😊".to_string(),
                )
            }
        } else if completed_today == 1 {
            // 完成了1個任務
            let body = if total_exp > 0 {
                if in_progress_count > 0 {
                    format!("完成了1個任務，獲得了 {} XP！還有{}個任務進行中 🎉", total_exp, in_progress_count)
                } else {
                    format!("完成了1個任務，獲得了 {} XP 經驗值 🎉", total_exp)
                }
            } else {
                if in_progress_count > 0 {
                    format!("完成了1個任務！還有{}個任務進行中 🎉", in_progress_count)
                } else {
                    "很棒的開始！繼續保持 🎉".to_string()
                }
            };
            ("今天完成了 1 個任務！".to_string(), body)
        } else {
            // 完成了多個任務
            let body = if total_exp > 0 {
                if in_progress_count > 0 {
                    format!(
                        "完成了 {} 個任務，獲得 {} XP！還有{}個任務進行中 💪",
                        completed_today, total_exp, in_progress_count
                    )
                } else {
                    format!(
                        "完成了 {} 個任務，獲得 {} XP！所有任務都完成了，太棒了！🎊",
                        completed_today, total_exp
                    )
                }
            } else {
                if in_progress_count > 0 {
                    format!(
                        "完成了 {} 個任務！還有{}個任務進行中 💪",
                        completed_today, in_progress_count
                    )
                } else {
                    format!("完成了 {} 個任務！繼續保持這個節奏 🎉", completed_today)
                }
            };
            ("今天表現出色！".to_string(), body)
        };

        Ok(json!({
            "title": title,
            "body": body,
            "icon": "/icon.svg",
            "badge": "/icon.svg",
            "tag": "evening-notification",
            "data": {
                "url": "/mission",
                "type": "evening",
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }
        }))
    }

    /// 生成自定義通知（用於用戶自定義時段）
    pub async fn generate_custom_notification(
        rb: &RBatis,
        user_id: &str,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        // 查詢待完成任務數量
        let pending_count: i64 = rb
            .query_decode(
                "SELECT COUNT(*) as count FROM task WHERE user_id = ? AND status IN (0, 1, 5)",
                vec![rbs::to_value!(user_id)],
            )
            .await
            .unwrap_or(0);

        let (title, body) = if pending_count == 0 {
            (
                "人生升級系統".to_string(),
                "目前沒有待辦任務，享受自由時光 ✨".to_string(),
            )
        } else {
            (
                "任務提醒".to_string(),
                format!("你還有 {} 個任務待完成，加油！💪", pending_count),
            )
        };

        Ok(json!({
            "title": title,
            "body": body,
            "icon": "/icon.svg",
            "badge": "/icon.svg",
            "tag": "custom-notification",
            "data": {
                "url": "/mission",
                "type": "custom",
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_generator() {
        // 基本結構測試
        // 實際測試需要資料庫連接
    }
}
