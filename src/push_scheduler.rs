use rbatis::RBatis;
use tokio_cron_scheduler::{Job, JobScheduler};
use log::{info, error};
use chrono::{Utc, Timelike, FixedOffset, NaiveDate, TimeZone};
use crate::models::{PushNotificationPayload, UserNotificationSettings};
use crate::push_service::PushService;
use crate::notification_generator::NotificationGenerator;
use crate::calendar_service::CalendarService;

/// 啟動推送通知調度器
pub async fn start_push_scheduler(
    rb: RBatis,
    calendar_service: CalendarService,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("啟動動態推送通知調度器");

    let scheduler = JobScheduler::new().await?;

    // 每分鐘檢查一次是否需要發送通知
    let cron_expr = "0 * * * * *"; // 每分鐘執行
    info!("使用 Cron 表達式: {}", cron_expr);

    // 克隆 RBatis 和 CalendarService 以便在閉包中使用
    let rb_for_job = rb.clone();
    let calendar_for_job = calendar_service;

    let job = Job::new_async(cron_expr, move |_uuid, _l| {
        let rb = rb_for_job.clone();
        let calendar = calendar_for_job.clone();

        Box::pin(async move {
            // 使用 UTC+8 時區
            let tz = FixedOffset::east_opt(8 * 3600).unwrap();
            let now = Utc::now().with_timezone(&tz);
            let current_time = format!("{:02}:{:02}", now.hour(), now.minute());
            let today = now.date_naive();

            info!("檢查定時推送通知任務 - 當前時間: {} (UTC+8)", current_time);

            match process_scheduled_notifications(&rb, &calendar, &current_time, today).await {
                Ok(total_sent) => {
                    if total_sent > 0 {
                        info!("定時推送完成：共發送 {} 個通知", total_sent);
                    }
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

/// 處理定時推送通知
async fn process_scheduled_notifications(
    rb: &RBatis,
    calendar: &CalendarService,
    current_time: &str,
    today: NaiveDate,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    let mut total_sent = 0;

    // 判斷今天是工作日還是假日
    let is_holiday = calendar.is_holiday(today);
    let is_workday = !is_holiday;

    info!("今天是 {}，工作日: {}, 假日: {}", today, is_workday, is_holiday);

    // 查詢所有已啟用通知的用戶設定
    let settings_list: Vec<UserNotificationSettings> = rb
        .query_decode(
            "SELECT * FROM user_notification_settings WHERE enabled = 1",
            vec![],
        )
        .await
        .unwrap_or_default();

    info!("找到 {} 個已啟用通知的用戶", settings_list.len());

    for settings in settings_list {
        // 檢查是否應該在今天發送通知
        let should_notify = if is_workday {
            settings.notify_on_workdays.unwrap_or(true)
        } else {
            settings.notify_on_holidays.unwrap_or(false)
        };

        if !should_notify {
            continue;
        }

        let user_id = match &settings.user_id {
            Some(id) => id,
            None => continue,
        };

        // 檢查早上通知
        if settings.morning_enabled.unwrap_or(false) {
            let morning_time = settings.morning_time.as_deref().unwrap_or("08:00");
            if current_time == morning_time {
                info!("為用戶 {} 發送早上通知", user_id);
                if send_morning_notification(rb, user_id).await.is_ok() {
                    total_sent += 1;
                }
            }
        }

        // 檢查晚上通知
        if settings.evening_enabled.unwrap_or(false) {
            let evening_time = settings.evening_time.as_deref().unwrap_or("22:00");
            if current_time == evening_time {
                info!("為用戶 {} 發送晚上通知", user_id);
                if send_evening_notification(rb, user_id).await.is_ok() {
                    total_sent += 1;
                }
            }
        }

        // 檢查自定義通知時段
        if let Some(custom_schedules_str) = &settings.custom_schedules {
            if let Ok(custom_schedules) = serde_json::from_str::<Vec<CustomScheduleItem>>(custom_schedules_str) {
                for schedule in custom_schedules {
                    if schedule.enabled && current_time == schedule.time {
                        info!("為用戶 {} 發送自定義通知", user_id);
                        if send_custom_notification(rb, user_id).await.is_ok() {
                            total_sent += 1;
                        }
                    }
                }
            }
        }
    }

    Ok(total_sent)
}

#[derive(serde::Deserialize)]
struct CustomScheduleItem {
    time: String,
    enabled: bool,
    #[allow(dead_code)]
    schedule_type: String,
}

/// 發送早上通知
async fn send_morning_notification(
    rb: &RBatis,
    user_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 生成通知內容
    let notification = NotificationGenerator::generate_morning_notification(rb, user_id).await?;

    // 轉換為 PushNotificationPayload
    let payload = PushNotificationPayload {
        title: notification["title"].as_str().unwrap_or("早安").to_string(),
        body: notification["body"].as_str().unwrap_or("").to_string(),
        icon: notification["icon"].as_str().map(|s| s.to_string()),
        badge: notification["badge"].as_str().map(|s| s.to_string()),
        tag: notification["tag"].as_str().map(|s| s.to_string()),
        data: notification.get("data").cloned(),
    };

    // 發送推送
    let push_service = PushService::new()?;
    push_service.send_notification_to_user(rb, user_id, &payload).await?;

    Ok(())
}

/// 發送晚上通知
async fn send_evening_notification(
    rb: &RBatis,
    user_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 生成通知內容
    let notification = NotificationGenerator::generate_evening_notification(rb, user_id).await?;

    // 轉換為 PushNotificationPayload
    let payload = PushNotificationPayload {
        title: notification["title"].as_str().unwrap_or("今天辛苦了").to_string(),
        body: notification["body"].as_str().unwrap_or("").to_string(),
        icon: notification["icon"].as_str().map(|s| s.to_string()),
        badge: notification["badge"].as_str().map(|s| s.to_string()),
        tag: notification["tag"].as_str().map(|s| s.to_string()),
        data: notification.get("data").cloned(),
    };

    // 發送推送
    let push_service = PushService::new()?;
    push_service.send_notification_to_user(rb, user_id, &payload).await?;

    Ok(())
}

/// 發送自定義通知
async fn send_custom_notification(
    rb: &RBatis,
    user_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 生成通知內容
    let notification = NotificationGenerator::generate_custom_notification(rb, user_id).await?;

    // 轉換為 PushNotificationPayload
    let payload = PushNotificationPayload {
        title: notification["title"].as_str().unwrap_or("人生升級系統").to_string(),
        body: notification["body"].as_str().unwrap_or("").to_string(),
        icon: notification["icon"].as_str().map(|s| s.to_string()),
        badge: notification["badge"].as_str().map(|s| s.to_string()),
        tag: notification["tag"].as_str().map(|s| s.to_string()),
        data: notification.get("data").cloned(),
    };

    // 發送推送
    let push_service = PushService::new()?;
    push_service.send_notification_to_user(rb, user_id, &payload).await?;

    Ok(())
}
