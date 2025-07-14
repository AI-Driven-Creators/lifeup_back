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

// 任務模型
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: Option<String>,
    pub user_id: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<i32>, // 0: 待完成, 1: 進行中, 2: 已完成, 3: 已取消, 4: 已暫停
    pub priority: Option<i32>, // 0: 低, 1: 中, 2: 高
    pub task_type: Option<String>, // main, side, challenge, daily
    pub difficulty: Option<i32>, // 1-5 難度等級
    pub experience: Option<i32>, // 經驗值獎勵
    pub parent_task_id: Option<String>, // 父任務ID
    pub is_parent_task: Option<i32>, // 是否為大任務（0/1）
    pub task_order: Option<i32>, // 任務排序
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub due_date: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    // 重複性任務相關欄位
    pub is_recurring: Option<i32>, // 是否為重複性任務（0/1）
    pub recurrence_pattern: Option<String>, // 重複模式：daily, weekdays, weekends, weekly
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub start_date: Option<DateTime<Utc>>, // 開始日期
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub end_date: Option<DateTime<Utc>>, // 結束日期
    pub completion_target: Option<f64>, // 完成率目標（0.0-1.0）
    pub completion_rate: Option<f64>, // 當前完成率（0.0-1.0）
    pub task_date: Option<String>, // 任務日期（用於日常子任務）
    pub cancel_count: Option<i32>, // 取消次數
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub last_cancelled_at: Option<DateTime<Utc>>, // 最後取消時間
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

// 建立任務的請求
#[derive(Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<i32>,
    pub task_type: Option<String>, // main, side, challenge, daily
    pub difficulty: Option<i32>, // 1-5 難度等級
    pub experience: Option<i32>, // 經驗值獎勵
    pub due_date: Option<DateTime<Utc>>,
    pub user_id: Option<String>, // 添加 user_id 欄位
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
    pub recurrence_pattern: String, // daily, weekdays, weekends, weekly
    pub start_date: DateTime<Utc>,
    pub end_date: Option<DateTime<Utc>>,
    pub completion_target: Option<f64>, // 完成率目標
    pub subtask_templates: Vec<SubTaskTemplate>, // 子任務模板列表
    pub user_id: Option<String>,
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