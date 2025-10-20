use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;
use chrono::Utc;
use rbatis::RBatis;
use crate::models::AchievementRequirementType;
use crate::config::AIConfig;
use crate::behavior_analytics::{UserBehaviorSummary, BehaviorAnalytics};

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
                    content: user_message,
                },
            ],
            max_completion_tokens: 4000,
            response_format: ResponseFormat {
                format_type: "json_object".to_string(),
            },
        };

        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        log::info!("OpenAI API 響應狀態: {}", status);

        if !status.is_success() {
            let error_text = response.text().await?;
            log::error!("OpenAI API 錯誤響應: {}", error_text);
            return Err(anyhow::anyhow!("OpenAI API 錯誤 ({}): {}", status, error_text));
        }

        // 先獲取文本再解析
        let response_text = response.text().await?;
        log::info!("OpenAI API 響應長度: {} bytes", response_text.len());

        if response_text.is_empty() {
            log::error!("OpenAI API 返回空響應");
            return Err(anyhow::anyhow!("OpenAI API 返回空響應"));
        }

        log::info!("OpenAI 完整響應: {}", &response_text);

        let openai_response: OpenAIResponse = serde_json::from_str(&response_text)
            .map_err(|e| {
                log::error!("解析 OpenAI 響應失敗: {}. 響應內容: {}", e, &response_text[..std::cmp::min(200, response_text.len())]);
                anyhow::anyhow!("解析 OpenAI 響應失敗: {}", e)
            })?;

        if let Some(choice) = openai_response.choices.first() {
            let achievement_json = &choice.message.content;
            log::info!("AI 返回的成就 JSON 長度: {} 字符", achievement_json.len());
            log::debug!("AI 返回的成就 JSON 內容: {}", achievement_json);

            if achievement_json.trim().is_empty() {
                log::error!("AI 返回的 content 為空");
                return Err(anyhow::anyhow!("AI 返回的內容為空"));
            }

            let generated_achievement: AIGeneratedAchievement = serde_json::from_str(achievement_json)
                .map_err(|e| {
                    log::error!("解析成就 JSON 失敗: {}. JSON 內容: {}", e, achievement_json);
                    anyhow::anyhow!("解析成就 JSON 失敗: {}", e)
                })?;

            // 驗證生成的成就
            validate_generated_achievement(&generated_achievement)?;

            Ok(generated_achievement)
        } else {
            log::error!("OpenAI 響應中沒有 choices");
            Err(anyhow::anyhow!("OpenAI 未返回有效回應"))
        }
    }

    async fn generate_task_preview(&self, prompt: &str) -> Result<String> {
        // 使用不同的請求結構，因為我們不需要 JSON 格式
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

    async fn generate_task_preview_with_history(&self, system_prompt: &str, history: &[(String, String)], current_message: &str) -> Result<String> {
        // 構建 messages 數組
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
            let task_json = &choice.message.content;
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

        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        log::info!("OpenAI API 響應狀態: {}", status);

        if !status.is_success() {
            let error_text = response.text().await?;
            log::error!("OpenAI API 錯誤響應: {}", error_text);
            return Err(anyhow::anyhow!("OpenAI API 錯誤 ({}): {}", status, error_text));
        }

        let response_text = response.text().await?;
        log::info!("OpenAI API 響應長度: {} bytes", response_text.len());

        if response_text.is_empty() {
            log::error!("OpenAI API 返回空響應");
            return Err(anyhow::anyhow!("OpenAI API 返回空響應"));
        }

        let openai_response: OpenAIResponse = serde_json::from_str(&response_text)
            .map_err(|e| {
                log::error!("解析 OpenAI 響應失敗: {}. 響應內容: {}", e, &response_text[..std::cmp::min(200, response_text.len())]);
                anyhow::anyhow!("解析 OpenAI 響應失敗: {}", e)
            })?;

        if let Some(choice) = openai_response.choices.first() {
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
            log::error!("OpenAI 響應中沒有 choices");
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

        log::info!("OpenRouter 完整響應: {}", &response_text);

        let openrouter_response: OpenRouterResponse = serde_json::from_str(&response_text)
            .map_err(|e| {
                log::error!("解析 OpenRouter 響應失敗: {}. 響應內容: {}", e, &response_text[..std::cmp::min(200, response_text.len())]);
                anyhow::anyhow!("解析 OpenRouter 響應失敗: {}", e)
            })?;

        if let Some(choice) = openrouter_response.choices.first() {
            let achievement_json = &choice.message.content;
            log::info!("AI 返回的成就 JSON 長度: {} 字符", achievement_json.len());
            log::debug!("AI 返回的成就 JSON 內容: {}", achievement_json);

            if achievement_json.trim().is_empty() {
                log::error!("AI 返回的 content 為空");
                return Err(anyhow::anyhow!("AI 返回的內容為空"));
            }

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

    async fn generate_task_preview(&self, prompt: &str) -> Result<String> {
        let request = serde_json::json!({
            "model": self.model,
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
        log::info!("OpenRouter Request: {}", serde_json::to_string_pretty(&request).unwrap());
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

    async fn generate_task_preview_with_history(&self, system_prompt: &str, history: &[(String, String)], current_message: &str) -> Result<String> {
        // 構建 messages 數組
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
            "model": self.model,
            "messages": messages,
            "max_completion_tokens": 4000
        });
        
        log::info!("OpenRouter Request with History: {}", serde_json::to_string_pretty(&request).unwrap());
        
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