use actix_web::{web, HttpResponse, Result};
use rbatis::RBatis;
use uuid::Uuid;
use chrono::{Utc, Datelike};
use rbs::value;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

use crate::models::{Task, User, GenerateTaskRequest, TaskStatus, Achievement, UserAchievement};
use crate::career_routes::parse_ai_tasks_response;
use crate::ai_service::{convert_to_achievement_model, AIGeneratedTaskPlan};
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
    // 載入配置
    let config = crate::config::Config::from_env();
    
    // 創建 AI 服務
    let ai_service = match crate::ai_service::create_ai_service(&config.app.ai) {
        Ok(service) => service,
        Err(e) => {
            log::error!("AI 服務初始化失敗: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("AI 服務初始化失敗: {}", e),
            }));
        }
    };
    
    // 使用 AI 生成任務 JSON
    match ai_service.generate_task_from_text(&req.description).await {
        Ok(ai_task) => {
            log::info!("AI 成功生成任務 JSON: {:?}", ai_task);
            
            // 將 AI 生成的任務轉換為符合 schema 的 JSON
            let task_json = CreateTaskInput {
                title: ai_task.title.unwrap_or_else(|| "未命名任務".to_string()),
                description: ai_task.description,
                task_type: ai_task.task_type,
                priority: ai_task.priority,
                difficulty: ai_task.difficulty,
                experience: ai_task.experience,
                due_date: ai_task.due_date,
                is_recurring: ai_task.is_recurring,
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

// API: 專門生成每日任務 JSON（使用針對每日任務優化的提示詞）
pub async fn generate_daily_task_json(
    req: web::Json<GenerateTaskJsonRequest>,
) -> Result<HttpResponse> {
    // 載入配置
    let config = crate::config::Config::from_env();

    // 創建 AI 服務
    let ai_service = match crate::ai_service::create_ai_service(&config.app.ai) {
        Ok(service) => service,
        Err(e) => {
            log::error!("AI 服務初始化失敗: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("AI 服務初始化失敗: {}", e),
            }));
        }
    };

    // 使用 AI 生成每日任務 JSON（專門的每日任務方法）
    match ai_service.generate_daily_task_from_text(&req.description).await {
        Ok(ai_task) => {
            log::info!("AI 成功生成每日任務 JSON: {:?}", ai_task);

            // 將 AI 生成的任務轉換為符合 schema 的 JSON
            let task_json = CreateTaskInput {
                title: ai_task.title.unwrap_or_else(|| "未命名每日任務".to_string()),
                description: ai_task.description,
                task_type: Some("daily".to_string()), // 強制設定為 daily
                priority: ai_task.priority,
                difficulty: ai_task.difficulty,
                experience: ai_task.experience,
                due_date: None, // 每日任務不設定截止日期
                is_recurring: Some(false),
                recurrence_pattern: None,
                start_date: None,
                end_date: None,
                completion_target: None,
            };

            Ok(HttpResponse::Ok().json(ApiResponse {
                success: true,
                data: Some(serde_json::json!({ "task_json": task_json })),
                message: "AI 成功生成每日任務 JSON".to_string(),
            }))
        }
        Err(e) => {
            log::error!("AI 生成每日任務 JSON 失敗: {}", e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("AI 生成每日任務 JSON 失敗: {}", e),
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
    
    // 決定使用者 ID（過濾空字串）
    let user_id = if let Some(id) = req.user_id.clone().filter(|s| !s.trim().is_empty()) {
        // 驗證用戶是否存在
        match User::select_by_map(rb.get_ref(), value!{"id": id.clone()}).await {
            Ok(users) if !users.is_empty() => id,
            _ => {
                log::warn!("提供的用戶ID不存在: {}，使用預設用戶", id);
                // 查詢或建立預設測試用戶
                match User::select_by_map(rb.get_ref(), value!{"email": "test@lifeup.com"}).await {
                    Ok(users) if !users.is_empty() => users[0].id.clone().unwrap(),
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
            }
        }
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
        is_parent_task: if task_input.task_type == Some("main".to_string()) || task_input.is_recurring.unwrap_or(false) {
            Some(1)
        } else {
            Some(0)
        },
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
        attributes: None,
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
                            attributes: None,
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
    
    // 載入配置
    let config = crate::config::Config::from_env();
    
    // 創建 AI 服務
    let ai_service = match crate::ai_service::create_ai_service(&config.app.ai) {
        Ok(service) => service,
        Err(e) => {
            log::error!("AI 服務初始化失敗: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("AI 服務初始化失敗: {}", e),
            }));
        }
    };
    
    // 使用 AI 生成任務
    match ai_service.generate_task_from_text(&req.description).await {
        Ok(ai_task) => {
            // 轉換為 CreateTaskInput
            let task_input = CreateTaskInput {
                title: ai_task.title.unwrap_or_else(|| "未命名任務".to_string()),
                description: ai_task.description,
                task_type: ai_task.task_type,
                priority: ai_task.priority,
                difficulty: ai_task.difficulty,
                experience: ai_task.experience,
                due_date: ai_task.due_date,
                is_recurring: ai_task.is_recurring,
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
// 輔助函數：驗證日期字串是否可解析（支持多種格式）
fn is_valid_date_string(date_str: &str) -> bool {
    // 嘗試 RFC3339 格式（帶時區）
    if chrono::DateTime::parse_from_rfc3339(date_str).is_ok() {
        return true;
    }

    // 嘗試 ISO 8601 格式（不帶時區，假設 UTC）
    if date_str.parse::<chrono::NaiveDateTime>().is_ok() {
        return true;
    }

    // 嘗試只有日期的格式
    if date_str.parse::<chrono::NaiveDate>().is_ok() {
        return true;
    }

    false
}

fn validate_task_json(task_input: &CreateTaskInput) -> (bool, Vec<String>) {
    let mut errors = Vec::new();

    // 驗證標題
    if task_input.title.trim().is_empty() {
        errors.push("任務標題不能為空".to_string());
    }

    // 驗證優先級
    if let Some(priority) = task_input.priority {
        if priority < 0 || priority > 2 {
            errors.push("優先級必須在 0-2 之間".to_string());
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

    // 驗證日期格式（寬容處理多種格式）
    if let Some(due_date) = &task_input.due_date {
        if !is_valid_date_string(due_date) {
            errors.push("截止日期格式不正確".to_string());
        }
    }

    if let Some(start_date) = &task_input.start_date {
        if !is_valid_date_string(start_date) {
            errors.push("開始日期格式不正確".to_string());
        }
    }

    if let Some(end_date) = &task_input.end_date {
        if !is_valid_date_string(end_date) {
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
        if target < 0.0 || target > 1.0 {
            errors.push("完成目標必須在 0.0-1.0 之間".to_string());
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
    
    // 如果驗證通過，生成任務預覽（使用 Markdown 格式）
    let task_preview = if is_valid {
        // 生成 Markdown 格式的預覽
        let mut simple_preview = format!("## 📋 {}", task_input.title);
        Some(simple_preview)
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
    // 載入配置
    let config = crate::config::Config::from_env();
    
    // 創建 AI 服務
    let ai_service = match crate::ai_service::create_ai_service(&config.app.ai) {
        Ok(service) => service,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("AI 服務初始化失敗: {}", e),
            }));
        }
    };
    
    // 將聊天記錄組合成描述
    let description = req.chat_history.join("\n");
    
    // 使用 AI 生成任務
    match ai_service.generate_task_from_text(&description).await {
        Ok(ai_task) => {
            let task_json = CreateTaskInput {
                title: ai_task.title.unwrap_or_else(|| "未命名任務".to_string()),
                description: ai_task.description,
                task_type: ai_task.task_type,
                priority: ai_task.priority,
                difficulty: ai_task.difficulty,
                experience: ai_task.experience,
                due_date: ai_task.due_date,
                is_recurring: ai_task.is_recurring,
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
    let config = crate::config::Config::from_env();
    let ai_service = match crate::ai_service::create_ai_service(&config.app.ai) {
        Ok(service) => service,
        Err(e) => {
            log::error!("AI 服務初始化失敗: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("AI 服務初始化失敗: {}", e),
            }));
        }
    };
    
    let ai_achievement = match ai_service.generate_achievement_from_text(&ai_prompt).await {
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

// ============= 專家系統 API =============

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateTaskWithExpertRequest {
    pub description: String,
    pub prompt_description: Option<String>,
    pub user_id: Option<String>,
    pub expert_name: String,
    pub expert_description: String,
    pub expert_match: Option<crate::ai_service::ExpertMatch>,
    pub selected_options: Option<Vec<String>>,
    pub selected_directions: Option<Vec<AnalysisDirection>>,
    pub expert_outputs: Option<HashMap<String, String>>,
    pub skill_level_label: Option<String>,
    pub learning_duration_label: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExpertTaskResponse {
    pub expert_match: crate::ai_service::ExpertMatch,
    pub task_json: CreateTaskInput,
    pub task_plan: crate::ai_service::AIGeneratedTaskPlan,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MatchExpertRequest {
    pub description: String,
    pub user_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExpertMatchResponse {
    pub expert_match: crate::ai_service::ExpertMatch,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExpertAnalysisRequest {
    pub description: String,
    pub expert_name: String,
    pub expert_description: String,
    pub analysis_type: String, // "analyze", "goals", "resources"
    pub user_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExpertAnalysisResponse {
    pub analysis_result: String,
    pub directions: Option<Vec<AnalysisDirection>>,
    pub goals: Option<Vec<AnalysisGoal>>,
    pub resources: Option<Vec<AnalysisResource>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnalysisGoal {
    pub title: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnalysisDirection {
    pub title: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnalysisResource {
    pub title: String,
    pub description: String,
}

// API: 使用專家系統生成任務
pub async fn generate_task_with_expert(
    rb: web::Data<RBatis>,
    req: web::Json<GenerateTaskWithExpertRequest>,
) -> Result<HttpResponse> {
    let prompt_description = req.prompt_description.clone().unwrap_or_else(|| req.description.clone());
    let skill_label = req.skill_level_label.clone().unwrap_or_else(|| "".to_string());
    let duration_label = req.learning_duration_label.clone().unwrap_or_else(|| "".to_string());
    log::info!(
        "[generate_task_with_expert] 收到請求: user_id={:?}, description_length={}, prompt_length={}, options={:?}, directions={:?}",
        req.user_id,
        req.description.len(),
        prompt_description.len(),
        req.selected_options.as_ref().map(|o| o.join(",")),
        req.selected_directions.as_ref().map(|d| d.iter().map(|item| item.title.clone()).collect::<Vec<_>>())
    );

    // 載入配置
    let config = crate::config::Config::from_env();
    
    // 創建 AI 服務
    let ai_service = match crate::ai_service::create_ai_service(&config.app.ai) {
        Ok(service) => service,
        Err(e) => {
            log::error!("AI 服務初始化失敗: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("AI 服務初始化失敗: {}", e),
            }));
        }
    };
    
    let expert_match = if let Some(existing_match) = req.expert_match.clone() {
        log::info!(
            "[generate_task_with_expert] 使用前端提供的專家: {}",
            existing_match.expert.name
        );
        existing_match
    } else {
        log::info!("[generate_task_with_expert] 前端未提供專家，使用 expert_name/description 重建虛擬專家");
        return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("前端未提供專家，使用 expert_name/description 重建虛擬專家失敗"),
        }));
    };
    
    let ai_input_prompt = crate::ai_service::build_task_generation_prompt(
        &prompt_description,
        &expert_match,
        req.selected_options.clone(),
        req.selected_directions.clone(),
        req.expert_outputs.clone(),
        &skill_label,
        &duration_label
    );
    log::info!("[generate_task_with_expert] 構建的提示詞長度: {}", ai_input_prompt.len());
    
    // 第二步：使用專家生成任務計劃
    log::info!(
        "[generate_task_with_expert] 使用專家 {} 生成任務計劃",
        expert_match.expert.name
    );
    log::info!("[generate_task_with_expert] 送往 AI 描述長度: {}", ai_input_prompt.len());

    let ai_task_plan = match ai_service.generate_task_with_expert(&ai_input_prompt, &expert_match).await {
        Ok(task_plan) => {
            log::info!("專家成功生成任務計劃: {:?} (包含 {} 個子任務)",
                      task_plan.main_task.title, task_plan.subtasks.len());
            task_plan
        }
        Err(e) => {
            log::error!("專家生成任務計劃失敗: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("專家生成任務計劃失敗: {}", e),
            }));
        }
    };

    // 整合選中的強化選項和分析結果到主任務描述中
    let mut enhanced_description = ai_task_plan.main_task.description.clone().unwrap_or_default();

    // 如果有選中的選項，添加到描述中
    if let Some(selected_options) = &req.selected_options {
        if !selected_options.is_empty() {
            let option_labels: Vec<String> = selected_options.iter()
                .map(|opt| match opt.as_str() {
                    "analyze" => "分析加強方向",
                    "goals" => "生成明確目標",
                    "resources" => "建議學習資源",
                    _ => opt
                })
                .map(|s| s.to_string())
                .collect();

            enhanced_description.push_str(&format!("\n\n【小教練重點加強】\n{}", option_labels.join("、")));
        }
    }

    // 檢查是否有分析結果
    let has_analyze_output = req.expert_outputs.as_ref()
        .and_then(|outputs| outputs.get("analyze"))
        .map(|v| !v.is_empty())
        .unwrap_or(false);

    // 如果有選中的加強方向，且沒有分析結果（避免重複）
    if let Some(directions) = &req.selected_directions {
        if !directions.is_empty() && !has_analyze_output {
            enhanced_description.push_str("\n\n【選定的加強方向】");
            for (i, direction) in directions.iter().enumerate() {
                enhanced_description.push_str(&format!("\n{}. {} - {}", i + 1, direction.title, direction.description));
            }
        }
    }

    // 如果有專家輸出，添加摘要到描述中
    if let Some(outputs) = &req.expert_outputs {
        for (key, value) in outputs.iter() {
            if !value.is_empty() {
                let label = match key.as_str() {
                    "analyze" => "分析結果",
                    "goals" => "明確目標",
                    "resources" => "學習資源",
                    _ => key
                };

                // 對於分析結果，如果已經包含了選定方向的內容，就包含完整內容
                // 否則只添加簡短摘要
                let summary = if key == "analyze" {
                    // 分析結果通常已包含選定方向，顯示完整內容
                    value.clone()
                } else if value.len() > 200 {
                    // 使用字符邊界安全的方式截取字串
                    let truncated = value.chars().take(200).collect::<String>();
                    format!("{}...", truncated)
                } else {
                    value.clone()
                };

                enhanced_description.push_str(&format!("\n\n【{}】\n{}", label, summary));
            }
        }
    }

    // 更新主任務的描述
    let mut updated_main_task = ai_task_plan.main_task.clone();
    updated_main_task.description = Some(enhanced_description);

    log::info!(
        "[generate_task_with_expert] 任務計劃已生成（不插入資料庫），包含 {} 個子任務計劃",
        ai_task_plan.subtasks.len()
    );

    // 轉換為 CreateTaskInput 格式（主任務）- 使用增強後的描述
    let task_json = CreateTaskInput {
        title: updated_main_task.title.clone().unwrap_or_else(|| "未命名任務".to_string()),
        description: updated_main_task.description.clone(),
        task_type: updated_main_task.task_type.clone(),
        priority: updated_main_task.priority,
        difficulty: updated_main_task.difficulty,
        experience: updated_main_task.experience,
        due_date: updated_main_task.due_date.clone(),
        is_recurring: updated_main_task.is_recurring,
        recurrence_pattern: updated_main_task.recurrence_pattern.clone(),
        start_date: updated_main_task.start_date.clone(),
        end_date: updated_main_task.end_date.clone(),
        completion_target: updated_main_task.completion_target,
    };

    let response = ExpertTaskResponse {
        expert_match,
        task_json,
        task_plan: ai_task_plan.clone(),
    };

    log::info!(
        "[generate_task_with_expert] 任務計劃生成完成，主任務: {:?}，子任務計劃數: {}",
        updated_main_task.title,
        ai_task_plan.subtasks.len()
    );

    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        data: Some(response),
        message: format!("任務計劃生成成功，包含主任務和 {} 個子任務計劃（尚未創建）", ai_task_plan.subtasks.len()),
    }))
}

// API: 只匹配專家（不生成任務）
pub async fn match_expert_only(
    req: web::Json<MatchExpertRequest>,
) -> Result<HttpResponse> {
    // 載入配置
    let config = crate::config::Config::from_env();
    
    // 創建 AI 服務
    let ai_service = match crate::ai_service::create_ai_service(&config.app.ai) {
        Ok(service) => service,
        Err(e) => {
            log::error!("AI 服務初始化失敗: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("AI 服務初始化失敗: {}", e),
            }));
        }
    };
    
    // 只進行專家匹配
    log::info!("開始為任務描述匹配專家: {}", req.description);
    let expert_match = match ai_service.match_expert_for_task(&req.description).await {
        Ok(match_result) => {
            log::info!("成功匹配專家: {}",
                match_result.expert.name);
            match_result
        }
        Err(e) => {
            log::error!("專家匹配失敗: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("專家匹配失敗: {}", e),
            }));
        }
    };
    
    let response = ExpertMatchResponse {
        expert_match,
    };
    
    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        data: Some(response),
        message: "專家匹配成功".to_string(),
    }))
}

// API: 為已存在的任務生成子任務
#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateSubtasksRequest {
    pub parent_task_id: String,
    pub task_description: String,
    pub task_plan: Option<AIGeneratedTaskPlan>, // 可選的任務計劃，如果前端已經有了就直接使用
    pub user_id: Option<String>,
    pub expert_match: Option<crate::ai_service::ExpertMatch>, // 專家信息，用於生成一致的子任務
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateSubtasksResponse {
    pub subtasks_created: Vec<Task>,
    pub total_count: usize,
}

pub async fn generate_subtasks_for_task(
    rb: web::Data<RBatis>,
    req: web::Json<GenerateSubtasksRequest>,
) -> Result<HttpResponse> {
    log::info!(
        "[generate_subtasks_for_task] 開始為任務 {} 生成子任務",
        req.parent_task_id
    );

    // 驗證父任務是否存在
    let parent_tasks: Vec<Task> = match Task::select_by_map(rb.get_ref(), value!{
        "id": &req.parent_task_id
    }).await {
        Ok(tasks) => tasks,
        Err(e) => {
            log::error!("查詢父任務失敗: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("查詢父任務失敗: {}", e),
            }));
        }
    };

    if parent_tasks.is_empty() {
        return Ok(HttpResponse::NotFound().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: "找不到指定的父任務".to_string(),
        }));
    }

    let parent_task = &parent_tasks[0];
    let user_id = parent_task.user_id.clone().unwrap_or_else(|| "default_user".to_string());

    // 如果前端提供了任務計劃且有子任務，就直接使用
    let subtasks_to_create = if let Some(task_plan) = &req.task_plan {
        if !task_plan.subtasks.is_empty() {
            log::info!("[generate_subtasks_for_task] 使用前端提供的任務計劃，包含 {} 個子任務", task_plan.subtasks.len());
            task_plan.subtasks.clone()
        } else {
            log::info!("[generate_subtasks_for_task] 任務計劃中沒有子任務，需要使用 AI 生成");
            // 繼續執行 AI 生成邏輯（見下方）
            Vec::new() // 暫時返回空，下面會處理
        }
    } else {
        log::info!("[generate_subtasks_for_task] 沒有任務計劃，需要使用 AI 生成");
        Vec::new() // 暫時返回空，下面會處理
    };

    // 如果還沒有子任務，使用 AI 生成（異步處理）
    if subtasks_to_create.is_empty() {
        log::info!("[generate_subtasks_for_task] 開始異步生成子任務");

        // 複製必要的數據用於異步任務
        let parent_task_id_clone = req.parent_task_id.clone();
        let task_description_clone = req.task_description.clone();
        let parent_task_title = parent_task.title.clone().unwrap_or_else(|| "未命名任務".to_string());
        let user_id_clone = user_id.clone();
        let expert_match_clone = req.expert_match.clone().unwrap_or_else(|| {
            crate::ai_service::ExpertMatch {
                expert: crate::ai_service::Expert {
                    name: "通用專家".to_string(),
                    description: "提供通用任務規劃和學習建議".to_string(),
                    expertise_areas: vec!["general".to_string()],
                    emoji: "🎯".to_string(),
                },
                ai_expert_name: "任務規劃專家".to_string(),
                ai_expert_description: "協助將主任務分解為可執行的子任務".to_string(),
            }
        });

        // 獲取數據庫連接的克隆
        let rb_clone = rb.get_ref().clone();

        // 啟動異步任務處理
        tokio::spawn(async move {
            log::info!("[異步任務] 開始生成子任務 for task {}", parent_task_id_clone);

            // 載入配置
            let config = crate::config::Config::from_env();

            // 創建 AI 服務
            let ai_service = match crate::ai_service::create_ai_service(&config.app.ai) {
                Ok(service) => service,
                Err(e) => {
                    log::error!("[異步任務] AI 服務初始化失敗: {}", e);
                    return;
                }
            };

            // 使用新的專用子任務生成函數
            let subtasks = match ai_service.generate_subtasks_for_main_task(
                &parent_task_title,
                &task_description_clone,
                &expert_match_clone,
            ).await {
                Ok(subtasks) => subtasks,
                Err(e) => {
                    log::error!("[異步任務] AI 生成子任務失敗: {}", e);
                    return;
                }
            };

            // 創建子任務
            let now = chrono::Utc::now();
            let mut task_order = 1;
            let mut created_count = 0;

            for (index, ai_subtask) in subtasks.into_iter().enumerate() {
                log::info!("[異步任務] 處理第 {} 個子任務: {:?}", index + 1, ai_subtask.title);

                let subtask_id = uuid::Uuid::new_v4().to_string();

                let subtask = Task {
                    id: Some(subtask_id.clone()),
                    user_id: Some(user_id_clone.clone()),
                    title: ai_subtask.title.clone(),
                    description: ai_subtask.description.clone(),
                    status: Some(0), // pending
                    priority: ai_subtask.priority,
                    task_type: ai_subtask.task_type.clone().or_else(|| Some("expert_subtask".to_string())),
                    difficulty: ai_subtask.difficulty,
                    experience: ai_subtask.experience,
                    due_date: ai_subtask.due_date.clone().and_then(|d| chrono::DateTime::parse_from_rfc3339(&d).ok().map(|dt| dt.with_timezone(&chrono::Utc))),
                    is_recurring: Some(0),
                    recurrence_pattern: None,
                    start_date: None,
                    end_date: None,
                    completion_target: ai_subtask.completion_target,
                    is_parent_task: Some(0), // 標記為子任務
                    task_order: Some(task_order),
                    created_at: Some(now),
                    updated_at: Some(now),
                    parent_task_id: Some(parent_task_id_clone.clone()),
                    career_mainline_id: None,
                    task_category: Some("expert_subtask".to_string()),
                    skill_tags: None,
                    completion_rate: Some(0.0),
                    task_date: None,
                    cancel_count: Some(0),
                    last_cancelled_at: None,
                    attributes: None,
                };

                if let Err(e) = Task::insert(&rb_clone, &subtask).await {
                    log::error!("[異步任務] 創建子任務失敗: {}", e);
                    continue;
                }

                created_count += 1;
                task_order += 1;
            }

            // 更新父任務狀態
            if created_count > 0 {
                let update_sql = "UPDATE task SET is_parent_task = 1 WHERE id = ?";
                if let Err(e) = rb_clone.exec(update_sql, vec![
                    rbs::Value::String(parent_task_id_clone.clone()),
                ]).await {
                    log::warn!("[異步任務] 更新父任務狀態失敗: {}", e);
                }

                log::info!("[異步任務] 成功為任務 {} 創建了 {} 個子任務", parent_task_id_clone, created_count);
            }
        });

        // 立即返回成功響應
        return Ok(HttpResponse::Ok().json(ApiResponse {
            success: true,
            data: Some(GenerateSubtasksResponse {
                total_count: 0,
                subtasks_created: Vec::new(),
            }),
            message: "子任務正在後台生成中，請稍後刷新查看".to_string(),
        }));
    }

    // 如果已經有子任務，直接使用（保持原有邏輯）
    let subtasks_to_create = subtasks_to_create;

    // 創建子任務
    let now = chrono::Utc::now();
    let mut created_subtasks = Vec::new();
    let mut task_order = 1;

    log::info!("[generate_subtasks_for_task] 準備創建 {} 個子任務", subtasks_to_create.len());

    for (index, ai_subtask) in subtasks_to_create.into_iter().enumerate() {
        log::info!("[generate_subtasks_for_task] 處理第 {} 個子任務: {:?}", index + 1, ai_subtask.title);

        let subtask_id = uuid::Uuid::new_v4().to_string();

        let subtask = Task {
            id: Some(subtask_id.clone()),
            user_id: Some(user_id.clone()),
            title: ai_subtask.title.clone(),
            description: ai_subtask.description.clone(),
            status: Some(0), // pending
            priority: ai_subtask.priority,
            task_type: ai_subtask.task_type.clone().or_else(|| Some("expert_subtask".to_string())),
            difficulty: ai_subtask.difficulty,
            experience: ai_subtask.experience,
            due_date: ai_subtask.due_date.clone().and_then(|d| chrono::DateTime::parse_from_rfc3339(&d).ok().map(|dt| dt.with_timezone(&chrono::Utc))),
            is_recurring: Some(0),
            recurrence_pattern: None,
            start_date: None,
            end_date: None,
            completion_target: ai_subtask.completion_target,
            is_parent_task: Some(0), // 標記為子任務
            task_order: Some(task_order),
            created_at: Some(now),
            updated_at: Some(now),
            parent_task_id: Some(req.parent_task_id.clone()),
            career_mainline_id: None,
            task_category: Some("expert_subtask".to_string()),
            skill_tags: None,
            completion_rate: Some(0.0),
            task_date: None,
            cancel_count: Some(0),
            last_cancelled_at: None,
            attributes: None,
        };

        // 插入子任務到資料庫
        match Task::insert(rb.get_ref(), &subtask).await {
            Ok(exec_result) => {
                log::info!("成功創建子任務: {}, 影響行數: {:?}",
                    subtask.title.as_deref().unwrap_or("未命名"),
                    exec_result.rows_affected);
                created_subtasks.push(subtask);
                task_order += 1;
            }
            Err(e) => {
                log::error!("創建子任務失敗: {}", e);
                log::error!("失敗的子任務數據: {:?}", subtask);
                // 繼續創建其他子任務
            }
        }
    }

    // 更新父任務的 is_parent_task 標記
    if !created_subtasks.is_empty() {
        let update_sql = "UPDATE task SET is_parent_task = 1 WHERE id = ?";
        if let Err(e) = rb.exec(update_sql, vec![
            rbs::Value::String(req.parent_task_id.clone()),
        ]).await {
            log::warn!("更新父任務狀態失敗: {}", e);
        }
    }

    log::info!(
        "[generate_subtasks_for_task] 成功為任務 {} 創建了 {} 個子任務",
        req.parent_task_id,
        created_subtasks.len()
    );

    let subtasks_count = created_subtasks.len();
    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        data: Some(GenerateSubtasksResponse {
            total_count: subtasks_count,
            subtasks_created: created_subtasks,
        }),
        message: format!("成功創建 {} 個子任務", subtasks_count),
    }))
}

// API: 專家分析
pub async fn expert_analysis(
    req: web::Json<ExpertAnalysisRequest>,
) -> Result<HttpResponse> {
    // 載入配置
    let config = crate::config::Config::from_env();
    
    // 創建 AI 服務
    let ai_service = match crate::ai_service::create_ai_service(&config.app.ai) {
        Ok(service) => service,
        Err(e) => {
            log::error!("AI 服務初始化失敗: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("AI 服務初始化失敗: {}", e),
            }));
        }
    };
    
    // 創建一個臨時的專家對象，使用AI返回的信息
    let expert = crate::ai_service::Expert {
        name: req.expert_name.clone(),
        description: "AI匹配的專家".to_string(), // 這個描述不會被使用
        expertise_areas: vec!["AI匹配".to_string()],
        emoji: "🤖".to_string(),
    };
    
    // 進行專家分析
    log::info!("開始專家分析: {} - {}", req.expert_name, req.analysis_type);
    let analysis_result = match ai_service.analyze_with_expert(&req.description, &req.expert_name, &req.expert_description, &req.analysis_type).await {
        Ok(result) => {
            log::info!("專家分析完成: {}", req.expert_name);
            result
        }
        Err(e) => {
            log::error!("專家分析失敗: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("專家分析失敗: {}", e),
            }));
        }
    };
    
    // 解析JSON結果
    let mut response = ExpertAnalysisResponse {
        analysis_result: analysis_result.clone(),
        directions: None,
        goals: None,
        resources: None,
    };

    // 處理分析加強方向
    if req.analysis_type == "analyze" {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&analysis_result) {
            if let Some(directions_array) = parsed.get("directions").and_then(|v| v.as_array()) {
                let directions: Vec<AnalysisDirection> = directions_array
                    .iter()
                    .filter_map(|item| {
                        if let (Some(title), Some(description)) = (
                            item.get("title").and_then(|v| v.as_str()),
                            item.get("description").and_then(|v| v.as_str()),
                        ) {
                            Some(AnalysisDirection {
                                title: title.to_string(),
                                description: description.to_string(),
                            })
                        } else {
                            None
                        }
                    })
                    .collect();

                if !directions.is_empty() {
                    response.directions = Some(directions);
                }
            }
        }
    }

    // 處理生成明確目標
    if req.analysis_type == "goals" {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&analysis_result) {
            if let Some(goals_array) = parsed.get("goals").and_then(|v| v.as_array()) {
                let goals: Vec<AnalysisGoal> = goals_array
                    .iter()
                    .filter_map(|item| {
                        if let (Some(title), Some(description)) = (
                            item.get("title").and_then(|v| v.as_str()),
                            item.get("description").and_then(|v| v.as_str()),
                        ) {
                            Some(AnalysisGoal {
                                title: title.to_string(),
                                description: description.to_string(),
                            })
                        } else {
                            None
                        }
                    })
                    .collect();

                if !goals.is_empty() {
                    response.goals = Some(goals);
                }
            }
        }
    }

    // 處理建議學習資源
    if req.analysis_type == "resources" {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&analysis_result) {
            if let Some(resources_array) = parsed.get("resources").and_then(|v| v.as_array()) {
                let resources: Vec<AnalysisResource> = resources_array
                    .iter()
                    .filter_map(|item| {
                        if let (Some(title), Some(description)) = (
                            item.get("title").and_then(|v| v.as_str()),
                            item.get("description").and_then(|v| v.as_str()),
                        ) {
                            Some(AnalysisResource {
                                title: title.to_string(),
                                description: description.to_string(),
                            })
                        } else {
                            None
                        }
                    })
                    .collect();

                if !resources.is_empty() {
                    response.resources = Some(resources);
                }
            }
        }
    }
    
    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        data: Some(response),
        message: "專家分析成功".to_string(),
    }))
}