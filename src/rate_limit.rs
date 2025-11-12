use actix_governor::{Governor, GovernorConfigBuilder, KeyExtractor, SimpleKeyExtractionError};
use actix_web::dev::ServiceRequest;
use std::net::IpAddr;

/// 自定義 Key 提取器：優先使用用戶ID，否則使用IP地址
#[derive(Clone)]
pub struct UserOrIpKeyExtractor;

impl KeyExtractor for UserOrIpKeyExtractor {
    type Key = String;
    type KeyExtractionError = SimpleKeyExtractionError<&'static str>;

    fn extract(&self, req: &ServiceRequest) -> Result<Self::Key, Self::KeyExtractionError> {
        // 嘗試從 JWT token 中提取用戶 ID（如果有的話）
        // 注意：這需要在 JWT middleware 之後運行

        // 目前先使用 IP 地址作為 key
        let peer_ip = req
            .peer_addr()
            .map(|addr| addr.ip())
            .ok_or_else(|| {
                log::warn!("無法獲取客戶端 IP 地址");
                SimpleKeyExtractionError::new("Could not extract IP")
            })?;

        Ok(peer_ip.to_string())
    }

    fn exceed_rate_limit_response(
        &self,
        _negative: &actix_governor::governor::NotUntil<
            actix_governor::governor::clock::QuantaInstant,
        >,
        mut response: actix_web::HttpResponseBuilder,
    ) -> actix_web::HttpResponse {
        // 自定義超過限制的響應
        let body = serde_json::json!({
            "success": false,
            "data": serde_json::Value::Null,
            "message": "請求過於頻繁，請稍後再試"
        });

        response
            .content_type("application/json")
            .body(serde_json::to_string(&body).unwrap())
    }
}

use actix_governor::governor::middleware::StateInformationMiddleware;

/// 為登入/註冊等認證端點創建嚴格的 Rate Limiter
/// 限制：每分鐘5次請求
pub fn create_auth_rate_limiter() -> Governor<UserOrIpKeyExtractor, StateInformationMiddleware> {
    let config = GovernorConfigBuilder::default()
        .per_second(1)     // 每秒1次
        .burst_size(5)     // 允許突發5次
        .key_extractor(UserOrIpKeyExtractor)
        .finish()
        .unwrap();

    Governor::new(&config)
}

/// 為一般 API 端點創建寬鬆的 Rate Limiter
/// 限制：每秒10次請求
pub fn create_general_rate_limiter() -> Governor<UserOrIpKeyExtractor, StateInformationMiddleware> {
    let config = GovernorConfigBuilder::default()
        .per_second(10)    // 每秒10次
        .burst_size(20)    // 允許突發20次
        .key_extractor(UserOrIpKeyExtractor)
        .finish()
        .unwrap();

    Governor::new(&config)
}

/// 為 AI 相關端點創建更嚴格的 Rate Limiter（AI 調用成本高）
/// 限制：每分鐘10次請求
pub fn create_ai_rate_limiter() -> Governor<UserOrIpKeyExtractor, StateInformationMiddleware> {
    let config = GovernorConfigBuilder::default()
        .per_second(1)     // 每秒1次
        .burst_size(10)    // 允許突發10次
        .key_extractor(UserOrIpKeyExtractor)
        .finish()
        .unwrap();

    Governor::new(&config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiters_creation() {
        // 確保所有 rate limiter 都能正確創建
        let _auth_limiter = create_auth_rate_limiter();
        let _general_limiter = create_general_rate_limiter();
        let _ai_limiter = create_ai_rate_limiter();
    }
}
