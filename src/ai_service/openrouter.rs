use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;
use chrono::Utc;
use rbatis::RBatis;
use crate::behavior_analytics::BehaviorAnalytics;
use super::r#trait::AIService;
use super::common::{
    AIGeneratedAchievement, AIGeneratedTask, AIGeneratedTaskPlan, AIGeneratedSkillTags, ExpertMatch, Expert,
    format_ai_output, get_expert_database, build_achievement_prompt_from_summary,
    validate_generated_achievement, validate_generated_task,
    AITaskPrimaryFields, AITaskSecondaryFields, AIPlanPrimaryFields, AIPlanSecondaryFields
};

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

#[derive(Deserialize)]
struct Choice {
    message: ChatMessage,
}

pub struct OpenRouterService {
    api_key: String,
    model: String,
    model_small: String,
    model_fast: String,
    model_normal: String,
    model_think: String,
    model_background: String,
    client: reqwest::Client,
}

impl OpenRouterService {
    pub fn new(api_key: String, model: String, model_small: String, model_fast: String, model_normal: String, model_think: String, model_background: String) -> Self {
        Self {
            api_key,
            model,
            model_small,
            model_fast,
            model_normal,
            model_think,
            model_background,
            client: reqwest::Client::new(),
        }
    }

    // 根據模型等級獲取對應模型
    fn get_model_by_tier(&self, tier: super::common::ModelTier) -> &str {
        use super::common::ModelTier;
        match tier {
            ModelTier::Small => &self.model_small,
            ModelTier::Fast => &self.model_fast,
            ModelTier::Normal => &self.model_normal,
            ModelTier::Think => &self.model_think,
            ModelTier::Background => &self.model_background,
        }
    }
}

#[async_trait::async_trait]
impl AIService for OpenRouterService {
    async fn generate_achievement_from_text(&self, user_input: &str) -> Result<AIGeneratedAchievement> {
        let system_prompt = r#"你是一個成就設計助手。根據使用者的行為資料分析，生成個性化且具有激勵性的成就。

請仔細分析使用者的：
1. 已有成就列表
2. 任務完成狀況
3. 任務取消/失敗狀況
4. 待完成任務

**設計原則：**
- 成就名稱要幽默且具體，如「成為英語字典」「跑火入魔」
- 基於使用者實際行為模式生成，不要憑空想像
- 如果使用者在某領域已有基礎成就且表現優秀，可考慮升級版成就
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

        let user_message = format!("請根據以下使用者行為資料生成合適的成就：{}", user_input);

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

    async fn generate_achievement_from_user_id(&self, rb: &RBatis, user_id: &str) -> Result<AIGeneratedAchievement> {
        // 1. 生成使用者行為摘要
        log::info!("為使用者 {} 生成行為摘要...", user_id);
        let summary = BehaviorAnalytics::generate_summary(rb, user_id).await?;
        log::info!("行為摘要生成完成：完成{}個任務，最長連續{}天", summary.total_tasks_completed, summary.longest_streak.days);

        // 2. 構建基於摘要的 prompt
        let system_prompt = build_achievement_prompt_from_summary(&summary);

        // 3. 呼叫 AI 生成成就
        let request = OpenRouterRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: "請基於以上使用者資料，生成一個最合適的成就。".to_string(),
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
                let preview = response_text.chars().take(200).collect::<String>();
                log::error!("解析 OpenRouter 響應失敗: {}. 響應內容: {}", e, preview);
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

    async fn generate_task_preview(&self, prompt: &str) -> Result<String> {
        let request = serde_json::json!({
            "model": self.model.clone(),
            "messages": [
                {
                    "role": "system",
                    "content": "你是一個充滿活力和鼓勵的任務助手。用積極正面的語氣為使用者介紹任務，讓他們感到興奮和有動力去完成。"
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
        let current_time_str = now.format("%Y-%m-%dT%H:%M:%S").to_string();

        let primary_prompt = format!(
            r#"你是一個任務規劃助手。根據使用者的自然語言描述，先生成任務的主要欄位。

**重要：現在的時間是 {}。** 在生成任何與日期相關的欄位（如 due_date）時，請以此時間為基準進行推算。

**截止日期生成規則：**
- 對於大部分任務，你都應該設定一個合理的截止日期
- 短期任務（1-3天內完成）：設定1-3天後的截止日期
- 中期任務（1-2週完成）：設定1-2週後的截止日期
- 長期任務（1個月以上）：設定1-3個月後的截止日期
- 只有對於沒有明確時間限制的習慣類任務才設定 due_date 為 null
- 如果使用者明確提到時間（如"明天"、"下週"、"月底"），一定要根據當前時間計算對應的截止日期

任務類型說明：
- main: 主要任務（重要的長期目標，通常設定較長的截止日期）
- side: 副線任務（次要的短期任務，通常設定較短的截止日期）
- challenge: 挑戰任務（困難且有成就感的任務，根據具體內容設定截止日期）
- daily: 日常任務（例行性任務，重複性任務通常不設定截止日期）

請以 JSON 格式回應，包含以下欄位：
{{
  "title": "任務標題",
  "description": "任務描述（選填）",
  "task_type": "main/side/challenge/daily",
  "due_date": "截止日期（ISO 8601格式，大多數情況下都應該設定，若為重複性任務則為 null）",
  "recurrence_pattern": "重複模式（僅在重複性任務時填寫，否則為 null）"
}}

若判定為重複性任務，recurrence_pattern 必須是 "daily"、"weekdays"、"weekends" 或 "weekly"，且 due_date 必須為 null。
"#,
            current_time_str
        );

        let primary_request = OpenRouterRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: primary_prompt,
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: format!("請根據以下描述生成任務主要欄位：{}", user_input),
                },
            ],
            max_completion_tokens: 2000,
            response_format: ResponseFormat {
                format_type: "json_object".to_string(),
            },
        };

        if let Ok(body) = serde_json::to_string(&primary_request) {
            log::info!("[AI INPUT][generate_task_from_text_primary] {}", format_ai_output(&body));
        }

        let primary_response = self
            .client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://openrouter.ai")
            .header("X-Title", "LifeUp Backend")
            .json(&primary_request)
            .send()
            .await?;

        let primary_status = primary_response.status();
        let primary_text = primary_response.text().await?;
        log::info!("[AI OUTPUT][generate_task_from_text_primary] {}", format_ai_output(&primary_text));

        if !primary_status.is_success() {
            return Err(anyhow::anyhow!("OpenRouter API 錯誤 (primary) ({}): {}", primary_status, primary_text));
        }

        let primary_parsed: OpenRouterResponse = serde_json::from_str(&primary_text)?;
        let primary_choice = primary_parsed
            .choices
            .first()
            .ok_or_else(|| anyhow::anyhow!("OpenRouter 未返回有效主欄位"))?;

        let primary_task: AITaskPrimaryFields = serde_json::from_str(&primary_choice.message.content)?;

        let secondary_prompt = format!(
            r#"基於以下任務主要欄位資訊，補全剩餘欄位。

**任務主要欄位：**
{}

請以 JSON 格式回應，包含以下欄位：
{{
  "priority": 0-2,
  "difficulty": 1-5,
  "experience": 經驗值,
  "is_recurring": 布林值,
  "completion_target": 完成率目標（重複性任務時提供，否則為 null），
  "start_date": "開始日期（ISO 8601格式，僅在需要時提供）",
  "end_date": "結束日期（ISO 8601格式，僅在需要時提供）"
}}

規則：
- 優先級：0=低, 1=中, 2=高。
- 難度：1=非常簡單, 5=非常困難。
- 經驗值通常是 difficulty * 20 + priority * 10。
- 若任務為重複性，is_recurring 應為 true，completion_target 預設 0.8，start_date 需提供，due_date 保持為 null。
- 若非重複性任務，is_recurring 為 false，completion_target、start_date、end_date 預設為 null。
"#,
            serde_json::to_string_pretty(&primary_task)?
        );

        let secondary_request = OpenRouterRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: secondary_prompt,
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: "請根據以上資訊補全剩餘欄位".to_string(),
                },
            ],
            max_completion_tokens: 2000,
            response_format: ResponseFormat {
                format_type: "json_object".to_string(),
            },
        };

        if let Ok(body) = serde_json::to_string(&secondary_request) {
            log::info!("[AI INPUT][generate_task_from_text_secondary] {}", format_ai_output(&body));
        }

        let secondary_response = self
            .client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://openrouter.ai")
            .header("X-Title", "LifeUp Backend")
            .json(&secondary_request)
            .send()
            .await?;

        let secondary_status = secondary_response.status();
        let secondary_text = secondary_response.text().await?;
        log::info!("[AI OUTPUT][generate_task_from_text_secondary] {}", format_ai_output(&secondary_text));

        if !secondary_status.is_success() {
            return Err(anyhow::anyhow!("OpenRouter API 錯誤 (secondary) ({}): {}", secondary_status, secondary_text));
        }

        let secondary_parsed: OpenRouterResponse = serde_json::from_str(&secondary_text)?;
        let secondary_choice = secondary_parsed
            .choices
            .first()
            .ok_or_else(|| anyhow::anyhow!("OpenRouter 未返回有效次欄位"))?;

        let secondary_task: AITaskSecondaryFields = serde_json::from_str(&secondary_choice.message.content)?;

        let combined_task = AIGeneratedTask {
            title: primary_task.title,
            description: primary_task.description,
            task_type: primary_task.task_type,
            priority: secondary_task.priority,
            difficulty: secondary_task.difficulty,
            experience: secondary_task.experience,
            due_date: primary_task.due_date,
            is_recurring: secondary_task.is_recurring,
            recurrence_pattern: primary_task.recurrence_pattern,
            start_date: secondary_task.start_date,
            end_date: secondary_task.end_date,
            completion_target: secondary_task.completion_target,
        }
        .with_defaults()
        .normalize_recurring();

        let validated_task = validate_generated_task(&combined_task)?;

        Ok(validated_task)
    }

    async fn generate_daily_task_from_text(&self, user_input: &str) -> Result<AIGeneratedTask> {
        let primary_prompt = r#"你是一個每日任務規劃助手。根據使用者的描述，生成適合每天執行的日常任務。

**每日任務特性：**
- 這是需要每天重複執行的習慣或例行事項
- 任務應該簡單明確，容易在一天內完成
- 通常是健康、學習、工作、生活習慣相關
- 不設定截止日期（due_date 為 null）
- task_type 固定為 "daily"

**使用者技能水準適應（重要）：**
- **務必仔細分析使用者的技能水準**，從描述中推斷其熟悉程度（如「想學」、「初學」、「已經在做」等關鍵字）
- **初學者/入門階段**：從最基礎、低門檻的任務開始
  * 例如想學登山 → 「走樓梯10分鐘」、「在平地健走20分鐘」而非直接登山
  * 例如想學英語 → 「學習5個基礎單字」、「聽英文歌曲10分鐘」而非閱讀文章
  * 難度設為 1，避免過度挑戰導致放棄
- **中級階段**：有一定基礎，可適度增加難度
  * 例如登山中級者 → 「爬郊山步道30分鐘」、「負重健走」
  * 例如英語中級者 → 「閱讀簡單英文文章」、「練習日常對話」
  * 難度設為 2
- **資深/專家階段**：已有豐富經驗，可設定專業挑戰
  * 例如登山資深者 → 「登小山」、「進階登山訓練」
  * 例如英語專家 → 「撰寫英文文章」、「英文演講練習」
  * 難度設為 3
- **漸進式設計原則**：確保任務符合使用者當前能力，避免一開始就要求過高而導致挫折

**任務難度和經驗值設定：**
- 簡單的日常習慣（如喝水8杯、記錄心情、走樓梯）：difficulty=1, experience=5
- 需要一定執行時間的任務（如運動30分鐘、閱讀20頁）：difficulty=2, experience=10
- 需要專注力和持續性的任務（如學習新技能1小時、冥想30分鐘、專業訓練）：difficulty=3, experience=15

**任務類型說明：**
- 每日任務的 task_type 必須是 "daily"
- 這類任務適合養成習慣，每天都可以重複執行
- 不要設定截止日期，因為這是持續性的習慣

請以 JSON 格式回應：
{
  "title": "任務標題（簡潔明確，例如：每日走樓梯10分鐘）",
  "description": "任務描述（可選，說明如何執行這個習慣，並鼓勵使用者循序漸進）",
  "task_type": "daily",
  "priority": 0-2,
  "difficulty": 1-3,
  "experience": 5-15,
  "due_date": null,
  "is_recurring": false,
  "recurrence_pattern": null
}
"#;

        let request = OpenRouterRequest {
            model: self.model_fast.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: primary_prompt.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: format!("請根據以下描述生成每日任務：{}", user_input),
                },
            ],
            max_completion_tokens: 1000,
            response_format: ResponseFormat {
                format_type: "json_object".to_string(),
            },
        };

        if let Ok(body) = serde_json::to_string(&request) {
            log::info!("[AI INPUT][generate_daily_task_from_text] {}", format_ai_output(&body));
        }

        let response = self
            .client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://openrouter.ai")
            .header("X-Title", "LifeUp Backend")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await?;
        log::info!("[AI OUTPUT][generate_daily_task_from_text] {}", format_ai_output(&text));

        if !status.is_success() {
            return Err(anyhow::anyhow!("OpenRouter API 錯誤 ({}): {}", status, text));
        }

        let parsed: OpenRouterResponse = serde_json::from_str(&text)?;
        let choice = parsed
            .choices
            .first()
            .ok_or_else(|| anyhow::anyhow!("OpenRouter 未返回有效回應"))?;

        let daily_task: AIGeneratedTask = serde_json::from_str(&choice.message.content)?;

        // 強制設定每日任務的特定屬性
        let daily_task_normalized = AIGeneratedTask {
            title: daily_task.title,
            description: daily_task.description,
            task_type: Some("daily".to_string()), // 強制為 daily
            priority: daily_task.priority,
            difficulty: daily_task.difficulty.or(Some(2)), // 預設難度為 2
            experience: daily_task.experience.or(Some(10)), // 預設經驗值為 10
            due_date: None, // 強制為 null
            is_recurring: Some(false),
            recurrence_pattern: None,
            start_date: None,
            end_date: None,
            completion_target: None,
        };

        let validated_task = validate_generated_task(&daily_task_normalized)?;

        Ok(validated_task)
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
            r#"你是一個專家匹配助手。根據使用者的任務描述，從以下專家列表中選擇最適合的專家。

可用專家列表：
{}

請分析使用者的任務描述，選擇最適合的專家，並提供匹配理由。
選擇原則：
1. 根據任務的核心領域選擇專家，只能選一個
2. 考慮專家的專業領域是否與任務匹配
回應格式（JSON），必需嚴格遵守：
{{
  "expert_name": "專家的完整名稱",
  "expert_description": "專家的詳細描述"
}}
"#,
            expert_list
        );

        log::info!("[AI INPUT][match_expert_for_task] {}", user_input);

        let request = OpenRouterRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user_input.to_string(),
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

            // 直接使用AI返回的專家信息，創建虛擬專家對象
            let virtual_expert = Expert {
                name: expert_name.clone(),
                description: expert_description.clone(),
                expertise_areas: vec!["AI匹配".to_string()],
                emoji: "🤖".to_string(),
            };

            Ok(ExpertMatch {
                expert: virtual_expert,
                ai_expert_name: expert_name,
                ai_expert_description: expert_description,
            })
        } else {
            Err(anyhow::anyhow!("OpenRouter 未返回有效回應"))
        }
    }

    async fn generate_task_with_expert(&self, user_input: &str, expert_match: &ExpertMatch) -> Result<AIGeneratedTaskPlan> {
        let now = Utc::now();
        let current_time_str = now.format("%Y-%m-%dT%H:%M:%S").to_string();

        let system_prompt = format!(
            r#"你是{}，{}

**重要：現在的時間是 {}。** 在生成任何與日期相關的欄位（如 due_date）時，請以此時間為基準進行推算。

請根據使用者需求生成一個完整的學習任務。

要求：
1. 主任務作為整體學習目標，task_type 必須為 "main"
2. 任務描述應該簡單明確
3. 學習型任務不設為重複性，is_recurring 必須為 false，recurrence_pattern 必須為 null
4. 主任務固定設置：priority = 2、difficulty = 3、experience = 100
5. 不需要設置 start_date、end_date、completion_target（全部為 null）

請以 JSON 格式回應，包含以下所有欄位：
{{
  "title": "任務標題(繁體中文)",
  "description": "詳細描述（包含學習目標和方法建議，繁體中文）",
  "task_type": "main",
  "priority": 2,
  "difficulty": 3,
  "experience": 100,
  "due_date": "ISO 8601 格式時間或 null",
  "is_recurring": false,
  "recurrence_pattern": null,
  "start_date": null,
  "end_date": null,
  "completion_target": null
}}

不要輸出其他欄位或額外文字。"#,
            expert_match.ai_expert_name,
            expert_match.ai_expert_description,
            current_time_str
        );

        let request = OpenRouterRequest {
            model: self.model_fast.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: format!("請根據以下描述生成完整的學習任務：{}", user_input),
                },
            ],
            max_completion_tokens: 3000,
            response_format: ResponseFormat {
                format_type: "json_object".to_string(),
            },
        };

        if let Ok(body) = serde_json::to_string(&request) {
            log::info!(
                "[AI INPUT][generate_task_with_expert][OpenRouter] {}",
                format_ai_output(&body)
            );
        }

        let response = self
            .client
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
        log::info!(
            "[AI OUTPUT][generate_task_with_expert][OpenRouter] {}",
            format_ai_output(&response_text)
        );

        if !status.is_success() {
            return Err(anyhow::anyhow!(
                "OpenRouter API 錯誤 ({}): {}",
                status,
                response_text
            ));
        }

        let openrouter_response: OpenRouterResponse = serde_json::from_str(&response_text)?;
        let choice = openrouter_response
            .choices
            .first()
            .ok_or_else(|| anyhow::anyhow!("OpenRouter 未返回有效回應"))?;

        // 直接解析為 AIGeneratedTask
        let mut main_task: AIGeneratedTask = serde_json::from_str(&choice.message.content)?;

        // 確保設置正確的默認值
        main_task.task_type = Some("main".to_string());
        main_task.priority = Some(2);
        main_task.difficulty = Some(3);
        main_task.experience = Some(100);
        main_task.is_recurring = Some(false);
        main_task.recurrence_pattern = None;
        main_task.start_date = None;
        main_task.end_date = None;
        main_task.completion_target = None;

        let main_task = main_task.with_defaults().normalize_recurring();
        let validated_main_task = validate_generated_task(&main_task)?;

        // 不生成子任務
        let subtasks: Vec<AIGeneratedTask> = Vec::new();

        Ok(AIGeneratedTaskPlan {
            main_task: validated_main_task,
            subtasks,
        })
    }

    async fn analyze_with_expert(&self, user_input: &str, expert_name: &str, expert_description: &str, analysis_type: &str) -> Result<String> {
        let analysis_prompts = match analysis_type {
            "analyze" => format!(
                r#"你是{}，{}

請根據使用者的需求分析出3-6個適合的加強方向。

使用者需求：{}

請以JSON格式加繁體中文回應，格式如下：
{{
  "directions": [
    {{"title": "方向標題", "description": "簡短描述"}},
    {{"title": "方向標題", "description": "簡短描述"}}
    ...
  ]
}}

每個方向標題要簡潔明確，描述要簡短（不超過20字）。"#,
                expert_name, expert_description, user_input
            ),
            "goals" => format!(
                r#"你是{}，{}

請根據使用者的需求生成4-6個明確、可衡量的學習目標。目標應該具體、可達成、有時間性。

使用者需求：{}

請以JSON格式加繁體中文回應，格式如下：
{{
  "goals": [
    {{"title": "目標標題", "description": "具體描述和衡量標準"}},
    {{"title": "目標標題", "description": "具體描述和衡量標準"}},
    ...
  ]
}}

必須返回恰好5個目標。每個目標標題要簡潔明確，描述要包含具體的衡量標準（不超過30字）。"#,
                expert_name, expert_description, user_input
            ),
            "resources" => format!(
                r#"你是{}，{}

請根據使用者的需求推薦4-6個優質的學習資源，包括書籍、課程、網站、工具等。

使用者需求：{}

請以JSON格式加繁體中文回應，格式如下：
{{
  "resources": [
    {{"title": "資源名稱", "description": "資源描述和推薦理由"}},
    {{"title": "資源名稱", "description": "資源描述和推薦理由"}},
    ...
  ]
}}

必須返回恰好5個學習資源。每個資源名稱要簡潔明確，描述要簡短說明為什麼推薦（不超過30字）。"#,
                expert_name, expert_description, user_input
            ),
            _ => return Err(anyhow::anyhow!("不支援的分析類型: {}", analysis_type)),
        };

        log::info!("[AI INPUT][analyze_with_expert] description={} type={} expert_name={} expert_description={}", user_input, analysis_type, expert_name, expert_description);

        let request = OpenRouterRequest {
            model: self.model_fast.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: analysis_prompts,
                },
            ],
            max_completion_tokens: 4000,
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

    async fn generate_subtasks_for_main_task(&self, main_task_title: &str, main_task_description: &str, expert_match: &ExpertMatch) -> Result<Vec<AIGeneratedTask>> {
        let now = Utc::now();
        let current_time_str = now.format("%Y-%m-%dT%H:%M:%S").to_string();

        let prompt = format!(
            r#"你是{}，{}

現在的時間是 {}。

已有主任務：
標題：{}
描述：{}

請為這個主任務生成 5 個具體可執行的子任務。
跟一個每日任務，每日任務的 task_type 必須為 "daily"
要求：
- 每個子任務應該明確具體，可直接執行
- 子任務的 task_type 可為 "main","side","challenge","daily"
- 難度遞增（1-4），從簡單到困難
- 提供合理的經驗值（10-50）
- 子任務不需要設定截止時間

回應格式：
{{
  "subtasks": [
    {{
      "title": "...",
      "description": "...",
      "task_type": "main/side/challenge",
      "priority": 1-3,
      "difficulty": 1-4,
      "experience": 10-50,
      "due_date": null,
      "is_recurring": false,
      "recurrence_pattern": null
    }}
  ]
}}

請只生成子任務，不要重複主任務。"#,
            expert_match.ai_expert_name,
            expert_match.ai_expert_description,
            current_time_str,
            main_task_title,
            main_task_description
        );

        let request = OpenRouterRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "user".to_string(),
                    content: prompt,
                },
            ],
            max_completion_tokens: 2000,
            response_format: ResponseFormat {
                format_type: "json_object".to_string(),
            },
        };

        let response = self.client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("X-Title", "LifeUp")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await?;

        if !status.is_success() {
            return Err(anyhow::anyhow!("OpenRouter API 錯誤: {}", text));
        }

        let openrouter_response: OpenRouterResponse = serde_json::from_str(&text)?;
        let choice = openrouter_response
            .choices
            .first()
            .ok_or_else(|| anyhow::anyhow!("OpenRouter 未返回有效子任務"))?;

        // 解析返回的JSON
        let subtasks_response: serde_json::Value = serde_json::from_str(&choice.message.content)?;
        let subtasks_array = subtasks_response["subtasks"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("未找到子任務陣列"))?;

        let mut result = Vec::new();
        for subtask_json in subtasks_array {
            let subtask = AIGeneratedTask {
                title: subtask_json["title"].as_str().map(String::from),
                description: subtask_json["description"].as_str().map(String::from),
                task_type: subtask_json["task_type"].as_str().map(String::from),
                priority: subtask_json["priority"].as_i64().map(|v| v as i32),
                difficulty: subtask_json["difficulty"].as_i64().map(|v| v as i32),
                experience: subtask_json["experience"].as_i64().map(|v| v as i32),
                due_date: None,
                is_recurring: Some(false),
                recurrence_pattern: None,
                start_date: None,
                end_date: None,
                completion_target: None,
            };
            result.push(subtask.with_defaults());
        }

        log::info!("成功生成 {} 個子任務", result.len());
        Ok(result)
    }

    async fn generate_with_model(&self, model: &str, prompt: &str) -> Result<String> {
        // 根據模型類型動態調整 max_tokens
        let max_tokens = if model.contains("perplexity") {
            16000  // Perplexity 模型給予更大的空間
        } else if model.contains("gpt-oss-120b") {
            12000  // GPT-OSS-120B 大模型需要更多空間來生成完整的任務細節
        } else if model.contains("claude") || model.contains("anthropic") {
            8000   // Claude 模型需要更多空間來生成完整的任務細節
        } else if model.contains("gpt-4o") && !model.contains("mini") {
            8000   // GPT-4o (非 mini) 支持更長的輸出
        } else if model.contains("deepseek") || model.contains("o1") || model.contains("gpt-5") {
            6000   // DeepSeek/o1/gpt-5 等新模型給予較多空間
        } else if model.contains("gpt") {
            6000   // 其他 GPT 模型（包括 gpt-4o-mini）給予較多空間
        } else {
            4000   // 其他模型使用預設值
        };

        log::info!("使用模型 {} 生成回應，max_completion_tokens: {}", model, max_tokens);

        // 建構基本請求
        let mut request = serde_json::json!({
            "model": model,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "max_completion_tokens": max_tokens
        });

        // 若是 Perplexity 模型，添加 web_search_options 啟用搜尋功能
        if model.contains("perplexity") {
            request["web_search_options"] = serde_json::json!({
                "search_context_size": "medium"  // 使用 medium 平衡成本與搜尋品質
            });
            log::info!("🔍 為 Perplexity 模型啟用網路搜尋功能 (search_context_size: medium)");
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
            Ok(choice.message.content.clone())
        } else {
            Err(anyhow::anyhow!("OpenRouter 未返回有效回應"))
        }
    }

    async fn classify_user_intent(&self, user_input: &str) -> Result<crate::ai_tasks::ClassifyIntentResponse> {
        let system_prompt = r#"你是一個智能任務意圖分析助手。你的任務是分析使用者的輸入,判斷他們是想要:
1. **詳細任務** (detailed_task): 用戶已經有明確的計劃和詳細描述,可以直接轉換為具體任務
2. **模糊目標** (vague_goal): 用戶只有一個大致的想法或目標,需要專家協助規劃和細化

**判斷標準:**

詳細任務的特徵:
- 包含明確的行動步驟或具體做法
- 有時間安排、頻率描述(例如:每天、每週、持續3個月)
- 描述了具體要達成什麼(例如:閱讀某本書、完成某個項目、練習某個技能30分鐘)
- 提到了具體的資源、工具或方法
- 使用了「我要做...」、「計劃...」、「每天...」等行動導向的詞彙
- 例如: "我想每天早上慢跑30分鐘,持續3個月"、"學習Python,每天寫代碼1小時"、"閱讀《原子習慣》,每天20頁"

模糊目標的特徵:
- 只表達了一個願望或興趣,沒有具體計劃
- 使用「想學...」、「對...感興趣」、「希望...」等願望性詞彙
- 沒有提及具體的執行方式、時間安排
- 缺乏明確的衡量標準或階段性目標
- 例如: "我想學寫小說"、"想提升登山能力"、"對攝影感興趣"、"想變得更健康"

**任務類型建議:**
- 如果是詳細任務且描述每日重複: task_type = "daily"
- 如果是詳細任務且是長期目標: task_type = "main"
- 如果是詳細任務且是中短期項目: task_type = "side"
- 如果是模糊目標: 不建議task_type,需要專家協助規劃

請以 JSON 格式回應:
{
  "intent_type": "detailed_task 或 vague_goal",
  "confidence": 0.0到1.0的信心度,
  "suggested_task_type": "main/side/daily/null",
  "reasoning": "簡短說明你的判斷理由(30字以內)"
}
"#;

        let request = OpenRouterRequest {
            model: self.model_fast.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: format!("請分析以下用戶輸入的意圖:\n\n{}", user_input),
                },
            ],
            max_completion_tokens: 500,
            response_format: ResponseFormat {
                format_type: "json_object".to_string(),
            },
        };

        if let Ok(body) = serde_json::to_string(&request) {
            log::info!("[AI INPUT][classify_user_intent] {}", format_ai_output(&body));
        }

        let response = self
            .client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://openrouter.ai")
            .header("X-Title", "LifeUp Backend")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await?;
        log::info!("[AI OUTPUT][classify_user_intent] {}", format_ai_output(&text));

        if !status.is_success() {
            return Err(anyhow::anyhow!("OpenRouter API 錯誤 ({}): {}", status, text));
        }

        let parsed: OpenRouterResponse = serde_json::from_str(&text)?;
        let choice = parsed
            .choices
            .first()
            .ok_or_else(|| anyhow::anyhow!("OpenRouter 未返回有效回應"))?;

        let classification: crate::ai_tasks::ClassifyIntentResponse =
            serde_json::from_str(&choice.message.content)?;

        Ok(classification)
    }

    async fn generate_skill_tags(
        &self,
        task_title: &str,
        task_description: Option<&str>,
        user_existing_skills: &[String]
    ) -> Result<AIGeneratedSkillTags> {
        // 使用 Fast 模型進行快速技能標籤生成
        let model = self.get_model_by_tier(super::common::ModelTier::Fast);

        // 構建提示詞
        let existing_skills_str = if user_existing_skills.is_empty() {
            "（使用者目前還沒有任何技能）".to_string()
        } else {
            user_existing_skills.join("、")
        };

        let description_part = task_description
            .map(|d| format!("\n任務描述：{}", d))
            .unwrap_or_default();

        let system_prompt = format!(
            r#"你是一個技能標籤生成助手。你的任務是為使用者的任務生成 1-3 個相關技能標籤，並標註每個技能對應的六大屬性。

**重要：你必須只返回 JSON 格式，不要返回其他內容！**

使用者現有技能：{}

**六大屬性定義：**
- intelligence (智力): 學習、分析、邏輯思考、程式設計、研究等
- endurance (毅力): 堅持、健身、長期目標、自律、耐力等
- creativity (創造力): 藝術、設計、創意思考、寫作、音樂等
- social (社交力): 溝通、團隊合作、人際關係、演講、領導等
- focus (專注力): 專注、效率、時間管理、任務執行、細節處理等
- adaptability (適應力): 學習新事物、解決問題、應變能力、多任務處理等

規則：
1. 優先使用使用者現有的技能名稱
2. 技能名稱要簡潔明確，使用繁體中文，最多 6 個字
3. 返回 1-3 個技能
4. 技能應該是通用類型，例如：「烹飪」「Python 程式設計」「時間管理」
5. 為每個技能選擇最相關的屬性（從六大屬性中選一個）

必須返回此 JSON 格式：
{{
  "skills": [
    {{"skill": "技能名稱", "attribute": "intelligence"}},
    {{"skill": "技能名稱", "attribute": "focus"}}
  ]
}}"#,
            existing_skills_str
        );

        let user_prompt = format!(
            "任務名稱：{}{}",
            task_title,
            description_part
        );

        log::info!("🎯 生成技能標籤 - 任務: {}", task_title);
        log::debug!("現有技能數量: {}", user_existing_skills.len());

        // 合併 system prompt 和 user prompt
        let combined_prompt = format!("{}\n\n{}", system_prompt, user_prompt);

        let request = OpenRouterRequest {
            model: model.to_string(),
            messages: vec![
                ChatMessage {
                    role: "user".to_string(),
                    content: combined_prompt,
                },
            ],
            max_completion_tokens: 500,
            response_format: ResponseFormat {
                format_type: "json_object".to_string(),
            },
        };

        let response = self
            .client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "無法讀取錯誤訊息".to_string());
            log::error!("OpenRouter API 錯誤 ({}): {}", status, error_text);
            return Err(anyhow::anyhow!("OpenRouter API 錯誤: {} - {}", status, error_text));
        }

        let text = response.text().await?;
        log::debug!("OpenRouter 原始回應: {}", text);

        let parsed: OpenRouterResponse = serde_json::from_str(&text)?;
        let choice = parsed
            .choices
            .first()
            .ok_or_else(|| anyhow::anyhow!("OpenRouter 未返回有效回應"))?;

        // 清理 AI 回應內容，移除可能的代碼塊標記
        let content = choice.message.content.trim();
        let cleaned_content = if content.starts_with("```json") {
            // 移除 ```json 開頭和 ``` 結尾
            content
                .strip_prefix("```json")
                .unwrap_or(content)
                .strip_suffix("```")
                .unwrap_or(content)
                .trim()
        } else if content.starts_with("```") {
            // 移除 ``` 開頭和 ``` 結尾
            content
                .strip_prefix("```")
                .unwrap_or(content)
                .strip_suffix("```")
                .unwrap_or(content)
                .trim()
        } else {
            content
        };

        log::debug!("清理後的內容: {}", cleaned_content);

        let skill_tags: AIGeneratedSkillTags = serde_json::from_str(cleaned_content)
            .map_err(|e| {
                log::error!("解析技能標籤失敗: {}", e);
                log::error!("AI 回應內容: {}", choice.message.content);
                log::error!("清理後內容: {}", cleaned_content);
                anyhow::anyhow!("解析 AI 回應失敗: {}", e)
            })?;

        log::info!("✅ 生成技能標籤成功: {:?}", skill_tags.skills);

        Ok(skill_tags)
    }
}