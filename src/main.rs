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
    // 載入配置
    let config = Config::from_env();
    
    // 初始化日誌
    fast_log::init(fast_log::Config::new().console()).expect("日誌初始化失敗");
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
            due_date TEXT,
            created_at TEXT,
            updated_at TEXT,
            FOREIGN KEY (user_id) REFERENCES user (id)
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
    ];

    for (i, sql) in tables.iter().enumerate() {
        match rb.exec(sql, vec![]).await {
            Ok(_) => log::info!("資料庫表 {} 建立成功", i + 1),
            Err(e) => log::error!("資料庫表 {} 建立失敗: {}", i + 1, e),
        }
    }
    
    log::info!("所有資料庫表建立完成");
}
