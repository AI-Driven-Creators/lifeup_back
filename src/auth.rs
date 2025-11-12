use actix_web::{dev::ServiceRequest, Error, HttpMessage, HttpResponse};
use actix_web::error::ErrorUnauthorized;
use actix_web::dev::{forward_ready, Service, ServiceResponse, Transform};
use actix_web::body::EitherBody;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation, Algorithm};
use serde::{Deserialize, Serialize};
use std::env;
use chrono::{Duration, Utc};
use std::future::{ready, Ready};
use std::pin::Pin;
use std::task::{Context, Poll};
use futures::future::LocalBoxFuture;

// JWT Claims 結構
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,      // Subject (user_id)
    pub email: String,    // User email
    pub exp: usize,       // Expiration time (timestamp)
    pub iat: usize,       // Issued at (timestamp)
}

// JWT 配置常量
const JWT_EXPIRATION_HOURS: i64 = 24; // Token 有效期 24 小時

/// 獲取 JWT 密鑰
fn get_jwt_secret() -> String {
    env::var("JWT_SECRET").unwrap_or_else(|_| {
        log::warn!("JWT_SECRET 未設置，使用默認密鑰（不安全！）");
        "your-secret-key-change-this-in-production-min-32-chars-long".to_string()
    })
}

/// 生成 JWT token
pub fn generate_jwt(user_id: &str, email: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let exp = (now + Duration::hours(JWT_EXPIRATION_HOURS)).timestamp() as usize;
    let iat = now.timestamp() as usize;

    let claims = Claims {
        sub: user_id.to_string(),
        email: email.to_string(),
        exp,
        iat,
    };

    let secret = get_jwt_secret();
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

/// 驗證 JWT token
pub fn verify_jwt(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let secret = get_jwt_secret();
    let validation = Validation::new(Algorithm::HS256);

    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map(|data| data.claims)
}

/// 從 HTTP 請求中提取 JWT token
pub fn extract_token_from_header(req: &ServiceRequest) -> Result<String, Error> {
    // 從 Authorization header 中提取 token
    let auth_header = req
        .headers()
        .get("Authorization")
        .ok_or_else(|| ErrorUnauthorized("缺少 Authorization header"))?;

    let auth_str = auth_header
        .to_str()
        .map_err(|_| ErrorUnauthorized("無效的 Authorization header"))?;

    // 檢查是否為 Bearer token
    if !auth_str.starts_with("Bearer ") {
        return Err(ErrorUnauthorized("Authorization header 必須以 'Bearer ' 開頭"));
    }

    // 提取 token（移除 "Bearer " 前綴）
    let token = auth_str[7..].to_string();
    Ok(token)
}

/// 從請求中提取並驗證 user_id
pub fn get_user_id_from_request(req: &ServiceRequest) -> Result<String, Error> {
    // 首先嘗試從 JWT token 獲取
    if let Ok(token) = extract_token_from_header(req) {
        match verify_jwt(&token) {
            Ok(claims) => return Ok(claims.sub),
            Err(e) => {
                log::warn!("JWT 驗證失敗: {}", e);
                return Err(ErrorUnauthorized(format!("無效的 JWT: {}", e)));
            }
        }
    }

    // 如果沒有 JWT，返回錯誤（不再允許從 URL 參數獲取）
    Err(ErrorUnauthorized("需要 JWT 認證"))
}

/// 從請求擴展中獲取 user_id（由中間件設置）
pub fn get_user_id_from_extensions(req: &ServiceRequest) -> Option<String> {
    req.extensions().get::<String>().cloned()
}

// JWT 認證中間件
pub struct JwtAuth;

impl<S, B> Transform<S, ServiceRequest> for JwtAuth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = JwtAuthMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(JwtAuthMiddleware { service }))
    }
}

pub struct JwtAuthMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for JwtAuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // 提取並驗證 JWT token
        let token_result = extract_token_from_header(&req);

        match token_result {
            Ok(token) => {
                match verify_jwt(&token) {
                    Ok(claims) => {
                        // 將 user_id 存入請求擴展
                        req.extensions_mut().insert(claims.sub.clone());
                        req.extensions_mut().insert(claims.clone());

                        let fut = self.service.call(req);
                        Box::pin(async move {
                            let res = fut.await?;
                            Ok(res.map_into_left_body())
                        })
                    }
                    Err(e) => {
                        log::warn!("JWT 驗證失敗: {}", e);

                        // 獲取請求的 Origin 頭部
                        let origin = req.headers()
                            .get("origin")
                            .and_then(|v| v.to_str().ok())
                            .map(|s| s.to_string());

                        Box::pin(async move {
                            let mut response = HttpResponse::Unauthorized()
                                .content_type("application/json")
                                .json(serde_json::json!({
                                    "success": false,
                                    "data": serde_json::Value::Null,
                                    "message": format!("無效的 JWT: {}", e)
                                }));

                            // 添加 CORS 頭部
                            if let Some(origin_value) = origin {
                                response.headers_mut().insert(
                                    actix_web::http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
                                    actix_web::http::header::HeaderValue::from_str(&origin_value).unwrap()
                                );
                                response.headers_mut().insert(
                                    actix_web::http::header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
                                    actix_web::http::header::HeaderValue::from_static("true")
                                );
                            }

                            Ok(req.into_response(response).map_into_boxed_body().map_into_right_body())
                        })
                    }
                }
            }
            Err(e) => {
                // 獲取請求的 Origin 頭部
                let origin = req.headers()
                    .get("origin")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());

                let error_msg = e.to_string();

                Box::pin(async move {
                    let mut response = HttpResponse::Unauthorized()
                        .content_type("application/json")
                        .json(serde_json::json!({
                            "success": false,
                            "data": serde_json::Value::Null,
                            "message": error_msg
                        }));

                    // 添加 CORS 頭部
                    if let Some(origin_value) = origin {
                        response.headers_mut().insert(
                            actix_web::http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
                            actix_web::http::header::HeaderValue::from_str(&origin_value).unwrap()
                        );
                        response.headers_mut().insert(
                            actix_web::http::header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
                            actix_web::http::header::HeaderValue::from_static("true")
                        );
                    }

                    Ok(req.into_response(response).map_into_boxed_body().map_into_right_body())
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_verify_jwt() {
        let user_id = "test-user-123";
        let email = "test@example.com";

        // 生成 token
        let token = generate_jwt(user_id, email).unwrap();
        assert!(!token.is_empty());

        // 驗證 token
        let claims = verify_jwt(&token).unwrap();
        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.email, email);
    }

    #[test]
    fn test_invalid_token() {
        let result = verify_jwt("invalid.token.here");
        assert!(result.is_err());
    }
}
