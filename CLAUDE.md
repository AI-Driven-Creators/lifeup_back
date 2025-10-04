# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 專案概覽

LifeUp 是一個生活管理應用的後端服務，使用 Rust + Actix-web + Rbatis 架構建構，採用遊戲化概念幫助使用者管理日常任務與技能發展。

## 常用開發指令

### 建構與執行
```bash
# 開發模式執行
cargo run

# 發布模式執行
cargo run --release

# 編譯專案
cargo build

# 發布版本編譯
cargo build --release
```

### 資料庫管理
```bash
# 完整重置資料庫並插入測試數據
cargo run -- --reset-db

# 僅插入種子數據（保留現有數據）
cargo run -- --seed

# 使用互動式資料庫管理工具
./scripts/reset_db.sh
```

### 測試
```bash
# 執行所有測試
cargo test

# 執行特定測試
cargo test test_name

# 顯示測試輸出
cargo test -- --nocapture
```

### 程式碼檢查
```bash
# 檢查程式碼（不編譯）
cargo check

# 格式化程式碼
cargo fmt

# 執行 Clippy 檢查
cargo clippy
```

## 核心架構

### 技術棧
- **Web 框架**: Actix-web 4.4 - 高效能非同步 Web 框架
- **ORM**: Rbatis 4.6 - 支援多種資料庫的 ORM 框架
- **資料庫**: SQLite
- **序列化**: Serde - Rust 生態系統標準序列化庫
- **日誌**: fast_log - 高效能日誌系統
- **非同步運行時**: Tokio - Rust 非同步運行時

### 專案結構
- `src/main.rs`: 應用程式入口，初始化伺服器、資料庫連接、路由配置
- `src/config.rs`: 配置管理模組，處理環境變數和應用配置
- `src/models.rs`: 資料模型定義，包含所有實體結構與 CRUD 操作
- `src/routes.rs`: API 路由處理函數，實現業務邏輯
- `src/database_reset.rs`: 資料庫重置功能，用於開發環境
- `src/seed_data.rs`: 種子數據生成，提供測試數據

### 資料模型架構

#### 核心實體
- **User**: 使用者基本資訊
- **Task**: 任務系統，支援主任務、子任務、每日任務、挑戰任務
- **Skill**: 技能系統，分為技術技能和軟技能
- **UserProfile**: 遊戲化用戶資料（等級、經驗值、稱號等）
- **UserAttributes**: 六維屬性系統（智力、毅力、創造力、社交力、專注力、適應力）

#### 任務狀態系統
任務擁有 8 種狀態，透過 `TaskStatus` 枚舉管理：
- Pending (0): 待處理
- InProgress (1): 進行中
- Completed (2): 已完成
- Cancelled (3): 已取消
- Paused (4): 已暫停
- DailyInProgress (5): 每日任務進行中
- DailyCompleted (6): 每日任務已完成
- DailyNotCompleted (7): 每日任務未完成

#### 重複性任務系統
支援多種重複模式：daily、weekdays、weekends、weekly，包含完成率追蹤和自動子任務生成。

### API 架構

所有 API 路由在 `src/routes.rs` 中定義，採用 RESTful 設計：
- `/health`: 健康檢查
- `/api/users/*`: 使用者管理
- `/api/tasks/*`: 任務管理（包含重複性任務、子任務、進度追蹤）
- `/api/skills/*`: 技能管理
- `/api/chat/*`: AI 教練聊天功能
- `/api/profile/*`: 遊戲化資料管理
- `/api/attributes/*`: 屬性系統管理
- `/api/progress/*`: 進度追蹤

### 資料庫操作

使用 Rbatis ORM 框架，透過 `crud!` 宏自動生成 CRUD 操作：
```rust
crud!(User{});  // 自動生成 select_all, select_by_map, insert, update, delete 等方法
```

複雜查詢使用原生 SQL 透過 `rb.query_decode` 執行。

## 開發注意事項

1. **環境變數配置**：透過 `.env` 文件或環境變數設置，主要包含：
   - `DATABASE_URL`: 資料庫連接字串
   - `SERVER_HOST` 和 `SERVER_PORT`: 伺服器配置
   - `RUST_LOG`: 日誌級別
   - `ENVIRONMENT`: 運行環境（development/production）

2. **錯誤處理**：統一使用 `ApiResponse` 結構返回，包含 success、data、message 欄位

3. **時間處理**：使用 chrono 庫處理時間，統一使用 UTC 時區

4. **UUID 生成**：所有實體 ID 使用 UUID v4 生成

5. **CORS 配置**：開發環境允許所有來源，生產環境需要調整

6. **日誌系統**：使用 fast_log，支援多種日誌級別（error、warn、info、debug、trace）

7. **資料庫遷移**：在 `main.rs` 中的 `migrate_database` 函數處理資料庫架構更新

8. **測試數據**：`seed_data.rs` 提供完整的測試數據集，包含使用者、任務、技能等

## 部署考量

- 生產環境自動阻止資料庫重置操作
- 支援 Docker 部署（參考 README.md 中的 Dockerfile）
- 可配置為 systemd 服務
- 資料庫備份功能內建於重置流程中