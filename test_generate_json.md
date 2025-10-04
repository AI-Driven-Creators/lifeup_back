# 測試 AI 生成任務 JSON 的 curl 指令

## 1. 每日運動任務
```bash
curl -X POST http://127.0.0.1:8080/api/tasks/generate-json -H "Content-Type: application/json" -d "{\"description\":\"每天早上跑步30分鐘\"}"
```

**預期生成的 JSON 範例：**
```json
{
  "title": "每日晨跑訓練",
  "description": "每天早上進行30分鐘的跑步訓練，提升心肺功能",
  "task_type": "daily",
  "priority": 1,
  "difficulty": 3,
  "experience": 50,
  "is_recurring": true,
  "recurrence_pattern": "daily",
  "start_date": "2024-01-01T06:00:00Z",
  "completion_target": 0.8
}
```

**使用此 JSON 創建任務：**
```bash
curl -X POST http://127.0.0.1:8080/api/tasks/create-from-json -H "Content-Type: application/json" -d "{\"title\":\"每日晨跑訓練\",\"description\":\"每天早上進行30分鐘的跑步訓練，提升心肺功能\",\"task_type\":\"daily\",\"priority\":1,\"difficulty\":3,\"experience\":50,\"is_recurring\":true,\"recurrence_pattern\":\"daily\",\"start_date\":\"2024-01-01T06:00:00Z\",\"completion_target\":0.8}"
```

## 2. 學習任務
```bash
curl -X POST http://127.0.0.1:8080/api/tasks/generate-json -H "Content-Type: application/json" -d "{\"description\":\"學習 Rust 程式語言\"}"
```

**預期生成的 JSON 範例：**
```json
{
  "title": "Rust 程式語言學習",
  "description": "系統性學習 Rust 程式語言，掌握所有權、生命週期等核心概念",
  "task_type": "main",
  "priority": 2,
  "difficulty": 4,
  "experience": 120,
  "due_date": "2024-06-30T23:59:59Z",
  "is_recurring": false
}
```

**使用此 JSON 創建任務：**
```bash
curl -X POST http://127.0.0.1:8080/api/tasks/create-from-json -H "Content-Type: application/json" -d "{\"title\":\"Rust 程式語言學習\",\"description\":\"系統性學習 Rust 程式語言，掌握所有權、生命週期等核心概念\",\"task_type\":\"main\",\"priority\":2,\"difficulty\":4,\"experience\":120,\"due_date\":\"2024-06-30T23:59:59Z\",\"is_recurring\":false}"
```

## 3. 工作日重複任務
```bash
curl -X POST http://127.0.0.1:8080/api/tasks/generate-json -H "Content-Type: application/json" -d "{\"description\":\"週一到週五寫工作日誌\"}"
```

**預期生成的 JSON 範例：**
```json
{
  "title": "工作日誌撰寫",
  "description": "週一到週五每天記錄工作進度、學習心得和待改進事項",
  "task_type": "daily",
  "priority": 1,
  "difficulty": 2,
  "experience": 30,
  "is_recurring": true,
  "recurrence_pattern": "weekdays",
  "start_date": "2024-01-01T18:00:00Z",
  "completion_target": 0.9
}
```

**使用此 JSON 創建任務：**
```bash
curl -X POST http://127.0.0.1:8080/api/tasks/create-from-json -H "Content-Type: application/json" -d "{\"title\":\"工作日誌撰寫\",\"description\":\"週一到週五每天記錄工作進度、學習心得和待改進事項\",\"task_type\":\"daily\",\"priority\":1,\"difficulty\":2,\"experience\":30,\"is_recurring\":true,\"recurrence_pattern\":\"weekdays\",\"start_date\":\"2024-01-01T18:00:00Z\",\"completion_target\":0.9}"
```

## 4. 週末任務
```bash
curl -X POST http://127.0.0.1:8080/api/tasks/generate-json -H "Content-Type: application/json" -d "{\"description\":\"週末整理房間和洗衣服\"}"
```

**預期生成的 JSON 範例：**
```json
{
  "title": "週末家務整理",
  "description": "週六日進行房間清潔、整理和洗衣服等家務工作",
  "task_type": "daily",
  "priority": 1,
  "difficulty": 2,
  "experience": 40,
  "is_recurring": true,
  "recurrence_pattern": "weekends",
  "start_date": "2024-01-06T09:00:00Z",
  "completion_target": 0.85
}
```

**使用此 JSON 創建任務：**
```bash
curl -X POST http://127.0.0.1:8080/api/tasks/create-from-json -H "Content-Type: application/json" -d "{\"title\":\"週末家務整理\",\"description\":\"週六日進行房間清潔、整理和洗衣服等家務工作\",\"task_type\":\"daily\",\"priority\":1,\"difficulty\":2,\"experience\":40,\"is_recurring\":true,\"recurrence_pattern\":\"weekends\",\"start_date\":\"2024-01-06T09:00:00Z\",\"completion_target\":0.85}"
```

## 5. 挑戰任務
```bash
curl -X POST http://127.0.0.1:8080/api/tasks/generate-json -H "Content-Type: application/json" -d "{\"description\":\"完成一個馬拉松比賽\"}"
```

**預期生成的 JSON 範例：**
```json
{
  "title": "馬拉松挑戰",
  "description": "訓練並完成人生第一場全程馬拉松比賽（42.195公里）",
  "task_type": "challenge",
  "priority": 2,
  "difficulty": 5,
  "experience": 300,
  "due_date": "2024-12-01T08:00:00Z",
  "is_recurring": false
}
```

**使用此 JSON 創建任務：**
```bash
curl -X POST http://127.0.0.1:8080/api/tasks/create-from-json -H "Content-Type: application/json" -d "{\"title\":\"馬拉松挑戰\",\"description\":\"訓練並完成人生第一場全程馬拉松比賽（42.195公里）\",\"task_type\":\"challenge\",\"priority\":2,\"difficulty\":5,\"experience\":300,\"due_date\":\"2024-12-01T08:00:00Z\",\"is_recurring\":false}"
```

## 6. 每週特定日期任務
```bash
curl -X POST http://127.0.0.1:8080/api/tasks/generate-json -H "Content-Type: application/json" -d "{\"description\":\"每週三和週五做瑜伽\"}"
```

**預期生成的 JSON 範例：**
```json
{
  "title": "瑜伽練習課程",
  "description": "每週三和週五進行1小時瑜伽練習，增強柔韌性和平衡感",
  "task_type": "daily",
  "priority": 1,
  "difficulty": 2,
  "experience": 45,
  "is_recurring": true,
  "recurrence_pattern": "weekdays",
  "start_date": "2024-01-03T19:00:00Z",
  "completion_target": 0.8
}
```

**使用此 JSON 創建任務：**
```bash
curl -X POST http://127.0.0.1:8080/api/tasks/create-from-json -H "Content-Type: application/json" -d "{\"title\":\"瑜伽練習課程\",\"description\":\"每週三和週五進行1小時瑜伽練習，增強柔韌性和平衡感\",\"task_type\":\"daily\",\"priority\":1,\"difficulty\":2,\"experience\":45,\"is_recurring\":true,\"recurrence_pattern\":\"weekdays\",\"start_date\":\"2024-01-03T19:00:00Z\",\"completion_target\":0.8}"
```

## 7. 專案開發任務
```bash
curl -X POST http://127.0.0.1:8080/api/tasks/generate-json -H "Content-Type: application/json" -d "{\"description\":\"開發一個個人網站\"}"
```

**預期生成的 JSON 範例：**
```json
{
  "title": "個人網站開發專案",
  "description": "設計並開發一個展示個人作品和技能的響應式網站",
  "task_type": "main",
  "priority": 2,
  "difficulty": 4,
  "experience": 200,
  "due_date": "2024-08-31T23:59:59Z",
  "is_recurring": false
}
```

**使用此 JSON 創建任務：**
```bash
curl -X POST http://127.0.0.1:8080/api/tasks/create-from-json -H "Content-Type: application/json" -d "{\"title\":\"個人網站開發專案\",\"description\":\"設計並開發一個展示個人作品和技能的響應式網站\",\"task_type\":\"main\",\"priority\":2,\"difficulty\":4,\"experience\":200,\"due_date\":\"2024-08-31T23:59:59Z\",\"is_recurring\":false}"
```

## 8. 健身計畫
```bash
curl -X POST http://127.0.0.1:8080/api/tasks/generate-json -H "Content-Type: application/json" -d "{\"description\":\"每天做100個伏地挺身\"}"
```

**預期生成的 JSON 範例：**
```json
{
  "title": "每日伏地挺身訓練",
  "description": "每天完成100個伏地挺身，增強上肢力量和核心肌群",
  "task_type": "daily",
  "priority": 1,
  "difficulty": 3,
  "experience": 60,
  "is_recurring": true,
  "recurrence_pattern": "daily",
  "start_date": "2024-01-01T07:00:00Z",
  "completion_target": 0.85
}
```

**使用此 JSON 創建任務：**
```bash
curl -X POST http://127.0.0.1:8080/api/tasks/create-from-json -H "Content-Type: application/json" -d "{\"title\":\"每日伏地挺身訓練\",\"description\":\"每天完成100個伏地挺身，增強上肢力量和核心肌群\",\"task_type\":\"daily\",\"priority\":1,\"difficulty\":3,\"experience\":60,\"is_recurring\":true,\"recurrence_pattern\":\"daily\",\"start_date\":\"2024-01-01T07:00:00Z\",\"completion_target\":0.85}"
```

## 9. 閱讀任務
```bash
curl -X POST http://127.0.0.1:8080/api/tasks/generate-json -H "Content-Type: application/json" -d "{\"description\":\"每天閱讀30頁書\"}"
```

**預期生成的 JSON 範例：**
```json
{
  "title": "每日閱讀計畫",
  "description": "每天閱讀30頁書籍，擴展知識面和提升思維能力",
  "task_type": "daily",
  "priority": 1,
  "difficulty": 2,
  "experience": 35,
  "is_recurring": true,
  "recurrence_pattern": "daily",
  "start_date": "2024-01-01T20:00:00Z",
  "completion_target": 0.9
}
```

**使用此 JSON 創建任務：**
```bash
curl -X POST http://127.0.0.1:8080/api/tasks/create-from-json -H "Content-Type: application/json" -d "{\"title\":\"每日閱讀計畫\",\"description\":\"每天閱讀30頁書籍，擴展知識面和提升思維能力\",\"task_type\":\"daily\",\"priority\":1,\"difficulty\":2,\"experience\":35,\"is_recurring\":true,\"recurrence_pattern\":\"daily\",\"start_date\":\"2024-01-01T20:00:00Z\",\"completion_target\":0.9}"
```

## 10. 冥想任務
```bash
curl -X POST http://127.0.0.1:8080/api/tasks/generate-json -H "Content-Type: application/json" -d "{\"description\":\"每天早晚各冥想10分鐘\"}"
```

**預期生成的 JSON 範例：**
```json
{
  "title": "每日冥想練習",
  "description": "每天早上和晚上各進行10分鐘冥想，提升專注力和內心平靜",
  "task_type": "daily",
  "priority": 1,
  "difficulty": 1,
  "experience": 25,
  "is_recurring": true,
  "recurrence_pattern": "daily",
  "start_date": "2024-01-01T06:30:00Z",
  "completion_target": 0.95
}
```

**使用此 JSON 創建任務：**
```bash
curl -X POST http://127.0.0.1:8080/api/tasks/create-from-json -H "Content-Type: application/json" -d "{\"title\":\"每日冥想練習\",\"description\":\"每天早上和晚上各進行10分鐘冥想，提升專注力和內心平靜\",\"task_type\":\"daily\",\"priority\":1,\"difficulty\":1,\"experience\":25,\"is_recurring\":true,\"recurrence_pattern\":\"daily\",\"start_date\":\"2024-01-01T06:30:00Z\",\"completion_target\":0.95}"
```

## Windows PowerShell 版本

如果你使用 Windows PowerShell，可以使用以下格式：

```powershell
# PowerShell 範例 1：每日運動任務
Invoke-RestMethod -Uri "http://127.0.0.1:8080/api/tasks/generate-json" `
  -Method POST `
  -ContentType "application/json" `
  -Body '{"description":"每天早上跑步30分鐘"}' | ConvertTo-Json

# PowerShell 範例 2：學習任務
Invoke-RestMethod -Uri "http://127.0.0.1:8080/api/tasks/generate-json" `
  -Method POST `
  -ContentType "application/json" `
  -Body '{"description":"學習 Rust 程式語言"}' | ConvertTo-Json
```

## 預期返回的 JSON 結構

每個請求應該返回符合 `task_schema.md` 的 JSON，例如：

```json
{
  "success": true,
  "data": {
    "title": "每日跑步訓練",
    "description": "每天早上進行30分鐘的跑步訓練",
    "task_type": "daily",
    "priority": 1,
    "difficulty": 3,
    "experience": 50,
    "due_date": null,
    "is_recurring": true,
    "recurrence_pattern": "daily",
    "start_date": "2024-01-01T06:00:00Z",
    "end_date": null,
    "completion_target": 0.8
  },
  "message": "AI 成功生成任務 JSON"
}
```

## 測試步驟

1. 確保伺服器正在運行（`cargo run`）
2. 選擇上面任一個 curl 指令執行
3. 檢查返回的 JSON 是否符合預期格式
4. 可以將返回的 `data` 部分用於測試插入資料庫 API

## 將生成的 JSON 創建為任務

### 方法 1：使用簡化的 API（推薦）

獲得 JSON 後，可以直接使用以下格式創建任務：

```bash
curl -X POST http://127.0.0.1:8080/api/tasks/create-from-json \
  -H "Content-Type: application/json" \
  -d '{
    "title": "每天早上跑步30分鐘",
    "description": "每天早上進行30分鐘的跑步訓練",
    "task_type": "daily",
    "priority": 1,
    "difficulty": 3,
    "experience": 50,
    "is_recurring": true,
    "recurrence_pattern": "daily",
    "start_date": "2024-01-01T06:00:00Z",
    "completion_target": 0.8,
    "user_id": null
  }'
```

### 方法 2：使用包裝格式的 API

```bash
curl -X POST http://127.0.0.1:8080/api/tasks/insert-json \
  -H "Content-Type: application/json" \
  -d '{
    "task_json": {
      # 這裡放入上一步返回的 data 內容
    },
    "user_id": null
  }'
```

## 完整的兩步驟工作流程示例

### 步驟 1：生成 JSON
```bash
curl -X POST http://127.0.0.1:8080/api/tasks/generate-json -H "Content-Type: application/json" -d "{\"description\":\"每天晚上讀書30分鐘\"}"
```

### 步驟 2：創建任務（使用上一步返回的 JSON）
```bash
curl -X POST http://127.0.0.1:8080/api/tasks/create-from-json -H "Content-Type: application/json" -d "{\"title\":\"每日閱讀計畫\",\"description\":\"每天晚上進行30分鐘的閱讀\",\"task_type\":\"daily\",\"priority\":1,\"difficulty\":2,\"experience\":40,\"is_recurring\":true,\"recurrence_pattern\":\"daily\",\"start_date\":\"2024-01-01T19:00:00Z\",\"completion_target\":0.85}"
```