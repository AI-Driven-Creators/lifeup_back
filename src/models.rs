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
    pub status: Option<i32>, // 0: 待完成, 1: 進行中, 2: 已完成, 3: 已取消
    pub priority: Option<i32>, // 0: 低, 1: 中, 2: 高
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub due_date: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
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
    pub due_date: Option<DateTime<Utc>>,
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