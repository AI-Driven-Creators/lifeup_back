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
    pub allowed_origins: Vec<String>,
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

    // 多步驟任務生成模型配置
    pub outline_model: String,        // 大綱生成模型（輕量快速）
    pub detail_model: String,         // 細節擴展模型（推理能力強）
    pub resource_model: String,       // 資源推薦模型（帶搜尋能力）

    // 模型等級配置 (Small/Fast/Normal/Think/Background)
    pub model_small: String,          // 超輕量模型（簡單文字處理、格式轉換）
    pub model_fast: String,           // 快速回應模型（簡單對話、快速回覆）
    pub model_normal: String,         // 標準推理模型（任務生成、成就生成）
    pub model_think: String,          // 深度推理模型（複雜規劃、專家分析）
    pub model_background: String,     // 背景處理模型（大量數據分析、批次處理）

    // Token 预算控制
    pub max_prompt_tokens: usize,
    pub max_completion_tokens: i32,

    // 数据采样配置
    pub recent_tasks_sample_size: usize,
    pub recent_cancellations_sample_size: usize,
    pub top_categories_limit: usize,

    // 時間窗口配置
    pub analysis_window_days: i64,
    pub recent_activity_days: i64,

    // 特征开关
    pub enable_milestone_detection: bool,
    pub enable_streak_analysis: bool,
}

impl Config {
    pub fn from_env() -> Self {
        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://lifeup.db".to_string());
        let server_host = env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let server_port = env::var("SERVER_PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse()
            .unwrap_or(8080);

        // CORS 配置 - 讀取允許的來源列表
        let allowed_origins = env::var("ALLOWED_ORIGINS")
            .unwrap_or_else(|_| "http://localhost:5173".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<String>>();

        let environment = env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string());
        let log_level = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());

        // AI 配置
        let raw_api_option = env::var("API_OPTION").unwrap_or_else(|_| "OpenRouter".to_string());
        let api_option_normalized = raw_api_option.trim().to_lowercase();
        let api_option = match api_option_normalized.as_str() {
            "openai" => "OpenAI".to_string(),
            "openrouter" => "OpenRouter".to_string(),
            other => {
                log::warn!(
                    "未識別的 API_OPTION 值: '{}', 將維持原值。可用選項: OpenAI, OpenRouter",
                    other
                );
                raw_api_option.trim().to_string()
            }
        };
        let openai_api_key = env::var("OPENAI_API_KEY").ok();
        let openai_model = env::var("OPENAI_MODEL")
            .unwrap_or_else(|_| "gpt-4o-mini".to_string());
        let openrouter_api_key = env::var("OPENROUTER_API_KEY").ok();
        let openrouter_model = env::var("OPENROUTER_MODEL")
            .unwrap_or_else(|_| "openrouter.ai/google/gemma-3n-e4b-it".to_string());

        // 多步驟任務生成模型配置
        let outline_model = env::var("OUTLINE_MODEL")
            .unwrap_or_else(|_| "openai/gpt-4o-mini".to_string());
        let detail_model = env::var("DETAIL_MODEL")
            .unwrap_or_else(|_| "openai/gpt-4o".to_string());
        let resource_model = env::var("RESOURCE_MODEL")
            .unwrap_or_else(|_| "perplexity/sonar".to_string());

        // 模型等級配置 (Small/Fast/Normal/Think/Background)
        // 若未設定，依序降級: 新模型 -> 舊模型 -> 預設值
        let model_small = env::var("AI_MODEL_SMALL")
            .unwrap_or_else(|_| {
                env::var("OPENROUTER_MODEL")
                    .or_else(|_| env::var("OPENAI_MODEL"))
                    .unwrap_or_else(|_| "google/gemma-3n-e4b-it".to_string())
            });
        let model_fast = env::var("AI_MODEL_FAST")
            .unwrap_or_else(|_| {
                env::var("OPENROUTER_MODEL")
                    .or_else(|_| env::var("OPENAI_MODEL"))
                    .unwrap_or_else(|_| "qwen/qwen3-8b".to_string())
            });
        let model_normal = env::var("AI_MODEL_NORMAL")
            .unwrap_or_else(|_| {
                env::var("OPENROUTER_MODEL")
                    .or_else(|_| env::var("OPENAI_MODEL"))
                    .unwrap_or_else(|_| "google/gemma-3n-e4b-it".to_string())
            });
        let model_think = env::var("AI_MODEL_THINK")
            .unwrap_or_else(|_| {
                env::var("OPENAI_MODEL")
                    .or_else(|_| env::var("OPENROUTER_MODEL"))
                    .unwrap_or_else(|_| "openai/gpt-oss-120b".to_string())
            });
        let model_background = env::var("AI_MODEL_BACKGROUND")
            .unwrap_or_else(|_| {
                env::var("OPENAI_MODEL")
                    .or_else(|_| env::var("OPENROUTER_MODEL"))
                    .unwrap_or_else(|_| "google/gemma-3n-e4b-it".to_string())
            });

        // Token 预算控制
        let max_prompt_tokens = env::var("AI_MAX_PROMPT_TOKENS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2000);
        let max_completion_tokens = env::var("AI_MAX_COMPLETION_TOKENS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(4000);

        // 数据采样配置
        let recent_tasks_sample_size = env::var("AI_RECENT_TASKS_SAMPLE_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(20);
        let recent_cancellations_sample_size = env::var("AI_RECENT_CANCELLATIONS_SAMPLE_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);
        let top_categories_limit = env::var("AI_TOP_CATEGORIES_LIMIT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);

        // 時間窗口配置
        let analysis_window_days = env::var("AI_ANALYSIS_WINDOW_DAYS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(90);
        let recent_activity_days = env::var("AI_RECENT_ACTIVITY_DAYS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);

        // 特征开关
        let enable_milestone_detection = env::var("AI_ENABLE_MILESTONE_DETECTION")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(true);
        let enable_streak_analysis = env::var("AI_ENABLE_STREAK_ANALYSIS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(true);

        // 調試日誌 - 注意：此時日誌系統可能還未初始化
        // 這些日誌會在 main.rs 中重新顯示

        Config {
            database: DatabaseConfig { url: database_url },
            server: ServerConfig {
                host: server_host,
                port: server_port,
                allowed_origins,
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
                    outline_model,
                    detail_model,
                    resource_model,
                    model_small,
                    model_fast,
                    model_normal,
                    model_think,
                    model_background,
                    max_prompt_tokens,
                    max_completion_tokens,
                    recent_tasks_sample_size,
                    recent_cancellations_sample_size,
                    top_categories_limit,
                    analysis_window_days,
                    recent_activity_days,
                    enable_milestone_detection,
                    enable_streak_analysis,
                },
            },
        }
    }

    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }
} 