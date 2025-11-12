use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};

/// 自定義密碼驗證函數
/// 要求：至少4個字符
fn validate_password_strength(password: &str) -> Result<(), ValidationError> {
    if password.len() < 4 {
        return Err(ValidationError::new("password_too_short"));
    }
    Ok(())
}

/// 自定義任務標題驗證函數
fn validate_task_title(title: &str) -> Result<(), ValidationError> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return Err(ValidationError::new("title_empty"));
    }
    if trimmed.len() < 2 {
        return Err(ValidationError::new("title_too_short"));
    }
    Ok(())
}

/// 用戶註冊請求驗證
#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct RegisterRequest {
    #[validate(length(min = 2, max = 50, message = "用戶名長度必須在2-50字符之間"))]
    pub name: String,

    #[validate(email(message = "請提供有效的電子郵件地址"))]
    pub email: String,

    #[validate(custom(function = "validate_password_strength"))]
    pub password: String,
}

/// 用戶登入請求驗證
#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct LoginRequest {
    #[validate(email(message = "請提供有效的電子郵件地址"))]
    pub email: String,

    #[validate(length(min = 1, message = "密碼不能為空"))]
    pub password: String,
}

/// 任務創建請求驗證
#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct CreateTaskRequest {
    #[validate(custom(function = "validate_task_title"))]
    pub title: String,

    #[validate(length(max = 5000, message = "任務描述不能超過5000字符"))]
    pub description: Option<String>,

    #[validate(range(min = 1, max = 5, message = "優先級必須在1-5之間"))]
    pub priority: Option<i32>,

    #[validate(length(max = 50, message = "任務類型不能超過50字符"))]
    pub task_type: Option<String>,

    #[validate(range(min = 1, max = 10, message = "難度必須在1-10之間"))]
    pub difficulty: Option<i32>,

    #[validate(range(min = 0, max = 10000, message = "經驗值必須在0-10000之間"))]
    pub experience: Option<i32>,

    pub user_id: Option<String>,
    pub parent_task_id: Option<String>,
    pub task_order: Option<i32>,
    pub due_date: Option<String>,

    // 常駐目標相關參數
    pub is_recurring: Option<i32>,

    #[validate(length(max = 50, message = "重複模式不能超過50字符"))]
    pub recurrence_pattern: Option<String>,

    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub completion_target: Option<i32>,
}

/// 任務更新請求驗證
#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct UpdateTaskRequest {
    #[validate(custom(function = "validate_task_title"))]
    pub title: Option<String>,

    #[validate(length(max = 5000, message = "任務描述不能超過5000字符"))]
    pub description: Option<String>,

    #[validate(range(min = 0, max = 7, message = "狀態必須在0-7之間"))]
    pub status: Option<i32>,

    #[validate(range(min = 1, max = 5, message = "優先級必須在1-5之間"))]
    pub priority: Option<i32>,

    #[validate(length(max = 50, message = "任務類型不能超過50字符"))]
    pub task_type: Option<String>,

    #[validate(range(min = 1, max = 10, message = "難度必須在1-10之間"))]
    pub difficulty: Option<i32>,

    #[validate(range(min = 0, max = 10000, message = "經驗值必須在0-10000之間"))]
    pub experience: Option<i32>,

    pub due_date: Option<String>,
    pub task_order: Option<i32>,
    pub skill_tags: Option<Vec<String>>,
}

/// 技能創建請求驗證
#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct CreateSkillRequest {
    #[validate(length(min = 2, max = 100, message = "技能名稱長度必須在2-100字符之間"))]
    pub name: String,

    #[validate(length(max = 500, message = "技能描述不能超過500字符"))]
    pub description: Option<String>,

    #[validate(range(min = 1, max = 100, message = "等級必須在1-100之間"))]
    pub level: Option<i32>,

    pub user_id: Option<String>,
}

/// 聊天消息請求驗證
#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct ChatMessageRequest {
    #[validate(length(min = 1, max = 5000, message = "消息長度必須在1-5000字符之間"))]
    pub message: String,

    pub user_id: Option<String>,
}

/// 經驗值更新請求驗證
#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct ExperienceUpdateRequest {
    #[validate(range(min = -1000, max = 10000, message = "經驗值增量必須在-1000到10000之間"))]
    pub experience_gain: i32,

    #[validate(length(max = 200, message = "原因不能超過200字符"))]
    pub reason: Option<String>,
}

/// 輔助函數：將 validator 錯誤轉換為易讀的字符串
pub fn validation_errors_to_string(errors: &validator::ValidationErrors) -> String {
    let mut messages = Vec::new();

    for (field, field_errors) in errors.field_errors() {
        for error in field_errors {
            let msg = if let Some(message) = &error.message {
                message.to_string()
            } else {
                format!("欄位 {} 驗證失敗: {:?}", field, error.code)
            };
            messages.push(msg);
        }
    }

    messages.join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_validation() {
        // 有效密碼
        assert!(validate_password_strength("1234").is_ok());
        assert!(validate_password_strength("password").is_ok());
        assert!(validate_password_strength("Password123").is_ok());

        // 太短（少於4個字符）
        assert!(validate_password_strength("123").is_err());
        assert!(validate_password_strength("ab").is_err());
    }

    #[test]
    fn test_register_request_validation() {
        let valid_request = RegisterRequest {
            name: "John Doe".to_string(),
            email: "john@example.com".to_string(),
            password: "1234".to_string(),
        };
        assert!(valid_request.validate().is_ok());

        let invalid_email = RegisterRequest {
            name: "John Doe".to_string(),
            email: "invalid-email".to_string(),
            password: "1234".to_string(),
        };
        assert!(invalid_email.validate().is_err());
    }

    #[test]
    fn test_task_title_validation() {
        assert!(validate_task_title("Valid Task").is_ok());
        assert!(validate_task_title("  ").is_err());
        assert!(validate_task_title("A").is_err());
    }
}
