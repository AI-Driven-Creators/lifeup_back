use actix_web::{web, HttpResponse, Result};
use rbatis::RBatis;
use uuid::Uuid;
use chrono::Utc;
use rbs::to_value;

use crate::models::*;

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
    match User::select_by_map(rb.get_ref(), to_value!({"id": user_id})).await {
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
        priority: req.priority,
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
        user_id: Some("default_user".to_string()), // 暫時使用預設使用者
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
        user_id: Some("default_user".to_string()),
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
        user_id: Some("default_user".to_string()),
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