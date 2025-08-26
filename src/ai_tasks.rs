use actix_web::{web, HttpResponse, Result};
use rbatis::RBatis;
use uuid::Uuid;
use chrono::{Utc, Datelike};
use rbs::value;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::models::{Task, User, GenerateTaskRequest, TaskStatus};
use crate::ai_service::OpenAIService;

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: String,
}

// ============= 第一步：AI 生成 JSON =============

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateTaskJsonRequest {
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskInput {
    pub title: String,
    pub description: Option<String>,
    pub task_type: Option<String>,
    pub priority: Option<i32>,
    pub difficulty: Option<i32>,
    pub experience: Option<i32>,
    pub due_date: Option<String>,
    pub is_recurring: Option<bool>,
    pub recurrence_pattern: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub completion_target: Option<f64>,
}

// 簡化的任務創建請求（直接接受 AI 生成的 JSON 格式）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskFromJsonRequest {
    pub title: String,
    pub description: Option<String>,
    pub task_type: Option<String>,
    pub priority: Option<i32>,
    pub difficulty: Option<i32>,
    pub experience: Option<i32>,
    pub due_date: Option<String>,
    pub is_recurring: Option<bool>,
    pub recurrence_pattern: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub completion_target: Option<f64>,
    pub user_id: Option<String>,  // 可選的用戶 ID
}

// API 1: AI 生成符合 task_schema.md 的 JSON
pub async fn generate_task_json(
    req: web::Json<GenerateTaskJsonRequest>,
) -> Result<HttpResponse> {
    // 從環境變數獲取 OpenAI API Key
    let api_key = match std::env::var("OPENAI_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            log::error!("未設置 OPENAI_API_KEY 環境變數");
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: "未配置 OpenAI API Key，請聯繫管理員".to_string(),
            }));
        }
    };
    
    let openai_service = OpenAIService::new(api_key);
    
    // 使用 AI 生成任務 JSON
    match openai_service.generate_task_from_text(&req.description).await {
        Ok(ai_task) => {
            log::info!("AI 成功生成任務 JSON: {:?}", ai_task);
            
            // 將 AI 生成的任務轉換為符合 schema 的 JSON
            let task_json = CreateTaskInput {
                title: ai_task.title,
                description: ai_task.description,
                task_type: Some(ai_task.task_type),
                priority: Some(ai_task.priority),
                difficulty: Some(ai_task.difficulty),
                experience: Some(ai_task.experience),
                due_date: ai_task.due_date,
                is_recurring: Some(ai_task.is_recurring),
                recurrence_pattern: ai_task.recurrence_pattern,
                start_date: ai_task.start_date,
                end_date: ai_task.end_date,
                completion_target: ai_task.completion_target,
            };
            
            Ok(HttpResponse::Ok().json(ApiResponse {
                success: true,
                data: Some(task_json),
                message: "AI 成功生成任務 JSON".to_string(),
            }))
        }
        Err(e) => {
            log::error!("AI 生成任務 JSON 失敗: {}", e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("AI 生成任務 JSON 失敗: {}", e),
            }))
        }
    }
}

// ============= 第二步：JSON 插入資料庫 =============

#[derive(Debug, Serialize, Deserialize)]
pub struct InsertTaskRequest {
    pub task_json: CreateTaskInput,
    pub user_id: Option<String>,
}

// API 2: 將 JSON 轉換為任務並插入資料庫
pub async fn insert_task_from_json(
    rb: web::Data<RBatis>,
    req: web::Json<InsertTaskRequest>,
) -> Result<HttpResponse> {
    let task_input = &req.task_json;
    
    // 決定使用者 ID
    let user_id = if let Some(id) = req.user_id.clone() {
        id
    } else {
        // 查詢或建立預設測試用戶
        match User::select_by_map(rb.get_ref(), value!{"email": "test@lifeup.com"}).await {
            Ok(users) if !users.is_empty() => {
                users[0].id.clone().unwrap()
            }
            _ => {
                let test_user = User {
                    id: Some(Uuid::new_v4().to_string()),
                    name: Some("測試用戶".to_string()),
                    email: Some("test@lifeup.com".to_string()),
                    created_at: Some(Utc::now()),
                    updated_at: Some(Utc::now()),
                };
                
                match User::insert(rb.get_ref(), &test_user).await {
                    Ok(_) => {
                        log::info!("已自動建立測試用戶");
                        test_user.id.unwrap()
                    }
                    Err(e) => {
                        log::error!("建立測試用戶失敗: {}", e);
                        return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                            success: false,
                            data: None,
                            message: format!("建立測試用戶失敗: {}", e),
                        }));
                    }
                }
            }
        }
    };
    
    // 建立主任務
    let task = Task {
        id: Some(Uuid::new_v4().to_string()),
        user_id: Some(user_id.clone()),
        title: Some(task_input.title.clone()),
        description: task_input.description.clone(),
        status: Some(0), // 預設為待處理
        priority: task_input.priority,
        task_type: task_input.task_type.clone(),
        difficulty: task_input.difficulty,
        experience: task_input.experience,
        parent_task_id: None,
        is_parent_task: if task_input.is_recurring.unwrap_or(false) { Some(1) } else { Some(0) },
        task_order: Some(0),
        due_date: task_input.due_date.as_ref().and_then(|d| {
            chrono::DateTime::parse_from_rfc3339(d)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        }),
        created_at: Some(Utc::now()),
        updated_at: Some(Utc::now()),
        is_recurring: if task_input.is_recurring.unwrap_or(false) { Some(1) } else { Some(0) },
        recurrence_pattern: task_input.recurrence_pattern.clone(),
        start_date: task_input.start_date.as_ref().and_then(|d| {
            chrono::DateTime::parse_from_rfc3339(d)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        }),
        end_date: task_input.end_date.as_ref().and_then(|d| {
            chrono::DateTime::parse_from_rfc3339(d)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        }),
        completion_target: task_input.completion_target,
        completion_rate: Some(0.0),
        task_date: None,
        cancel_count: Some(0),
        last_cancelled_at: None,
        skill_tags: None,
    };
    
    // 儲存主任務到資料庫
    match Task::insert(rb.get_ref(), &task).await {
        Ok(_) => {
            log::info!("任務已成功儲存到資料庫");
            
            // 如果是重複性任務，生成子任務
            let mut daily_tasks = Vec::new();
            if task_input.is_recurring.unwrap_or(false) && task_input.recurrence_pattern.is_some() {
                let pattern = task_input.recurrence_pattern.as_deref().unwrap();
                
                // 解析開始和結束日期
                let start_date = if let Some(start_str) = &task_input.start_date {
                    chrono::DateTime::parse_from_rfc3339(start_str)
                        .ok()
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|| Utc::now())
                } else {
                    Utc::now()
                };
                
                let end_date = if let Some(end_str) = &task_input.end_date {
                    chrono::DateTime::parse_from_rfc3339(end_str)
                        .ok()
                        .map(|dt| dt.with_timezone(&Utc))
                } else {
                    Some(Utc::now() + chrono::Duration::days(90))
                };
                
                // 計算需要生成的天數
                let days_to_generate = if let Some(end) = end_date {
                    (end.date_naive() - start_date.date_naive()).num_days() + 1
                } else {
                    90
                };
                
                log::info!("準備生成 {} 天的重複性任務，模式: {}", days_to_generate, pattern);
                
                // 批量收集要插入的任務
                let mut tasks_to_insert = Vec::new();
                
                for day_offset in 0..days_to_generate {
                    let current_date = start_date + chrono::Duration::days(day_offset);
                    let weekday = current_date.weekday();
                    let date_str = current_date.format("%Y-%m-%d").to_string();
                    
                    // 根據重複模式決定是否在這一天建立任務
                    let should_create = match pattern {
                        "daily" => true,
                        "weekdays" => {
                            weekday != chrono::Weekday::Sat && 
                            weekday != chrono::Weekday::Sun
                        },
                        "weekends" => {
                            weekday == chrono::Weekday::Sat || 
                            weekday == chrono::Weekday::Sun
                        },
                        "weekly" => {
                            weekday == start_date.weekday()
                        },
                        _ => false,
                    };
                    
                    if should_create {
                        let daily_task = Task {
                            id: Some(Uuid::new_v4().to_string()),
                            user_id: Some(user_id.clone()),
                            title: Some(format!("{} - {}", task_input.title, date_str)),
                            description: task_input.description.clone(),
                            status: Some(0), // 所有新創建的任務都設為待完成
                            priority: task_input.priority,
                            task_type: Some("daily_recurring".to_string()),
                            difficulty: task_input.difficulty,
                            experience: task_input.experience,
                            parent_task_id: task.id.clone(),
                            is_parent_task: Some(0),
                            task_order: Some(day_offset as i32 + 1),
                            due_date: Some(current_date.date_naive().and_hms_opt(23, 59, 59).unwrap().and_utc()),
                            created_at: Some(Utc::now()),
                            updated_at: Some(Utc::now()),
                            is_recurring: Some(0),
                            recurrence_pattern: None,
                            start_date: None,
                            end_date: None,
                            completion_target: None,
                            completion_rate: None,
                            task_date: Some(date_str),
                            cancel_count: Some(0),
                            last_cancelled_at: None,
                            skill_tags: None,
                        };
                        
                        tasks_to_insert.push(daily_task);
                    }
                }
                
                // 批量插入所有子任務 - 使用 raw SQL 提升性能
                let total_tasks = tasks_to_insert.len();
                if !tasks_to_insert.is_empty() {
                    // 構建批量插入的 SQL 語句
                    let mut sql = String::from(r#"
                        INSERT INTO task (
                            id, user_id, title, description, status, priority, task_type, difficulty, 
                            experience, parent_task_id, is_parent_task, task_order, due_date, 
                            created_at, updated_at, is_recurring, recurrence_pattern, start_date, 
                            end_date, completion_target, completion_rate, task_date, cancel_count, 
                            last_cancelled_at, skill_tags
                        ) VALUES
                    "#);
                    
                    let mut values = Vec::new();
                    let mut placeholders = Vec::new();
                    
                    for (i, task) in tasks_to_insert.iter().enumerate() {
                        if i > 0 {
                            placeholders.push(",".to_string());
                        }
                        placeholders.push("(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)".to_string());
                        
                        values.extend_from_slice(&[
                            rbs::Value::String(task.id.clone().unwrap()),
                            rbs::Value::String(task.user_id.clone().unwrap()),
                            rbs::Value::String(task.title.clone().unwrap()),
                            task.description.as_ref().map(|d| rbs::Value::String(d.clone())).unwrap_or(rbs::Value::Null),
                            rbs::Value::I32(task.status.unwrap_or(0)),
                            rbs::Value::I32(task.priority.unwrap_or(1)),
                            rbs::Value::String(task.task_type.clone().unwrap_or("daily_recurring".to_string())),
                            rbs::Value::I32(task.difficulty.unwrap_or(1)),
                            rbs::Value::I32(task.experience.unwrap_or(10)),
                            task.parent_task_id.as_ref().map(|p| rbs::Value::String(p.clone())).unwrap_or(rbs::Value::Null),
                            rbs::Value::Bool(task.is_parent_task.unwrap_or(0) == 1),
                            rbs::Value::I32(task.task_order.unwrap_or(0)),
                            task.due_date.as_ref().map(|d| rbs::Value::String(d.format("%Y-%m-%d %H:%M:%S%.3f").to_string())).unwrap_or(rbs::Value::Null),
                            rbs::Value::String(task.created_at.unwrap().format("%Y-%m-%d %H:%M:%S%.3f").to_string()),
                            rbs::Value::String(task.updated_at.unwrap().format("%Y-%m-%d %H:%M:%S%.3f").to_string()),
                            rbs::Value::Bool(task.is_recurring.unwrap_or(0) == 1),
                            task.recurrence_pattern.as_ref().map(|r| rbs::Value::String(r.clone())).unwrap_or(rbs::Value::Null),
                            task.start_date.as_ref().map(|s| rbs::Value::String(s.format("%Y-%m-%d %H:%M:%S%.3f").to_string())).unwrap_or(rbs::Value::Null),
                            task.end_date.as_ref().map(|e| rbs::Value::String(e.format("%Y-%m-%d %H:%M:%S%.3f").to_string())).unwrap_or(rbs::Value::Null),
                            task.completion_target.map(|c| rbs::Value::F64(c)).unwrap_or(rbs::Value::Null),
                            task.completion_rate.map(|c| rbs::Value::F64(c)).unwrap_or(rbs::Value::Null),
                            task.task_date.as_ref().map(|t| rbs::Value::String(t.clone())).unwrap_or(rbs::Value::Null),
                            rbs::Value::I32(task.cancel_count.unwrap_or(0)),
                            task.last_cancelled_at.as_ref().map(|l| rbs::Value::String(l.format("%Y-%m-%d %H:%M:%S%.3f").to_string())).unwrap_or(rbs::Value::Null),
                            task.skill_tags.as_ref().map(|s| rbs::Value::String(serde_json::to_string(s).unwrap_or("[]".to_string()))).unwrap_or(rbs::Value::Null),
                        ]);
                    }
                    
                    sql.push_str(&placeholders.join(" "));
                    
                    // 執行批量插入
                    match rb.exec(&sql, values).await {
                        Ok(_) => {
                            daily_tasks = tasks_to_insert;
                            log::info!("批量插入 {} 個子任務成功", total_tasks);
                        }
                        Err(e) => {
                            log::error!("批量插入子任務失敗: {}", e);
                            // 如果批量插入失敗，回退到逐個插入
                            for task_batch in tasks_to_insert {
                                if let Ok(_) = Task::insert(rb.get_ref(), &task_batch).await {
                                    daily_tasks.push(task_batch);
                                }
                            }
                        }
                    }
                }
                
                log::info!("成功生成 {} 個子任務", daily_tasks.len());
                
                // 設置父任務的初始完成率為 0
                if !daily_tasks.is_empty() {
                    let update_sql = "UPDATE task SET completion_rate = ? WHERE id = ?";
                    let _ = rb.exec(update_sql, vec![
                        rbs::Value::F64(0.0),
                        rbs::Value::String(task.id.clone().unwrap()),
                    ]).await;
                    
                    log::info!("已設置父任務完成率為 0%");
                }
            }
            
            // 返回成功響應
            Ok(HttpResponse::Ok().json(ApiResponse {
                success: true,
                data: Some(serde_json::json!({
                    "task": task,
                    "daily_tasks": daily_tasks,
                    "total_generated": daily_tasks.len()
                })),
                message: format!(
                    "任務成功儲存到資料庫{}",
                    if !daily_tasks.is_empty() {
                        format!("，並生成了 {} 個子任務", daily_tasks.len())
                    } else {
                        String::new()
                    }
                ),
            }))
        }
        Err(e) => {
            log::error!("儲存任務失敗: {}", e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("儲存任務失敗: {}", e),
            }))
        }
    }
}

// ============= 原本的組合式 API（保留以相容現有功能） =============

// 組合式 API：AI 生成任務並直接插入資料庫
pub async fn generate_task_with_ai(
    rb: web::Data<RBatis>,
    req: web::Json<GenerateTaskRequest>,
) -> Result<HttpResponse> {
    // 先生成 JSON
    let json_req = GenerateTaskJsonRequest {
        description: req.description.clone(),
    };
    
    // 從環境變數獲取 OpenAI API Key
    let api_key = match std::env::var("OPENAI_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            log::error!("未設置 OPENAI_API_KEY 環境變數");
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: "未配置 OpenAI API Key，請聯繫管理員".to_string(),
            }));
        }
    };
    
    let openai_service = OpenAIService::new(api_key);
    
    // 使用 AI 生成任務
    match openai_service.generate_task_from_text(&req.description).await {
        Ok(ai_task) => {
            // 轉換為 CreateTaskInput
            let task_input = CreateTaskInput {
                title: ai_task.title,
                description: ai_task.description,
                task_type: Some(ai_task.task_type),
                priority: Some(ai_task.priority),
                difficulty: Some(ai_task.difficulty),
                experience: Some(ai_task.experience),
                due_date: ai_task.due_date,
                is_recurring: Some(ai_task.is_recurring),
                recurrence_pattern: ai_task.recurrence_pattern,
                start_date: ai_task.start_date,
                end_date: ai_task.end_date,
                completion_target: ai_task.completion_target,
            };
            
            // 再插入資料庫
            let insert_req = InsertTaskRequest {
                task_json: task_input,
                user_id: req.user_id.clone(),
            };
            
            insert_task_from_json(rb, web::Json(insert_req)).await
        }
        Err(e) => {
            log::error!("AI 生成任務失敗: {}", e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("AI 生成任務失敗: {}", e),
            }))
        }
    }
}

// ============= 新增：任務驗證和預覽 API =============

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidateTaskRequest {
    pub task_json: CreateTaskInput,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskPreviewResponse {
    pub is_valid: bool,
    pub validation_errors: Vec<String>,
    pub task_preview: Option<String>,
    pub task_json: Option<CreateTaskInput>,
}

// 驗證任務 JSON 格式的函數
fn validate_task_json(task_input: &CreateTaskInput) -> (bool, Vec<String>) {
    let mut errors = Vec::new();
    
    // 驗證標題
    if task_input.title.trim().is_empty() {
        errors.push("任務標題不能為空".to_string());
    }
    
    // 驗證優先級
    if let Some(priority) = task_input.priority {
        if priority < 1 || priority > 5 {
            errors.push("優先級必須在 1-5 之間".to_string());
        }
    }
    
    // 驗證難度
    if let Some(difficulty) = task_input.difficulty {
        if difficulty < 1 || difficulty > 5 {
            errors.push("難度必須在 1-5 之間".to_string());
        }
    }
    
    // 驗證經驗值
    if let Some(experience) = task_input.experience {
        if experience < 0 {
            errors.push("經驗值不能為負數".to_string());
        }
    }
    
    // 驗證日期格式
    if let Some(due_date) = &task_input.due_date {
        if chrono::DateTime::parse_from_rfc3339(due_date).is_err() {
            errors.push("截止日期格式不正確".to_string());
        }
    }
    
    if let Some(start_date) = &task_input.start_date {
        if chrono::DateTime::parse_from_rfc3339(start_date).is_err() {
            errors.push("開始日期格式不正確".to_string());
        }
    }
    
    if let Some(end_date) = &task_input.end_date {
        if chrono::DateTime::parse_from_rfc3339(end_date).is_err() {
            errors.push("結束日期格式不正確".to_string());
        }
    }
    
    // 驗證重複模式
    if task_input.is_recurring.unwrap_or(false) {
        if let Some(pattern) = &task_input.recurrence_pattern {
            let valid_patterns = vec!["daily", "weekdays", "weekends", "weekly"];
            if !valid_patterns.contains(&pattern.as_str()) {
                errors.push(format!("無效的重複模式: {}。有效模式為: daily, weekdays, weekends, weekly", pattern));
            }
        } else {
            errors.push("重複性任務必須指定重複模式".to_string());
        }
    }
    
    // 驗證完成目標
    if let Some(target) = task_input.completion_target {
        if target < 0.0 || target > 100.0 {
            errors.push("完成目標必須在 0-100 之間".to_string());
        }
    }
    
    (errors.is_empty(), errors)
}

// API: 驗證並預覽任務
pub async fn validate_and_preview_task(
    req: web::Json<ValidateTaskRequest>,
) -> Result<HttpResponse> {
    let task_input = &req.task_json;
    
    // 驗證任務 JSON
    let (is_valid, validation_errors) = validate_task_json(task_input);
    
    // 如果驗證通過，生成任務預覽
    let task_preview = if is_valid {
        // 先生成簡單的預覽
        let mut simple_preview = format!("📋 任務名稱：{}\n", task_input.title);
        
        if let Some(desc) = &task_input.description {
            simple_preview.push_str(&format!("📝 描述：{}\n", desc));
        }
        
        simple_preview.push_str(&format!("🎯 類型：{}\n", task_input.task_type.as_deref().unwrap_or("一般任務")));
        simple_preview.push_str(&format!("⭐ 優先級：{}/5\n", task_input.priority.unwrap_or(3)));
        simple_preview.push_str(&format!("🔥 難度：{}/5\n", task_input.difficulty.unwrap_or(3)));
        simple_preview.push_str(&format!("💎 經驗值：{}\n", task_input.experience.unwrap_or(10)));
        
        if let Some(due_date) = &task_input.due_date {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(due_date) {
                simple_preview.push_str(&format!("📅 截止日期：{}\n", dt.format("%Y-%m-%d %H:%M")));
            }
        }
        
        if task_input.is_recurring.unwrap_or(false) {
            simple_preview.push_str(&format!("🔄 重複模式：{}\n", task_input.recurrence_pattern.as_deref().unwrap_or("無")));
            
            if let Some(start_date) = &task_input.start_date {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(start_date) {
                    simple_preview.push_str(&format!("🚀 開始日期：{}\n", dt.format("%Y-%m-%d")));
                }
            }
            
            if let Some(end_date) = &task_input.end_date {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(end_date) {
                    simple_preview.push_str(&format!("🏁 結束日期：{}\n", dt.format("%Y-%m-%d")));
                }
            }
        }
        
        // 如果有 OpenAI API Key，嘗試使用 AI 生成更豐富的預覽
        if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
            let openai_service = OpenAIService::new(api_key);
            
            // 建構提示詞
            let prompt = format!(
                "請為以下任務生成一個簡潔有趣的介紹（限制在100字以內）：\n\
                任務名稱：{}\n\
                描述：{}\n\
                類型：{}\n\
                優先級：{}/5\n\
                難度：{}/5\n\
                經驗值：{}\n\
                請用鼓勵和積極的語氣，讓用戶想要完成這個任務。",
                task_input.title,
                task_input.description.as_deref().unwrap_or("無"),
                task_input.task_type.as_deref().unwrap_or("一般任務"),
                task_input.priority.unwrap_or(3),
                task_input.difficulty.unwrap_or(3),
                task_input.experience.unwrap_or(10)
            );
            
            // 使用 AI 生成預覽
            match openai_service.generate_task_preview(&prompt).await {
                Ok(ai_preview) => Some(ai_preview),
                Err(_) => Some(simple_preview), // 如果 AI 生成失敗，使用簡單預覽
            }
        } else {
            Some(simple_preview)
        }
    } else {
        None
    };
    
    Ok(HttpResponse::Ok().json(ApiResponse {
        success: is_valid,
        data: Some(TaskPreviewResponse {
            is_valid,
            validation_errors,
            task_preview,
            task_json: if is_valid { Some(task_input.clone()) } else { None },
        }),
        message: if is_valid {
            "任務驗證成功".to_string()
        } else {
            "任務驗證失敗，請檢查錯誤".to_string()
        },
    }))
}

// API: 從聊天記錄生成任務 JSON
#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateTaskFromChatRequest {
    pub chat_history: Vec<String>,  // 最近的幾條聊天記錄
}

pub async fn generate_task_from_chat(
    req: web::Json<GenerateTaskFromChatRequest>,
) -> Result<HttpResponse> {
    // 從環境變數獲取 OpenAI API Key
    let api_key = match std::env::var("OPENAI_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: "未配置 OpenAI API Key".to_string(),
            }));
        }
    };
    
    let openai_service = OpenAIService::new(api_key);
    
    // 將聊天記錄組合成描述
    let description = req.chat_history.join("\n");
    
    // 使用 AI 生成任務
    match openai_service.generate_task_from_text(&description).await {
        Ok(ai_task) => {
            let task_json = CreateTaskInput {
                title: ai_task.title,
                description: ai_task.description,
                task_type: Some(ai_task.task_type),
                priority: Some(ai_task.priority),
                difficulty: Some(ai_task.difficulty),
                experience: Some(ai_task.experience),
                due_date: ai_task.due_date,
                is_recurring: Some(ai_task.is_recurring),
                recurrence_pattern: ai_task.recurrence_pattern,
                start_date: ai_task.start_date,
                end_date: ai_task.end_date,
                completion_target: ai_task.completion_target,
            };
            
            // 驗證生成的任務
            let (is_valid, validation_errors) = validate_task_json(&task_json);
            
            if is_valid {
                Ok(HttpResponse::Ok().json(ApiResponse {
                    success: true,
                    data: Some(task_json),
                    message: "成功從對話生成任務".to_string(),
                }))
            } else {
                Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: format!("生成的任務格式有誤: {}", validation_errors.join(", ")),
                }))
            }
        }
        Err(e) => {
            Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("生成任務失敗: {}", e),
            }))
        }
    }
}

// ============= 新增：簡化的任務創建 API =============

// API 3: 直接從 JSON 創建任務（用戶友好版本）
pub async fn create_task_from_json(
    rb: web::Data<RBatis>,
    req: web::Json<CreateTaskFromJsonRequest>,
) -> Result<HttpResponse> {
    // 將請求轉換為 CreateTaskInput 格式
    let task_input = CreateTaskInput {
        title: req.title.clone(),
        description: req.description.clone(),
        task_type: req.task_type.clone(),
        priority: req.priority,
        difficulty: req.difficulty,
        experience: req.experience,
        due_date: req.due_date.clone(),
        is_recurring: req.is_recurring,
        recurrence_pattern: req.recurrence_pattern.clone(),
        start_date: req.start_date.clone(),
        end_date: req.end_date.clone(),
        completion_target: req.completion_target,
    };
    
    // 再包裝為 InsertTaskRequest 格式
    let insert_req = InsertTaskRequest {
        task_json: task_input,
        user_id: req.user_id.clone(),
    };
    
    // 調用現有的插入邏輯
    insert_task_from_json(rb, web::Json(insert_req)).await
}