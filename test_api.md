# 測試新的 AI 任務 API

## 1. 測試第一個 API：AI 生成 JSON

```bash
curl -X POST http://127.0.0.1:8080/api/tasks/generate-json \
  -H "Content-Type: application/json" \
  -d '{"description":"每週一三五做瑜伽練習"}'
```

預期返回：符合 task_schema.md 的 JSON 格式任務資料

## 2. 測試第二個 API：JSON 插入資料庫

使用第一個 API 返回的 JSON 資料：

```bash
curl -X POST http://127.0.0.1:8080/api/tasks/insert-json \
  -H "Content-Type: application/json" \
  -d '{
    "task_json": {
      "title": "每週瑜伽練習",
      "description": "每週一、三、五進行瑜伽練習，保持身心健康",
      "task_type": "daily",
      "priority": 1,
      "difficulty": 2,
      "experience": 50,
      "is_recurring": true,
      "recurrence_pattern": "weekly",
      "start_date": "2024-01-01T00:00:00Z",
      "end_date": "2024-03-31T23:59:59Z",
      "completion_target": 0.8
    },
    "user_id": null
  }'
```

預期返回：任務成功插入資料庫的確認訊息

## 3. 測試原有的組合 API（保持相容性）

```bash
curl -X POST http://127.0.0.1:8080/api/tasks/generate \
  -H "Content-Type: application/json" \
  -d '{"description":"每天早上跑步30分鐘"}'
```

預期返回：AI 生成任務並自動插入資料庫