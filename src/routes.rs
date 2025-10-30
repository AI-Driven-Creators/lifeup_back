use actix_web::{web, HttpResponse, Result};
use rbatis::RBatis;
use uuid::Uuid;
use chrono::{Utc, Datelike};
use crate::models::*;
use crate::ai_service::convert_to_achievement_model;
use rbs::{Value, value};
use bcrypt::{hash, verify, DEFAULT_COST};
use serde_json::json;
use rand;
// API 回應結構
#[derive(serde::Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    message: String,
}

#[derive(serde::Serialize)]
struct TaskProgressResponse {
    task_id: String,
    total_days: i32,
    completed_days: i32,
    missed_days: i32,
    completion_rate: f64,
    target_rate: f64,
    is_daily_completed: bool,
    remaining_days: i32,
}

#[derive(serde::Serialize)]
struct AchievementWithStats {
    id: String,
    name: String,
    description: Option<String>,
    icon: Option<String>,
    category: Option<String>,
    requirement_type: Option<String>,
    requirement_value: Option<i32>,
    experience_reward: Option<i32>,
    completion_count: i32,
    total_users: i32,
    completion_rate: f64,
    created_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(serde::Deserialize)]
pub struct CreateRecurringTaskRequest {
    pub user_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub task_type: Option<String>,
    pub difficulty: Option<i32>,
    pub experience: Option<i32>,
    pub start_date: Option<chrono::DateTime<chrono::Utc>>,
    pub end_date: Option<chrono::DateTime<chrono::Utc>>,
    pub recurrence_pattern: String,
    pub completion_target: Option<f64>,
    pub subtask_templates: Vec<SubTaskTemplate>,
    pub skill_tags: Option<Vec<String>>,
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
    // 正規化 email（去除空格並轉小寫）
    let normalized_email = req.email.trim().to_lowercase();
    log::info!("註冊請求: name={}, email={}", req.name, normalized_email);

    // 檢查email是否已被註冊
    match User::select_by_map(rb.get_ref(), value!{"email": normalized_email.clone()}).await {
        Ok(existing_users) => {
            if !existing_users.is_empty() {
                log::info!("註冊失敗：email 已存在 -> {}", normalized_email);
                return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: "該email已被註冊".to_string(),
                }));
            }
        }
        Err(e) => {
            log::error!("檢查 email 是否存在時發生錯誤: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("檢查email失敗: {}", e),
            }));
        }
    }

    // 哈希密碼
    let password_hash = match hash(&req.password, DEFAULT_COST) {
        Ok(hash) => hash,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("密碼處理失敗: {}", e),
            }));
        }
    };

    let now = Utc::now();
    let new_user = User {
        id: Some(Uuid::new_v4().to_string()),
        name: Some(req.name.clone()),
        email: Some(normalized_email),
        password_hash: Some(password_hash),
        created_at: Some(now),
        updated_at: Some(now),
    };

    match User::insert(rb.get_ref(), &new_user).await {
        Ok(_) => Ok(HttpResponse::Created().json(ApiResponse {
            success: true,
            data: Some(new_user),
            message: "使用者建立成功".to_string(),
        })),
        Err(e) => {
            // 若觸發唯一索引違反，轉換為 400 回應
            let err_str = e.to_string();
            if err_str.contains("UNIQUE") || err_str.contains("unique") {
                log::info!("註冊失敗（唯一索引）：{}", err_str);
                return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: "該email已被註冊".to_string(),
                }));
            }
            log::error!("使用者建立失敗: {}", err_str);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
                message: format!("使用者建立失敗: {}", err_str),
            }))
        }
    }
}

// 登入路由
pub async fn login(
    rb: web::Data<RBatis>,
    req: web::Json<LoginRequest>,
) -> Result<HttpResponse> {
    // 根據email查找用戶
    let normalized_email = req.email.trim().to_lowercase();
    log::info!("登入請求: email={}", normalized_email);
    match User::select_by_map(rb.get_ref(), value!{"email": normalized_email.clone()}).await {
        Ok(users) => {
            if let Some(user) = users.first() {
                // 驗證密碼
                if let Some(password_hash) = &user.password_hash {
                    match verify(&req.password, password_hash) {
                        Ok(true) => {
                            // 登入成功，返回用戶信息（不包含密碼哈希）
                            let mut user_response = user.clone();
                            user_response.password_hash = None; // 不返回密碼哈希

                            Ok(HttpResponse::Ok().json(ApiResponse {
                                success: true,
                                data: Some(LoginResponse {
                                    user: user_response,
                                    message: "登入成功".to_string(),
                                }),
                                message: "登入成功".to_string(),
                            }))
                        }
                        Ok(false) => Ok(HttpResponse::Unauthorized().json(ApiResponse::<()> {
                            success: false,
                            data: None,
                            message: "密碼錯誤".to_string(),
                        })),
                        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                            success: false,
                            data: None,
                            message: format!("密碼驗證失敗: {}", e),
                        })),
                    }
                } else {
                    Ok(HttpResponse::Unauthorized().json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        message: "用戶密碼未設定".to_string(),
                    }))
                }
            } else {
                Ok(HttpResponse::Unauthorized().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: "用戶不存在".to_string(),
                }))
            }
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("用戶查找失敗: {}", e),
        })),
    }
}

// 登出路由（目前簡單實現，未來可以配合session使用）
pub async fn logout() -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().json(ApiResponse::<()> {
        success: true,
        data: None,
        message: "登出成功".to_string(),
    }))
}

// 任務相關路由 - 只返回父任務（非子任務）
pub async fn get_tasks(
    rb: web::Data<RBatis>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse> {
    // 獲取用戶ID參數
    let user_id = match query.get("user_id") {
        Some(id) => id,
        None => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: "缺少user_id參數".to_string(),
            }));
        }
    };

    // 只獲取指定用戶的父任務：parent_task_id 為 NULL 且 user_id 匹配
    let sql = "SELECT * FROM task WHERE parent_task_id IS NULL AND user_id = ? ORDER BY created_at DESC";

    match rb.query_decode::<Vec<crate::models::Task>>(sql, vec![rbs::Value::String(user_id.clone())]).await {
        Ok(tasks) => Ok(HttpResponse::Ok().json(ApiResponse {
            success: true,
            data: Some(tasks),
            message: "獲取父任務列表成功".to_string(),
        })),
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("獲取父任務列表失敗: {}", e),
        })),
    }
}

pub async fn create_task(
    rb: web::Data<RBatis>,
    req: web::Json<crate::models::CreateTaskRequest>,
) -> Result<HttpResponse> {
    // 驗證 user_id 是否存在
    let user_id = match &req.user_id {
        Some(id) => id.clone(),
        None => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: "缺少user_id參數".to_string(),
            }));
        }
    };

    let now = Utc::now();
    let new_task = crate::models::Task {
        id: Some(Uuid::new_v4().to_string()),
        user_id: Some(user_id), // 使用驗證過的 user_id
        title: Some(req.title.clone()),
        description: req.description.clone(),
        status: Some(0), // 待完成
        priority: req.priority.or(Some(1)),
        task_type: req.task_type.clone().or(Some("daily".to_string())),
        difficulty: req.difficulty.or(Some(1)),
        experience: {
            if req.experience.is_some() {
                req.experience
            } else {
                // 除了每日任務之外，其他任務類型的父任務初始經驗值都為0
                let task_type = req.task_type.as_deref().unwrap_or("daily");
                if task_type == "daily" {
                    Some(10) // 每日任務使用預設值
                } else {
                    Some(0) // 其他任務類型（main/side/challenge）初始為0
                }
            }
        },
        parent_task_id: req.parent_task_id.clone(),
        is_parent_task: Some(if req.parent_task_id.is_some() { 0 } else if req.task_type.as_ref().map_or(false, |t| t == "main" || t == "side" || t == "challenge") { 1 } else { 0 }), // 有父任務的是子任務(0)，否則按類型判斷
        task_order: req.task_order.or(Some(0)),
        due_date: req.due_date,
        created_at: Some(now),
        updated_at: Some(now),
        // 重複性任務相關欄位
        is_recurring: req.is_recurring.or(Some(0)),
        recurrence_pattern: req.recurrence_pattern.clone(),
        start_date: req.start_date,
        end_date: req.end_date,
        completion_target: req.completion_target,
        completion_rate: if req.is_recurring == Some(1) { Some(0.0) } else { None },
        task_date: req.task_date.clone(),
        cancel_count: Some(0),
        last_cancelled_at: None,
        skill_tags: req.skill_tags.clone(),
        career_mainline_id: None,
        task_category: None,
        attributes: req.attributes.clone(),
    };

    match crate::models::Task::insert(rb.get_ref(), &new_task).await {
        Ok(_) => {
            // 如果這是子任務，需要更新父任務的經驗值
            if let Some(parent_task_id) = &new_task.parent_task_id {
                if let Err(e) = update_parent_task_experience(rb.get_ref(), parent_task_id).await {
                    log::warn!("更新父任務經驗值時發生錯誤: {}", e);
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
pub async fn get_skills(
    rb: web::Data<RBatis>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse> {
    // 獲取用戶ID參數
    let user_id = match query.get("user_id") {
        Some(id) => id,
        None => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: "缺少user_id參數".to_string(),
            }));
        }
    };

    match Skill::select_by_map(rb.get_ref(), value!{"user_id": user_id.clone()}).await {
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
    // 驗證 user_id 是否存在
    let user_id = match &req.user_id {
        Some(id) => id.clone(),
        None => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: "缺少user_id參數".to_string(),
            }));
        }
    };

    let now = Utc::now();
    let new_skill = crate::models::Skill {
        id: Some(Uuid::new_v4().to_string()),
        user_id: Some(user_id), // 使用驗證過的 user_id
        name: Some(req.name.clone()),
        description: req.description.clone(),
        category: req.category.clone(),
        attribute: req.attribute.clone(),
        level: req.level,
        experience: req.experience,
        max_experience: req.max_experience,
        icon: req.icon.clone(),
        created_at: Some(now),
        updated_at: Some(now),
    };

    match crate::models::Skill::insert(rb.get_ref(), &new_skill).await {
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

// 更新技能經驗值
pub async fn update_skill_experience(
    rb: web::Data<RBatis>,
    path: web::Path<String>,
    req: web::Json<crate::models::UpdateSkillExperienceRequest>,
) -> Result<HttpResponse> {
    let skill_id = path.into_inner();
    
    // 查詢技能
    match crate::models::Skill::select_by_map(rb.get_ref(), value!{"id": skill_id.clone()}).await {
        Ok(skills) => {
            if let Some(mut skill) = skills.into_iter().next() {
                // 增加經驗值
                let current_exp = skill.experience.unwrap_or(0);
                let new_exp = current_exp + req.experience_gain;
                skill.experience = Some(new_exp);
                
                // 檢查升級
                let current_level = skill.level.unwrap_or(1);
                let max_exp = skill.max_experience.unwrap_or(100);
                let mut final_exp = new_exp;
                let mut final_level = current_level;
                
                // 升級邏輯：如果經驗值超過最大值且等級未達上限
                while final_exp >= max_exp && final_level < 5 {
                    final_exp -= max_exp;
                    final_level += 1;
                    // 每升一級，下一級所需經驗值增加
                    let new_max_exp = final_level * 200 + 100;
                    skill.max_experience = Some(new_max_exp);
                }
                
                skill.experience = Some(final_exp);
                skill.level = Some(final_level);
                skill.updated_at = Some(Utc::now());
                
                // 更新資料庫
                match crate::models::Skill::update_by_map(
                    rb.get_ref(),
                    &skill,
                    value!{"id": skill_id}
                ).await {
                    Ok(_) => {
                        let level_up = final_level > current_level;
                        let response_message = if level_up {
                            format!("技能經驗值更新成功！恭喜升級到 {} 級！", final_level)
                        } else {
                            "技能經驗值更新成功".to_string()
                        };
                        
                        Ok(HttpResponse::Ok().json(ApiResponse {
                            success: true,
                            data: Some(json!({
                                "skill": skill,
                                "experience_gained": req.experience_gain,
                                "level_up": level_up,
                                "previous_level": current_level,
                                "new_level": final_level,
                                "reason": req.reason.clone().unwrap_or_default()
                            })),
                            message: response_message,
                        }))
                    },
                    Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        message: format!("更新技能經驗值失敗: {}", e),
                    }))
                }
            } else {
                Ok(HttpResponse::NotFound().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: "找不到該技能".to_string(),
                }))
            }
        },
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("查詢技能失敗: {}", e),
        }))
    }
}

// 更新使用者經驗值
pub async fn update_user_experience(
    rb: web::Data<RBatis>,
    path: web::Path<String>,
    req: web::Json<crate::models::UpdateUserExperienceRequest>,
) -> Result<HttpResponse> {
    let user_id = path.into_inner();

    // 查詢使用者資料
    match crate::models::UserProfile::select_by_map(rb.get_ref(), value!{"user_id": user_id.clone()}).await {
        Ok(profiles) => {
            if let Some(mut profile) = profiles.into_iter().next() {
                // 增加經驗值
                let current_exp = profile.experience.unwrap_or(0);
                let new_exp = current_exp + req.experience_gain;

                // 檢查升級或降級
                let current_level = profile.level.unwrap_or(1);
                let max_exp = profile.max_experience.unwrap_or(100);
                let mut final_exp = new_exp;
                let mut final_level = current_level;
                let mut new_max_exp = max_exp;

                // 升級邏輯：經驗值超過最大值時升級
                while final_exp >= new_max_exp && final_level > 0 {
                    final_exp -= new_max_exp;
                    final_level += 1;
                    // 每升一級，下一級所需經驗值增加 10%
                    new_max_exp = (new_max_exp as f64 * 1.1) as i32;
                }

                // 降級邏輯：經驗值為負數時降級
                while final_exp < 0 && final_level > 1 {
                    final_level -= 1;
                    // 計算上一級的最大經驗值（反向計算）
                    new_max_exp = (new_max_exp as f64 / 1.1) as i32;
                    final_exp += new_max_exp;
                }

                // 如果等級已經是1且經驗值仍為負，將經驗值設為0
                if final_level <= 1 && final_exp < 0 {
                    final_level = 1;
                    final_exp = 0;
                    new_max_exp = 100; // 重置為初始最大經驗值
                }

                profile.experience = Some(final_exp);
                profile.level = Some(final_level);
                profile.max_experience = Some(new_max_exp);
                profile.updated_at = Some(Utc::now());

                // 更新資料庫
                match crate::models::UserProfile::update_by_map(
                    rb.get_ref(),
                    &profile,
                    value!{"user_id": user_id}
                ).await {
                    Ok(_) => {
                        let level_up = final_level > current_level;
                        let level_down = final_level < current_level;
                        let response_message = if level_up {
                            format!("經驗值更新成功！恭喜升級到 {} 級！", final_level)
                        } else if level_down {
                            format!("經驗值更新成功！降級到 {} 級", final_level)
                        } else {
                            "經驗值更新成功".to_string()
                        };

                        Ok(HttpResponse::Ok().json(ApiResponse {
                            success: true,
                            data: Some(json!({
                                "profile": profile,
                                "experience_gained": req.experience_gain,
                                "level_up": level_up,
                                "level_down": level_down,
                                "previous_level": current_level,
                                "new_level": final_level
                            })),
                            message: response_message,
                        }))
                    },
                    Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        message: format!("更新使用者經驗值失敗: {}", e),
                    }))
                }
            } else {
                Ok(HttpResponse::NotFound().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: "找不到該使用者資料".to_string(),
                }))
            }
        },
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("查詢使用者資料失敗: {}", e),
        }))
    }
}

// 更新使用者屬性 API
#[derive(serde::Deserialize)]
pub struct UpdateUserAttributesRequest {
    pub attributes: std::collections::HashMap<String, i32>,  // {"intelligence": 5, "creativity": -3}
}

pub async fn update_user_attributes(
    rb: web::Data<RBatis>,
    path: web::Path<String>,
    req: web::Json<UpdateUserAttributesRequest>,
) -> Result<HttpResponse> {
    let user_id = path.into_inner();

    // 查詢或創建使用者屬性記錄
    match crate::models::UserAttributes::select_by_map(rb.get_ref(), value!{"user_id": user_id.clone()}).await {
        Ok(mut attributes_list) => {
            let mut user_attrs = if let Some(attrs) = attributes_list.pop() {
                attrs
            } else {
                // 創建新的屬性記錄，初始值為 50
                crate::models::UserAttributes {
                    id: Some(Uuid::new_v4().to_string()),
                    user_id: Some(user_id.clone()),
                    intelligence: Some(50),
                    endurance: Some(50),
                    creativity: Some(50),
                    social: Some(50),
                    focus: Some(50),
                    adaptability: Some(50),
                    created_at: Some(Utc::now()),
                    updated_at: Some(Utc::now()),
                }
            };

            // 更新屬性值（支援增加或減少）
            let mut updated_attrs: std::collections::HashMap<String, (i32, i32)> = std::collections::HashMap::new();

            for (attr_name, change) in &req.attributes {
                match attr_name.as_str() {
                    "intelligence" => {
                        let old_val = user_attrs.intelligence.unwrap_or(50);
                        let new_val = (old_val + change).max(0).min(100);  // 限制在 0-100 之間
                        user_attrs.intelligence = Some(new_val);
                        updated_attrs.insert("intelligence".to_string(), (old_val, new_val));
                    },
                    "endurance" => {
                        let old_val = user_attrs.endurance.unwrap_or(50);
                        let new_val = (old_val + change).max(0).min(100);
                        user_attrs.endurance = Some(new_val);
                        updated_attrs.insert("endurance".to_string(), (old_val, new_val));
                    },
                    "creativity" => {
                        let old_val = user_attrs.creativity.unwrap_or(50);
                        let new_val = (old_val + change).max(0).min(100);
                        user_attrs.creativity = Some(new_val);
                        updated_attrs.insert("creativity".to_string(), (old_val, new_val));
                    },
                    "social" => {
                        let old_val = user_attrs.social.unwrap_or(50);
                        let new_val = (old_val + change).max(0).min(100);
                        user_attrs.social = Some(new_val);
                        updated_attrs.insert("social".to_string(), (old_val, new_val));
                    },
                    "focus" => {
                        let old_val = user_attrs.focus.unwrap_or(50);
                        let new_val = (old_val + change).max(0).min(100);
                        user_attrs.focus = Some(new_val);
                        updated_attrs.insert("focus".to_string(), (old_val, new_val));
                    },
                    "adaptability" => {
                        let old_val = user_attrs.adaptability.unwrap_or(50);
                        let new_val = (old_val + change).max(0).min(100);
                        user_attrs.adaptability = Some(new_val);
                        updated_attrs.insert("adaptability".to_string(), (old_val, new_val));
                    },
                    _ => {
                        log::warn!("未知的屬性名稱: {}", attr_name);
                    }
                }
            }

            user_attrs.updated_at = Some(Utc::now());

            // 更新或插入資料庫
            let db_result = if user_attrs.id.is_some() {
                crate::models::UserAttributes::update_by_map(
                    rb.get_ref(),
                    &user_attrs,
                    value!{"user_id": user_id.clone()}
                ).await
            } else {
                crate::models::UserAttributes::insert(rb.get_ref(), &user_attrs).await
            };

            match db_result {
                Ok(_) => {
                    log::info!("使用者 {} 屬性更新成功: {:?}", user_id, updated_attrs);

                    Ok(HttpResponse::Ok().json(ApiResponse {
                        success: true,
                        data: Some(json!({
                            "attributes": user_attrs,
                            "changes": updated_attrs
                        })),
                        message: "屬性更新成功".to_string(),
                    }))
                },
                Err(e) => {
                    log::error!("更新使用者屬性失敗: {}", e);
                    Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        message: format!("更新使用者屬性失敗: {}", e),
                    }))
                }
            }
        },
        Err(e) => {
            log::error!("查詢使用者屬性失敗: {}", e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("查詢使用者屬性失敗: {}", e),
            }))
        }
    }
}

// 聊天相關路由
pub async fn get_chat_messages(
    rb: web::Data<RBatis>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse> {
    let user_id = query.get("user_id").map(|s| s.as_str());

    let (sql, params): (String, Vec<rbs::Value>) = if let Some(uid) = user_id {
        (
            r#"
                SELECT * FROM chat_message
                WHERE user_id = ?
                ORDER BY created_at DESC, role DESC
                LIMIT 30
            "#.to_string(),
            vec![rbs::Value::String(uid.to_string())]
        )
    } else {
        log::warn!("獲取聊天記錄時未提供 user_id，返回空結果");
        // 不提供 user_id 時返回空結果，避免洩露其他用戶的對話
        return Ok(HttpResponse::Ok().json(ApiResponse {
            success: true,
            data: Some(Vec::<crate::models::ChatMessage>::new()),
            message: "請提供 user_id 參數".to_string(),
        }));
    };

    match rb.query_decode::<Vec<crate::models::ChatMessage>>(&sql, params).await {
        Ok(mut messages) => {
            // 反轉順序，讓最早的消息在前
            messages.reverse();
            Ok(HttpResponse::Ok().json(ApiResponse {
                success: true,
                data: Some(messages),
                message: "獲取聊天記錄成功".to_string(),
            }))
        },
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("獲取聊天記錄失敗: {}", e),
        })),
    }
}

// 獲取所有聊天記錄（用於下載）
pub async fn get_all_chat_messages(
    rb: web::Data<RBatis>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse> {
    let user_id = query.get("user_id").map(|s| s.as_str());

    let messages = if let Some(uid) = user_id {
        // 只獲取指定用戶的聊天記錄
        let sql = "SELECT * FROM chat_message WHERE user_id = ? ORDER BY created_at ASC";
        match rb.query_decode::<Vec<crate::models::ChatMessage>>(sql, vec![rbs::Value::String(uid.to_string())]).await {
            Ok(msgs) => msgs,
            Err(e) => {
                return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: format!("獲取聊天記錄失敗: {}", e),
                }));
            }
        }
    } else {
        log::warn!("下載聊天記錄時未提供 user_id，返回空結果");
        Vec::new()
    };

    // UTF-8 BOM (Byte Order Mark) 用於確保正確編碼，特別是在 Windows 和手機上
    let utf8_bom = "\u{FEFF}";
    let mut text_content = String::from(utf8_bom);
    text_content.push_str("=== AI 教練對話記錄 ===\n\n");

    for msg in messages {
        let role = msg.role.unwrap_or_else(|| "unknown".to_string());
        let content = msg.content.unwrap_or_else(|| "".to_string());
        let time = msg.created_at
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "未知時間".to_string());

        let role_display = if role == "user" { "用戶" } else { "AI教練" };
        text_content.push_str(&format!("[{}] {} - {}\n{}\n\n", time, role_display, role, content));
    }

    // 返回文本檔案，添加 UTF-8 BOM 確保編碼正確
    Ok(HttpResponse::Ok()
        .content_type("text/plain; charset=utf-8")
        .insert_header(("Content-Disposition", "attachment; filename=\"chat_history.txt\""))
        .body(text_content))
}

pub async fn send_message(
    rb: web::Data<RBatis>,
    req: web::Json<ChatRequest>,
) -> Result<HttpResponse> {
    let now = Utc::now();

    // 儲存使用者訊息
    let user_message = crate::models::ChatMessage {
        id: Some(Uuid::new_v4().to_string()),
        user_id: Some(req.user_id.clone()),
        role: Some("user".to_string()),
        content: Some(req.message.clone()),
        created_at: Some(now),
    };

    if let Err(e) = crate::models::ChatMessage::insert(rb.get_ref(), &user_message).await {
        return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("儲存使用者訊息失敗: {}", e),
        }));
    }

    // 模擬 AI 回覆
    let ai_response = format!("收到您的訊息：{}。我是您的 AI 教練，有什麼可以幫助您的嗎？", req.message);

    let assistant_message = crate::models::ChatMessage {
        id: Some(Uuid::new_v4().to_string()),
        user_id: Some(req.user_id.clone()),
        role: Some("assistant".to_string()),
        content: Some(ai_response.clone()),
        created_at: Some(now),
    };

    match crate::models::ChatMessage::insert(rb.get_ref(), &assistant_message).await {
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

// 保存單條聊天訊息（用於系統訊息、專家訊息等）
#[derive(serde::Deserialize)]
pub struct SaveMessageRequest {
    pub user_id: String,
    pub role: String,       // "user", "assistant", "coach", "system"
    pub content: String,
}

pub async fn save_chat_message(
    rb: web::Data<RBatis>,
    req: web::Json<SaveMessageRequest>,
) -> Result<HttpResponse> {
    log::info!("收到保存聊天訊息請求: role={}, user_id={}", req.role, req.user_id);

    let now = Utc::now();
    let chat_message = crate::models::ChatMessage {
        id: Some(Uuid::new_v4().to_string()),
        user_id: Some(req.user_id.clone()),
        role: Some(req.role.clone()),
        content: Some(req.content.clone()),
        created_at: Some(now),
    };

    match crate::models::ChatMessage::insert(rb.get_ref(), &chat_message).await {
        Ok(_) => {
            log::info!("成功保存聊天訊息");
            Ok(HttpResponse::Ok().json(ApiResponse {
                success: true,
                data: Some(chat_message),
                message: "訊息保存成功".to_string(),
            }))
        },
        Err(e) => {
            log::error!("保存聊天訊息失敗: {}", e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("保存訊息失敗: {}", e),
            }))
        }
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
    match crate::models::Task::select_by_map(rb.get_ref(), value!{"id": task_id.clone()}).await {
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
                if let Some(task_order) = req.task_order {
                    task.task_order = Some(task_order);
                }
                task.updated_at = Some(Utc::now());
                
                // 執行更新
                let update_sql = "UPDATE task SET title = ?, description = ?, status = ?, priority = ?, task_type = ?, difficulty = ?, experience = ?, due_date = ?, task_order = ?, updated_at = ? WHERE id = ?";
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
                        Value::I32(task.task_order.unwrap_or(0)),
                        Value::String(task.updated_at.unwrap().to_string()),
                        Value::String(task_id.clone()),
                    ],
                ).await;
                
                match result {
                    Ok(_) => {
                        // 如果這是子任務，任何變化都要檢查和更新父任務
                        if let Some(parent_task_id) = &task.parent_task_id {
                            // 更新父任務狀態
                            if let Err(e) = check_and_update_parent_task_status(rb.get_ref(), parent_task_id).await {
                                log::warn!("檢查父任務狀態時發生錯誤: {}", e);
                            }
                            // 更新父任務經驗值
                            if let Err(e) = update_parent_task_experience(rb.get_ref(), parent_task_id).await {
                                log::warn!("更新父任務經驗值時發生錯誤: {}", e);
                            }
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

// 刪除任務
pub async fn delete_task(
    rb: web::Data<RBatis>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let task_id = path.into_inner();

    log::info!("刪除任務: {}", task_id);

    // 先查詢任務是否存在
    match crate::models::Task::select_by_map(rb.get_ref(), value!{"id": task_id.clone()}).await {
        Ok(tasks) => {
            if let Some(task) = tasks.into_iter().next() {
                // 檢查是否為父任務
                if task.is_parent_task.unwrap_or(0) == 1 {
                    // 如果是父任務，先刪除所有子任務
                    match crate::models::Task::select_by_map(rb.get_ref(), value!{"parent_task_id": task_id.clone()}).await {
                        Ok(subtasks) => {
                            let subtasks_count = subtasks.len();
                            for subtask in &subtasks {
                                if let Some(subtask_id) = &subtask.id {
                                    if let Err(e) = crate::models::Task::delete_by_map(rb.get_ref(), value!{"id": subtask_id.clone()}).await {
                                        log::error!("刪除子任務 {} 失敗: {}", subtask_id, e);
                                        return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                                            success: false,
                                            data: None,
                                            message: format!("刪除子任務失敗: {}", e),
                                        }));
                                    }
                                }
                            }
                            log::info!("成功刪除 {} 個子任務", subtasks_count);
                        }
                        Err(e) => {
                            log::error!("查詢子任務失敗: {}", e);
                            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                                success: false,
                                data: None,
                                message: format!("查詢子任務失敗: {}", e),
                            }));
                        }
                    }
                }

                // 記住父任務ID用於稍後更新經驗值
                let parent_task_id = task.parent_task_id.clone();

                // 刪除任務本身
                match crate::models::Task::delete_by_map(rb.get_ref(), value!{"id": task_id}).await {
                    Ok(_) => {
                        // 如果這是子任務，需要更新父任務的經驗值
                        if let Some(parent_id) = &parent_task_id {
                            if let Err(e) = update_parent_task_experience(rb.get_ref(), parent_id).await {
                                log::warn!("更新父任務經驗值時發生錯誤: {}", e);
                            }
                        }

                        log::info!("任務 {} 刪除成功", task.title.unwrap_or_default());
                        Ok(HttpResponse::Ok().json(ApiResponse {
                            success: true,
                            data: Some(serde_json::json!({
                                "deleted_task_id": task.id,
                                "message": "任務刪除成功"
                            })),
                            message: "任務刪除成功".to_string(),
                        }))
                    }
                    Err(e) => {
                        log::error!("刪除任務失敗: {}", e);
                        Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                            success: false,
                            data: None,
                            message: format!("刪除任務失敗: {}", e),
                        }))
                    }
                }
            } else {
                Ok(HttpResponse::NotFound().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: "任務不存在".to_string(),
                }))
            }
        }
        Err(e) => {
            log::error!("查詢任務失敗: {}", e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("查詢任務失敗: {}", e),
            }))
        }
    }
}

// 根據ID獲取單個任務
pub async fn get_task(rb: web::Data<RBatis>, path: web::Path<String>) -> Result<HttpResponse> {
    let task_id = path.into_inner();
    match crate::models::Task::select_by_map(rb.get_ref(), value!{"id": task_id.clone()}).await {
        Ok(tasks) => {
            if let Some(task) = tasks.first() {
                Ok(HttpResponse::Ok().json(ApiResponse {
                    success: true,
                    data: Some(task.clone()),
                    message: "獲取任務成功".to_string(),
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
            message: format!("獲取任務失敗: {}", e),
        })),
    }
}

// 根據任務類型獲取任務 - 只返回父任務（非子任務）
pub async fn get_tasks_by_type(
    rb: web::Data<RBatis>,
    path: web::Path<String>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse> {
    let task_type = path.into_inner();
    log::info!("獲取任務類型: {}", task_type);

    // 獲取用戶ID參數
    let user_id = match query.get("user_id") {
        Some(id) => id,
        None => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: "缺少user_id參數".to_string(),
            }));
        }
    };

    // 只獲取指定用戶和類型的父任務：parent_task_id 為 NULL 且 task_type 匹配且 user_id 匹配
    let sql = "SELECT * FROM task WHERE task_type = ? AND parent_task_id IS NULL AND user_id = ? ORDER BY created_at DESC";

    match rb.query_decode::<Vec<crate::models::Task>>(sql, vec![rbs::Value::String(task_type.clone()), rbs::Value::String(user_id.clone())]).await {
        Ok(tasks) => {
            log::info!("成功獲取{}個{}類型任務", tasks.len(), task_type);
            
            // 嘗試手動序列化以找出問題
            match serde_json::to_string(&tasks) {
                Ok(_) => {
                    log::info!("任務數據序列化成功");
                    Ok(HttpResponse::Ok().json(ApiResponse {
                        success: true,
                        data: Some(tasks),
                        message: format!("獲取{}任務列表成功", task_type),
                    }))
                },
                Err(serialize_error) => {
                    log::error!("任務數據序列化失敗: {}", serialize_error);
                    Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        message: format!("任務數據序列化失敗: {}", serialize_error),
                    }))
                }
            }
        },
        Err(e) => {
            log::error!("獲取{}任務列表失敗: {}", task_type, e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("獲取{}任務列表失敗: {}", task_type, e),
            }))
        }
    }
}

// 根據技能名稱獲取相關任務
pub async fn get_tasks_by_skill(
    rb: web::Data<RBatis>,
    path: web::Path<String>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse> {
    let skill_name = path.into_inner();
    log::info!("獲取技能相關任務: {}", skill_name);

    // 獲取用戶ID參數
    let user_id = match query.get("user_id") {
        Some(id) => id,
        None => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: "缺少user_id參數".to_string(),
            }));
        }
    };

    // 查詢指定用戶的包含指定技能標籤的任務，但排除子任務
    let sql = "SELECT * FROM task WHERE skill_tags LIKE ? AND (task_type != 'subtask' OR task_type IS NULL) AND user_id = ?";
    let skill_pattern = format!("%\"{}\"%", skill_name);
    
    match rb.query_decode::<Vec<Task>>(sql, vec![Value::String(skill_pattern), Value::String(user_id.clone())]).await {
        Ok(tasks) => {
            log::info!("成功獲取{}個「{}」相關任務", tasks.len(), skill_name);
            
            // 將任務狀態轉換為字串格式以供前端使用
            let tasks_with_string_status: Vec<serde_json::Value> = tasks.iter().map(|task| {
                let status_string = match task.status {
                    Some(0) => "pending",
                    Some(1) => "in_progress", 
                    Some(2) => "completed",
                    Some(3) => "cancelled",
                    Some(4) => "paused",
                    Some(5) => "daily_in_progress",
                    Some(6) => "daily_completed", 
                    Some(7) => "daily_not_completed",
                    _ => "pending"
                };
                
                serde_json::json!({
                    "id": task.id,
                    "title": task.title,
                    "description": task.description,
                    "status": status_string,
                    "priority": task.priority,
                    "task_type": task.task_type,
                    "difficulty": task.difficulty,
                    "experience": task.experience,
                    "parent_task_id": task.parent_task_id,
                    "is_parent_task": task.is_parent_task,
                    "task_order": task.task_order,
                    "due_date": task.due_date,
                    "created_at": task.created_at,
                    "updated_at": task.updated_at,
                    "skill_tags": task.skill_tags
                })
            }).collect();
            
            Ok(HttpResponse::Ok().json(ApiResponse {
                success: true,
                data: Some(tasks_with_string_status),
                message: format!("獲取「{}」相關任務成功", skill_name),
            }))
        },
        Err(e) => {
            log::error!("獲取「{}」相關任務失敗: {}", skill_name, e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("獲取「{}」相關任務失敗: {}", skill_name, e),
            }))
        }
    }
}

// 獲取子任務模板
#[derive(Clone, serde::Deserialize, serde::Serialize)]
struct SubTaskTemplate {
    title: String,
    description: Option<String>,
    difficulty: i32,
    experience: i32,
    order: i32,
    skill_tags: Option<Vec<String>>,
}

fn get_subtask_templates(_task_title: &str) -> Vec<SubTaskTemplate> {
    // 返回通用的子任務模板，適用於所有類型的任務
    vec![
        SubTaskTemplate {
            title: "準備階段".to_string(),
            description: Some("收集資源和制定計劃".to_string()),
            difficulty: 1,
            experience: 20,
            order: 1,
            skill_tags: None,
        },
        SubTaskTemplate {
            title: "學習基礎".to_string(),
            description: Some("掌握基本概念和技能".to_string()),
            difficulty: 2,
            experience: 30,
            order: 2,
            skill_tags: None,
        },
        SubTaskTemplate {
            title: "實踐練習".to_string(),
            description: Some("通過實作加深理解".to_string()),
            difficulty: 3,
            experience: 50,
            order: 3,
            skill_tags: None,
        },
        SubTaskTemplate {
            title: "深入學習".to_string(),
            description: Some("掌握進階技能和概念".to_string()),
            difficulty: 4,
            experience: 60,
            order: 4,
            skill_tags: None,
        },
        SubTaskTemplate {
            title: "完成項目".to_string(),
            description: Some("完成實際應用項目".to_string()),
            difficulty: 4,
            experience: 80,
            order: 5,
            skill_tags: None,
        },
        SubTaskTemplate {
            title: "總結回顧".to_string(),
            description: Some("總結經驗並規劃下一步".to_string()),
            difficulty: 2,
            experience: 30,
            order: 6,
            skill_tags: None,
        },
    ]
}

// 開始任務（生成子任務）
#[derive(serde::Deserialize)]
pub struct StartTaskRequest {
    pub generate_subtasks: Option<bool>,
}

pub async fn start_task(
    rb: web::Data<RBatis>,
    path: web::Path<String>,
    req: web::Json<StartTaskRequest>,
) -> Result<HttpResponse> {
    let task_id = path.into_inner();
    
    // 查詢任務
    match crate::models::Task::select_by_map(rb.get_ref(), value!{"id": task_id.clone()}).await {
        Ok(tasks) => {
            if let Some(mut task) = tasks.into_iter().next() {
                let is_parent_task = task.is_parent_task.unwrap_or(0) == 1;
                let is_daily_task = task.task_type.as_deref() == Some("daily");

                // 檢查是否為大任務或每日任務
                if !is_parent_task && !is_daily_task {
                    return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        message: "只有大任務或每日任務可以開始".to_string(),
                    }));
                }

                // 決定新狀態：每日任務使用 daily_in_progress (5)，其他使用 in_progress (1)
                let new_status = if is_daily_task { 5 } else { 1 };

                // 更新任務狀態為進行中
                task.status = Some(new_status);
                task.updated_at = Some(Utc::now());
                
                let update_sql = "UPDATE task SET status = ?, updated_at = ? WHERE id = ?";
                if let Err(e) = rb.exec(
                    update_sql,
                    vec![
                        Value::I32(new_status),
                        Value::String(task.updated_at.unwrap().to_string()),
                        Value::String(task_id.clone()),
                    ],
                ).await {
                    return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        message: format!("更新任務狀態失敗: {}", e),
                    }));
                }

                // 每日任務直接返回成功，不需要生成子任務
                if is_daily_task {
                    return Ok(HttpResponse::Ok().json(ApiResponse {
                        success: true,
                        data: Some(task),
                        message: "每日任務已開始".to_string(),
                    }));
                }

                // 檢查是否需要生成子任務（僅限父任務）
                if is_parent_task && req.generate_subtasks.unwrap_or(false) {
                    // 先查詢現有的子任務
                    match Task::select_by_map(rb.get_ref(), value!{"parent_task_id": task_id.clone()}).await {
                        Ok(existing_subtasks) => {
                            if existing_subtasks.is_empty() {
                                // 沒有現有子任務，生成新的子任務
                                let templates = get_subtask_templates(&task.title.clone().unwrap_or_default());
                                let mut subtasks = Vec::new();

                                for template in templates {
                                    let subtask = crate::models::Task {
                                        id: Some(Uuid::new_v4().to_string()),
                                        user_id: task.user_id.clone(),
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
                                        skill_tags: task.skill_tags.clone(), // 子任務繼承父任務的技能標籤
                                        career_mainline_id: None,
                                        task_category: None,
                                        attributes: None,
                                    };
                                    
                                    if let Err(e) = crate::models::Task::insert(rb.get_ref(), &subtask).await {
                                        log::error!("Failed to create subtask: {}", e);
                                    } else {
                                        subtasks.push(subtask);
                                    }
                                }

                                // 計算所有子任務的經驗值總和並更新父任務
                                let total_experience: i32 = subtasks.iter()
                                    .map(|subtask| subtask.experience.unwrap_or(0))
                                    .sum();

                                // 更新父任務的經驗值
                                let update_parent_exp_sql = "UPDATE task SET experience = ?, updated_at = ? WHERE id = ?";
                                if let Err(e) = rb.exec(
                                    update_parent_exp_sql,
                                    vec![
                                        Value::I32(total_experience),
                                        Value::String(Utc::now().to_string()),
                                        Value::String(task_id.clone()),
                                    ],
                                ).await {
                                    log::error!("更新父任務經驗值失敗: {}", e);
                                    // 繼續執行，不影響主要功能
                                } else {
                                    // 更新內存中的父任務經驗值
                                    task.experience = Some(total_experience);
                                    log::info!("父任務 {} 經驗值已更新為子任務總和: {}", task_id, total_experience);
                                }

                                Ok(HttpResponse::Ok().json(ApiResponse {
                                    success: true,
                                    data: Some(serde_json::json!({
                                        "parent_task": task,
                                        "subtasks": subtasks,
                                        "subtasks_count": subtasks.len(),
                                        "total_experience": total_experience
                                    })),
                                    message: format!("任務開始成功，生成了 {} 個子任務，總經驗值: {}", subtasks.len(), total_experience),
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
                                            // 更新父任務經驗值
                                            if let Err(e) = update_parent_task_experience(rb.get_ref(), &task_id).await {
                                                log::error!("更新父任務經驗值失敗: {}", e);
                                            }

                                            Ok(HttpResponse::Ok().json(ApiResponse {
                                                success: true,
                                                data: Some(serde_json::json!({
                                                    "parent_task": task,
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
                                    // 子任務已存在且不需要恢復，更新父任務經驗值並返回現有子任務
                                    if let Err(e) = update_parent_task_experience(rb.get_ref(), &task_id).await {
                                        log::error!("更新父任務經驗值失敗: {}", e);
                                    }

                                    Ok(HttpResponse::Ok().json(ApiResponse {
                                        success: true,
                                        data: Some(serde_json::json!({
                                            "parent_task": task,
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
                    // 父任務開始但不生成子任務
                    Ok(HttpResponse::Ok().json(ApiResponse {
                        success: true,
                        data: Some(task),
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
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse> {
    let parent_task_id = path.into_inner();
    let query_params = query.into_inner();
    
    // 檢查是否為每日任務查詢（通過查詢參數判斷）
    let is_daily_task = query_params.get("daily").map(|v| v == "true").unwrap_or(false);
    let days_limit = query_params.get("days").and_then(|v| v.parse::<i32>().ok()).unwrap_or(3);
    
    if is_daily_task {
        // 對於每日任務，使用原生SQL查詢最近幾天的數據以避免序列化問題
        let today = Utc::now().date_naive();
        let start_date = today - chrono::Duration::days((days_limit - 1) as i64);
        
        let sql = "SELECT * FROM task WHERE parent_task_id = ? AND task_date >= ? AND task_date <= ? ORDER BY task_date DESC LIMIT 100";
        match rb.query_decode::<Vec<Task>>(sql, vec![
            Value::String(parent_task_id.clone()),
            Value::String(start_date.to_string()),
            Value::String(today.to_string())
        ]).await {
            Ok(all_subtasks) => {
                // SQL 查詢已經過濾了日期，現在只需要調整狀態
                let filtered_subtasks: Vec<Task> = all_subtasks
                    .into_iter()
                    .map(|mut task| {
                        // 對於每日任務，將所有未完成的狀態統一為 daily_not_completed
                        if let Some(status) = task.status {
                            match status {
                                0 | 1 | 4 | 5 => { // pending, in_progress, paused, daily_in_progress
                                    task.status = Some(TaskStatus::DailyNotCompleted.to_i32());
                                },
                                2 | 6 => { // completed, daily_completed
                                    task.status = Some(TaskStatus::DailyCompleted.to_i32());
                                },
                                _ => {} // 其他狀態保持不變
                            }
                        }
                        task
                    })
                    .collect();
                
                Ok(HttpResponse::Ok().json(ApiResponse {
                    success: true,
                    data: Some(filtered_subtasks),
                    message: "獲取每日子任務列表成功".to_string(),
                }))
            },
            Err(e) => {
                log::warn!("每日子任務查詢序列化錯誤，任務ID: {}, 錯誤: {}", parent_task_id, e);
                // 暫時返回空列表以避免序列化問題
                Ok(HttpResponse::Ok().json(ApiResponse {
                    success: true,
                    data: Some(Vec::<Task>::new()),
                    message: "獲取每日子任務列表成功（暫時無子任務）".to_string(),
                }))
            },
        }
    } else {
        // 對於非每日任務（主任務、支線任務等），使用正常的子任務查詢
        let sql = "SELECT * FROM task WHERE parent_task_id = ? ORDER BY task_order ASC";
        match rb.query_decode::<Vec<Task>>(sql, vec![
            Value::String(parent_task_id.clone())
        ]).await {
            Ok(subtasks) => {
                Ok(HttpResponse::Ok().json(ApiResponse {
                    success: true,
                    data: Some(subtasks),
                    message: "獲取子任務列表成功".to_string(),
                }))
            },
            Err(e) => {
                log::error!("查詢子任務失敗，父任務ID: {}, 錯誤: {}", parent_task_id, e);
                Ok(HttpResponse::InternalServerError().json(ApiResponse::<Vec<Task>> {
                    success: false,
                    data: None,
                    message: format!("查詢子任務失敗: {}", e),
                }))
            }
        }
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
    let update_subtasks_sql = format!(
        "UPDATE task SET status = 4, updated_at = ? WHERE parent_task_id = ? AND status != {}",
        crate::models::TaskStatus::DailyCompleted.to_i32()
    );
    if let Err(e) = rb.exec(
        &update_subtasks_sql,
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
                let delete_subtasks_sql = format!(
                    "DELETE FROM task WHERE parent_task_id = ? AND status != {}",
                    crate::models::TaskStatus::DailyCompleted.to_i32()
                );
                if let Err(e) = rb.exec(
                    &delete_subtasks_sql,
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
pub async fn get_homepage_tasks(
    rb: web::Data<RBatis>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse> {
    log::info!("開始獲取首頁任務...");

    // 獲取用戶ID參數
    let user_id = match query.get("user_id") {
        Some(id) => id,
        None => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: "缺少user_id參數".to_string(),
            }));
        }
    };

    // 獲取指定用戶的子任務和每日任務，並關聯父任務標題
    let sql = r#"
        SELECT
            t.id,
            t.user_id,
            t.title,
            t.description,
            t.status,
            t.priority,
            t.task_type,
            t.difficulty,
            t.experience,
            t.parent_task_id,
            t.is_parent_task,
            t.task_order,
            t.due_date,
            t.created_at,
            t.updated_at,
            t.is_recurring,
            t.recurrence_pattern,
            t.start_date,
            t.end_date,
            t.completion_target,
            t.completion_rate,
            t.task_date,
            t.cancel_count,
            t.last_cancelled_at,
            t.skill_tags,
            t.career_mainline_id,
            t.task_category,
            t.attributes,
            p.title as parent_task_title
        FROM task t
        LEFT JOIN task p ON t.parent_task_id = p.id
        WHERE t.parent_task_id IS NOT NULL
            AND t.user_id = ?
            AND (t.task_date >= date('now', '-2 days') OR t.task_date IS NULL)
            AND t.status IN (0, 1, 2, 4, 5, 6, 7)  -- 顯示待處理、進行中、已完成、暫停、每日進行中、每日已完成、每日未完成等狀態
        ORDER BY t.task_date DESC, t.task_order, t.created_at
    "#;
    
    log::debug!("執行SQL查詢: {}", sql);
    
    match rb.query(sql, vec![rbs::Value::String(user_id.clone())]).await {
        Ok(tasks) => {
            let tasks_count = if let rbs::Value::Array(ref arr) = tasks {
                arr.len()
            } else {
                0
            };
            log::info!("成功獲取 {} 個首頁任務", tasks_count);
            
            // 檢查前幾個任務的parent_task_title字段
            if let rbs::Value::Array(ref task_array) = tasks {
                for (i, task) in task_array.iter().take(5).enumerate() {
                    if let rbs::Value::Map(ref task_map) = task {
                        let title_key = rbs::Value::String("title".to_string());
                        let parent_key = rbs::Value::String("parent_task_title".to_string());
                        
                        let title = match task_map.get(&title_key) {
                            rbs::Value::String(s) => s.as_str(),
                            _ => "無標題"
                        };
                        
                        let parent_title = match task_map.get(&parent_key) {
                            rbs::Value::String(s) => s.as_str(),
                            rbs::Value::Null => "無父任務",
                            _ => "未知"
                        };
                        
                        log::info!("任務 {}: {} -> 父任務: {}", i+1, title, parent_title);
                    }
                }
            }
            
            Ok(HttpResponse::Ok().json(ApiResponse {
                success: true,
                data: Some(tasks),
                message: "獲取首頁任務成功".to_string(),
            }))
        },
        Err(e) => {
            log::error!("獲取首頁任務失敗: {}", e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("獲取首頁任務失敗: {}", e),
            }))
        },
    }
}

// 建立重複性任務
pub async fn create_recurring_task(
    rb: web::Data<RBatis>,
    req: web::Json<CreateRecurringTaskRequest>,
) -> Result<HttpResponse> {
    // 驗證 user_id 是否存在
    let user_id = match &req.user_id {
        Some(id) => id.clone(),
        None => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: "缺少user_id參數".to_string(),
            }));
        }
    };

    let now = Utc::now();

    // 建立父任務
    let parent_task = Task {
        id: Some(Uuid::new_v4().to_string()),
        user_id: Some(user_id.clone()),
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
        start_date: req.start_date,
        end_date: req.end_date,
        completion_target: req.completion_target.or(Some(0.8)),
        completion_rate: Some(0.0),
        task_date: None,
        cancel_count: Some(0),
        last_cancelled_at: None,
        skill_tags: req.skill_tags.clone(), // 從請求中獲取技能標籤
        career_mainline_id: None,
        task_category: None,
        attributes: None,
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
                    title: Some(template.title.clone()),
                    description: template.description.clone(),
                    difficulty: Some(template.difficulty),
                    experience: Some(template.experience),
                    task_order: Some(template.order),
                    created_at: Some(now),
                    updated_at: Some(now),
                    skill_tags: template.skill_tags.clone(), // 從模板複製技能標籤
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

    // 獲取父任務以取得 user_id
    let parent_tasks = match Task::select_by_map(rb.get_ref(), value!{"id": parent_task_id.clone()}).await {
        Ok(tasks) => tasks,
        Err(e) => {
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
            message: "找不到父任務".to_string(),
        }));
    }

    let parent_task = &parent_tasks[0];
    let user_id = parent_task.user_id.clone().unwrap_or_else(|| {
        log::warn!("Parent task {} has no user_id", parent_task_id);
        String::new()
    });

    // 檢查今日任務是否已存在
    let existing_tasks_sql = "SELECT COUNT(*) as count FROM task WHERE parent_task_id = ? AND task_date = ?";
    let result = rb.exec(existing_tasks_sql, vec![
        Value::String(parent_task_id.clone()),
        Value::String(today.clone()),
    ]).await;

    match result {
        Ok(_exec_result) => {
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
                let daily_task = crate::models::Task {
                    id: Some(Uuid::new_v4().to_string()),
                    user_id: Some(user_id.clone()),
                    title: Some(template.title.unwrap_or_default()),
                    description: template.description.clone(),
                    status: Some(0), // 待完成
                    priority: Some(1),
                    task_type: Some("daily_recurring".to_string()),
                    difficulty: template.difficulty,
                    experience: template.experience,
                    parent_task_id: Some(parent_task_id.clone()),
                    is_parent_task: Some(0),
                    task_order: template.task_order,
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
                    skill_tags: template.skill_tags.clone(), // 從模板複製技能標籤
                    career_mainline_id: None,
                    task_category: None,
                    attributes: None,
                };
                
                if let Ok(_) = crate::models::Task::insert(rb.get_ref(), &daily_task).await {
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
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse> {
    let parent_task_id = path.into_inner();
    let today = Utc::now().format("%Y-%m-%d").to_string();

    // 獲取用戶ID參數
    let user_id = match query.get("user_id") {
        Some(id) => id,
        None => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: "缺少user_id參數".to_string(),
            }));
        }
    };

    // 獲取父任務信息並驗證用戶權限
    match crate::models::Task::select_by_map(rb.get_ref(), value!{"id": parent_task_id.clone()}).await {
        Ok(tasks) => {
            if let Some(parent_task) = tasks.first() {
                // 驗證任務是否屬於當前用戶
                if parent_task.user_id.as_ref() != Some(user_id) {
                    return Ok(HttpResponse::Forbidden().json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        message: "無權限存取此任務".to_string(),
                    }));
                }
                if parent_task.is_recurring == Some(1) {
                    // 重複性任務的進度計算
                    let start_date = parent_task.start_date.unwrap_or(Utc::now());
                    let end_date = parent_task.end_date.unwrap_or(Utc::now() + chrono::Duration::days(365));
                    let recurrence_pattern = parent_task.recurrence_pattern.as_deref().unwrap_or("daily");
                    
                    // 根據重複模式計算實際應該執行的總天數
                    let period_days = (end_date - start_date).num_days() as i32 + 1;
                    log::info!("任務 {} 日期範圍: {} 到 {}, 期間天數: {}, 重複模式: {}", 
                               parent_task_id, 
                               start_date.format("%Y-%m-%d"), 
                               end_date.format("%Y-%m-%d"), 
                               period_days, 
                               recurrence_pattern);
                    
                    let total_days = match recurrence_pattern {
                        "daily" => period_days,
                        "weekdays" => {
                            // 計算期間內的工作日天數
                            let mut weekdays = 0;
                            for i in 0..period_days {
                                let check_date = start_date + chrono::Duration::days(i as i64);
                                let weekday = check_date.weekday();
                                if weekday != chrono::Weekday::Sat && weekday != chrono::Weekday::Sun {
                                    weekdays += 1;
                                }
                            }
                            weekdays
                        },
                        "weekends" => {
                            // 計算期間內的週末天數
                            let mut weekends = 0;
                            for i in 0..period_days {
                                let check_date = start_date + chrono::Duration::days(i as i64);
                                let weekday = check_date.weekday();
                                if weekday == chrono::Weekday::Sat || weekday == chrono::Weekday::Sun {
                                    weekends += 1;
                                }
                            }
                            weekends
                        },
                        _ => period_days, // 預設為每日
                    };
                    
                    // 計算到今日為止應該有的天數
                    let current_period_days = std::cmp::min(
                        (Utc::now() - start_date).num_days() as i32 + 1,
                        period_days
                    );
                    let days_since_start = match recurrence_pattern {
                        "daily" => current_period_days,
                        "weekdays" => {
                            let mut weekdays = 0;
                            for i in 0..current_period_days {
                                let check_date = start_date + chrono::Duration::days(i as i64);
                                let weekday = check_date.weekday();
                                if weekday != chrono::Weekday::Sat && weekday != chrono::Weekday::Sun {
                                    weekdays += 1;
                                }
                            }
                            weekdays
                        },
                        "weekends" => {
                            let mut weekends = 0;
                            for i in 0..current_period_days {
                                let check_date = start_date + chrono::Duration::days(i as i64);
                                let weekday = check_date.weekday();
                                if weekday == chrono::Weekday::Sat || weekday == chrono::Weekday::Sun {
                                    weekends += 1;
                                }
                            }
                            weekends
                        },
                        _ => current_period_days,
                    };
                    
                    // 先簡化查詢，看看是否有任何已完成的子任務
                    let completed_days_sql = format!(
                        "SELECT COUNT(DISTINCT task_date) as count FROM task 
                         WHERE parent_task_id = '{}' AND status = {} AND task_date IS NOT NULL 
                         AND task_date >= '{}' AND task_date <= '{}'",
                        parent_task_id,
                        crate::models::TaskStatus::DailyCompleted.to_i32(),
                        start_date.format("%Y-%m-%d"),
                        std::cmp::min(Utc::now(), end_date).format("%Y-%m-%d")
                    );
                    
                    let completed_days = match rb.query_decode::<Vec<serde_json::Value>>(&completed_days_sql, vec![]).await {
                        Ok(result) => {
                            if let Some(row) = result.first() {
                                if let Some(count) = row.get("count").and_then(|v| v.as_i64()) {
                                    log::info!("任務 {} 查詢到 {} 個已完成天數", parent_task_id, count);
                                    count as i32
                                } else {
                                    log::warn!("任務 {} 無法解析count欄位: {:?}", parent_task_id, row);
                                    0
                                }
                            } else {
                                log::warn!("任務 {} 查詢結果為空", parent_task_id);
                                0
                            }
                        },
                        Err(e) => {
                            log::error!("任務 {} 查詢失敗: {}", parent_task_id, e);
                            log::error!("SQL: {}", completed_days_sql);
                            0
                        },
                    };
                    
                    // 計算錯過的天數（到今日為止應該完成但未完成的天數）
                    let missed_days = days_since_start - completed_days;
                    
                    // 檢查今日是否完成
                    let today_tasks_sql = format!(
                        "SELECT 
                            COUNT(*) as total,
                            SUM(CASE WHEN status = {} THEN 1 ELSE 0 END) as completed
                         FROM task 
                         WHERE parent_task_id = '{}' AND task_date = '{}'",
                        crate::models::TaskStatus::DailyCompleted.to_i32(),
                        parent_task_id, today
                    );
                    
                    let (is_daily_completed, _total_today, _completed_today) = match rb.query_decode::<Vec<serde_json::Value>>(&today_tasks_sql, vec![]).await {
                        Ok(result) => {
                            if let Some(row) = result.first() {
                                let total = row.get("total").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                                let completed = row.get("completed").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                                (total > 0 && completed == total, total, completed)
                            } else {
                                (false, 0, 0)
                            }
                        }
                        Err(_) => (false, 0, 0),
                    };
                    
                    // 計算完成率和剩餘天數
                    let completion_rate = if total_days > 0 {
                        completed_days as f64 / total_days as f64
                    } else {
                        0.0
                    };

                    log::info!("任務 {} 完成率計算: {}/{} = {:.1}%",
                               parent_task_id, completed_days, total_days, completion_rate * 100.0);

                    // 計算連續完成天數（從今天往回推算）
                    let consecutive_days_sql = format!(
                        "SELECT task_date FROM task
                         WHERE parent_task_id = '{}' AND status = {} AND task_date IS NOT NULL
                         AND task_date <= '{}'
                         ORDER BY task_date DESC",
                        parent_task_id,
                        crate::models::TaskStatus::DailyCompleted.to_i32(),
                        today
                    );

                    let consecutive_days = match rb.query_decode::<Vec<serde_json::Value>>(&consecutive_days_sql, vec![]).await {
                        Ok(result) => {
                            let mut streak = 0;
                            let mut check_date = Utc::now().date_naive();

                            for row in result.iter() {
                                if let Some(task_date_str) = row.get("task_date").and_then(|v| v.as_str()) {
                                    if let Ok(task_date) = chrono::NaiveDate::parse_from_str(task_date_str, "%Y-%m-%d") {
                                        // 檢查是否與預期日期連續
                                        if task_date == check_date {
                                            streak += 1;
                                            check_date = check_date - chrono::Duration::days(1);
                                        } else if task_date < check_date {
                                            // 發現斷層，停止計算
                                            break;
                                        }
                                    }
                                }
                            }

                            log::info!("任務 {} 連續完成天數: {}", parent_task_id, streak);
                            streak
                        },
                        Err(e) => {
                            log::error!("任務 {} 連續天數查詢失敗: {}", parent_task_id, e);
                            0
                        },
                    };

                    let target_rate = parent_task.completion_target.unwrap_or(0.8);
                    let remaining_days = std::cmp::max(0, total_days - days_since_start);

                    let progress = TaskProgressResponse {
                        task_id: parent_task_id,
                        total_days,
                        completed_days,
                        missed_days: std::cmp::max(0, missed_days), // 確保不為負數
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
                } else {
                    // 一般任務的進度計算
                    let completion_rate = parent_task.completion_rate.unwrap_or(0.0);
                    let target_rate = parent_task.completion_target.unwrap_or(1.0);

                    // 對於一般任務，我們簡化處理
                    let progress = TaskProgressResponse {
                        task_id: parent_task_id,
                        total_days: 1,
                        completed_days: if parent_task.status == Some(crate::models::TaskStatus::Completed.to_i32()) { 1 } else { 0 },
                        missed_days: 0,
                        completion_rate,
                        target_rate,
                        is_daily_completed: parent_task.status == Some(crate::models::TaskStatus::Completed.to_i32()),
                        remaining_days: if parent_task.status == Some(crate::models::TaskStatus::Completed.to_i32()) { 0 } else { 1 },
                    };

                    Ok(HttpResponse::Ok().json(ApiResponse {
                        success: true,
                        data: Some(progress),
                        message: "獲取任務進度成功".to_string(),
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
            let mut profile = profiles.first().cloned();
            let mut attr = attrs.first().cloned();
            
            if user.is_none() {
                log::error!("未找到用戶資料");
                return Ok(HttpResponse::NotFound().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: "用戶不存在".to_string(),
                }));
            }
            
            // 若缺少 profile 或 attributes，嘗試自動補齊（懶初始化）
            if profile.is_none() || attr.is_none() {
                log::warn!(
                    "用戶 {} 缺少資料：profile={} attrs={}，嘗試自動建立...",
                    user_id,
                    profile.is_none(),
                    attr.is_none()
                );
                let now = Utc::now().to_rfc3339();

                if profile.is_none() {
                    let profile_id = Uuid::new_v4().to_string();
                    let insert_profile_sql = r#"
                        INSERT INTO user_profile (
                            id, user_id, level, experience, max_experience, title,
                            adventure_days, consecutive_login_days, persona_type, created_at, updated_at
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#;
                    if let Err(e) = rb.exec(
                        insert_profile_sql,
                        vec![
                            profile_id.into(),
                            user_id.clone().into(),
                            1i32.into(),
                            0i32.into(),
                            100i32.into(),
                            "新手冒險者".into(),
                            1i32.into(),
                            1i32.into(),
                            "internal".into(),
                            now.clone().into(),
                            now.clone().into(),
                        ],
                    ).await {
                        log::error!("自動建立 user_profile 失敗: {}", e);
                    } else {
                        log::info!("已自動為用戶 {} 建立 user_profile", user_id);
                    }
                    // 重新查詢
                    if let Ok(profiles2) = UserProfile::select_by_map(rb.get_ref(), value!{"user_id": user_id.clone()}).await {
                        profile = profiles2.first().cloned();
                    }
                }

                if attr.is_none() {
                    let attributes_id = Uuid::new_v4().to_string();
                    let insert_attr_sql = r#"
                        INSERT INTO user_attributes (
                            id, user_id, intelligence, endurance, creativity, social, focus, adaptability, created_at, updated_at
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#;
                    if let Err(e) = rb.exec(
                        insert_attr_sql,
                        vec![
                            attributes_id.into(),
                            user_id.clone().into(),
                            50i32.into(),
                            50i32.into(),
                            50i32.into(),
                            50i32.into(),
                            50i32.into(),
                            50i32.into(),
                            now.clone().into(),
                            now.clone().into(),
                        ],
                    ).await {
                        log::error!("自動建立 user_attributes 失敗: {}", e);
                    } else {
                        log::info!("已自動為用戶 {} 建立 user_attributes", user_id);
                    }
                    // 重新查詢
                    if let Ok(attrs2) = UserAttributes::select_by_map(rb.get_ref(), value!{"user_id": user_id.clone()}).await {
                        attr = attrs2.first().cloned();
                    }
                }

                if profile.is_none() || attr.is_none() {
                    log::error!("補齊後依然缺少用戶資料 (profile 或 attributes)");
                    return Ok(HttpResponse::NotFound().json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        message: "用戶資料尚未初始化，請稍後重試".to_string(),
                    }));
                }
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
        Ok(achievements) => {
            // 獲取總用戶數
            let total_users = match get_total_user_count(rb.get_ref()).await {
                Ok(count) => count,
                Err(e) => {
                    log::warn!("獲取總用戶數失敗: {}", e);
                    0
                }
            };

            // 為每個成就添加統計資訊
            let mut achievements_with_stats = Vec::new();

            for achievement in achievements {
                let achievement_id = achievement.id.as_ref().map(|s| s.as_str()).unwrap_or("");

                // 獲取統計資訊
                let stats = AchievementStats::select_by_map(rb.get_ref(), value!{"achievement_id": achievement_id}).await.unwrap_or_default();
                let completion_count = stats.first()
                    .and_then(|s| s.completion_count)
                    .unwrap_or(0);

                // 計算完成率
                let completion_rate = if total_users > 0 {
                    completion_count as f64 / total_users as f64
                } else {
                    0.0
                };

                let achievement_with_stats = AchievementWithStats {
                    id: achievement.id.as_ref().unwrap_or(&String::new()).clone(),
                    name: achievement.name.as_ref().unwrap_or(&String::new()).clone(),
                    description: achievement.description.clone(),
                    icon: achievement.icon.clone(),
                    category: achievement.category.clone(),
                    requirement_type: achievement.requirement_type.as_ref().map(|rt| rt.to_string().to_owned()),
                    requirement_value: achievement.requirement_value,
                    experience_reward: achievement.experience_reward,
                    completion_count,
                    total_users,
                    completion_rate,
                    created_at: achievement.created_at,
                };

                achievements_with_stats.push(achievement_with_stats);
            }

            Ok(HttpResponse::Ok().json(ApiResponse {
                success: true,
                data: Some(achievements_with_stats),
                message: "獲取成就列表成功".to_string(),
            }))
        },
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("獲取成就列表失敗: {}", e),
        })),
    }
}

// 獲取用戶已解鎖的成就
pub async fn get_user_achievements(
    rb: web::Data<RBatis>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let user_id = path.into_inner();

    // 使用 SQL JOIN 查詢直接獲取用戶已解鎖的成就及其詳細資訊
    let sql = r#"
        SELECT
            a.id, a.name, a.description, a.icon, a.category,
            a.requirement_type, a.requirement_value, a.experience_reward,
            ua.achieved_at, ua.progress
        FROM
            achievement a
        JOIN
            user_achievement ua ON a.id = ua.achievement_id
        WHERE
            ua.user_id = ?
    "#;

    // 定義一個結構來接收查詢結果
    #[derive(serde::Serialize, serde::Deserialize, Clone)]
    struct UnlockedAchievementData {
        id: Option<String>,
        name: Option<String>,
        description: Option<String>,
        icon: Option<String>,
        category: Option<String>,
        requirement_type: Option<String>,
        requirement_value: Option<i32>,
        experience_reward: Option<i32>,
        achieved_at: Option<chrono::DateTime<chrono::Utc>>,
        progress: Option<i32>,
    }

    match rb.query_decode::<Vec<UnlockedAchievementData>>(sql, vec![rbs::Value::String(user_id)]).await {
        Ok(unlocked_achievements) => {
            // 組合數據，添加 unlocked: true 字段
            let result: Vec<serde_json::Value> = unlocked_achievements.iter().map(|ach| {
                serde_json::json!({
                    "id": ach.id,
                    "name": ach.name,
                    "description": ach.description,
                    "icon": ach.icon,
                    "category": ach.category,
                    "requirement_type": ach.requirement_type,
                    "requirement_value": ach.requirement_value,
                    "experience_reward": ach.experience_reward,
                    "unlocked": true, // 因為查詢結果都是已解鎖的
                    "progress": ach.progress,
                    "achieved_at": ach.achieved_at.as_ref().map(|dt| dt.to_string())
                })
            }).collect();

            Ok(HttpResponse::Ok().json(ApiResponse {
                success: true,
                data: Some(result),
                message: "獲取用戶已解鎖的成就成功".to_string(),
            }))
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("獲取用戶成就失敗: {}", e),
        })),
    }
}

// 獲取用戶的完整成就狀態（包含已解鎖和待完成）
pub async fn get_user_achievements_status(
    rb: web::Data<RBatis>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let user_id = path.into_inner();

    // 獲取所有成就
    let all_achievements = match Achievement::select_all(rb.get_ref()).await {
        Ok(achievements) => achievements,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("獲取成就列表失敗: {}", e),
            }));
        }
    };

    // 獲取用戶已解鎖的成就
    let user_achievements = match UserAchievement::select_by_map(
        rb.get_ref(), 
        value!{"user_id": user_id.clone()}
    ).await {
        Ok(achievements) => achievements,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("獲取用戶成就記錄失敗: {}", e),
            }));
        }
    };

    // 創建已解鎖成就的 HashMap 用於快速查找
    let mut unlocked_map: std::collections::HashMap<String, &UserAchievement> = std::collections::HashMap::new();
    for ua in &user_achievements {
        if let Some(achievement_id) = &ua.achievement_id {
            unlocked_map.insert(achievement_id.clone(), ua);
        }
    }

    // 合併數據，為每個成就添加狀態信息
    let default_id = String::new();
    let result: Vec<serde_json::Value> = all_achievements.iter().map(|achievement| {
        let achievement_id = achievement.id.as_ref().unwrap_or(&default_id);
        let is_unlocked = unlocked_map.contains_key(achievement_id);
        
        let mut achievement_data = serde_json::json!({
            "id": achievement.id,
            "name": achievement.name,
            "description": achievement.description,
            "icon": achievement.icon,
            "category": achievement.category,
            "requirement_type": achievement.requirement_type,
            "requirement_value": achievement.requirement_value,
            "experience_reward": achievement.experience_reward,
            "unlocked": is_unlocked,
            "progress": 0,
            "achieved_at": null
        });

        // 如果已解鎖，添加解鎖信息
        if let Some(user_achievement) = unlocked_map.get(achievement_id) {
            achievement_data["progress"] = serde_json::json!(user_achievement.progress.unwrap_or(0));
            achievement_data["achieved_at"] = serde_json::json!(
                user_achievement.achieved_at.as_ref().map(|dt| dt.to_string())
            );
        }

        achievement_data
    }).collect();

    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        data: Some(result),
        message: "獲取用戶完整成就狀態成功".to_string(),
    }))
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
                                Ok(_) => {
                                    // 成功插入用戶成就記錄後，更新成就統計
                                    if let Err(e) = increment_achievement_completion_count(rb.get_ref(), &achievement_id).await {
                                        log::warn!("更新成就統計失敗: {}", e);
                                        // 不影響主要流程，只記錄警告
                                    }

                                    Ok(HttpResponse::Created().json(ApiResponse {
                                        success: true,
                                        data: Some(serde_json::json!({
                                            "achievement": achievement,
                                            "unlocked_at": now.to_string(),
                                            "experience_reward": achievement.experience_reward
                                        })),
                                        message: format!("成就「{}」解鎖成功！", achievement.name.as_ref().unwrap_or(&"未知成就".to_string())),
                                    }))
                                },
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

// AI 生成任務功能已移至 ai_tasks.rs 模組

// AI 成就生成相關

#[derive(serde::Deserialize)]
pub struct GenerateAchievementRequest {
    pub user_id: String, // 使用者 ID，用於統計分析
    #[serde(default)]
    pub user_input: Option<String>, // 可選：相容舊版本
}

pub async fn generate_achievement_with_ai(
    rb: web::Data<RBatis>,
    req: web::Json<GenerateAchievementRequest>,
) -> Result<HttpResponse> {
    // 載入配置
    let config = crate::config::Config::from_env();

    // 創建 AI 服務
    let ai_service = match crate::ai_service::create_ai_service(&config.app.ai) {
        Ok(service) => service,
        Err(e) => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("AI 服務初始化失敗: {}", e),
            }));
        }
    };

    // 生成成就 - 使用新的統計摘要策略
    log::info!("開始為使用者 {} 生成成就（使用統計摘要優化）", req.user_id);

    match ai_service.generate_achievement_from_user_id(rb.get_ref(), &req.user_id).await {
        Ok(ai_achievement) => {
            // 轉換為資料庫模型
            let achievement_model = convert_to_achievement_model(ai_achievement.clone());
            
            // 插入到資料庫
            match Achievement::insert(rb.get_ref(), &achievement_model).await {
                Ok(_) => Ok(HttpResponse::Created().json(ApiResponse {
                    success: true,
                    data: Some(serde_json::json!({
                        "ai_generated": ai_achievement,
                        "database_record": achievement_model
                    })),
                    message: format!("成功生成並儲存成就：{}", ai_achievement.name),
                })),
                Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: format!("儲存成就到資料庫失敗: {}", e),
                })),
            }
        },
        Err(e) => Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: format!("生成成就失敗: {}", e),
        })),
    }
}

// 獲取單個成就詳細資訊（包含統計數據）
pub async fn get_achievement_details(
    rb: web::Data<RBatis>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let achievement_id = path.into_inner();

    match get_achievement_with_stats(rb.get_ref(), &achievement_id).await {
        Ok(Some(achievement_with_stats)) => {
            Ok(HttpResponse::Ok().json(ApiResponse {
                success: true,
                data: Some(achievement_with_stats),
                message: "獲取成就詳細資訊成功".to_string(),
            }))
        }
        Ok(None) => {
            Ok(HttpResponse::NotFound().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: "成就不存在".to_string(),
            }))
        }
        Err(e) => {
            Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("獲取成就詳細資訊失敗: {}", e),
            }))
        }
    }
}

// ChatGPT 聊天API端點
#[derive(serde::Deserialize)]
pub struct ChatGPTRequest {
    pub message: String,
    pub user_id: Option<String>,
}

#[derive(serde::Serialize)]
pub struct ChatGPTResponse {
    pub content: String,
}

pub async fn send_message_to_chatgpt(
    rb: web::Data<RBatis>,
    req: web::Json<ChatGPTRequest>,
) -> Result<HttpResponse> {
    log::info!("收到ChatGPT API請求: {}", req.message);
    log::debug!("請求 user_id: {:?}", req.user_id);
    let now = Utc::now();

    // 決定用戶ID（可選，如果沒有就不保存聊天記錄）
    let user_id = if let Some(id) = req.user_id.clone().filter(|s| !s.trim().is_empty()) {
        // 驗證提供的用戶ID是否存在
        match User::select_by_map(rb.get_ref(), value!{"id": id.clone()}).await {
            Ok(users) if !users.is_empty() => Some(id),
            _ => {
                log::warn!("提供的用戶ID不存在: {}", id);
                // 嘗試使用預設測試用戶
                match User::select_by_map(rb.get_ref(), value!{"email": "test@lifeup.com"}).await {
                    Ok(users) if !users.is_empty() => {
                        log::info!("使用預設測試用戶");
                        Some(users[0].id.clone().unwrap())
                    },
                    _ => {
                        log::warn!("找不到預設測試用戶，將以訪客身份對話");
                        None
                    }
                }
            }
        }
    } else {
        // 沒有提供用戶ID，嘗試使用預設測試用戶
        match User::select_by_map(rb.get_ref(), value!{"email": "test@lifeup.com"}).await {
            Ok(users) if !users.is_empty() => {
                log::info!("使用預設測試用戶");
                Some(users[0].id.clone().unwrap())
            },
            _ => {
                log::warn!("找不到預設測試用戶，將以訪客身份對話");
                None
            }
        }
    };
    
    // 如果有用戶ID，儲存使用者訊息到資料庫
    if let Some(uid) = user_id.clone() {
        let user_message = ChatMessage {
            id: Some(Uuid::new_v4().to_string()),
            user_id: Some(uid),
            role: Some("user".to_string()),
            content: Some(req.message.clone()),
            created_at: Some(now),
        };

        if let Err(e) = ChatMessage::insert(rb.get_ref(), &user_message).await {
            log::error!("儲存使用者訊息失敗: {}", e);
        } else {
            log::info!("成功儲存使用者訊息");
        }
    } else {
        log::info!("訪客模式，不保存用戶訊息");
    }

    // 呼叫ChatGPT API或使用本地回應
    let ai_response = match call_chatgpt_api(&req.message).await {
        Ok(response) => response,
        Err(e) => {
            log::error!("AI 回應取得失敗: {}", e);
            return Ok(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": format!("AI 服務調用失敗: {}", e)
            })));
        }
    };

    // 如果有用戶ID，儲存AI回覆到資料庫
    if let Some(uid) = user_id.clone() {
        let assistant_now = Utc::now();
        let assistant_message = ChatMessage {
            id: Some(Uuid::new_v4().to_string()),
            user_id: Some(uid),
            role: Some("assistant".to_string()),
            content: Some(ai_response.clone()),
            created_at: Some(assistant_now),
        };

        if let Err(e) = ChatMessage::insert(rb.get_ref(), &assistant_message).await {
            log::error!("儲存AI回覆失敗: {}", e);
        } else {
            log::info!("成功儲存AI回覆");
        }
    } else {
        log::info!("訪客模式，不保存AI回覆");
    }
    
    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "text": ai_response
    })))
}

// 簡單的測試端點
pub async fn test_endpoint() -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().json(json!({
        "message": "測試端點正常工作",
        "timestamp": Utc::now().to_string()
    })))
}

async fn call_chatgpt_api(message: &str) -> Result<String, Box<dyn std::error::Error>> {
    log::info!("開始呼叫AI 提供者");
    
    // 載入配置
    let config = crate::config::Config::from_env();
    let provider = config.app.ai.api_option.clone();
    
    // 創建 AI 服務
    let ai_service = match crate::ai_service::create_ai_service(&config.app.ai) {
        Ok(service) => service,
        Err(e) => {
            log::error!("AI 服務初始化失敗: {}", e);
            return Err(format!("AI 服務初始化失敗 ({})", e).into());
        }
    };
    
    // 使用專家系統匹配最適合的專家
    log::info!("開始為訊息匹配專家 (provider: {}): {}", provider, message);
    let expert_match = ai_service.match_expert_for_task(message).await.map_err(|e| {
        log::error!("專家匹配失敗 (provider: {}): {}", provider, e);
        e
    })?;

    log::info!(
        "成功匹配專家 (provider: {}): {}",
        provider,
        expert_match.expert.name
    );
    
    // 使用專家的專業知識構建提示詞
    let prompt = format!(
        "你是{}，{}。請根據你的專業知識為用戶提供建議。一律使用繁體中文回答。\n\n用戶訊息：{}", 
        expert_match.expert.name,
        expert_match.expert.description,
        message
    );

    log::info!(
        "準備發送請求到 AI API (provider: {}，專家: {})",
        provider,
        expert_match.expert.name
    );
    
    match ai_service.generate_task_preview(&prompt).await {
        Ok(response) => {
            log::info!("成功從 AI API (provider: {}) 獲取回應", provider);
            // 在回應前加上專家信息
            let expert_response = format!("[{}] {}", expert_match.expert.emoji, response);
            Ok(expert_response)
        },
        Err(e) => {
            log::error!("AI API 調用失敗 (provider: {}): {}", provider, e);
            Err(format!("AI API 調用失敗: {}", e).into())
        }
    }
}

// 檢查並更新父任務狀態
async fn check_and_update_parent_task_status(rb: &RBatis, parent_task_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    log::info!("檢查父任務 {} 的狀態", parent_task_id);

    // 先獲取父任務資訊
    let parent_tasks = Task::select_by_map(rb, value!{"id": parent_task_id}).await?;
    let parent_task = match parent_tasks.first() {
        Some(task) => task,
        None => {
            log::warn!("找不到父任務: {}", parent_task_id);
            return Ok(());
        }
    };

    // 判斷是否為重複性任務
    let is_recurring = parent_task.is_recurring.unwrap_or(0) == 1;

    // 查詢子任務
    let all_subtasks = Task::select_by_map(rb, value!{"parent_task_id": parent_task_id}).await?;

    if all_subtasks.is_empty() {
        log::info!("父任務 {} 沒有子任務", parent_task_id);
        return Ok(());
    }

    // 根據任務類型過濾相關子任務
    let relevant_subtasks: Vec<&Task> = if is_recurring {
        // 重複性任務：只看今日的子任務
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        all_subtasks.iter()
            .filter(|task| {
                task.task_date.as_ref().map(|d| d == &today).unwrap_or(false)
            })
            .collect()
    } else {
        // 普通任務：看所有子任務
        all_subtasks.iter().collect()
    };

    if relevant_subtasks.is_empty() {
        log::info!("父任務 {} 沒有相關的子任務（今日：{}）", parent_task_id, is_recurring);
        // 如果沒有相關子任務，父任務應該是 pending 狀態
        update_parent_task_status(rb, parent_task_id, 0).await?;
        return Ok(());
    }

    // 統計子任務狀態
    let total_subtasks = relevant_subtasks.len();
    let completed_subtasks = relevant_subtasks.iter()
        .filter(|task| {
            if is_recurring {
                // 重複性任務：daily_completed 或 completed 都算完成
                task.status == Some(6) || task.status == Some(2)
            } else {
                // 普通任務：只有 completed 算完成
                task.status == Some(2)
            }
        })
        .count();

    let in_progress_subtasks = relevant_subtasks.iter()
        .filter(|task| {
            if is_recurring {
                // 重複性任務：daily_in_progress 或 in_progress 都算進行中
                task.status == Some(5) || task.status == Some(1)
            } else {
                // 普通任務：只有 in_progress 算進行中
                task.status == Some(1)
            }
        })
        .count();

    // 統計 pending 狀態的子任務
    let pending_subtasks = relevant_subtasks.iter()
        .filter(|task| task.status == Some(0)) // pending
        .count();

    log::info!("父任務 {} (重複性: {}) 有 {} 個相關子任務，其中 {} 個已完成，{} 個進行中，{} 個待處理",
               parent_task_id, is_recurring, total_subtasks, completed_subtasks, in_progress_subtasks, pending_subtasks);

    // 根據子任務狀態推導父任務狀態
    let new_parent_status = if completed_subtasks == total_subtasks {
        // 所有子任務完成 → 父任務完成
        2 // completed
    } else if pending_subtasks == total_subtasks {
        // 所有子任務都是 pending → 父任務 pending
        0 // pending
    } else {
        // 其他情況（有任何子任務非 pending 狀態）→ 父任務進行中
        // 包括：有 in_progress、有 completed 但未全部完成、混合狀態等
        1 // in_progress
    };

    // 檢查是否需要更新父任務狀態
    if parent_task.status != Some(new_parent_status) {
        log::info!("父任務 {} 狀態需要更新: {} → {}",
                   parent_task_id,
                   parent_task.status.unwrap_or(-1),
                   new_parent_status);
        update_parent_task_status(rb, parent_task_id, new_parent_status).await?;
    } else {
        log::info!("父任務 {} 狀態無需更新，保持: {}", parent_task_id, new_parent_status);
    }

    Ok(())
}

// 輔助函數：更新父任務狀態
async fn update_parent_task_status(rb: &RBatis, parent_task_id: &str, new_status: i32) -> Result<(), Box<dyn std::error::Error>> {
    let update_sql = "UPDATE task SET status = ?, updated_at = ? WHERE id = ?";
    rb.exec(
        update_sql,
        vec![
            Value::I32(new_status),
            Value::String(chrono::Utc::now().to_string()),
            Value::String(parent_task_id.to_string()),
        ],
    ).await?;

    let status_name = match new_status {
        0 => "pending",
        1 => "in_progress",
        2 => "completed",
        _ => "unknown",
    };
    log::info!("父任務 {} 狀態更新為: {}", parent_task_id, status_name);
    Ok(())
}

// 輔助函數：更新父任務經驗值為所有子任務經驗值總和
pub async fn update_parent_task_experience(rb: &RBatis, parent_task_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    // 查詢所有子任務
    let subtasks = crate::models::Task::select_by_map(rb, value!{"parent_task_id": parent_task_id}).await?;

    if subtasks.is_empty() {
        log::info!("父任務 {} 沒有子任務，保持原有經驗值", parent_task_id);
        return Ok(());
    }

    // 計算所有子任務的經驗值總和
    let total_experience: i32 = subtasks.iter()
        .map(|subtask| subtask.experience.unwrap_or(0))
        .sum();

    // 更新父任務的經驗值
    let update_sql = "UPDATE task SET experience = ?, updated_at = ? WHERE id = ?";
    rb.exec(
        update_sql,
        vec![
            Value::I32(total_experience),
            Value::String(chrono::Utc::now().to_string()),
            Value::String(parent_task_id.to_string()),
        ],
    ).await?;

    log::info!("父任務 {} 經驗值已更新為子任務總和: {} (共 {} 個子任務)",
               parent_task_id, total_experience, subtasks.len());
    Ok(())
}

// ============= 教練個性系統 API =============

use crate::models::{
    CoachPersonalityType, UserCoachPreference, 
    SetCoachPersonalityRequest, CoachPersonalityResponse,
    AvailablePersonalitiesResponse, CoachPersonalityInfo,
    ChatWithPersonalityRequest, DirectPersonalityChatRequest
};

// 獲取所有可用的教練個性
pub async fn get_available_personalities(
    rb: web::Data<RBatis>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse> {
    let user_id = query.get("user_id").map(|s| s.clone());

    // 獲取用戶當前選擇的個性
    let current_personality = if let Some(uid) = &user_id {
        match UserCoachPreference::select_by_map(rb.get_ref(), value!{"user_id": uid}).await {
            Ok(preferences) => {
                if let Some(pref) = preferences.first() {
                    pref.personality_type.clone()
                } else {
                    None
                }
            }
            Err(_) => None
        }
    } else {
        None
    };

    // 定義所有可用的教練個性
    let personalities = vec![
        CoachPersonalityInfo {
            personality_type: "harsh_critic".to_string(),
            display_name: "森氣氣".to_string(),
            description: "直言不諱，用嚴厲的話語督促你成長".to_string(),
            emoji: "😤".to_string(),
        },
        CoachPersonalityInfo {
            personality_type: "emotional_support".to_string(),
            display_name: "小太陽".to_string(),
            description: "溫暖體貼，提供情感支持和正向鼓勵".to_string(),
            emoji: "🤗".to_string(),
        },
        CoachPersonalityInfo {
            personality_type: "analytical".to_string(),
            display_name: "小書蟲".to_string(),
            description: "理性客觀，用數據和邏輯幫你分析問題".to_string(),
            emoji: "📊".to_string(),
        },
    ];

    let response = AvailablePersonalitiesResponse {
        personalities,
        current_personality,
    };

    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        data: Some(response),
        message: "成功獲取可用教練個性".to_string(),
    }))
}

// 設定教練個性
pub async fn set_coach_personality(
    rb: web::Data<RBatis>,
    req: web::Json<SetCoachPersonalityRequest>,
) -> Result<HttpResponse> {
    log::info!("收到設定教練個性請求: {:?}", req);
    
    // 驗證個性類型是否有效
    let personality_type = match CoachPersonalityType::from_string(&req.personality_type) {
        Some(p) => p,
        None => {
            log::error!("無效的教練個性類型: {}", req.personality_type);
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("無效的教練個性類型: {}", req.personality_type),
            }));
        }
    };

    // 決定用戶ID（如果沒有提供，使用預設測試用戶）
    let user_id = if let Some(id) = req.user_id.clone().filter(|s| !s.trim().is_empty()) {
        log::info!("驗證用戶ID: {}", id);
        // 驗證提供的用戶ID是否存在
        match User::select_by_map(rb.get_ref(), value!{"id": id.clone()}).await {
            Ok(users) if !users.is_empty() => {
                log::info!("用戶ID驗證成功: {}", id);
                id
            },
            Ok(_) => {
                log::error!("找不到用戶ID: {}", id);
                return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: format!("找不到用戶ID: {}", id),
                }));
            },
            Err(e) => {
                log::error!("查詢用戶失敗: {}", e);
                return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: format!("查詢用戶失敗: {}", e),
                }));
            }
        }
    } else {
        // 查詢或建立預設測試用戶
        match User::select_by_map(rb.get_ref(), value!{"email": "test@lifeup.com"}).await {
            Ok(users) if !users.is_empty() => {
                users[0].id.clone().unwrap()
            }
            _ => {
                // 如果沒有測試用戶，創建一個
                let test_user = User {
                    id: Some(uuid::Uuid::new_v4().to_string()),
                    name: Some("測試用戶".to_string()),
                    email: Some("test@lifeup.com".to_string()),
                    password_hash: Some("".to_string()),
                    created_at: Some(Utc::now()),
                    updated_at: Some(Utc::now()),
                };
                
                match User::insert(rb.get_ref(), &test_user).await {
                    Ok(_) => {
                        log::info!("已創建預設測試用戶");
                        test_user.id.unwrap()
                    }
                    Err(e) => {
                        log::error!("創建測試用戶失敗: {}", e);
                        return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                            success: false,
                            data: None,
                            message: "創建測試用戶失敗".to_string(),
                        }));
                    }
                }
            }
        }
    };

    // 檢查是否已存在該用戶的個性設定
    let existing_preferences = UserCoachPreference::select_by_map(rb.get_ref(), value!{"user_id": user_id.clone()}).await
        .unwrap_or_else(|_| vec![]);

    if let Some(existing) = existing_preferences.into_iter().next() {
        // 更新現有設定
        let update_sql = "UPDATE user_coach_preference SET personality_type = ?, updated_at = ? WHERE id = ?";
        match rb.exec(update_sql, vec![
            rbs::Value::String(req.personality_type.clone()),
            rbs::Value::String(Utc::now().to_string()),
            rbs::Value::String(existing.id.clone().unwrap())
        ]).await {
            Ok(_) => {
                log::info!("已更新用戶 {} 的教練個性為 {}", user_id, req.personality_type);
            }
            Err(e) => {
                log::error!("更新教練個性失敗: {}", e);
                return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: format!("更新失敗: {}", e),
                }));
            }
        }
    } else {
        // 創建新設定
        let new_preference = UserCoachPreference {
            id: Some(uuid::Uuid::new_v4().to_string()),
            user_id: Some(user_id.clone()),
            personality_type: Some(req.personality_type.clone()),
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
        };

        match UserCoachPreference::insert(rb.get_ref(), &new_preference).await {
            Ok(_) => {
                log::info!("已為用戶 {} 創建教練個性設定: {}", user_id, req.personality_type);
            }
            Err(e) => {
                log::error!("創建教練個性設定失敗: {}", e);
                return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: format!("創建失敗: {}", e),
                }));
            }
        }
    }

    let response = CoachPersonalityResponse {
        personality_type: req.personality_type.clone(),
        display_name: personality_type.display_name().to_string(),
        description: personality_type.description().to_string(),
        is_active: true,
    };

    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        data: Some(response),
        message: format!("已成功設定教練個性為：{}", personality_type.display_name()),
    }))
}

// 獲取當前教練個性
pub async fn get_current_personality(
    rb: web::Data<RBatis>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse> {
    let user_id = match query.get("user_id").filter(|s| !s.trim().is_empty()) {
        Some(id) => {
            // 驗證用戶ID是否存在
            match User::select_by_map(rb.get_ref(), value!{"id": id.clone()}).await {
                Ok(users) if !users.is_empty() => id.clone(),
                _ => {
                    return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        message: format!("找不到用戶ID: {}", id),
                    }));
                }
            }
        },
        None => {
            // 使用預設測試用戶
            match User::select_by_map(rb.get_ref(), value!{"email": "test@lifeup.com"}).await {
                Ok(users) if !users.is_empty() => {
                    users[0].id.clone().unwrap()
                }
                _ => {
                    return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        message: "找不到預設測試用戶".to_string(),
                    }));
                }
            }
        }
    };

    match UserCoachPreference::select_by_map(rb.get_ref(), value!{"user_id": user_id}).await {
        Ok(preferences) => {
            if let Some(pref) = preferences.first() {
                if let Some(personality_str) = &pref.personality_type {
                    if let Some(personality_type) = CoachPersonalityType::from_string(personality_str) {
                        let response = CoachPersonalityResponse {
                            personality_type: personality_str.clone(),
                            display_name: personality_type.display_name().to_string(),
                            description: personality_type.description().to_string(),
                            is_active: true,
                        };

                        return Ok(HttpResponse::Ok().json(ApiResponse {
                            success: true,
                            data: Some(response),
                            message: "成功獲取當前教練個性".to_string(),
                        }));
                    }
                }
            }

            // 如果沒有設定，返回預設個性（情緒支持型）
            let default_personality = CoachPersonalityType::EmotionalSupport;
            let response = CoachPersonalityResponse {
                personality_type: "emotional_support".to_string(),
                display_name: default_personality.display_name().to_string(),
                description: default_personality.description().to_string(),
                is_active: false,
            };

            Ok(HttpResponse::Ok().json(ApiResponse {
                success: true,
                data: Some(response),
                message: "使用預設教練個性".to_string(),
            }))
        }
        Err(e) => {
            log::error!("查詢教練個性失敗: {}", e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("查詢失敗: {}", e),
            }))
        }
    }
}

// 獲取用戶的教練個性類型
async fn get_user_personality_type(rb: &RBatis, user_id: Option<String>) -> Result<CoachPersonalityType, Box<dyn std::error::Error>> {
    if let Some(uid) = user_id {
        match UserCoachPreference::select_by_map(rb, value!{"user_id": uid}).await {
            Ok(preferences) => {
                if let Some(pref) = preferences.first() {
                    if let Some(personality_str) = &pref.personality_type {
                        if let Some(personality_type) = CoachPersonalityType::from_string(personality_str) {
                            return Ok(personality_type);
                        }
                    }
                }
            }
            Err(_) => {}
        }
    }
    
    // 預設返回情緒支持型
    Ok(CoachPersonalityType::EmotionalSupport)
}

// 帶個性的AI API呼叫
async fn call_ai_api_with_personality(rb: &RBatis, message: &str, user_id: Option<String>) -> Result<String, Box<dyn std::error::Error>> {
    log::info!("開始呼叫個性化AI API");
    
    // 載入配置
    let config = crate::config::Config::from_env();
    
    // 創建 AI 服務
    let ai_service = match crate::ai_service::create_ai_service(&config.app.ai) {
        Ok(service) => service,
        Err(e) => {
            log::error!("AI 服務初始化失敗: {}", e);
            return Err(format!("AI 服務初始化失敗: {}", e).into());
        }
    };
    
    // 使用專家系統匹配最適合的專家
    log::info!("開始為訊息匹配專家: {}", message);
    let expert_match = match ai_service.match_expert_for_task(message).await {
        Ok(match_result) => {
            log::info!("成功匹配專家: {}",
                match_result.expert.name);
            Some(match_result)
        }
        Err(e) => {
            log::warn!("專家匹配失敗，將使用通用個性化教練: {}", e);
            None
        }
    };
    
    // 獲取用戶的教練個性
    let personality_type = get_user_personality_type(rb, user_id.clone()).await?;
    let base_system_prompt = personality_type.system_prompt();
    
    // 結合專家和個性化系統
    let system_prompt = if let Some(expert) = &expert_match {
        format!(
            "你是{}，{}。同時，你具有{}的教練個性。請結合你的專業知識和個性特質為用戶提供建議。一律使用繁體中文回答。\n\n{}",
            expert.expert.name,
            expert.expert.description,
            personality_type.display_name(),
            base_system_prompt
        )
    } else {
        base_system_prompt.to_string()
    };
    
    log::info!("使用教練個性: {:?}, 專家: {:?}", personality_type, 
        expert_match.as_ref().map(|e| &e.expert.name));
    
    // 獲取上一次的對話內容（用戶問題和AI回答）
    let mut prompt = system_prompt.to_string();
    
    if let Some(uid) = user_id {
        log::info!("嘗試獲取用戶 {} 的聊天記錄", uid);
        // 獲取最近的兩條聊天記錄（用戶問題和AI回答）
        // 創建一個簡化的 ChatMessage 結構來處理序列化問題
        #[derive(serde::Deserialize)]
        struct SimpleChatMessage {
            id: Option<String>,
            user_id: Option<String>,
            role: Option<String>,
            content: Option<serde_json::Value>, // 使用 serde_json::Value 來處理可能的 JSON 格式
            created_at: Option<String>,
        }
        
        let sql = format!("SELECT * FROM chat_message WHERE user_id = '{}' ORDER BY created_at DESC LIMIT 10", uid);
        match rb.query_decode::<Vec<SimpleChatMessage>>(&sql, vec![]).await {
            Ok(messages) => {
                log::info!("找到 {} 條聊天記錄", messages.len());
                
                // 獲取最新的用戶問題和AI回答
                let mut last_user_message = None;
                let mut last_ai_message = None;
                
                for (i, msg) in messages.iter().take(4).enumerate() { // 檢查最近4條記錄
                    log::info!("處理記錄 {}: role={:?}, content={:?}", i, msg.role, msg.content);
                    if let Some(role) = &msg.role {
                        // 處理 content 字段，可能是字符串或 JSON 對象
                        let content_str = match &msg.content {
                            Some(serde_json::Value::String(s)) => Some(s.clone()),
                            Some(serde_json::Value::Object(obj)) => {
                                // 如果是 JSON 對象，嘗試提取 text 字段
                                obj.get("text").and_then(|v| v.as_str()).map(|s| s.to_string())
                            },
                            _ => None,
                        };
                        
                        if role == "user" && last_user_message.is_none() {
                            last_user_message = content_str;
                            log::info!("找到用戶訊息: {:?}", last_user_message);
                        } else if role == "assistant" && last_ai_message.is_none() {
                            last_ai_message = content_str;
                            log::info!("找到AI訊息: {:?}", last_ai_message);
                        }
                    }
                }
                
                // 如果有上一次的對話，準備歷史對話數據
                let history = match (&last_user_message, &last_ai_message) {
                    (Some(user_msg), Some(ai_msg)) => {
                        log::info!("包含上一次對話內容");
                        vec![(user_msg.clone(), ai_msg.clone())]
                    },
                    _ => {
                        log::info!("沒有找到完整的上一次對話 - 用戶訊息: {:?}, AI訊息: {:?}", last_user_message, last_ai_message);
                        vec![]
                    }
                };
                
                // 使用帶歷史對話的方法
                match ai_service.generate_task_preview_with_history(&system_prompt, &history, &message).await {
                    Ok(response) => {
                        log::info!("成功獲取個性化AI回應");
                        // 如果有專家匹配，在回應前加上專家信息
                        let final_response = if let Some(expert) = &expert_match {
                            format!("[{}] {}", expert.expert.emoji, response)
                        } else {
                            response
                        };
                        return Ok(final_response);
                    },
                    Err(e) => {
                        log::error!("個性化AI API 調用失敗: {}", e);
                        return Err(format!("AI API 調用失敗: {}", e).into());
                    }
                }
            },
            Err(e) => {
                log::warn!("獲取聊天記錄失敗: {}", e);
                log::warn!("錯誤詳情: {:?}", e);
            }
        }
    } else {
        log::info!("沒有用戶ID，跳過聊天記錄查詢");
    }
    
    // 如果沒有用戶ID或查詢失敗，使用原始方法
    log::info!("準備發送個性化請求到AI API");
    
    match ai_service.generate_task_preview(&prompt).await {
        Ok(response) => {
            log::info!("成功獲取個性化AI回應");
            // 如果有專家匹配，在回應前加上專家信息
            let final_response = if let Some(expert) = &expert_match {
                format!("[{}] {}", expert.expert.emoji, response)
            } else {
                response
            };
            Ok(final_response)
        },
        Err(e) => {
            log::error!("個性化AI API 調用失敗: {}", e);
            Err(format!("AI API 調用失敗: {}", e).into())
        }
    }
}

// 新增：帶個性的聊天API
pub async fn send_message_with_personality(
    rb: web::Data<RBatis>,
    body: web::Bytes,
) -> Result<HttpResponse> {
    // 先記錄原始請求體
    let body_str = String::from_utf8_lossy(&body);
    log::info!("收到帶個性的AI API請求，原始body: {}", body_str);

    // 嘗試解析 JSON
    let req: ChatWithPersonalityRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            log::error!("無法解析 JSON 請求: {}", e);
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("JSON 解析錯誤: {}", e),
            }));
        }
    };

    log::info!("解析後的請求: message={}, user_id={:?}", req.message, req.user_id);
    let now = Utc::now();

    // 決定用戶ID（可選，如果沒有就不保存聊天記錄）
    let user_id = if let Some(id) = req.user_id.clone().filter(|s| !s.trim().is_empty()) {
        // 驗證提供的用戶ID是否存在
        match User::select_by_map(rb.get_ref(), value!{"id": id.clone()}).await {
            Ok(users) if !users.is_empty() => Some(id),
            _ => {
                log::warn!("提供的用戶ID不存在: {}", id);
                // 嘗試使用預設測試用戶
                match User::select_by_map(rb.get_ref(), value!{"email": "test@lifeup.com"}).await {
                    Ok(users) if !users.is_empty() => {
                        log::info!("使用預設測試用戶");
                        Some(users[0].id.clone().unwrap())
                    },
                    _ => {
                        log::warn!("找不到預設測試用戶，將以訪客身份對話");
                        None
                    }
                }
            }
        }
    } else {
        // 沒有提供用戶ID，嘗試使用預設測試用戶
        match User::select_by_map(rb.get_ref(), value!{"email": "test@lifeup.com"}).await {
            Ok(users) if !users.is_empty() => {
                log::info!("使用預設測試用戶");
                Some(users[0].id.clone().unwrap())
            },
            _ => {
                log::warn!("找不到預設測試用戶，將以訪客身份對話");
                None
            }
        }
    };

    // 如果有用戶ID，儲存用戶訊息到資料庫
    if let Some(uid) = user_id.clone() {
        let user_message = ChatMessage {
            id: Some(Uuid::new_v4().to_string()),
            user_id: Some(uid),
            role: Some("user".to_string()),
            content: Some(req.message.clone()),
            created_at: Some(now),
        };

        if let Err(e) = ChatMessage::insert(rb.get_ref(), &user_message).await {
            log::error!("儲存用戶訊息失敗: {}", e);
        }
    } else {
        log::info!("訪客模式，不保存聊天記錄");
    }

    // 呼叫帶個性的AI API
    let ai_response = match call_ai_api_with_personality(rb.get_ref(), &req.message, user_id.clone()).await {
        Ok(response) => {
            log::info!("成功獲取個性化AI回應");
            response
        }
        Err(e) => {
            log::warn!("個性化AI API呼叫失敗，使用本地回應: {}", e);
            // 根據用戶個性提供不同的備援回應
            let personality_type = get_user_personality_type(rb.get_ref(), user_id.clone()).await
                .unwrap_or(CoachPersonalityType::EmotionalSupport);

            match personality_type {
                CoachPersonalityType::HarshCritic => {
                    format!("系統暫時有問題，但這不是你偷懶的藉口！先想想你的問題：「{}」，我一會兒就來好好「指導」你！", req.message)
                }
                CoachPersonalityType::EmotionalSupport => {
                    format!("收到你的訊息了～雖然系統暫時不太穩定，但我會努力幫助你的💕 關於「{}」這個問題，等等再來詳細聊聊好嗎？", req.message)
                }
                CoachPersonalityType::Analytical => {
                    format!("系統錯誤代碼：AI服務暫時不可用。你的查詢「{}」已記錄，待服務恢復後將基於數據模型提供專業分析。", req.message)
                }
            }
        }
    };

    // 如果有用戶ID，儲存AI回應到資料庫
    if let Some(uid) = user_id.clone() {
        let assistant_now = Utc::now();
        let assistant_message = ChatMessage {
            id: Some(Uuid::new_v4().to_string()),
            user_id: Some(uid),
            role: Some("assistant".to_string()),
            content: Some(ai_response.clone()),
            created_at: Some(assistant_now),
        };

        if let Err(e) = ChatMessage::insert(rb.get_ref(), &assistant_message).await {
            log::error!("儲存AI回應失敗: {}", e);
        }
    }

    // 返回回應
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "text": ai_response
    })))
}

// 直接指定個性的聊天API（用於測試）
pub async fn send_message_with_direct_personality(
    rb: web::Data<RBatis>,
    req: web::Json<DirectPersonalityChatRequest>,
) -> Result<HttpResponse> {
    log::info!("收到直接指定個性的AI API請求: {} (個性: {})", req.message, req.personality_type);
    
    // 解析個性類型
    let personality_type = match CoachPersonalityType::from_string(&req.personality_type) {
        Some(pt) => pt,
        None => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("無效的個性類型: {}", req.personality_type),
            }));
        }
    };

    // 直接使用指定的個性呼叫AI服務
    let ai_response = match call_ai_api_with_direct_personality(&req.message, personality_type.clone()).await {
        Ok(response) => {
            log::info!("成功獲取指定個性的AI回應");
            response
        }
        Err(e) => {
            log::warn!("指定個性的AI API呼叫失敗，使用備援回應: {}", e);
            // 根據指定個性提供備援回應
            match personality_type {
                CoachPersonalityType::HarshCritic => {
                    format!("系統有問題？這不是你逃避問題的理由！關於「{}」，等系統修好了我會好好「指導」你的！", req.message)
                }
                CoachPersonalityType::EmotionalSupport => {
                    format!("哎呀，系統暫時不穩定呢～但是沒關係，關於「{}」這個問題，我們等等再一起討論吧💕", req.message)
                }
                CoachPersonalityType::Analytical => {
                    format!("錯誤分析：AI服務暫時不可用。查詢主題：「{}」。預計修復時間：未知。建議：稍後重試。", req.message)
                }
            }
        }
    };

    // 返回回應
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "text": ai_response,
        "personality_type": req.personality_type,
        "personality_display_name": personality_type.display_name()
    })))
}

// 直接使用指定個性呼叫AI API
async fn call_ai_api_with_direct_personality(message: &str, personality_type: CoachPersonalityType) -> Result<String, Box<dyn std::error::Error>> {
    log::info!("開始呼叫指定個性的AI API: {:?}", personality_type);
    
    // 載入配置
    let config = crate::config::Config::from_env();
    
    // 創建 AI 服務
    let ai_service = match crate::ai_service::create_ai_service(&config.app.ai) {
        Ok(service) => service,
        Err(e) => {
            log::error!("AI 服務初始化失敗: {}", e);
            return Err(format!("AI 服務初始化失敗: {}", e).into());
        }
    };
    
    // 使用專家系統匹配最適合的專家
    log::info!("開始為訊息匹配專家: {}", message);
    let expert_match = match ai_service.match_expert_for_task(message).await {
        Ok(match_result) => {
            log::info!("成功匹配專家: {}",
                match_result.expert.name);
            Some(match_result)
        }
        Err(e) => {
            log::warn!("專家匹配失敗，將使用通用指定個性教練: {}", e);
            None
        }
    };
    
    let base_system_prompt = personality_type.system_prompt();
    
    // 結合專家和指定個性
    let system_prompt = if let Some(expert) = &expert_match {
        format!(
            "你是{}，{}。同時，你具有{}的教練個性。請結合你的專業知識和個性特質為用戶提供建議。一律使用繁體中文回答。\n\n{}",
            expert.expert.name,
            expert.expert.description,
            personality_type.display_name(),
            base_system_prompt
        )
    } else {
        base_system_prompt.to_string()
    };
    
    log::info!("使用指定個性: {:?}, 專家: {:?}", personality_type, 
        expert_match.as_ref().map(|e| &e.expert.name));
    
    let prompt = format!("{}\n\n用戶訊息：{}", system_prompt, message);

    log::info!("準備發送指定個性請求到AI API");
    
    match ai_service.generate_task_preview(&prompt).await {
        Ok(response) => {
            log::info!("成功提取指定個性AI回應內容");
            // 如果有專家匹配，在回應前加上專家信息
            let final_response = if let Some(expert) = &expert_match {
                format!("[{}] {}", expert.expert.emoji, response)
            } else {
                response
            };
            Ok(final_response)
        },
        Err(e) => {
            log::error!("指定個性AI API 調用失敗: {}", e);
            Err(format!("AI API 調用失敗: {}", e).into())
        }
    }
}

// 重置類型枚舉
#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResetType {
    All,           // 重置所有數據
    Tasks,         // 只重置任務
    Skills,        // 只重置技能
    Chat,          // 只重置聊天記錄
    Progress,      // 只重置進度數據
    Achievements,  // 只重置成就
    Profile,       // 只重置遊戲化資料
}

// 選擇性重置請求結構
#[derive(serde::Deserialize)]
pub struct SelectiveResetRequest {
    pub reset_types: Vec<ResetType>,
}

// 重置結果結構
#[derive(serde::Serialize, Clone)]
pub struct ResetResult {
    pub total_deleted: i32,
    pub details: std::collections::HashMap<String, i32>,
}

// 完全重置用戶數據 API
pub async fn reset_user_data(rb: web::Data<RBatis>, path: web::Path<String>) -> Result<HttpResponse> {
    let user_id = path.into_inner();
    log::info!("開始完全重置用戶 {} 的數據...", user_id);

    match reset_user_all_data(rb.get_ref(), &user_id).await {
        Ok(result) => {
            log::info!("用戶 {} 數據重置成功，共刪除 {} 筆記錄", user_id, result.total_deleted);
            Ok(HttpResponse::Ok().json(ApiResponse {
                success: true,
                data: Some(result.clone()),
                message: format!("用戶數據重置成功，共刪除 {} 筆記錄", result.total_deleted),
            }))
        }
        Err(e) => {
            log::error!("用戶 {} 數據重置失敗: {}", user_id, e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("用戶數據重置失敗: {}", e),
            }))
        }
    }
}

// 選擇性重置用戶數據 API
pub async fn reset_user_data_selective(
    rb: web::Data<RBatis>,
    path: web::Path<String>,
    body: web::Json<SelectiveResetRequest>
) -> Result<HttpResponse> {
    let user_id = path.into_inner();
    let request = body.into_inner();

    log::info!("開始選擇性重置用戶 {} 的數據，重置類型: {:?}", user_id, request.reset_types.len());

    match reset_user_selective_data(rb.get_ref(), &user_id, request.reset_types).await {
        Ok(result) => {
            log::info!("用戶 {} 選擇性數據重置成功，共刪除 {} 筆記錄", user_id, result.total_deleted);
            Ok(HttpResponse::Ok().json(ApiResponse {
                success: true,
                data: Some(result.clone()),
                message: format!("用戶數據重置成功，共刪除 {} 筆記錄", result.total_deleted),
            }))
        }
        Err(e) => {
            log::error!("用戶 {} 選擇性數據重置失敗: {}", user_id, e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("用戶數據重置失敗: {}", e),
            }))
        }
    }
}

/// 完全重置用戶所有數據
async fn reset_user_all_data(rb: &RBatis, user_id: &str) -> Result<ResetResult, Box<dyn std::error::Error>> {
    let mut total_deleted = 0i32;
    let mut details = std::collections::HashMap::new();

    // 定義要重置的表和對應的條件
    // 按照外鍵依賴關係的順序刪除
    let delete_operations = vec![
        // 1. 先刪除依賴其他表的記錄
        ("user_achievement", format!("user_id = '{}'", user_id)),
        ("weekly_attribute_snapshot", format!("user_id = '{}'", user_id)),
        ("daily_progress", format!("user_id = '{}'", user_id)),
        ("chat_message", format!("user_id = '{}'", user_id)),

        // 2. 刪除重複任務模板（通過父任務關聯）
        ("recurring_task_template", format!("parent_task_id IN (SELECT id FROM task WHERE user_id = '{}')", user_id)),

        // 3. 刪除任務相關的記錄（子任務先於父任務）
        ("task", format!("user_id = '{}' AND parent_task_id IS NOT NULL", user_id)),
        ("task", format!("user_id = '{}' AND parent_task_id IS NULL", user_id)),

        // 4. 刪除技能記錄
        ("skill", format!("user_id = '{}'", user_id)),

        // 5. 刪除用戶相關記錄（但保留用戶主記錄）
        ("user_attributes", format!("user_id = '{}'", user_id)),
        ("user_profile", format!("user_id = '{}'", user_id)),
        ("user_coach_preference", format!("user_id = '{}'", user_id)),
        ("career_mainlines", format!("user_id = '{}'", user_id)),
        ("quiz_results", format!("user_id = '{}'", user_id)),
    ];

    for (table, condition) in delete_operations {
        let sql = format!("DELETE FROM {} WHERE {}", table, condition);

        match rb.exec(&sql, vec![]).await {
            Ok(result) => {
                let deleted = result.rows_affected as i32;
                if deleted > 0 {
                    log::info!("從 {} 表刪除了 {} 筆記錄", table, deleted);
                    details.insert(table.to_string(), deleted);
                    total_deleted += deleted;
                }
            }
            Err(e) => {
                log::warn!("刪除 {} 表時出現錯誤: {}", table, e);
                // 繼續執行其他刪除操作，不中斷整個流程
            }
        }
    }

    Ok(ResetResult {
        total_deleted,
        details,
    })
}

/// 選擇性重置用戶數據
async fn reset_user_selective_data(
    rb: &RBatis,
    user_id: &str,
    reset_types: Vec<ResetType>
) -> Result<ResetResult, Box<dyn std::error::Error>> {
    let mut total_deleted = 0i32;
    let mut details = std::collections::HashMap::new();

    for reset_type in reset_types {
        match reset_type {
            ResetType::All => {
                // 如果包含 All，直接調用完全重置
                return reset_user_all_data(rb, user_id).await;
            }
            ResetType::Tasks => {
                let operations = vec![
                    ("recurring_task_template", format!("parent_task_id IN (SELECT id FROM task WHERE user_id = '{}')", user_id)),
                    ("task", format!("user_id = '{}' AND parent_task_id IS NOT NULL", user_id)),
                    ("task", format!("user_id = '{}' AND parent_task_id IS NULL", user_id)),
                ];
                let deleted = execute_delete_operations(rb, operations).await?;
                details.insert("tasks".to_string(), deleted);
                total_deleted += deleted;
            }
            ResetType::Skills => {
                let deleted = delete_user_data(rb, "skill", user_id).await?;
                details.insert("skills".to_string(), deleted);
                total_deleted += deleted;
            }
            ResetType::Chat => {
                let deleted = delete_user_data(rb, "chat_message", user_id).await?;
                details.insert("chat".to_string(), deleted);
                total_deleted += deleted;
            }
            ResetType::Progress => {
                let operations = vec![
                    ("daily_progress", format!("user_id = '{}'", user_id)),
                    ("weekly_attribute_snapshot", format!("user_id = '{}'", user_id)),
                ];
                let deleted = execute_delete_operations(rb, operations).await?;
                details.insert("progress".to_string(), deleted);
                total_deleted += deleted;
            }
            ResetType::Achievements => {
                let deleted = delete_user_data(rb, "user_achievement", user_id).await?;
                details.insert("achievements".to_string(), deleted);
                total_deleted += deleted;
            }
            ResetType::Profile => {
                let operations = vec![
                    ("user_attributes", format!("user_id = '{}'", user_id)),
                    ("user_profile", format!("user_id = '{}'", user_id)),
                    ("user_coach_preference", format!("user_id = '{}'", user_id)),
                    ("career_mainlines", format!("user_id = '{}'", user_id)),
                    ("quiz_results", format!("user_id = '{}'", user_id)),
                ];
                let deleted = execute_delete_operations(rb, operations).await?;
                details.insert("profile".to_string(), deleted);
                total_deleted += deleted;
            }
        }
    }

    Ok(ResetResult {
        total_deleted,
        details,
    })
}

/// 執行單個表的刪除操作
async fn delete_user_data(rb: &RBatis, table: &str, user_id: &str) -> Result<i32, Box<dyn std::error::Error>> {
    let sql = format!("DELETE FROM {} WHERE user_id = '{}'", table, user_id);

    match rb.exec(&sql, vec![]).await {
        Ok(result) => {
            let deleted = result.rows_affected as i32;
            if deleted > 0 {
                log::info!("從 {} 表刪除了 {} 筆記錄", table, deleted);
            }
            Ok(deleted)
        }
        Err(e) => {
            log::warn!("刪除 {} 表時出現錯誤: {}", table, e);
            Err(e.into())
        }
    }
}

/// 執行多個刪除操作
async fn execute_delete_operations(
    rb: &RBatis,
    operations: Vec<(&str, String)>
) -> Result<i32, Box<dyn std::error::Error>> {
    let mut total_deleted = 0i32;

    for (table, condition) in operations {
        let sql = format!("DELETE FROM {} WHERE {}", table, condition);

        match rb.exec(&sql, vec![]).await {
            Ok(result) => {
                let deleted = result.rows_affected as i32;
                if deleted > 0 {
                    log::info!("從 {} 表刪除了 {} 筆記錄", table, deleted);
                    total_deleted += deleted;
                }
            }
            Err(e) => {
                log::warn!("刪除 {} 表時出現錯誤: {}", table, e);
                // 繼續執行其他刪除操作，不中斷整個流程
            }
        }
    }

    Ok(total_deleted)
}

// 成就統計相關輔助函數
async fn increment_achievement_completion_count(rb: &RBatis, achievement_id: &str) -> rbatis::Result<()> {
    let now = Utc::now();

    // 檢查是否已存在統計記錄
    match AchievementStats::select_by_map(rb, value!{"achievement_id": achievement_id}).await {
        Ok(stats) => {
            if let Some(stat) = stats.first() {
                // 更新現有記錄
                let sql = "UPDATE achievement_stats SET completion_count = completion_count + 1, updated_at = ? WHERE achievement_id = ?";
                rb.exec(sql, vec![Value::String(now.to_rfc3339()), Value::String(achievement_id.to_string())]).await?;
            } else {
                // 創建新記錄
                let new_stat = AchievementStats {
                    id: Some(Uuid::new_v4().to_string()),
                    achievement_id: Some(achievement_id.to_string()),
                    completion_count: Some(1),
                    created_at: Some(now),
                    updated_at: Some(now),
                };
                AchievementStats::insert(rb, &new_stat).await?;
            }
        }
        Err(e) => return Err(e),
    }

    Ok(())
}

async fn get_total_user_count(rb: &RBatis) -> rbatis::Result<i32> {
    let sql = "SELECT COUNT(*) as count FROM user";
    let result: Vec<serde_json::Value> = rb.query_decode(sql, vec![]).await?;

    if let Some(row) = result.first() {
        if let Some(count) = row.get("count").and_then(|v| v.as_i64()) {
            return Ok(count as i32);
        }
    }

    Ok(0)
}

async fn get_achievement_with_stats(rb: &RBatis, achievement_id: &str) -> rbatis::Result<Option<AchievementWithStats>> {
    // 獲取成就基本資訊
    let achievements = Achievement::select_by_map(rb, value!{"id": achievement_id}).await?;
    let achievement = match achievements.first() {
        Some(ach) => ach,
        None => return Ok(None),
    };

    // 獲取統計資訊
    let stats = AchievementStats::select_by_map(rb, value!{"achievement_id": achievement_id}).await?;
    let completion_count = stats.first()
        .and_then(|s| s.completion_count)
        .unwrap_or(0);

    // 獲取總用戶數
    let total_users = get_total_user_count(rb).await?;

    // 計算完成率
    let completion_rate = if total_users > 0 {
        completion_count as f64 / total_users as f64
    } else {
        0.0
    };

    let achievement_with_stats = AchievementWithStats {
        id: achievement.id.as_ref().unwrap_or(&String::new()).clone(),
        name: achievement.name.as_ref().unwrap_or(&String::new()).clone(),
        description: achievement.description.clone(),
        icon: achievement.icon.clone(),
        category: achievement.category.clone(),
        requirement_type: achievement.requirement_type.as_ref().map(|rt| rt.to_string().to_owned()),
        requirement_value: achievement.requirement_value,
        experience_reward: achievement.experience_reward,
        completion_count,
        total_users,
        completion_rate,
        created_at: achievement.created_at,
    };

    Ok(Some(achievement_with_stats))
}

// 同步成就統計數據 - 重建所有成就的統計記錄
async fn sync_achievement_stats(rb: &RBatis) -> rbatis::Result<i32> {
    let now = Utc::now();
    let mut synced_count = 0;

    // 獲取所有成就
    let achievements = Achievement::select_all(rb).await?;

    for achievement in achievements {
        let achievement_id = match &achievement.id {
            Some(id) => id,
            None => continue,
        };

        // 統計該成就被多少用戶完成
        let sql = "SELECT COUNT(*) as count FROM user_achievement WHERE achievement_id = ?";
        let result: Vec<serde_json::Value> = rb.query_decode(sql, vec![Value::String(achievement_id.clone())]).await?;

        let completion_count = if let Some(row) = result.first() {
            row.get("count").and_then(|v| v.as_i64()).unwrap_or(0) as i32
        } else {
            0
        };

        // 檢查是否已存在統計記錄
        let existing_stats = AchievementStats::select_by_map(rb, value!{"achievement_id": achievement_id}).await?;

        if let Some(existing_stat) = existing_stats.first() {
            // 更新現有記錄
            let update_sql = "UPDATE achievement_stats SET completion_count = ?, updated_at = ? WHERE achievement_id = ?";
            rb.exec(update_sql, vec![
                Value::from(completion_count),
                Value::String(now.to_rfc3339()),
                Value::String(achievement_id.clone())
            ]).await?;
        } else {
            // 創建新記錄
            let new_stat = AchievementStats {
                id: Some(Uuid::new_v4().to_string()),
                achievement_id: Some(achievement_id.clone()),
                completion_count: Some(completion_count),
                created_at: Some(now),
                updated_at: Some(now),
            };
            AchievementStats::insert(rb, &new_stat).await?;
        }

        synced_count += 1;
        log::info!("同步成就 {} 統計數據：完成人數 {}", achievement_id, completion_count);
    }

    log::info!("成就統計數據同步完成，共處理 {} 個成就", synced_count);
    Ok(synced_count)
}

// 同步成就統計數據的管理員 API
pub async fn sync_achievement_statistics(rb: web::Data<RBatis>) -> Result<HttpResponse> {
    log::info!("開始同步成就統計數據...");

    match sync_achievement_stats(rb.get_ref()).await {
        Ok(synced_count) => {
            Ok(HttpResponse::Ok().json(ApiResponse {
                success: true,
                data: Some(serde_json::json!({
                    "synced_achievements": synced_count,
                    "message": format!("成功同步 {} 個成就的統計數據", synced_count)
                })),
                message: format!("成就統計數據同步完成，共處理 {} 個成就", synced_count),
            }))
        }
        Err(e) => {
            log::error!("同步成就統計數據失敗: {}", e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("同步成就統計數據失敗: {}", e),
            }))
        }
    }
}

// ================= Task History API =================

/// 獲取用戶的任務完成歷史
/// GET /api/users/{user_id}/task-history?limit=5&offset=0&task_type=all
pub async fn get_task_history(
    rb: web::Data<RBatis>,
    path: web::Path<String>,
    query: web::Query<TaskHistoryQuery>,
) -> Result<HttpResponse> {
    let user_id = path.into_inner();

    log::info!(
        "獲取用戶 {} 的任務歷史，參數: limit={}, offset={}, task_type={}",
        user_id,
        query.limit,
        query.offset,
        query.task_type
    );

    // 構建 SQL 查詢
    let base_sql = "
        SELECT id, title, task_type, updated_at, experience
        FROM task
        WHERE user_id = ?
          AND status IN (2, 6)
    ";

    // 根據任務類型添加過濾條件
    let filter_sql = if query.task_type != "all" {
        format!("{} AND task_type = ?", base_sql)
    } else {
        base_sql.to_string()
    };

    let order_limit_sql = format!(
        "{} ORDER BY updated_at DESC LIMIT ? OFFSET ?",
        filter_sql
    );

    // 構建計數查詢
    let count_sql = if query.task_type != "all" {
        format!(
            "SELECT COUNT(*) as count FROM task WHERE user_id = ? AND status IN (2, 6) AND task_type = ?"
        )
    } else {
        "SELECT COUNT(*) as count FROM task WHERE user_id = ? AND status IN (2, 6)".to_string()
    };

    // 執行查詢
    let tasks_result = if query.task_type != "all" {
        rb.query_decode::<Vec<Task>>(
            &order_limit_sql,
            vec![
                Value::from(user_id.as_str()),
                Value::from(query.task_type.as_str()),
                Value::from(query.limit),
                Value::from(query.offset),
            ],
        )
        .await
    } else {
        rb.query_decode::<Vec<Task>>(
            &order_limit_sql,
            vec![
                Value::from(user_id.as_str()),
                Value::from(query.limit),
                Value::from(query.offset),
            ],
        )
        .await
    };

    // 執行計數查詢
    let count_result = if query.task_type != "all" {
        rb.query_decode::<Vec<serde_json::Value>>(
            &count_sql,
            vec![Value::from(user_id.as_str()), Value::from(query.task_type.as_str())],
        )
        .await
    } else {
        rb.query_decode::<Vec<serde_json::Value>>(&count_sql, vec![Value::from(user_id.as_str())])
            .await
    };

    match (tasks_result, count_result) {
        (Ok(tasks), Ok(count_rows)) => {
            let total_count = count_rows
                .first()
                .and_then(|row| row.get("count"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            // 轉換為 TaskHistoryItem
            let history_items: Vec<TaskHistoryItem> = tasks
                .iter()
                .filter_map(|task| {
                    Some(TaskHistoryItem {
                        id: task.id.clone()?,
                        title: task.title.clone().unwrap_or_default(),
                        task_type: task.task_type.clone().unwrap_or_default(),
                        completed_at: task.updated_at?,
                        experience: task.experience.unwrap_or(0),
                    })
                })
                .collect();

            let has_more = (query.offset + query.limit) < total_count;

            let response = TaskHistoryResponse {
                tasks: history_items,
                total_count,
                has_more,
            };

            Ok(HttpResponse::Ok().json(ApiResponse {
                success: true,
                data: Some(response),
                message: "獲取任務歷史成功".to_string(),
            }))
        }
        (Err(e), _) | (_, Err(e)) => {
            log::error!("獲取任務歷史失敗: {}", e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("獲取任務歷史失敗: {}", e),
            }))
        }
    }
}

// ============= AI 技能標籤生成 =============

#[derive(serde::Deserialize)]
pub struct GenerateSkillTagsRequest {
    pub task_title: String,
    pub task_description: Option<String>,
    pub user_id: String,
}

#[derive(serde::Serialize)]
pub struct GenerateSkillTagsResponse {
    pub skills: Vec<String>,
    pub reasoning: Option<String>,
}

/// AI 生成技能標籤
pub async fn generate_skill_tags(
    rb: web::Data<RBatis>,
    req: web::Json<GenerateSkillTagsRequest>,
) -> Result<HttpResponse> {
    log::info!("📝 收到技能標籤生成請求 - 任務: {}", req.task_title);

    // 載入 AI 配置
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

    // 獲取使用者現有的技能列表
    let user_existing_skills: Vec<String> = match Skill::select_by_map(
        rb.get_ref(),
        value!{"user_id": &req.user_id}
    ).await {
        Ok(skills) => skills
            .iter()
            .filter_map(|s| s.name.clone())
            .collect(),
        Err(e) => {
            log::warn!("獲取使用者技能失敗，將使用空列表: {}", e);
            Vec::new()
        }
    };

    log::debug!("使用者現有技能: {:?}", user_existing_skills);

    // 調用 AI 生成技能標籤
    match ai_service.generate_skill_tags(
        &req.task_title,
        req.task_description.as_deref(),
        &user_existing_skills
    ).await {
        Ok(result) => {
            log::info!("✅ 成功生成技能標籤: {:?}", result.skills);
            Ok(HttpResponse::Ok().json(ApiResponse {
                success: true,
                data: Some(GenerateSkillTagsResponse {
                    skills: result.skills,
                    reasoning: result.reasoning,
                }),
                message: "成功生成技能標籤".to_string(),
            }))
        }
        Err(e) => {
            log::error!("生成技能標籤失敗: {}", e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("生成技能標籤失敗: {}", e),
            }))
        }
    }
}


