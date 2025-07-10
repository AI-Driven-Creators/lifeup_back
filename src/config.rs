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

        Config {
            database: DatabaseConfig { url: database_url },
            server: ServerConfig {
                host: server_host,
                port: server_port,
            },
            app: AppConfig {
                environment,
                log_level,
            },
        }
    }

    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }
} 