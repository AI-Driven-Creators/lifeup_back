use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;
use chrono::Utc;

// OpenAI API 請求結構
#[derive(Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: i32,
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

pub struct OpenAIService {
    api_key: String,
    client: reqwest::Client,
}

impl OpenAIService {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }

    pub async fn generate_task_from_text(&self, user_input: &str) -> Result<AIGeneratedTask> {
        // 獲取當前時間並格式化
        let now = Utc::now();
        let current_time_str = now.to_rfc3339(); // e.g., "2025-08-17T12:00:00Z"

        let system_prompt = format!(
            r#"你是一個任務規劃助手。根據用戶的自然語言描述，生成結構化的任務資料。

**重要：現在的時間是 {}。** 在生成任何與日期相關的欄位（如 start_date, due_date）時，請以此時間為基準進行推算。例如，如果使用者說「明天」，你應該計算出對應的日期。

任務類型說明：
- main: 主要任務（重要的長期目標）
- side: 副線任務（次要的短期任務）
- challenge: 挑戰任務（困難且有成就感的任務）
- daily: 日常任務（例行性任務）

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
  "due_date": "截止日期（ISO 8601格式，選填）",
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

範例輸入："每天早上跑步30分鐘"
範例輸出：
{{ 
  "title": "晨跑30分鐘",
  "description": "每天早上進行30分鐘的慢跑運動",
  "task_type": "daily",
  "priority": 1,
  "difficulty": 2,
  "experience": 50,
  "due_date": null,
  "is_recurring": true,
  "recurrence_pattern": "daily",
  "start_date": "2024-01-01T06:00:00Z",
  "end_date": null,
  "completion_target": 0.8
}}"#,
            current_time_str
        );

        let user_message = format!("請根據以下描述生成任務：{}", user_input);

        let request = OpenAIRequest {
            model: "gpt-4o-mini".to_string(),
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
            temperature: 0.7,
            max_tokens: 500,
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
    }
}