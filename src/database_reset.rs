use rbatis::RBatis;
use log::{info, error, warn};
use std::env;

/// 重置數據庫 - 刪除並重建所有表
pub async fn reset_database(rb: &RBatis) -> Result<(), Box<dyn std::error::Error>> {
    // 安全檢查：僅在開發環境執行
    let environment = env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string());
    if environment != "development" {
        error!("數據庫重置僅允許在開發環境執行！當前環境: {}", environment);
        return Err("Production environment reset not allowed".into());
    }

    info!("開始重置數據庫...");

    // 刪除所有表（按依賴順序）
    let drop_tables = vec![
        "DROP TABLE IF EXISTS chat_message",
        "DROP TABLE IF EXISTS recurring_task_template", 
        "DROP TABLE IF EXISTS task",
        "DROP TABLE IF EXISTS skill",
        "DROP TABLE IF EXISTS user",
    ];

    for sql in drop_tables {
        match rb.exec(sql, vec![]).await {
            Ok(_) => info!("成功刪除表"),
            Err(e) => warn!("刪除表失敗（可能不存在）: {}", e),
        }
    }

    // 重新建立所有表
    create_all_tables(rb).await?;
    
    info!("數據庫重置完成！");
    Ok(())
}

/// 建立所有數據庫表
async fn create_all_tables(rb: &RBatis) -> Result<(), Box<dyn std::error::Error>> {
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
            Ok(_) => info!("數據庫表 {} 建立成功", i + 1),
            Err(e) => {
                error!("數據庫表 {} 建立失敗: {}", i + 1, e);
                return Err(e.into());
            }
        }
    }
    
    info!("所有數據庫表建立完成");
    Ok(())
}