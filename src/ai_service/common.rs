use anyhow::Result;
use serde::{Deserialize, Serialize};
use chrono::Utc;
use crate::models::AchievementRequirementType;
use crate::behavior_analytics::UserBehaviorSummary;
use crate::ai_tasks::AnalysisDirection;
use std::collections::HashMap;

// 模型等級枚舉
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelTier {
    Small,      // 超輕量（簡單文字處理、格式轉換、基礎驗證）
    Fast,       // 快速回應（簡單對話、快速回覆、任務預覽）
    Normal,     // 標準推理（任務生成、成就生成、專家匹配）
    Think,      // 深度推理（複雜規劃、專家分析、子任務生成）
    Background, // 背景處理（大量數據分析、批次處理、深度研究）
}

// 格式化 AI 輸出為單行日誌
pub fn format_ai_output(text: &str) -> String {
    text.replace("\\n", " ")
        .replace("\\\"", "\"")
        .chars()
        .filter(|c| !c.is_control() || *c == ' ')
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

// 專家信息結構
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Expert {
    pub name: String,
    pub description: String,
    pub expertise_areas: Vec<String>,
    pub emoji: String,
}

// 專家匹配結果
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExpertMatch {
    pub expert: Expert,
    pub ai_expert_name: String,
    pub ai_expert_description: String,
}

// AI 生成的任務結構（簡化版）
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AIGeneratedTask {
    pub title: Option<String>,
    pub description: Option<String>,
    pub task_type: Option<String>,
    pub priority: Option<i32>,
    pub difficulty: Option<i32>,
    pub experience: Option<i32>,
    pub due_date: Option<String>,
    pub is_recurring: Option<bool>,
    pub recurrence_pattern: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub completion_target: Option<f64>,
}

// AI 生成的任務計劃（包含主任務和子任務）
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AIGeneratedTaskPlan {
    pub main_task: AIGeneratedTask,
    pub subtasks: Vec<AIGeneratedTask>,
}

// AI 生成的成就結構
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AIGeneratedAchievement {
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub category: String,
    pub requirement_type: String,
    pub requirement_value: i32,
    pub experience_reward: i32,
}

// AI 生成的技能標籤結構
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AIGeneratedSkillTags {
    pub skills: Vec<String>,           // AI 生成的技能名稱列表
    pub reasoning: Option<String>,     // AI 的選擇理由（可選）
}

// 內部使用的輔助結構
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AITaskPrimaryFields {
    pub title: Option<String>,
    pub description: Option<String>,
    pub task_type: Option<String>,
    pub due_date: Option<String>,
    pub recurrence_pattern: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AITaskSecondaryFields {
    pub priority: Option<i32>,
    pub difficulty: Option<i32>,
    pub experience: Option<i32>,
    pub is_recurring: Option<bool>,
    pub completion_target: Option<f64>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AIPlanPrimaryFields {
    pub main_task: AITaskPrimaryFields,
    #[serde(default)]
    pub subtasks: Vec<AITaskPrimaryFields>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AIPlanSecondaryFields {
    pub main_task: AITaskSecondaryFields,
    #[serde(default)]
    pub subtasks: Vec<AITaskSecondaryFields>,
}

// 輔助函數：應用默認值到 AIGeneratedTask
impl AIGeneratedTask {
    pub fn with_defaults(self) -> Self {
        Self {
            title: self.title.or(Some("未命名任務".to_string())),
            description: self.description,
            task_type: self.task_type.or(Some("side".to_string())),
            priority: self.priority.or(Some(1)),
            difficulty: self.difficulty.or(Some(2)),
            experience: self.experience.or(Some(30)),
            due_date: self.due_date,
            is_recurring: self.is_recurring.or(Some(false)),
            recurrence_pattern: self.recurrence_pattern,
            start_date: self.start_date,
            end_date: self.end_date,
            completion_target: self.completion_target,
        }
    }

    pub fn normalize_recurring(mut self) -> Self {
        // 如果有重複模式，確保 is_recurring 為 true
        if self.recurrence_pattern.is_some() {
            self.is_recurring = Some(true);
            if self.completion_target.is_none() {
                self.completion_target = Some(0.8);
            }
            // 重複性任務不應該有截止日期
            if self.due_date.is_some() {
                self.due_date = None;
            }
        }
        self
    }
}

// 專家數據庫
pub fn get_expert_database() -> Vec<Expert> {
    vec![
        Expert {
            name: "資深英文教學老師".to_string(),
            description: "擁有15年英語教學經驗，專精於語言學習方法和技巧".to_string(),
            expertise_areas: vec!["英語學習".to_string(), "語言教學".to_string(), "口語練習".to_string(), "文法學習".to_string()],
            emoji: "📚".to_string(),
        },
        Expert {
            name: "程式設計導師".to_string(),
            description: "資深軟體工程師，專精於多種程式語言和開發框架".to_string(),
            expertise_areas: vec!["程式設計".to_string(), "軟體開發".to_string(), "演算法".to_string(), "系統設計".to_string()],
            emoji: "💻".to_string(),
        },
        Expert {
            name: "健身教練".to_string(),
            description: "專業健身教練，專精於運動訓練和健康管理".to_string(),
            expertise_areas: vec!["健身訓練".to_string(), "運動計劃".to_string(), "健康管理".to_string(), "營養搭配".to_string()],
            emoji: "💪".to_string(),
        },
        Expert {
            name: "理財規劃師".to_string(),
            description: "專業理財顧問，專精於投資理財和財務規劃".to_string(),
            expertise_areas: vec!["理財規劃".to_string(), "投資策略".to_string(), "財務管理".to_string(), "儲蓄計劃".to_string()],
            emoji: "💰".to_string(),
        },
        Expert {
            name: "時間管理顧問".to_string(),
            description: "專業時間管理顧問，專精於效率提升和目標達成".to_string(),
            expertise_areas: vec!["時間管理".to_string(), "效率提升".to_string(), "目標設定".to_string(), "習慣養成".to_string()],
            emoji: "⏰".to_string(),
        },
        Expert {
            name: "創意設計師".to_string(),
            description: "資深設計師，專精於創意思維和視覺設計".to_string(),
            expertise_areas: vec!["創意設計".to_string(), "視覺設計".to_string(), "品牌設計".to_string(), "UI/UX設計".to_string()],
            emoji: "🎨".to_string(),
        },
        Expert {
            name: "心理諮商師".to_string(),
            description: "專業心理諮商師，專精於情緒管理和心理調適".to_string(),
            expertise_areas: vec!["心理諮商".to_string(), "情緒管理".to_string(), "壓力調適".to_string(), "人際關係".to_string()],
            emoji: "🧠".to_string(),
        },
        Expert {
            name: "廚藝導師".to_string(),
            description: "專業廚師，專精於各種料理技巧和營養搭配".to_string(),
            expertise_areas: vec!["烹飪技巧".to_string(), "料理製作".to_string(), "營養搭配".to_string(), "食材選擇".to_string()],
            emoji: "👨‍🍳".to_string(),
        },
        Expert {
            name: "音樂老師".to_string(),
            description: "專業音樂教師，專精於樂器演奏和音樂理論".to_string(),
            expertise_areas: vec!["音樂學習".to_string(), "樂器演奏".to_string(), "音樂理論".to_string(), "聲樂訓練".to_string()],
            emoji: "🎵".to_string(),
        },
        Expert {
            name: "學習方法顧問".to_string(),
            description: "教育心理學專家，專精於學習方法和記憶技巧".to_string(),
            expertise_areas: vec!["學習方法".to_string(), "記憶技巧".to_string(), "考試準備".to_string(), "知識管理".to_string()],
            emoji: "📖".to_string(),
        },
    ]
}

// 根据用户行为摘要构建成就生成的 prompt
pub fn build_achievement_prompt_from_summary(summary: &UserBehaviorSummary) -> String {
    // 格式化分类统计
    let top_categories: Vec<String> = summary.top_categories
        .iter()
        .map(|c| format!(
            "{}（完成{}次，完成率{:.0}%，平均难度{:.1}）",
            c.category,
            c.completed_count,
            c.completion_rate * 100.0,
            c.avg_difficulty
        ))
        .collect();

    // 格式化最近完成的任务
    let recent_tasks: Vec<String> = summary.recent_completions
        .iter()
        .take(10)
        .map(|t| format!("  - {}: {}", t.completion_date.split('T').next().unwrap_or(&t.completion_date), t.title))
        .collect();

    // 格式化最近取消的任务
    let recent_cancellations: Vec<String> = summary.recent_cancellations
        .iter()
        .take(5)
        .map(|t| format!("  - {}: {}", t.completion_date.split('T').next().unwrap_or(&t.completion_date), t.title))
        .collect();

    // 格式化里程碑
    let milestones: Vec<String> = summary.milestone_events
        .iter()
        .map(|m| format!("  - {}: {}", m.event_type, m.description))
        .collect();

    format!(
        r#"你是一個成就設計助手。根據用戶的行為數據分析，生成個性化且具有激勵性的成就。

【用戶統計數據】
- 總完成任務：{total_completed} 次
- 總取消任務：{total_cancelled} 次
- 待處理任務：{total_pending} 個
- 最長連續記錄：{longest_streak} 天（{streak_task}）
- 當前連續：{current_streak} 天
- 近 30 天活躍：{active_30} 天
- 總經驗值：{total_exp}

【任務分類分布】（Top {cat_count}）
{categories}

【最近完成任務】（最近 {recent_count} 條樣本）
{recent_tasks}

【最近取消任務】（最近 {cancel_count} 條樣本）
{recent_cancellations}

【里程碑事件】
{milestones}

【已解鎖成就】
{achievements}

**設計原則：**
- 成就名稱要幽默且具體，如「成為英語字典」「跑火入魔」
- 基於用戶實際行為模式生成，不要憑空想像
- 考慮用戶的優勢領域（完成率高的分類）和潛力領域
- 避免與現有成就重複
- 如果有明顯的連續記錄，可以考慮相關的持續性成就

**成就分類：**
- task_mastery: 任務精通類
- consistency: 持續性類
- challenge_overcome: 克服挑戰類
- skill_development: 技能發展類

**達成條件類型：**
- consecutive_days: 連續天數
- total_completions: 總完成次數
- task_complete: 完成任務總數
- streak_recovery: 從失敗中恢復
- skill_level: 技能等級
- learning_task_complete: 學習任務完成
- intelligence_attribute: 智力屬性達成
- endurance_attribute: 毅力屬性達成
- creativity_attribute: 創造力屬性達成
- social_attribute: 社交力屬性達成
- focus_attribute: 專注力屬性達成
- adaptability_attribute: 適應力屬性達成

**經驗值獎勵計算：**
- 基於難度：簡單成就 50-100，中等 100-200，困難 200-500

請以 JSON 格式回應：
{{
  "name": "成就名稱（幽默且具體）",
  "description": "成就描述（選填）",
  "icon": "圖標名稱（選填）",
  "category": "成就分類",
  "requirement_type": "達成條件類型",
  "requirement_value": 數值,
  "experience_reward": 經驗值獎勵
}}"#,
        total_completed = summary.total_tasks_completed,
        total_cancelled = summary.total_tasks_cancelled,
        total_pending = summary.total_tasks_pending,
        longest_streak = summary.longest_streak.days,
        streak_task = summary.longest_streak.task_title,
        current_streak = summary.current_streak.days,
        active_30 = summary.active_days_last_30,
        total_exp = summary.total_experience,
        cat_count = summary.top_categories.len(),
        categories = if top_categories.is_empty() { "  （暫無數據）".to_string() } else { top_categories.join("\n") },
        recent_count = summary.recent_completions.len().min(10),
        recent_tasks = if recent_tasks.is_empty() { "  （暫無數據）".to_string() } else { recent_tasks.join("\n") },
        cancel_count = summary.recent_cancellations.len().min(5),
        recent_cancellations = if recent_cancellations.is_empty() { "  （暫無數據）".to_string() } else { recent_cancellations.join("\n") },
        milestones = if milestones.is_empty() { "  （暫無數據）".to_string() } else { milestones.join("\n") },
        achievements = if summary.unlocked_achievements.is_empty() { "（暫無）".to_string() } else { summary.unlocked_achievements.join("、") },
    )
}

// 驗證生成的任務
pub fn validate_generated_task(task: &AIGeneratedTask) -> Result<AIGeneratedTask> {
    let mut corrected_task = task.clone();

    // 修正任務標題
    if corrected_task.title.is_none() || corrected_task.title.as_ref().unwrap().is_empty() {
        log::warn!("任務標題為空，設為預設值");
        corrected_task.title = Some("未命名任務".to_string());
    }

    // 修正任務類型
    if let Some(task_type) = &corrected_task.task_type {
        if !["main", "side", "challenge", "daily"].contains(&task_type.as_str()) {
            log::warn!("無效的任務類型: {}，設為預設值 'side'", task_type);
            corrected_task.task_type = Some("side".to_string());
        }
    } else {
        log::warn!("任務類型為空，設為預設值 'side'");
        corrected_task.task_type = Some("side".to_string());
    }

    // 修正優先級（clamp 到 0-2）
    if let Some(priority) = corrected_task.priority {
        if priority < 0 || priority > 2 {
            let clamped = priority.clamp(0, 2);
            log::warn!("優先級 {} 超出範圍，調整為 {}", priority, clamped);
            corrected_task.priority = Some(clamped);
        }
    } else {
        corrected_task.priority = Some(1); // 預設中優先級
    }

    // 修正難度（clamp 到 1-5）
    if let Some(difficulty) = corrected_task.difficulty {
        if difficulty < 1 || difficulty > 5 {
            let clamped = difficulty.clamp(1, 5);
            log::warn!("難度 {} 超出範圍，調整為 {}", difficulty, clamped);
            corrected_task.difficulty = Some(clamped);
        }
    } else {
        corrected_task.difficulty = Some(2); // 預設簡單難度
    }

    // 修正經驗值（確保非負）
    if let Some(experience) = corrected_task.experience {
        if experience < 0 {
            log::warn!("經驗值 {} 為負數，調整為 0", experience);
            corrected_task.experience = Some(0);
        }
    } else {
        corrected_task.experience = Some(30); // 預設經驗值
    }

    // 修正重複性任務設置
    if corrected_task.is_recurring.unwrap_or(false) {
        // 檢查重複模式
        if let Some(pattern) = &corrected_task.recurrence_pattern {
            if !["daily", "weekdays", "weekends", "weekly"].contains(&pattern.as_str()) {
                log::warn!("無效的重複模式: {}，設為預設值 'daily'", pattern);
                corrected_task.recurrence_pattern = Some("daily".to_string());
            }
        } else {
            log::warn!("重複性任務缺少重複模式，設為預設值 'daily'");
            corrected_task.recurrence_pattern = Some("daily".to_string());
        }

        // 檢查開始日期
        if corrected_task.start_date.is_none() {
            let now = Utc::now();
            let start_date_str = now.format("%Y-%m-%dT%H:%M:%S").to_string();
            log::warn!("重複性任務缺少開始日期，設為當前時間: {}", start_date_str);
            corrected_task.start_date = Some(start_date_str);
        }

        // 確保重複性任務沒有截止日期
        if corrected_task.due_date.is_some() {
            log::warn!("重複性任務不應有截止日期，移除 due_date");
            corrected_task.due_date = None;
        }

        // 確保有完成率目標
        if corrected_task.completion_target.is_none() {
            log::warn!("重複性任務缺少完成率目標，設為預設值 0.8");
            corrected_task.completion_target = Some(0.8);
        }
    }

    // 修正完成率目標（clamp 到 0.0-1.0）
    if let Some(target) = corrected_task.completion_target {
        if target < 0.0 || target > 1.0 {
            let clamped = target.clamp(0.0, 1.0);
            log::warn!("完成率目標 {} 超出範圍，調整為 {}", target, clamped);
            corrected_task.completion_target = Some(clamped);
        }
    }

    Ok(corrected_task)
}

// 驗證生成的成就
pub fn validate_generated_achievement(achievement: &AIGeneratedAchievement) -> Result<()> {
    // 驗證成就分類
    if !["task_mastery", "consistency", "challenge_overcome", "skill_development"].contains(&achievement.category.as_str()) {
        return Err(anyhow::anyhow!("無效的成就分類: {}", achievement.category));
    }

    // 驗證達成條件類型 - 使用枚舉的有效字符串列表
    let valid_requirement_types = AchievementRequirementType::all_valid_strings();
    if !valid_requirement_types.contains(&achievement.requirement_type.as_str()) {
        return Err(anyhow::anyhow!(
            "無效的達成條件類型: {}. 有效類型: {:?}",
            achievement.requirement_type,
            valid_requirement_types
        ));
    }

    // 驗證條件數值
    if achievement.requirement_value <= 0 {
        return Err(anyhow::anyhow!("達成條件數值必須大於0"));
    }

    // 驗證經驗值獎勵
    if achievement.experience_reward < 50 || achievement.experience_reward > 500 {
        return Err(anyhow::anyhow!("經驗值獎勵必須在 50-500 之間"));
    }

    // 驗證成就名稱長度
    if achievement.name.len() < 2 || achievement.name.len() > 50 {
        return Err(anyhow::anyhow!("成就名稱長度必須在 2-50 字之間"));
    }

    Ok(())
}

// 將 AI 生成的任務轉換為資料庫模型
pub fn convert_to_task_model(
    ai_task: AIGeneratedTask,
    user_id: String,
) -> crate::models::Task {
    use uuid::Uuid;

    let now = Utc::now();

    crate::models::Task {
        id: Some(Uuid::new_v4().to_string()),
        user_id: Some(user_id),
        title: ai_task.title,
        description: ai_task.description,
        status: Some(0), // 預設為待處理
        priority: ai_task.priority,
        task_type: ai_task.task_type,
        difficulty: ai_task.difficulty,
        experience: ai_task.experience,
        parent_task_id: None,
        is_parent_task: Some(0),
        task_order: Some(0),
        due_date: ai_task.due_date.and_then(|d| d.parse().ok()),
        created_at: Some(now),
        updated_at: Some(now),
        is_recurring: ai_task.is_recurring.map(|b| if b { 1 } else { 0 }),
        recurrence_pattern: ai_task.recurrence_pattern,
        start_date: ai_task.start_date.and_then(|d| d.parse().ok()),
        end_date: ai_task.end_date.and_then(|d| d.parse().ok()),
        completion_target: ai_task.completion_target,
        completion_rate: Some(0.0),
        task_date: None,
        cancel_count: Some(0),
        last_cancelled_at: None,
        skill_tags: None,
        career_mainline_id: None,
        task_category: None,
        attributes: None,
    }
}

// 將 AI 生成的成就轉換為資料庫模型
pub fn convert_to_achievement_model(
    ai_achievement: AIGeneratedAchievement,
) -> crate::models::Achievement {
    use uuid::Uuid;

    let now = Utc::now();

    // 將字符串轉換為枚舉
    let requirement_type = AchievementRequirementType::from_string(&ai_achievement.requirement_type);

    crate::models::Achievement {
        id: Some(Uuid::new_v4().to_string()),
        name: Some(ai_achievement.name),
        description: ai_achievement.description,
        icon: ai_achievement.icon,
        category: Some(ai_achievement.category),
        requirement_type,
        requirement_value: Some(ai_achievement.requirement_value),
        experience_reward: Some(ai_achievement.experience_reward),
        created_at: Some(now),
    }
}

pub fn build_task_generation_prompt(
    user_input: &str,
    expert_match: &ExpertMatch,
    selected_options: Option<Vec<String>>,
    selected_directions: Option<Vec<AnalysisDirection>>,
    expert_outputs: Option<HashMap<String, String>>,
    skill_label: &str,
    duration_label: &str,
) -> String {
    let mut prompt = String::new();
    prompt.push_str(user_input);

    if !skill_label.is_empty() || !duration_label.is_empty() {
        prompt.push_str("\n\n使用者背景：");
        if !skill_label.is_empty() {
            prompt.push_str(&format!("熟悉程度：{} ", skill_label));
        }
        if !duration_label.is_empty() {
            prompt.push_str(&format!("學習時長：{}", duration_label));
        }
    }

    if let Some(options) = selected_options {
        if !options.is_empty() {
            let option_labels = options.join("、");
            prompt.push_str(&format!("\n\n請特別針對以下需求提供任務輸出：{}", option_labels));
        }
    }

    if let Some(directions) = selected_directions {
        if !directions.is_empty() {
            prompt.push_str("\n\n使用者已選擇的重點強化方向：\n");
            for (index, direction) in directions.iter().enumerate() {
                prompt.push_str(&format!("{}. {} - {}\n", index + 1, direction.title, direction.description));
            }
        }
    }
    prompt.push_str(&format!(
        "\n\n請根據以上資訊，並以{} ({}) 的視角，產出符合要求的任務規劃。",
        expert_match.expert.name, expert_match.expert.description
    ));

    prompt
}