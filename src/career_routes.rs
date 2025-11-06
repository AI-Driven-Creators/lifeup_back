use actix_web::{web, HttpResponse, Result};
use rbatis::RBatis;
use uuid::Uuid;
use chrono::Utc;
use serde_json;
use log;
use rbs::{Value, value};

use crate::models::{
    QuizResults, CareerMainlines, Task, ChatMessage, User,
    SaveQuizResultsRequest, GenerateCareerTasksRequest,
    GeneratedTasksResponse, GeneratedTask, SurveyAnswers, SkillTag
};
use crate::ai_tasks::ApiResponse;

// ============= 測驗結果相關 API =============

pub async fn save_quiz_results(
    rb: web::Data<RBatis>,
    request: web::Json<SaveQuizResultsRequest>
) -> Result<HttpResponse> {
    log::info!("開始保存測驗結果");
    log::info!("測驗結果數據: {:?}", &*request);

    let quiz_id = Uuid::new_v4().to_string();
    log::info!("UUID 生成成功: {}", quiz_id);
    log::info!("開始查詢用戶...");
    // 使用第一個用戶ID（與任務系統保持一致）
    let user_id = match rb.query_decode::<Vec<crate::models::User>>("SELECT id FROM user LIMIT 1", vec![]).await {
        Ok(users) => {
            log::info!("查詢到 {} 個用戶", users.len());
            if users.is_empty() {
                log::error!("系統中沒有用戶");
                return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    message: "系統中沒有用戶，請先創建用戶".to_string(),
                }));
            }
            let user_id = users[0].id.clone().unwrap_or_default();
            log::info!("使用用戶ID: {}", user_id);
            user_id
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

    // 創建測驗結果記錄，使用錯誤處理來避免直接崩潰
    let values_json = match serde_json::to_string(&request.values_results) {
        Ok(json) => json,
        Err(e) => {
            log::error!("序列化價值觀結果失敗: {}", e);
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: "價值觀測驗結果數據格式錯誤".to_string(),
            }));
        }
    };

    let interests_json = match serde_json::to_string(&request.interests_results) {
        Ok(json) => json,
        Err(e) => {
            log::error!("序列化興趣結果失敗: {}", e);
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: "興趣測驗結果數據格式錯誤".to_string(),
            }));
        }
    };

    let talents_json = match serde_json::to_string(&request.talents_results) {
        Ok(json) => json,
        Err(e) => {
            log::error!("序列化天賦結果失敗: {}", e);
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: "天賦測驗結果數據格式錯誤".to_string(),
            }));
        }
    };

    let workstyle_json = match serde_json::to_string(&request.workstyle_results) {
        Ok(json) => json,
        Err(e) => {
            log::error!("序列化工作風格結果失敗: {}", e);
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: "工作風格測驗結果數據格式錯誤".to_string(),
            }));
        }
    };

    let quiz_result = QuizResults {
        id: Some(quiz_id.clone()),
        user_id: Some(user_id.clone()),
        values_results: Some(values_json),
        interests_results: Some(interests_json),
        talents_results: Some(talents_json),
        workstyle_results: Some(workstyle_json),
        completed_at: Some(now),
        is_active: Some(1),
        created_at: Some(now),
        updated_at: None,
    };

    // 注意：不再自動停用舊測驗結果，允許多個測驗結果並存
    // 這樣可以避免多用戶同時測驗時互相干擾
    // 如果需要清理舊數據，應該通過定時任務或手動操作

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

    // 獲取用戶ID - 優先使用請求中的 user_id，否則使用第一個用戶
    let user_id = if let Some(uid) = &request.user_id {
        uid.clone()
    } else {
        match rb.query_decode::<Vec<crate::models::User>>("SELECT id FROM user LIMIT 1", vec![]).await {
            Ok(users) => {
                if users.is_empty() {
                    return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        message: "缺少 user_id 參數".to_string(),
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

    // 將提示詞保存到 last_prompt.md
    if let Err(e) = std::fs::write("last_prompt.md", &ai_prompt) {
        log::error!("❌ 寫入 last_prompt.md 失敗: {}", e);
    } else {
        log::info!("✅ 已將 AI 提示詞保存到 last_prompt.md");
    }

    // 3. 調用 AI 服務生成任務
    let generation_start = std::time::Instant::now();
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
    // 將完整的 AI 回應輸出到 bug.json
    if let Err(write_err) = std::fs::write("last.json", &ai_response) {
        log::error!("❌ 寫入 last.json 失敗: {}", write_err);
    } else {
        log::info!("✅ 已將完整 AI 回應輸出到 last.json");
    }
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

    // 4.5. 處理預覽數據：添加經驗值
    log::info!("✅ 任務生成完成，處理預覽數據（添加經驗值）");

    // 為每個任務添加經驗值（根據難度計算）
    let process_task = |task: GeneratedTask| -> serde_json::Value {
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
    let processed_main_tasks: Vec<serde_json::Value> = generated_tasks.main_tasks
        .into_iter()
        .map(process_task)
        .collect();

    let processed_daily_tasks: Vec<serde_json::Value> = generated_tasks.daily_tasks
        .into_iter()
        .map(process_task)
        .collect();

    let processed_project_tasks: Vec<serde_json::Value> = generated_tasks.project_tasks
        .into_iter()
        .map(process_task)
        .collect();

    let total_tasks = processed_main_tasks.len() +
                     processed_daily_tasks.len() +
                     processed_project_tasks.len();

    log::info!("✅ 預覽數據處理完成，共 {} 個任務", total_tasks);

    // 返回預覽數據供前端顯示
    return Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        data: Some(serde_json::json!({
            "preview_mode": true,
            "quiz_result_id": request.quiz_result_id,
            "selected_career": request.selected_career,
            "user_id": user_id,
            "survey_answers": request.survey_answers,
            "learning_summary": generated_tasks.learning_summary,
            "personality_insights": generated_tasks.personality_insights,
            "estimated_months": generated_tasks.estimated_months,
            "total_tasks": total_tasks,
            "main_tasks": processed_main_tasks,
            "daily_tasks": processed_daily_tasks,
            "project_tasks": processed_project_tasks,
        })),
        message: "任務預覽生成成功，請確認是否接受".to_string(),
    }));
}

// 新增：接受並保存職業任務的 API
pub async fn accept_career_tasks(
    rb: web::Data<RBatis>,
    request: web::Json<serde_json::Value>
) -> Result<HttpResponse> {
    log::info!("用戶接受職業任務，開始保存到資料庫");

    // 解析請求數據
    let quiz_result_id = request["quiz_result_id"].as_str().unwrap_or_default().to_string();
    let selected_career = request["selected_career"].as_str().unwrap_or_default().to_string();
    let user_id = request["user_id"].as_str().unwrap_or_default().to_string();

    let survey_answers: SurveyAnswers = serde_json::from_value(request["survey_answers"].clone())
        .unwrap_or_default();

    let learning_summary = request["learning_summary"].as_str().unwrap_or_default().to_string();
    let estimated_months = request["estimated_months"].as_i64().unwrap_or(6) as i32;

    let main_tasks: Vec<GeneratedTask> = serde_json::from_value(request["main_tasks"].clone()).unwrap_or_default();
    let daily_tasks: Vec<GeneratedTask> = serde_json::from_value(request["daily_tasks"].clone()).unwrap_or_default();
    let project_tasks: Vec<GeneratedTask> = serde_json::from_value(request["project_tasks"].clone()).unwrap_or_default();

    // 解析生成的成就數據
    let achievements_data = request.get("achievements")
        .and_then(|v| v.get("achievements"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    log::info!("📊 解析到 {} 個AI生成的成就", achievements_data.len());

    let total_tasks = main_tasks.len() + daily_tasks.len() + project_tasks.len();

    // 檢查是否已經為此測驗結果和職業生成過任務 - 如果有則先刪除
    let existing_check = rb.query_decode::<Vec<CareerMainlines>>(
        "SELECT * FROM career_mainlines WHERE quiz_result_id = ? AND selected_career = ?",
        vec![
            rbs::to_value!(quiz_result_id.clone()),
            rbs::to_value!(selected_career.clone()),
        ],
    ).await;

    if let Ok(existing) = existing_check {
        for old_mainline in existing {
            if let Some(old_id) = &old_mainline.id {
                log::info!("刪除舊的職業主線任務: {}", old_id);
                // 刪除關聯的任務
                let _ = rb.exec("DELETE FROM task WHERE career_mainline_id = ?", vec![rbs::to_value!(old_id.clone())]).await;
                // 刪除職業主線記錄
                let _ = rb.exec("DELETE FROM career_mainlines WHERE id = ?", vec![rbs::to_value!(old_id.clone())]).await;
            }
        }
    }

    // 5. 創建職業主線記錄
    let mainline_id = Uuid::new_v4().to_string();

    let career_mainline = CareerMainlines {
        id: Some(mainline_id.clone()),
        user_id: Some(user_id.clone()),
        quiz_result_id: Some(quiz_result_id.clone()),
        selected_career: Some(selected_career.clone()),
        survey_answers: Some(serde_json::to_string(&survey_answers)?),
        total_tasks_generated: Some(total_tasks as i32),
        estimated_completion_months: Some(estimated_months),
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
        title: Some(format!("職業主線：{}", selected_career)),
        description: Some(format!("{}\n\n📋 包含 {} 個子任務，完成後將掌握相關職業技能。\n\n🎯 預計學習時程：{} 個月",
                                learning_summary,
                                total_tasks,
                                estimated_months)),
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
        skill_tags: {
            // 聚合所有子任務的技能標籤（只取名稱）
            let mut all_skills: std::collections::HashSet<String> = std::collections::HashSet::new();
            for task in &main_tasks {
                for skill in &task.skill_tags {
                    all_skills.insert(skill.name.clone());
                }
            }
            for task in &daily_tasks {
                for skill in &task.skill_tags {
                    all_skills.insert(skill.name.clone());
                }
            }
            for task in &project_tasks {
                for skill in &task.skill_tags {
                    all_skills.insert(skill.name.clone());
                }
            }
            if all_skills.is_empty() {
                None
            } else {
                Some(all_skills.into_iter().collect())
            }
        },
        attributes: None,
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
    for ai_task in &main_tasks {
        match create_subtask_from_ai_data(&rb, &user_id, &mainline_id, &parent_task_id, ai_task, "career_subtask", task_order).await {
            Ok(task) => {
                created_tasks.push(task);
                task_order += 1;
            }
            Err(e) => log::error!("創建學習子任務失敗: {}", e),
        }
    }

    // 創建每日任務（作為子任務）
    for ai_task in &daily_tasks {
        match create_subtask_from_ai_data(&rb, &user_id, &mainline_id, &parent_task_id, ai_task, "career_subtask", task_order).await {
            Ok(task) => {
                created_tasks.push(task);
                task_order += 1;
            }
            Err(e) => log::error!("創建學習子任務失敗: {}", e),
        }
    }

    // 創建項目任務（作為子任務）
    for ai_task in &project_tasks {
        match create_subtask_from_ai_data(&rb, &user_id, &mainline_id, &parent_task_id, ai_task, "career_subtask", task_order).await {
            Ok(task) => {
                created_tasks.push(task);
                task_order += 1;
            }
            Err(e) => log::error!("創建學習子任務失敗: {}", e),
        }
    }

    log::info!("✅ 成功創建 {} 個任務", created_tasks.len());

    // 更新父任務的經驗值為所有子任務經驗值總和
    if let Err(e) = crate::routes::update_parent_task_experience(rb.get_ref(), &parent_task_id).await {
        log::warn!("更新父任務經驗值時發生錯誤: {}", e);
    }

    // 7. 保存AI生成的成就到資料庫
    let mut saved_achievements = 0;
    for ach_data in &achievements_data {
        if let (Some(name), Some(description), Some(icon)) = (
            ach_data.get("name").and_then(|v| v.as_str()),
            ach_data.get("description").and_then(|v| v.as_str()),
            ach_data.get("icon").and_then(|v| v.as_str()),
        ) {
            let experience_reward = ach_data.get("experience_reward")
                .and_then(|v| v.as_i64())
                .unwrap_or(50) as i32;

            let category = ach_data.get("category")
                .and_then(|v| v.as_str())
                .unwrap_or("career_specific");

            let related_task_title = ach_data.get("related_task_title")
                .and_then(|v| v.as_str());

            // 尋找對應的任務ID
            let related_task_id = if let Some(task_title) = related_task_title {
                created_tasks.iter()
                    .find(|t| t.title.as_ref().map(|s| s.as_str()) == Some(task_title))
                    .and_then(|t| t.id.clone())
            } else {
                None
            };

            let achievement = crate::models::Achievement {
                id: Some(Uuid::new_v4().to_string()),
                name: Some(name.to_string()),
                description: Some(description.to_string()),
                icon: Some(icon.to_string()),
                category: Some(category.to_string()),
                requirement_type: None,  // 職業專屬成就不使用傳統的需求類型
                requirement_value: None,
                experience_reward: Some(experience_reward),
                career_mainline_id: Some(mainline_id.clone()),
                related_task_id,
                created_at: Some(Utc::now()),
            };

            match crate::models::Achievement::insert(rb.get_ref(), &achievement).await {
                Ok(_) => {
                    saved_achievements += 1;
                    log::info!("✅ 保存成就: {}", name);
                }
                Err(e) => log::error!("❌ 保存成就失敗: {} - {}", name, e),
            }
        }
    }

    log::info!("🏆 成功保存 {} 個職業專屬成就", saved_achievements);

    // 8. 記錄到聊天記錄（作為 AI 互動記錄）
    let chat_message = crate::models::ChatMessage {
        id: Some(Uuid::new_v4().to_string()),
        user_id: Some(user_id.clone()),
        role: Some("assistant".to_string()),
        content: Some(format!("為您的「{}」職業規劃生成了 {} 個學習任務：\n\n{}",
                             selected_career,
                             created_tasks.len(),
                             learning_summary)),
        created_at: Some(Utc::now()),
    };

    if let Err(e) = ChatMessage::insert(rb.get_ref(), &chat_message).await {
        log::warn!("保存聊天記錄失敗: {}", e);
    }

    // 9. 返回成功回應
    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        data: Some(serde_json::json!({
            "mainline_id": mainline_id,
            "parent_task_id": parent_task_id,
            "parent_task": {
                "id": parent_task_id,
                "title": format!("職業主線：{}", selected_career),
                "description": format!("{}\n\n📋 包含 {} 個子任務，完成後將掌握相關職業技能。",
                                     learning_summary, total_tasks),
                "subtasks_count": created_tasks.len()
            },
            "subtasks_created": created_tasks.len(),
            "achievements_created": saved_achievements,
            "learning_summary": learning_summary,
            "estimated_months": estimated_months,
            "subtasks": created_tasks
        })),
        message: format!("🎉 成功創建職業主線「{}」，包含 {} 個子任務！", selected_career, created_tasks.len()),
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

pub async fn get_quiz_result(rb: &RBatis, quiz_result_id: &str) -> Result<QuizResults, Box<dyn std::error::Error>> {
    log::info!("🔍 查詢測驗結果，ID: {}", quiz_result_id);

    let sql = "SELECT id, user_id, values_results, interests_results, talents_results, workstyle_results, completed_at, is_active, created_at FROM quiz_results WHERE id = ? AND is_active = 1";

    // 先用原始查詢獲取數據
    let raw_results: Vec<serde_json::Value> = rb.query_decode(sql, vec![rbs::Value::String(quiz_result_id.to_string())]).await?;

    log::info!("📊 查詢結果數量: {}", raw_results.len());
    
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
            updated_at: None, // 這個查詢沒有包含 updated_at，設為 None
        };
        log::info!("✅ 成功找到測驗結果");
        Ok(quiz_result)
    } else {
        log::error!("❌ 測驗結果不存在，ID: {}", quiz_result_id);

        // 嘗試不帶 is_active 條件查詢，看看是否存在但 is_active 不是 1
        let sql_debug = "SELECT id, is_active FROM quiz_results WHERE id = ?";
        if let Ok(debug_results) = rb.query_decode::<Vec<serde_json::Value>>(sql_debug, vec![rbs::Value::String(quiz_result_id.to_string())]).await {
            if debug_results.is_empty() {
                log::error!("❌ 測驗結果完全不存在（包括已停用的）");
            } else {
                log::error!("⚠️ 測驗結果存在但 is_active 不是 1: {:?}", debug_results);
            }
        }

        Err("測驗結果不存在或已過期".into())
    }
}

pub fn build_career_task_prompt(
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

請生成學習任務，分為三類，總共 12 個任務：

### 1. 主線任務 (8個)
- 核心技能學習，難度循序漸進
- 每個任務都有明確的學習成果
- 根據用戶個性特質調整學習方式
- 從基礎到進階，形成完整的學習路徑

### 2. 每日任務 (2個)
- 培養職業相關的日常習慣
- 每個任務15-30分鐘可完成
- 重複執行有助於技能累積
- 涵蓋不同方面的日常練習

### 3. 項目任務 (2個)
- 實戰練習和作品集建立
- 難度較高，需要綜合運用所學
- 有助於建立職業競爭力
- 提供不同類型的實戰經驗

## 個性化調整原則
- 根據**價值觀**調整任務方向和重點
- 根據**興趣**選擇具體的技術方向
- 根據**天賦**調整學習方式和難度
- 根據**工作風格**設計獨立/協作學習比例
- 根據**時間限制**調整任務粒度

## 屬性值分配原則

每個任務必須分配屬性獎勵，根據任務類型和難度：

### 六大屬性
- **intelligence** (智力): 學習、分析、理論研究相關任務
- **creativity** (創造力): 設計、創新、解決方案相關任務
- **focus** (專注力): 需要長時間專注的技術任務
- **endurance** (毅力): 長期、重複性、需要堅持的任務
- **social** (社交力): 團隊協作、溝通、人際互動任務
- **adaptability** (適應力): 學習新技術、應對變化的任務

### 分配規則
1. **每個任務選擇 1-2 個最相關的屬性**
2. **必須根據任務的實際內容選擇屬性，避免重複使用相同的屬性組合**：
   - 理論學習、概念理解 → intelligence
   - 創意設計、方案規劃 → creativity
   - 長時間學習、技術操作 → focus
   - 日常練習、持續執行 → endurance
   - 團隊合作、溝通表達 → social
   - 新技術學習、環境適應 → adaptability
3. **屬性值根據難度計算**（注意：使用者屬性滿分為 100，請謹慎分配）：
   - 難度 1: 單個屬性值 1-2
   - 難度 2: 單個屬性值 2-3
   - 難度 3: 單個屬性值 3-4
   - 難度 4: 單個屬性值 4-5
   - 難度 5: 單個屬性值 5-6
4. **一個任務的所有屬性值總和不應超過 8**
5. **整個任務列表必須涵蓋多種不同的屬性組合，不要重複使用相同的屬性**

## 嚴格 JSON 格式要求

**重要：**
1. 回應必須是有效的JSON格式，不包含額外文字
2. 所有字符串必須用雙引號包圍
3. 不能有尾隨逗號
4. 所有必需字段都必須存在
5. difficulty 必須是 1-5 的整數
6. estimated_hours 必須是正整數
7. skill_tags 和 resources 必須是字符串陣列
8. **attributes 必須是物件，包含 1-2 個屬性及其數值**

```json
{{
  "learning_summary": "基於用戶特質的學習路徑總結，說明整體規劃思路",
  "estimated_months": 8,
  "personality_insights": "個性特質如何影響這個學習計劃的分析",
  "main_tasks": [
    {{
      "title": "主線任務標題1（理論學習）",
      "description": "任務總體說明。\\n\\n【學習目標】\\n具體要達成的學習目標。\\n\\n【執行步驟】\\n1. 第一步具體要做什麼\\n2. 第二步具體要做什麼\\n3. 第三步具體要做什麼\\n\\n【完成標準】\\n如何判斷任務完成。",
      "difficulty": 2,
      "estimated_hours": 15,
      "skill_tags": [{{"name": "核心技能1", "category": "technical"}}],
      "resources": ["學習資源1", "學習資源2"],
      "attributes": {{"intelligence": 2, "adaptability": 2}}
    }},
    {{
      "title": "主線任務標題2（創意設計）",
      "description": "任務總體說明。\\n\\n【學習目標】\\n具體要達成的學習目標。\\n\\n【執行步驟】\\n1. 第一步具體要做什麼\\n2. 第二步具體要做什麼\\n3. 第三步具體要做什麼\\n\\n【完成標準】\\n如何判斷任務完成。",
      "difficulty": 3,
      "estimated_hours": 20,
      "skill_tags": [{{"name": "核心技能2", "category": "technical"}}],
      "resources": ["學習資源3", "學習資源4"],
      "attributes": {{"creativity": 4}}
    }},
    {{
      "title": "主線任務標題3（技術實作）",
      "description": "任務總體說明。\\n\\n【學習目標】\\n具體要達成的學習目標。\\n\\n【執行步驟】\\n1. 第一步具體要做什麼\\n2. 第二步具體要做什麼\\n3. 第三步具體要做什麼\\n\\n【完成標準】\\n如何判斷任務完成。",
      "difficulty": 4,
      "estimated_hours": 25,
      "skill_tags": [{{"name": "核心技能3", "category": "technical"}}],
      "resources": ["學習資源5"],
      "attributes": {{"focus": 4, "intelligence": 2}}
    }},
    {{
      "title": "主線任務標題4（團隊協作）",
      "description": "任務總體說明。\\n\\n【學習目標】\\n具體要達成的學習目標。\\n\\n【執行步驟】\\n1. 第一步具體要做什麼\\n2. 第二步具體要做什麼\\n3. 第三步具體要做什麼\\n\\n【完成標準】\\n如何判斷任務完成。",
      "difficulty": 3,
      "estimated_hours": 18,
      "skill_tags": [{{"name": "核心技能4", "category": "soft"}}],
      "resources": ["學習資源6"],
      "attributes": {{"social": 3, "creativity": 2}}
    }}
  ],
  "daily_tasks": [
    {{
      "title": "每日任務標題1（日常練習）",
      "description": "任務總體說明。\\n\\n【學習目標】\\n具體要達成的學習目標。\\n\\n【執行步驟】\\n1. 第一步具體要做什麼\\n2. 第二步具體要做什麼\\n\\n【完成標準】\\n如何判斷任務完成。",
      "difficulty": 2,
      "estimated_hours": 1,
      "skill_tags": [{{"name": "日常技能1", "category": "soft"}}],
      "resources": ["資源1"],
      "attributes": {{"endurance": 2}}
    }}
  ],
  "project_tasks": [
    {{
      "title": "項目任務標題1（綜合應用）",
      "description": "任務總體說明。\\n\\n【學習目標】\\n具體要達成的學習目標。\\n\\n【執行步驟】\\n1. 第一步具體要做什麼\\n2. 第二步具體要做什麼\\n3. 第三步具體要做什麼\\n4. 第四步具體要做什麼\\n\\n【完成標準】\\n如何判斷任務完成。",
      "difficulty": 5,
      "estimated_hours": 40,
      "skill_tags": [{{"name": "實戰技能1", "category": "technical"}}, {{"name": "綜合技能2", "category": "soft"}}],
      "resources": ["項目資源1", "項目資源2"],
      "attributes": {{"creativity": 5, "adaptability": 3}}
    }}
  ]
}}
```

**請嚴格按照上述JSON格式回應，確保每個任務對象都包含所有必需字段：title, description, difficulty, estimated_hours, skill_tags, resources, attributes。

**description 欄位要求：**
- 必須包含詳細的執行說明，讓使用者清楚知道「要做什麼」和「怎麼做」
- 必須使用以下格式（使用 \\n 換行符號）：
  "任務總體說明。\\n\\n【學習目標】\\n具體要達成的學習目標。\\n\\n【執行步驟】\\n1. 第一步\\n2. 第二步\\n3. 第三步\\n\\n【完成標準】\\n如何判斷任務完成。"
- 執行步驟要具體且可操作，避免空泛的描述
- 每個段落之間使用 \\n\\n 分隔，每個步驟使用 \\n 分隔

重要提醒：
- **attributes 欄位是必需的**，必須包含 1-2 個屬性及其數值（根據上述分配規則）
- **嚴禁在所有任務中重複使用相同的屬性組合**，必須根據任務實際內容選擇最合適的屬性
- 12 個子任務必須盡可能涵蓋所有六大屬性（intelligence、creativity、focus、endurance、social、adaptability）
- skill_tags 現在必須是物件陣列格式，每個技能包含 name（技能名稱）和 category（分類）
- category 只能是 "technical"（技術技能）或 "soft"（軟技能）
- 技術技能包括：程式語言、開發工具、技術操作、硬體知識、數學概念等
- 軟技能包括：溝通、領導、分析思考、時間管理、創意思維等
- **每個任務都必須有 estimated_hours 欄位**，用於計算經驗值

**語言要求：**
- **必須使用繁體中文**，絕對不可以出現任何簡體字
- 所有內容包括：title、description、skill_tags、resources 等都必須是繁體中文
- JSON 結構必須完全符合格式要求**
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

pub fn extract_quiz_summary(quiz_json: &Option<String>) -> String {
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

/// 修復 JSON 字符串值中未轉義的雙引號
/// 例如："description": "完成"SQL課程"學習" => "description": "完成\"SQL課程\"學習"
fn fix_unescaped_quotes(json_str: &str) -> String {
    let mut result = String::with_capacity(json_str.len() + 100);
    let chars: Vec<char> = json_str.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        // 找到字符串值的開始（格式為 ": "）
        if ch == ':' && i + 1 < chars.len() {
            result.push(ch);
            i += 1;

            // 跳過空白
            while i < chars.len() && chars[i].is_whitespace() {
                result.push(chars[i]);
                i += 1;
            }

            // 如果是字符串開始
            if i < chars.len() && chars[i] == '"' {
                result.push('"');
                i += 1;

                // 處理字符串內容
                let mut escaped = false;
                while i < chars.len() {
                    let c = chars[i];

                    if escaped {
                        // 前一個字符是反斜杠,當前字符已經被轉義
                        result.push(c);
                        escaped = false;
                        i += 1;
                        continue;
                    }

                    if c == '\\' {
                        // 遇到反斜杠,下一個字符被轉義
                        result.push(c);
                        escaped = true;
                        i += 1;
                        continue;
                    }

                    if c == '"' {
                        // 檢查這是否是字符串結束符
                        // 判斷方法:看後面是否跟著 , 或 } 或 ] 或換行
                        let mut j = i + 1;
                        while j < chars.len() && chars[j].is_whitespace() {
                            j += 1;
                        }

                        if j >= chars.len() || chars[j] == ',' || chars[j] == '}' || chars[j] == ']' {
                            // 這是字符串結束符
                            result.push('"');
                            i += 1;
                            break;
                        } else {
                            // 這是字符串中間的未轉義引號,需要轉義
                            result.push('\\');
                            result.push('"');
                            i += 1;
                        }
                    } else {
                        result.push(c);
                        i += 1;
                    }
                }
            } else {
                // 不是字符串,直接複製
                if i < chars.len() {
                    result.push(chars[i]);
                    i += 1;
                }
            }
        } else {
            result.push(ch);
            i += 1;
        }
    }

    result
}

pub fn parse_ai_tasks_response(ai_response: &str) -> Result<GeneratedTasksResponse, Box<dyn std::error::Error>> {
    // 清理 AI 回應，移除可能的 markdown 標記和多餘空白
    let mut cleaned_response = ai_response
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string();

    // 修復常見的 JSON 格式問題
    // 1. 將中文引號替換為轉義後的引號
    cleaned_response = cleaned_response
        .replace("\u{201C}", "\\\"")  // 左雙引號 "
        .replace("\u{201D}", "\\\"")  // 右雙引號 "
        .replace("\u{2018}", "'")     // 左單引號 '
        .replace("\u{2019}", "'");    // 右單引號 '

    // 2. 修復 JSON 字符串中未轉義的雙引號
    // 這是最常見的問題：AI 在 description 等欄位中使用了未轉義的 "
    cleaned_response = fix_unescaped_quotes(&cleaned_response);

    let preview = cleaned_response.chars().take(500).collect::<String>();
    log::debug!("清理並修復後的 AI 回應前500字符: {}", preview);

    // 檢查是否為有效 JSON 開頭
    if !cleaned_response.starts_with('{') {
        log::error!("❌ AI 回應不是有效的 JSON 格式，未以 {{ 開頭");
        log::error!("前 200 個字符: {}", safe_substring(&cleaned_response, 0, 200));

        // 將錯誤 JSON 輸出到 bug.json
        if let Err(e) = std::fs::write("bug.json", &cleaned_response) {
            log::error!("❌ 寫入 bug.json 失敗: {}", e);
        } else {
            log::info!("✅ 已將錯誤 JSON 輸出到 bug.json");
        }

        return Err("AI 回應格式錯誤：不是有效的 JSON".into());
    }

    // 嘗試解析 JSON
    match serde_json::from_str::<GeneratedTasksResponse>(&cleaned_response) {
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

            // 將錯誤 JSON 輸出到 bug.json
            if let Err(write_err) = std::fs::write("bug.json", &cleaned_response) {
                log::error!("❌ 寫入 bug.json 失敗: {}", write_err);
            } else {
                log::info!("✅ 已將錯誤 JSON 輸出到 bug.json");
            }

            // 記錄更多調試信息（安全截斷字符串）
            let response_len = cleaned_response.len();
            let first_500 = safe_substring(&cleaned_response, 0, 500);
            let last_500 = if response_len > 500 {
                safe_substring(&cleaned_response, response_len.saturating_sub(500), response_len)
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
        description: Some(ai_task.description.clone()),
        status: Some(0), // pending
        priority: Some(ai_task.difficulty),
        task_type: Some(task_category.to_string()),
        difficulty: Some(ai_task.difficulty),
        experience: Some(experience),
        career_mainline_id: Some(mainline_id.to_string()),
        task_category: Some(task_category.to_string()),
        skill_tags: {
            // 將SkillTag陣列轉換為字串陣列的JSON
            let skill_names: Vec<String> = ai_task.skill_tags.iter().map(|s| s.name.clone()).collect();
            Some(skill_names)
        },
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
        attributes: ai_task.attributes.clone(),
    };

    // 在保存任務之前，先確保所有技能標籤都存在於技能表中
    log::info!("🔧 開始確保技能存在，任務: {}, 技能標籤數: {}", ai_task.title, ai_task.skill_tags.len());
    for skill_tag in &ai_task.skill_tags {
        log::info!("  - 技能: {} (類型: {})", skill_tag.name, skill_tag.category);
    }

    match ensure_skills_exist(rb, user_id, &ai_task.skill_tags).await {
        Ok(_) => {
            log::info!("✅ 所有技能已確保存在");
        }
        Err(e) => {
            log::error!("❌ 創建技能時發生錯誤: {}", e);
        }
    }

    // 保存到資料庫
    Task::insert(rb, &task).await?;
    log::debug!("✅ 創建任務: {} (類型: {}, 難度: {}, 技能標籤: {:?})",
               ai_task.title, task_category, ai_task.difficulty, ai_task.skill_tags);

    Ok(task)
}

// ============= 匯入職涯任務（由已產生的 JSON） =============

#[derive(serde::Deserialize)]
pub struct ImportCareerTasksRequest {
    pub selected_career: Option<String>,
    pub user_id: Option<String>,
    pub raw_json: String,
}

pub async fn import_career_tasks(
    rb: web::Data<RBatis>,
    req: web::Json<ImportCareerTasksRequest>
) -> Result<HttpResponse> {
    // 1) 解析 JSON
    let generated_tasks = match parse_ai_tasks_response(&req.raw_json) {
        Ok(tasks) => tasks,
        Err(e) => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()> {
                success: false,
                data: None,
                message: format!("JSON 解析失敗: {}", e),
            }));
        }
    };

    // 2) 準備 user_id（沿用 create-from-json 的策略：若未提供，使用/建立測試用戶）
    let user_id = if let Some(uid) = req.user_id.clone().filter(|s| !s.trim().is_empty()) {
        uid
    } else {
        match crate::models::User::select_by_map(rb.get_ref(), value!{"email": "test@lifeup.com"}).await {
            Ok(users) if !users.is_empty() => users[0].id.clone().unwrap_or_default(),
            _ => {
                let test_user = crate::models::User {
                    id: Some(uuid::Uuid::new_v4().to_string()),
                    name: Some("測試用戶".to_string()),
                    email: Some("test@lifeup.com".to_string()),
                    password_hash: Some("".to_string()),
                    created_at: Some(Utc::now()),
                    updated_at: Some(Utc::now()),
                };
                match crate::models::User::insert(rb.get_ref(), &test_user).await {
                    Ok(_) => test_user.id.unwrap(),
                    Err(e) => {
                        return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
                            success: false,
                            data: None,
                            message: format!("建立預設用戶失敗: {}", e),
                        }));
                    }
                }
            }
        }
    };

    // 3) 建立一筆 quiz_results（作為主線外鍵）
    let quiz_id = Uuid::new_v4().to_string();
    let quiz = crate::models::QuizResults {
        id: Some(quiz_id.clone()),
        user_id: Some(user_id.clone()),
        values_results: Some("{}".to_string()),
        interests_results: Some("{}".to_string()),
        talents_results: Some("{}".to_string()),
        workstyle_results: Some("{}".to_string()),
        completed_at: Some(Utc::now()),
        is_active: Some(1),
        created_at: Some(Utc::now()),
        updated_at: Some(Utc::now()),
    };
    if let Err(e) = crate::models::QuizResults::insert(rb.get_ref(), &quiz).await {
        log::error!("插入 quiz_results 失敗: {}", e);
        return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: "建立主線前置資料失敗".to_string(),
        }));
    }

    // 4) 建立 career_mainlines 記錄
    let mainline_id = Uuid::new_v4().to_string();
    let total_tasks = generated_tasks.main_tasks.len() + generated_tasks.daily_tasks.len() + generated_tasks.project_tasks.len();
    let career_name = req.selected_career.clone().unwrap_or_else(|| "CLI 導入主線".to_string());

    let career_mainline = crate::models::CareerMainlines {
        id: Some(mainline_id.clone()),
        user_id: Some(user_id.clone()),
        quiz_result_id: Some(quiz_id.clone()),
        selected_career: Some(career_name.clone()),
        survey_answers: None,
        total_tasks_generated: Some(total_tasks as i32),
        estimated_completion_months: Some(generated_tasks.estimated_months),
        status: Some("active".to_string()),
        progress_percentage: Some(0.0),
        created_at: Some(Utc::now()),
        updated_at: Some(Utc::now()),
    };
    if let Err(e) = crate::models::CareerMainlines::insert(rb.get_ref(), &career_mainline).await {
        log::error!("創建職業主線失敗: {}", e);
        return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: "創建主線失敗".to_string(),
        }));
    }

    // 5) 建立父任務
    let parent_task_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    let parent_task = Task {
        id: Some(parent_task_id.clone()),
        user_id: Some(user_id.clone()),
        title: Some(format!("職業主線：{}", career_name)),
        description: Some(format!("{}\n\n📋 包含 {} 個子任務，完成後將掌握相關職業技能。\n\n🎯 預計學習時程：{} 個月",
            generated_tasks.learning_summary,
            total_tasks,
            generated_tasks.estimated_months)),
        status: Some(0),
        priority: Some(2),
        task_type: Some("main".to_string()),
        difficulty: Some(3),
        experience: Some(100),
        career_mainline_id: Some(mainline_id.clone()),
        task_category: Some("career_mainline".to_string()),
        is_parent_task: Some(1),
        task_order: Some(0),
        created_at: Some(now),
        updated_at: Some(now),
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
        skill_tags: {
            let mut all = std::collections::HashSet::new();
            for t in &generated_tasks.main_tasks { for s in &t.skill_tags { all.insert(s.name.clone()); } }
            for t in &generated_tasks.daily_tasks { for s in &t.skill_tags { all.insert(s.name.clone()); } }
            for t in &generated_tasks.project_tasks { for s in &t.skill_tags { all.insert(s.name.clone()); } }
            if all.is_empty() { None } else { Some(all.into_iter().collect()) }
        },
        attributes: None,
    };
    if let Err(e) = Task::insert(rb.get_ref(), &parent_task).await {
        log::error!("創建父任務失敗: {}", e);
        return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()> {
            success: false,
            data: None,
            message: "創建父任務失敗".to_string(),
        }));
    }

    // 6) 逐一建立子任務
    let mut created_tasks = Vec::new();
    let mut task_order = 1;
    for ai_task in &generated_tasks.main_tasks {
        if let Ok(task) = create_subtask_from_ai_data(&rb, &user_id, &mainline_id, &parent_task_id, ai_task, "career_subtask", task_order).await {
            created_tasks.push(task);
            task_order += 1;
        }
    }
    for ai_task in &generated_tasks.daily_tasks {
        if let Ok(task) = create_subtask_from_ai_data(&rb, &user_id, &mainline_id, &parent_task_id, ai_task, "career_subtask", task_order).await {
            created_tasks.push(task);
            task_order += 1;
        }
    }
    for ai_task in &generated_tasks.project_tasks {
        if let Ok(task) = create_subtask_from_ai_data(&rb, &user_id, &mainline_id, &parent_task_id, ai_task, "career_subtask", task_order).await {
            created_tasks.push(task);
            task_order += 1;
        }
    }

    // 7) 更新父任務經驗值
    let _ = crate::routes::update_parent_task_experience(rb.get_ref(), &parent_task_id).await;

    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        data: Some(serde_json::json!({
            "mainline_id": mainline_id,
            "parent_task_id": parent_task_id,
            "subtasks_created": created_tasks.len(),
            "estimated_months": generated_tasks.estimated_months,
        })),
        message: format!("成功匯入 {} 個子任務", created_tasks.len()),
    }))
}

// 輔助函數：確保技能存在於技能表中
async fn ensure_skills_exist(rb: &RBatis, user_id: &str, skill_tags: &[SkillTag]) -> Result<(), Box<dyn std::error::Error>> {
    use crate::models::Skill;

    log::info!("📊 ensure_skills_exist 被調用，user_id: {}, 技能標籤數: {}", user_id, skill_tags.len());

    for skill_tag in skill_tags {
        let skill_name = &skill_tag.name;
        log::info!("  🔍 檢查技能: {}", skill_name);

        // 檢查技能是否已存在
        let existing_skills = Skill::select_by_map(rb, value!{
            "user_id": user_id,
            "name": skill_name
        }).await?;

        log::info!("  📋 查詢結果: 找到 {} 個同名技能", existing_skills.len());

        if existing_skills.is_empty() {
            // 技能不存在，創建新技能
            let skill_category = &skill_tag.category;  // 使用AI提供的分類
            log::info!("  🆕 技能不存在，準備創建: {} (類型: {})", skill_name, skill_category);

            let new_skill = Skill {
                id: Some(uuid::Uuid::new_v4().to_string()),
                user_id: Some(user_id.to_string()),
                name: Some(skill_name.clone()),
                description: Some(format!("通過任務自動創建的技能：{}", skill_name)),
                category: Some(skill_category.clone()),
                attribute: Some("intelligence".to_string()), // 默認屬性為智力
                level: Some(1),
                experience: Some(0),
                max_experience: Some(100),
                icon: Some("🎯".to_string()), // 默認圖標
                created_at: Some(chrono::Utc::now()),
                updated_at: Some(chrono::Utc::now()),
            };

            match Skill::insert(rb, &new_skill).await {
                Ok(_) => {
                    log::info!("  ✅ 成功創建技能: {} (ID: {}, 類型: {})",
                              skill_name,
                              new_skill.id.as_ref().unwrap_or(&"unknown".to_string()),
                              skill_category);
                }
                Err(e) => {
                    log::error!("  ❌ 創建技能 {} 失敗: {}", skill_name, e);
                    return Err(e.into());
                }
            }
        } else {
            log::info!("  ✓ 技能 {} 已存在 (ID: {}), 跳過創建",
                      skill_name,
                      existing_skills[0].id.as_ref().unwrap_or(&"unknown".to_string()));
        }
    }

    log::info!("✅ ensure_skills_exist 完成，所有技能已確保存在");
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn validate_career_json_file() {
        // 讀取專案根目錄下的 career.json（測試時工作目錄為 crate 根目錄）
        let path = "career.json";
        let content = fs::read_to_string(path).expect("無法讀取 career.json，請確認檔案位於 crate 根目錄");

        // 使用既有的解析函數（可處理 ```json 包裹的內容）
        let parsed = parse_ai_tasks_response(&content).expect("career.json 解析失敗，請檢查 JSON 結構");

        // 基本內容檢查
        let total = parsed.main_tasks.len() + parsed.daily_tasks.len() + parsed.project_tasks.len();
        assert!(total > 0, "必須至少包含一個任務（main/daily/project 任一類）");

        // 檢查每個任務必要欄位與數值範圍
        for task in parsed
            .main_tasks
            .iter()
            .chain(parsed.daily_tasks.iter())
            .chain(parsed.project_tasks.iter())
        {
            assert!(!task.title.trim().is_empty(), "title 不可為空");
            assert!(!task.description.trim().is_empty(), "description 不可為空");
            assert!((1..=5).contains(&task.difficulty), "difficulty 必須在 1..=5 範圍內: {}", task.difficulty);
            assert!(task.estimated_hours > 0, "estimated_hours 必須為正整數（小數會四捨五入）: {}", task.estimated_hours);
            assert!(!task.skill_tags.is_empty(), "skill_tags 不可為空");
            for tag in &task.skill_tags {
                assert!(!tag.name.trim().is_empty(), "skill_tags.name 不可為空");
                assert!(tag.category == "technical" || tag.category == "soft", "skill_tags.category 僅能為 technical/soft: {}", tag.category);
            }
        }

        println!(
            "career.json 驗證通過：main={} daily={} project={}，estimated_months={}，summary_len={}",
            parsed.main_tasks.len(),
            parsed.daily_tasks.len(),
            parsed.project_tasks.len(),
            parsed.estimated_months,
            parsed.learning_summary.len()
        );
    }
}
