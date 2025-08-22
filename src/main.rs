mod config;
mod models;
mod routes;
mod database_reset;
mod seed_data;
mod ai_service;
mod ai_tasks;
mod achievement_service;
use actix_cors::Cors;
use actix_web::{web, App, HttpServer};
use rbatis::RBatis;
use rbdc_sqlite::driver::SqliteDriver;

use config::Config;
use routes::*;
use database_reset::reset_database;
use seed_data::seed_database;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // 載入 .env 文件
    dotenv::dotenv().ok();
    
    // 處理命令行參數
    let args: Vec<String> = std::env::args().collect();
    let reset_db = args.contains(&"--reset-db".to_string());
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

    // 初始化 rbatis
    let rb = RBatis::new();
    
    // 連接資料庫
    rb.init(SqliteDriver {}, &config.database.url).unwrap();
    log::info!("資料庫連接成功: {}", config.database.url);

    // 處理數據庫重置命令
    if reset_db {
        log::info!("執行數據庫重置...");
        if let Err(e) = reset_database(&rb).await {
            log::error!("數據庫重置失敗: {}", e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()));
        }
        if let Err(e) = seed_database(&rb).await {
            log::error!("種子數據插入失敗: {}", e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()));
        }
        log::info!("數據庫重置和種子數據插入完成！");
        return Ok(());
    }
    
    // 處理僅插入種子數據命令
    if seed_only {
        log::info!("僅插入種子數據...");
        if let Err(e) = seed_database(&rb).await {
            log::error!("種子數據插入失敗: {}", e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()));
        }
        log::info!("種子數據插入完成！");
        return Ok(());
    }

    // 正常啟動模式：建立資料庫表
    create_tables(&rb).await;    
    // 處理僅插入種子數據命令
    if seed_only {
        log::info!("僅插入種子數據...");
        if let Err(e) = seed_database(&rb).await {
            log::error!("種子數據插入失敗: {}", e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()));
        }
        log::info!("種子數據插入完成！");
        return Ok(());
    }

    // 正常啟動模式：建立資料庫表
    create_tables(&rb).await;


    // 用 web::Data 包裝 rbatis 實例
    let rb_data = web::Data::new(rb);

    // 啟動 HTTP 伺服器
    let server_addr = config.server_addr();
    log::info!("啟動 HTTP 伺服器在 http://{}", server_addr);
    log::info!("Worker 數量: 2");
    
    HttpServer::new(move || {
        // 配置 CORS
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(cors)
            .app_data(rb_data.clone())
            // 健康檢查
            .route("/health", web::get().to(health_check))
            // 使用者相關路由
            .route("/api/users", web::get().to(get_users))
            .route("/api/users", web::post().to(create_user))
            .route("/api/users/{id}", web::get().to(get_user))
            // 任務相關路由
            .route("/api/tasks", web::get().to(get_tasks))
            .route("/api/tasks", web::post().to(create_task))
            .route("/api/tasks/homepage", web::get().to(get_homepage_tasks))
            .route("/api/tasks/type/{task_type}", web::get().to(get_tasks_by_type))
            .route("/api/tasks/{id}", web::get().to(get_task))
            .route("/api/tasks/{id}", web::put().to(update_task))
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
            .route("/api/chat/chatgpt", web::post().to(send_message_to_chatgpt))
            .route("/api/chat/test", web::get().to(test_endpoint))
            // 遊戲化數據相關路由
            .route("/api/users/{id}/gamified", web::get().to(get_gamified_user_data))
            // 成就相關路由
            .route("/api/achievements", web::get().to(get_achievements))
            .route("/api/users/{user_id}/achievements", web::get().to(get_user_achievements))
            .route("/api/users/{user_id}/achievements/{achievement_id}/unlock", web::post().to(unlock_user_achievement))
            // 週屬性相關路由
            .route("/api/users/{user_id}/attributes/weekly/{weeks_ago}", web::get().to(get_weekly_attributes))
            
            // AI 任務生成路由
            .route("/api/tasks/generate", web::post().to(crate::ai_tasks::generate_task_with_ai))
            .route("/api/tasks/generate-json", web::post().to(crate::ai_tasks::generate_task_json))
            .route("/api/tasks/insert-json", web::post().to(crate::ai_tasks::insert_task_from_json))
            .route("/api/tasks/create-from-json", web::post().to(crate::ai_tasks::create_task_from_json))
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
    ];

    for (i, sql) in tables.iter().enumerate() {
        match rb.exec(sql, vec![]).await {
            Ok(_) => log::info!("資料庫表 {} 建立成功", i + 1),
            Err(e) => log::error!("資料庫表 {} 建立失敗: {}", i + 1, e),
        }
    }
    
    log::info!("所有資料庫表建立完成");
}

