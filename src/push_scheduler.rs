use rbatis::RBatis;
use tokio_cron_scheduler::{Job, JobScheduler};
use log::{info, error};
use std::env;
use crate::models::PushNotificationPayload;
use crate::push_service::PushService;
use chrono::Utc;

/// 啟動推送通知調度器
pub async fn start_push_scheduler(rb: RBatis) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 獲取推送間隔配置（秒）
    let push_interval = env::var("PUSH_INTERVAL_SECONDS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(60); // 默認60秒（每分鐘）

    info!("啟動推送通知調度器，間隔: {} 秒", push_interval);

    let scheduler = JobScheduler::new().await?;

    // 根據間隔創建 cron 表達式
    let cron_expr = if push_interval == 60 {
        // 每分鐘執行
        "0 * * * * *"
    } else if push_interval == 3600 {
        // 每小時執行
        "0 0 * * * *"
    } else if push_interval == 86400 {
        // 每天執行（上午 9 點）
        "0 0 9 * * *"
    } else {
        // 自定義間隔，使用每分鐘，然後在任務中檢查
        "0 * * * * *"
    };

    info!("使用 Cron 表達式: {}", cron_expr);

    // 克隆 RBatis 以便在閉包中使用
    let rb_for_job = rb.clone();
    let last_run = std::sync::Arc::new(tokio::sync::Mutex::new(Utc::now()));

    let job = Job::new_async(cron_expr, move |_uuid, _l| {
        let rb = rb_for_job.clone();
        let last_run = last_run.clone();
        let interval = push_interval;

        Box::pin(async move {
            // 檢查是否到了執行時間（對於非標準間隔）
            let mut last = last_run.lock().await;
            let now = Utc::now();
            let elapsed = (now - *last).num_seconds() as u64;

            if elapsed < interval {
                // 還沒到執行時間
                return;
            }

            *last = now;
            drop(last); // 釋放鎖

            info!("執行定時推送通知任務");

            match send_scheduled_notification(&rb).await {
                Ok((success, failed)) => {
                    info!("定時推送完成：成功 {} 個，失敗 {} 個", success, failed);
                }
                Err(e) => {
                    error!("定時推送失敗: {}", e);
                }
            }
        })
    })?;

    scheduler.add(job).await?;
    scheduler.start().await?;

    info!("推送通知調度器已啟動");

    // 在後台運行調度器
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    });

    Ok(())
}

/// 發送定時推送通知
async fn send_scheduled_notification(rb: &RBatis) -> Result<(usize, usize), Box<dyn std::error::Error + Send + Sync>> {
    let push_service = PushService::new()?;

    // 構建推送內容
    let payload = PushNotificationPayload {
        title: "人生升級系統".to_string(),
        body: "嘿！記得回來查看你的進度喔 💪".to_string(),
        icon: Some("/icon.svg".to_string()),
        badge: Some("/icon.svg".to_string()),
        tag: Some("scheduled-notification".to_string()),
        data: Some(serde_json::json!({
            "url": "/",
            "timestamp": Utc::now().to_rfc3339(),
            "type": "scheduled"
        })),
    };

    // 廣播給所有訂閱者
    push_service.broadcast_notification(rb, &payload).await
}
