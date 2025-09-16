use actix_web::{web, HttpResponse, Result};
use rbatis::RBatis;
use uuid::Uuid;
use chrono::Utc;
use serde_json;
use log;

use crate::models::{
    QuizResults, CareerMainlines, Task, ChatMessage, User,
    SaveQuizResultsRequest, GenerateCareerTasksRequest, 
    GeneratedTasksResponse, GeneratedTask, SurveyAnswers
};
use crate::ai_tasks::ApiResponse;
use crate::ai_service::OpenAIService;

// ============= 測驗結果相關 API =============

pub async fn save_quiz_results(
    rb: web::Data<RBatis>,
    request: web::Json<SaveQuizResultsRequest>
) -> Result<HttpResponse> {
    log::info!("開始保存測驗結果");

    let quiz_id = Uuid::new_v4().to_string();
    // 使用第一個用戶ID（與任務系統保持一致）
    let user_id = match rb.query_decode::<Vec<crate::models::User>>("SELECT id FROM user LIMIT 1", vec![]).await {
        Ok(users) => {
            if users.is_empty() {
                return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: "系統中沒有用戶，請先創建用戶".to_string(),
                }));
            }
            users[0].id.clone().unwrap_or_default()
        }
        Err(e) => {
            log::error!("查詢用戶失敗: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: "數據庫查詢失敗".to_string(),
            }));
        }
    };
    
    let now = Utc::now();

    // 創建測驗結果記錄
    let quiz_result = QuizResults {
        id: Some(quiz_id.clone()),
        user_id: Some(user_id.clone()),
        values_results: Some(serde_json::to_string(&request.values_results)?),
        interests_results: Some(serde_json::to_string(&request.interests_results)?),
        talents_results: Some(serde_json::to_string(&request.talents_results)?),
        workstyle_results: Some(serde_json::to_string(&request.workstyle_results)?),
        completed_at: Some(now),
        is_active: Some(1),
        created_at: Some(now),
    };

    // 先停用之前的測驗結果
    let sql_deactivate = "UPDATE quiz_results SET is_active = 0 WHERE user_id = ?";
    if let Err(e) = rb.exec(sql_deactivate, vec![rbs::Value::String(user_id.clone())]).await {
        log::error!("停用舊測驗結果失敗: {}", e);
    }

    // 保存新的測驗結果
    match QuizResults::insert(rb.get_ref(), &quiz_result).await {
        Ok(_) => {
            log::info!("✅ 測驗結果保存成功: {}", quiz_id);
            Ok(HttpResponse::Ok().json(ApiResponse {
                success: true,
                data: Some(serde_json::json!({
                    "quiz_result_id": quiz_id,
                    "message": "測驗結果已保存"
                })),
                message: "測驗結果保存成功".to_string(),
            }))
        }
        Err(e) => {
            log::error!("❌ 測驗結果保存失敗: {}", e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("保存失敗: {}", e),
            }))
        }
    }
}

// ============= 職業任務生成相關 API =============

pub async fn generate_career_tasks(
    rb: web::Data<RBatis>,
    request: web::Json<GenerateCareerTasksRequest>
) -> Result<HttpResponse> {
    log::info!("開始生成職業任務: 職業={}, 測驗ID={}", 
               request.selected_career, request.quiz_result_id);

    // 獲取第一個用戶ID（與測驗結果保存保持一致）
    let user_id = match rb.query_decode::<Vec<crate::models::User>>("SELECT id FROM user LIMIT 1", vec![]).await {
        Ok(users) => {
            if users.is_empty() {
                return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: "系統中沒有用戶，請先創建用戶".to_string(),
                }));
            }
            users[0].id.clone().unwrap_or_default()
        }
        Err(e) => {
            log::error!("查詢用戶失敗: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: "數據庫查詢失敗".to_string(),
            }));
        }
    };

    // 1. 獲取測驗結果
    let quiz_result = match get_quiz_result(&rb, &request.quiz_result_id).await {
        Ok(result) => result,
        Err(e) => {
            log::error!("獲取測驗結果失敗: {}", e);
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: "找不到測驗結果".to_string(),
            }));
        }
    };

    // 2. 構建 AI 提示詞
    let ai_prompt = build_career_task_prompt(&quiz_result, &request.selected_career, &request.survey_answers);
    log::debug!("AI 提示詞: {}", ai_prompt);

    // 3. 調用 AI 服務生成任務
    let generation_start = std::time::Instant::now();
    let api_key = std::env::var("OPENAI_API_KEY")
        .unwrap_or_else(|_| "dummy-key-for-demo".to_string());
    let ai_service = OpenAIService::new(api_key);
    let ai_response = match ai_service.generate_task_preview(&ai_prompt).await {
        Ok(response) => response,
        Err(e) => {
            log::error!("AI 任務生成失敗: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: "AI 服務暫時無法使用".to_string(),
            }));
        }
    };

    let generation_time = generation_start.elapsed().as_millis();
    log::info!("🤖 AI 生成完成，耗時: {}ms", generation_time);

    // 4. 解析 AI 回應
    let generated_tasks = match parse_ai_tasks_response(&ai_response) {
        Ok(tasks) => tasks,
        Err(e) => {
            log::error!("解析 AI 回應失敗: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: "AI 回應格式錯誤".to_string(),
            }));
        }
    };

    // 5. 創建職業主線記錄
    let mainline_id = Uuid::new_v4().to_string();
    let total_tasks = generated_tasks.main_tasks.len() + 
                     generated_tasks.daily_tasks.len() + 
                     generated_tasks.project_tasks.len();

    let career_mainline = CareerMainlines {
        id: Some(mainline_id.clone()),
        user_id: Some(user_id.clone()),
        quiz_result_id: Some(request.quiz_result_id.clone()),
        selected_career: Some(request.selected_career.clone()),
        survey_answers: Some(serde_json::to_string(&request.survey_answers)?),
        total_tasks_generated: Some(total_tasks as i32),
        estimated_completion_months: Some(generated_tasks.estimated_months),
        status: Some("active".to_string()),
        progress_percentage: Some(0.0),
        created_at: Some(Utc::now()),
        updated_at: Some(Utc::now()),
    };

    if let Err(e) = CareerMainlines::insert(rb.get_ref(), &career_mainline).await {
        log::error!("創建職業主線失敗: {}", e);
        return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: "創建學習主線失敗".to_string(),
        }));
    }

    // 6. 建立職業主線父任務
    let parent_task_id = Uuid::new_v4().to_string();
    let now = Utc::now();

    let parent_task = Task {
        id: Some(parent_task_id.clone()),
        user_id: Some(user_id.clone()),
        title: Some(format!("職業主線：{}", request.selected_career)),
        description: Some(format!("{}\n\n📋 包含 {} 個子任務，完成後將掌握相關職業技能。\n\n🎯 預計學習時程：{} 個月",
                                generated_tasks.learning_summary,
                                total_tasks,
                                generated_tasks.estimated_months)),
        status: Some(0), // pending
        priority: Some(2), // 高優先級
        task_type: Some("main".to_string()),
        difficulty: Some(3),
        experience: Some(100), // 父任務給予較高經驗值
        career_mainline_id: Some(mainline_id.clone()),
        task_category: Some("career_mainline".to_string()),
        is_parent_task: Some(1), // 標記為父任務
        task_order: Some(0),
        created_at: Some(now),
        updated_at: Some(now),
        // 其他欄位使用預設值
        parent_task_id: None,
        due_date: None,
        is_recurring: Some(0),
        recurrence_pattern: None,
        start_date: None,
        end_date: None,
        completion_target: Some(1.0),
        completion_rate: Some(0.0),
        task_date: None,
        cancel_count: Some(0),
        last_cancelled_at: None,
        skill_tags: None,
    };

    // 保存父任務
    if let Err(e) = Task::insert(rb.get_ref(), &parent_task).await {
        log::error!("創建職業主線父任務失敗: {}", e);
        return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: "創建職業主線失敗".to_string(),
        }));
    }

    log::info!("✅ 創建職業主線父任務: {}", parent_task_id);

    // 7. 將生成的任務插入資料庫作為子任務
    let mut created_tasks = Vec::new();
    let mut task_order = 1;

    // 統一創建所有子任務為同一類型，確保循序漸進的學習體驗

    // 創建主要任務（作為子任務）
    for ai_task in &generated_tasks.main_tasks {
        match create_subtask_from_ai_data(&rb, &user_id, &mainline_id, &parent_task_id, ai_task, "career_subtask", task_order).await {
            Ok(task) => {
                created_tasks.push(task);
                task_order += 1;
            }
            Err(e) => log::error!("創建學習子任務失敗: {}", e),
        }
    }

    // 創建每日任務（作為子任務）
    for ai_task in &generated_tasks.daily_tasks {
        match create_subtask_from_ai_data(&rb, &user_id, &mainline_id, &parent_task_id, ai_task, "career_subtask", task_order).await {
            Ok(task) => {
                created_tasks.push(task);
                task_order += 1;
            }
            Err(e) => log::error!("創建學習子任務失敗: {}", e),
        }
    }

    // 創建項目任務（作為子任務）
    for ai_task in &generated_tasks.project_tasks {
        match create_subtask_from_ai_data(&rb, &user_id, &mainline_id, &parent_task_id, ai_task, "career_subtask", task_order).await {
            Ok(task) => {
                created_tasks.push(task);
                task_order += 1;
            }
            Err(e) => log::error!("創建學習子任務失敗: {}", e),
        }
    }

    log::info!("✅ 成功創建 {} 個任務", created_tasks.len());

    // 7. 記錄到聊天記錄（作為 AI 互動記錄）
    let chat_message = crate::models::ChatMessage {
        id: Some(Uuid::new_v4().to_string()),
        user_id: Some(user_id),
        role: Some("assistant".to_string()),
        content: Some(format!("為您的「{}」職業規劃生成了 {} 個學習任務：\n\n{}", 
                             request.selected_career, 
                             created_tasks.len(),
                             generated_tasks.learning_summary)),
        created_at: Some(Utc::now()),
    };

    if let Err(e) = ChatMessage::insert(rb.get_ref(), &chat_message).await {
        log::warn!("保存聊天記錄失敗: {}", e);
    }

    // 8. 返回成功回應
    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        data: Some(serde_json::json!({
            "mainline_id": mainline_id,
            "parent_task_id": parent_task_id,
            "parent_task": {
                "id": parent_task_id,
                "title": format!("職業主線：{}", request.selected_career),
                "description": format!("{}\n\n📋 包含 {} 個子任務，完成後將掌握相關職業技能。",
                                     generated_tasks.learning_summary, total_tasks),
                "subtasks_count": created_tasks.len()
            },
            "subtasks_created": created_tasks.len(),
            "learning_summary": generated_tasks.learning_summary,
            "estimated_months": generated_tasks.estimated_months,
            "personality_insights": generated_tasks.personality_insights,
            "subtasks": created_tasks
        })),
        message: format!("🎉 成功創建職業主線「{}」，包含 {} 個子任務！", request.selected_career, created_tasks.len()),
    }))
}

// ============= 輔助函數 =============

// 安全截斷字符串，避免在 UTF-8 字符邊界中間截斷
fn safe_substring(s: &str, start: usize, end: usize) -> &str {
    let start_pos = if start >= s.len() { s.len() } else { start };
    let end_pos = if end > s.len() { s.len() } else { end };
    
    // 找到有效的字符邊界
    let start_boundary = s.char_indices()
        .find(|(i, _)| *i >= start_pos)
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    
    let end_boundary = s.char_indices()
        .rev()
        .find(|(i, _)| *i <= end_pos)
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    
    if start_boundary <= end_boundary {
        &s[start_boundary..end_boundary]
    } else {
        ""
    }
}

async fn get_quiz_result(rb: &RBatis, quiz_result_id: &str) -> Result<QuizResults, Box<dyn std::error::Error>> {
    let sql = "SELECT id, user_id, values_results, interests_results, talents_results, workstyle_results, completed_at, is_active, created_at FROM quiz_results WHERE id = ? AND is_active = 1";
    
    // 先用原始查詢獲取數據
    let raw_results: Vec<serde_json::Value> = rb.query_decode(sql, vec![rbs::Value::String(quiz_result_id.to_string())]).await?;
    
    if let Some(row) = raw_results.first() {
        let quiz_result = QuizResults {
            id: row.get("id").and_then(|v| v.as_str()).map(String::from),
            user_id: row.get("user_id").and_then(|v| v.as_str()).map(String::from),
            values_results: row.get("values_results").and_then(|v| v.as_str()).map(String::from),
            interests_results: row.get("interests_results").and_then(|v| v.as_str()).map(String::from),
            talents_results: row.get("talents_results").and_then(|v| v.as_str()).map(String::from),
            workstyle_results: row.get("workstyle_results").and_then(|v| v.as_str()).map(String::from),
            completed_at: row.get("completed_at").and_then(|v| v.as_str()).and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|dt| dt.with_timezone(&chrono::Utc)),
            is_active: row.get("is_active").and_then(|v| v.as_i64()).map(|i| i as i32),
            created_at: row.get("created_at").and_then(|v| v.as_str()).and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|dt| dt.with_timezone(&chrono::Utc)),
        };
        Ok(quiz_result)
    } else {
        Err("測驗結果不存在或已過期".into())
    }
}

fn build_career_task_prompt(
    quiz_result: &QuizResults, 
    selected_career: &str, 
    survey_answers: &SurveyAnswers
) -> String {
    format!(r#"
你是專業的職涯規劃師。基於用戶的完整檔案，為「{career}」職業設計個人化學習任務。

## 用戶完整檔案

### 個性測驗結果
- 價值觀偏好：{values}
- 興趣領域：{interests}  
- 天賦特質：{talents}
- 工作風格：{workstyle}

### 職業選擇與偏好
- 目標職業：{career}
- 當前程度：{current_level}
- 可用時間：{available_time}
- 學習方式：{learning_styles}
- 期望時程：{timeline}
- 學習動機：{motivation}

## 任務生成要求

請生成 15-18 個學習任務，分為三類：

### 1. 主線任務 (6-8個)
- 核心技能學習，難度循序漸進
- 每個任務都有明確的學習成果
- 根據用戶個性特質調整學習方式

### 2. 每日任務 (4-5個)  
- 培養職業相關的日常習慣
- 每個任務15-30分鐘可完成
- 重複執行有助於技能累積

### 3. 項目任務 (4-5個)
- 實戰練習和作品集建立
- 難度較高，需要綜合運用所學
- 有助於建立職業競爭力

## 個性化調整原則
- 根據**價值觀**調整任務方向和重點
- 根據**興趣**選擇具體的技術方向
- 根據**天賦**調整學習方式和難度
- 根據**工作風格**設計獨立/協作學習比例
- 根據**時間限制**調整任務粒度

## 嚴格 JSON 格式要求

**重要：**
1. 回應必須是有效的JSON格式，不包含額外文字
2. 所有字符串必須用雙引號包圍  
3. 不能有尾隨逗號
4. 所有必需字段都必須存在
5. difficulty 必須是 1-5 的整數
6. estimated_hours 必須是正整數
7. skill_tags 和 resources 必須是字符串陣列

```json
{{
  "learning_summary": "基於用戶特質的學習路徑總結，說明整體規劃思路",
  "estimated_months": 8,
  "personality_insights": "個性特質如何影響這個學習計劃的分析",
  "main_tasks": [
    {{
      "title": "主線任務標題1",
      "description": "詳細說明任務內容和學習目標",
      "difficulty": 3,
      "estimated_hours": 20,
      "skill_tags": ["核心技能1", "核心技能2"],
      "resources": ["學習資源1", "學習資源2"],
      "personality_match": "為什麼這個任務適合用戶的個性特質"
    }},
    {{
      "title": "主線任務標題2",
      "description": "詳細說明任務內容和學習目標",
      "difficulty": 4,
      "estimated_hours": 25,
      "skill_tags": ["核心技能3", "核心技能4"],
      "resources": ["學習資源3", "學習資源4"],
      "personality_match": "個性化匹配說明"
    }}
  ],
  "daily_tasks": [
    {{
      "title": "每日任務標題1",
      "description": "每日執行的習慣性任務說明",
      "difficulty": 2,
      "estimated_hours": 1,
      "skill_tags": ["日常技能1"],
      "resources": ["資源1"],
      "personality_match": "個性化匹配說明"
    }}
  ],
  "project_tasks": [
    {{
      "title": "項目任務標題1",
      "description": "實戰項目的具體要求和目標",
      "difficulty": 5,
      "estimated_hours": 40,
      "skill_tags": ["實戰技能1", "綜合技能2"],
      "resources": ["項目資源1", "項目資源2"],
      "personality_match": "個性化匹配說明"
    }}
  ]
}}
```

**請嚴格按照上述JSON格式回應，確保每個任務對象都包含所有必需字段：title, description, difficulty, estimated_hours, skill_tags, resources, personality_match。使用繁體中文內容，但JSON結構必須完全符合格式要求。**
"#, 
        career = selected_career,
        values = extract_quiz_summary(&quiz_result.values_results),
        interests = extract_quiz_summary(&quiz_result.interests_results),
        talents = extract_quiz_summary(&quiz_result.talents_results),
        workstyle = extract_quiz_summary(&quiz_result.workstyle_results),
        current_level = survey_answers.current_level,
        available_time = survey_answers.available_time,
        learning_styles = survey_answers.learning_styles.join("、"),
        timeline = survey_answers.timeline,
        motivation = survey_answers.motivation.as_ref().unwrap_or(&"提升個人能力".to_string())
    )
}

fn extract_quiz_summary(quiz_json: &Option<String>) -> String {
    // 簡化處理：從JSON中提取關鍵資訊
    // TODO: 實現更詳細的測驗結果解析
    match quiz_json {
        Some(json_str) => {
            // 嘗試解析JSON並提取關鍵資訊
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                // 這裡可以根據實際的測驗結果結構來提取資訊
                format!("測驗結果：{}", parsed.to_string())
            } else {
                "測驗結果解析中".to_string()
            }
        }
        None => "無測驗結果".to_string()
    }
}

fn parse_ai_tasks_response(ai_response: &str) -> Result<GeneratedTasksResponse, Box<dyn std::error::Error>> {
    // 清理 AI 回應，移除可能的 markdown 標記和多餘空白
    let cleaned_response = ai_response
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    
    log::debug!("清理後的 AI 回應: {}", cleaned_response);
    
    // 檢查是否為有效 JSON 開頭
    if !cleaned_response.starts_with('{') {
        log::error!("❌ AI 回應不是有效的 JSON 格式，未以 {{ 開頭");
        log::error!("前 200 個字符: {}", &cleaned_response[..std::cmp::min(200, cleaned_response.len())]);
        return Err("AI 回應格式錯誤：不是有效的 JSON".into());
    }
    
    // 嘗試解析 JSON
    match serde_json::from_str::<GeneratedTasksResponse>(cleaned_response) {
        Ok(parsed) => {
            // 驗證任務數據完整性
            let main_count = parsed.main_tasks.len();
            let daily_count = parsed.daily_tasks.len();
            let project_count = parsed.project_tasks.len();
            let total_count = main_count + daily_count + project_count;
            
            log::info!("✅ 成功解析 AI 任務回應 - 主線任務: {}, 每日任務: {}, 項目任務: {}, 總計: {}", 
                      main_count, daily_count, project_count, total_count);
            
            // 驗證每個任務是否包含必需字段
            for (i, task) in parsed.main_tasks.iter().enumerate() {
                if task.difficulty < 1 || task.difficulty > 5 {
                    log::warn!("⚠️ 主線任務 {} 難度值異常: {}", i+1, task.difficulty);
                }
                if task.estimated_hours <= 0 {
                    log::warn!("⚠️ 主線任務 {} 預估時數異常: {}", i+1, task.estimated_hours);
                }
            }
            
            Ok(parsed)
        }
        Err(e) => {
            log::error!("❌ JSON 解析失敗: {}", e);
            log::error!("錯誤位置: {}", e.to_string());
            
            // 記錄更多調試信息（安全截斷字符串）
            let response_len = cleaned_response.len();
            let first_500 = safe_substring(cleaned_response, 0, 500);
            let last_500 = if response_len > 500 {
                safe_substring(cleaned_response, response_len.saturating_sub(500), response_len)
            } else {
                ""
            };
            
            log::error!("回應長度: {} 字符", response_len);
            log::error!("前 500 字符: {}", first_500);
            if !last_500.is_empty() {
                log::error!("後 500 字符: {}", last_500);
            }
            
            // 嘗試查找常見的 JSON 格式問題
            if cleaned_response.contains("\"difficulty\":") {
                log::debug!("找到 difficulty 字段定義");
            } else {
                log::error!("❌ 未找到 difficulty 字段定義");
            }
            
            if cleaned_response.contains("\"estimated_hours\":") {
                log::debug!("找到 estimated_hours 字段定義");
            } else {
                log::error!("❌ 未找到 estimated_hours 字段定義");
            }
            
            Err(Box::new(e))
        }
    }
}

async fn create_subtask_from_ai_data(
    rb: &RBatis,
    user_id: &str,
    mainline_id: &str,
    parent_task_id: &str,
    ai_task: &GeneratedTask,
    task_category: &str,
    task_order: i32,
) -> Result<Task, Box<dyn std::error::Error>> {
    let task_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    
    // 計算經驗值
    let experience = match ai_task.difficulty {
        1 => 15,
        2 => 25,
        3 => 35,
        4 => 50,
        5 => 75,
        _ => 25,
    };

    let task = Task {
        id: Some(task_id),
        user_id: Some(user_id.to_string()),
        title: Some(ai_task.title.clone()),
        description: Some(format!("{}\n\n💡 個性化說明：{}\n\n📚 推薦資源：\n{}", 
                                ai_task.description,
                                ai_task.personality_match.as_ref().unwrap_or(&"".to_string()),
                                ai_task.resources.join("\n"))),
        status: Some(0), // pending
        priority: Some(ai_task.difficulty),
        task_type: Some(task_category.to_string()),
        difficulty: Some(ai_task.difficulty),
        experience: Some(experience),
        career_mainline_id: Some(mainline_id.to_string()),
        task_category: Some(task_category.to_string()),
        skill_tags: Some(ai_task.skill_tags.clone()),
        task_order: Some(task_order),
        created_at: Some(now),
        updated_at: Some(now),
        // 設為子任務
        parent_task_id: Some(parent_task_id.to_string()),
        is_parent_task: Some(0),
        due_date: None,
        is_recurring: Some(0),
        recurrence_pattern: None,
        start_date: None,
        end_date: None,
        completion_target: Some(1.0),
        completion_rate: Some(0.0),
        task_date: None,
        cancel_count: Some(0),
        last_cancelled_at: None,
    };

    // 保存到資料庫
    Task::insert(rb, &task).await?;
    log::debug!("✅ 創建任務: {} (類型: {}, 難度: {})", 
               ai_task.title, task_category, ai_task.difficulty);
    
    Ok(task)
}