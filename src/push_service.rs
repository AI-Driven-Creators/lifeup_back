use rbatis::RBatis;
use crate::models::{PushSubscription, SubscribeRequest, UnsubscribeRequest, PushNotificationPayload};
use chrono::Utc;
use uuid::Uuid;
use web_push::*;
use std::env;
use std::fs::File;
use log::{info, error};
use url::Url;

/// 推送服務 - 處理Web Push Notification相關功能
pub struct PushService {
    vapid_private_key: String,
    vapid_public_key: String,
}

impl PushService {
    /// 創建新的推送服務實例
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let vapid_private_key = env::var("VAPID_PRIVATE_KEY")
            .map_err(|_| "VAPID_PRIVATE_KEY not found in environment")?;
        let vapid_public_key = env::var("VAPID_PUBLIC_KEY")
            .map_err(|_| "VAPID_PUBLIC_KEY not found in environment")?;

        Ok(Self {
            vapid_private_key,
            vapid_public_key,
        })
    }

    /// 獲取VAPID公鑰（供前端使用）
    pub fn get_public_key(&self) -> String {
        self.vapid_public_key.clone()
    }

    /// 保存推送訂閱到資料庫
    pub async fn save_subscription(
        &self,
        rb: &RBatis,
        req: SubscribeRequest,
    ) -> Result<PushSubscription, Box<dyn std::error::Error + Send + Sync>> {
        let now = Utc::now();

        // 檢查是否已存在該endpoint的訂閱
        let endpoint_for_query = req.endpoint.clone();
        let existing: Vec<PushSubscription> = rb
            .query_decode(
                "SELECT * FROM push_subscription WHERE endpoint = ?",
                vec![rbs::to_value!(endpoint_for_query)],
            )
            .await?;

        if let Some(mut subscription) = existing.into_iter().next() {
            // 更新現有訂閱
            subscription.p256dh_key = Some(req.keys.p256dh);
            subscription.auth_key = Some(req.keys.auth);
            subscription.updated_at = Some(now);
            if req.user_id.is_some() {
                subscription.user_id = req.user_id;
            }

            let p256dh_clone = subscription.p256dh_key.clone();
            let auth_clone = subscription.auth_key.clone();
            let updated_at_clone = subscription.updated_at.clone();
            let user_id_clone = subscription.user_id.clone();
            let id_clone = subscription.id.clone();

            rb.exec(
                "UPDATE push_subscription SET p256dh_key = ?, auth_key = ?, updated_at = ?, user_id = ? WHERE id = ?",
                vec![
                    rbs::to_value!(p256dh_clone),
                    rbs::to_value!(auth_clone),
                    rbs::to_value!(updated_at_clone),
                    rbs::to_value!(user_id_clone),
                    rbs::to_value!(id_clone),
                ],
            ).await?;
            info!("更新推送訂閱: {}", subscription.endpoint.as_ref().unwrap_or(&"unknown".to_string()));
            Ok(subscription)
        } else {
            // 創建新訂閱
            let subscription = PushSubscription {
                id: Some(Uuid::new_v4().to_string()),
                user_id: req.user_id,
                endpoint: Some(req.endpoint),
                p256dh_key: Some(req.keys.p256dh),
                auth_key: Some(req.keys.auth),
                created_at: Some(now),
                updated_at: Some(now),
            };

            let id_clone = subscription.id.clone();
            let user_id_clone = subscription.user_id.clone();
            let endpoint_clone = subscription.endpoint.clone();
            let p256dh_clone = subscription.p256dh_key.clone();
            let auth_clone = subscription.auth_key.clone();
            let created_at_clone = subscription.created_at.clone();
            let updated_at_clone = subscription.updated_at.clone();

            rb.exec(
                "INSERT INTO push_subscription (id, user_id, endpoint, p256dh_key, auth_key, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
                vec![
                    rbs::to_value!(id_clone),
                    rbs::to_value!(user_id_clone),
                    rbs::to_value!(endpoint_clone),
                    rbs::to_value!(p256dh_clone),
                    rbs::to_value!(auth_clone),
                    rbs::to_value!(created_at_clone),
                    rbs::to_value!(updated_at_clone),
                ],
            ).await?;
            info!("新增推送訂閱: {}", subscription.endpoint.as_ref().unwrap_or(&"unknown".to_string()));
            Ok(subscription)
        }
    }

    /// 刪除推送訂閱
    pub async fn remove_subscription(
        &self,
        rb: &RBatis,
        req: UnsubscribeRequest,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let endpoint = req.endpoint.clone();
        let result = rb
            .exec(
                "DELETE FROM push_subscription WHERE endpoint = ?",
                vec![rbs::to_value!(req.endpoint)],
            )
            .await?;

        info!("刪除推送訂閱: {}", endpoint);
        Ok(result.rows_affected > 0)
    }

    /// 獲取所有訂閱
    pub async fn get_all_subscriptions(
        &self,
        rb: &RBatis,
    ) -> Result<Vec<PushSubscription>, Box<dyn std::error::Error + Send + Sync>> {
        let subscriptions: Vec<PushSubscription> = rb
            .query_decode("SELECT * FROM push_subscription", vec![])
            .await?;

        Ok(subscriptions)
    }

    /// 發送推送通知到指定訂閱
    pub async fn send_notification(
        &self,
        subscription: &PushSubscription,
        payload: &PushNotificationPayload,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 構建訂閱信息
        let endpoint = subscription.endpoint.as_ref()
            .ok_or("Missing endpoint")?;
        let p256dh = subscription.p256dh_key.as_ref()
            .ok_or("Missing p256dh key")?;
        let auth = subscription.auth_key.as_ref()
            .ok_or("Missing auth key")?;

        let subscription_info = SubscriptionInfo::new(
            endpoint,
            p256dh,
            auth,
        );

        // 構建 VAPID 簽名構建器 - 從 PEM 文件讀取
        let pem_file = File::open("vapid_private.pem")
            .map_err(|e| format!("Failed to open PEM file: {}", e))?;

        let mut builder = VapidSignatureBuilder::from_pem(
            pem_file,
            &subscription_info,
        ).map_err(|e| format!("Failed to create VAPID signature builder: {:?}", e))?;

        builder.add_claim("sub", "mailto:noreply@lifeup-study.top");

        info!("Using PEM-based VAPID signature");
        info!("VAPID public key: {}", self.vapid_public_key);

        let signature = builder.build()?;

        // 準備推送消息內容
        let payload_json = serde_json::to_string(payload)?;

        let mut message_builder = WebPushMessageBuilder::new(&subscription_info)?;
        message_builder.set_payload(ContentEncoding::Aes128Gcm, payload_json.as_bytes());
        message_builder.set_vapid_signature(signature);

        let message = message_builder.build()?;

        // 創建Web Push客戶端並發送
        let client = WebPushClient::new()?;

        match client.send(message).await {
            Ok(_) => {
                info!("成功發送推送通知到: {}", endpoint);
                Ok(())
            },
            Err(error) => {
                error!("發送推送通知失敗: {:?}", error);
                Err(Box::new(error))
            }
        }
    }

    /// 批量發送推送通知到所有訂閱
    pub async fn broadcast_notification(
        &self,
        rb: &RBatis,
        payload: &PushNotificationPayload,
    ) -> Result<(usize, usize), Box<dyn std::error::Error + Send + Sync>> {
        let subscriptions = self.get_all_subscriptions(rb).await?;
        let total = subscriptions.len();
        let mut success = 0;
        let mut failed = 0;

        for subscription in subscriptions {
            let endpoint_clone = subscription.endpoint.clone();

            match self.send_notification(&subscription, payload).await {
                Ok(_) => success += 1,
                Err(e) => {
                    error!("發送到 {} 失敗: {}",
                        endpoint_clone.clone().unwrap_or_default(), e);
                    failed += 1;

                    // 如果訂閱失效（410 Gone），從資料庫中刪除
                    // 這裡簡化處理，實際應該檢查具體錯誤碼
                    if e.to_string().contains("410") || e.to_string().contains("expired") {
                        let _ = rb.exec(
                            "DELETE FROM push_subscription WHERE endpoint = ?",
                            vec![rbs::to_value!(endpoint_clone.unwrap_or_default())],
                        ).await;
                    }
                }
            }
        }

        info!("批量發送完成: 總數={}, 成功={}, 失敗={}", total, success, failed);
        Ok((success, failed))
    }
}
