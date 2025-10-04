use actix_web::{web, HttpResponse, Result};
use rbatis::RBatis;
use uuid::Uuid;
use chrono::{Utc, Datelike};
use rbs::value;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::models::{Task, User, GenerateTaskRequest, TaskStatus, Achievement, UserAchievement};
use crate::ai_service::{OpenAIService, convert_to_achievement_model};
use crate::achievement_service::AchievementService;

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
                    password_hash: Some("".to_string()),
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
        career_mainline_id: None,
        task_category: None,
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
        career_mainline_id: None,
        task_category: None,
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

// ============= 自動成就生成功能 =============

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateAchievementFromTasksRequest {
    pub user_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskSummaryData {
    pub completed_tasks: Vec<TaskSummary>,
    pub pending_tasks: Vec<TaskSummary>,
    pub cancelled_tasks: Vec<TaskSummary>,
    pub existing_achievements: Vec<ExistingAchievementInfo>,
    pub achievement_statistics: AchievementStatistics,
    pub task_statistics: TaskStatistics,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExistingAchievementInfo {
    pub name: String,
    pub description: Option<String>,
    pub category: String,
    pub requirement_type: String,
    pub requirement_value: i32,
    pub experience_reward: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AchievementStatistics {
    pub total_achievements: i32,
    pub achievements_by_category: std::collections::HashMap<String, i32>,
    pub achievements_by_requirement_type: std::collections::HashMap<String, Vec<i32>>, // 條件類型 -> 數值列表
    pub covered_requirement_ranges: Vec<String>, // 已覆蓋的條件範圍描述
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskSummary {
    pub title: String,
    pub task_type: Option<String>,
    pub difficulty: Option<i32>,
    pub skill_tags: Option<Vec<String>>,
    pub cancel_count: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskStatistics {
    pub total_completed: i32,
    pub total_cancelled: i32,
    pub total_pending: i32,
    pub completion_rate: f64,
    pub most_common_task_type: Option<String>,
    pub average_difficulty: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeneratedAchievementResponse {
    pub achievement: Achievement,
    pub is_unlocked: bool,
    pub task_summary: TaskSummaryData,
}

// API: 從用戶任務數據自動生成成就
pub async fn generate_achievement_from_tasks(
    rb: web::Data<RBatis>,
    path: web::Path<String>, // user_id
) -> Result<HttpResponse> {
    let user_id = path.into_inner();
    
    // 基本參數驗證
    if user_id.trim().is_empty() {
        return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: "用戶ID不能為空".to_string(),
        }));
    }
    
    log::info!("開始為用戶 {} 生成成就", user_id);
    
    // 1. 收集用戶任務數據
    let task_data = match collect_user_task_data(rb.get_ref(), &user_id).await {
        Ok(data) => data,
        Err(e) => {
            log::error!("收集用戶任務數據失敗: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("收集任務數據失敗: {}", e),
            }));
        }
    };
    
    // 2. 檢查是否有足夠的數據生成成就
    if task_data.task_statistics.total_completed == 0 && task_data.task_statistics.total_cancelled == 0 {
        return Ok(HttpResponse::Ok().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: "✨ 還沒有足夠的任務數據來生成成就，先完成一些任務來展現你的實力吧！".to_string(),
        }));
    }
    
    // 3. 格式化數據為 AI 提示
    let ai_prompt = format_task_data_for_ai(&task_data);
    log::info!("生成的 AI 提示長度: {} 字符", ai_prompt.len());
    
    // 4. 調用 AI 生成成就
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
    
    let ai_achievement = match openai_service.generate_achievement_from_text(&ai_prompt).await {
        Ok(achievement) => achievement,
        Err(e) => {
            log::error!("AI 生成成就失敗: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("AI 生成成就失敗: {}", e),
            }));
        }
    };
    
    // 5. 相似性檢查
    if let Err(similarity_error) = check_achievement_similarity(&ai_achievement, &task_data.existing_achievements) {
        log::info!("成就相似性檢查未通過，建議用戶完成更多任務: {}", similarity_error);
        return Ok(HttpResponse::Ok().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: "🎯 目前沒有新成就可以生成，再完成一些不同類型的任務來解鎖更多成就吧！".to_string(),
        }));
    }
    
    log::info!("成就「{}」通過相似性檢查", ai_achievement.name);
    
    // 6. 轉換為數據庫模型並保存
    let achievement_model = convert_to_achievement_model(ai_achievement);
    
    match Achievement::insert(rb.get_ref(), &achievement_model).await {
        Ok(_) => {
            log::info!("成就 {} 已成功保存到數據庫", achievement_model.name.as_deref().unwrap_or("未知"));
            
            // 7. 檢查是否應該立即解鎖此成就
            let is_unlocked = match AchievementService::check_and_unlock_achievements(rb.get_ref(), &user_id).await {
                Ok(unlocked_achievements) => {
                    let achievement_id = achievement_model.id.as_ref().unwrap();
                    unlocked_achievements.iter().any(|a| a.id.as_ref() == Some(achievement_id))
                }
                Err(e) => {
                    log::warn!("檢查成就解鎖狀態失敗: {}", e);
                    false
                }
            };
            
            // 8. 返回成功響應
            let achievement_name = achievement_model.name.as_deref().unwrap_or("未知").to_string();
            Ok(HttpResponse::Ok().json(ApiResponse {
                success: true,
                data: Some(GeneratedAchievementResponse {
                    achievement: achievement_model,
                    is_unlocked,
                    task_summary: task_data,
                }),
                message: format!(
                    "成功生成成就「{}」{}",
                    achievement_name,
                    if is_unlocked { "，並已解鎖" } else { "" }
                ),
            }))
        }
        Err(e) => {
            log::error!("保存成就到數據庫失敗: {}", e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("保存成就失敗: {}", e),
            }))
        }
    }
}

// 收集用戶任務數據
async fn collect_user_task_data(rb: &RBatis, user_id: &str) -> Result<TaskSummaryData, anyhow::Error> {
    // 查詢已完成的任務
    let completed_tasks: Vec<Task> = Task::select_by_map(rb, value!{
        "user_id": user_id,
        "status": TaskStatus::Completed.to_i32()
    }).await?;
    
    // 查詢待完成和進行中的任務
    let pending_sql = "SELECT * FROM task WHERE user_id = ? AND status IN (?, ?, ?) LIMIT 20";
    let pending_tasks: Vec<Task> = rb.query_decode(pending_sql, vec![
        user_id.into(),
        TaskStatus::Pending.to_i32().into(),
        TaskStatus::InProgress.to_i32().into(),
        TaskStatus::Paused.to_i32().into(),
    ]).await?;
    
    // 查詢已取消的任務
    let cancelled_tasks: Vec<Task> = Task::select_by_map(rb, value!{
        "user_id": user_id,
        "status": TaskStatus::Cancelled.to_i32()
    }).await?;
    
    // 查詢現有成就並進行詳細分析
    let existing_achievements: Vec<Achievement> = Achievement::select_all(rb).await?;
    
    // 轉換為詳細成就信息
    let achievement_infos: Vec<ExistingAchievementInfo> = existing_achievements
        .iter()
        .map(|a| ExistingAchievementInfo {
            name: a.name.clone().unwrap_or_default(),
            description: a.description.clone(),
            category: a.category.clone().unwrap_or_default(),
            requirement_type: a.requirement_type.as_ref()
                .map(|rt| rt.to_string().to_string())
                .unwrap_or_default(),
            requirement_value: a.requirement_value.unwrap_or(0),
            experience_reward: a.experience_reward.unwrap_or(0),
        })
        .collect();
    
    // 計算成就統計信息
    let mut achievements_by_category = std::collections::HashMap::new();
    let mut achievements_by_requirement_type = std::collections::HashMap::new();
    
    for achievement in &achievement_infos {
        // 按分類統計
        *achievements_by_category
            .entry(achievement.category.clone())
            .or_insert(0) += 1;
        
        // 按條件類型統計
        achievements_by_requirement_type
            .entry(achievement.requirement_type.clone())
            .or_insert_with(Vec::new)
            .push(achievement.requirement_value);
    }
    
    // 生成已覆蓋條件範圍描述
    let mut covered_ranges = Vec::new();
    for (req_type, values) in &achievements_by_requirement_type {
        if !values.is_empty() {
            let min_val = *values.iter().min().unwrap();
            let max_val = *values.iter().max().unwrap();
            if min_val == max_val {
                covered_ranges.push(format!("{}: {}", req_type, min_val));
            } else {
                covered_ranges.push(format!("{}: {}-{}", req_type, min_val, max_val));
            }
        }
    }
    
    let achievement_statistics = AchievementStatistics {
        total_achievements: achievement_infos.len() as i32,
        achievements_by_category,
        achievements_by_requirement_type,
        covered_requirement_ranges: covered_ranges,
    };
    
    // 計算統計數據
    let total_completed = completed_tasks.len() as i32;
    let total_cancelled = cancelled_tasks.len() as i32;
    let total_pending = pending_tasks.len() as i32;
    let total_tasks = total_completed + total_cancelled + total_pending;
    
    let completion_rate = if total_tasks > 0 {
        total_completed as f64 / total_tasks as f64
    } else {
        0.0
    };
    
    // 找出最常見的任務類型
    let mut task_type_counts = std::collections::HashMap::new();
    for task in &completed_tasks {
        if let Some(task_type) = &task.task_type {
            *task_type_counts.entry(task_type.clone()).or_insert(0) += 1;
        }
    }
    
    let most_common_task_type = task_type_counts
        .iter()
        .max_by_key(|(_, &count)| count)
        .map(|(task_type, _)| task_type.clone());
    
    // 計算平均難度
    let difficulties: Vec<i32> = completed_tasks
        .iter()
        .filter_map(|t| t.difficulty)
        .collect();
    
    let average_difficulty = if !difficulties.is_empty() {
        difficulties.iter().sum::<i32>() as f64 / difficulties.len() as f64
    } else {
        0.0
    };
    
    // 轉換為摘要格式
    let completed_summaries: Vec<TaskSummary> = completed_tasks
        .into_iter()
        .take(10) // 限制數量避免提示過長
        .map(|t| TaskSummary {
            title: t.title.unwrap_or_default(),
            task_type: t.task_type,
            difficulty: t.difficulty,
            skill_tags: t.skill_tags,
            cancel_count: t.cancel_count,
        })
        .collect();
    
    let pending_summaries: Vec<TaskSummary> = pending_tasks
        .into_iter()
        .take(5)
        .map(|t| TaskSummary {
            title: t.title.unwrap_or_default(),
            task_type: t.task_type,
            difficulty: t.difficulty,
            skill_tags: t.skill_tags,
            cancel_count: t.cancel_count,
        })
        .collect();
    
    let cancelled_summaries: Vec<TaskSummary> = cancelled_tasks
        .into_iter()
        .take(5)
        .map(|t| TaskSummary {
            title: t.title.unwrap_or_default(),
            task_type: t.task_type,
            difficulty: t.difficulty,
            skill_tags: t.skill_tags,
            cancel_count: t.cancel_count,
        })
        .collect();
    
    Ok(TaskSummaryData {
        completed_tasks: completed_summaries,
        pending_tasks: pending_summaries,
        cancelled_tasks: cancelled_summaries,
        existing_achievements: achievement_infos,
        achievement_statistics,
        task_statistics: TaskStatistics {
            total_completed,
            total_cancelled,
            total_pending,
            completion_rate,
            most_common_task_type,
            average_difficulty,
        },
    })
}

// 格式化任務數據為 AI 提示
fn format_task_data_for_ai(data: &TaskSummaryData) -> String {
    let mut prompt = String::new();
    
    // =========== 用戶任務行為分析 ===========
    prompt.push_str(&format!(
        "**用戶任務完成情況分析：**\n\
        - 總完成任務：{} 個（完成率 {:.1}%）\n\
        - 待完成任務：{} 個\n\
        - 已取消任務：{} 個\n",
        data.task_statistics.total_completed,
        data.task_statistics.completion_rate * 100.0,
        data.task_statistics.total_pending,
        data.task_statistics.total_cancelled
    ));
    
    if let Some(common_type) = &data.task_statistics.most_common_task_type {
        prompt.push_str(&format!("- 最擅長的任務類型：{}\n", common_type));
    }
    
    if data.task_statistics.average_difficulty > 0.0 {
        prompt.push_str(&format!("- 平均挑戰難度：{:.1}/5\n", data.task_statistics.average_difficulty));
    }
    
    // 分析用戶行為特點
    prompt.push_str("\n**用戶行為特點分析：**\n");
    if data.task_statistics.completion_rate >= 0.8 {
        prompt.push_str("- 高完成率用戶，執行力強，適合挑戰型成就\n");
    } else if data.task_statistics.completion_rate >= 0.6 {
        prompt.push_str("- 中等完成率用戶，需要鼓勵型成就\n");
    } else {
        prompt.push_str("- 需要激勵的用戶，建議生成容易達成的基礎成就\n");
    }
    
    if data.task_statistics.average_difficulty >= 4.0 {
        prompt.push_str("- 喜歡挑戰高難度任務，適合精通類成就\n");
    }
    
    // =========== 現有成就詳細分析 ===========
    prompt.push_str(&format!("\n**現有成就系統分析（共 {} 個成就）：**\n", data.achievement_statistics.total_achievements));
    
    // 按分類展示現有成就
    if !data.achievement_statistics.achievements_by_category.is_empty() {
        prompt.push_str("已覆蓋的成就分類：\n");
        for (category, count) in &data.achievement_statistics.achievements_by_category {
            prompt.push_str(&format!("  - {}: {} 個成就\n", category, count));
        }
    }
    
    // 已覆蓋的條件類型和範圍
    if !data.achievement_statistics.covered_requirement_ranges.is_empty() {
        prompt.push_str("\n已覆蓋的達成條件範圍：\n");
        for range in &data.achievement_statistics.covered_requirement_ranges {
            prompt.push_str(&format!("  - {}\n", range));
        }
    }
    
    // 詳細現有成就列表
    if !data.existing_achievements.is_empty() {
        prompt.push_str(&format!("\n現有成就詳細清單（前 {} 個）：\n", std::cmp::min(15, data.existing_achievements.len())));
        for (i, achievement) in data.existing_achievements.iter().enumerate().take(15) {
            prompt.push_str(&format!(
                "{}. 「{}」- {} (條件: {} {}, 獎勵: {} XP)\n",
                i + 1,
                achievement.name,
                achievement.description.as_deref().unwrap_or("無描述"),
                achievement.requirement_type,
                achievement.requirement_value,
                achievement.experience_reward
            ));
        }
    }
    
    // =========== 任務行為樣本 ===========
    if !data.completed_tasks.is_empty() {
        prompt.push_str(&format!("\n**用戶優勢表現（已完成任務樣本）：**\n"));
        for (i, task) in data.completed_tasks.iter().enumerate().take(6) {
            prompt.push_str(&format!(
                "{}. 「{}」({}, 難度 {})\n",
                i + 1,
                task.title,
                task.task_type.as_deref().unwrap_or("未知類型"),
                task.difficulty.unwrap_or(0)
            ));
        }
    }
    
    if data.task_statistics.total_cancelled > 0 && !data.cancelled_tasks.is_empty() {
        prompt.push_str(&format!("\n**需要改進的領域（經常取消的任務）：**\n"));
        for (i, task) in data.cancelled_tasks.iter().enumerate().take(3) {
            prompt.push_str(&format!(
                "{}. 「{}」(取消 {} 次)\n",
                i + 1,
                task.title,
                task.cancel_count.unwrap_or(0)
            ));
        }
    }
    
    // =========== AI 生成要求 ===========
    prompt.push_str(&format!(
        "\n**重要生成要求：**\n\
        \n🚫 **嚴格避免重複：**\n\
        1. 絕對不要生成與現有成就名稱相似的成就\n\
        2. 避免使用已覆蓋的達成條件類型和數值範圍\n\
        3. 不要重複現有成就的核心概念或主題\n\
        \n✨ **創新生成策略：**\n\
        1. 重點關注用戶的**獨特行為模式**和**未被表彰的優勢**\n\
        2. 基於用戶的任務完成數據，找出**尚未被成就覆蓋**的亮點\n\
        3. 優先選擇**空白的條件類型**或**不同的數值範圍**\n\
        4. 成就名稱要有創意、個性化，避免通用化描述\n\
        \n🎯 **生成重點：**\n\
        - 如果某個條件類型已存在，請選擇明顯不同的數值範圍或完全不同的條件類型\n\
        - 重點表彰用戶在任務數據中表現出的獨特特質\n\
        - 成就名稱要幽默、生動，體現用戶的個性化成就\n\
        - 避免生成過於相似的成就分類\n\
        \n請基於以上分析，生成一個**完全創新**且**高度個性化**的成就！"
    ));
    
    prompt
}

// 檢查成就相似性
fn check_achievement_similarity(
    new_achievement: &crate::ai_service::AIGeneratedAchievement,
    existing_achievements: &[ExistingAchievementInfo]
) -> Result<(), String> {
    let new_name_lower = new_achievement.name.to_lowercase();
    
    for existing in existing_achievements {
        let existing_name_lower = existing.name.to_lowercase();
        
        // 1. 檢查名稱相似性
        if names_are_similar(&new_name_lower, &existing_name_lower) {
            return Err(format!(
                "成就名稱過於相似：新成就「{}」與現有成就「{}」名稱相似", 
                new_achievement.name, 
                existing.name
            ));
        }
        
        // 2. 檢查條件類型和數值的重複性
        if new_achievement.requirement_type == existing.requirement_type {
            let value_diff = (new_achievement.requirement_value - existing.requirement_value).abs();
            let similarity_threshold = calculate_value_similarity_threshold(&new_achievement.requirement_type, existing.requirement_value);
            
            if value_diff <= similarity_threshold {
                return Err(format!(
                    "達成條件過於相似：新成就條件「{} {}」與現有成就「{}」的條件「{} {}」過於接近",
                    new_achievement.requirement_type,
                    new_achievement.requirement_value,
                    existing.name,
                    existing.requirement_type,
                    existing.requirement_value
                ));
            }
        }
        
        // 3. 檢查描述相似性（如果有）
        if let Some(new_desc) = &new_achievement.description {
            if let Some(existing_desc) = &existing.description {
                if descriptions_are_similar(new_desc, existing_desc) {
                    return Err(format!(
                        "成就描述過於相似：與現有成就「{}」的描述相似", 
                        existing.name
                    ));
                }
            }
        }
    }
    
    Ok(())
}

// 檢查兩個名稱是否相似
fn names_are_similar(name1: &str, name2: &str) -> bool {
    // 如果名稱完全相同
    if name1 == name2 {
        return true;
    }
    
    // 檢查關鍵詞重疊
    let words1: std::collections::HashSet<&str> = name1.split_whitespace().collect();
    let words2: std::collections::HashSet<&str> = name2.split_whitespace().collect();
    
    let intersection: std::collections::HashSet<_> = words1.intersection(&words2).collect();
    let union: std::collections::HashSet<_> = words1.union(&words2).collect();
    
    // 如果重疊度超過70%則認為相似
    let similarity = intersection.len() as f64 / union.len() as f64;
    similarity > 0.7
}

// 檢查兩個描述是否相似
fn descriptions_are_similar(desc1: &str, desc2: &str) -> bool {
    let desc1_lower = desc1.to_lowercase();
    let desc2_lower = desc2.to_lowercase();
    
    // 簡單的相似度檢查：如果有大量重複字符
    let common_chars = desc1_lower.chars()
        .filter(|c| desc2_lower.contains(*c))
        .count();
    
    let max_len = std::cmp::max(desc1_lower.len(), desc2_lower.len());
    let similarity = common_chars as f64 / max_len as f64;
    
    similarity > 0.8
}

// 計算數值相似性閾值
fn calculate_value_similarity_threshold(requirement_type: &str, existing_value: i32) -> i32 {
    match requirement_type {
        "task_complete" | "total_completions" => {
            // 任務完成數：根據現有數值的20%或最少3個任務的差距
            std::cmp::max(3, existing_value / 5)
        },
        "consecutive_days" => {
            // 連續天數：至少7天的差距
            std::cmp::max(7, existing_value / 4)
        },
        "skill_level" => {
            // 技能等級：至少1級差距
            1
        },
        "learning_task_complete" => {
            // 學習任務：至少2個任務差距
            std::cmp::max(2, existing_value / 3)
        },
        // 屬性相關成就：至少10點差距
        "intelligence_attribute" | "endurance_attribute" | "creativity_attribute" |
        "social_attribute" | "focus_attribute" | "adaptability_attribute" => {
            std::cmp::max(10, existing_value / 5)
        },
        _ => existing_value / 4, // 默認25%差距
    }
}