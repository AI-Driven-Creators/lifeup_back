use rbatis::crud;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, Deserializer};

// 成就達成條件類型枚舉
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum AchievementRequirementType {
    #[serde(rename = "task_complete")]
    TaskComplete,           // 完成任務總數
    #[serde(rename = "consecutive_days")]
    ConsecutiveDays,        // 連續天數
    #[serde(rename = "skill_level")]
    SkillLevel,             // 技能等級達成
    #[serde(rename = "total_completions")]
    TotalCompletions,       // 總完成次數
    #[serde(rename = "streak_recovery")]
    StreakRecovery,         // 從失敗中恢復
    #[serde(rename = "learning_task_complete")]
    LearningTaskComplete,   // 學習任務完成
    #[serde(rename = "intelligence_attribute")]
    IntelligenceAttribute,  // 智力屬性達成
    #[serde(rename = "endurance_attribute")]
    EnduranceAttribute,     // 毅力屬性達成
    #[serde(rename = "creativity_attribute")]
    CreativityAttribute,    // 創造力屬性達成
    #[serde(rename = "social_attribute")]
    SocialAttribute,        // 社交力屬性達成
    #[serde(rename = "focus_attribute")]
    FocusAttribute,         // 專注力屬性達成
    #[serde(rename = "adaptability_attribute")]
    AdaptabilityAttribute,  // 適應力屬性達成
}

impl AchievementRequirementType {
    // 從字符串轉換為枚舉
    pub fn from_string(value: &str) -> Option<AchievementRequirementType> {
        match value {
            "task_complete" => Some(AchievementRequirementType::TaskComplete),
            "consecutive_days" => Some(AchievementRequirementType::ConsecutiveDays),
            "skill_level" => Some(AchievementRequirementType::SkillLevel),
            "total_completions" => Some(AchievementRequirementType::TotalCompletions),
            "streak_recovery" => Some(AchievementRequirementType::StreakRecovery),
            "learning_task_complete" => Some(AchievementRequirementType::LearningTaskComplete),
            "intelligence_attribute" => Some(AchievementRequirementType::IntelligenceAttribute),
            "endurance_attribute" => Some(AchievementRequirementType::EnduranceAttribute),
            "creativity_attribute" => Some(AchievementRequirementType::CreativityAttribute),
            "social_attribute" => Some(AchievementRequirementType::SocialAttribute),
            "focus_attribute" => Some(AchievementRequirementType::FocusAttribute),
            "adaptability_attribute" => Some(AchievementRequirementType::AdaptabilityAttribute),
            _ => None,
        }
    }

    // 轉換為字符串
    pub fn to_string(&self) -> &'static str {
        match self {
            AchievementRequirementType::TaskComplete => "task_complete",
            AchievementRequirementType::ConsecutiveDays => "consecutive_days",
            AchievementRequirementType::SkillLevel => "skill_level",
            AchievementRequirementType::TotalCompletions => "total_completions",
            AchievementRequirementType::StreakRecovery => "streak_recovery",
            AchievementRequirementType::LearningTaskComplete => "learning_task_complete",
            AchievementRequirementType::IntelligenceAttribute => "intelligence_attribute",
            AchievementRequirementType::EnduranceAttribute => "endurance_attribute",
            AchievementRequirementType::CreativityAttribute => "creativity_attribute",
            AchievementRequirementType::SocialAttribute => "social_attribute",
            AchievementRequirementType::FocusAttribute => "focus_attribute",
            AchievementRequirementType::AdaptabilityAttribute => "adaptability_attribute",
        }
    }

    // 獲取所有有效的字符串值
    pub fn all_valid_strings() -> Vec<&'static str> {
        vec![
            "task_complete",
            "consecutive_days", 
            "skill_level",
            "total_completions",
            "streak_recovery",
            "learning_task_complete",
            "intelligence_attribute",
            "endurance_attribute",
            "creativity_attribute",
            "social_attribute",
            "focus_attribute",
            "adaptability_attribute",
        ]
    }
}

// 任務狀態枚舉
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending = 0,          // 待處理
    InProgress = 1,       // 進行中
    Completed = 2,        // 已完成
    Cancelled = 3,        // 已取消
    Paused = 4,           // 已暫停
    DailyInProgress = 5,  // 每日任務進行中
    DailyCompleted = 6,   // 每日任務已完成
    DailyNotCompleted = 7, // 每日任務未完成
}

impl TaskStatus {
    // 從數值轉換為狀態
    pub fn from_i32(value: i32) -> Option<TaskStatus> {
        match value {
            0 => Some(TaskStatus::Pending),
            1 => Some(TaskStatus::InProgress),
            2 => Some(TaskStatus::Completed),
            3 => Some(TaskStatus::Cancelled),
            4 => Some(TaskStatus::Paused),
            5 => Some(TaskStatus::DailyInProgress),
            6 => Some(TaskStatus::DailyCompleted),
            7 => Some(TaskStatus::DailyNotCompleted),
            _ => None,
        }
    }

    // 轉換為數值
    pub fn to_i32(&self) -> i32 {
        match self {
            TaskStatus::Pending => 0,
            TaskStatus::InProgress => 1,
            TaskStatus::Completed => 2,
            TaskStatus::Cancelled => 3,
            TaskStatus::Paused => 4,
            TaskStatus::DailyInProgress => 5,
            TaskStatus::DailyCompleted => 6,
            TaskStatus::DailyNotCompleted => 7,
        }
    }

    // 轉換為字符串
    pub fn to_string(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::Completed => "completed",
            TaskStatus::Cancelled => "cancelled",
            TaskStatus::Paused => "paused",
            TaskStatus::DailyInProgress => "daily_in_progress",
            TaskStatus::DailyCompleted => "daily_completed",
            TaskStatus::DailyNotCompleted => "daily_not_completed",
        }
    }

    // 從字符串轉換為狀態
    pub fn from_string(value: &str) -> Option<TaskStatus> {
        match value {
            "pending" => Some(TaskStatus::Pending),
            "in_progress" => Some(TaskStatus::InProgress),
            "completed" => Some(TaskStatus::Completed),
            "cancelled" => Some(TaskStatus::Cancelled),
            "paused" => Some(TaskStatus::Paused),
            "daily_in_progress" => Some(TaskStatus::DailyInProgress),
            "daily_completed" => Some(TaskStatus::DailyCompleted),
            "daily_not_completed" => Some(TaskStatus::DailyNotCompleted),
            _ => None,
        }
    }
}

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

// 自定義反序列化函數處理 requirement_type 字段
fn deserialize_requirement_type<'de, D>(deserializer: D) -> Result<Option<AchievementRequirementType>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        Some(s) if s.is_empty() => Ok(None),
        Some(s) => match AchievementRequirementType::from_string(&s) {
            Some(req_type) => Ok(Some(req_type)),
            None => Err(serde::de::Error::custom(format!("Unknown requirement type: {}", s))),
        },
        None => Ok(None),
    }
}

// 自定義序列化函數處理 requirement_type 字段
fn serialize_requirement_type<S>(req_type: &Option<AchievementRequirementType>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match req_type {
        Some(req_type) => serializer.serialize_str(req_type.to_string()),
        None => serializer.serialize_none(),
    }
}

// 自定義反序列化函數處理skill_tags字段
fn deserialize_skill_tags<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<serde_json::Value> = Option::deserialize(deserializer)?;
    match opt {
        Some(value) => {
            match value {
                serde_json::Value::String(s) => {
                    // 嘗試解析 JSON 字符串為數組
                    if s.is_empty() {
                        Ok(None)
                    } else {
                        match serde_json::from_str::<Vec<String>>(&s) {
                            Ok(vec) => {
                                Ok(Some(vec))
                            }
                            Err(_) => {
                                // 如果解析失敗，將字符串作為單個元素
                                Ok(Some(vec![s]))
                            }
                        }
                    }
                },
                serde_json::Value::Array(arr) => {
                    // 直接處理數組
                    let string_vec: Vec<String> = arr.into_iter()
                        .map(|v| match v {
                            serde_json::Value::String(s) => s,
                            _ => v.to_string()
                        })
                        .collect();
                    Ok(Some(string_vec))
                },
                serde_json::Value::Null => Ok(None),
                _ => {
                    // 其他類型轉換為字符串作為單個元素
                    Ok(Some(vec![value.to_string()]))
                },
            }
        },
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
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default, deserialize_with = "deserialize_optional_datetime")]
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
    #[serde(default, deserialize_with = "deserialize_skill_tags")]
    pub skill_tags: Option<Vec<String>>, // 相關技能標籤，JSON格式儲存["Vue.js", "JavaScript"]
}
crud!(Task{});

// 技能模型
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Skill {
    pub id: Option<String>,
    pub user_id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>, // 'technical' 或 'soft'
    pub level: Option<i32>, // 1-10 等級
    pub experience: Option<i32>, // 當前經驗值
    pub max_experience: Option<i32>, // 最大經驗值
    pub icon: Option<String>, // emoji 圖標
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
    pub skill_tags: Option<Vec<String>>, // 技能標籤陣列
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
    pub category: Option<String>, // 'technical' 或 'soft'
    pub level: Option<i32>,
    pub experience: Option<i32>,
    pub max_experience: Option<i32>,
    pub icon: Option<String>,
}

// 更新技能經驗值的請求
#[derive(Deserialize)]
pub struct UpdateSkillExperienceRequest {
    pub experience_gain: i32, // 增加的經驗值
    pub reason: Option<String>, // 獲得經驗值的原因（如：完成任務）
}

// 聊天請求
#[derive(Deserialize)]
pub struct ChatRequest {
    pub message: String,
}

// AI 生成任務請求
#[derive(Deserialize)]
pub struct GenerateTaskRequest {
    pub description: String,
    pub user_id: Option<String>,
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
    pub missed_days: i32, // 缺席天數
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
    #[serde(serialize_with = "serialize_requirement_type", deserialize_with = "deserialize_requirement_type")]
    pub requirement_type: Option<AchievementRequirementType>, // 達成條件類型
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