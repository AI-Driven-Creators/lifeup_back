
















use rbatis::RBatis;
use log::{info, error, warn};
use std::env;

/// 重置數據庫 - 刪除並重建所有表
pub async fn reset_database(rb: &RBatis) -> Result<(), Box<dyn std::error::Error>> {
    // 安全檢查：僅在開發環境執行
    let environment = env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string());
    if environment != "development" {
        error!("數據庫重置僅允許在開發環境執行！當前環境: {}", environment);
        return Err("不允許在生產環境重置資料庫".into());
    }

    info!("開始重置數據庫...");

    // 暫時關閉外鍵檢查，避免刪除順序造成的約束錯誤
    let _ = rb.exec("PRAGMA foreign_keys = OFF;", vec![]).await;

    // 刪除所有表（按依賴順序）
    let drop_tables = vec![
        // 先刪關聯子表
        "DROP TABLE IF EXISTS user_achievement",
        "DROP TABLE IF EXISTS achievement_stats",
        "DROP TABLE IF EXISTS daily_progress",
        "DROP TABLE IF EXISTS recurring_task_template",
        "DROP TABLE IF EXISTS weekly_attribute_snapshot",
        "DROP TABLE IF EXISTS chat_message",
        "DROP TABLE IF EXISTS career_mainlines",
        "DROP TABLE IF EXISTS quiz_results",
        "DROP TABLE IF EXISTS user_coach_preference",
        "DROP TABLE IF EXISTS push_subscription",
        // 再刪引用 user 的資料表
        "DROP TABLE IF EXISTS task",
        "DROP TABLE IF EXISTS skill",
        "DROP TABLE IF EXISTS user_attributes",
        "DROP TABLE IF EXISTS user_profile",
        // 最後刪主表
        "DROP TABLE IF EXISTS achievement",
        "DROP TABLE IF EXISTS user",
    ];

    for sql in drop_tables {
        match rb.exec(sql, vec![]).await {
            Ok(_) => info!("成功刪除表: {}", sql),
            Err(e) => warn!("刪除表失敗（可能不存在）: {} -> {}", sql, e),
        }
    }

    // 刪除可能存在的索引（SQLite 不會隨表自動刪索引）
    let drop_indexes = vec![
        "DROP INDEX IF EXISTS idx_user_email_unique",
    ];
    for sql in drop_indexes {
        let _ = rb.exec(sql, vec![]).await;
    }

    // 重新建立所有表
    create_all_tables(rb).await?;
    // 重新建立唯一索引
    let _ = rb.exec(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_user_email_unique ON user(email)",
        vec![]
    ).await;
    // 雙保險：確保核心表無殘留資料
    let _ = rb.exec("DELETE FROM user", vec![]).await;
    // 開啟外鍵檢查
    let _ = rb.exec("PRAGMA foreign_keys = ON;", vec![]).await;
    
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
        // 推送訂閱表
        r#"
        CREATE TABLE IF NOT EXISTS push_subscription (
            id TEXT PRIMARY KEY,
            user_id TEXT,
            endpoint TEXT NOT NULL UNIQUE,
            p256dh_key TEXT NOT NULL,
            auth_key TEXT NOT NULL,
            created_at TEXT,
            updated_at TEXT,
            FOREIGN KEY (user_id) REFERENCES user (id)
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