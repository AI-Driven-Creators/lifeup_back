// AI 服務模組定義

// 子模組聲明
mod r#trait;  // 使用 r# 前綴因為 trait 是保留字
mod common;
mod openai;
mod openrouter;

// 重新導出公開的 API
pub use r#trait::AIService;
pub use common::{
    AIGeneratedAchievement, AIGeneratedTask, AIGeneratedTaskPlan, AIGeneratedSkillTags,
    ExpertMatch, Expert, ModelTier,
    get_expert_database, convert_to_achievement_model, convert_to_task_model,
    build_task_generation_prompt
};
pub use openai::OpenAIService;
pub use openrouter::OpenRouterService;

// 工廠函數
use anyhow::Result;
use crate::config::AIConfig;

// AI 服務工廠函數
pub fn create_ai_service(config: &AIConfig) -> Result<Box<dyn AIService + Send + Sync>> {
    match config.api_option.as_str() {
        "OpenAI" => {
            let api_key = config.openai_api_key.as_ref()
                .ok_or_else(|| anyhow::anyhow!("OpenAI API key 未設定"))?;
            Ok(Box::new(OpenAIService::new(
                api_key.clone(),
                config.openai_model.clone(),
                config.model_small.clone(),
                config.model_fast.clone(),
                config.model_normal.clone(),
                config.model_think.clone(),
                config.model_background.clone(),
            )))
        }
        "OpenRouter" => {
            let api_key = config.openrouter_api_key.as_ref()
                .ok_or_else(|| anyhow::anyhow!("OpenRouter API key 未設定"))?;
            Ok(Box::new(OpenRouterService::new(
                api_key.clone(),
                config.openrouter_model.clone(),
                config.model_small.clone(),
                config.model_fast.clone(),
                config.model_normal.clone(),
                config.model_think.clone(),
                config.model_background.clone(),
            )))
        }
        _ => Err(anyhow::anyhow!("不支援的 AI 服務選項: {}", config.api_option))
    }
}