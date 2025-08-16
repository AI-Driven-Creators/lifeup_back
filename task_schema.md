# Task JSON Schema 定義文件

## 完整版 Task Schema

用於完整的任務資料結構描述，包含所有可能的欄位。

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "Task",
  "description": "LifeUp 任務資料結構",
  "type": "object",
  "properties": {
    "id": {
      "type": ["string", "null"],
      "description": "任務唯一識別碼（UUID）",
      "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"
    },
    "user_id": {
      "type": ["string", "null"],
      "description": "所屬使用者ID"
    },
    "title": {
      "type": ["string", "null"],
      "description": "任務標題",
      "maxLength": 200
    },
    "description": {
      "type": ["string", "null"],
      "description": "任務描述",
      "maxLength": 2000
    },
    "status": {
      "type": ["integer", "null"],
      "description": "任務狀態：0=待處理, 1=進行中, 2=已完成, 3=已取消, 4=已暫停, 5=每日任務進行中, 6=每日任務已完成, 7=每日任務未完成",
      "enum": [0, 1, 2, 3, 4, 5, 6, 7, null],
      "default": 0
    },
    "priority": {
      "type": ["integer", "null"],
      "description": "優先級：0=低, 1=中, 2=高",
      "enum": [0, 1, 2, null],
      "default": 1
    },
    "task_type": {
      "type": ["string", "null"],
      "description": "任務類型",
      "enum": ["main", "side", "challenge", "daily", null]
    },
    "difficulty": {
      "type": ["integer", "null"],
      "description": "難度等級（1-5）",
      "minimum": 1,
      "maximum": 5
    },
    "experience": {
      "type": ["integer", "null"],
      "description": "經驗值獎勵",
      "minimum": 0
    },
    "parent_task_id": {
      "type": ["string", "null"],
      "description": "父任務ID（用於子任務）"
    },
    "is_parent_task": {
      "type": ["integer", "null"],
      "description": "是否為大任務：0=否, 1=是",
      "enum": [0, 1, null]
    },
    "task_order": {
      "type": ["integer", "null"],
      "description": "任務排序順序",
      "minimum": 0
    },
    "due_date": {
      "type": ["string", "null"],
      "description": "截止日期（ISO 8601 格式）",
      "format": "date-time"
    },
    "created_at": {
      "type": ["string", "null"],
      "description": "建立時間（ISO 8601 格式）",
      "format": "date-time"
    },
    "updated_at": {
      "type": ["string", "null"],
      "description": "更新時間（ISO 8601 格式）",
      "format": "date-time"
    },
    "is_recurring": {
      "type": ["integer", "null"],
      "description": "是否為重複性任務：0=否, 1=是",
      "enum": [0, 1, null]
    },
    "recurrence_pattern": {
      "type": ["string", "null"],
      "description": "重複模式",
      "enum": ["daily", "weekdays", "weekends", "weekly", null]
    },
    "start_date": {
      "type": ["string", "null"],
      "description": "開始日期（ISO 8601 格式）",
      "format": "date-time"
    },
    "end_date": {
      "type": ["string", "null"],
      "description": "結束日期（ISO 8601 格式）",
      "format": "date-time"
    },
    "completion_target": {
      "type": ["number", "null"],
      "description": "完成率目標（0.0-1.0）",
      "minimum": 0.0,
      "maximum": 1.0
    },
    "completion_rate": {
      "type": ["number", "null"],
      "description": "當前完成率（0.0-1.0）",
      "minimum": 0.0,
      "maximum": 1.0
    },
    "task_date": {
      "type": ["string", "null"],
      "description": "任務日期（YYYY-MM-DD 格式，用於日常子任務）",
      "pattern": "^\\d{4}-\\d{2}-\\d{2}$"
    },
    "cancel_count": {
      "type": ["integer", "null"],
      "description": "取消次數",
      "minimum": 0
    },
    "last_cancelled_at": {
      "type": ["string", "null"],
      "description": "最後取消時間（ISO 8601 格式）",
      "format": "date-time"
    }
  },
  "required": [],
  "additionalProperties": false
}
```

## 簡化版 CreateTaskInput Schema

專門用於建立新任務的輸入資料結構，只包含必要欄位。

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "CreateTaskInput",
  "description": "建立新任務的輸入資料結構",
  "type": "object",
  "properties": {
    "title": {
      "type": "string",
      "description": "任務標題",
      "minLength": 1,
      "maxLength": 200
    },
    "description": {
      "type": "string",
      "description": "任務描述",
      "maxLength": 2000
    },
    "task_type": {
      "type": "string",
      "description": "任務類型",
      "enum": ["main", "side", "challenge", "daily"],
      "default": "main"
    },
    "priority": {
      "type": "integer",
      "description": "優先級：0=低, 1=中, 2=高",
      "enum": [0, 1, 2],
      "default": 1
    },
    "difficulty": {
      "type": "integer",
      "description": "難度等級（1-5）",
      "minimum": 1,
      "maximum": 5,
      "default": 3
    },
    "experience": {
      "type": "integer",
      "description": "經驗值獎勵",
      "minimum": 0,
      "default": 50
    },
    "due_date": {
      "type": "string",
      "description": "截止日期（ISO 8601 格式）",
      "format": "date-time"
    },
    "is_recurring": {
      "type": "boolean",
      "description": "是否為重複性任務",
      "default": false
    },
    "recurrence_pattern": {
      "type": "string",
      "description": "重複模式（當 is_recurring 為 true 時必填）",
      "enum": ["daily", "weekdays", "weekends", "weekly"]
    },
    "start_date": {
      "type": "string",
      "description": "開始日期（重複性任務必填）",
      "format": "date-time"
    },
    "end_date": {
      "type": "string",
      "description": "結束日期（重複性任務選填）",
      "format": "date-time"
    },
    "completion_target": {
      "type": "number",
      "description": "完成率目標（0.0-1.0，重複性任務用）",
      "minimum": 0.0,
      "maximum": 1.0,
      "default": 0.8
    }
  },
  "required": ["title"],
  "additionalProperties": false,
  "dependencies": {
    "recurrence_pattern": ["is_recurring"],
    "start_date": ["is_recurring"]
  }
}
```

## 範例資料

### 範例 1：一般主任務

```json
{
  "title": "學習 Rust 程式語言",
  "description": "深入學習 Rust 的所有權系統、生命週期、並發程式設計等進階概念",
  "task_type": "main",
  "priority": 2,
  "difficulty": 4,
  "experience": 100,
  "due_date": "2024-12-31T23:59:59Z"
}
```

### 範例 2：每日重複任務

```json
{
  "title": "每日運動30分鐘",
  "description": "進行有氧運動或重量訓練，保持身體健康",
  "task_type": "daily",
  "priority": 1,
  "difficulty": 2,
  "experience": 50,
  "is_recurring": true,
  "recurrence_pattern": "daily",
  "start_date": "2024-01-01T00:00:00Z",
  "end_date": "2024-12-31T23:59:59Z",
  "completion_target": 0.8
}
```

### 範例 3：挑戰任務

```json
{
  "title": "完成馬拉松比賽",
  "description": "參加並完成全程馬拉松（42.195公里）",
  "task_type": "challenge",
  "priority": 2,
  "difficulty": 5,
  "experience": 500,
  "due_date": "2024-10-15T08:00:00Z"
}
```

### 範例 4：工作日重複任務

```json
{
  "title": "撰寫工作日誌",
  "description": "記錄每日工作進度和學習心得",
  "task_type": "daily",
  "priority": 1,
  "difficulty": 1,
  "experience": 30,
  "is_recurring": true,
  "recurrence_pattern": "weekdays",
  "start_date": "2024-01-01T00:00:00Z",
  "completion_target": 0.9
}
```

### 範例 5：副任務

```json
{
  "title": "整理房間",
  "description": "清潔並整理臥室和書房",
  "task_type": "side",
  "priority": 0,
  "difficulty": 2,
  "experience": 40,
  "due_date": "2024-02-01T18:00:00Z"
}
```
curl -X POST http://127.0.0.1:8080/api/tasks/generate -H "Content-Type: application/json" -d  {\"description\":\"每週一三五做瑜伽練習\"}"
### 範例 6：包含子任務的大任務（完整版）

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440001",
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "開發個人網站",
  "description": "使用現代技術棧開發並部署個人作品集網站",
  "task_type": "main",
  "status": 1,
  "priority": 2,
  "difficulty": 4,
  "experience": 200,
  "is_parent_task": 1,
  "task_order": 1,
  "due_date": "2024-06-30T23:59:59Z",
  "created_at": "2024-01-15T10:00:00Z",
  "updated_at": "2024-01-20T14:30:00Z"
}
```

### 範例 7：子任務（屬於上述大任務）

```json
{
  "title": "設計網站 UI/UX",
  "description": "使用 Figma 設計網站的使用者介面和體驗流程",
  "task_type": "main",
  "parent_task_id": "550e8400-e29b-41d4-a716-446655440001",
  "priority": 2,
  "difficulty": 3,
  "experience": 60,
  "task_order": 1,
  "due_date": "2024-02-15T23:59:59Z"
}
```

## 欄位說明

### 任務狀態 (status)
- `0`: 待處理 (Pending)
- `1`: 進行中 (InProgress)
- `2`: 已完成 (Completed)
- `3`: 已取消 (Cancelled)
- `4`: 已暫停 (Paused)
- `5`: 每日任務進行中 (DailyInProgress)
- `6`: 每日任務已完成 (DailyCompleted)
- `7`: 每日任務未完成 (DailyNotCompleted)

### 任務類型 (task_type)
- `main`: 主要任務
- `side`: 副線任務
- `challenge`: 挑戰任務
- `daily`: 日常任務

### 優先級 (priority)
- `0`: 低優先級
- `1`: 中優先級
- `2`: 高優先級

### 難度等級 (difficulty)
- `1`: 非常簡單
- `2`: 簡單
- `3`: 中等
- `4`: 困難
- `5`: 非常困難

### 重複模式 (recurrence_pattern)
- `daily`: 每天
- `weekdays`: 工作日（週一至週五）
- `weekends`: 週末（週六、週日）
- `weekly`: 每週

## 使用注意事項

1. **日期格式**：所有日期時間欄位都使用 ISO 8601 格式（例如：`2024-01-15T10:00:00Z`）

2. **重複性任務**：
   - 當 `is_recurring` 為 `true` 時，必須提供 `recurrence_pattern` 和 `start_date`
   - `completion_target` 表示目標完成率（例如 0.8 表示 80% 的完成率目標）

3. **父子任務關係**：
   - 大任務設置 `is_parent_task` 為 `1`
   - 子任務通過 `parent_task_id` 關聯到父任務

4. **經驗值設定**：
   - 根據任務難度和重要性設定適當的經驗值
   - 挑戰任務通常有較高的經驗值獎勵

5. **任務排序**：
   - 使用 `task_order` 欄位來控制任務在列表中的顯示順序
   - 數值越小，排序越靠前

## MCP Server 整合建議

在 MCP Server 中使用此 Schema 時，建議：

1. 使用簡化版 Schema 作為輸入驗證
2. AI 生成任務時，根據上下文自動判斷適當的任務類型和難度
3. 自動計算合理的經驗值（例如：經驗值 = 難度 × 20 + 優先級 × 10）
4. 根據任務描述自動推薦是否設為重複性任務