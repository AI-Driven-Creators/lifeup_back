use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;
use chrono::Utc;
use rbatis::RBatis;
use crate::models::AchievementRequirementType;
use crate::ai_tasks::AnalysisDirection;
use crate::config::AIConfig;
use crate::behavior_analytics::{UserBehaviorSummary, BehaviorAnalytics};

// 格式化 AI 輸出為單行日誌
fn format_ai_output(text: &str) -> String {
    text.replace("\\n", " ")
        .replace("\\\"", "\"")
        .chars()
        .filter(|c| !c.is_control() || *c == ' ')
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

// 專家數據庫
pub fn get_expert_database() -> Vec<Expert> {
    vec![
        Expert {
            name: "資深英文教學老師".to_string(),
            description: "擁有15年英語教學經驗，專精於語言學習方法和技巧".to_string(),
            expertise_areas: vec!["英語學習".to_string(), "語言教學".to_string(), "口語練習".to_string(), "文法學習".to_string()],
            emoji: "📚".to_string(),
        },
        Expert {
            name: "程式設計導師".to_string(),
            description: "資深軟體工程師，專精於多種程式語言和開發框架".to_string(),
            expertise_areas: vec!["程式設計".to_string(), "軟體開發".to_string(), "演算法".to_string(), "系統設計".to_string()],
            emoji: "💻".to_string(),
        },
        Expert {
            name: "健身教練".to_string(),
            description: "專業健身教練，專精於運動訓練和健康管理".to_string(),
            expertise_areas: vec!["健身訓練".to_string(), "運動計劃".to_string(), "健康管理".to_string(), "營養搭配".to_string()],
            emoji: "💪".to_string(),
        },
        Expert {
            name: "理財規劃師".to_string(),
            description: "專業理財顧問，專精於投資理財和財務規劃".to_string(),
            expertise_areas: vec!["理財規劃".to_string(), "投資策略".to_string(), "財務管理".to_string(), "儲蓄計劃".to_string()],
            emoji: "💰".to_string(),
        },
        Expert {
            name: "時間管理顧問".to_string(),
            description: "專業時間管理顧問，專精於效率提升和目標達成".to_string(),
            expertise_areas: vec!["時間管理".to_string(), "效率提升".to_string(), "目標設定".to_string(), "習慣養成".to_string()],
            emoji: "⏰".to_string(),
        },
        Expert {
            name: "創意設計師".to_string(),
            description: "資深設計師，專精於創意思維和視覺設計".to_string(),
            expertise_areas: vec!["創意設計".to_string(), "視覺設計".to_string(), "品牌設計".to_string(), "UI/UX設計".to_string()],
            emoji: "🎨".to_string(),
        },
        Expert {
            name: "心理諮商師".to_string(),
            description: "專業心理諮商師，專精於情緒管理和心理調適".to_string(),
            expertise_areas: vec!["心理諮商".to_string(), "情緒管理".to_string(), "壓力調適".to_string(), "人際關係".to_string()],
            emoji: "🧠".to_string(),
        },
        Expert {
            name: "廚藝導師".to_string(),
            description: "專業廚師，專精於各種料理技巧和營養搭配".to_string(),
            expertise_areas: vec!["烹飪技巧".to_string(), "料理製作".to_string(), "營養搭配".to_string(), "食材選擇".to_string()],
            emoji: "👨‍🍳".to_string(),
        },
        Expert {
            name: "音樂老師".to_string(),
            description: "專業音樂教師，專精於樂器演奏和音樂理論".to_string(),
            expertise_areas: vec!["音樂學習".to_string(), "樂器演奏".to_string(), "音樂理論".to_string(), "聲樂訓練".to_string()],
            emoji: "🎵".to_string(),
        },
        Expert {
            name: "學習方法顧問".to_string(),
            description: "教育心理學專家，專精於學習方法和記憶技巧".to_string(),
            expertise_areas: vec!["學習方法".to_string(), "記憶技巧".to_string(), "考試準備".to_string(), "知識管理".to_string()],
            emoji: "📖".to_string(),
        },
    ]
}

// OpenAI API 請求結構
#[derive(Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<ChatMessage>,
    // temperature 新模型只支持默認值 1，因此不再傳遞
    max_completion_tokens: i32,
    response_format: ResponseFormat,
}

#[derive(Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    format_type: String,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

// OpenAI API 回應結構
#[derive(Deserialize)]
struct OpenAIResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChatMessage,
}

// 專家信息結構
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Expert {
    pub name: String,
    pub description: String,
    pub expertise_areas: Vec<String>,
    pub emoji: String,
}

// 專家匹配結果
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExpertMatch {
    pub expert: Expert,
    pub confidence: f64,
    pub ai_expert_name: String,
    pub ai_expert_description: String,
}

// AI 生成的任務結構（簡化版）
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AIGeneratedTask {
    pub title: String,
    pub description: Option<String>,
    pub task_type: String,
    pub priority: i32,
    pub difficulty: i32,
    pub experience: i32,
    pub due_date: Option<String>,
    pub is_recurring: bool,
    pub recurrence_pattern: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub completion_target: Option<f64>,
}

// AI 生成的任務計劃（包含主任務和子任務）
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AIGeneratedTaskPlan {
    pub main_task: AIGeneratedTask,
    pub subtasks: Vec<AIGeneratedTask>,
}

// AI 生成的成就結構
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AIGeneratedAchievement {
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub category: String,
    pub requirement_type: String,
    pub requirement_value: i32,
    pub experience_reward: i32,
}

// ===== 辅助函数：构建基于统计摘要的 Prompt =====

/// 根据用户行为摘要构建成就生成的 prompt
fn build_achievement_prompt_from_summary(summary: &UserBehaviorSummary) -> String {
    // 格式化分类统计
    let top_categories: Vec<String> = summary.top_categories
        .iter()
        .map(|c| format!(
            "{}（完成{}次，完成率{:.0}%，平均难度{:.1}）",
            c.category,
            c.completed_count,
            c.completion_rate * 100.0,
            c.avg_difficulty
        ))
        .collect();

    // 格式化最近完成的任务
    let recent_tasks: Vec<String> = summary.recent_completions
        .iter()
        .take(10)
        .map(|t| format!("  - {}: {}", t.completion_date.split('T').next().unwrap_or(&t.completion_date), t.title))
        .collect();

    // 格式化最近取消的任务
    let recent_cancellations: Vec<String> = summary.recent_cancellations
        .iter()
        .take(5)
        .map(|t| format!("  - {}: {}", t.completion_date.split('T').next().unwrap_or(&t.completion_date), t.title))
        .collect();

    // 格式化里程碑
    let milestones: Vec<String> = summary.milestone_events
        .iter()
        .map(|m| format!("  - {}: {}", m.event_type, m.description))
        .collect();

    format!(
        r#"你是一個成就設計助手。根據用戶的行為數據分析，生成個性化且具有激勵性的成就。

【用戶統計數據】
- 總完成任務：{total_completed} 次
- 總取消任務：{total_cancelled} 次
- 待處理任務：{total_pending} 個
- 最長連續記錄：{longest_streak} 天（{streak_task}）
- 當前連續：{current_streak} 天
- 近 30 天活躍：{active_30} 天
- 總經驗值：{total_exp}

【任務分類分布】（Top {cat_count}）
{categories}

【最近完成任務】（最近 {recent_count} 條樣本）
{recent_tasks}

【最近取消任務】（最近 {cancel_count} 條樣本）
{recent_cancellations}

【里程碑事件】
{milestones}

【已解鎖成就】
{achievements}

**設計原則：**
- 成就名稱要幽默且具體，如「成為英語字典」「跑火入魔」
- 基於用戶實際行為模式生成，不要憑空想像
- 考慮用戶的優勢領域（完成率高的分類）和潛力領域
- 避免與現有成就重複
- 如果有明顯的連續記錄，可以考慮相關的持續性成就

**成就分類：**
- task_mastery: 任務精通類
- consistency: 持續性類
- challenge_overcome: 克服挑戰類
- skill_development: 技能發展類

**達成條件類型：**
- consecutive_days: 連續天數
- total_completions: 總完成次數
- task_complete: 完成任務總數
- streak_recovery: 從失敗中恢復
- skill_level: 技能等級
- learning_task_complete: 學習任務完成
- intelligence_attribute: 智力屬性達成
- endurance_attribute: 毅力屬性達成
- creativity_attribute: 創造力屬性達成
- social_attribute: 社交力屬性達成
- focus_attribute: 專注力屬性達成
- adaptability_attribute: 適應力屬性達成

**經驗值獎勵計算：**
- 基於難度：簡單成就 50-100，中等 100-200，困難 200-500

請以 JSON 格式回應：
{{
  "name": "成就名稱（幽默且具體）",
  "description": "成就描述（選填）",
  "icon": "圖標名稱（選填）",
  "category": "成就分類",
  "requirement_type": "達成條件類型",
  "requirement_value": 數值,
  "experience_reward": 經驗值獎勵
}}"#,
        total_completed = summary.total_tasks_completed,
        total_cancelled = summary.total_tasks_cancelled,
        total_pending = summary.total_tasks_pending,
        longest_streak = summary.longest_streak.days,
        streak_task = summary.longest_streak.task_title,
        current_streak = summary.current_streak.days,
        active_30 = summary.active_days_last_30,
        total_exp = summary.total_experience,
        cat_count = summary.top_categories.len(),
        categories = if top_categories.is_empty() { "  （暫無數據）".to_string() } else { top_categories.join("\n") },
        recent_count = summary.recent_completions.len().min(10),
        recent_tasks = if recent_tasks.is_empty() { "  （暫無數據）".to_string() } else { recent_tasks.join("\n") },
        cancel_count = summary.recent_cancellations.len().min(5),
        recent_cancellations = if recent_cancellations.is_empty() { "  （暫無數據）".to_string() } else { recent_cancellations.join("\n") },
        milestones = if milestones.is_empty() { "  （暫無數據）".to_string() } else { milestones.join("\n") },
        achievements = if summary.unlocked_achievements.is_empty() { "（暫無）".to_string() } else { summary.unlocked_achievements.join("、") },
    )
}

// AI 服務 trait
#[async_trait::async_trait]
pub trait AIService {
    async fn generate_achievement_from_text(&self, user_input: &str) -> Result<AIGeneratedAchievement>;
    async fn generate_achievement_from_user_id(&self, rb: &RBatis, user_id: &str) -> Result<AIGeneratedAchievement>;
    async fn generate_task_preview(&self, prompt: &str) -> Result<String>;
    async fn generate_task_preview_with_history(&self, system_prompt: &str, history: &[(String, String)], current_message: &str) -> Result<String>;
    async fn generate_task_from_text(&self, user_input: &str) -> Result<AIGeneratedTask>;
    async fn match_expert_for_task(&self, user_input: &str) -> Result<ExpertMatch>;
    async fn generate_task_with_expert(&self, user_input: &str, expert_match: &ExpertMatch) -> Result<AIGeneratedTaskPlan>;
    async fn analyze_with_expert(&self, user_input: &str, expert_name: &str, expert_description: &str, analysis_type: &str) -> Result<String>;

    // 新增：使用指定模型生成文字回應
    async fn generate_with_model(&self, model: &str, prompt: &str) -> Result<String>;
}

// OpenRouter API 請求結構
#[derive(Serialize)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_completion_tokens: i32,
    response_format: ResponseFormat,
}

// OpenRouter API 回應結構
#[derive(Deserialize)]
struct OpenRouterResponse {
    choices: Vec<Choice>,
}

pub struct OpenAIService {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl OpenAIService {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl AIService for OpenAIService {

    async fn generate_achievement_from_text(&self, user_input: &str) -> Result<AIGeneratedAchievement> {
        let system_prompt = r#"你是一個成就設計助手。根據用戶的行為數據分析，生成個性化且具有激勵性的成就。

請仔細分析用戶的：
1. 已有成就列表
2. 任務完成狀況
3. 任務取消/失敗狀況
4. 待完成任務

**設計原則：**
- 成就名稱要幽默且具體，如「成為英語字典」「跑火入魔」
- 基於用戶實際行為模式生成，不要憑空想像
- 如果用戶在某領域已有基礎成就且表現優秀，可考慮升級版成就
- 避免與現有成就重複

**成就分類：**
- task_mastery: 任務精通類
- consistency: 持續性類  
- challenge_overcome: 克服挑戰類
- skill_development: 技能發展類

**達成條件類型：**
- consecutive_days: 連續天數
- total_completions: 總完成次數  
- task_complete: 完成任務總數
- streak_recovery: 從失敗中恢復
- skill_level: 技能等級
- learning_task_complete: 學習任務完成
- intelligence_attribute: 智力屬性達成
- endurance_attribute: 毅力屬性達成  
- creativity_attribute: 創造力屬性達成
- social_attribute: 社交力屬性達成
- focus_attribute: 專注力屬性達成
- adaptability_attribute: 適應力屬性達成

**經驗值獎勵計算：**
- 基於難度：簡單成就 50-100，中等 100-200，困難 200-500

請以 JSON 格式回應：
{
  "name": "成就名稱（幽默且具體）",
  "description": "成就描述（選填）", 
  "icon": "圖標名稱（選填）",
  "category": "成就分類",
  "requirement_type": "達成條件類型",
  "requirement_value": 數值,
  "experience_reward": 經驗值獎勵
}

範例：
輸入：使用者連續完成「背英語單字」30天，但經常取消「運動」任務
輸出：
{
  "name": "成為英語字典",
  "description": "連續30天完成背英語單字，詞彙量已經超越一般字典",
  "icon": "📖",
  "category": "task_mastery",
  "requirement_type": "consecutive_days", 
  "requirement_value": 30,
  "experience_reward": 300
}"#;

        let user_message = format!("請根據以下用戶行為數據生成合適的成就：{}", user_input);

        let request = OpenAIRequest {
            model: self.model.clone().to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user_message.clone(),
                },
            ],
            max_completion_tokens: 4000,
            response_format: ResponseFormat {
                format_type: "json_object".to_string(),
            },
        };

        if let Ok(body) = serde_json::to_string(&request) {
            log::info!("[AI INPUT][generate_achievement_from_text] {}", format_ai_output(&body));
        }

        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;
        log::info!("[AI OUTPUT][generate_achievement_from_text] {}", format_ai_output(&response_text));

        if !status.is_success() {
            return Err(anyhow::anyhow!("OpenAI API 錯誤 ({}): {}", status, response_text));
        }

        let openai_response: OpenAIResponse = serde_json::from_str(&response_text)?;

        if let Some(choice) = openai_response.choices.first() {
            let achievement_json = &choice.message.content;
            let generated_achievement: AIGeneratedAchievement = serde_json::from_str(achievement_json)?;

            validate_generated_achievement(&generated_achievement)?;

            Ok(generated_achievement)
        } else {
            Err(anyhow::anyhow!("OpenAI 未返回有效回應"))
        }
    }

    async fn generate_task_preview(&self, prompt: &str) -> Result<String> {
        let request = serde_json::json!({
            "model": self.model.clone(),
            "messages": [
                {
                    "role": "system",
                    "content": "你是一個充滿活力和鼓勵的任務助手。用積極正面的語氣為用戶介紹任務，讓他們感到興奮和有動力去完成。"
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "max_completion_tokens": 4000
        });

        if let Ok(body) = serde_json::to_string(&request) {
            log::info!("[AI INPUT][generate_task_preview] {}", format_ai_output(&body));
        }

        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;
        log::info!("[AI OUTPUT][generate_task_preview] {}", format_ai_output(&response_text));

        if !status.is_success() {
            return Err(anyhow::anyhow!("OpenAI API 錯誤 ({}): {}", status, response_text));
        }

        let openai_response: OpenAIResponse = serde_json::from_str(&response_text)?;
        
        if let Some(choice) = openai_response.choices.first() {
            Ok(choice.message.content.clone())
        } else {
            Err(anyhow::anyhow!("OpenAI 未返回有效回應"))
        }
    }

    async fn generate_task_preview_with_history(&self, system_prompt: &str, history: &[(String, String)], current_message: &str) -> Result<String> {
        let mut messages = vec![];

        // 先添加歷史對話
        for (user_msg, assistant_msg) in history {
            messages.push(serde_json::json!({
                "role": "user",
                "content": user_msg
            }));
            messages.push(serde_json::json!({
                "role": "assistant",
                "content": assistant_msg
            }));
        }

        // 然後添加系統提示詞
        messages.push(serde_json::json!({
            "role": "system",
            "content": system_prompt
        }));

        // 最後添加當前用戶訊息
        messages.push(serde_json::json!({
            "role": "user",
            "content": current_message
        }));

        let request = serde_json::json!({
            "model": self.model.clone(),
            "messages": messages,
            "max_completion_tokens": 4000
        });

        if let Ok(body) = serde_json::to_string(&request) {
            log::info!("[AI INPUT][generate_task_preview_with_history] {}", format_ai_output(&body));
        }

        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;
        log::info!("[AI OUTPUT][generate_task_preview_with_history] {}", format_ai_output(&response_text));

        if !status.is_success() {
            return Err(anyhow::anyhow!("OpenAI API 錯誤 ({}): {}", status, response_text));
        }

        let openai_response: OpenAIResponse = serde_json::from_str(&response_text)?;
        
        if let Some(choice) = openai_response.choices.first() {
            Ok(choice.message.content.clone())
        } else {
            Err(anyhow::anyhow!("OpenRouter 未返回有效回應"))
        }
    }

    async fn generate_task_from_text(&self, user_input: &str) -> Result<AIGeneratedTask> {
        // 獲取當前時間並格式化
        let now = Utc::now();
        let current_time_str = now.to_rfc3339(); // e.g., "2025-08-17T12:00:00Z"

        let system_prompt = format!(
            r#"你是一個任務規劃助手。根據用戶的自然語言描述，生成結構化的任務資料。

**重要：現在的時間是 {}。** 在生成任何與日期相關的欄位（如 start_date, due_date）時，請以此時間為基準進行推算。

**截止日期生成規則：**
- 對於大部分任務，你都應該設定一個合理的截止日期
- 短期任務（1-3天內完成）：設定1-3天後的截止日期
- 中期任務（1-2週完成）：設定1-2週後的截止日期
- 長期任務（1個月以上）：設定1-3個月後的截止日期
- 只有對於沒有明確時間限制的習慣類任務才設定 due_date 為 null
- 如果用戶明確提到時間（如"明天"、"下週"、"月底"），一定要根據當前時間計算對應的截止日期

任務類型說明：
- main: 主要任務（重要的長期目標，通常設定較長的截止日期）
- side: 副線任務（次要的短期任務，通常設定較短的截止日期）
- challenge: 挑戰任務（困難且有成就感的任務，根據具體內容設定截止日期）
- daily: 日常任務（例行性任務，重複性任務通常不設定截止日期）

優先級：0=低, 1=中, 2=高
難度：1-5（1=非常簡單, 5=非常困難）
經驗值：根據難度和重要性計算，通常是 difficulty * 20 + priority * 10

重複模式（僅限日常任務）：
- daily: 每天
- weekdays: 工作日
- weekends: 週末
- weekly: 每週

請以 JSON 格式回應，包含以下欄位：
{{
  "title": "任務標題",
  "description": "任務描述（選填）",
  "task_type": "main/side/challenge/daily",
  "priority": 0-2,
  "difficulty": 1-5,
  "experience": 經驗值,
  "due_date": "截止日期（ISO 8601格式，大多數情況下都應該設定）",
  "is_recurring": false,
  "recurrence_pattern": null,
  "start_date": null,
  "end_date": null,
  "completion_target": null
}}

如果是重複性任務，請設置：
- is_recurring: true
- recurrence_pattern: "daily/weekdays/weekends/weekly"
- start_date: 開始日期（ISO 8601格式）
- completion_target: 0.8（預設80%完成率目標）
- due_date: null（重複性任務通常不設定單一截止日期）

範例輸入："學習Python程式設計"
範例輸出：
{{
  "title": "學習Python程式設計",
  "description": "系統性學習Python程式語言基礎知識",
  "task_type": "main",
  "priority": 2,
  "difficulty": 3,
  "experience": 80,
  "due_date": "2024-02-15T23:59:59Z",
  "is_recurring": false,
  "recurrence_pattern": null,
  "start_date": null,
  "end_date": null,
  "completion_target": null
}}

範例輸入："明天交報告"
範例輸出：
{{
  "title": "完成並提交報告",
  "description": "整理資料並完成報告撰寫",
  "task_type": "side",
  "priority": 2,
  "difficulty": 2,
  "experience": 60,
  "due_date": "2024-01-02T18:00:00Z",
  "is_recurring": false,
  "recurrence_pattern": null,
  "start_date": null,
  "end_date": null,
  "completion_target": null
}}"#,
            current_time_str
        );

        let user_message = format!("請根據以下描述生成任務：{}", user_input);

        let request = OpenAIRequest {
            model: self.model.clone().to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user_message,
                },
            ],
            max_completion_tokens: 500,
            response_format: ResponseFormat {
                format_type: "json_object".to_string(),
            },
        };

        if let Ok(body) = serde_json::to_string(&request) {
            log::info!("[AI INPUT][generate_task_from_text] {}", format_ai_output(&body));
        }

        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;
        log::info!("[AI OUTPUT][generate_task_from_text] {}", format_ai_output(&response_text));

        if !status.is_success() {
            return Err(anyhow::anyhow!("OpenAI API 錯誤 ({}): {}", status, response_text));
        }

        let openai_response: OpenAIResponse = serde_json::from_str(&response_text)?;
        
        if let Some(choice) = openai_response.choices.first() {
            let task_json = &choice.message.content;
            log::info!("[AI OUTPUT][generate_task_from_text] {}", format_ai_output(&task_json));
            let generated_task: AIGeneratedTask = serde_json::from_str(task_json)?;
            
            // 驗證生成的任務
            validate_generated_task(&generated_task)?;
            
            Ok(generated_task)
        } else {
            Err(anyhow::anyhow!("OpenAI 未返回有效回應"))
        }
    }

    // 新方法：基于用户 ID 生成成就（使用统计摘要）
    async fn generate_achievement_from_user_id(&self, rb: &RBatis, user_id: &str) -> Result<AIGeneratedAchievement> {
        // 1. 生成用户行为摘要
        log::info!("为用户 {} 生成行为摘要...", user_id);
        let summary = BehaviorAnalytics::generate_summary(rb, user_id).await?;
        log::info!("行为摘要生成完成：完成{}个任务，最长连续{}天", summary.total_tasks_completed, summary.longest_streak.days);

        // 2. 构建基于摘要的 prompt
        let system_prompt = build_achievement_prompt_from_summary(&summary);

        // 3. 调用 AI 生成成就
        let request = OpenAIRequest {
            model: self.model.clone().to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: "請基於以上用戶數據，生成一個最合適的成就。".to_string(),
                },
            ],
            max_completion_tokens: 4000,
            response_format: ResponseFormat {
                format_type: "json_object".to_string(),
            },
        };

        if let Ok(body) = serde_json::to_string(&request) {
            log::info!("[AI INPUT][generate_achievement_from_user_id] {}", format_ai_output(&body));
        }

        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;
        log::info!("[AI OUTPUT][generate_achievement_from_user_id] {}", format_ai_output(&response_text));

        if !status.is_success() {
            return Err(anyhow::anyhow!("OpenAI API 錯誤 ({}): {}", status, response_text));
        }

        let openai_response: OpenAIResponse = serde_json::from_str(&response_text)?;

        if let Some(choice) = openai_response.choices.first() {
            let achievement_json = &choice.message.content;
            let generated_achievement: AIGeneratedAchievement = serde_json::from_str(achievement_json)?;

            // 驗證生成的成就
            validate_generated_achievement(&generated_achievement)?;

            Ok(generated_achievement)
        } else {
            Err(anyhow::anyhow!("OpenAI 未返回有效回應"))
        }
    }

    async fn match_expert_for_task(&self, user_input: &str) -> Result<ExpertMatch> {
        let experts = get_expert_database();
        
        // 構建專家匹配的提示詞
        let expert_list = experts.iter()
            .enumerate()
            .map(|(i, expert)| {
                format!("{}. {} ({}) - 專精領域: {}", 
                    i + 1, 
                    expert.name, 
                    expert.emoji,
                    expert.expertise_areas.join("、")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let system_prompt = format!(
            r#"你是一個專家匹配助手。根據用戶的任務描述，從以下專家列表中選擇最適合的專家。

可用專家列表：
{}

請分析用戶的任務描述，選擇最適合的專家，並提供匹配理由。

回應格式（JSON）：
{{
  "expert_name": "專家的完整名稱",
  "expert_description": "專家的詳細描述",
  "confidence": 匹配信心度（0.0-1.0）
}}

選擇原則：
1. 根據任務的核心領域選擇專家
2. 考慮專家的專業領域是否與任務匹配
3. 如果沒有完全匹配的專家，選擇最接近的
4. 信心度基於匹配程度：完全匹配=1.0，部分匹配=0.6-0.8，勉強匹配=0.3-0.5"#,
            expert_list
        );

        log::info!("[AI INPUT][match_expert_for_task] {}", user_input);

        let request = OpenAIRequest {
            model: self.model.clone().to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: (&user_input).to_string(),
                },
            ],
            max_completion_tokens: 500,
            response_format: ResponseFormat {
                format_type: "json_object".to_string(),
            },
        };

        if let Ok(body) = serde_json::to_string(&request) {
            log::info!("[AI INPUT][match_expert_for_task_payload] {}", format_ai_output(&body));
        }

        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;
        log::info!("[AI OUTPUT][match_expert_for_task] {}", format_ai_output(&response_text));

        if !status.is_success() {
            return Err(anyhow::anyhow!("OpenAI API 錯誤 ({}): {}", status, response_text));
        }

        let openai_response: OpenAIResponse = serde_json::from_str(&response_text)?;

        if let Some(choice) = openai_response.choices.first() {
            let match_json = &choice.message.content;
            let match_result: serde_json::Value = serde_json::from_str(match_json)?;
            
            let expert_name = match_result["expert_name"].as_str()
                .ok_or_else(|| anyhow::anyhow!("無效的專家名稱"))?.to_string();
            
            let expert_description = match_result["expert_description"].as_str()
                .ok_or_else(|| anyhow::anyhow!("無效的專家描述"))?.to_string();
            
            let confidence = match_result["confidence"].as_f64()
                .ok_or_else(|| anyhow::anyhow!("無效的信心度"))?;

            // 直接使用AI返回的專家信息，創建虛擬專家對象
            let virtual_expert = Expert {
                name: expert_name.clone(),
                description: expert_description.clone(),
                expertise_areas: vec!["AI匹配".to_string()],
                emoji: "🤖".to_string(),
            };

            Ok(ExpertMatch {
                expert: virtual_expert,
                confidence,
                ai_expert_name: expert_name,
                ai_expert_description: expert_description,
            })
        } else {
            Err(anyhow::anyhow!("OpenAI 未返回有效回應"))
        }
    }

    async fn generate_task_with_expert(&self, user_input: &str, expert_match: &ExpertMatch) -> Result<AIGeneratedTaskPlan> {
        let now = Utc::now();
        let current_time_str = now.to_rfc3339();

        let system_prompt = format!(
            r#"你是{}，{}

**重要：現在的時間是 {}。** 在生成任何與日期相關的欄位（如 start_date, due_date）時，請以此時間為基準進行推算。

**截止日期生成規則：**
- 對於大部分任務，你都應該設定一個合理的截止日期
- 短期任務（1-3天內完成）：設定1-3天後的截止日期
- 中期任務（1-2週完成）：設定1-2週後的截止日期
- 長期任務（1個月以上）：設定1-3個月後的截止日期
- 只有對於沒有明確時間限制的習慣類任務才設定 due_date 為 null
- 如果用戶明確提到時間（如"明天"、"下週"、"月底"），一定要根據當前時間計算對應的截止日期

任務類型說明：
- main: 主要任務（重要的長期目標，通常設定較長的截止日期）
- side: 副線任務（次要的短期任務，通常設定較短的截止日期）
- challenge: 挑戰任務（困難且有成就感的任務，根據具體內容設定截止日期）
- daily: 日常任務（例行性任務，重複性任務通常不設定截止日期）

優先級：0=低, 1=中, 2=高
難度：1-5（1=非常簡單, 5=非常困難）
經驗值：根據難度和重要性計算，通常是 difficulty * 20 + priority * 10

**重要：你必須生成一個包含主任務和子任務的完整學習計劃。**

請為用戶生成一個完整的學習計劃，包含：
1. 一個主任務（整體學習目標）
2. 3-5個具體的子任務

**主任務要求：**
- 作為整體學習目標的概括
- 包含學習總結和預估完成時間
- 設定為高優先級（priority: 2）
- 難度設為中等（difficulty: 3）
- 經驗值設為100

**子任務要求：**
- 生成3-5個具體的子任務
- 每個子任務都應該有明確的學習目標和執行步驟
- 子任務難度從簡單到困難遞增（1-4）
- 每個子任務都應該設定合理的截止日期
- 子任務類型可以是：main（主要學習）、side（輔助練習）、challenge（挑戰項目）

**你必須嚴格按照以下 JSON 格式回應，不能有任何偏差：**

{{
  "main_task": {{
    "title": "主任務標題",
    "description": "主任務描述，包含學習總結和預估完成時間",
    "task_type": "main",
    "priority": 2,
    "difficulty": 3,
    "experience": 100,
    "due_date": "主任務截止日期（ISO 8601格式）",
    "is_recurring": false,
    "recurrence_pattern": null,
    "start_date": null,
    "end_date": null,
    "completion_target": null
  }},
  "subtasks": [
    {{
      "title": "子任務1標題",
      "description": "子任務1詳細描述，包含學習目標和執行步驟",
      "task_type": "main",
      "priority": 1,
      "difficulty": 1,
      "experience": 25,
      "due_date": "子任務1截止日期（ISO 8601格式）",
      "is_recurring": false,
      "recurrence_pattern": null,
      "start_date": null,
      "end_date": null,
      "completion_target": null
    }},
    {{
      "title": "子任務2標題",
      "description": "子任務2詳細描述，包含學習目標和執行步驟",
      "task_type": "side",
      "priority": 1,
      "difficulty": 2,
      "experience": 35,
      "due_date": "子任務2截止日期（ISO 8601格式）",
      "is_recurring": false,
      "recurrence_pattern": null,
      "start_date": null,
      "end_date": null,
      "completion_target": null
    }}
  ]
}}

**注意：你的回應必須是有效的 JSON 格式，包含 main_task 和 subtasks 兩個字段。不要添加任何額外的文字或解釋。**"#,
            expert_match.ai_expert_name,
            expert_match.ai_expert_description,
            current_time_str
        );

        log::info!("[AI INPUT][generate_task_with_expert] {}", user_input);

        let request = OpenAIRequest {
            model: self.model.clone().to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: format!("請為以下任務描述生成詳細的任務規劃：{}", user_input),
                },
            ],
            max_completion_tokens: 2000,
            response_format: ResponseFormat {
                format_type: "json_object".to_string(),
            },
        };

        if let Ok(body) = serde_json::to_string(&request) {
            log::info!("[AI INPUT][generate_task_with_expert_payload] {}", format_ai_output(&body));
        }

        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;
        log::info!("[AI OUTPUT][generate_task_with_expert] {}", format_ai_output(&response_text));

        if !status.is_success() {
            return Err(anyhow::anyhow!("OpenAI API 錯誤 ({}): {}", status, response_text));
        }

        let openai_response: OpenAIResponse = serde_json::from_str(&response_text)?;
        
        if let Some(choice) = openai_response.choices.first() {
            let task_json = &choice.message.content;
            let generated_task_plan: AIGeneratedTaskPlan = serde_json::from_str(task_json)?;
            
            // 驗證生成的任務計劃
            validate_generated_task(&generated_task_plan.main_task)?;
            for subtask in &generated_task_plan.subtasks {
                validate_generated_task(subtask)?;
            }
            
            Ok(generated_task_plan)
        } else {
            Err(anyhow::anyhow!("OpenAI 未返回有效回應"))
        }
    }

    async fn analyze_with_expert(&self, user_input: &str, expert_name: &str, expert_description: &str, analysis_type: &str) -> Result<String> {
        let analysis_prompts = match analysis_type {
            "analyze" => format!(
                r#"你是{}，{}

請根據用戶的需求分析出3-6個適合的加強方向。

用戶需求：{}

請以JSON格式回應，格式如下：
{{
  "directions": [
    {{"title": "方向標題", "description": "簡短描述"}},
    {{"title": "方向標題", "description": "簡短描述"}}
  ]
}}

每個方向標題要簡潔明確，描述要簡短（不超過20字）。"#,
                expert_name, expert_description, user_input
            ),
            "goals" => format!(
                r#"你是{}，{}

請根據用戶的需求生成3-5個明確、可衡量的學習目標。目標應該具體、可達成、有時間性。

用戶需求：{}

請以清晰的格式回應，每個目標用編號列出，並說明如何衡量達成情況。"#,
                expert_name, expert_description, user_input
            ),
            "resources" => format!(
                r#"你是{}，{}

請根據用戶的需求推薦3-5個優質的學習資源，包括書籍、課程、網站、工具等。

用戶需求：{}

請以清晰的格式回應，每個資源用編號列出，並簡要說明為什麼推薦這個資源。"#,
                expert_name, expert_description, user_input
            ),
            _ => return Err(anyhow::anyhow!("不支援的分析類型: {}", analysis_type)),
        };

        log::info!("[AI INPUT][analyze_with_expert] description={} type={} expert_name={} expert_description={}", user_input, analysis_type, expert_name, expert_description);

        let request = OpenAIRequest {
            model: self.model.clone().to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: analysis_prompts,
                },
            ],
            max_completion_tokens: 1000,
            response_format: ResponseFormat {
                format_type: "json_object".to_string(),
            },
        };

        if let Ok(body) = serde_json::to_string(&request) {
            log::info!("[AI INPUT][analyze_with_expert_payload] {}", format_ai_output(&body));
        }

        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;
        log::info!("[AI OUTPUT][analyze_with_expert] {}", format_ai_output(&response_text));

        if !status.is_success() {
            return Err(anyhow::anyhow!("OpenAI API 錯誤 ({}): {}", status, response_text));
        }

        let openai_response: OpenAIResponse = serde_json::from_str(&response_text)?;

        if let Some(choice) = openai_response.choices.first() {
            Ok(choice.message.content.clone())
        } else {
            Err(anyhow::anyhow!("OpenAI 未返回有效回應"))
        }
    }

    // 新增：使用指定模型生成文字回應（OpenAI 實作）
    async fn generate_with_model(&self, model: &str, prompt: &str) -> Result<String> {
        let request = serde_json::json!({
            "model": model,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "max_completion_tokens": 4000
        });

        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("OpenAI API 錯誤: {}", error_text));
        }

        let openai_response: OpenAIResponse = response.json().await?;

        if let Some(choice) = openai_response.choices.first() {
            Ok(choice.message.content.clone())
        } else {
            Err(anyhow::anyhow!("OpenAI 未返回有效回應"))
        }
    }
}

// OpenRouter 服務實現
pub struct OpenRouterService {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl OpenRouterService {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl AIService for OpenRouterService {
    async fn generate_achievement_from_text(&self, user_input: &str) -> Result<AIGeneratedAchievement> {
        let system_prompt = r#"你是一個成就設計助手。根據用戶的行為數據分析，生成個性化且具有激勵性的成就。

請仔細分析用戶的：
1. 已有成就列表
2. 任務完成狀況
3. 任務取消/失敗狀況
4. 待完成任務

**設計原則：**
- 成就名稱要幽默且具體，如「成為英語字典」「跑火入魔」
- 基於用戶實際行為模式生成，不要憑空想像
- 如果用戶在某領域已有基礎成就且表現優秀，可考慮升級版成就
- 避免與現有成就重複

**成就分類：**
- task_mastery: 任務精通類
- consistency: 持續性類  
- challenge_overcome: 克服挑戰類
- skill_development: 技能發展類

**達成條件類型：**
- consecutive_days: 連續天數
- total_completions: 總完成次數  
- task_complete: 完成任務總數
- streak_recovery: 從失敗中恢復
- skill_level: 技能等級
- learning_task_complete: 學習任務完成
- intelligence_attribute: 智力屬性達成
- endurance_attribute: 毅力屬性達成  
- creativity_attribute: 創造力屬性達成
- social_attribute: 社交力屬性達成
- focus_attribute: 專注力屬性達成
- adaptability_attribute: 適應力屬性達成

**經驗值獎勵計算：**
- 基於難度：簡單成就 50-100，中等 100-200，困難 200-500

請以 JSON 格式回應：
{
  "name": "成就名稱（幽默且具體）",
  "description": "成就描述（選填）", 
  "icon": "圖標名稱（選填）",
  "category": "成就分類",
  "requirement_type": "達成條件類型",
  "requirement_value": 數值,
  "experience_reward": 經驗值獎勵
}

範例：
輸入：使用者連續完成「背英語單字」30天，但經常取消「運動」任務
輸出：
{
  "name": "成為英語字典",
  "description": "連續30天完成背英語單字，詞彙量已經超越一般字典",
  "icon": "📖",
  "category": "task_mastery",
  "requirement_type": "consecutive_days", 
  "requirement_value": 30,
  "experience_reward": 300
}"#;

        let user_message = format!("請根據以下用戶行為數據生成合適的成就：{}", user_input);

        let request = OpenRouterRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user_message,
                },
            ],
            max_completion_tokens: 4000,
            response_format: ResponseFormat {
                format_type: "json_object".to_string(),
            },
        };

        if let Ok(body) = serde_json::to_string(&request) {
            log::info!("[AI INPUT][generate_achievement_from_text] {}", format_ai_output(&body));
        }

        let response = self.client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://openrouter.ai")
            .header("X-Title", "LifeUp Backend")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;
        log::info!("[AI OUTPUT][generate_achievement_from_text] {}", format_ai_output(&response_text));

        if !status.is_success() {
            return Err(anyhow::anyhow!("OpenRouter API 錯誤 ({}): {}", status, response_text));
        }

        let openrouter_response: OpenRouterResponse = serde_json::from_str(&response_text)?;

        if let Some(choice) = openrouter_response.choices.first() {
            let achievement_json = &choice.message.content;
            let generated_achievement: AIGeneratedAchievement = serde_json::from_str(achievement_json)?;

            validate_generated_achievement(&generated_achievement)?;

            Ok(generated_achievement)
        } else {
            Err(anyhow::anyhow!("OpenRouter 未返回有效回應"))
        }
    }

    async fn generate_task_preview(&self, prompt: &str) -> Result<String> {
        let request = serde_json::json!({
            "model": self.model.clone(),
            "messages": [
                {
                    "role": "system",
                    "content": "你是一個充滿活力和鼓勵的任務助手。用積極正面的語氣為用戶介紹任務，讓他們感到興奮和有動力去完成。"
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "max_completion_tokens": 4000
        });

        if let Ok(body) = serde_json::to_string(&request) {
            log::info!("[AI INPUT][generate_task_preview] {}", format_ai_output(&body));
        }

        let response = self.client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://openrouter.ai")
            .header("X-Title", "LifeUp Backend")
            .json(&request)
            .send()
            .await?;
        
        let status = response.status();
        let response_text = response.text().await?;
        log::info!("[AI OUTPUT][generate_task_preview] {}", format_ai_output(&response_text));

        if !status.is_success() {
            return Err(anyhow::anyhow!("OpenRouter API 錯誤 ({}): {}", status, response_text));
        }

        let openrouter_response: OpenRouterResponse = serde_json::from_str(&response_text)?;
        
        if let Some(choice) = openrouter_response.choices.first() {
            Ok(choice.message.content.clone())
        } else {
            Err(anyhow::anyhow!("OpenRouter 未返回有效回應"))
        }
    }

    async fn generate_task_preview_with_history(&self, system_prompt: &str, history: &[(String, String)], current_message: &str) -> Result<String> {
        let mut messages = vec![];

        for (user_msg, assistant_msg) in history {
            messages.push(serde_json::json!({
                "role": "user",
                "content": user_msg
            }));
            messages.push(serde_json::json!({
                "role": "assistant",
                "content": assistant_msg
            }));
        }

        messages.push(serde_json::json!({
            "role": "system",
            "content": system_prompt
        }));

        messages.push(serde_json::json!({
            "role": "user",
            "content": current_message
        }));

        let request = serde_json::json!({
            "model": self.model.clone(),
            "messages": messages,
            "max_completion_tokens": 4000
        });
        
        if let Ok(body) = serde_json::to_string(&request) {
            log::info!("[AI INPUT][generate_task_preview_with_history] {}", format_ai_output(&body));
        }
        
        let response = self.client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://openrouter.ai")
            .header("X-Title", "LifeUp Backend")
            .json(&request)
            .send()
            .await?;
        
        let status = response.status();
        let response_text = response.text().await?;
        log::info!("[AI OUTPUT][generate_task_preview_with_history] {}", format_ai_output(&response_text));

        if !status.is_success() {
            return Err(anyhow::anyhow!("OpenRouter API 錯誤 ({}): {}", status, response_text));
        }

        let openrouter_response: OpenRouterResponse = serde_json::from_str(&response_text)?;

        if let Some(choice) = openrouter_response.choices.first() {
            Ok(choice.message.content.clone())
        } else {
            Err(anyhow::anyhow!("OpenRouter 未返回有效回應"))
        }
    }

    async fn generate_task_from_text(&self, user_input: &str) -> Result<AIGeneratedTask> {
        let now = Utc::now();
        let current_time_str = now.to_rfc3339();

        let system_prompt = format!(
            r#"你是一個任務規劃助手。根據用戶的自然語言描述，生成結構化的任務資料。

**重要：現在的時間是 {}。** 在生成任何與日期相關的欄位（如 start_date, due_date）時，請以此時間為基準進行推算。

**截止日期生成規則：**
- 對於大部分任務，你都應該設定一個合理的截止日期
- 短期任務（1-3天內完成）：設定1-3天後的截止日期
- 中期任務（1-2週完成）：設定1-2週後的截止日期
- 長期任務（1個月以上）：設定1-3個月後的截止日期
- 只有對於沒有明確時間限制的習慣類任務才設定 due_date 為 null
- 如果用戶明確提到時間（如"明天"、"下週"、"月底"），一定要根據當前時間計算對應的截止日期

任務類型說明：
- main: 主要任務（重要的長期目標，通常設定較長的截止日期）
- side: 副線任務（次要的短期任務，通常設定較短的截止日期）
- challenge: 挑戰任務（困難且有成就感的任務，根據具體內容設定截止日期）
- daily: 日常任務（例行性任務，重複性任務通常不設定截止日期）

優先級：0=低, 1=中, 2=高
難度：1-5（1=非常簡單, 5=非常困難）
經驗值：根據難度和重要性計算，通常是 difficulty * 20 + priority * 10

重複模式（僅限日常任務）：
- daily: 每天
- weekdays: 工作日
- weekends: 週末
- weekly: 每週

請以 JSON 格式回應，包含以下欄位：
{{
  "title": "任務標題",
  "description": "任務描述（選填）",
  "task_type": "main/side/challenge/daily",
  "priority": 0-2,
  "difficulty": 1-5,
  "experience": 經驗值,
  "due_date": "截止日期（ISO 8601格式，大多數情況下都應該設定）",
  "is_recurring": false,
  "recurrence_pattern": null,
  "start_date": null,
  "end_date": null,
  "completion_target": null
}}

如果是重複性任務，請設置：
- is_recurring: true
- recurrence_pattern: "daily/weekdays/weekends/weekly"
- start_date: 開始日期（ISO 8601格式）
- completion_target: 0.8（預設80%完成率目標）
- due_date: null（重複性任務通常不設定單一截止日期）

範例輸入："學習Python程式設計"
範例輸出：
{{
  "title": "學習Python程式設計",
  "description": "系統性學習Python程式語言基礎知識",
  "task_type": "main",
  "priority": 2,
  "difficulty": 3,
  "experience": 80,
  "due_date": "2024-02-15T23:59:59Z",
  "is_recurring": false,
  "recurrence_pattern": null,
  "start_date": null,
  "end_date": null,
  "completion_target": null
}}

範例輸入："明天交報告"
範例輸出：
{{
  "title": "完成並提交報告",
  "description": "整理資料並完成報告撰寫",
  "task_type": "side",
  "priority": 2,
  "difficulty": 2,
  "experience": 60,
  "due_date": "2024-01-02T18:00:00Z",
  "is_recurring": false,
  "recurrence_pattern": null,
  "start_date": null,
  "end_date": null,
  "completion_target": null
}}"#,
            current_time_str
        );

        let user_message = format!("請根據以下描述生成任務：{}", user_input);

        let request = OpenRouterRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user_message,
                },
            ],
            max_completion_tokens: 1000,
            response_format: ResponseFormat {
                format_type: "json_object".to_string(),
            },
        };
        log::info!("OpenRouter Request: {}", serde_json::to_string_pretty(&request).unwrap());
        if let Ok(body) = serde_json::to_string(&request) {
            log::info!("[AI INPUT][generate_task_from_text] {}", format_ai_output(&body));
        }

        let response = self.client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://openrouter.ai")
            .header("X-Title", "LifeUp Backend")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("OpenRouter API 錯誤: {}", error_text));
        }

        let openrouter_response: OpenRouterResponse = response.json().await?;
        
        if let Some(choice) = openrouter_response.choices.first() {
            let task_json = &choice.message.content;
            log::info!("[AI OUTPUT][generate_task_from_text] {}", format_ai_output(&task_json));
            let generated_task: AIGeneratedTask = serde_json::from_str(task_json)?;
            
            validate_generated_task(&generated_task)?;
            
            Ok(generated_task)
        } else {
            Err(anyhow::anyhow!("OpenRouter 未返回有效回應"))
        }
    }

    // 新方法：基于用户 ID 生成成就（使用统计摘要）
    async fn generate_achievement_from_user_id(&self, rb: &RBatis, user_id: &str) -> Result<AIGeneratedAchievement> {
        // 1. 生成用户行为摘要
        log::info!("为用户 {} 生成行为摘要...", user_id);
        let summary = BehaviorAnalytics::generate_summary(rb, user_id).await?;
        log::info!("行为摘要生成完成：完成{}个任务，最长连续{}天", summary.total_tasks_completed, summary.longest_streak.days);

        // 2. 构建基于摘要的 prompt
        let system_prompt = build_achievement_prompt_from_summary(&summary);

        // 3. 调用 AI 生成成就
        let request = OpenRouterRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: "請基於以上用戶數據，生成一個最合適的成就。".to_string(),
                },
            ],
            max_completion_tokens: 4000,
            response_format: ResponseFormat {
                format_type: "json_object".to_string(),
            },
        };

        if let Ok(body) = serde_json::to_string(&request) {
            log::info!("[AI INPUT][generate_achievement_from_user_id] {}", format_ai_output(&body));
        }

        let response = self.client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://openrouter.ai")
            .header("X-Title", "LifeUp Backend")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        log::info!("OpenRouter API 響應狀態: {}", status);

        if !status.is_success() {
            let error_text = response.text().await?;
            log::error!("OpenRouter API 錯誤響應: {}", error_text);
            return Err(anyhow::anyhow!("OpenRouter API 錯誤 ({}): {}", status, error_text));
        }

        let response_text = response.text().await?;
        log::info!("OpenRouter API 響應長度: {} bytes", response_text.len());

        if response_text.is_empty() {
            log::error!("OpenRouter API 返回空響應");
            return Err(anyhow::anyhow!("OpenRouter API 返回空響應"));
        }

        let openrouter_response: OpenRouterResponse = serde_json::from_str(&response_text)
            .map_err(|e| {
                log::error!("解析 OpenRouter 響應失敗: {}. 響應內容: {}", e, &response_text[..std::cmp::min(200, response_text.len())]);
                anyhow::anyhow!("解析 OpenRouter 響應失敗: {}", e)
            })?;

        if let Some(choice) = openrouter_response.choices.first() {
            let achievement_json = &choice.message.content;
            log::info!("AI 返回的成就 JSON 長度: {} 字符", achievement_json.len());

            let generated_achievement: AIGeneratedAchievement = serde_json::from_str(achievement_json)
                .map_err(|e| {
                    log::error!("解析成就 JSON 失敗: {}. JSON 內容: {}", e, achievement_json);
                    anyhow::anyhow!("解析成就 JSON 失敗: {}", e)
                })?;

            validate_generated_achievement(&generated_achievement)?;

            Ok(generated_achievement)
        } else {
            log::error!("OpenRouter 響應中沒有 choices");
            Err(anyhow::anyhow!("OpenRouter 未返回有效回應"))
        }
    }

    async fn match_expert_for_task(&self, user_input: &str) -> Result<ExpertMatch> {
        let experts = get_expert_database();
        
        // 構建專家匹配的提示詞
        let expert_list = experts.iter()
            .enumerate()
            .map(|(i, expert)| {
                format!("{}. {} ({}) - 專精領域: {}", 
                    i + 1, 
                    expert.name, 
                    expert.emoji,
                    expert.expertise_areas.join("、")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let system_prompt = format!(
            r#"你是一個專家匹配助手。根據用戶的任務描述，從以下專家列表中選擇最適合的專家。

可用專家列表：
{}

請分析用戶的任務描述，選擇最適合的專家，並提供匹配理由。

回應格式（JSON）：
{{
  "expert_name": "專家的完整名稱",
  "expert_description": "專家的詳細描述",
  "confidence": 匹配信心度（0.0-1.0）
}}

選擇原則：
1. 根據任務的核心領域選擇專家
2. 考慮專家的專業領域是否與任務匹配
3. 如果沒有完全匹配的專家，選擇最接近的
. 信心度基於匹配程度：完全匹配=1.0，部分匹配=0.6-0.8，勉強匹配=0.3-0.5"#,
            expert_list
        );

        log::info!("[AI INPUT][match_expert_for_task] {}", user_input);

        let request = OpenRouterRequest {
            model: self.model.clone().to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: (&user_input).to_string(),
                },
            ],
            max_completion_tokens: 500,
            response_format: ResponseFormat {
                format_type: "json_object".to_string(),
            },
        };

        if let Ok(body) = serde_json::to_string(&request) {
            log::info!("[AI INPUT][match_expert_for_task_payload] {}", format_ai_output(&body));
        }

        let response = self.client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://openrouter.ai")
            .header("X-Title", "LifeUp Backend")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;
        log::info!("[AI OUTPUT][match_expert_for_task] {}", format_ai_output(&response_text));

        if !status.is_success() {
            return Err(anyhow::anyhow!("OpenRouter API 錯誤 ({}): {}", status, response_text));
        }

        let openrouter_response: OpenRouterResponse = serde_json::from_str(&response_text)?;
        
        if let Some(choice) = openrouter_response.choices.first() {
            let match_json = &choice.message.content;
            let match_result: serde_json::Value = serde_json::from_str(match_json)?;
            
            let expert_name = match_result["expert_name"].as_str()
                .ok_or_else(|| anyhow::anyhow!("無效的專家名稱"))?.to_string();
            
            let expert_description = match_result["expert_description"].as_str()
                .ok_or_else(|| anyhow::anyhow!("無效的專家描述"))?.to_string();
            
            let confidence = match_result["confidence"].as_f64()
                .ok_or_else(|| anyhow::anyhow!("無效的信心度"))?;

            // 直接使用AI返回的專家信息，創建虛擬專家對象
            let virtual_expert = Expert {
                name: expert_name.clone(),
                description: expert_description.clone(),
                expertise_areas: vec!["AI匹配".to_string()],
                emoji: "🤖".to_string(),
            };

            Ok(ExpertMatch {
                expert: virtual_expert,
                confidence,
                ai_expert_name: expert_name,
                ai_expert_description: expert_description,
            })
        } else {
            Err(anyhow::anyhow!("OpenRouter 未返回有效回應"))
        }
    }

    async fn generate_task_with_expert(&self, user_input: &str, expert_match: &ExpertMatch) -> Result<AIGeneratedTaskPlan> {
        let now = Utc::now();
        let current_time_str = now.to_rfc3339();

        let system_prompt = format!(
            r#"你是{}，{}

**重要：現在的時間是 {}。** 在生成任何與日期相關的欄位（如 start_date, due_date）時，請以此時間為基準進行推算。

**截止日期生成規則：**
- 對於大部分任務，你都應該設定一個合理的截止日期
- 短期任務（1-3天內完成）：設定1-3天後的截止日期
- 中期任務（1-2週完成）：設定1-2週後的截止日期
- 長期任務（1個月以上）：設定1-3個月後的截止日期
- 只有對於沒有明確時間限制的習慣類任務才設定 due_date 為 null
- 如果用戶明確提到時間（如"明天"、"下週"、"月底"），一定要根據當前時間計算對應的截止日期

任務類型說明：
- main: 主要任務（重要的長期目標，通常設定較長的截止日期）
- side: 副線任務（次要的短期任務，通常設定較短的截止日期）
- challenge: 挑戰任務（困難且有成就感的任務，根據具體內容設定截止日期）
- daily: 日常任務（例行性任務，重複性任務通常不設定截止日期）

優先級：0=低, 1=中, 2=高
難度：1-5（1=非常簡單, 5=非常困難）
經驗值：根據難度和重要性計算，通常是 difficulty * 20 + priority * 10

**重要：你必須生成一個包含主任務和子任務的完整學習計劃。**

請為用戶生成一個完整的學習計劃，包含：
1. 一個主任務（整體學習目標）
2. 3-5個具體的子任務

**主任務要求：**
- 作為整體學習目標的概括
- 包含學習總結和預估完成時間
- 設定為高優先級（priority: 2）
- 難度設為中等（difficulty: 3）
- 經驗值設為100

**子任務要求：**
- 生成3-5個具體的子任務
- 每個子任務都應該有明確的學習目標和執行步驟
- 子任務難度從簡單到困難遞增（1-4）
- 每個子任務都應該設定合理的截止日期
- 子任務類型可以是：main（主要學習）、side（輔助練習）、challenge（挑戰項目）

**你必須嚴格按照以下 JSON 格式回應，不能有任何偏差：**

{{
  "main_task": {{
    "title": "主任務標題",
    "description": "主任務描述，包含學習總結和預估完成時間",
    "task_type": "main",
    "priority": 2,
    "difficulty": 3,
    "experience": 100,
    "due_date": "主任務截止日期（ISO 8601格式）",
    "is_recurring": false,
    "recurrence_pattern": null,
    "start_date": null,
    "end_date": null,
    "completion_target": null
  }},
  "subtasks": [
    {{
      "title": "子任務1標題",
      "description": "子任務1詳細描述，包含學習目標和執行步驟",
      "task_type": "main",
      "priority": 1,
      "difficulty": 1,
      "experience": 25,
      "due_date": "子任務1截止日期（ISO 8601格式）",
      "is_recurring": false,
      "recurrence_pattern": null,
      "start_date": null,
      "end_date": null,
      "completion_target": null
    }},
    {{
      "title": "子任務2標題",
      "description": "子任務2詳細描述，包含學習目標和執行步驟",
      "task_type": "side",
      "priority": 1,
      "difficulty": 2,
      "experience": 35,
      "due_date": "子任務2截止日期（ISO 8601格式）",
      "is_recurring": false,
      "recurrence_pattern": null,
      "start_date": null,
      "end_date": null,
      "completion_target": null
    }}
  ]
}}

**注意：你的回應必須是有效的 JSON 格式，包含 main_task 和 subtasks 兩個字段。不要添加任何額外的文字或解釋。**"#,
            expert_match.ai_expert_name,
            expert_match.ai_expert_description,
            current_time_str
        );

        log::info!("[AI INPUT][generate_task_with_expert][OpenRouter] {}", user_input);

        let request = OpenRouterRequest {
            model: self.model.clone().to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: format!("請為以下任務描述生成詳細的任務規劃：{}", user_input),
                },
            ],
            max_completion_tokens: 2000,
            response_format: ResponseFormat {
                format_type: "json_object".to_string(),
            },
        };

        if let Ok(body) = serde_json::to_string(&request) {
            log::info!("[AI INPUT][generate_task_with_expert_payload][OpenRouter] {}", body);
        }

        let response = self.client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://openrouter.ai")
            .header("X-Title", "LifeUp Backend")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;
        log::info!("[AI OUTPUT][generate_task_with_expert][OpenRouter] {}", format_ai_output(&response_text));

        if !status.is_success() {
            return Err(anyhow::anyhow!("OpenRouter API 錯誤 ({}): {}", status, response_text));
        }

        let openrouter_response: OpenRouterResponse = serde_json::from_str(&response_text)?;

        if let Some(choice) = openrouter_response.choices.first() {
            let task_json = &choice.message.content;
            let generated_task_plan: AIGeneratedTaskPlan = serde_json::from_str(task_json)?;

            validate_generated_task(&generated_task_plan.main_task)?;
            for subtask in &generated_task_plan.subtasks {
                validate_generated_task(subtask)?;
            }

            Ok(generated_task_plan)
        } else {
            Err(anyhow::anyhow!("OpenRouter 未返回有效回應"))
        }
    }

    async fn analyze_with_expert(&self, user_input: &str, expert_name: &str, expert_description: &str, analysis_type: &str) -> Result<String> {
        let analysis_prompts = match analysis_type {
            "analyze" => format!(
                r#"你是{}，{}

請根據用戶的需求分析出3-6個適合的加強方向。

用戶需求：{}

請以JSON格式回應，格式如下：
{{
  "directions": [
    {{"title": "方向標題", "description": "簡短描述"}},
    {{"title": "方向標題", "description": "簡短描述"}}
  ]
}}

每個方向標題要簡潔明確，描述要簡短（不超過20字）。"#,
                expert_name, expert_description, user_input
            ),
            "goals" => format!(
                r#"你是{}，{}

請根據用戶的需求生成3-5個明確、可衡量的學習目標。目標應該具體、可達成、有時間性。

用戶需求：{}

請以清晰的格式回應，每個目標用編號列出，並說明如何衡量達成情況。"#,
                expert_name, expert_description, user_input
            ),
            "resources" => format!(
                r#"你是{}，{}

請根據用戶的需求推薦3-5個優質的學習資源，包括書籍、課程、網站、工具等。

用戶需求：{}

請以清晰的格式回應，每個資源用編號列出，並簡要說明為什麼推薦這個資源。"#,
                expert_name, expert_description, user_input
            ),
            _ => return Err(anyhow::anyhow!("不支援的分析類型: {}", analysis_type)),
        };

        log::info!("[AI INPUT][analyze_with_expert] description={} type={} expert_name={} expert_description={}", user_input, analysis_type, expert_name, expert_description);

        let request = OpenAIRequest {
            model: self.model.clone().to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: analysis_prompts,
                },
            ],
            max_completion_tokens: 1000,
            response_format: ResponseFormat {
                format_type: "json_object".to_string(),
            },
        };

        if let Ok(body) = serde_json::to_string(&request) {
            log::info!("[AI INPUT][analyze_with_expert_payload] {}", format_ai_output(&body));
        }

        let response = self.client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://openrouter.ai")
            .header("X-Title", "LifeUp Backend")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;
        log::info!("[AI OUTPUT][analyze_with_expert] {}", format_ai_output(&response_text));

        if !status.is_success() {
            return Err(anyhow::anyhow!("OpenRouter API 錯誤 ({}): {}", status, response_text));
        }

        let openrouter_response: OpenRouterResponse = serde_json::from_str(&response_text)?;

        if let Some(choice) = openrouter_response.choices.first() {
            Ok(choice.message.content.clone())
        } else {
            Err(anyhow::anyhow!("OpenRouter 未返回有效回應"))
        }
    }

    // 新增：使用指定模型生成文字回應
    async fn generate_with_model(&self, model: &str, prompt: &str) -> Result<String> {
        // 根據模型類型動態調整 max_tokens
        let max_tokens = if model.contains("perplexity") {
            16000  // Perplexity 模型給予更大的空間
        } else if model.contains("claude") || model.contains("anthropic") {
            8000   // Claude 模型需要更多空間來生成完整的任務細節
        } else {
            4000   // 其他模型使用預設值
        };

        log::info!("使用模型 {} 生成回應，max_completion_tokens: {}", model, max_tokens);

        let request = serde_json::json!({
            "model": model,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "max_completion_tokens": max_tokens
        });

        let response = self.client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://openrouter.ai")
            .header("X-Title", "LifeUp Backend")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("OpenRouter API 錯誤: {}", error_text));
        }

        let openrouter_response: OpenRouterResponse = response.json().await?;

        if let Some(choice) = openrouter_response.choices.first() {
            Ok(choice.message.content.clone())
        } else {
            Err(anyhow::anyhow!("OpenRouter 未返回有效回應"))
        }
    }
}

// AI 服務工廠函數
pub fn create_ai_service(config: &AIConfig) -> Result<Box<dyn AIService + Send + Sync>> {
    match config.api_option.as_str() {
        "OpenAI" => {
            let api_key = config.openai_api_key.as_ref()
                .ok_or_else(|| anyhow::anyhow!("OpenAI API key 未設定"))?;
            Ok(Box::new(OpenAIService::new(api_key.clone(), config.openai_model.clone())))
        }
        "OpenRouter" => {
            let api_key = config.openrouter_api_key.as_ref()
                .ok_or_else(|| anyhow::anyhow!("OpenRouter API key 未設定"))?;
            Ok(Box::new(OpenRouterService::new(api_key.clone(), config.openrouter_model.clone())))
        }
        _ => Err(anyhow::anyhow!("不支援的 AI 服務選項: {}", config.api_option))
    }
}

fn validate_generated_task(task: &AIGeneratedTask) -> Result<()> {
    // 驗證任務類型
    if !["main", "side", "challenge", "daily"].contains(&task.task_type.as_str()) {
        return Err(anyhow::anyhow!("無效的任務類型: {}", task.task_type));
    }

    // 驗證優先級
    if task.priority < 0 || task.priority > 2 {
        return Err(anyhow::anyhow!("優先級必須在 0-2 之間"));
    }

    // 驗證難度
    if task.difficulty < 1 || task.difficulty > 5 {
        return Err(anyhow::anyhow!("難度必須在 1-5 之間"));
    }

    // 驗證經驗值
    if task.experience < 0 {
        return Err(anyhow::anyhow!("經驗值不能為負數"));
    }

    // 驗證重複性任務設置
    if task.is_recurring {
        if task.recurrence_pattern.is_none() {
            return Err(anyhow::anyhow!("重複性任務必須指定重複模式"));
        }
        
        let pattern = task.recurrence_pattern.as_ref().unwrap();
        if !["daily", "weekdays", "weekends", "weekly"].contains(&pattern.as_str()) {
            return Err(anyhow::anyhow!("無效的重複模式: {}", pattern));
        }

        if task.start_date.is_none() {
            return Err(anyhow::anyhow!("重複性任務必須指定開始日期"));
        }
    }

    // 驗證完成率目標
    if let Some(target) = task.completion_target {
        if target < 0.0 || target > 1.0 {
            return Err(anyhow::anyhow!("完成率目標必須在 0.0-1.0 之間"));
        }
    }

    Ok(())
}

fn validate_generated_achievement(achievement: &AIGeneratedAchievement) -> Result<()> {
    // 驗證成就分類
    if !["task_mastery", "consistency", "challenge_overcome", "skill_development"].contains(&achievement.category.as_str()) {
        return Err(anyhow::anyhow!("無效的成就分類: {}", achievement.category));
    }

    // 驗證達成條件類型 - 使用枚舉的有效字符串列表
    let valid_requirement_types = AchievementRequirementType::all_valid_strings();
    if !valid_requirement_types.contains(&achievement.requirement_type.as_str()) {
        return Err(anyhow::anyhow!(
            "無效的達成條件類型: {}. 有效類型: {:?}", 
            achievement.requirement_type,
            valid_requirement_types
        ));
    }

    // 驗證條件數值
    if achievement.requirement_value <= 0 {
        return Err(anyhow::anyhow!("達成條件數值必須大於0"));
    }

    // 驗證經驗值獎勵
    if achievement.experience_reward < 50 || achievement.experience_reward > 500 {
        return Err(anyhow::anyhow!("經驗值獎勵必須在 50-500 之間"));
    }

    // 驗證成就名稱長度
    if achievement.name.len() < 2 || achievement.name.len() > 50 {
        return Err(anyhow::anyhow!("成就名稱長度必須在 2-50 字之間"));
    }

    Ok(())
}

// 將 AI 生成的任務轉換為資料庫模型
pub fn convert_to_task_model(
    ai_task: AIGeneratedTask,
    user_id: String,
) -> crate::models::Task {
    use uuid::Uuid;
    
    let now = Utc::now();
    
    crate::models::Task {
        id: Some(Uuid::new_v4().to_string()),
        user_id: Some(user_id),
        title: Some(ai_task.title),
        description: ai_task.description,
        status: Some(0), // 預設為待處理
        priority: Some(ai_task.priority),
        task_type: Some(ai_task.task_type),
        difficulty: Some(ai_task.difficulty),
        experience: Some(ai_task.experience),
        parent_task_id: None,
        is_parent_task: Some(0),
        task_order: Some(0),
        due_date: ai_task.due_date.and_then(|d| d.parse().ok()),
        created_at: Some(now),
        updated_at: Some(now),
        is_recurring: Some(if ai_task.is_recurring { 1 } else { 0 }),
        recurrence_pattern: ai_task.recurrence_pattern,
        start_date: ai_task.start_date.and_then(|d| d.parse().ok()),
        end_date: ai_task.end_date.and_then(|d| d.parse().ok()),
        completion_target: ai_task.completion_target,
        completion_rate: Some(0.0),
        task_date: None,
        cancel_count: Some(0),
        last_cancelled_at: None,
        skill_tags: None,
        career_mainline_id: None,
        task_category: None,
        attributes: None,
    }
}

// 將 AI 生成的成就轉換為資料庫模型
pub fn convert_to_achievement_model(
    ai_achievement: AIGeneratedAchievement,
) -> crate::models::Achievement {
    use uuid::Uuid;
    
    let now = Utc::now();
    
    // 將字符串轉換為枚舉
    let requirement_type = AchievementRequirementType::from_string(&ai_achievement.requirement_type);
    
    crate::models::Achievement {
        id: Some(Uuid::new_v4().to_string()),
        name: Some(ai_achievement.name),
        description: ai_achievement.description,
        icon: ai_achievement.icon,
        category: Some(ai_achievement.category),
        requirement_type,
        requirement_value: Some(ai_achievement.requirement_value),
        experience_reward: Some(ai_achievement.experience_reward),
        created_at: Some(now),
    }
}

pub fn build_task_generation_prompt(
    user_input: &str,
    expert_match: &ExpertMatch,
    selected_options: Option<Vec<String>>,
    selected_directions: Option<Vec<AnalysisDirection>>,
    expert_outputs: Option<std::collections::HashMap<String, String>>,
    skill_label: &str,
    duration_label: &str,
) -> String {
    let mut prompt = String::new();
    prompt.push_str(user_input);

    if !skill_label.is_empty() || !duration_label.is_empty() {
        prompt.push_str("\n\n使用者背景：");
        if !skill_label.is_empty() {
            prompt.push_str(&format!("熟悉程度：{} ", skill_label));
        }
        if !duration_label.is_empty() {
            prompt.push_str(&format!("學習時長：{}", duration_label));
        }
    }

    if let Some(options) = selected_options {
        if !options.is_empty() {
            let option_labels = options.join("、");
            prompt.push_str(&format!("\n\n請特別針對以下需求提供任務輸出：{}", option_labels));
        }
    }

    if let Some(directions) = selected_directions {
        if !directions.is_empty() {
            prompt.push_str("\n\n使用者已選擇的重點強化方向：\n");
            for (index, direction) in directions.iter().enumerate() {
                prompt.push_str(&format!("{}. {} - {}\n", index + 1, direction.title, direction.description));
            }
        }
    }

    if let Some(outputs) = expert_outputs {
        if !outputs.is_empty() {
            prompt.push_str("\n\n前一步驟的分析結果：\n");
            for (key, value) in outputs {
                prompt.push_str(&format!("[{}]\n{}\n\n", key, value));
            }
        }
    }

    prompt.push_str(&format!(
        "\n\n請根據以上資訊，並以{} ({}) 的視角，產出符合要求的任務規劃。",
        expert_match.expert.name, expert_match.expert.description
    ));

    prompt
}