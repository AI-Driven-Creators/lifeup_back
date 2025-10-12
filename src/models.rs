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
    pub password_hash: Option<String>, // 密碼哈希
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
crud!(User{});

// 建立使用者的請求
#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub name: String,
    pub email: String,
    pub password: String,
}

// 更新使用者的請求
#[derive(Deserialize)]
pub struct UpdateUserRequest {
    pub name: Option<String>,
    pub email: Option<String>,
}

// 登入請求
#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

// 登入回應
#[derive(Serialize)]
pub struct LoginResponse {
    pub user: User,
    pub message: String,
}

// ================= Additional domain models =================

// Task model
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: Option<String>,
    pub user_id: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<i32>,
    pub priority: Option<i32>,
    pub task_type: Option<String>,
    pub difficulty: Option<i32>,
    pub experience: Option<i32>,
    pub parent_task_id: Option<String>,
    pub is_parent_task: Option<i32>,
    pub task_order: Option<i32>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub due_date: Option<DateTime<Utc>>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub updated_at: Option<DateTime<Utc>>,
    pub is_recurring: Option<i32>,
    pub recurrence_pattern: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub start_date: Option<DateTime<Utc>>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub end_date: Option<DateTime<Utc>>,
    pub completion_target: Option<f64>,
    pub completion_rate: Option<f64>,
    pub task_date: Option<String>,
    pub cancel_count: Option<i32>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub last_cancelled_at: Option<DateTime<Utc>>,
    pub skill_tags: Option<Vec<String>>,
    pub career_mainline_id: Option<String>,
    pub task_category: Option<String>,
    pub attributes: Option<serde_json::Value>,  // 任務完成時獲得的屬性獎勵 {"intelligence": 5, "creativity": 3}
}
crud!(Task{});

// Skill model
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Skill {
    pub id: Option<String>,
    pub user_id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub level: Option<i32>,
    pub experience: Option<i32>,
    pub max_experience: Option<i32>,
    pub icon: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub updated_at: Option<DateTime<Utc>>,
}
crud!(Skill{});

// Chat message model
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: Option<String>,
    pub user_id: Option<String>,
    pub role: Option<String>,
    pub content: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub created_at: Option<DateTime<Utc>>,
}
crud!(ChatMessage{});

// User profile and attributes
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
    pub persona_type: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub updated_at: Option<DateTime<Utc>>,
}
crud!(UserProfile{});

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserAttributes {
    pub id: Option<String>,
    pub user_id: Option<String>,
    pub intelligence: Option<i32>,
    pub endurance: Option<i32>,
    pub creativity: Option<i32>,
    pub social: Option<i32>,
    pub focus: Option<i32>,
    pub adaptability: Option<i32>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub updated_at: Option<DateTime<Utc>>,
}
crud!(UserAttributes{});

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DailyProgress {
    pub id: Option<String>,
    pub user_id: Option<String>,
    pub date: Option<String>,
    pub completed_tasks: Option<i32>,
    pub total_tasks: Option<i32>,
    pub experience_gained: Option<i32>,
    pub attributes_gained: Option<serde_json::Value>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub updated_at: Option<DateTime<Utc>>,
}
crud!(DailyProgress{});

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecurringTaskTemplate {
    pub id: Option<String>,
    pub parent_task_id: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub difficulty: Option<i32>,
    pub experience: Option<i32>,
    pub task_order: Option<i32>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub updated_at: Option<DateTime<Utc>>,
}
crud!(RecurringTaskTemplate{});

// Achievements
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Achievement {
    pub id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub category: Option<String>,
    #[serde(deserialize_with = "deserialize_requirement_type", serialize_with = "serialize_requirement_type", default)]
    pub requirement_type: Option<AchievementRequirementType>,
    pub requirement_value: Option<i32>,
    pub experience_reward: Option<i32>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub created_at: Option<DateTime<Utc>>,
}
crud!(Achievement{});

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AchievementStats {
    pub id: Option<String>,
    pub achievement_id: Option<String>,
    pub completion_count: Option<i32>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub updated_at: Option<DateTime<Utc>>,
}
crud!(AchievementStats{});

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserAchievement {
    pub id: Option<String>,
    pub user_id: Option<String>,
    pub achievement_id: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub achieved_at: Option<DateTime<Utc>>,
    pub progress: Option<i32>,
}
crud!(UserAchievement{});

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeeklyAttributeSnapshot {
    pub id: Option<String>,
    pub user_id: Option<String>,
    pub week_start_date: Option<String>,
    pub year: Option<i32>,
    pub week_number: Option<i32>,
    pub intelligence: Option<i32>,
    pub endurance: Option<i32>,
    pub creativity: Option<i32>,
    pub social: Option<i32>,
    pub focus: Option<i32>,
    pub adaptability: Option<i32>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub created_at: Option<DateTime<Utc>>,
}
crud!(WeeklyAttributeSnapshot{});

// Coach personality
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CoachPersonalityType {
    #[serde(rename = "harsh_critic")]
    HarshCritic,
    #[serde(rename = "emotional_support")]
    EmotionalSupport,
    #[serde(rename = "analytical")]
    Analytical,
}

impl CoachPersonalityType {
    pub fn from_string(v: &str) -> Option<CoachPersonalityType> {
        match v {
            "harsh_critic" => Some(CoachPersonalityType::HarshCritic),
            "emotional_support" => Some(CoachPersonalityType::EmotionalSupport),
            "analytical" => Some(CoachPersonalityType::Analytical),
            _ => None,
        }
    }
    pub fn display_name(&self) -> &str {
        match self {
            CoachPersonalityType::HarshCritic => "森氣氣",
            CoachPersonalityType::EmotionalSupport => "小太陽",
            CoachPersonalityType::Analytical => "小書蟲",
        }
    }
    pub fn description(&self) -> &str {
        match self {
            CoachPersonalityType::HarshCritic => "直言不諱，用嚴厲的話語督促你成長",
            CoachPersonalityType::EmotionalSupport => "溫暖體貼，提供情感支持和正向鼓勵",
            CoachPersonalityType::Analytical => "理性客觀，用數據和邏輯幫你分析問題",
        }
    }
    pub fn system_prompt(&self) -> &str {
        match self {
            CoachPersonalityType::HarshCritic => "你是一位嚴厲的教練，直言不諱，促使使用者面對問題並行動。",
            CoachPersonalityType::EmotionalSupport => "你是一位溫暖的教練，給予鼓勵與支持，讓使用者感到被理解。",
            CoachPersonalityType::Analytical => "你是一位理性分析的教練，提供結構化建議與數據化分析。",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserCoachPreference {
    pub id: Option<String>,
    pub user_id: Option<String>,
    pub personality_type: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub updated_at: Option<DateTime<Utc>>,
}
crud!(UserCoachPreference{});

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetCoachPersonalityRequest {
    pub user_id: Option<String>,
    pub personality_type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CoachPersonalityResponse {
    pub personality_type: String,
    pub display_name: String,
    pub description: String,
    pub is_active: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CoachPersonalityInfo {
    pub personality_type: String,
    pub display_name: String,
    pub description: String,
    pub emoji: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AvailablePersonalitiesResponse {
    pub personalities: Vec<CoachPersonalityInfo>,
    pub current_personality: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DirectPersonalityChatRequest {
    pub message: String,
    pub personality_type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatWithPersonalityRequest {
    pub message: String,
    pub user_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatRequest {
    pub message: String,
}

// Requests for tasks and skills
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateTaskRequest {
    pub user_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<i32>,
    pub task_type: Option<String>,
    pub difficulty: Option<i32>,
    pub experience: Option<i32>,
    pub parent_task_id: Option<String>,
    pub task_order: Option<i32>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub due_date: Option<DateTime<Utc>>,
    pub task_date: Option<String>,
    pub is_recurring: Option<i32>,
    pub recurrence_pattern: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub start_date: Option<DateTime<Utc>>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub end_date: Option<DateTime<Utc>>,
    pub completion_target: Option<f64>,
    pub skill_tags: Option<Vec<String>>,
    pub attributes: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateTaskRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<i32>,
    pub priority: Option<i32>,
    pub task_type: Option<String>,
    pub difficulty: Option<i32>,
    pub experience: Option<i32>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub due_date: Option<DateTime<Utc>>,
    pub task_order: Option<i32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateSkillRequest {
    pub user_id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub level: Option<i32>,
    pub experience: Option<i32>,
    pub max_experience: Option<i32>,
    pub icon: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateSkillExperienceRequest {
    pub experience_gain: i32,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateUserExperienceRequest {
    pub experience_gain: i32,
}

// AI and career
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerateTaskRequest {
    pub description: String,
    pub user_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SaveQuizResultsRequest {
    pub values_results: serde_json::Value,
    pub interests_results: serde_json::Value,
    pub talents_results: serde_json::Value,
    pub workstyle_results: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SurveyAnswers {
    #[serde(default)]
    pub current_level: String,
    #[serde(default)]
    pub available_time: String,
    #[serde(default)]
    pub learning_styles: Vec<String>,
    #[serde(default)]
    pub timeline: String,
    pub motivation: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerateCareerTasksRequest {
    pub selected_career: String,
    pub quiz_result_id: String,
    pub survey_answers: SurveyAnswers,
    pub user_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillTag {
    pub name: String,
    pub category: String,
}

// 自定義反序列化器：將浮點數四捨五入為整數
fn deserialize_rounded_i32<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i as i32)
            } else if let Some(f) = n.as_f64() {
                Ok(f.round() as i32)
            } else {
                Err(Error::custom("無效的數字格式"))
            }
        }
        _ => Err(Error::custom("期望數字類型")),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeneratedTask {
    pub title: String,
    pub description: String,
    #[serde(deserialize_with = "deserialize_rounded_i32")]
    pub difficulty: i32,
    #[serde(deserialize_with = "deserialize_rounded_i32")]
    pub estimated_hours: i32,
    pub skill_tags: Vec<SkillTag>,
    pub resources: Vec<String>,
    pub personality_match: Option<String>,
    #[serde(default)]
    pub attributes: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeneratedTasksResponse {
    pub learning_summary: String,
    #[serde(deserialize_with = "deserialize_rounded_i32")]
    pub estimated_months: i32,
    pub personality_insights: String,
    pub main_tasks: Vec<GeneratedTask>,
    pub daily_tasks: Vec<GeneratedTask>,
    pub project_tasks: Vec<GeneratedTask>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuizResults {
    pub id: Option<String>,
    pub user_id: Option<String>,
    pub values_results: Option<String>,
    pub interests_results: Option<String>,
    pub talents_results: Option<String>,
    pub workstyle_results: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub completed_at: Option<DateTime<Utc>>,
    pub is_active: Option<i32>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub updated_at: Option<DateTime<Utc>>,
}
crud!(QuizResults{});

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CareerMainlines {
    pub id: Option<String>,
    pub user_id: Option<String>,
    pub quiz_result_id: Option<String>,
    pub selected_career: Option<String>,
    pub survey_answers: Option<String>,
    pub total_tasks_generated: Option<i32>,
    pub estimated_completion_months: Option<i32>,
    pub status: Option<String>,
    pub progress_percentage: Option<f64>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(deserialize_with = "deserialize_optional_datetime", default)]
    pub updated_at: Option<DateTime<Utc>>,
}
crud!(CareerMainlines{});