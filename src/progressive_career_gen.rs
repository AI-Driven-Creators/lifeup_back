use actix_web::{web, HttpResponse, Result};
use rbatis::RBatis;
use serde::{Deserialize, Serialize};
use serde_json;
use log;
use std::time::Duration;
use tokio::sync::mpsc;
use futures::stream::StreamExt;

use crate::config::AIConfig;
use crate::ai_service::AIService;
use crate::models::SurveyAnswers;
use crate::ai_tasks::ApiResponse;

/// 多步驟任務生成請求
#[derive(Debug, Deserialize, Clone)]
pub struct ProgressiveGenerationRequest {
    pub quiz_result_id: String,
    pub selected_career: String,
    pub user_id: Option<String>,
    pub survey_answers: SurveyAnswers,
}

/// 生成進度事件
#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type")]
pub enum ProgressEvent {
    #[serde(rename = "status")]
    Status {
        stage: String,
        message: String,
        progress: u8,  // 0-100
    },
    #[serde(rename = "outline_complete")]
    OutlineComplete {
        content: serde_json::Value,
    },
    #[serde(rename = "details_complete")]
    DetailsComplete {
        content: serde_json::Value,
    },
    #[serde(rename = "resources_complete")]
    ResourcesComplete {
        content: serde_json::Value,
    },
    #[serde(rename = "complete")]
    Complete {
        final_data: serde_json::Value,
    },
    #[serde(rename = "error")]
    Error {
        message: String,
        stage: String,
    },
}

/// SSE 格式化
fn format_sse_event(event: &ProgressEvent) -> String {
    let data = serde_json::to_string(event).unwrap_or_else(|_| "{}".to_string());
    format!("data: {}\n\n", data)
}

/// 漸進式職業任務生成 (SSE)
///
/// 使用 Server-Sent Events 即時推送生成進度
pub async fn generate_career_tasks_progressive_sse(
    rb: web::Data<RBatis>,
    request: web::Json<ProgressiveGenerationRequest>,
    config: web::Data<crate::config::Config>,
) -> Result<HttpResponse> {
    log::info!("🚀 開始 SSE 漸進式生成職業任務：{}", request.selected_career);

    let req = request.into_inner();
    let rb_clone = rb.clone();
    let config_clone = config.clone();

    // 創建 SSE 通道
    let (tx, mut rx) = mpsc::channel::<ProgressEvent>(100);

    // 在背景執行生成邏輯
    tokio::spawn(async move {
        if let Err(e) = run_progressive_generation(rb_clone, req, config_clone, tx.clone()).await {
            log::error!("生成任務時發生錯誤: {}", e);
            let _ = tx.send(ProgressEvent::Error {
                message: e.to_string(),
                stage: "unknown".to_string(),
            }).await;
        }
    });

    // 建立 SSE 串流
    let stream = async_stream::stream! {
        while let Some(event) = rx.recv().await {
            yield Ok::<_, actix_web::Error>(
                web::Bytes::from(format_sse_event(&event))
            );
        }
    };

    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(Box::pin(stream)))
}

/// 執行漸進式生成邏輯
async fn run_progressive_generation(
    rb: web::Data<RBatis>,
    request: ProgressiveGenerationRequest,
    config: web::Data<crate::config::Config>,
    tx: mpsc::Sender<ProgressEvent>,
) -> anyhow::Result<()> {

    // 發送初始狀態
    tx.send(ProgressEvent::Status {
        stage: "init".to_string(),
        message: "初始化任務生成系統...".to_string(),
        progress: 0,
    }).await?;

    // 獲取用戶 ID
    let user_id = if let Some(uid) = &request.user_id {
        uid.clone()
    } else {
        match rb.query_decode::<Vec<crate::models::User>>("SELECT id FROM user LIMIT 1", vec![]).await {
            Ok(users) if !users.is_empty() => users[0].id.clone().unwrap_or_default(),
            _ => {
                return Err(anyhow::anyhow!("找不到用戶"));
            }
        }
    };

    // 創建 AI 服務
    let ai_service = crate::ai_service::create_ai_service(&config.app.ai)?;

    // 獲取測驗結果
    tx.send(ProgressEvent::Status {
        stage: "loading".to_string(),
        message: "載入測驗結果...".to_string(),
        progress: 5,
    }).await?;

    let quiz_result = crate::career_routes::get_quiz_result(&rb, &request.quiz_result_id).await
        .map_err(|e| anyhow::anyhow!("獲取測驗結果失敗: {}", e))?;

    // ===== 階段 1：大綱生成 =====
    tx.send(ProgressEvent::Status {
        stage: "outline".to_string(),
        message: format!("正在生成任務大綱（使用模型：{}）...", config.app.ai.outline_model),
        progress: 10,
    }).await?;

    let outline_prompt = build_outline_prompt(&quiz_result, &request.selected_career, &request.survey_answers);

    let outline_result = ai_service.generate_with_model(&config.app.ai.outline_model, &outline_prompt).await?;

    // 解析大綱結果
    let outline_json: serde_json::Value = serde_json::from_str(&outline_result.trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim())?;

    tx.send(ProgressEvent::OutlineComplete {
        content: outline_json.clone(),
    }).await?;

    tx.send(ProgressEvent::Status {
        stage: "outline_done".to_string(),
        message: "✅ 大綱生成完成".to_string(),
        progress: 35,
    }).await?;

    // ===== 階段 2：細節擴展 =====
    tx.send(ProgressEvent::Status {
        stage: "details".to_string(),
        message: format!("正在擴展任務細節（使用模型：{}）...", config.app.ai.detail_model),
        progress: 40,
    }).await?;

    let detail_prompt = build_detail_prompt(&outline_result, &request.selected_career);

    let detailed_result = ai_service.generate_with_model(&config.app.ai.detail_model, &detail_prompt).await?;

    // 解析細節結果
    let tasks_response = crate::career_routes::parse_ai_tasks_response(&detailed_result)
        .map_err(|e| anyhow::anyhow!("解析任務失敗: {}", e))?;
    let tasks_json = serde_json::to_value(&tasks_response)?;

    tx.send(ProgressEvent::DetailsComplete {
        content: tasks_json.clone(),
    }).await?;

    tx.send(ProgressEvent::Status {
        stage: "details_done".to_string(),
        message: "✅ 細節擴展完成".to_string(),
        progress: 70,
    }).await?;

    // ===== 階段 3：資源推薦 =====
    log::info!("🔍 開始階段 3：資源推薦（模型：{}）", config.app.ai.resource_model);

    tx.send(ProgressEvent::Status {
        stage: "resources".to_string(),
        message: format!("正在搜尋學習資源（使用模型：{}）...", config.app.ai.resource_model),
        progress: 75,
    }).await?;

    let resource_prompt = build_resource_prompt(&detailed_result, &request.selected_career);
    let preview = resource_prompt.chars().take(200).collect::<String>();
    log::debug!("資源推薦 prompt 前 200 字元: {}", preview);

    log::info!("📡 呼叫 Perplexity API 進行資源搜尋...");
    let resource_result = ai_service.generate_with_model(&config.app.ai.resource_model, &resource_prompt).await
        .unwrap_or_else(|e| {
            log::warn!("⚠️  資源推薦失敗（非致命）: {}", e);
            "{}".to_string()
        });

    log::info!("✅ 資源推薦 API 呼叫完成，回應長度: {} 字元", resource_result.len());

    // 保存 Perplexity 原始回應以供調試
    if let Err(e) = std::fs::write("perplexity_resources.json", &resource_result) {
        log::warn!("無法保存 Perplexity 回應: {}", e);
    } else {
        log::info!("✅ Perplexity 原始回應已保存到 perplexity_resources.json");
    }

    // 解析資源結果
    let cleaned_resource_result = resource_result.trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let resources_json: serde_json::Value = serde_json::from_str(cleaned_resource_result)
        .unwrap_or_else(|e| {
            log::error!("❌ 資源 JSON 解析失敗: {}", e);
            log::error!("前 500 字元: {}", truncate_str_safe(cleaned_resource_result, 500));
            serde_json::json!({"resources": []})
        });

    log::info!("📊 解析後的資源數據: {}", serde_json::to_string_pretty(&resources_json).unwrap_or_else(|_| "無法序列化".to_string()));

    tx.send(ProgressEvent::ResourcesComplete {
        content: resources_json.clone(),
    }).await?;

    tx.send(ProgressEvent::Status {
        stage: "resources_done".to_string(),
        message: "✅ 資源推薦完成".to_string(),
        progress: 95,
    }).await?;

    // ===== 最終合併 =====
    tx.send(ProgressEvent::Status {
        stage: "finalizing".to_string(),
        message: "正在整合所有結果...".to_string(),
        progress: 98,
    }).await?;

    // 為每個任務添加經驗值（根據難度計算）
    let process_task = |task: crate::models::GeneratedTask| -> serde_json::Value {
        let experience = match task.difficulty {
            1 => 15,
            2 => 25,
            3 => 35,
            4 => 50,
            5 => 75,
            _ => 25,
        };

        // 將 GeneratedTask 轉換為 JSON，並添加 experience 欄位
        let mut task_json = serde_json::to_value(&task).unwrap_or(serde_json::json!({}));
        task_json["experience"] = serde_json::json!(experience);
        task_json
    };

    // 處理所有任務類型
    let processed_main_tasks: Vec<serde_json::Value> = tasks_response.main_tasks
        .into_iter()
        .map(process_task)
        .collect();

    let processed_daily_tasks: Vec<serde_json::Value> = tasks_response.daily_tasks
        .into_iter()
        .map(process_task)
        .collect();

    let processed_project_tasks: Vec<serde_json::Value> = tasks_response.project_tasks
        .into_iter()
        .map(process_task)
        .collect();

    let final_data = serde_json::json!({
        "preview_mode": true,
        "quiz_result_id": request.quiz_result_id,
        "selected_career": request.selected_career,
        "user_id": user_id,
        "survey_answers": request.survey_answers,
        "outline": outline_json,
        "tasks": tasks_json,
        "resources": resources_json,
        "learning_summary": tasks_response.learning_summary,
        "personality_insights": tasks_response.personality_insights,
        "estimated_months": tasks_response.estimated_months,
        "total_tasks": processed_main_tasks.len() + processed_daily_tasks.len() + processed_project_tasks.len(),
        "main_tasks": processed_main_tasks,
        "daily_tasks": processed_daily_tasks,
        "project_tasks": processed_project_tasks,
    });

    tx.send(ProgressEvent::Complete {
        final_data,
    }).await?;

    tx.send(ProgressEvent::Status {
        stage: "complete".to_string(),
        message: "🎉 任務生成完成！".to_string(),
        progress: 100,
    }).await?;

    Ok(())
}

// ============= Prompt 建構函數 =============

fn build_outline_prompt(
    quiz_result: &crate::models::QuizResults,
    career: &str,
    survey_answers: &SurveyAnswers,
) -> String {
    format!(
        r#"你是專業的職涯規劃師。請為「{}」職業生成學習主線任務大綱。

## 用戶資料
- 價值觀偏好：{}
- 興趣領域：{}
- 天賦特質：{}
- 工作風格：{}
- 當前程度：{}
- 可用時間：{}
- 期望時程：{}

## 要求
請生成 8-12 個主要學習里程碑，每個里程碑包含：
1. 里程碑名稱
2. 預估學習時數
3. 難度級別（1-5）
4. 簡短描述（1-2 句話）

請以 JSON 格式回應：
```json
{{
  "learning_summary": "整體學習路徑說明（2-3 句話）",
  "estimated_months": 6,
  "milestones": [
    {{
      "title": "里程碑標題",
      "estimated_hours": 20,
      "difficulty": 3,
      "description": "簡短描述"
    }}
  ]
}}
```

**重要：只回傳 JSON，不要其他文字。必須使用繁體中文。**"#,
        career,
        extract_quiz_summary(&quiz_result.values_results),
        extract_quiz_summary(&quiz_result.interests_results),
        extract_quiz_summary(&quiz_result.talents_results),
        extract_quiz_summary(&quiz_result.workstyle_results),
        survey_answers.current_level,
        survey_answers.available_time,
        survey_answers.timeline,
    )
}

fn build_detail_prompt(outline: &str, career: &str) -> String {
    let prompt = crate::career_routes::build_career_task_prompt(
        &crate::models::QuizResults::default(),
        career,
        &SurveyAnswers::default()
    );

    format!(
        r#"{}

## 已生成的大綱
{}

## ⚠️ 重要約束條件
請基於以上大綱，生成完整的任務細節。

**🔴 絕對必須遵守以下規則：**
1. **任務標題 (title) 必須與大綱中的 milestone.title 完全一致，一字不改**
2. 可以在描述 (description) 中自由擴展細節
3. 可以添加學習目標、執行步驟、完成標準等內容
4. **但 title 欄位必須與大綱保持 100% 相同**

範例：
- 大綱中的 title: "基礎解剖學入門"
- 你的輸出 title: "基礎解剖學入門" ✅ 正確
- 你的輸出 title: "解剖學基礎知識" ❌ 錯誤（改了標題）
- 你的輸出 title: "基礎解剖學入門與實踐" ❌ 錯誤（添加了內容）

**必須嚴格遵守原始 prompt 的所有格式要求。**"#,
        prompt,
        outline
    )
}

fn build_resource_prompt(tasks_json: &str, career: &str) -> String {
    // 解析 JSON 並提取所有任務標題
    let task_titles = extract_task_titles_from_json(tasks_json);
    let task_count = task_titles.len();
    let task_list = task_titles.iter()
        .enumerate()
        .map(|(i, title)| format!("{}. {}", i + 1, title))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"你現在必須扮演一個**網路資源搜尋機器人**。你的工作是為「{}」相關學習任務找到**真實存在、可以點擊訪問的網路資源**。

## ⚠️ 關鍵要求
1. **必須提供完整的 https:// 開頭的 URL**
2. **不允許使用通用名稱**（如「SQL教程」「課程視頻」）
3. **每個資源都必須是你剛搜尋到的真實網站**
4. **如果找不到繁體中文資源，請提供英文資源並註明**
5. **🔴 絕對必須為以下列表中的【每一個任務】都生成至少 1-2 個資源推薦**
6. **🔴 不要遺漏任何一個任務，總共有 {} 個任務需要生成資源**

## 📋 任務列表（共 {} 個任務，每個都必須有資源）
{}

## 🔍 執行搜尋（請逐一執行以下搜尋）
請依序搜尋並記錄結果：

### 搜尋 1: 台灣線上課程平台
- 關鍵字：「{} 課程 hahow」
- 關鍵字：「{} 教學 pressplay」
- 關鍵字：「{} udemy 繁體中文」

### 搜尋 2: YouTube 繁體中文教學
- 關鍵字：「{} 教學 youtube 中文」
- 關鍵字：「{} tutorial youtube 繁中字幕」

### 搜尋 3: 技術文章與部落格
- 關鍵字：「{} 學習筆記 medium」
- 關鍵字：「{} 教程 2024 繁體」

## 📤 輸出要求

### 搜尋策略
1. **必須搜尋最新的資源** (2023-2025年)
2. **優先搜尋繁體中文資源**，如果沒有才推薦英文
3. **每個資源都必須提供真實的 URL**
4. 優先順序：
   - 台灣/香港的線上課程平台 (Hahow, PressPlay, Coursera 繁中版)
   - 繁體中文技術部落格和教學網站
   - YouTube 繁體中文教學頻道
   - 免費或高品質的付費資源

### 搜尋關鍵字範例
- "產品經理 課程 繁體中文 2024"
- "PM 線上學習 台灣"
- "產品設計 教學 Hahow"

## 📤 輸出格式範例

**錯誤示範（禁止）：**
```json
{{
  "recommendations": [{{
    "title": "SQL教程",  // ❌ 太籠統
    "url": "https://example.com"  // ❌ 假網址
  }}]
}}
```

**正確示範（必須）：**
```json
{{
  "search_timestamp": "2025-01-18",
  "search_queries_used": ["資料庫管理員 課程 hahow", "SQL 教學 youtube"],
  "resources": [
    {{
      "task_title": "SQL語法入門",
      "recommendations": [
        {{
          "title": "零基礎SQL資料庫語言入門 - Hahow",
          "type": "線上課程",
          "platform": "Hahow",
          "url": "https://hahow.in/courses/5f8a1b2c3d4e5f6g7h8i9j0k",
          "instructor": "講師名稱",
          "description": "適合零基礎學員，包含MySQL實戰演練",
          "language": "繁體中文",
          "price": "NT$1,800",
          "rating": "4.8/5.0",
          "students": "2,500+",
          "source": "從 Hahow 搜尋結果第1項"
        }},
        {{
          "title": "SQL Tutorial - Full Database Course for Beginners",
          "type": "影片教學",
          "platform": "YouTube",
          "url": "https://www.youtube.com/watch?v=HXV3zeQKqGY",
          "channel": "freeCodeCamp.org",
          "description": "4小時完整SQL教學，適合初學者",
          "language": "English（有繁中字幕）",
          "price": "免費",
          "views": "8M+",
          "source": "從 YouTube 搜尋結果第1項"
        }}
      ]
    }}
  ]
}}
```

## 重要提醒
- ⚠️ **必須搜尋真實資源**，不要編造 URL
- ⚠️ **URL 必須完整且可訪問**（以 https:// 開頭）
- ⚠️ **優先推薦 2024-2025 年的最新內容**
- ⚠️ **只回傳 JSON，不要其他文字**
- ⚠️ **如果搜尋不到繁體中文資源，才推薦高品質英文資源**
- ⚠️ **所有輸出內容（包括 description、title 等）必須使用繁體中文書寫**

現在請開始搜尋並推薦資源："#,
        career,      // 第1個: 主標題
        task_count,  // 任務總數 (第2次)
        task_count,  // 任務總數 (第3次)
        task_list,   // 任務列表
        career,      // 搜尋1-1
        career,      // 搜尋1-2
        career,      // 搜尋1-3
        career,      // 搜尋2-1
        career,      // 搜尋2-2
        career,      // 搜尋3-1
        career       // 搜尋3-2
    )
}

/// 從 JSON 字符串中提取所有任務標題
fn extract_task_titles_from_json(tasks_json: &str) -> Vec<String> {
    let mut titles = Vec::new();

    //  解析 JSON
    let cleaned = tasks_json.trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(cleaned) {
        // 提取 main_tasks
        if let Some(main_tasks) = parsed.get("main_tasks").and_then(|v| v.as_array()) {
            for task in main_tasks {
                if let Some(title) = task.get("title").and_then(|v| v.as_str()) {
                    titles.push(title.to_string());
                }
            }
        }

        // 提取 daily_tasks
        if let Some(daily_tasks) = parsed.get("daily_tasks").and_then(|v| v.as_array()) {
            for task in daily_tasks {
                if let Some(title) = task.get("title").and_then(|v| v.as_str()) {
                    titles.push(title.to_string());
                }
            }
        }

        // 提取 project_tasks
        if let Some(project_tasks) = parsed.get("project_tasks").and_then(|v| v.as_array()) {
            for task in project_tasks {
                if let Some(title) = task.get("title").and_then(|v| v.as_str()) {
                    titles.push(title.to_string());
                }
            }
        }
    }

    titles
}

/// 安全地截取字符串，避免在 UTF-8 字符邊界中間截斷
fn truncate_str_safe(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }

    // 從 max_bytes 開始往前找，直到找到字符邊界
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }

    &s[..end]
}

fn extract_quiz_summary(quiz_json: &Option<String>) -> String {
    match quiz_json {
        Some(json_str) => {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                format!("{}", parsed)
            } else {
                "解析中".to_string()
            }
        }
        None => "無資料".to_string()
    }
}

// 為缺少的 Default 實作
impl Default for crate::models::QuizResults {
    fn default() -> Self {
        Self {
            id: None,
            user_id: None,
            values_results: None,
            interests_results: None,
            talents_results: None,
            workstyle_results: None,
            completed_at: None,
            is_active: None,
            created_at: None,
            updated_at: None,
        }
    }
}

// SurveyAnswers 的 Default 已在 models.rs 中定義，這裡移除重複實作
