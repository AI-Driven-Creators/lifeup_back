use rbatis::RBatis;
use uuid::Uuid;
use chrono::{Utc, Duration, Datelike, NaiveDate};
use log::{info, error};
use rand::Rng;

/// 插入種子數據到數據庫
pub async fn seed_database(rb: &RBatis) -> Result<(), Box<dyn std::error::Error>> {
    info!("開始插入種子數據...");

    // 插入測試用戶
    let user_id = insert_test_user(rb).await?;
    
    // 插入遊戲化用戶資料
    insert_user_profile(rb, &user_id).await?;
    
    // 插入用戶屬性
    insert_user_attributes(rb, &user_id).await?;
    
    // 插入任務數據
    insert_test_tasks(rb, &user_id).await?;
    
    // 插入技能數據
    insert_test_skills(rb, &user_id).await?;
    
    // 插入聊天記錄
    insert_test_chat_messages(rb, &user_id).await?;
    
    // 插入成就數據
    insert_achievements(rb).await?;
    
    // 插入用戶成就關聯
    insert_user_achievements(rb, &user_id).await?;
    
    // 插入每日進度數據
    insert_daily_progress(rb, &user_id).await?;
    
    // 插入週屬性快照數據
    insert_weekly_attribute_snapshots(rb, &user_id).await?;

    info!("種子數據插入完成！");
    Ok(())
}

/// 插入測試用戶
async fn insert_test_user(rb: &RBatis) -> Result<String, Box<dyn std::error::Error>> {
    let user_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    
    let sql = r#"
        INSERT INTO user (id, name, email, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?)
    "#;
    
    match rb.exec(sql, vec![
        user_id.clone().into(),
        "小雅".into(),
        "xiaoya@lifeup.com".into(),
        now.clone().into(),
        now.into(),
    ]).await {
        Ok(_) => {
            info!("測試用戶插入成功: {}", user_id);
            Ok(user_id)
        }
        Err(e) => {
            error!("測試用戶插入失敗: {}", e);
            Err(e.into())
        }
    }
}

/// 插入測試任務數據
async fn insert_test_tasks(rb: &RBatis, user_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let now = Utc::now();
    
    // 主任務數據
    let main_tasks = vec![
        (
            "學習 Vue.js 開發",
            "從基礎到進階學習 Vue.js，建立完整的知識體系",
            "main", 4, 150, 1, // in_progress
            true, // is_parent_task
        ),
        (
            "掌握 Rust 程式語言", 
            "深入學習 Rust 語言，掌握系統程式設計",
            "main", 5, 200, 0, // pending
            true,
        ),
        (
            "建立健康作息",
            "養成良好的生活習慣，提升生活品質",
            "main", 3, 100, 1, // in_progress
            true,
        ),
        (
            "開發個人專案",
            "完成一個完整的全端專案",
            "main", 4, 180, 1, // in_progress
            true,
        ),
        (
            "準備證照考試",
            "準備相關技術證照考試",
            "main", 3, 120, 4, // paused (用於測試父任務暫停)
            true,
        ),
    ];

    let mut main_task_ids = Vec::new();
    
    for (i, (title, desc, task_type, difficulty, exp, status, is_parent)) in main_tasks.iter().enumerate() {
        let task_id = Uuid::new_v4().to_string();
        let created_at = (now - Duration::days(30 - i as i64)).to_rfc3339();
        let updated_at = (now - Duration::days(i as i64)).to_rfc3339();
        
        let sql = r#"
            INSERT INTO task (id, user_id, title, description, status, priority, task_type, 
                            difficulty, experience, is_parent_task, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;
        
        rb.exec(sql, vec![
            task_id.clone().into(),
            user_id.into(),
            title.to_string().into(),
            desc.to_string().into(),
            (*status as i32).into(),
            1i32.into(),
            task_type.to_string().into(),
            (*difficulty as i32).into(),
            (*exp as i32).into(),
            (*is_parent).into(),
            created_at.into(),
            updated_at.into(),
        ]).await?;
        
        main_task_ids.push(task_id);
    }

    // 側任務
    let side_tasks = vec![
        ("閱讀技術書籍", "每週閱讀技術相關書籍", "side", 2, 50, 0), // pending
        ("學習設計軟體", "掌握 Figma 和 Photoshop", "side", 3, 80, 4), // paused
        ("整理工作環境", "優化工作空間配置", "side", 1, 30, 2), // completed
        ("建立個人品牌", "經營技術部落格和社群媒體", "side", 3, 90, 1), // in_progress
    ];

    let mut side_task_ids = Vec::new();
    
    for (i, (title, desc, task_type, difficulty, exp, status)) in side_tasks.iter().enumerate() {
        let task_id = Uuid::new_v4().to_string();
        let created_at = (now - Duration::days(20 - i as i64)).to_rfc3339();
        let updated_at = (now - Duration::days(5 - i as i64)).to_rfc3339();
        
        let sql = r#"
            INSERT INTO task (id, user_id, title, description, status, priority, task_type, 
                            difficulty, experience, is_parent_task, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;
        
        rb.exec(sql, vec![
            task_id.clone().into(),
            user_id.into(),
            title.to_string().into(),
            desc.to_string().into(),
            (*status as i32).into(),
            1i32.into(),
            task_type.to_string().into(),
            (*difficulty as i32).into(),
            (*exp as i32).into(),
            true.into(), // 修改：設定支線任務為大任務，允許生成子任務
            created_at.into(),
            updated_at.into(),
        ]).await?;
        
        side_task_ids.push(task_id);
    }

    // 挑戰任務
    let challenge_tasks = vec![
        ("完成馬拉松", "完成42.195公里馬拉松賽事", "challenge", 5, 500, 0, 0), // pending
        ("學會吉他演奏", "能夠彈奏基礎歌曲", "challenge", 4, 300, 1, 0), // in_progress
        ("發表技術文章", "在知名平台發表技術文章", "challenge", 3, 200, 3, 2), // cancelled, 取消2次
    ];

    for (title, desc, task_type, difficulty, exp, status, cancel_count) in challenge_tasks {
        let task_id = Uuid::new_v4().to_string();
        let created_at = (now - Duration::days(15)).to_rfc3339();
        let updated_at = (now - Duration::days(2)).to_rfc3339();
        let last_cancelled_at = if cancel_count > 0 { 
            Some((now - Duration::days(1)).to_rfc3339()) 
        } else { 
            None 
        };
        
        let sql = r#"
            INSERT INTO task (id, user_id, title, description, status, priority, task_type, 
                            difficulty, experience, is_parent_task, cancel_count, last_cancelled_at,
                            created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;
        
        rb.exec(sql, vec![
            task_id.into(),
            user_id.into(),
            title.into(),
            desc.into(),
            status.into(),
            1i32.into(),
            task_type.into(),
            difficulty.into(),
            exp.into(),
            true.into(), // 修改：設定挑戰任務為大任務，允許生成子任務
            cancel_count.into(),
            last_cancelled_at.map(|s| s.into()).unwrap_or_else(|| rbs::Value::Null),
            created_at.into(),
            updated_at.into(),
        ]).await?;
    }

    // 每日任務
    let daily_tasks = vec![
        ("冥想 15 分鐘", "每日冥想練習，培養專注力", "daily", 1, 20, 2), // completed
        ("閱讀 30 分鐘", "每日閱讀習慣", "daily", 2, 25, 1), // in_progress
        ("運動 45 分鐘", "保持身體健康", "daily", 3, 40, 0), // pending
        ("寫日記", "記錄每日生活和想法", "daily", 1, 15, 4), // paused
        ("學習新單字", "擴展詞彙量", "daily", 2, 20, 1), // in_progress
    ];

    for (title, desc, task_type, difficulty, exp, status) in daily_tasks {
        let task_id = Uuid::new_v4().to_string();
        let created_at = (now - Duration::days(10)).to_rfc3339();
        let updated_at = now.to_rfc3339();
        
        let sql = r#"
            INSERT INTO task (id, user_id, title, description, status, priority, task_type, 
                            difficulty, experience, is_parent_task, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;
        
        rb.exec(sql, vec![
            task_id.into(),
            user_id.into(),
            title.into(),
            desc.into(),
            status.into(),
            1i32.into(),
            task_type.into(),
            difficulty.into(),
            exp.into(),
            false.into(),
            created_at.into(),
            updated_at.into(),
        ]).await?;
    }

    // 為所有主任務插入子任務
    for (i, task_id) in main_task_ids.iter().enumerate() {
        match i {
            0 => insert_subtasks_for_vuejs(rb, user_id, task_id).await?, // Vue.js 學習
            1 => insert_subtasks_for_rust(rb, user_id, task_id).await?, // Rust 程式語言
            2 => insert_subtasks_for_health(rb, user_id, task_id).await?, // 建立健康作息
            3 => insert_subtasks_for_project(rb, user_id, task_id).await?, // 開發個人專案
            4 => insert_paused_subtasks(rb, user_id, task_id).await?, // 證照考試(暫停)
            _ => {}
        }
    }
    
    // 為所有支線任務插入子任務
    for (i, task_id) in side_task_ids.iter().enumerate() {
        match i {
            0 => insert_subtasks_for_reading(rb, user_id, task_id).await?, // 閱讀技術書籍
            1 => insert_subtasks_for_design(rb, user_id, task_id).await?, // 學習設計軟體
            2 => insert_subtasks_for_workspace(rb, user_id, task_id).await?, // 整理工作環境
            3 => insert_subtasks_for_branding(rb, user_id, task_id).await?, // 建立個人品牌
            _ => {}
        }
    }

    info!("測試任務數據插入完成");
    Ok(())
}

/// 為 Vue.js 學習任務插入子任務
async fn insert_subtasks_for_vuejs(rb: &RBatis, user_id: &str, parent_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let subtasks = vec![
        ("環境設置", "安裝 Node.js 和 Vue CLI", 1, 20, 2, 1), // completed
        ("基礎概念學習", "學習 Vue.js 基本概念", 2, 30, 2, 2), // completed  
        ("組件開發", "掌握組件化開發", 3, 50, 1, 3), // in_progress
        ("狀態管理", "學習 Vuex/Pinia", 4, 60, 0, 4), // pending
        ("專案實作", "完成實際專案開發", 4, 80, 0, 5), // pending
    ];

    let now = Utc::now();
    
    for (title, desc, difficulty, exp, status, order) in subtasks {
        let task_id = Uuid::new_v4().to_string();
        let created_at = (now - Duration::days(25 - order)).to_rfc3339();
        let updated_at = (now - Duration::days(order)).to_rfc3339();
        
        let sql = r#"
            INSERT INTO task (id, user_id, title, description, status, priority, task_type, 
                            difficulty, experience, parent_task_id, is_parent_task, task_order,
                            created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;
        
        rb.exec(sql, vec![
            task_id.into(),
            user_id.into(),
            title.into(),
            desc.into(),
            status.into(),
            1i32.into(),
            "subtask".into(),
            difficulty.into(),
            exp.into(),
            parent_id.into(),
            false.into(),
            order.into(),
            created_at.into(),
            updated_at.into(),
        ]).await?;
    }
    
    Ok(())
}

/// 為暫停的主任務插入暫停的子任務
async fn insert_paused_subtasks(rb: &RBatis, user_id: &str, parent_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let subtasks = vec![
        ("準備階段", "收集考試資料和教材", 1, 20, 4, 1), // paused
        ("基礎學習", "掌握基本概念", 2, 30, 4, 2), // paused
        ("深入研讀", "深入學習進階內容", 3, 40, 4, 3), // paused
        ("模擬考試", "進行模擬測試", 3, 30, 4, 4), // paused
    ];

    let now = Utc::now();
    
    for (title, desc, difficulty, exp, status, order) in subtasks {
        let task_id = Uuid::new_v4().to_string();
        let created_at = (now - Duration::days(20)).to_rfc3339();
        let updated_at = (now - Duration::days(5)).to_rfc3339();
        
        let sql = r#"
            INSERT INTO task (id, user_id, title, description, status, priority, task_type, 
                            difficulty, experience, parent_task_id, is_parent_task, task_order,
                            created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;
        
        rb.exec(sql, vec![
            task_id.into(),
            user_id.into(),
            title.into(),
            desc.into(),
            status.into(),
            1i32.into(),
            "subtask".into(),
            difficulty.into(),
            exp.into(),
            parent_id.into(),
            false.into(),
            order.into(),
            created_at.into(),
            updated_at.into(),
        ]).await?;
    }
    
    Ok(())
}

/// 為 Rust 程式語言任務插入子任務
async fn insert_subtasks_for_rust(rb: &RBatis, user_id: &str, parent_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let subtasks = vec![
        ("安裝 Rust 環境", "安裝 Rust 工具鏈和 IDE", 1, 15, 0, 1), // pending
        ("學習語法基礎", "掌握變量、函數、控制流程", 2, 25, 0, 2), // pending
        ("所有權系統", "理解所有權、借用和生命週期", 4, 40, 0, 3), // pending
    ];

    let now = Utc::now();
    
    for (title, desc, difficulty, exp, status, order) in subtasks {
        let task_id = Uuid::new_v4().to_string();
        let created_at = (now - Duration::days(25 - order)).to_rfc3339();
        let updated_at = (now - Duration::days(order)).to_rfc3339();
        
        let sql = r#"
            INSERT INTO task (id, user_id, title, description, status, priority, task_type, 
                            difficulty, experience, parent_task_id, is_parent_task, task_order,
                            created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;
        
        rb.exec(sql, vec![
            task_id.into(),
            user_id.into(),
            title.into(),
            desc.into(),
            status.into(),
            1i32.into(),
            "subtask".into(),
            difficulty.into(),
            exp.into(),
            parent_id.into(),
            false.into(),
            order.into(),
            created_at.into(),
            updated_at.into(),
        ]).await?;
    }
    
    Ok(())
}

/// 為建立健康作息任務插入子任務
async fn insert_subtasks_for_health(rb: &RBatis, user_id: &str, parent_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let subtasks = vec![
        ("制定睡眠計劃", "建立固定的作息時間", 1, 20, 1, 1), // in_progress
        ("規律運動", "建立每日運動習慣", 2, 30, 0, 2), // pending
        ("健康飲食", "規劃營養均衡的飲食", 2, 25, 0, 3), // pending
    ];

    let now = Utc::now();
    
    for (title, desc, difficulty, exp, status, order) in subtasks {
        let task_id = Uuid::new_v4().to_string();
        let created_at = (now - Duration::days(25 - order)).to_rfc3339();
        let updated_at = (now - Duration::days(order)).to_rfc3339();
        
        let sql = r#"
            INSERT INTO task (id, user_id, title, description, status, priority, task_type, 
                            difficulty, experience, parent_task_id, is_parent_task, task_order,
                            created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;
        
        rb.exec(sql, vec![
            task_id.into(),
            user_id.into(),
            title.into(),
            desc.into(),
            status.into(),
            1i32.into(),
            "subtask".into(),
            difficulty.into(),
            exp.into(),
            parent_id.into(),
            false.into(),
            order.into(),
            created_at.into(),
            updated_at.into(),
        ]).await?;
    }
    
    Ok(())
}

/// 為開發個人專案任務插入子任務
async fn insert_subtasks_for_project(rb: &RBatis, user_id: &str, parent_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let subtasks = vec![
        ("專案規劃", "確定專案需求和技術架構", 3, 35, 1, 1), // in_progress
        ("前端開發", "開發使用者介面", 4, 50, 0, 2), // pending
        ("後端開發", "開發 API 和資料庫", 4, 45, 0, 3), // pending
        ("部署上線", "將專案部署到生產環境", 3, 30, 0, 4), // pending
    ];

    let now = Utc::now();
    
    for (title, desc, difficulty, exp, status, order) in subtasks {
        let task_id = Uuid::new_v4().to_string();
        let created_at = (now - Duration::days(25 - order)).to_rfc3339();
        let updated_at = (now - Duration::days(order)).to_rfc3339();
        
        let sql = r#"
            INSERT INTO task (id, user_id, title, description, status, priority, task_type, 
                            difficulty, experience, parent_task_id, is_parent_task, task_order,
                            created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;
        
        rb.exec(sql, vec![
            task_id.into(),
            user_id.into(),
            title.into(),
            desc.into(),
            status.into(),
            1i32.into(),
            "subtask".into(),
            difficulty.into(),
            exp.into(),
            parent_id.into(),
            false.into(),
            order.into(),
            created_at.into(),
            updated_at.into(),
        ]).await?;
    }
    
    Ok(())
}

/// 為閱讀技術書籍任務插入子任務
async fn insert_subtasks_for_reading(rb: &RBatis, user_id: &str, parent_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let subtasks = vec![
        ("選擇書籍", "挑選合適的技術書籍", 1, 10, 0, 1), // pending
        ("制定閱讀計劃", "規劃每週閱讀進度", 2, 15, 0, 2), // pending
        ("撰寫筆記", "整理讀書心得", 2, 20, 0, 3), // pending
    ];

    let now = Utc::now();
    
    for (title, desc, difficulty, exp, status, order) in subtasks {
        let task_id = Uuid::new_v4().to_string();
        let created_at = (now - Duration::days(25 - order)).to_rfc3339();
        let updated_at = (now - Duration::days(order)).to_rfc3339();
        
        let sql = r#"
            INSERT INTO task (id, user_id, title, description, status, priority, task_type, 
                            difficulty, experience, parent_task_id, is_parent_task, task_order,
                            created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;
        
        rb.exec(sql, vec![
            task_id.into(),
            user_id.into(),
            title.into(),
            desc.into(),
            status.into(),
            1i32.into(),
            "subtask".into(),
            difficulty.into(),
            exp.into(),
            parent_id.into(),
            false.into(),
            order.into(),
            created_at.into(),
            updated_at.into(),
        ]).await?;
    }
    
    Ok(())
}

/// 為學習設計軟體任務插入子任務
async fn insert_subtasks_for_design(rb: &RBatis, user_id: &str, parent_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let subtasks = vec![
        ("學習 Figma 基礎", "掌握 Figma 基本操作", 2, 25, 4, 1), // paused
        ("學習 Photoshop", "掌握圖像處理技巧", 3, 30, 4, 2), // paused
        ("實作設計項目", "完成實際設計作品", 3, 25, 4, 3), // paused
    ];

    let now = Utc::now();
    
    for (title, desc, difficulty, exp, status, order) in subtasks {
        let task_id = Uuid::new_v4().to_string();
        let created_at = (now - Duration::days(25 - order)).to_rfc3339();
        let updated_at = (now - Duration::days(order)).to_rfc3339();
        
        let sql = r#"
            INSERT INTO task (id, user_id, title, description, status, priority, task_type, 
                            difficulty, experience, parent_task_id, is_parent_task, task_order,
                            created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;
        
        rb.exec(sql, vec![
            task_id.into(),
            user_id.into(),
            title.into(),
            desc.into(),
            status.into(),
            1i32.into(),
            "subtask".into(),
            difficulty.into(),
            exp.into(),
            parent_id.into(),
            false.into(),
            order.into(),
            created_at.into(),
            updated_at.into(),
        ]).await?;
    }
    
    Ok(())
}

/// 為整理工作環境任務插入子任務
async fn insert_subtasks_for_workspace(rb: &RBatis, user_id: &str, parent_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let subtasks = vec![
        ("桌面整理", "清理和整理桌面空間", 1, 10, 2, 1), // completed
        ("軟體優化", "整理電腦軟體和檔案", 2, 15, 2, 2), // completed
        ("環境佈置", "優化工作氛圍", 1, 5, 2, 3), // completed
    ];

    let now = Utc::now();
    
    for (title, desc, difficulty, exp, status, order) in subtasks {
        let task_id = Uuid::new_v4().to_string();
        let created_at = (now - Duration::days(25 - order)).to_rfc3339();
        let updated_at = (now - Duration::days(order)).to_rfc3339();
        
        let sql = r#"
            INSERT INTO task (id, user_id, title, description, status, priority, task_type, 
                            difficulty, experience, parent_task_id, is_parent_task, task_order,
                            created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;
        
        rb.exec(sql, vec![
            task_id.into(),
            user_id.into(),
            title.into(),
            desc.into(),
            status.into(),
            1i32.into(),
            "subtask".into(),
            difficulty.into(),
            exp.into(),
            parent_id.into(),
            false.into(),
            order.into(),
            created_at.into(),
            updated_at.into(),
        ]).await?;
    }
    
    Ok(())
}

/// 為建立個人品牌任務插入子任務
async fn insert_subtasks_for_branding(rb: &RBatis, user_id: &str, parent_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let subtasks = vec![
        ("建立部落格", "創建技術分享部落格", 3, 35, 1, 1), // in_progress
        ("經營社群媒體", "在各平台分享技術內容", 2, 25, 0, 2), // pending
        ("參與技術社群", "加入開發者社群並互動", 2, 30, 0, 3), // pending
    ];

    let now = Utc::now();
    
    for (title, desc, difficulty, exp, status, order) in subtasks {
        let task_id = Uuid::new_v4().to_string();
        let created_at = (now - Duration::days(25 - order)).to_rfc3339();
        let updated_at = (now - Duration::days(order)).to_rfc3339();
        
        let sql = r#"
            INSERT INTO task (id, user_id, title, description, status, priority, task_type, 
                            difficulty, experience, parent_task_id, is_parent_task, task_order,
                            created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;
        
        rb.exec(sql, vec![
            task_id.into(),
            user_id.into(),
            title.into(),
            desc.into(),
            status.into(),
            1i32.into(),
            "subtask".into(),
            difficulty.into(),
            exp.into(),
            parent_id.into(),
            false.into(),
            order.into(),
            created_at.into(),
            updated_at.into(),
        ]).await?;
    }
    
    Ok(())
}

/// 插入測試技能數據
async fn insert_test_skills(rb: &RBatis, user_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let technical_skills = vec![
        ("Vue.js", "前端框架開發技能", "technical", 3, 1250, 1500, "💻"),
        ("Rust", "系統程式設計語言", "technical", 2, 800, 1200, "⚙️"),
        ("JavaScript", "動態程式語言", "technical", 4, 1800, 2000, "📝"),
        ("TypeScript", "JavaScript 超集", "technical", 3, 1100, 1500, "🔷"),
        ("UI/UX 設計", "使用者介面設計", "technical", 4, 1600, 2000, "🎨"),
        ("機器學習", "人工智慧技術", "technical", 2, 600, 1200, "🤖"),
    ];

    let soft_skills = vec![
        ("溝通", "有效的人際溝通能力", "soft", 4, 1400, 2000, "💬"),
        ("領導力", "團隊領導與管理能力", "soft", 3, 1200, 1500, "👑"),
        ("問題解決", "分析和解決複雜問題", "soft", 5, 2200, 2500, "🧩"),
        ("時間管理", "高效安排和利用時間", "soft", 2, 700, 1200, "⏰"),
        ("團隊合作", "與他人協作完成目標", "soft", 4, 1500, 2000, "🤝"),
        ("適應力", "快速適應環境變化", "soft", 3, 1000, 1500, "🔄"),
    ];

    let now = Utc::now();
    let mut all_skills = Vec::new();
    all_skills.extend(technical_skills);
    all_skills.extend(soft_skills);
    
    let skills_count = all_skills.len();
    
    for (name, desc, category, level, experience, max_experience, icon) in all_skills {
        let skill_id = Uuid::new_v4().to_string();
        let created_at = (now - Duration::days(60)).to_rfc3339();
        let updated_at = (now - Duration::days(1)).to_rfc3339();
        
        let sql = r#"
            INSERT INTO skill (id, user_id, name, description, category, level, experience, max_experience, icon, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;
        
        rb.exec(sql, vec![
            skill_id.into(),
            user_id.into(),
            name.into(),
            desc.into(),
            category.into(),
            level.into(),
            experience.into(),
            max_experience.into(),
            icon.into(),
            created_at.into(),
            updated_at.into(),
        ]).await?;
    }

    info!("測試技能數據插入完成（{} 個技能）", skills_count);
    Ok(())
}

/// 插入測試聊天記錄
async fn insert_test_chat_messages(rb: &RBatis, user_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let messages = vec![
        ("user", "你好！我想要開始學習程式設計"),
        ("coach", "很高興認識你！學習程式設計是個很棒的決定。我建議你可以從以下幾個方向開始：1. 選擇一個適合初學者的語言，比如 JavaScript 或 Python；2. 建立每日學習習慣；3. 從小專案開始實作。你比較偏向哪個方向呢？"),
        ("user", "我想學習前端開發，特別是 Vue.js"),
        ("coach", "很好的選擇！Vue.js 是一個非常適合初學者的前端框架。我建議你按照這個順序學習：1. 先掌握 HTML、CSS 和 JavaScript 基礎；2. 了解 Vue.js 的核心概念；3. 學習組件化開發；4. 掌握狀態管理。要不要我幫你制定一個詳細的學習計劃？"),
        ("user", "好的，請幫我制定計劃"),
        ("coach", "太棒了！我已經為你創建了「學習 Vue.js 開發」的主任務，並且分解成幾個階段性的子任務。你可以按照順序逐步完成，每完成一個階段就會獲得經驗值獎勵。記住，學習程式設計最重要的是保持持續性，每天進步一點點就很棒了！"),
    ];

    let now = Utc::now();
    
    for (i, (role, content)) in messages.iter().enumerate() {
        let message_id = Uuid::new_v4().to_string();
        let created_at = (now - Duration::hours(24 - i as i64 * 2)).to_rfc3339();
        
        let sql = r#"
            INSERT INTO chat_message (id, user_id, role, content, created_at)
            VALUES (?, ?, ?, ?, ?)
        "#;
        
        rb.exec(sql, vec![
            message_id.into(),
            user_id.into(),
            role.to_string().into(),
            content.to_string().into(),
            created_at.into(),
        ]).await?;
    }

    info!("測試聊天記錄插入完成");
    Ok(())
}

/// 插入用戶遊戲化資料
async fn insert_user_profile(rb: &RBatis, user_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let profile_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    
    // 生成隨機的連續登入天數 (1-100)
    let mut rng = rand::thread_rng();
    let consecutive_login_days = rng.gen_range(1..=100);
    
    info!("生成隨機連續登入天數: {} 天", consecutive_login_days);
    
    let sql = r#"
        INSERT INTO user_profile (id, user_id, level, experience, max_experience, title, 
                                  adventure_days, consecutive_login_days, persona_type, 
                                  created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    "#;
    
    match rb.exec(sql, vec![
        profile_id.into(),
        user_id.into(),
        12i32.into(),  // level
        2340i32.into(), // experience
        2500i32.into(), // max_experience
        "自律達人".into(), // title
        87i32.into(),   // adventure_days
        consecutive_login_days.into(),   // consecutive_login_days (隨機 1-100)
        "internal".into(), // persona_type
        now.clone().into(),
        now.into(),
    ]).await {
        Ok(_) => {
            info!("用戶遊戲化資料插入成功");
            Ok(())
        }
        Err(e) => {
            error!("用戶遊戲化資料插入失敗: {}", e);
            Err(e.into())
        }
    }
}

/// 插入用戶屬性
async fn insert_user_attributes(rb: &RBatis, user_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let attributes_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    
    let sql = r#"
        INSERT INTO user_attributes (id, user_id, intelligence, endurance, creativity, 
                                     social, focus, adaptability, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    "#;
    
    match rb.exec(sql, vec![
        attributes_id.into(),
        user_id.into(),
        82i32.into(),  // intelligence
        45i32.into(),  // endurance
        75i32.into(),  // creativity
        52i32.into(),  // social
        68i32.into(),  // focus
        58i32.into(),  // adaptability
        now.clone().into(),
        now.into(),
    ]).await {
        Ok(_) => {
            info!("用戶屬性插入成功");
            Ok(())
        }
        Err(e) => {
            error!("用戶屬性插入失敗: {}", e);
            Err(e.into())
        }
    }
}

/// 插入成就數據
async fn insert_achievements(rb: &RBatis) -> Result<(), Box<dyn std::error::Error>> {
    let achievements = vec![
        ("第一步", "完成第一個任務", "🎯", "task", "task_complete", 1, 50),
        ("堅持不懈", "連續 7 天完成任務", "🔥", "habit", "consecutive_days", 7, 100),
        ("學習達人", "完成 10 個學習類任務", "📚", "learning", "learning_task_complete", 10, 150),
        ("技能大師", "任一技能達到 5 級", "⭐", "skill", "skill_level", 5, 200),
        ("社交達人", "社交力屬性達到 80", "👥", "attribute", "social_attribute", 80, 100),
        ("專注力王", "專注力屬性達到 90", "🎯", "attribute", "focus_attribute", 90, 120),
        ("創意無限", "創造力屬性達到 85", "🎨", "attribute", "creativity_attribute", 85, 110),
        ("智慧之光", "智力屬性達到 90", "💡", "attribute", "intelligence_attribute", 90, 130),
        ("堅毅如山", "毅力屬性達到 80", "⛰️", "attribute", "endurance_attribute", 80, 100),
        ("靈活應變", "適應力屬性達到 85", "🌊", "attribute", "adaptability_attribute", 85, 115),
    ];

    let now = Utc::now().to_rfc3339();
    
    for (name, desc, icon, category, req_type, req_value, exp_reward) in achievements {
        let achievement_id = Uuid::new_v4().to_string();
        
        let sql = r#"
            INSERT INTO achievement (id, name, description, icon, category, requirement_type, 
                                     requirement_value, experience_reward, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;
        
        rb.exec(sql, vec![
            achievement_id.into(),
            name.into(),
            desc.into(),
            icon.into(),
            category.into(),
            req_type.into(),
            req_value.into(),
            exp_reward.into(),
            now.clone().into(),
        ]).await?;
    }

    info!("成就數據插入完成");
    Ok(())
}

/// 插入用戶成就關聯 (已達成的成就)
async fn insert_user_achievements(rb: &RBatis, user_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    // 獲取前幾個成就作為已達成
    let achieved_names = vec!["第一步", "學習達人", "智慧之光"];
    
    // 先查詢這些成就的 ID
    for achievement_name in achieved_names {
        let achievement_query = r#"SELECT id FROM achievement WHERE name = ?"#;
        let achievement_result: Vec<serde_json::Value> = rb.query_decode(achievement_query, vec![achievement_name.into()]).await?;
        
        if let Some(achievement) = achievement_result.first() {
            if let Some(achievement_id) = achievement.get("id").and_then(|v| v.as_str()) {
                let user_achievement_id = Uuid::new_v4().to_string();
                let achieved_at = (Utc::now() - Duration::days(5)).to_rfc3339();
                
                let sql = r#"
                    INSERT INTO user_achievement (id, user_id, achievement_id, achieved_at, progress)
                    VALUES (?, ?, ?, ?, ?)
                "#;
                
                rb.exec(sql, vec![
                    user_achievement_id.into(),
                    user_id.into(),
                    achievement_id.into(),
                    achieved_at.into(),
                    100i32.into(), // 完成進度
                ]).await?;
            }
        }
    }

    info!("用戶成就關聯插入完成");
    Ok(())
}

/// 插入每日進度數據
async fn insert_daily_progress(rb: &RBatis, user_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let now = Utc::now();
    
    // 插入過去幾天的進度記錄
    for i in 0..7 {
        let date = (now - Duration::days(i)).format("%Y-%m-%d").to_string();
        let progress_id = Uuid::new_v4().to_string();
        let created_at = (now - Duration::days(i)).to_rfc3339();
        
        // 模擬不同的每日進度
        let mut rng = rand::thread_rng();
        let (completed, total, exp_gained) = if i == 0 {
            // 今天：隨機生成
            let total_tasks = rng.gen_range(3..=8);
            let completed_tasks = rng.gen_range(1..=total_tasks);
            let experience = completed_tasks * rng.gen_range(20..=50);
            (completed_tasks, total_tasks, experience)
        } else {
            // 過去幾天：使用預設值
            match i {
                1 => (4, 5, 200),  // 昨天
                2 => (5, 5, 250),  // 前天
                3 => (2, 5, 100),
                4 => (3, 4, 175),
                5 => (4, 6, 220),
                6 => (1, 3, 75),
                _ => (3, 5, 150),
            }
        };
        
        // 屬性增長 JSON
        let intelligence_gain = if i == 0 { rng.gen_range(1..=5) } else { 2 };
        let endurance_gain = if i == 0 { rng.gen_range(0..=3) } else { 1 };
        let attributes_gained = format!(r#"{{"intelligence": {}, "endurance": {}}}"#, intelligence_gain, endurance_gain);
        
        if i == 0 {
            info!("生成隨機今日進度: 完成 {}/{} 任務，獲得 {} 經驗，智力 +{}，耐力 +{}", 
                  completed, total, exp_gained, intelligence_gain, endurance_gain);
        }
        
        let sql = r#"
            INSERT INTO daily_progress (id, user_id, date, completed_tasks, total_tasks, 
                                        experience_gained, attributes_gained, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;
        
        rb.exec(sql, vec![
            progress_id.into(),
            user_id.into(),
            date.into(),
            completed.into(),
            total.into(),
            exp_gained.into(),
            attributes_gained.into(),
            created_at.clone().into(),
            created_at.into(),
        ]).await?;
    }

    info!("每日進度數據插入完成");
    Ok(())
}

/// 插入週屬性快照數據
async fn insert_weekly_attribute_snapshots(rb: &RBatis, user_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let now = Utc::now();
    let mut rng = rand::thread_rng();
    
    // 生成過去 8 週的屬性快照數據（包含本週）
    for weeks_ago in 0..8 {
        let target_date = now - Duration::weeks(weeks_ago);
        let target_naive = target_date.naive_utc().date();
        
        // 計算該週的週一日期（ISO 8601 標準）
        let days_from_monday = target_naive.weekday().days_since(chrono::Weekday::Mon);
        let week_start = target_naive - Duration::days(days_from_monday as i64);
        
        // 計算 ISO 週數和年份
        let iso_week = week_start.iso_week();
        let year = iso_week.year();
        let week_number = iso_week.week();
        
        let snapshot_id = Uuid::new_v4().to_string();
        let created_at = target_date.to_rfc3339();
        
        // 計算該週的屬性值 - 基於當前屬性值生成歷史變化
        // 假設屬性有隨機波動，但總體趨勢是成長的
        let base_intelligence = 82i32;
        let base_endurance = 45i32;
        let base_creativity = 75i32;
        let base_social = 52i32;
        let base_focus = 68i32;
        let base_adaptability = 58i32;
        
        // 根據週數計算屬性差異，越久以前的數值越低
        let growth_factor = if weeks_ago == 0 { 0.0 } else { weeks_ago as f32 * 0.5 }; // 每週約降低0.5的屬性
        let random_variance = 3; // 隨機波動範圍
        
        let intelligence = std::cmp::max(30, base_intelligence - (growth_factor as i32) + rng.gen_range(-random_variance..=random_variance));
        let endurance = std::cmp::max(30, base_endurance - (growth_factor as i32) + rng.gen_range(-random_variance..=random_variance));
        let creativity = std::cmp::max(30, base_creativity - (growth_factor as i32) + rng.gen_range(-random_variance..=random_variance));
        let social = std::cmp::max(30, base_social - (growth_factor as i32) + rng.gen_range(-random_variance..=random_variance));
        let focus = std::cmp::max(30, base_focus - (growth_factor as i32) + rng.gen_range(-random_variance..=random_variance));
        let adaptability = std::cmp::max(30, base_adaptability - (growth_factor as i32) + rng.gen_range(-random_variance..=random_variance));
        
        if weeks_ago == 0 {
            info!("生成本週屬性快照: 智力 {}, 專注 {}, 創意 {}, 社交 {}, 適應 {}, 耐力 {}", 
                  intelligence, focus, creativity, social, adaptability, endurance);
        } else if weeks_ago == 1 {
            info!("生成上週屬性快照: 智力 {}, 專注 {}, 創意 {}, 社交 {}, 適應 {}, 耐力 {}", 
                  intelligence, focus, creativity, social, adaptability, endurance);
        }
        
        let sql = r#"
            INSERT INTO weekly_attribute_snapshot 
            (id, user_id, week_start_date, year, week_number, intelligence, endurance, 
             creativity, social, focus, adaptability, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;
        
        rb.exec(sql, vec![
            snapshot_id.into(),
            user_id.into(),
            week_start.format("%Y-%m-%d").to_string().into(),
            year.into(),
            (week_number as i32).into(),
            intelligence.into(),
            endurance.into(),
            creativity.into(),
            social.into(),
            focus.into(),
            adaptability.into(),
            created_at.into(),
        ]).await?;
    }
    
    info!("週屬性快照數據插入完成（8 週數據）");
    Ok(())
}