mod config;
mod models;
mod routes;

use actix_cors::Cors;
use actix_web::{web, App, HttpServer};
use rbatis::RBatis;
use rbdc_sqlite::driver::SqliteDriver;

use config::Config;
use routes::*;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // 載入 .env 文件
    dotenv::dotenv().ok();
    
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

    // 建立資料庫表
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
            .route("/api/tasks/{id}", web::put().to(update_task))
            .route("/api/tasks/type/{task_type}", web::get().to(get_tasks_by_type))
            .route("/api/tasks/homepage", web::get().to(get_homepage_tasks))
            .route("/api/tasks/{id}/start", web::post().to(start_task))
            .route("/api/tasks/{id}/subtasks", web::get().to(get_subtasks))
            .route("/api/tasks/{id}/pause", web::put().to(pause_task))
            .route("/api/tasks/{id}/cancel", web::put().to(cancel_task))
            // 重複性任務路由
            .route("/api/recurring-tasks", web::post().to(create_recurring_task))
            .route("/api/tasks/{id}/generate-daily", web::post().to(generate_daily_tasks))
            .route("/api/tasks/{id}/progress", web::get().to(get_task_progress))
            // 技能相關路由
            .route("/api/skills", web::get().to(get_skills))
            .route("/api/skills", web::post().to(create_skill))
            // 聊天相關路由
            .route("/api/chat/messages", web::get().to(get_chat_messages))
            .route("/api/chat/send", web::post().to(send_message))
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
            level INTEGER DEFAULT 1,
            progress REAL DEFAULT 0.0,
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
    ];

    for (i, sql) in tables.iter().enumerate() {
        match rb.exec(sql, vec![]).await {
            Ok(_) => log::info!("資料庫表 {} 建立成功", i + 1),
            Err(e) => log::error!("資料庫表 {} 建立失敗: {}", i + 1, e),
        }
    }
    
    log::info!("所有資料庫表建立完成");
}
