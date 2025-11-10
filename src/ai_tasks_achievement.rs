// AI 自動成就生成 - 根據任務生成對應成就

use rbatis::RBatis;
use crate::models::{Task, Achievement};
use crate::ai_service::convert_to_achievement_model;

/// 根據任務內容生成對應的成就
/// 此函數會分析任務的標題、描述、類型等信息，使用 AI 生成一個與任務完成相關的成就
pub async fn generate_achievement_for_task(
    rb: &RBatis,
    task: &Task,
) -> Result<Option<Achievement>, anyhow::Error> {
    let task_title = task.title.as_deref().unwrap_or("未命名任務");
    let task_desc = task.description.as_deref().unwrap_or("");
    let task_type = task.task_type.as_deref().unwrap_or("daily");
    let difficulty = task.difficulty.unwrap_or(1);

    log::info!("為任務「{}」生成對應成就", task_title);

    // 構建 AI 提示詞
    let ai_prompt = format!(
        r#"請根據以下任務信息，生成一個對應的成就目標。

**任務信息**：
- 標題：{}
- 描述：{}
- 類型：{}
- 難度：{}

**生成要求**：
1. 成就名稱要簡潔有力，突出任務的核心目標
2. 成就描述要激勵用戶完成這個任務
3. 成就圖標選擇與任務主題相關的 emoji
4. 類別必須從以下選擇：task_mastery（任務精通）、consistency（堅持不懈）、challenge_overcome（克服挑戰）、skill_development（技能發展）
5. 需求類型必須是 "task_complete"
6. 需求值設為 1（完成一個任務）
7. 經驗獎勵根據任務難度設置：難度1給50經驗，難度2給100經驗，難度3給150經驗，難度4給200經驗，難度5給250經驗

請用 JSON 格式回覆，包含以下欄位：
{{
  "name": "成就名稱",
  "description": "成就描述",
  "icon": "emoji圖標",
  "category": "從上述四個類別中選一個",
  "requirement_type": "task_complete",
  "requirement_value": 1,
  "experience_reward": 依難度計算的經驗值
}}

範例：
任務：「每天閱讀30分鐘」
成就：{{
  "name": "閱讀習慣養成者",
  "description": "堅持每天閱讀30分鐘，培養良好的閱讀習慣",
  "icon": "📚",
  "category": "consistency",
  "requirement_type": "task_complete",
  "requirement_value": 1,
  "experience_reward": 100
}}
"#,
        task_title,
        task_desc,
        task_type,
        difficulty
    );

    // 調用 AI 生成
    let config = crate::config::Config::from_env();
    let ai_service = match crate::ai_service::create_ai_service(&config.app.ai) {
        Ok(service) => service,
        Err(e) => {
            log::error!("AI 服務初始化失敗: {}", e);
            return Ok(None);
        }
    };

    log::debug!("AI 提示詞長度: {} 字符", ai_prompt.len());

    match ai_service.generate_achievement_from_text(&ai_prompt).await {
        Ok(ai_achievement) => {
            log::info!("✨ 為任務「{}」生成成就：「{}」", task_title, ai_achievement.name);

            // 轉換為數據庫模型
            let mut achievement_model = convert_to_achievement_model(ai_achievement);

            // 設置 related_task_id，標記這個成就與特定任務相關
            achievement_model.related_task_id = task.id.clone();

            // 保存到數據庫
            match Achievement::insert(rb, &achievement_model).await {
                Ok(_) => {
                    log::info!("🎉 成就「{}」已保存", achievement_model.name.as_deref().unwrap_or("未知"));
                    Ok(Some(achievement_model))
                }
                Err(e) => {
                    log::error!("保存成就失敗: {}", e);
                    Err(e.into())
                }
            }
        }
        Err(e) => {
            log::warn!("AI 生成成就失敗: {}", e);
            Ok(None)
        }
    }
}

/// 異步生成任務對應的成就（不阻塞主流程）
pub fn spawn_generate_achievement_for_task(rb: RBatis, task: Task) {
    tokio::spawn(async move {
        if let Err(e) = generate_achievement_for_task(&rb, &task).await {
            log::error!("異步生成成就失敗: {}", e);
        }
    });
}
