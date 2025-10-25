use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;
use chrono::Utc;
use rbatis::RBatis;
use crate::behavior_analytics::BehaviorAnalytics;
use super::r#trait::AIService;
use super::common::{
    AIGeneratedAchievement, AIGeneratedTask, AIGeneratedTaskPlan, ExpertMatch, Expert,
    format_ai_output, get_expert_database, build_achievement_prompt_from_summary,
    validate_generated_achievement, validate_generated_task,
    AITaskPrimaryFields, AITaskSecondaryFields, AIPlanPrimaryFields, AIPlanSecondaryFields
};

// OpenAI API 請求結構
#[derive(Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<ChatMessage>,
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
            model: self.model.clone(),
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

    async fn generate_achievement_from_user_id(&self, rb: &RBatis, user_id: &str) -> Result<AIGeneratedAchievement> {
        // 1. 生成用户行为摘要
        log::info!("为用户 {} 生成行为摘要...", user_id);
        let summary = BehaviorAnalytics::generate_summary(rb, user_id).await?;
        log::info!("行为摘要生成完成：完成{}个任务，最长连续{}天", summary.total_tasks_completed, summary.longest_streak.days);

        // 2. 构建基于摘要的 prompt
        let system_prompt = build_achievement_prompt_from_summary(&summary);

        // 3. 调用 AI 生成成就
        let request = OpenAIRequest {
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
            Err(anyhow::anyhow!("OpenAI 未返回有效回應"))
        }
    }

    async fn generate_task_from_text(&self, user_input: &str) -> Result<AIGeneratedTask> {
        let now = Utc::now();
        let current_time_str = now.format("%Y-%m-%dT%H:%M:%S").to_string();

        let primary_prompt = format!(
            r#"你是一個任務規劃助手。根據用戶的自然語言描述，先生成任務的主要欄位。

**重要：現在的時間是 {}。** 在生成任何與日期相關的欄位（如 due_date）時，請以此時間為基準進行推算。

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

        let primary_request = OpenAIRequest {
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
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&primary_request)
            .send()
            .await?;

        let primary_status = primary_response.status();
        let primary_text = primary_response.text().await?;
        log::info!("[AI OUTPUT][generate_task_from_text_primary] {}", format_ai_output(&primary_text));

        if !primary_status.is_success() {
            return Err(anyhow::anyhow!("OpenAI API 錯誤 (primary) ({}): {}", primary_status, primary_text));
        }

        let primary_parsed: OpenAIResponse = serde_json::from_str(&primary_text)?;
        let primary_choice = primary_parsed
            .choices
            .first()
            .ok_or_else(|| anyhow::anyhow!("OpenAI 未返回有效主欄位"))?;

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

        let secondary_request = OpenAIRequest {
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
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&secondary_request)
            .send()
            .await?;

        let secondary_status = secondary_response.status();
        let secondary_text = secondary_response.text().await?;
        log::info!("[AI OUTPUT][generate_task_from_text_secondary] {}", format_ai_output(&secondary_text));

        if !secondary_status.is_success() {
            return Err(anyhow::anyhow!("OpenAI API 錯誤 (secondary) ({}): {}", secondary_status, secondary_text));
        }

        let secondary_parsed: OpenAIResponse = serde_json::from_str(&secondary_text)?;
        let secondary_choice = secondary_parsed
            .choices
            .first()
            .ok_or_else(|| anyhow::anyhow!("OpenAI 未返回有效次欄位"))?;

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
  "expert_description": "專家的詳細描述"
}}

選擇原則：
1. 根據任務的核心領域選擇專家
2. 考慮專家的專業領域是否與任務匹配
3. 如果沒有完全匹配的專家，選擇最接近的"#,
            expert_list
        );

        log::info!("[AI INPUT][match_expert_for_task] {}", format_ai_output(&user_input));

        let request = OpenAIRequest {
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
            max_completion_tokens: 4000,
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
            Err(anyhow::anyhow!("OpenAI 未返回有效回應"))
        }
    }

    async fn generate_task_with_expert(&self, user_input: &str, expert_match: &ExpertMatch) -> Result<AIGeneratedTaskPlan> {
        let now = Utc::now();
        let current_time_str = now.format("%Y-%m-%dT%H:%M:%S").to_string();

        let system_prompt = format!(
            r#"你是{}，{}

**重要：現在的時間是 {}。** 在生成任何與日期相關的欄位（如 due_date）時，請以此時間為基準進行推算。

請根據用戶需求生成一個完整的學習任務。

要求：
1. 主任務作為整體學習目標，task_type 必須為 "main"
2. 任務描述應該詳細且具體，包含學習目標、方法建議等
3. 學習型任務不設為重複性，is_recurring 必須為 false，recurrence_pattern 必須為 null
4. 主任務固定設置：priority = 2、difficulty = 3、experience = 100
5. 不需要設置 start_date、end_date、completion_target（全部為 null）

請以 JSON 格式回應，包含以下所有欄位：
{{
  "title": "任務標題",
  "description": "詳細描述（包含學習目標和方法建議）",
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

        let request = OpenAIRequest {
            model: self.model.clone(),
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
                "[AI INPUT][generate_task_with_expert][OpenAI] {}",
                format_ai_output(&body)
            );
        }

        let response = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;
        log::info!(
            "[AI OUTPUT][generate_task_with_expert][OpenAI] {}",
            format_ai_output(&response_text)
        );

        if !status.is_success() {
            return Err(anyhow::anyhow!(
                "OpenAI API 錯誤 ({}): {}",
                status,
                response_text
            ));
        }

        let openai_response: OpenAIResponse = serde_json::from_str(&response_text)?;
        let choice = openai_response
            .choices
            .first()
            .ok_or_else(|| anyhow::anyhow!("OpenAI 未返回有效回應"))?;

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

請根據用戶的需求分析出5個適合的加強方向。
用戶需求：{}
每個方向標題要簡潔明確，描述要簡短（不超過20字）。
請以JSON格式回應，格式如下：
{{
  "directions": [
    {{"title": "方向標題", "description": "簡短描述"}},
    {{"title": "方向標題", "description": "簡短描述"}}
  ]
}}
"#,
                expert_name, expert_description, user_input
            ),
            "goals" => format!(
                r#"你是{}，{}
請根據用戶的需求生成5個明確、可衡量的學習目標。目標應該具體、可達成、有時間性。
每個目標標題要簡潔明確，描述要包含具體的衡量標準（不超過30字）。
用戶需求：{}
請以JSON格式回應，格式如下：
{{
  "goals": [
    {{"title": "目標標題", "description": "具體描述和衡量標準"}},
    {{"title": "目標標題", "description": "具體描述和衡量標準"}},
    {{"title": "目標標題", "description": "具體描述和衡量標準"}},
    {{"title": "目標標題", "description": "具體描述和衡量標準"}},
    {{"title": "目標標題", "description": "具體描述和衡量標準"}}
  ]
}}

必須返回恰好5個目標。"#,
                expert_name, expert_description, user_input
            ),
            "resources" => format!(
                r#"你是{}，{}

請根據用戶的需求推薦5個優質的學習資源，包括書籍、課程、網站、工具等。

用戶需求：{}

請以JSON格式回應，格式如下：
{{
  "resources": [
    {{"title": "資源名稱", "description": "資源描述和推薦理由"}},
    {{"title": "資源名稱", "description": "資源描述和推薦理由"}},
    {{"title": "資源名稱", "description": "資源描述和推薦理由"}},
    {{"title": "資源名稱", "description": "資源描述和推薦理由"}},
    {{"title": "資源名稱", "description": "資源描述和推薦理由"}}
  ]
}}

必須返回恰好5個學習資源。每個資源名稱要簡潔明確，描述要簡短說明為什麼推薦（不超過30字）。"#,
                expert_name, expert_description, user_input
            ),
            _ => return Err(anyhow::anyhow!("不支援的分析類型: {}", analysis_type)),
        };

        log::info!("[AI INPUT][analyze_with_expert] description={} type={} expert_name={} expert_description={}", user_input, analysis_type, expert_name, expert_description);

        let request = OpenAIRequest {
            model: self.model.clone(),
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

    async fn generate_subtasks_for_main_task(&self, _main_task_title: &str, _main_task_description: &str, _expert_match: &ExpertMatch) -> Result<Vec<AIGeneratedTask>> {
        // OpenAI服務暫時不支援，返回空列表
        log::warn!("OpenAI服務暫時不支援生成子任務");
        Ok(Vec::new())
    }

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