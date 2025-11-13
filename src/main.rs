mod config;
mod models;
mod routes;
mod auth;
mod validation;
// mod rate_limit; // TODO: 暫時禁用，等待 actix-governor 版本兼容性問題解決
mod database_reset;
mod seed_data;
mod ai_service;
mod ai_tasks;
mod ai_tasks_achievement;
mod achievement_service;
mod career_routes;
mod behavior_analytics;
mod progressive_career_gen;
#[cfg(feature = "push-notifications")]
mod push_service;
#[cfg(feature = "push-notifications")]
mod push_scheduler;
mod calendar_service;
mod notification_generator;
use actix_cors::Cors;
use actix_web::{web, App, HttpServer};
use actix_web::middleware::Logger;
use rbatis::RBatis;
use rbdc_sqlite::driver::SqliteDriver;
use rustls::{Certificate, PrivateKey, ServerConfig};
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::fs::File;
use std::io::BufReader;

use config::Config;
use routes::*;
use database_reset::reset_database;
use seed_data::{seed_database, seed_minimum_user_data};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // 處理命令行參數
    let args: Vec<String> = std::env::args().collect();
    let is_production = args.contains(&"prod".to_string());
    let reset_db = args.contains(&"--reset-db".to_string());
    let init_db = args.contains(&"--init-db".to_string());
    let seed_only = args.contains(&"--seed".to_string());

    // 根據命令行參數載入對應的 .env 文件
    if is_production {
        dotenv::from_filename(".env.production").ok();
    } else {
        dotenv::from_filename(".env.development").ok();
    }

    // 載入配置
    let config = Config::from_env();

    // 初始化日誌 - 根據配置設置日誌級別
    let log_level = match config.app.log_level.to_lowercase().as_str() {
        "error" => log::LevelFilter::Error,
        "warn" => log::LevelFilter::Warn,
        "info" => log::LevelFilter::Info,
        "debug" => log::LevelFilter::Debug,
        "trace" => log::LevelFilter::Trace,
        _ => log::LevelFilter::Info,  // 默認為 Info 級別
    };

    // 使用 log4rs 初始化日誌系統
    use log4rs::{
        append::{console::ConsoleAppender, file::FileAppender},
        config::{Appender, Root},
        encode::pattern::PatternEncoder,
        Config as Log4rsConfig,
    };

    // 創建控制台輸出器（stdout）
    let stdout = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d(%Y-%m-%d %H:%M:%S)} [{l}] {t} - {m}{n}")))
        .build();

    // 創建文件輸出器
    let logfile = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d(%Y-%m-%d %H:%M:%S)} [{l}] {t} - {m}{n}")))
        .build("logs/lifeup.log")
        .expect("無法創建日誌文件");

    // 配置 log4rs
    let log_config = Log4rsConfig::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(Root::builder()
            .appender("stdout")
            .appender("logfile")
            .build(log_level))
        .expect("日誌配置失敗");

    log4rs::init_config(log_config).expect("日誌初始化失敗");

    if is_production {
        log::info!("LifeUp Backend 啟動中... [生產模式]");
    } else {
        log::info!("LifeUp Backend 啟動中... [開發模式]");
    }

    // 基本配置日誌 (不記錄敏感資訊)
    log::info!("環境: {}", config.app.environment);
    log::info!("日誌級別: {}", config.app.log_level);
    log::info!("數據庫: {}", if config.database.url.contains("sqlite") { "SQLite" } else { "其他" });
    log::info!("允許的 CORS 來源: {:?}", config.server.allowed_origins);

    // AI 配置調試日誌 (不記錄 API 金鑰)
    log::info!("AI 配置載入: API_OPTION={}", config.app.ai.api_option);
    log::info!("OpenAI API Key: {}", if config.app.ai.openai_api_key.is_some() { "已設置" } else { "未設置" });
    log::info!("OpenRouter API Key: {}", if config.app.ai.openrouter_api_key.is_some() { "已設置" } else { "未設置" });
    log::info!("OpenAI 模型: {}", config.app.ai.openai_model);
    log::info!("OpenRouter 模型: {}", config.app.ai.openrouter_model);

    // 初始化 rbatis
    let rb = RBatis::new();
    
    // 連接資料庫
    rb.init(SqliteDriver {}, &config.database.url).unwrap();
    log::info!("資料庫連接成功: {}", config.database.url);

    // 處理資料庫重置命令 (--reset-db: 完全重置 + 插入測試資料)
    if reset_db {
        log::info!("執行資料庫重置...");
        if let Err(e) = reset_database(&rb).await {
            log::error!("資料庫重置失敗: {}", e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()));
        }
        if let Err(e) = seed_database(&rb).await {
            log::error!("種子資料插入失敗: {}", e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()));
        }
        log::info!("資料庫重置和種子資料插入完成！");
        return Ok(());
    }
    
    // 處理資料庫初始化命令 (--init-db: 僅建立表結構，不插入任何使用者資料)
    if init_db {
        log::info!("執行資料庫初始化...");
        if let Err(e) = reset_database(&rb).await {
            log::error!("資料庫初始化失敗: {}", e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()));
        }
        log::info!("資料庫初始化完成（未插入任何使用者資料），請先註冊帳號。");
        return Ok(());
    }

    // 處理僅插入種子資料命令 (--seed: 保留現有表，只插入資料)
    if seed_only {
        log::info!("僅插入種子資料...");
        if let Err(e) = seed_database(&rb).await {
            log::error!("種子資料插入失敗: {}", e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()));
        }
        log::info!("種子資料插入完成！");
        return Ok(());
    }

    // 確保資料表存在並執行必要的遷移
    create_tables(&rb).await;
    migrate_database(&rb).await;

    // 初始化日曆服務（用於假日判斷）
    let calendar_service = match calendar_service::CalendarService::new() {
        Ok(service) => {
            log::info!("日曆服務初始化成功，載入 {} 個假日", service.get_holiday_count());
            service
        }
        Err(e) => {
            log::warn!("日曆服務初始化失敗: {}", e);
            log::info!("將使用簡單的週末判斷作為備用");
            calendar_service::CalendarService::new().unwrap()
        }
    };

    // 啟動推送通知調度器（僅在啟用推送通知功能時）
    #[cfg(feature = "push-notifications")]
    {
        if let Err(e) = push_scheduler::start_push_scheduler(rb.clone(), calendar_service.clone()).await {
            log::warn!("推送通知調度器啟動失敗（可能是 VAPID 金鑰未配置）: {}", e);
            log::info!("推送通知功能將不可用，但不影響其他服務運行");
        } else {
            log::info!("推送通知調度器已成功啟動");
        }
    }
    #[cfg(not(feature = "push-notifications"))]
    log::info!("推送通知功能已停用（未啟用 push-notifications feature）");

    let server_addr = config.server_addr();

    // 共享資料庫連線
    let rb_data = web::Data::new(rb.clone());

    // 根據環境決定使用 HTTP 還是 HTTPS
    if is_production {
        // 生產模式：使用 HTTPS
        log::info!("啟動 HTTPS 伺服器在 https://{}", &server_addr);

        // 載入 SSL 證書和私鑰
        let cert_file = &mut BufReader::new(File::open("/root/lfup/ssl/fullchain.crt")?);
        let key_file = &mut BufReader::new(File::open("/root/lfup/ssl/lifeup.key")?);

        let cert_chain: Vec<Certificate> = certs(cert_file)?
            .into_iter()
            .map(Certificate)
            .collect();

        let mut keys: Vec<PrivateKey> = pkcs8_private_keys(key_file)?
            .into_iter()
            .map(PrivateKey)
            .collect();

        if keys.is_empty() {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "找不到私鑰"));
        }

        let rustls_config = ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(cert_chain, keys.remove(0))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

        HttpServer::new(move || {
            // 設定 CORS - 只允許配置的來源 (HTTPS)
            log::info!("配置 CORS，允許的來源: {:?}", config.server.allowed_origins);

            let mut cors = Cors::default()
                .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS", "PATCH"])
                .allowed_headers(vec![
                    actix_web::http::header::AUTHORIZATION,
                    actix_web::http::header::ACCEPT,
                    actix_web::http::header::CONTENT_TYPE,
                    actix_web::http::header::HeaderName::from_static("x-requested-with"),
                ])
                .expose_headers(vec![actix_web::http::header::CONTENT_TYPE])
                .supports_credentials()
                .max_age(3600);

            // 添加允許的來源
            for origin in &config.server.allowed_origins {
                log::info!("添加 CORS 來源: {}", origin);
                cors = cors.allowed_origin(origin.as_str());
            }

            App::new()
                // HTTP 請求日誌
                .wrap(Logger::default())
                .wrap(cors)
                .app_data(rb_data.clone())
            // === 公開路由（不需要 JWT 認證）===
            .route("/health", web::get().to(health_check))
            .route("/api/auth/login", web::post().to(login))
            .route("/api/users", web::post().to(create_user))  // 註冊

            // === 受保護路由（需要 JWT 認證）===
            .service(
                web::scope("/api")
                    .wrap(auth::JwtAuth)  // 🔒 應用 JWT 認證中間件
                    // 認證相關
                    .route("/auth/logout", web::post().to(logout))
                    // 使用者相關
                    .route("/users", web::get().to(get_users))
                    .route("/users/{id}", web::get().to(get_user))
                    .route("/users/{id}/gamified", web::get().to(get_gamified_user_data))
                    .route("/users/{id}/experience", web::post().to(update_user_experience))
                    .route("/users/{id}/attributes", web::post().to(update_user_attributes))
                    .route("/users/{user_id}/achievements", web::get().to(get_user_achievements))
                    .route("/users/{user_id}/achievements/status", web::get().to(get_user_achievements_status))
                    .route("/users/{user_id}/achievements/{achievement_id}/unlock", web::post().to(unlock_user_achievement))
                    .route("/users/{user_id}/attributes/weekly/{weeks_ago}", web::get().to(get_weekly_attributes))
                    .route("/users/{user_id}/reset", web::delete().to(reset_user_data))
                    .route("/users/{user_id}/reset", web::post().to(reset_user_data_selective))
                    .route("/users/{user_id}/task-history", web::get().to(get_task_history))
                    // 任務相關路由
                    .route("/tasks", web::get().to(get_tasks))
                    .route("/tasks", web::post().to(create_task))
                    .route("/tasks/homepage", web::get().to(get_homepage_tasks))
                    .route("/tasks/type/{task_type}", web::get().to(get_tasks_by_type))
                    .route("/tasks/{id}", web::get().to(get_task))
                    .route("/tasks/{id}", web::put().to(update_task))
                    .route("/tasks/{id}", web::delete().to(delete_task))
                    .route("/tasks/{id}/start", web::post().to(start_task))
                    .route("/tasks/{id}/subtasks", web::get().to(get_subtasks))
                    .route("/tasks/{id}/pause", web::put().to(pause_task))
                    .route("/tasks/{id}/cancel", web::put().to(cancel_task))
                    .route("/tasks/{id}/restart", web::put().to(restart_task))
                    .route("/tasks/{id}/generate-daily", web::post().to(generate_daily_tasks))
                    .route("/tasks/{id}/progress", web::get().to(get_task_progress))
                    .route("/tasks/generate-skill-tags", web::post().to(generate_skill_tags))
                    .route("/tasks/generate", web::post().to(crate::ai_tasks::generate_task_with_ai))
                    .route("/tasks/generate-json", web::post().to(crate::ai_tasks::generate_task_json))
                    .route("/tasks/generate-daily-task-json", web::post().to(crate::ai_tasks::generate_daily_task_json))
                    .route("/tasks/insert-json", web::post().to(crate::ai_tasks::insert_task_from_json))
                    .route("/tasks/create-from-json", web::post().to(crate::ai_tasks::create_task_from_json))
                    .route("/tasks/validate-preview", web::post().to(crate::ai_tasks::validate_and_preview_task))
                    .route("/tasks/generate-from-chat", web::post().to(crate::ai_tasks::generate_task_from_chat))
                    .route("/tasks/generate-with-expert", web::post().to(crate::ai_tasks::generate_task_with_expert))
                    .route("/tasks/match-expert", web::post().to(crate::ai_tasks::match_expert_only))
                    .route("/tasks/expert-analysis", web::post().to(crate::ai_tasks::expert_analysis))
                    .route("/tasks/generate-subtasks", web::post().to(crate::ai_tasks::generate_subtasks_for_task))
                    .route("/tasks/classify-intent", web::post().to(crate::ai_tasks::classify_user_intent))
                    // 重複性任務路由
                    .route("/recurring-tasks", web::post().to(create_recurring_task))
                    // 技能相關路由
                    .route("/skills", web::get().to(get_skills))
                    .route("/skills", web::post().to(create_skill))
                    .route("/skills/{id}/experience", web::post().to(update_skill_experience))
                    .route("/skills/{skill_name}/tasks", web::get().to(get_tasks_by_skill))
                    // 聊天相關路由
                    .route("/chat/messages", web::get().to(get_chat_messages))
                    .route("/chat/messages/all", web::get().to(get_all_chat_messages))
                    .route("/chat/send", web::post().to(send_message))
                    .route("/chat/save-message", web::post().to(save_chat_message))
                    .route("/chat/chatgpt", web::post().to(send_message_to_chatgpt))
                    .route("/chat/personality", web::post().to(send_message_with_personality))
                    .route("/chat/test-personality", web::post().to(send_message_with_direct_personality))
                    .route("/chat/test", web::get().to(test_endpoint))
                    // 教練個性相關路由
                    .route("/coach/personalities", web::get().to(get_available_personalities))
                    .route("/coach/personality", web::post().to(set_coach_personality))
                    .route("/coach/personality/current", web::get().to(get_current_personality))
                    // 成就相關路由
                    .route("/achievements", web::get().to(get_achievements))
                    .route("/achievements/{id}", web::get().to(get_achievement_details))
                    .route("/achievements/sync-stats", web::post().to(sync_achievement_statistics))
                    .route("/achievements/generate", web::post().to(generate_achievement_with_ai))
                    .route("/achievements/generate-from-tasks/{user_id}", web::post().to(crate::ai_tasks::generate_achievement_from_tasks))
                    // 職業主線任務系統路由
                    .route("/quiz/save-results", web::post().to(crate::career_routes::save_quiz_results))
                    .route("/career/generate-tasks", web::post().to(crate::career_routes::generate_career_tasks))
                    .route("/career/accept-tasks", web::post().to(crate::career_routes::accept_career_tasks))
                    .route("/career/import", web::post().to(crate::career_routes::import_career_tasks))
                    .route("/career/generate-tasks-progressive", web::post().to(crate::progressive_career_gen::generate_career_tasks_progressive_sse))
                    // 推送通知路由
                    .route("/push/subscribe", web::post().to(subscribe_push))
                    .route("/push/unsubscribe", web::post().to(unsubscribe_push))
                    .route("/push/test/{user_id}", web::post().to(send_test_push))
                    .route("/push/vapid-public-key", web::get().to(get_vapid_public_key))
                    .route("/push/subscriptions", web::get().to(get_all_subscriptions))
                    .route("/push/clear-all", web::post().to(clear_all_subscriptions))
                    .route("/notifications/test-push/{user_id}", web::post().to(send_delayed_test_push))
                    // 通知設定路由
                    .route("/notification-settings/{user_id}", web::get().to(get_notification_settings))
                    .route("/notification-settings/{user_id}", web::put().to(update_notification_settings))
                    .route("/notifications/preview-morning/{user_id}", web::post().to(preview_morning_notification))
                    .route("/notifications/preview-evening/{user_id}", web::post().to(preview_evening_notification))
                    .app_data(web::Data::new(config.clone()))
            )
            // 職業主線任務系統路由
            .route("/api/quiz/save-results", web::post().to(crate::career_routes::save_quiz_results))
            .route("/api/career/generate-tasks", web::post().to(crate::career_routes::generate_career_tasks))
            .route("/api/career/accept-tasks", web::post().to(crate::career_routes::accept_career_tasks))
            .route("/api/career/import", web::post().to(crate::career_routes::import_career_tasks))
            // 多步驟漸進式任務生成（SSE）
            .route("/api/career/generate-tasks-progressive", web::post().to(crate::progressive_career_gen::generate_career_tasks_progressive_sse))
            .app_data(web::Data::new(config.clone()))
            // 使用者資料重置路由
            .route("/api/users/{user_id}/reset", web::delete().to(reset_user_data))
            .route("/api/users/{user_id}/reset", web::post().to(reset_user_data_selective))
            // 任務歷史路由
            .route("/api/users/{user_id}/task-history", web::get().to(get_task_history))
            // 推送通知相關路由（條件編譯）
            .configure(configure_push_routes)
        })
        .workers(2)
        .bind_rustls_021(&server_addr, rustls_config)?
        .run()
        .await
    } else {
        // 開發模式：使用 HTTP
        log::info!("啟動 HTTP 伺服器在 http://{}", &server_addr);

        HttpServer::new(move || {
            // 設定 CORS - 只允許配置的來源 (HTTP)
            log::info!("配置 CORS，允許的來源: {:?}", config.server.allowed_origins);

            let mut cors = Cors::default()
                .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS", "PATCH"])
                .allowed_headers(vec![
                    actix_web::http::header::AUTHORIZATION,
                    actix_web::http::header::ACCEPT,
                    actix_web::http::header::CONTENT_TYPE,
                    actix_web::http::header::HeaderName::from_static("x-requested-with"),
                ])
                .expose_headers(vec![actix_web::http::header::CONTENT_TYPE])
                .supports_credentials()
                .max_age(3600);

            // 添加允許的來源
            for origin in &config.server.allowed_origins {
                log::info!("添加 CORS 來源: {}", origin);
                cors = cors.allowed_origin(origin.as_str());
            }

            App::new()
                // HTTP 請求日誌
                .wrap(Logger::default())
                .wrap(cors)
                .app_data(rb_data.clone())
            // === 公開路由（不需要 JWT 認證）===
            .route("/health", web::get().to(health_check))
            .route("/api/auth/login", web::post().to(login))
            .route("/api/users", web::post().to(create_user))  // 註冊

            // === 受保護路由（需要 JWT 認證）===
            .service(
                web::scope("/api")
                    .wrap(auth::JwtAuth)  // 🔒 應用 JWT 認證中間件
                    // 認證相關
                    .route("/auth/logout", web::post().to(logout))
                    // 使用者相關
                    .route("/users", web::get().to(get_users))
                    .route("/users/{id}", web::get().to(get_user))
                    .route("/users/{id}/gamified", web::get().to(get_gamified_user_data))
                    .route("/users/{id}/experience", web::post().to(update_user_experience))
                    .route("/users/{id}/attributes", web::post().to(update_user_attributes))
                    .route("/users/{user_id}/achievements", web::get().to(get_user_achievements))
                    .route("/users/{user_id}/achievements/status", web::get().to(get_user_achievements_status))
                    .route("/users/{user_id}/achievements/{achievement_id}/unlock", web::post().to(unlock_user_achievement))
                    .route("/users/{user_id}/attributes/weekly/{weeks_ago}", web::get().to(get_weekly_attributes))
                    .route("/users/{user_id}/reset", web::delete().to(reset_user_data))
                    .route("/users/{user_id}/reset", web::post().to(reset_user_data_selective))
                    .route("/users/{user_id}/task-history", web::get().to(get_task_history))
                    // 任務相關路由
                    .route("/tasks", web::get().to(get_tasks))
                    .route("/tasks", web::post().to(create_task))
                    .route("/tasks/homepage", web::get().to(get_homepage_tasks))
                    .route("/tasks/type/{task_type}", web::get().to(get_tasks_by_type))
                    .route("/tasks/{id}", web::get().to(get_task))
                    .route("/tasks/{id}", web::put().to(update_task))
                    .route("/tasks/{id}", web::delete().to(delete_task))
                    .route("/tasks/{id}/start", web::post().to(start_task))
                    .route("/tasks/{id}/subtasks", web::get().to(get_subtasks))
                    .route("/tasks/{id}/pause", web::put().to(pause_task))
                    .route("/tasks/{id}/cancel", web::put().to(cancel_task))
                    .route("/tasks/{id}/restart", web::put().to(restart_task))
                    .route("/tasks/{id}/generate-daily", web::post().to(generate_daily_tasks))
                    .route("/tasks/{id}/progress", web::get().to(get_task_progress))
                    .route("/tasks/generate-skill-tags", web::post().to(generate_skill_tags))
                    .route("/tasks/generate", web::post().to(crate::ai_tasks::generate_task_with_ai))
                    .route("/tasks/generate-json", web::post().to(crate::ai_tasks::generate_task_json))
                    .route("/tasks/generate-daily-task-json", web::post().to(crate::ai_tasks::generate_daily_task_json))
                    .route("/tasks/insert-json", web::post().to(crate::ai_tasks::insert_task_from_json))
                    .route("/tasks/create-from-json", web::post().to(crate::ai_tasks::create_task_from_json))
                    .route("/tasks/validate-preview", web::post().to(crate::ai_tasks::validate_and_preview_task))
                    .route("/tasks/generate-from-chat", web::post().to(crate::ai_tasks::generate_task_from_chat))
                    .route("/tasks/generate-with-expert", web::post().to(crate::ai_tasks::generate_task_with_expert))
                    .route("/tasks/match-expert", web::post().to(crate::ai_tasks::match_expert_only))
                    .route("/tasks/expert-analysis", web::post().to(crate::ai_tasks::expert_analysis))
                    .route("/tasks/generate-subtasks", web::post().to(crate::ai_tasks::generate_subtasks_for_task))
                    .route("/tasks/classify-intent", web::post().to(crate::ai_tasks::classify_user_intent))
                    // 重複性任務路由
                    .route("/recurring-tasks", web::post().to(create_recurring_task))
                    // 技能相關路由
                    .route("/skills", web::get().to(get_skills))
                    .route("/skills", web::post().to(create_skill))
                    .route("/skills/{id}/experience", web::post().to(update_skill_experience))
                    .route("/skills/{skill_name}/tasks", web::get().to(get_tasks_by_skill))
                    // 聊天相關路由
                    .route("/chat/messages", web::get().to(get_chat_messages))
                    .route("/chat/messages/all", web::get().to(get_all_chat_messages))
                    .route("/chat/send", web::post().to(send_message))
                    .route("/chat/save-message", web::post().to(save_chat_message))
                    .route("/chat/chatgpt", web::post().to(send_message_to_chatgpt))
                    .route("/chat/personality", web::post().to(send_message_with_personality))
                    .route("/chat/test-personality", web::post().to(send_message_with_direct_personality))
                    .route("/chat/test", web::get().to(test_endpoint))
                    // 教練個性相關路由
                    .route("/coach/personalities", web::get().to(get_available_personalities))
                    .route("/coach/personality", web::post().to(set_coach_personality))
                    .route("/coach/personality/current", web::get().to(get_current_personality))
                    // 成就相關路由
                    .route("/achievements", web::get().to(get_achievements))
                    .route("/achievements/{id}", web::get().to(get_achievement_details))
                    .route("/achievements/sync-stats", web::post().to(sync_achievement_statistics))
                    .route("/achievements/generate", web::post().to(generate_achievement_with_ai))
                    .route("/achievements/generate-from-tasks/{user_id}", web::post().to(crate::ai_tasks::generate_achievement_from_tasks))
                    // 職業主線任務系統路由
                    .route("/quiz/save-results", web::post().to(crate::career_routes::save_quiz_results))
                    .route("/career/generate-tasks", web::post().to(crate::career_routes::generate_career_tasks))
                    .route("/career/accept-tasks", web::post().to(crate::career_routes::accept_career_tasks))
                    .route("/career/import", web::post().to(crate::career_routes::import_career_tasks))
                    .route("/career/generate-tasks-progressive", web::post().to(crate::progressive_career_gen::generate_career_tasks_progressive_sse))
                    // 推送通知路由
                    .route("/push/subscribe", web::post().to(subscribe_push))
                    .route("/push/unsubscribe", web::post().to(unsubscribe_push))
                    .route("/push/test/{user_id}", web::post().to(send_test_push))
                    .route("/push/vapid-public-key", web::get().to(get_vapid_public_key))
                    .route("/push/subscriptions", web::get().to(get_all_subscriptions))
                    .route("/push/clear-all", web::post().to(clear_all_subscriptions))
                    .route("/notifications/test-push/{user_id}", web::post().to(send_delayed_test_push))
                    // 通知設定路由
                    .route("/notification-settings/{user_id}", web::get().to(get_notification_settings))
                    .route("/notification-settings/{user_id}", web::put().to(update_notification_settings))
                    .route("/notifications/preview-morning/{user_id}", web::post().to(preview_morning_notification))
                    .route("/notifications/preview-evening/{user_id}", web::post().to(preview_evening_notification))
                    .app_data(web::Data::new(config.clone()))
            )
            // 職業主線任務系統路由
            .route("/api/quiz/save-results", web::post().to(crate::career_routes::save_quiz_results))
            .route("/api/career/generate-tasks", web::post().to(crate::career_routes::generate_career_tasks))
            .route("/api/career/accept-tasks", web::post().to(crate::career_routes::accept_career_tasks))
            .route("/api/career/import", web::post().to(crate::career_routes::import_career_tasks))
            // 多步驟漸進式任務生成（SSE）
            .route("/api/career/generate-tasks-progressive", web::post().to(crate::progressive_career_gen::generate_career_tasks_progressive_sse))
            .app_data(web::Data::new(config.clone()))
            // 使用者資料重置路由
            .route("/api/users/{user_id}/reset", web::delete().to(reset_user_data))
            .route("/api/users/{user_id}/reset", web::post().to(reset_user_data_selective))
            // 任務歷史路由
            .route("/api/users/{user_id}/task-history", web::get().to(get_task_history))
            // 推送通知相關路由（條件編譯）
            .configure(configure_push_routes)
        })
        .workers(2)
        .bind(&server_addr)?
        .run()
        .await
    }
}

async fn create_tables(rb: &RBatis) {
    let tables = vec![
        // 使用者表
        r#"
        CREATE TABLE IF NOT EXISTS user (
            id TEXT PRIMARY KEY,
            name TEXT,
            email TEXT,
            password_hash TEXT,
            created_at TEXT,
            updated_at TEXT
        )
        "#,
        // 任務表
        r#"
        CREATE TABLE IF NOT EXISTS task (
            id TEXT PRIMARY KEY,
            user_id TEXT,
            title TEXT,
            description TEXT,
            status INTEGER DEFAULT 0,
            priority INTEGER DEFAULT 1,
            task_type TEXT DEFAULT 'daily',
            difficulty INTEGER DEFAULT 1,
            experience INTEGER DEFAULT 10,
            parent_task_id TEXT,
            is_parent_task BOOLEAN DEFAULT FALSE,
            task_order INTEGER DEFAULT 0,
            due_date TEXT,
            created_at TEXT,
            updated_at TEXT,
            is_recurring BOOLEAN DEFAULT FALSE,
            recurrence_pattern TEXT,
            start_date TEXT,
            end_date TEXT,
            completion_target REAL DEFAULT 0.8,
            completion_rate REAL DEFAULT 0.0,
            task_date TEXT,
            cancel_count INTEGER DEFAULT 0,
            last_cancelled_at TEXT,
            skill_tags TEXT,
            career_mainline_id TEXT,
            task_category TEXT,
            attributes TEXT,
            FOREIGN KEY (user_id) REFERENCES user (id),
            FOREIGN KEY (parent_task_id) REFERENCES task (id)
        )
        "#,
        // 技能表
        r#"
        CREATE TABLE IF NOT EXISTS skill (
            id TEXT PRIMARY KEY,
            user_id TEXT,
            name TEXT,
            description TEXT,
            category TEXT DEFAULT 'technical',
            attribute TEXT DEFAULT 'intelligence',
            level INTEGER DEFAULT 1,
            experience INTEGER DEFAULT 0,
            max_experience INTEGER DEFAULT 100,
            icon TEXT DEFAULT '⭐',
            created_at TEXT,
            updated_at TEXT,
            FOREIGN KEY (user_id) REFERENCES user (id)
        )
        "#,
        // 聊天記錄表
        r#"
        CREATE TABLE IF NOT EXISTS chat_message (
            id TEXT PRIMARY KEY,
            user_id TEXT,
            role TEXT,
            content TEXT,
            created_at TEXT,
            FOREIGN KEY (user_id) REFERENCES user (id)
        )
        "#,
        // 重複性任務模板表
        r#"
        CREATE TABLE IF NOT EXISTS recurring_task_template (
            id TEXT PRIMARY KEY,
            parent_task_id TEXT,
            title TEXT NOT NULL,
            description TEXT,
            difficulty INTEGER DEFAULT 1,
            experience INTEGER DEFAULT 10,
            task_order INTEGER DEFAULT 0,
            created_at TEXT,
            updated_at TEXT,
            FOREIGN KEY (parent_task_id) REFERENCES task (id)
        )
        "#,
        // 用戶遊戲化資料表
        r#"
        CREATE TABLE IF NOT EXISTS user_profile (
            id TEXT PRIMARY KEY,
            user_id TEXT UNIQUE NOT NULL,
            level INTEGER DEFAULT 1,
            experience INTEGER DEFAULT 0,
            max_experience INTEGER DEFAULT 100,
            title TEXT DEFAULT '新手冒險者',
            adventure_days INTEGER DEFAULT 1,
            consecutive_login_days INTEGER DEFAULT 1,
            persona_type TEXT DEFAULT 'internal',
            created_at TEXT,
            updated_at TEXT,
            FOREIGN KEY (user_id) REFERENCES user (id)
        )
        "#,
        // 用戶屬性表
        r#"
        CREATE TABLE IF NOT EXISTS user_attributes (
            id TEXT PRIMARY KEY,
            user_id TEXT UNIQUE NOT NULL,
            intelligence INTEGER DEFAULT 50,
            endurance INTEGER DEFAULT 50,
            creativity INTEGER DEFAULT 50,
            social INTEGER DEFAULT 50,
            focus INTEGER DEFAULT 50,
            adaptability INTEGER DEFAULT 50,
            created_at TEXT,
            updated_at TEXT,
            FOREIGN KEY (user_id) REFERENCES user (id)
        )
        "#,
        // 每日進度表
        r#"
        CREATE TABLE IF NOT EXISTS daily_progress (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            date TEXT NOT NULL,
            completed_tasks INTEGER DEFAULT 0,
            total_tasks INTEGER DEFAULT 0,
            experience_gained INTEGER DEFAULT 0,
            attributes_gained TEXT DEFAULT '{}',
            created_at TEXT,
            updated_at TEXT,
            UNIQUE(user_id, date),
            FOREIGN KEY (user_id) REFERENCES user (id)
        )
        "#,
        // 成就表
        r#"
        CREATE TABLE IF NOT EXISTS achievement (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            icon TEXT,
            category TEXT DEFAULT 'general',
            requirement_type TEXT NOT NULL,
            requirement_value INTEGER DEFAULT 1,
            experience_reward INTEGER DEFAULT 50,
            created_at TEXT
        )
        "#,
        // 用戶成就關聯表
        r#"
        CREATE TABLE IF NOT EXISTS user_achievement (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            achievement_id TEXT NOT NULL,
            achieved_at TEXT,
            progress INTEGER DEFAULT 0,
            UNIQUE(user_id, achievement_id),
            FOREIGN KEY (user_id) REFERENCES user (id),
            FOREIGN KEY (achievement_id) REFERENCES achievement (id)
        )
        "#,
        // 週屬性快照表
        r#"
        CREATE TABLE IF NOT EXISTS weekly_attribute_snapshot (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            week_start_date TEXT NOT NULL,
            year INTEGER NOT NULL,
            week_number INTEGER NOT NULL,
            intelligence INTEGER DEFAULT 50,
            endurance INTEGER DEFAULT 50,
            creativity INTEGER DEFAULT 50,
            social INTEGER DEFAULT 50,
            focus INTEGER DEFAULT 50,
            adaptability INTEGER DEFAULT 50,
            created_at TEXT,
            UNIQUE(user_id, year, week_number),
            FOREIGN KEY (user_id) REFERENCES user (id)
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS user_coach_preference (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            personality_type TEXT NOT NULL,
            created_at TEXT,
            updated_at TEXT,
            UNIQUE(user_id),
            FOREIGN KEY (user_id) REFERENCES user (id)
        )
        "#,
        // 測驗結果表
        r#"
        CREATE TABLE IF NOT EXISTS quiz_results (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            values_results TEXT NOT NULL,
            interests_results TEXT NOT NULL,
            talents_results TEXT NOT NULL,
            workstyle_results TEXT NOT NULL,
            completed_at TEXT NOT NULL,
            is_active BOOLEAN DEFAULT TRUE,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (user_id) REFERENCES user (id)
        )
        "#,
        // 職業主線任務表
        r#"
        CREATE TABLE IF NOT EXISTS career_mainlines (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            quiz_result_id TEXT NOT NULL,
            selected_career TEXT NOT NULL,
            survey_answers TEXT,
            total_tasks_generated INTEGER DEFAULT 0,
            estimated_completion_months INTEGER,
            status TEXT DEFAULT 'active',
            progress_percentage REAL DEFAULT 0.0,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (user_id) REFERENCES user (id),
            FOREIGN KEY (quiz_result_id) REFERENCES quiz_results (id)
        )
        "#,
        // 成就統計表
        r#"
        CREATE TABLE IF NOT EXISTS achievement_stats (
            id TEXT PRIMARY KEY,
            achievement_id TEXT UNIQUE NOT NULL,
            completion_count INTEGER DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (achievement_id) REFERENCES achievement (id)
        )
        "#,
    ];

    for (i, sql) in tables.iter().enumerate() {
        match rb.exec(sql, vec![]).await {
            Ok(_) => log::info!("資料庫表 {} 建立成功", i + 1),
            Err(e) => log::error!("資料庫表 {} 建立失敗: {}", i + 1, e),
        }
    }
    
    log::info!("所有資料庫表建立完成");
}

/// 配置推送通知相關路由（僅在啟用 push-notifications feature 時）
#[cfg(feature = "push-notifications")]
fn configure_push_routes(cfg: &mut actix_web::web::ServiceConfig) {
    cfg
        // 推送通知路由
        .route("/api/push/subscribe", web::post().to(subscribe_push))
        .route("/api/push/unsubscribe", web::post().to(unsubscribe_push))
        .route("/api/push/test/{user_id}", web::post().to(send_test_push))
        .route("/api/push/vapid-public-key", web::get().to(get_vapid_public_key))
        .route("/api/push/subscriptions", web::get().to(get_all_subscriptions))
        .route("/api/push/clear-all", web::post().to(clear_all_subscriptions))
        .route("/api/notifications/test-push/{user_id}", web::post().to(send_delayed_test_push))
        // 通知設定路由
        .route("/api/notification-settings/{user_id}", web::get().to(get_notification_settings))
        .route("/api/notification-settings/{user_id}", web::put().to(update_notification_settings))
        .route("/api/notifications/preview-morning/{user_id}", web::post().to(preview_morning_notification))
        .route("/api/notifications/preview-evening/{user_id}", web::post().to(preview_evening_notification));
}

/// 配置推送通知相關路由的空實現（當未啟用 push-notifications feature 時）
#[cfg(not(feature = "push-notifications"))]
fn configure_push_routes(_cfg: &mut actix_web::web::ServiceConfig) {
    // 推送通知功能未啟用，不配置任何路由
}

async fn migrate_database(rb: &RBatis) {
    // 創建用戶通知設定表
    let create_table_query = r#"
        CREATE TABLE IF NOT EXISTS user_notification_settings (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL UNIQUE,
            enabled INTEGER DEFAULT 1,
            notify_on_workdays INTEGER DEFAULT 1,
            notify_on_holidays INTEGER DEFAULT 0,
            morning_enabled INTEGER DEFAULT 1,
            morning_time TEXT DEFAULT '08:00',
            evening_enabled INTEGER DEFAULT 1,
            evening_time TEXT DEFAULT '22:00',
            custom_schedules TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (user_id) REFERENCES user(id)
        )
    "#;

    match rb.exec(create_table_query, vec![]).await {
        Ok(_) => log::info!("✅ user_notification_settings 表創建成功"),
        Err(e) => log::warn!("user_notification_settings 表創建警告: {}", e),
    }

    // 添加職業任務相關欄位到 task 表
    let alter_table_queries = vec![
        "ALTER TABLE user ADD COLUMN password_hash TEXT",
        "ALTER TABLE task ADD COLUMN career_mainline_id TEXT",
        "ALTER TABLE task ADD COLUMN task_category TEXT",
        "ALTER TABLE task ADD COLUMN attributes TEXT",
        "ALTER TABLE quiz_results ADD COLUMN updated_at TEXT",
        "ALTER TABLE skill ADD COLUMN attribute TEXT DEFAULT 'intelligence'",
        "ALTER TABLE achievement ADD COLUMN career_mainline_id TEXT",
        "ALTER TABLE achievement ADD COLUMN related_task_id TEXT",
        // 確保 email 唯一
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_user_email_unique ON user(email)",
        // 添加最後登入日期欄位，用於計算連續登入天數
        "ALTER TABLE user_profile ADD COLUMN last_login_date TEXT",
    ];

    // SQLite 不支援直接修改欄位約束，需要重建表
    // 先檢查是否需要處理 requirement_type 的 NOT NULL 約束
    let check_nullable = rb.query_decode::<Vec<serde_json::Value>>(
        "PRAGMA table_info(achievement)",
        vec![]
    ).await;

    if let Ok(columns) = check_nullable {
        let requirement_type_col = columns.iter().find(|col| {
            col.get("name").and_then(|v| v.as_str()) == Some("requirement_type")
        });

        if let Some(col) = requirement_type_col {
            let is_not_null = col.get("notnull").and_then(|v| v.as_i64()).unwrap_or(0) == 1;
            if is_not_null {
                log::info!("檢測到 achievement.requirement_type 為 NOT NULL，開始遷移...");

                // 重建 achievement 表，讓 requirement_type 可為 NULL
                let rebuild_queries = vec![
                    // 1. 備份現有數據
                    "CREATE TABLE achievement_backup AS SELECT * FROM achievement",
                    // 2. 刪除舊表
                    "DROP TABLE achievement",
                    // 3. 創建新表（requirement_type 允許 NULL）
                    r#"CREATE TABLE achievement (
                        id TEXT PRIMARY KEY,
                        name TEXT NOT NULL,
                        description TEXT,
                        icon TEXT,
                        category TEXT DEFAULT 'general',
                        requirement_type TEXT,
                        requirement_value INTEGER DEFAULT 1,
                        experience_reward INTEGER DEFAULT 50,
                        career_mainline_id TEXT,
                        related_task_id TEXT,
                        created_at TEXT
                    )"#,
                    // 4. 恢復數據
                    "INSERT INTO achievement SELECT * FROM achievement_backup",
                    // 5. 刪除備份表
                    "DROP TABLE achievement_backup",
                ];

                for query in rebuild_queries {
                    match rb.exec(query, vec![]).await {
                        Ok(_) => log::info!("✅ 執行成功: {}", query.chars().take(50).collect::<String>()),
                        Err(e) => log::error!("❌ 執行失敗: {} - {}", query.chars().take(50).collect::<String>(), e),
                    }
                }

                log::info!("✅ achievement 表遷移完成，requirement_type 現在允許 NULL");
            }
        }
    }

    for query in alter_table_queries {
        match rb.exec(query, vec![]).await {
            Ok(_) => log::info!("資料庫遷移成功: {}", query),
            Err(e) => {
                // 忽略欄位已存在的錯誤
                if e.to_string().contains("duplicate column name") {
                    log::info!("欄位已存在，跳過: {}", query);
                } else {
                    log::warn!("資料庫遷移警告: {} - {}", query, e);
                }
            }
        }
    }
    log::info!("資料庫遷移完成");
}

