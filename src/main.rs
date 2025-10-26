mod config;
mod models;
mod routes;
mod database_reset;
mod seed_data;
mod ai_service;
mod ai_tasks;
mod achievement_service;
mod career_routes;
mod behavior_analytics;
mod progressive_career_gen;
use actix_cors::Cors;
use actix_web::{web, App, HttpServer};
use actix_web::middleware::Logger;
use rbatis::RBatis;
use rbdc_sqlite::driver::SqliteDriver;

use config::Config;
use routes::*;
use database_reset::reset_database;
use seed_data::{seed_database, seed_minimum_user_data};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // 載入 .env 文件
    dotenv::dotenv().ok();
    
    // 處理命令行參數
    let args: Vec<String> = std::env::args().collect();
    let reset_db = args.contains(&"--reset-db".to_string());
    let init_db = args.contains(&"--init-db".to_string());
    let seed_only = args.contains(&"--seed".to_string());
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
    
    fast_log::init(
        fast_log::Config::new()
            .console()
            .level(log_level)
    ).expect("日誌初始化失敗");
    log::info!("LifeUp Backend 啟動中...");
    log::info!("配置: {:?}", config);
    
    // AI 配置調試日誌
    log::info!("AI 配置載入: API_OPTION={}", config.app.ai.api_option);
    log::info!("OpenAI API Key 存在: {}", config.app.ai.openai_api_key.is_some());
    log::info!("OpenRouter API Key 存在: {}", config.app.ai.openrouter_api_key.is_some());
    if let Some(key) = &config.app.ai.openrouter_api_key {
        let prefix = key.chars().take(10).collect::<String>();
        log::info!("OpenRouter API Key 前綴: {}", prefix);
    }
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

    let server_addr = config.server_addr();
    log::info!("啟動 HTTP 伺服器在 http://{}", &server_addr);

    // 共享資料庫連線
    let rb_data = web::Data::new(rb.clone());

    HttpServer::new(move || {
        // 設定 CORS
        let cors = Cors::default()
            .allow_any_origin()
            .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
            .allowed_headers(vec![
                actix_web::http::header::AUTHORIZATION,
                actix_web::http::header::ACCEPT,
                actix_web::http::header::CONTENT_TYPE,
            ])
            .max_age(3600);

        App::new()
            // HTTP 請求日誌
            .wrap(Logger::default())
            .wrap(cors)
            .app_data(rb_data.clone())
            // 健康檢查
            .route("/health", web::get().to(health_check))
            // 使用者相關路由
            .route("/api/users", web::get().to(get_users))
            .route("/api/users", web::post().to(create_user))
            .route("/api/auth/login", web::post().to(login))
            .route("/api/auth/logout", web::post().to(logout))
            .route("/api/users/{id}", web::get().to(get_user))
            // 任務相關路由
            .route("/api/tasks", web::get().to(get_tasks))
            .route("/api/tasks", web::post().to(create_task))
            .route("/api/tasks/homepage", web::get().to(get_homepage_tasks))
            .route("/api/tasks/type/{task_type}", web::get().to(get_tasks_by_type))
            .route("/api/tasks/{id}", web::get().to(get_task))
            .route("/api/tasks/{id}", web::put().to(update_task))
            .route("/api/tasks/{id}", web::delete().to(delete_task))
            .route("/api/tasks/{id}/start", web::post().to(start_task))
            .route("/api/tasks/{id}/subtasks", web::get().to(get_subtasks))
            .route("/api/tasks/{id}/pause", web::put().to(pause_task))
            .route("/api/tasks/{id}/cancel", web::put().to(cancel_task))
            .route("/api/tasks/{id}/restart", web::put().to(restart_task))
            // 重複性任務路由
            .route("/api/recurring-tasks", web::post().to(create_recurring_task))
            .route("/api/tasks/{id}/generate-daily", web::post().to(generate_daily_tasks))
            .route("/api/tasks/{id}/progress", web::get().to(get_task_progress))
            // 技能相關路由
            .route("/api/skills", web::get().to(get_skills))
            .route("/api/skills", web::post().to(create_skill))
            .route("/api/skills/{id}/experience", web::post().to(update_skill_experience))
            .route("/api/skills/{skill_name}/tasks", web::get().to(get_tasks_by_skill))
            // 聊天相關路由
            .route("/api/chat/messages", web::get().to(get_chat_messages))
            .route("/api/chat/messages/all", web::get().to(get_all_chat_messages))
            .route("/api/chat/send", web::post().to(send_message))
            .route("/api/chat/save-message", web::post().to(save_chat_message))
            .route("/api/chat/chatgpt", web::post().to(send_message_to_chatgpt))
            .route("/api/chat/personality", web::post().to(send_message_with_personality))
            .route("/api/chat/test-personality", web::post().to(send_message_with_direct_personality))
            .route("/api/chat/test", web::get().to(test_endpoint))
            // 教練個性相關路由
            .route("/api/coach/personalities", web::get().to(get_available_personalities))
            .route("/api/coach/personality", web::post().to(set_coach_personality))
            .route("/api/coach/personality/current", web::get().to(get_current_personality))
            // 遊戲化數據相關路由
            .route("/api/users/{id}/gamified", web::get().to(get_gamified_user_data))
            .route("/api/users/{id}/experience", web::post().to(update_user_experience))
            .route("/api/users/{id}/attributes", web::post().to(update_user_attributes))
            // 成就相關路由
            .route("/api/achievements", web::get().to(get_achievements))
            .route("/api/achievements/{id}", web::get().to(get_achievement_details))
            .route("/api/achievements/sync-stats", web::post().to(sync_achievement_statistics))
            .route("/api/users/{user_id}/achievements", web::get().to(get_user_achievements))
            .route("/api/users/{user_id}/achievements/status", web::get().to(get_user_achievements_status))
            .route("/api/users/{user_id}/achievements/{achievement_id}/unlock", web::post().to(unlock_user_achievement))
            // 週屬性相關路由
            .route("/api/users/{user_id}/attributes/weekly/{weeks_ago}", web::get().to(get_weekly_attributes))
            // AI 任務生成路由
            .route("/api/tasks/generate", web::post().to(crate::ai_tasks::generate_task_with_ai))
            .route("/api/tasks/generate-json", web::post().to(crate::ai_tasks::generate_task_json))
            .route("/api/tasks/insert-json", web::post().to(crate::ai_tasks::insert_task_from_json))
            .route("/api/tasks/create-from-json", web::post().to(crate::ai_tasks::create_task_from_json))
            .route("/api/tasks/validate-preview", web::post().to(crate::ai_tasks::validate_and_preview_task))
            .route("/api/tasks/generate-from-chat", web::post().to(crate::ai_tasks::generate_task_from_chat))
            .route("/api/tasks/generate-with-expert", web::post().to(crate::ai_tasks::generate_task_with_expert))
            .route("/api/tasks/match-expert", web::post().to(crate::ai_tasks::match_expert_only))
            .route("/api/tasks/expert-analysis", web::post().to(crate::ai_tasks::expert_analysis))
            .route("/api/tasks/generate-subtasks", web::post().to(crate::ai_tasks::generate_subtasks_for_task))
            // AI 成就生成路由
            .route("/api/achievements/generate", web::post().to(generate_achievement_with_ai))
            .route(
                "/api/achievements/generate-from-tasks/{user_id}",
                web::post().to(crate::ai_tasks::generate_achievement_from_tasks),
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
    })
    .workers(2)
    .bind(&server_addr)?
    .run()
    .await
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

async fn migrate_database(rb: &RBatis) {
    // 添加職業任務相關欄位到 task 表
    let alter_table_queries = vec![
        "ALTER TABLE user ADD COLUMN password_hash TEXT",
        "ALTER TABLE task ADD COLUMN career_mainline_id TEXT",
        "ALTER TABLE task ADD COLUMN task_category TEXT",
        "ALTER TABLE task ADD COLUMN attributes TEXT",
        "ALTER TABLE quiz_results ADD COLUMN updated_at TEXT",
        "ALTER TABLE skill ADD COLUMN attribute TEXT DEFAULT 'intelligence'",
        // 確保 email 唯一
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_user_email_unique ON user(email)",
    ];

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

