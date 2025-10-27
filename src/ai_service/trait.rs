use anyhow::Result;
use rbatis::RBatis;
use super::common::{AIGeneratedAchievement, AIGeneratedTask, AIGeneratedTaskPlan, ExpertMatch};

// AI 服務 trait
#[async_trait::async_trait]
pub trait AIService {
    async fn generate_achievement_from_text(&self, user_input: &str) -> Result<AIGeneratedAchievement>;
    async fn generate_achievement_from_user_id(&self, rb: &RBatis, user_id: &str) -> Result<AIGeneratedAchievement>;
    async fn generate_task_preview(&self, prompt: &str) -> Result<String>;
    async fn generate_task_preview_with_history(&self, system_prompt: &str, history: &[(String, String)], current_message: &str) -> Result<String>;
    async fn generate_task_from_text(&self, user_input: &str) -> Result<AIGeneratedTask>;
    async fn match_expert_for_task(&self, user_input: &str) -> Result<ExpertMatch>;
    async fn generate_task_with_expert(&self, user_input: &str, expert_match: &ExpertMatch) -> Result<AIGeneratedTaskPlan>;
    async fn analyze_with_expert(&self, user_input: &str, expert_name: &str, expert_description: &str, analysis_type: &str) -> Result<String>;

    // 新增：專門生成子任務
    async fn generate_subtasks_for_main_task(&self, main_task_title: &str, main_task_description: &str, expert_match: &ExpertMatch) -> Result<Vec<AIGeneratedTask>>;

    // 新增：使用指定模型生成文字回應
    async fn generate_with_model(&self, model: &str, prompt: &str) -> Result<String>;

    // 新增：專門生成每日任務（使用針對每日任務優化的提示詞）
    async fn generate_daily_task_from_text(&self, user_input: &str) -> Result<AIGeneratedTask>;

    // 新增：用戶意圖分類（判斷是詳細任務還是模糊目標）
    async fn classify_user_intent(&self, user_input: &str) -> Result<crate::ai_tasks::ClassifyIntentResponse>;
}