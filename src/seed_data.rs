use rbatis::RBatis;
use uuid::Uuid;
use chrono::{Utc, Duration};
use log::{info, error};

/// 插入種子數據到數據庫
pub async fn seed_database(rb: &RBatis) -> Result<(), Box<dyn std::error::Error>> {
    info!("開始插入種子數據...");

    // 插入測試用戶
    let user_id = insert_test_user(rb).await?;
    
    // 插入任務數據
    insert_test_tasks(rb, &user_id).await?;
    
    // 插入技能數據
    insert_test_skills(rb, &user_id).await?;
    
    // 插入聊天記錄
    insert_test_chat_messages(rb, &user_id).await?;

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
        "測試用戶".into(),
        "test@lifeup.com".into(),
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
    let skills = vec![
        ("Vue.js", "前端框架開發技能", 3, 65.5),
        ("Rust", "系統程式設計語言", 2, 30.0),
        ("JavaScript", "動態程式語言", 4, 78.2),
        ("TypeScript", "JavaScript 超集", 3, 45.8),
        ("UI/UX 設計", "使用者介面設計", 2, 25.0),
        ("專案管理", "軟體專案管理能力", 3, 55.0),
    ];

    let now = Utc::now();
    
    for (name, desc, level, progress) in skills {
        let skill_id = Uuid::new_v4().to_string();
        let created_at = (now - Duration::days(60)).to_rfc3339();
        let updated_at = (now - Duration::days(1)).to_rfc3339();
        
        let sql = r#"
            INSERT INTO skill (id, user_id, name, description, level, progress, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#;
        
        rb.exec(sql, vec![
            skill_id.into(),
            user_id.into(),
            name.into(),
            desc.into(),
            level.into(),
            progress.into(),
            created_at.into(),
            updated_at.into(),
        ]).await?;
    }

    info!("測試技能數據插入完成");
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