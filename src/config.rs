use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub database: DatabaseConfig,
    pub server: ServerConfig,
    pub app: AppConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub environment: String,
    pub log_level: String,
    pub ai: AIConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AIConfig {
    pub api_option: String,
    pub openai_api_key: Option<String>,
    pub openai_model: String,
    pub openrouter_api_key: Option<String>,
    pub openrouter_model: String,
}

impl Config {
    pub fn from_env() -> Self {
        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://lifeup.db".to_string());
        let server_host = env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let server_port = env::var("SERVER_PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse()
            .unwrap_or(8080);
        let environment = env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string());
        let log_level = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());

        // AI 配置
        let api_option = env::var("API_OPTION").unwrap_or_else(|_| "OpenRouter".to_string());
        let openai_api_key = env::var("OPENAI_API_KEY").ok();
        let openai_model = env::var("OPENAI_MODEL")
            .unwrap_or_else(|_| "gpt-4o-mini".to_string());
        let openrouter_api_key = env::var("OPENROUTER_API_KEY").ok();
        let openrouter_model = env::var("OPENROUTER_MODEL")
            .unwrap_or_else(|_| "openrouter.ai/google/gemma-3n-e4b-it".to_string());

        // 調試日誌 - 注意：此時日誌系統可能還未初始化
        // 這些日誌會在 main.rs 中重新顯示

        Config {
            database: DatabaseConfig { url: database_url },
            server: ServerConfig {
                host: server_host,
                port: server_port,
            },
            app: AppConfig {
                environment,
                log_level,
                ai: AIConfig {
                    api_option,
                    openai_api_key,
                    openai_model,
                    openrouter_api_key,
                    openrouter_model,
                },
            },
        }
    }

    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }
} 