use actix_web::{web, HttpResponse, Result};
use rbatis::RBatis;
use uuid::Uuid;
use chrono::Utc;
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

// 任務相關路由
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
        is_parent_task: Some(if req.task_type.as_ref().map_or(false, |t| t != "daily") { 1 } else { 0 }), // 非每日任務默認為大任務
        task_order: Some(0),
        due_date: req.due_date,
        created_at: Some(now),
        updated_at: Some(now),
    };

    match Task::insert(rb.get_ref(), &new_task).await {
        Ok(_) => Ok(HttpResponse::Created().json(ApiResponse {
            success: true,
            data: Some(new_task),
            message: "任務建立成功".to_string(),
        })),
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
                    Ok(_) => Ok(HttpResponse::Ok().json(ApiResponse {
                        success: true,
                        data: Some(task),
                        message: "任務更新成功".to_string(),
                    })),
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
fn get_subtask_templates(task_title: &str) -> Vec<SubTaskTemplate> {
    match task_title {
        "學習 Vue.js" | "學會一門新程式語言" => vec![
            SubTaskTemplate {
                title: "安裝開發環境".to_string(),
                description: Some("設定開發工具和環境".to_string()),
                difficulty: 1,
                experience: 20,
                order: 1,
            },
            SubTaskTemplate {
                title: "學習基礎語法".to_string(),
                description: Some("掌握基本語法概念".to_string()),
                difficulty: 2,
                experience: 30,
                order: 2,
            },
            SubTaskTemplate {
                title: "實作練習項目".to_string(),
                description: Some("通過實作加深理解".to_string()),
                difficulty: 3,
                experience: 50,
                order: 3,
            },
            SubTaskTemplate {
                title: "完成綜合項目".to_string(),
                description: Some("完成一個完整的項目".to_string()),
                difficulty: 4,
                experience: 80,
                order: 4,
            },
        ],
        "完成一個馬拉松" => vec![
            SubTaskTemplate {
                title: "制定訓練計劃".to_string(),
                description: Some("設計個人化的訓練方案".to_string()),
                difficulty: 2,
                experience: 30,
                order: 1,
            },
            SubTaskTemplate {
                title: "基礎體能訓練".to_string(),
                description: Some("建立基本的跑步體能".to_string()),
                difficulty: 3,
                experience: 40,
                order: 2,
            },
            SubTaskTemplate {
                title: "增加跑步距離".to_string(),
                description: Some("逐步增加跑步距離".to_string()),
                difficulty: 3,
                experience: 50,
                order: 3,
            },
            SubTaskTemplate {
                title: "參加半程馬拉松".to_string(),
                description: Some("完成21公里的挑戰".to_string()),
                difficulty: 4,
                experience: 100,
                order: 4,
            },
            SubTaskTemplate {
                title: "完成全程馬拉松".to_string(),
                description: Some("完成42公里的終極挑戰".to_string()),
                difficulty: 5,
                experience: 200,
                order: 5,
            },
        ],
        "展開新的職場生涯" => vec![
            SubTaskTemplate {
                title: "評估現有技能".to_string(),
                description: Some("分析當前技能和經驗".to_string()),
                difficulty: 2,
                experience: 25,
                order: 1,
            },
            SubTaskTemplate {
                title: "制定學習計劃".to_string(),
                description: Some("設計技能提升路徑".to_string()),
                difficulty: 2,
                experience: 30,
                order: 2,
            },
            SubTaskTemplate {
                title: "更新履歷和作品集".to_string(),
                description: Some("完善個人資料和作品".to_string()),
                difficulty: 3,
                experience: 40,
                order: 3,
            },
            SubTaskTemplate {
                title: "網絡建立和求職".to_string(),
                description: Some("建立職場網絡並尋找機會".to_string()),
                difficulty: 4,
                experience: 60,
                order: 4,
            },
        ],
        _ => vec![], // 沒有預定義模板的任務
    }
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
                
                // 生成子任務
                if req.generate_subtasks.unwrap_or(true) {
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
    
    // 更新父任務為取消狀態
    let update_parent_sql = "UPDATE task SET status = 3, updated_at = ? WHERE id = ?";
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
        data: Some(serde_json::json!({"task_id": task_id})),
        message: "任務取消成功，相關子任務已刪除".to_string(),
    }))
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