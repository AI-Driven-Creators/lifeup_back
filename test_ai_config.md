# AI 服務配置測試

## 環境變數配置

現在支援以下 AI 服務配置：

### OpenAI 配置
```bash
API_OPTION=OpenAI
OPENAI_API_KEY=your_openai_api_key_here
OPENAI_MODEL=gpt-4o-mini  # 或其他 OpenAI 模型
```

### OpenRouter 配置
```bash
API_OPTION=OpenRouter
OPENROUTER_API_KEY=your_openrouter_api_key_here
OPENROUTER_MODEL=openrouter.ai/google/gemma-3n-e4b-it  # 或其他 OpenRouter 模型
```

## 支援的模型

### OpenAI 模型
- `gpt-4o-mini` (預設)
- `gpt-4o`
- `gpt-4-turbo`
- `gpt-3.5-turbo`

### OpenRouter 模型
- `openrouter.ai/google/gemma-3n-e4b-it` (預設)
- `openrouter.ai/meta-llama/llama-3.1-8b-instruct`
- `openrouter.ai/anthropic/claude-3.5-sonnet`
- 其他 OpenRouter 支援的模型

## 測試方法

1. 設定環境變數
2. 啟動後端服務：`cargo run`
3. 測試 AI 功能：
   - 任務生成：`POST /api/tasks/generate`
   - 成就生成：`POST /api/achievements/generate`
   - 聊天功能：`POST /api/chat/send`

## 配置驗證

系統會在啟動時驗證 AI 配置，如果配置錯誤會顯示相應的錯誤訊息。
