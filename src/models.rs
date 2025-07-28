/*!
 * @file models.rs
 * @brief 資料模型定義
 * @details 定義了 LifeUp 應用程式中使用的所有資料結構，包括使用者、任務、技能等模型
 */

use rbatis::crud;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, Deserializer};

// 自定義反序列化函數處理空字串的 DateTime
fn deserialize_optional_datetime<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        Some(s) if s.is_empty() => Ok(None),
        Some(s) => s.parse::<DateTime<Utc>>().map(Some).map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}

// 使用者模型
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub id: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
crud!(User{});

/**
 * @brief 任務資料模型
 * @details 表示系統中的任務，支援多種類型：主任務、支線任務、挑戰任務、每日任務等
 *          包含層次結構（父子任務）、重複性任務、狀態管理等功能
 */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: Option<String>,
    pub user_id: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    /// 任務狀態：0=待完成, 1=進行中, 2=已完成, 3=已取消, 4=已暫停
    pub status: Option<i32>,
    /// 任務優先級：0=低, 1=中, 2=高
    pub priority: Option<i32>,
    /// 任務類型：main=主任務, side=支線任務, challenge=挑戰任務, daily=每日任務
    pub task_type: Option<String>,
    /// 難度等級：1-5
    pub difficulty: Option<i32>,
    /// 經驗值獎勵
    pub experience: Option<i32>,
    /// 父任務ID（用於建立任務層次結構）
    pub parent_task_id: Option<String>,
    /// 是否為大任務：0=否, 1=是
    pub is_parent_task: Option<i32>,
    /// 任務排序順序
    pub task_order: Option<i32>,
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub due_date: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    /// 是否為重複性任務：0=否, 1=是
    pub is_recurring: Option<i32>,
    /// 重複模式：daily=每日, weekly=每週, monthly=每月
    pub recurrence_pattern: Option<String>,
    /// 開始日期
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub start_date: Option<DateTime<Utc>>,
    /// 結束日期
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub end_date: Option<DateTime<Utc>>,
    /// 完成率目標（0.0-1.0）
    pub completion_target: Option<f64>,
    /// 當前完成率（0.0-1.0）
    pub completion_rate: Option<f64>,
    /// 任務日期（用於日常子任務，格式：YYYY-MM-DD）
    pub task_date: Option<String>,
    /// 任務被取消的次數
    pub cancel_count: Option<i32>,
    /// 最後取消時間
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub last_cancelled_at: Option<DateTime<Utc>>,
    /// 週重複時的星期選擇（JSON字串格式：[1,2,3,4,5] 代表週一到週五）
    pub weekly_days: Option<String>,
    /// 月重複時的日期選擇（JSON字串格式：[1,15,28] 代表每月1日、15日、28日）
    pub monthly_days: Option<String>,
    /// 關聯的技能ID列表（JSON字串格式：["skill1","skill2"] 代表相關技能）
    pub related_skills: Option<String>,
}
crud!(Task{});

// 技能模型
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Skill {
    pub id: Option<String>,
    pub user_id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub level: Option<i32>, // 1-10 等級
    pub progress: Option<f64>, // 0.0-1.0 進度
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
crud!(Skill{});

/**
 * @brief 任務-技能關聯模型
 * @details 定義任務與技能的多對多關聯關係，包含經驗值加成等配置
 */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskSkillRelation {
    pub id: Option<String>,
    pub task_id: Option<String>,
    pub skill_id: Option<String>,
    /// 經驗值加成倍數（預設1.0，可設定0.5-2.0）
    pub experience_multiplier: Option<f64>,
    pub created_at: Option<DateTime<Utc>>,
}
crud!(TaskSkillRelation{});

// 聊天記錄模型
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: Option<String>,
    pub user_id: Option<String>,
    pub role: Option<String>, // "user" 或 "assistant"
    pub content: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}
crud!(ChatMessage{});

// 建立使用者的請求
#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub name: String,
    pub email: String,
}

// 更新使用者的請求
#[derive(Deserialize)]
pub struct UpdateUserRequest {
    pub name: Option<String>,
    pub email: Option<String>,
}

/**
 * @brief 建立任務的請求結構
 */
#[derive(Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<i32>,
    pub task_type: Option<String>, // main, side, challenge, daily
    pub difficulty: Option<i32>, // 1-5 難度等級
    pub experience: Option<i32>, // 經驗值獎勵
    pub due_date: Option<DateTime<Utc>>,
    /// 使用者ID
    pub user_id: Option<String>,
    /// 關聯的技能ID列表
    pub related_skills: Option<Vec<String>>,
}

// 更新任務的請求
#[derive(Deserialize)]
pub struct UpdateTaskRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<i32>,
    pub priority: Option<i32>,
    pub task_type: Option<String>, // main, side, challenge, daily
    pub difficulty: Option<i32>, // 1-5 難度等級
    pub experience: Option<i32>, // 經驗值獎勵
    pub due_date: Option<DateTime<Utc>>,
    /// 關聯的技能ID列表
    pub related_skills: Option<Vec<String>>,
}

// 子任務模板
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubTaskTemplate {
    pub title: String,
    pub description: Option<String>,
    pub difficulty: i32,
    pub experience: i32,
    pub order: i32,
}

// 重複性任務模板（存儲在資料庫中）
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecurringTaskTemplate {
    pub id: Option<String>,
    pub parent_task_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub difficulty: i32,
    pub experience: i32,
    pub task_order: i32,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
crud!(RecurringTaskTemplate{});

// 開始任務的請求
#[derive(Deserialize)]
pub struct StartTaskRequest {
    pub generate_subtasks: Option<bool>,
}

// 建立技能的請求
#[derive(Deserialize)]
pub struct CreateSkillRequest {
    pub name: String,
    pub description: Option<String>,
    pub level: Option<i32>,
}

// 聊天請求
#[derive(Deserialize)]
pub struct ChatRequest {
    pub message: String,
}

// 建立重複性任務的請求
#[derive(Deserialize)]
pub struct CreateRecurringTaskRequest {
    pub title: String,
    pub description: Option<String>,
    pub task_type: Option<String>,
    pub difficulty: Option<i32>,
    pub experience: Option<i32>,
    pub recurrence_pattern: String, // daily, weekly, monthly
    pub start_date: DateTime<Utc>,
    pub end_date: Option<DateTime<Utc>>,
    pub completion_target: Option<f64>, // 完成率目標
    pub subtask_templates: Vec<SubTaskTemplate>, // 子任務模板列表
    pub user_id: Option<String>,
    // 週重複時的星期選擇 (1=週一, 2=週二, ..., 7=週日)
    pub weekly_days: Option<Vec<i32>>,
    // 月重複時的日期選擇 (1-31)
    pub monthly_days: Option<Vec<i32>>,
}

// 任務進度回應
#[derive(Serialize)]
pub struct TaskProgressResponse {
    pub task_id: String,
    pub total_days: i32,
    pub completed_days: i32,
    pub completion_rate: f64,
    pub target_rate: f64,
    pub is_daily_completed: bool,
    pub remaining_days: i32,
}

// 遊戲化用戶資料模型
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserProfile {
    pub id: Option<String>,
    pub user_id: Option<String>,
    pub level: Option<i32>,
    pub experience: Option<i32>,
    pub max_experience: Option<i32>,
    pub title: Option<String>,
    pub adventure_days: Option<i32>,
    pub consecutive_login_days: Option<i32>,
    pub persona_type: Option<String>, // 'internal' 或 'external'
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
crud!(UserProfile{});

// 用戶屬性模型
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserAttributes {
    pub id: Option<String>,
    pub user_id: Option<String>,
    pub intelligence: Option<i32>, // 智力
    pub endurance: Option<i32>,    // 毅力
    pub creativity: Option<i32>,   // 創造力
    pub social: Option<i32>,       // 社交力
    pub focus: Option<i32>,        // 專注力
    pub adaptability: Option<i32>, // 適應力
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
crud!(UserAttributes{});

// 每日進度模型
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DailyProgress {
    pub id: Option<String>,
    pub user_id: Option<String>,
    pub date: Option<String>, // YYYY-MM-DD 格式
    pub completed_tasks: Option<i32>,
    pub total_tasks: Option<i32>,
    pub experience_gained: Option<i32>,
    pub attributes_gained: Option<serde_json::Value>, // 直接使用 JSON Value
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
crud!(DailyProgress{});

// 成就模型
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Achievement {
    pub id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub category: Option<String>, // 成就分類
    pub requirement_type: Option<String>, // 達成條件類型
    pub requirement_value: Option<i32>, // 達成條件數值
    pub experience_reward: Option<i32>, // 經驗值獎勵
    pub created_at: Option<DateTime<Utc>>,
}
crud!(Achievement{});

// 用戶成就關聯模型
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserAchievement {
    pub id: Option<String>,
    pub user_id: Option<String>,
    pub achievement_id: Option<String>,
    pub achieved_at: Option<DateTime<Utc>>,
    pub progress: Option<i32>, // 當前進度
}
crud!(UserAchievement{});

// 創建用戶資料請求
#[derive(Deserialize)]
pub struct CreateUserProfileRequest {
    pub level: Option<i32>,
    pub experience: Option<i32>,
    pub max_experience: Option<i32>,
    pub title: Option<String>,
    pub adventure_days: Option<i32>,
    pub consecutive_login_days: Option<i32>,
    pub persona_type: Option<String>,
}

// 更新用戶屬性請求
#[derive(Deserialize)]
pub struct UpdateUserAttributesRequest {
    pub intelligence: Option<i32>,
    pub endurance: Option<i32>,
    pub creativity: Option<i32>,
    pub social: Option<i32>,
    pub focus: Option<i32>,
    pub adaptability: Option<i32>,
}

// 今日進度回應
#[derive(Serialize)]
pub struct TodayProgressResponse {
    pub completed_tasks: i32,
    pub total_tasks: i32,
    pub experience_gained: i32,
    pub attribute_gains: serde_json::Value,
}

// 週屬性快照模型
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeeklyAttributeSnapshot {
    pub id: Option<String>,
    pub user_id: Option<String>,
    pub week_start_date: Option<String>, // YYYY-MM-DD 格式，該週的週一日期
    pub year: Option<i32>,
    pub week_number: Option<i32>, // 該年的第幾週
    pub intelligence: Option<i32>,
    pub endurance: Option<i32>,
    pub creativity: Option<i32>,
    pub social: Option<i32>,
    pub focus: Option<i32>,
    pub adaptability: Option<i32>,
    pub created_at: Option<DateTime<Utc>>,
}
crud!(WeeklyAttributeSnapshot{}); 
