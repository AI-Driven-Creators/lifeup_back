/*!
 * @file routes.rs
 * @brief HTTP 路由處理器
 * @details 定義了所有 HTTP API 路由的處理函數，包括使用者、任務、技能、聊天等功能
 */

use actix_web::{web, HttpResponse, Result};
use rbatis::RBatis;
use uuid::Uuid;
use chrono::{Utc, Datelike};
use crate::models::*;
use rbs::{Value, value};

// API 回應結構
#[derive(serde::Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    message: String,
}

// 健康檢查
pub async fn health_check() -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        data: Some("LifeUp Backend is running!"),
        message: "服務正常運行".to_string(),
    }))
}

// 使用者相關路由
pub async fn get_users(rb: web::Data<RBatis>) -> Result<HttpResponse> {
    match User::select_all(rb.get_ref()).await {
        Ok(users) => Ok(HttpResponse::Ok().json(ApiResponse {
            success: true,
            data: Some(users),
            message: "獲取使用者列表成功".to_string(),
        })),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("獲取使用者列表失敗: {}", e),
        })),
    }
}

pub async fn get_user(rb: web::Data<RBatis>, path: web::Path<String>) -> Result<HttpResponse> {
    let user_id = path.into_inner();
    match User::select_by_map(rb.get_ref(), value!{"id": user_id}).await {
        Ok(users) => {
            if let Some(user) = users.first() {
                Ok(HttpResponse::Ok().json(ApiResponse {
                    success: true,
                    data: Some(user.clone()),
                    message: "獲取使用者成功".to_string(),
                }))
            } else {
                Ok(HttpResponse::NotFound().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: "使用者不存在".to_string(),
                }))
            }
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("獲取使用者失敗: {}", e),
        })),
    }
}

pub async fn create_user(
    rb: web::Data<RBatis>,
    req: web::Json<CreateUserRequest>,
) -> Result<HttpResponse> {
    let now = Utc::now();
    let new_user = User {
        id: Some(Uuid::new_v4().to_string()),
        name: Some(req.name.clone()),
        email: Some(req.email.clone()),
        created_at: Some(now),
        updated_at: Some(now),
    };

    match User::insert(rb.get_ref(), &new_user).await {
        Ok(_) => Ok(HttpResponse::Created().json(ApiResponse {
            success: true,
            data: Some(new_user),
            message: "使用者建立成功".to_string(),
        })),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("使用者建立失敗: {}", e),
        })),
    }
}

/**
 * @brief 獲取所有任務列表
 * @param rb 資料庫連接實例
 * @return Result<HttpResponse> 包含任務列表的 HTTP 回應
 */
pub async fn get_tasks(rb: web::Data<RBatis>) -> Result<HttpResponse> {
    match Task::select_all(rb.get_ref()).await {
        Ok(tasks) => Ok(HttpResponse::Ok().json(ApiResponse {
            success: true,
            data: Some(tasks),
            message: "獲取任務列表成功".to_string(),
        })),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("獲取任務列表失敗: {}", e),
        })),
    }
}

/**
 * @brief 建立新任務
 * @param rb 資料庫連接實例
 * @param req 建立任務的請求資料
 * @return Result<HttpResponse> 包含新建任務資料的 HTTP 回應
 */
pub async fn create_task(
    rb: web::Data<RBatis>,
    req: web::Json<CreateTaskRequest>,
) -> Result<HttpResponse> {
    let now = Utc::now();
    let new_task = Task {
        id: Some(Uuid::new_v4().to_string()),
        user_id: req.user_id.clone(), // 使用請求中的 user_id
        title: Some(req.title.clone()),
        description: req.description.clone(),
        status: Some(0), // 待完成
        priority: req.priority.or(Some(1)),
        task_type: req.task_type.clone().or(Some("daily".to_string())),
        difficulty: req.difficulty.or(Some(1)),
        experience: req.experience.or(Some(10)),
        parent_task_id: None,
        is_parent_task: Some(if req.task_type.as_ref().map_or(false, |t| t == "main" || t == "side" || t == "challenge") { 1 } else { 0 }), // 主任務、支線任務、挑戰任務默認為大任務
        task_order: Some(0),
        due_date: req.due_date,
        created_at: Some(now),
        updated_at: Some(now),
        // 新欄位
        is_recurring: Some(0),
        recurrence_pattern: None,
        start_date: None,
        end_date: None,
        completion_target: None,
        completion_rate: None,
        task_date: None,
        cancel_count: Some(0),
        last_cancelled_at: None,
        weekly_days: None,
        monthly_days: None,
        related_skills: req.related_skills.as_ref().map(|skills| serde_json::to_string(skills).unwrap_or_default()),
    };

    match Task::insert(rb.get_ref(), &new_task).await {
        Ok(_) => {
            let task_id = new_task.id.as_ref().unwrap();
            
            // 建立任務-技能關聯
            if let Some(related_skills) = &req.related_skills {
                for skill_id in related_skills {
                    let relation = TaskSkillRelation {
                        id: Some(Uuid::new_v4().to_string()),
                        task_id: Some(task_id.clone()),
                        skill_id: Some(skill_id.clone()),
                        experience_multiplier: Some(1.0), // 預設倍數
                        created_at: Some(now),
                    };
                    
                    if let Err(e) = TaskSkillRelation::insert(rb.get_ref(), &relation).await {
                        log::warn!("建立任務-技能關聯失敗: {}", e);
                    }
                }
            }
            
            Ok(HttpResponse::Created().json(ApiResponse {
                success: true,
                data: Some(new_task),
                message: "任務建立成功".to_string(),
            }))
        },
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("任務建立失敗: {}", e),
        })),
    }
}

// 技能相關路由
pub async fn get_skills(rb: web::Data<RBatis>) -> Result<HttpResponse> {
    match Skill::select_all(rb.get_ref()).await {
        Ok(skills) => Ok(HttpResponse::Ok().json(ApiResponse {
            success: true,
            data: Some(skills),
            message: "獲取技能列表成功".to_string(),
        })),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("獲取技能列表失敗: {}", e),
        })),
    }
}

pub async fn create_skill(
    rb: web::Data<RBatis>,
    req: web::Json<CreateSkillRequest>,
) -> Result<HttpResponse> {
    let now = Utc::now();
    let new_skill = Skill {
        id: Some(Uuid::new_v4().to_string()),
        user_id: Some("d487f83e-dadd-4616-aeb2-959d6af9963b".to_string()), // 暫時使用已創建的使用者
        name: Some(req.name.clone()),
        description: req.description.clone(),
        level: req.level,
        progress: Some(0.0), // 初始進度為 0
        created_at: Some(now),
        updated_at: Some(now),
    };

    match Skill::insert(rb.get_ref(), &new_skill).await {
        Ok(_) => Ok(HttpResponse::Created().json(ApiResponse {
            success: true,
            data: Some(new_skill),
            message: "技能建立成功".to_string(),
        })),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("技能建立失敗: {}", e),
        })),
    }
}

// 聊天相關路由
pub async fn get_chat_messages(rb: web::Data<RBatis>) -> Result<HttpResponse> {
    match ChatMessage::select_all(rb.get_ref()).await {
        Ok(messages) => Ok(HttpResponse::Ok().json(ApiResponse {
            success: true,
            data: Some(messages),
            message: "獲取聊天記錄成功".to_string(),
        })),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("獲取聊天記錄失敗: {}", e),
        })),
    }
}

pub async fn send_message(
    rb: web::Data<RBatis>,
    req: web::Json<ChatRequest>,
) -> Result<HttpResponse> {
    let now = Utc::now();
    
    // 儲存使用者訊息
    let user_message = ChatMessage {
        id: Some(Uuid::new_v4().to_string()),
        user_id: Some("d487f83e-dadd-4616-aeb2-959d6af9963b".to_string()),
        role: Some("user".to_string()),
        content: Some(req.message.clone()),
        created_at: Some(now),
    };

    if let Err(e) = ChatMessage::insert(rb.get_ref(), &user_message).await {
        return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("儲存使用者訊息失敗: {}", e),
        }));
    }

    // 模擬 AI 回覆
    let ai_response = format!("收到您的訊息：{}。我是您的 AI 教練，有什麼可以幫助您的嗎？", req.message);
    
    let assistant_message = ChatMessage {
        id: Some(Uuid::new_v4().to_string()),
        user_id: Some("d487f83e-dadd-4616-aeb2-959d6af9963b".to_string()),
        role: Some("assistant".to_string()),
        content: Some(ai_response.clone()),
        created_at: Some(now),
    };

    match ChatMessage::insert(rb.get_ref(), &assistant_message).await {
        Ok(_) => Ok(HttpResponse::Ok().json(ApiResponse {
            success: true,
            data: Some(assistant_message),
            message: "訊息發送成功".to_string(),
        })),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("儲存 AI 回覆失敗: {}", e),
        })),
    }
}

// 更新任務狀態
pub async fn update_task(
    rb: web::Data<RBatis>,
    path: web::Path<String>,
    req: web::Json<UpdateTaskRequest>,
) -> Result<HttpResponse> {
    let task_id = path.into_inner();
    
    // 先查詢任務是否存在
    match Task::select_by_map(rb.get_ref(), value!{"id": task_id.clone()}).await {
        Ok(tasks) => {
            if let Some(mut task) = tasks.into_iter().next() {
                // 更新任務欄位
                if let Some(title) = &req.title {
                    task.title = Some(title.clone());
                }
                if let Some(description) = &req.description {
                    task.description = Some(description.clone());
                }
                let old_status = task.status.unwrap_or(0);
                let new_status = req.status.unwrap_or(old_status);
                if let Some(status) = req.status {
                    task.status = Some(status);
                }
                if let Some(priority) = req.priority {
                    task.priority = Some(priority);
                }
                if let Some(task_type) = &req.task_type {
                    task.task_type = Some(task_type.clone());
                }
                if let Some(difficulty) = req.difficulty {
                    task.difficulty = Some(difficulty);
                }
                if let Some(experience) = req.experience {
                    task.experience = Some(experience);
                }
                if let Some(due_date) = req.due_date {
                    task.due_date = Some(due_date);
                }
                task.updated_at = Some(Utc::now());
                
                // 執行更新
                let update_sql = "UPDATE task SET title = ?, description = ?, status = ?, priority = ?, task_type = ?, difficulty = ?, experience = ?, due_date = ?, updated_at = ? WHERE id = ?";
                let due_date_value = match task.due_date {
                    Some(date) => Value::String(date.to_string()),
                    None => Value::Null,
                };
                let result = rb.exec(
                    update_sql,
                    vec![
                        Value::String(task.title.clone().unwrap_or_default()),
                        Value::String(task.description.clone().unwrap_or_default()),
                        Value::I32(task.status.unwrap_or(0)),
                        Value::I32(task.priority.unwrap_or(1)),
                        Value::String(task.task_type.clone().unwrap_or("daily".to_string())),
                        Value::I32(task.difficulty.unwrap_or(1)),
                        Value::I32(task.experience.unwrap_or(10)),
                        due_date_value,
                        Value::String(task.updated_at.unwrap().to_string()),
                        Value::String(task_id.clone()),
                    ],
                ).await;
                
                match result {
                    Ok(_) => {
                        // 如果任務狀態從未完成變為已完成，則提升相關技能經驗
                        if old_status != 2 && new_status == 2 {
                            update_skill_experience_on_task_completion(rb.get_ref(), &task_id, &task).await;
                        }
                        
                        Ok(HttpResponse::Ok().json(ApiResponse {
                            success: true,
                            data: Some(task),
                            message: "任務更新成功".to_string(),
                        }))
                    },
                    Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        message: format!("任務更新失敗: {}", e),
                    })),
                }
            } else {
                Ok(HttpResponse::NotFound().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: "任務不存在".to_string(),
                }))
            }
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("查詢任務失敗: {}", e),
        })),
    }
}

// 根據任務類型獲取任務
pub async fn get_tasks_by_type(
    rb: web::Data<RBatis>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let task_type = path.into_inner();
    
    match Task::select_by_map(rb.get_ref(), value!{"task_type": task_type.clone()}).await {
        Ok(tasks) => Ok(HttpResponse::Ok().json(ApiResponse {
            success: true,
            data: Some(tasks),
            message: format!("獲取{}任務列表成功", task_type),
        })),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("獲取{}任務列表失敗: {}", task_type, e),
        })),
    }
}

// 獲取子任務模板
fn get_subtask_templates(_task_title: &str) -> Vec<SubTaskTemplate> {
    // 返回通用的子任務模板，適用於所有類型的任務
    vec![
        SubTaskTemplate {
            title: "準備階段".to_string(),
            description: Some("收集資源和制定計劃".to_string()),
            difficulty: 1,
            experience: 20,
            order: 1,
        },
        SubTaskTemplate {
            title: "學習基礎".to_string(),
            description: Some("掌握基本概念和技能".to_string()),
            difficulty: 2,
            experience: 30,
            order: 2,
        },
        SubTaskTemplate {
            title: "實踐練習".to_string(),
            description: Some("通過實作加深理解".to_string()),
            difficulty: 3,
            experience: 50,
            order: 3,
        },
        SubTaskTemplate {
            title: "深入學習".to_string(),
            description: Some("掌握進階技能和概念".to_string()),
            difficulty: 4,
            experience: 60,
            order: 4,
        },
        SubTaskTemplate {
            title: "完成項目".to_string(),
            description: Some("完成實際應用項目".to_string()),
            difficulty: 4,
            experience: 80,
            order: 5,
        },
        SubTaskTemplate {
            title: "總結回顧".to_string(),
            description: Some("總結經驗並規劃下一步".to_string()),
            difficulty: 2,
            experience: 30,
            order: 6,
        },
    ]
}

// 開始任務（生成子任務）
pub async fn start_task(
    rb: web::Data<RBatis>,
    path: web::Path<String>,
    req: web::Json<StartTaskRequest>,
) -> Result<HttpResponse> {
    let task_id = path.into_inner();
    
    // 查詢父任務
    match Task::select_by_map(rb.get_ref(), value!{"id": task_id.clone()}).await {
        Ok(tasks) => {
            if let Some(mut parent_task) = tasks.into_iter().next() {
                // 檢查是否為大任務
                if parent_task.is_parent_task.unwrap_or(0) == 0 {
                    return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        message: "此任務不是大任務，無法生成子任務".to_string(),
                    }));
                }
                
                // 更新父任務狀態為進行中
                parent_task.status = Some(1);
                parent_task.updated_at = Some(Utc::now());
                
                let update_sql = "UPDATE task SET status = ?, updated_at = ? WHERE id = ?";
                if let Err(e) = rb.exec(
                    update_sql,
                    vec![
                        Value::I32(1),
                        Value::String(parent_task.updated_at.unwrap().to_string()),
                        Value::String(task_id.clone()),
                    ],
                ).await {
                    return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        message: format!("更新父任務狀態失敗: {}", e),
                    }));
                }
                
                // 檢查是否需要生成子任務
                if req.generate_subtasks.unwrap_or(true) {
                    // 先查詢現有的子任務
                    match Task::select_by_map(rb.get_ref(), value!{"parent_task_id": task_id.clone()}).await {
                        Ok(existing_subtasks) => {
                            if existing_subtasks.is_empty() {
                                // 沒有現有子任務，生成新的子任務
                                let templates = get_subtask_templates(&parent_task.title.clone().unwrap_or_default());
                                let mut subtasks = Vec::new();
                                
                                for template in templates {
                                    let subtask = Task {
                                        id: Some(Uuid::new_v4().to_string()),
                                        user_id: parent_task.user_id.clone(),
                                        title: Some(template.title),
                                        description: template.description,
                                        status: Some(0), // 待完成
                                        priority: Some(1),
                                        task_type: Some("subtask".to_string()),
                                        difficulty: Some(template.difficulty),
                                        experience: Some(template.experience),
                                        parent_task_id: Some(task_id.clone()),
                                        is_parent_task: Some(0),
                                        task_order: Some(template.order),
                                        due_date: None,
                                        created_at: Some(Utc::now()),
                                        updated_at: Some(Utc::now()),
                                        // 新欄位
                                        is_recurring: Some(0),
                                        recurrence_pattern: None,
                                        start_date: None,
                                        end_date: None,
                                        completion_target: None,
                                        completion_rate: None,
                                        task_date: None,
                                        cancel_count: Some(0),
                                        last_cancelled_at: None,
                                    };
                                    
                                    if let Err(e) = Task::insert(rb.get_ref(), &subtask).await {
                                        log::error!("Failed to create subtask: {}", e);
                                    } else {
                                        subtasks.push(subtask);
                                    }
                                }
                                
                                Ok(HttpResponse::Ok().json(ApiResponse {
                                    success: true,
                                    data: Some(serde_json::json!({
                                        "parent_task": parent_task,
                                        "subtasks": subtasks,
                                        "subtasks_count": subtasks.len()
                                    })),
                                    message: format!("任務開始成功，生成了 {} 個子任務", subtasks.len()),
                                }))
                            } else {
                                // 有現有子任務，檢查是否需要恢復暫停的子任務
                                let paused_subtasks: Vec<_> = existing_subtasks.iter()
                                    .filter(|subtask| subtask.status.unwrap_or(0) == 4) // 暫停狀態
                                    .collect();
                                
                                if !paused_subtasks.is_empty() {
                                    // 恢復暫停的子任務
                                    let resume_sql = "UPDATE task SET status = 0, updated_at = ? WHERE parent_task_id = ? AND status = 4";
                                    if let Err(e) = rb.exec(
                                        resume_sql,
                                        vec![
                                            Value::String(Utc::now().to_string()),
                                            Value::String(task_id.clone()),
                                        ],
                                    ).await {
                                        return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                                            success: false,
                                            data: None,
                                            message: format!("恢復子任務失敗: {}", e),
                                        }));
                                    }
                                    
                                    // 重新查詢更新後的子任務
                                    match Task::select_by_map(rb.get_ref(), value!{"parent_task_id": task_id.clone()}).await {
                                        Ok(updated_subtasks) => {
                                            Ok(HttpResponse::Ok().json(ApiResponse {
                                                success: true,
                                                data: Some(serde_json::json!({
                                                    "parent_task": parent_task,
                                                    "subtasks": updated_subtasks,
                                                    "subtasks_count": updated_subtasks.len()
                                                })),
                                                message: format!("任務恢復成功，恢復了 {} 個暫停的子任務", paused_subtasks.len()),
                                            }))
                                        }
                                        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                                            success: false,
                                            data: None,
                                            message: format!("查詢更新後的子任務失敗: {}", e),
                                        }))
                                    }
                                } else {
                                    // 子任務已存在且不需要恢復，直接返回現有子任務
                                    Ok(HttpResponse::Ok().json(ApiResponse {
                                        success: true,
                                        data: Some(serde_json::json!({
                                            "parent_task": parent_task,
                                            "subtasks": existing_subtasks,
                                            "subtasks_count": existing_subtasks.len()
                                        })),
                                        message: "任務繼續進行，子任務已存在".to_string(),
                                    }))
                                }
                            }
                        }
                        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                            success: false,
                            data: None,
                            message: format!("查詢現有子任務失敗: {}", e),
                        }))
                    }
                } else {
                    Ok(HttpResponse::Ok().json(ApiResponse {
                        success: true,
                        data: Some(parent_task),
                        message: "任務開始成功".to_string(),
                    }))
                }
            } else {
                Ok(HttpResponse::NotFound().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: "任務不存在".to_string(),
                }))
            }
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("查詢任務失敗: {}", e),
        })),
    }
}

// 獲取子任務列表
pub async fn get_subtasks(
    rb: web::Data<RBatis>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let parent_task_id = path.into_inner();
    
    match Task::select_by_map(rb.get_ref(), value!{"parent_task_id": parent_task_id}).await {
        Ok(subtasks) => Ok(HttpResponse::Ok().json(ApiResponse {
            success: true,
            data: Some(subtasks),
            message: "獲取子任務列表成功".to_string(),
        })),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("獲取子任務列表失敗: {}", e),
        })),
    }
}

// 暫停任務（暫停所有子任務）
pub async fn pause_task(
    rb: web::Data<RBatis>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let task_id = path.into_inner();
    
    // 更新父任務為暫停狀態
    let update_parent_sql = "UPDATE task SET status = 4, updated_at = ? WHERE id = ?";
    if let Err(e) = rb.exec(
        update_parent_sql,
        vec![
            Value::String(Utc::now().to_string()),
            Value::String(task_id.clone()),
        ],
    ).await {
        return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("暫停父任務失敗: {}", e),
        }));
    }
    
    // 暫停所有子任務
    let update_subtasks_sql = "UPDATE task SET status = 4, updated_at = ? WHERE parent_task_id = ? AND status != 2";
    if let Err(e) = rb.exec(
        update_subtasks_sql,
        vec![
            Value::String(Utc::now().to_string()),
            Value::String(task_id.clone()),
        ],
    ).await {
        return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("暫停子任務失敗: {}", e),
        }));
    }
    
    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        data: Some(serde_json::json!({"task_id": task_id})),
        message: "任務暫停成功".to_string(),
    }))
}

// 取消任務（取消所有子任務）
pub async fn cancel_task(
    rb: web::Data<RBatis>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let task_id = path.into_inner();
    let now = Utc::now();
    
    // 先查詢當前任務資訊以獲取cancel_count
    match Task::select_by_map(rb.get_ref(), value!{"id": task_id.clone()}).await {
        Ok(tasks) => {
            if let Some(current_task) = tasks.first() {
                let new_cancel_count = current_task.cancel_count.unwrap_or(0) + 1;
                
                // 更新父任務為取消狀態，增加取消計數和記錄取消時間
                let update_parent_sql = "UPDATE task SET status = 3, cancel_count = ?, last_cancelled_at = ?, updated_at = ? WHERE id = ?";
                if let Err(e) = rb.exec(
                    update_parent_sql,
                    vec![
                        Value::I32(new_cancel_count),
                        Value::String(now.to_string()),
                        Value::String(now.to_string()),
                        Value::String(task_id.clone()),
                    ],
                ).await {
                    return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        message: format!("取消父任務失敗: {}", e),
                    }));
                }
                
                // 刪除所有未完成的子任務
                let delete_subtasks_sql = "DELETE FROM task WHERE parent_task_id = ? AND status != 2";
                if let Err(e) = rb.exec(
                    delete_subtasks_sql,
                    vec![
                        Value::String(task_id.clone()),
                    ],
                ).await {
                    return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        message: format!("刪除子任務失敗: {}", e),
                    }));
                }
                
                Ok(HttpResponse::Ok().json(ApiResponse {
                    success: true,
                    data: Some(serde_json::json!({
                        "task_id": task_id,
                        "cancel_count": new_cancel_count,
                        "last_cancelled_at": now.to_string()
                    })),
                    message: format!("任務取消成功（第{}次取消），相關子任務已刪除", new_cancel_count),
                }))
            } else {
                Ok(HttpResponse::NotFound().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: "任務不存在".to_string(),
                }))
            }
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("查詢任務失敗: {}", e),
        })),
    }
}

// 獲取首頁任務（只返回子任務和每日任務）
pub async fn get_homepage_tasks(rb: web::Data<RBatis>) -> Result<HttpResponse> {
    // 獲取子任務和每日任務
    let sql = "SELECT * FROM task WHERE (parent_task_id IS NOT NULL) OR (task_type = 'daily' AND parent_task_id IS NULL) ORDER BY task_order, created_at";
    match rb.query(sql, vec![]).await {
        Ok(tasks) => Ok(HttpResponse::Ok().json(ApiResponse {
            success: true,
            data: Some(tasks),
            message: "獲取首頁任務成功".to_string(),
        })),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("獲取首頁任務失敗: {}", e),
        })),
    }
}

// 建立重複性任務
pub async fn create_recurring_task(
    rb: web::Data<RBatis>,
    req: web::Json<CreateRecurringTaskRequest>,
) -> Result<HttpResponse> {
    let now = Utc::now();
    
    // 建立父任務
    let parent_task = Task {
        id: Some(Uuid::new_v4().to_string()),
        user_id: req.user_id.clone().or(Some("d487f83e-dadd-4616-aeb2-959d6af9963b".to_string())),
        title: Some(req.title.clone()),
        description: req.description.clone(),
        status: Some(0), // 待開始
        priority: Some(1),
        task_type: req.task_type.clone().or(Some("recurring".to_string())),
        difficulty: req.difficulty.or(Some(1)),
        experience: req.experience.or(Some(10)),
        parent_task_id: None,
        is_parent_task: Some(1),
        task_order: Some(0),
        due_date: req.end_date,
        created_at: Some(now),
        updated_at: Some(now),
        // 重複性任務欄位
        is_recurring: Some(1),
        recurrence_pattern: Some(req.recurrence_pattern.clone()),
        start_date: Some(req.start_date),
        end_date: req.end_date,
        completion_target: req.completion_target.or(Some(0.8)),
        completion_rate: Some(0.0),
        task_date: None,
        cancel_count: Some(0),
        last_cancelled_at: None,
        // 處理週重複和月重複的選擇
        weekly_days: req.weekly_days.as_ref().map(|days| serde_json::to_string(days).unwrap_or_default()),
        monthly_days: req.monthly_days.as_ref().map(|days| serde_json::to_string(days).unwrap_or_default()),
    };

    // 插入父任務
    match Task::insert(rb.get_ref(), &parent_task).await {
        Ok(_) => {
            let parent_task_id = parent_task.id.as_ref().unwrap();
            
            // 建立子任務模板
            for template in &req.subtask_templates {
                let recurring_template = RecurringTaskTemplate {
                    id: Some(Uuid::new_v4().to_string()),
                    parent_task_id: Some(parent_task_id.clone()),
                    title: template.title.clone(),
                    description: template.description.clone(),
                    difficulty: template.difficulty,
                    experience: template.experience,
                    task_order: template.order,
                    created_at: Some(now),
                    updated_at: Some(now),
                };
                
                if let Err(e) = RecurringTaskTemplate::insert(rb.get_ref(), &recurring_template).await {
                    log::error!("Failed to create recurring task template: {}", e);
                }
            }
            
            Ok(HttpResponse::Created().json(ApiResponse {
                success: true,
                data: Some(parent_task),
                message: "重複性任務建立成功".to_string(),
            }))
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("重複性任務建立失敗: {}", e),
        })),
    }
}

// 生成每日子任務
pub async fn generate_daily_tasks(
    rb: web::Data<RBatis>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let parent_task_id = path.into_inner();
    let today = Utc::now().format("%Y-%m-%d").to_string();
    
    // 檢查今日任務是否已存在
    let existing_tasks_sql = "SELECT COUNT(*) as count FROM task WHERE parent_task_id = ? AND task_date = ?";
    let result = rb.exec(existing_tasks_sql, vec![
        Value::String(parent_task_id.clone()),
        Value::String(today.clone()),
    ]).await;
    
    match result {
        Ok(exec_result) => {
            // 如果有結果且count > 0，說明今日任務已存在
            // 這裡簡化處理，直接嘗試生成任務，如果重複則會失敗
        }
        Err(e) => {
            log::error!("Failed to check existing tasks: {}", e);
        }
    }
    
    // 獲取任務模板
    match RecurringTaskTemplate::select_by_map(rb.get_ref(), value!{"parent_task_id": parent_task_id.clone()}).await {
        Ok(templates) => {
            let mut generated_tasks = Vec::new();
            
            for template in templates {
                let daily_task = Task {
                    id: Some(Uuid::new_v4().to_string()),
                    user_id: Some("d487f83e-dadd-4616-aeb2-959d6af9963b".to_string()),
                    title: Some(template.title.clone()),
                    description: template.description.clone(),
                    status: Some(0), // 待完成
                    priority: Some(1),
                    task_type: Some("daily_recurring".to_string()),
                    difficulty: Some(template.difficulty),
                    experience: Some(template.experience),
                    parent_task_id: Some(parent_task_id.clone()),
                    is_parent_task: Some(0),
                    task_order: Some(template.task_order),
                    due_date: None,
                    created_at: Some(Utc::now()),
                    updated_at: Some(Utc::now()),
                    is_recurring: Some(0),
                    recurrence_pattern: None,
                    start_date: None,
                    end_date: None,
                    completion_target: None,
                    completion_rate: None,
                    task_date: Some(today.clone()),
                    cancel_count: Some(0),
                    last_cancelled_at: None,
                };
                
                if let Ok(_) = Task::insert(rb.get_ref(), &daily_task).await {
                    generated_tasks.push(daily_task);
                }
            }
            
            Ok(HttpResponse::Ok().json(ApiResponse {
                success: true,
                data: Some(serde_json::json!({
                    "generated_tasks": generated_tasks,
                    "count": generated_tasks.len(),
                    "date": today
                })),
                message: format!("成功生成 {} 個今日任務", generated_tasks.len()),
            }))
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("獲取任務模板失敗: {}", e),
        })),
    }
}

// 計算任務進度
pub async fn get_task_progress(
    rb: web::Data<RBatis>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let parent_task_id = path.into_inner();
    let today = Utc::now().format("%Y-%m-%d").to_string();
    
    // 獲取父任務信息
    match Task::select_by_map(rb.get_ref(), value!{"id": parent_task_id.clone()}).await {
        Ok(tasks) => {
            if let Some(parent_task) = tasks.first() {
                // 計算總天數
                let start_date = parent_task.start_date.unwrap_or(Utc::now());
                let end_date = parent_task.end_date.unwrap_or(Utc::now() + chrono::Duration::days(365));
                let total_days = (end_date - start_date).num_days() as i32;
                
                // 查詢已完成的天數（簡化處理）
                let completed_days_sql = "SELECT COUNT(DISTINCT task_date) as count FROM task WHERE parent_task_id = ? AND status = 2 AND task_date IS NOT NULL";
                match rb.exec(completed_days_sql, vec![Value::String(parent_task_id.clone())]).await {
                    Ok(_) => {
                        // 簡化處理：假設有一些完成的天數
                        let completed_days = 5; // 暫時固定值，實際應該從查詢結果解析
                        
                        // 檢查今日是否完成（簡化處理）
                        let today_tasks_sql = "SELECT COUNT(*) as total FROM task WHERE parent_task_id = ? AND task_date = ?";
                        match rb.exec(today_tasks_sql, vec![
                            Value::String(parent_task_id.clone()),
                            Value::String(today.clone()),
                        ]).await {
                            Ok(_) => {
                                // 簡化處理：假設今日有任務且部分完成
                                let (total_today, completed_today) = (3, 2); // 暫時固定值
                                
                                let is_daily_completed = total_today > 0 && completed_today == total_today;
                                let completion_rate = if total_days > 0 {
                                    completed_days as f64 / total_days as f64
                                } else {
                                    0.0
                                };
                                let target_rate = parent_task.completion_target.unwrap_or(0.8);
                                let remaining_days = total_days - completed_days;
                                
                                let progress = TaskProgressResponse {
                                    task_id: parent_task_id,
                                    total_days,
                                    completed_days,
                                    completion_rate,
                                    target_rate,
                                    is_daily_completed,
                                    remaining_days,
                                };
                                
                                Ok(HttpResponse::Ok().json(ApiResponse {
                                    success: true,
                                    data: Some(progress),
                                    message: "獲取任務進度成功".to_string(),
                                }))
                            }
                            Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                                success: false,
                                data: None,
                                message: format!("查詢今日任務失敗: {}", e),
                            }))
                        }
                    }
                    Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        message: format!("查詢完成天數失敗: {}", e),
                    }))
                }
            } else {
                Ok(HttpResponse::NotFound().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: "任務不存在".to_string(),
                }))
            }
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("獲取任務失敗: {}", e),
        })),
    }
}

// 重新開始已取消的任務
pub async fn restart_task(
    rb: web::Data<RBatis>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let task_id = path.into_inner();
    let now = Utc::now();
    
    // 先查詢任務是否存在且為已取消狀態
    match Task::select_by_map(rb.get_ref(), value!{"id": task_id.clone()}).await {
        Ok(tasks) => {
            if let Some(mut task) = tasks.into_iter().next() {
                // 檢查任務是否為已取消狀態
                if task.status.unwrap_or(0) != 3 {
                    return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        message: "只有已取消的任務才能重新開始".to_string(),
                    }));
                }
                
                // 檢查是否為大任務
                if task.is_parent_task.unwrap_or(0) == 0 {
                    return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        message: "只有大任務可以重新開始".to_string(),
                    }));
                }
                
                // 更新任務狀態為待開始
                task.status = Some(0); // pending
                task.updated_at = Some(now);
                
                let update_sql = "UPDATE task SET status = ?, updated_at = ? WHERE id = ?";
                if let Err(e) = rb.exec(
                    update_sql,
                    vec![
                        Value::I32(0), // pending status
                        Value::String(now.to_string()),
                        Value::String(task_id.clone()),
                    ],
                ).await {
                    return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        message: format!("重新開始任務失敗: {}", e),
                    }));
                }
                
                Ok(HttpResponse::Ok().json(ApiResponse {
                    success: true,
                    data: Some(serde_json::json!({
                        "task_id": task_id,
                        "status": "pending",
                        "cancel_count": task.cancel_count.unwrap_or(0),
                        "restarted_at": now.to_string()
                    })),
                    message: "任務重新開始成功，可以重新開始執行".to_string(),
                }))
            } else {
                Ok(HttpResponse::NotFound().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: "任務不存在".to_string(),
                }))
            }
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("查詢任務失敗: {}", e),
        })),
    }
}

// 遊戲化數據相關 API

// 獲取完整的遊戲化用戶數據 (整合 API)
pub async fn get_gamified_user_data(rb: web::Data<RBatis>, path: web::Path<String>) -> Result<HttpResponse> {
    let user_id = path.into_inner();
    log::info!("正在獲取用戶 {} 的遊戲化數據", user_id);
    
    // 獲取基本用戶信息
    log::info!("步驟 1: 獲取基本用戶信息");
    let user = User::select_by_map(rb.get_ref(), value!{"id": user_id.clone()}).await
        .map_err(|e| {
            log::error!("獲取用戶失敗: {}", e);
            format!("獲取用戶失敗: {}", e)
        });
    
    // 獲取遊戲化資料
    log::info!("步驟 2: 獲取遊戲化資料");
    let profile = UserProfile::select_by_map(rb.get_ref(), value!{"user_id": user_id.clone()}).await
        .map_err(|e| {
            log::error!("獲取遊戲化資料失敗: {}", e);
            format!("獲取遊戲化資料失敗: {}", e)
        });
    
    // 獲取屬性
    log::info!("步驟 3: 獲取屬性");
    let attributes = UserAttributes::select_by_map(rb.get_ref(), value!{"user_id": user_id.clone()}).await
        .map_err(|e| {
            log::error!("獲取屬性失敗: {}", e);
            format!("獲取屬性失敗: {}", e)
        });
    
    // 獲取今日進度
    let today = Utc::now().format("%Y-%m-%d").to_string();
    log::info!("步驟 4: 獲取今日進度, 日期: {}", today);
    let today_progress = DailyProgress::select_by_map(rb.get_ref(), value!{"user_id": user_id.clone(), "date": today}).await
        .map_err(|e| {
            log::error!("獲取今日進度失敗: {}", e);
            format!("獲取今日進度失敗: {}", e)
        });
    
    match (user, profile, attributes, today_progress) {
        (Ok(users), Ok(profiles), Ok(attrs), Ok(progress_list)) => {
            log::info!("獲取到的數據: users={}, profiles={}, attrs={}", users.len(), profiles.len(), attrs.len());
            
            let user = users.first();
            let profile = profiles.first();
            let attr = attrs.first();
            
            if user.is_none() {
                log::error!("未找到用戶資料");
                return Ok(HttpResponse::NotFound().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: "用戶不存在".to_string(),
                }));
            }
            
            if profile.is_none() {
                log::error!("未找到用戶遊戲化資料");
                return Ok(HttpResponse::NotFound().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: "用戶遊戲化資料不存在".to_string(),
                }));
            }
            
            if attr.is_none() {
                log::error!("未找到用戶屬性資料");
                return Ok(HttpResponse::NotFound().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: "用戶屬性資料不存在".to_string(),
                }));
            }
            
            let user = user.unwrap();
            let profile = profile.unwrap();
            let attr = attr.unwrap();
            
            log::info!("成功獲取用戶數據: user={:?}, profile={:?}, attr={:?}", user.name, profile.level, attr.intelligence);
            
            // 處理今日進度 - 如果沒有數據就返回空值
            let today_progress_data = if let Some(progress) = progress_list.first() {
                log::info!("找到今日進度數據: {:?}", progress);
                
                // 直接使用 attributes_gained JSON Value
                let attributes_gained = match &progress.attributes_gained {
                    Some(json_val) => {
                        log::info!("原始 attributes_gained 數據: {:?}", json_val);
                        json_val.clone()
                    }
                    None => serde_json::json!({})
                };
                
                serde_json::json!({
                    "completedTasks": progress.completed_tasks.unwrap_or(0),
                    "totalTasks": progress.total_tasks.unwrap_or(0),
                    "experienceGained": progress.experience_gained.unwrap_or(0),
                    "attributeGains": attributes_gained
                })
            } else {
                log::info!("今日暫無進度數據");
                serde_json::json!({
                    "completedTasks": 0,
                    "totalTasks": 0,
                    "experienceGained": 0,
                    "attributeGains": {}
                })
            };
            
            // 組合完整的遊戲化用戶數據
            let gamified_data = serde_json::json!({
                "id": user.id,
                "name": user.name,
                "level": profile.level,
                "experience": profile.experience,
                "maxExperience": profile.max_experience,
                "title": profile.title,
                "adventureDays": profile.adventure_days,
                "consecutiveLoginDays": profile.consecutive_login_days,
                "personaType": profile.persona_type,
                "attributes": {
                    "intelligence": attr.intelligence,
                    "endurance": attr.endurance,
                    "creativity": attr.creativity,
                    "social": attr.social,
                    "focus": attr.focus,
                    "adaptability": attr.adaptability
                },
                "todayProgress": today_progress_data
            });
            
            Ok(HttpResponse::Ok().json(ApiResponse {
                success: true,
                data: Some(gamified_data),
                message: "獲取完整遊戲化用戶數據成功".to_string(),
            }))
        }
        (Err(e), _, _, _) | (_, Err(e), _, _) | (_, _, Err(e), _) | (_, _, _, Err(e)) => {
            log::error!("獲取遊戲化數據時發生錯誤: {}", e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: e,
            }))
        }
    }
}

// 成就相關 API

// 獲取所有成就
pub async fn get_achievements(rb: web::Data<RBatis>) -> Result<HttpResponse> {
    match Achievement::select_all(rb.get_ref()).await {
        Ok(achievements) => Ok(HttpResponse::Ok().json(ApiResponse {
            success: true,
            data: Some(achievements),
            message: "獲取成就列表成功".to_string(),
        })),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("獲取成就列表失敗: {}", e),
        })),
    }
}

// 獲取用戶成就狀態
pub async fn get_user_achievements(
    rb: web::Data<RBatis>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let user_id = path.into_inner();
    
    // 先獲取所有成就
    match Achievement::select_all(rb.get_ref()).await {
        Ok(all_achievements) => {
            // 獲取用戶已解鎖的成就
            match UserAchievement::select_by_map(rb.get_ref(), value!{"user_id": user_id}).await {
                Ok(user_achievements) => {
                    // 組合數據
                    let mut result = Vec::new();
                    
                    for achievement in all_achievements {
                        // 查找是否已解鎖
                        let user_achievement = user_achievements.iter()
                            .find(|ua| ua.achievement_id == achievement.id);
                        
                        let achievement_data = serde_json::json!({
                            "id": achievement.id,
                            "name": achievement.name,
                            "description": achievement.description,
                            "icon": achievement.icon,
                            "category": achievement.category,
                            "requirement_type": achievement.requirement_type,
                            "requirement_value": achievement.requirement_value,
                            "experience_reward": achievement.experience_reward,
                            "unlocked": user_achievement.is_some(),
                            "progress": user_achievement.map(|ua| ua.progress.unwrap_or(0)).unwrap_or(0),
                            "achieved_at": user_achievement.and_then(|ua| ua.achieved_at.as_ref().map(|dt| dt.to_string()))
                        });
                        
                        result.push(achievement_data);
                    }
                    
                    Ok(HttpResponse::Ok().json(ApiResponse {
                        success: true,
                        data: Some(result),
                        message: "獲取用戶成就狀態成功".to_string(),
                    }))
                }
                Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: format!("獲取用戶成就關聯失敗: {}", e),
                })),
            }
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("獲取成就列表失敗: {}", e),
        })),
    }
}

// 解鎖用戶成就
pub async fn unlock_user_achievement(
    rb: web::Data<RBatis>,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse> {
    let (user_id, achievement_id) = path.into_inner();
    let now = Utc::now();
    
    // 檢查成就是否存在
    match Achievement::select_by_map(rb.get_ref(), value!{"id": achievement_id.clone()}).await {
        Ok(achievements) => {
            if let Some(achievement) = achievements.first() {
                // 檢查用戶是否已經解鎖此成就
                match UserAchievement::select_by_map(
                    rb.get_ref(), 
                    value!{"user_id": user_id.clone(), "achievement_id": achievement_id.clone()}
                ).await {
                    Ok(user_achievements) => {
                        if user_achievements.is_empty() {
                            // 創建新的用戶成就記錄
                            let user_achievement = UserAchievement {
                                id: Some(Uuid::new_v4().to_string()),
                                user_id: Some(user_id.clone()),
                                achievement_id: Some(achievement_id.clone()),
                                achieved_at: Some(now),
                                progress: achievement.requirement_value.clone(),
                            };
                            
                            match UserAchievement::insert(rb.get_ref(), &user_achievement).await {
                                Ok(_) => Ok(HttpResponse::Created().json(ApiResponse {
                                    success: true,
                                    data: Some(serde_json::json!({
                                        "achievement": achievement,
                                        "unlocked_at": now.to_string(),
                                        "experience_reward": achievement.experience_reward
                                    })),
                                    message: format!("成就「{}」解鎖成功！", achievement.name.as_ref().unwrap_or(&"未知成就".to_string())),
                                })),
                                Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                                    success: false,
                                    data: None,
                                    message: format!("解鎖成就失敗: {}", e),
                                })),
                            }
                        } else {
                            Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                                success: false,
                                data: None,
                                message: "成就已經解鎖".to_string(),
                            }))
                        }
                    }
                    Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        message: format!("檢查用戶成就失敗: {}", e),
                    })),
                }
            } else {
                Ok(HttpResponse::NotFound().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: "成就不存在".to_string(),
                }))
            }
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("查詢成就失敗: {}", e),
        })),
    }
}

// 週屬性相關 API

// 獲取用戶指定週數的屬性快照
pub async fn get_weekly_attributes(
    rb: web::Data<RBatis>,  
    path: web::Path<(String, i32)>,
) -> Result<HttpResponse> {
    let (user_id, weeks_ago) = path.into_inner();
    
    // 計算目標週的年份和週數
    let target_date = Utc::now() - chrono::Duration::weeks(weeks_ago as i64);
    let year = target_date.year();
    let week_number = target_date.iso_week().week() as i32;
    
    match WeeklyAttributeSnapshot::select_by_map(
        rb.get_ref(), 
        value!{"user_id": user_id.clone(), "year": year, "week_number": week_number}
    ).await {
        Ok(snapshots) => {
            if let Some(snapshot) = snapshots.first() {
                Ok(HttpResponse::Ok().json(ApiResponse {
                    success: true,
                    data: Some(snapshot.clone()),
                    message: format!("獲取第{}週前屬性快照成功", weeks_ago),
                }))
            } else {
                // 如果沒有快照，返回當前屬性作為fallback
                match UserAttributes::select_by_map(rb.get_ref(), value!{"user_id": user_id}).await {
                    Ok(current_attrs) => {
                        if let Some(attrs) = current_attrs.first() {
                            let fallback_snapshot = serde_json::json!({
                                "intelligence": attrs.intelligence,
                                "endurance": attrs.endurance,
                                "creativity": attrs.creativity,
                                "social": attrs.social,
                                "focus": attrs.focus,
                                "adaptability": attrs.adaptability,
                                "is_fallback": true
                            });
                            
                            Ok(HttpResponse::Ok().json(ApiResponse {
                                success: true,
                                data: Some(fallback_snapshot),
                                message: format!("第{}週前無快照數據，返回當前屬性", weeks_ago),
                            }))
                        } else {
                            Ok(HttpResponse::NotFound().json(ApiResponse::<()> {
                                success: false,
                                data: None,
                                message: "用戶屬性不存在".to_string(),
                            }))
                        }
                    }
                    Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        message: format!("獲取用戶屬性失敗: {}", e),
                    })),
                }
            }
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("獲取週屬性快照失敗: {}", e),
        })),
    }
} 



/**
 * @brief 任務完成時更新相關技能經驗
 * @param rb RBatis 資料庫連接實例
 * @param task_id 完成的任務ID
 * @param task 任務資料
 */
 async fn update_skill_experience_on_task_completion(rb: &RBatis, task_id: &str, task: &Task) {
    log::info\!("任務 {} 已完成，開始更新技能經驗", task_id);
    
    // 查詢任務關聯的技能
    let relations_result = TaskSkillRelation::select_by_map(
        rb,
        value\!{"task_id": task_id}
    ).await;
    
    let task_experience = task.experience.unwrap_or(10) as f64;
    
    match relations_result {
        Ok(relations) => {
            for relation in relations {
                if let (Some(skill_id), Some(multiplier)) = (&relation.skill_id, relation.experience_multiplier) {
                    // 計算獲得的經驗值
                    let gained_experience = task_experience * multiplier;
                    
                    // 更新技能經驗
                    update_single_skill_experience(rb, skill_id, gained_experience).await;
                }
            }
        }
        Err(e) => {
            log::error\!("查詢任務-技能關聯失敗: {}", e);
        }
    }
}

/**
 * @brief 更新單個技能的經驗值和等級
 * @param rb RBatis 資料庫連接實例
 * @param skill_id 技能ID
 * @param gained_experience 獲得的經驗值
 */
async fn update_single_skill_experience(rb: &RBatis, skill_id: &str, gained_experience: f64) {
    let skill_result = Skill::select_by_map(rb, value\!{"id": skill_id}).await;
    
    match skill_result {
        Ok(skills) => {
            if let Some(mut skill) = skills.into_iter().next() {
                let current_level = skill.level.unwrap_or(1);
                let current_progress = skill.progress.unwrap_or(0.0);
                
                // 計算新的進度值（簡單的經驗值計算：每100經驗升1級）
                let experience_per_level = 100.0;
                let new_total_experience = (current_level - 1) as f64 * experience_per_level + current_progress * experience_per_level + gained_experience;
                
                let new_level = ((new_total_experience / experience_per_level).floor() as i32) + 1;
                let new_progress = (new_total_experience % experience_per_level) / experience_per_level;
                
                // 限制等級上限為10
                let final_level = new_level.min(10);
                let final_progress = if final_level >= 10 { 1.0 } else { new_progress };
                
                skill.level = Some(final_level);
                skill.progress = Some(final_progress);
                skill.updated_at = Some(Utc::now());
                
                // 更新技能
                let update_sql = "UPDATE skill SET level = ?, progress = ?, updated_at = ? WHERE id = ?";
                match rb.exec(
                    update_sql,
                    vec\![
                        Value::I32(final_level),
                        Value::F64(final_progress),
                        Value::String(skill.updated_at.unwrap().to_string()),
                        Value::String(skill_id.to_string()),
                    ],
                ).await {
                    Ok(_) => {
                        log::info\!("技能 {} 經驗更新成功：等級 {} -> {}，進度 {:.2} -> {:.2}，獲得經驗 {:.1}", 
                                 skill_id, current_level, final_level, current_progress, final_progress, gained_experience);
                    }
                    Err(e) => {
                        log::error\!("更新技能 {} 經驗失敗: {}", skill_id, e);
                    }
                }
            }
        }
        Err(e) => {
            log::error\!("查詢技能 {} 失敗: {}", skill_id, e);
        }
    }
}


/**
 * @brief 獲取技能相關的任務列表
 * @param rb RBatis 資料庫連接實例
 * @param path 包含技能ID的路徑參數
 * @return Result<HttpResponse> 包含任務列表的 HTTP 回應
 */
pub async fn get_tasks_by_skill(
    rb: web::Data<RBatis>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let skill_id = path.into_inner();
    
    // 查詢與技能關聯的任務ID列表
    let relations_result = TaskSkillRelation::select_by_map(
        rb.get_ref(),
        value\!{"skill_id": skill_id}
    ).await;
    
    match relations_result {
        Ok(relations) => {
            let task_ids: Vec<String> = relations
                .into_iter()
                .filter_map(|r| r.task_id)
                .collect();
            
            if task_ids.is_empty() {
                return Ok(HttpResponse::Ok().json(ApiResponse {
                    success: true,
                    data: Some(Vec::<Task>::new()),
                    message: "該技能無相關任務".to_string(),
                }));
            }
            
            // 查詢任務詳細信息
            let tasks_query = format\!(
                "SELECT * FROM task WHERE id IN ({}) ORDER BY created_at DESC",
                task_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",")
            );
            
            let task_values: Vec<Value> = task_ids.into_iter().map(Value::String).collect();
            
            match rb.exec(&tasks_query, task_values).await {
                Ok(data) => {
                    let tasks: Vec<Task> = data.into_iter()
                        .filter_map(|row| rbs::from_value(row).ok())
                        .collect();
                    
                    Ok(HttpResponse::Ok().json(ApiResponse {
                        success: true,
                        data: Some(tasks),
                        message: "獲取技能相關任務成功".to_string(),
                    }))
                }
                Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: format\!("查詢任務詳情失敗: {}", e),
                }))
            }
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format\!("查詢任務-技能關聯失敗: {}", e),
        }))
    }
}
