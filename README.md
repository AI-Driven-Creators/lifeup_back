# LifeUp Backend

LifeUp 生活管理應用的後端服務，基於 Rust + Actix-web + Rbatis 建構。

## 功能特性

- 🚀 **高效能**: 基於 Rust 和 Actix-web 的高效能 Web 服務
- 🗄️ **資料庫**: 使用 Rbatis ORM 框架，支援多種資料庫
- 🔐 **API**: RESTful API 設計，支援 CORS
- 📝 **日誌**: 完整的日誌記錄系統
- ⚙️ **配置**: 靈活的環境配置管理

## 技術棧

- **Web 框架**: Actix-web 4.4
- **ORM**: Rbatis 4.6
- **資料庫**: SQLite (預設，可切換為 MySQL/PostgreSQL)
- **序列化**: Serde
- **日誌**: fast_log
- **非同步**: Tokio

## 專案結構

```
lifeup_back/
├── src/
│   ├── main.rs           # 主程式入口
│   ├── config.rs         # 配置管理
│   ├── models.rs         # 資料模型
│   ├── routes.rs         # API 路由
│   ├── database_reset.rs # 數據庫重置模組
│   └── seed_data.rs      # 種子數據模組
├── scripts/
│   └── reset_db.sh       # 數據庫重置腳本
├── backups/              # 數據庫備份目錄
├── Cargo.toml            # 專案依賴
└── README.md            # 專案說明
```

## 安裝和執行

### 前置要求

- Rust 1.70+ 
- Cargo

### 安裝依賴

```bash
cd lifeup_back
cargo build
```

### 執行專案

```bash
# 開發模式
cargo run

# 發布模式
cargo run --release
```

服務將在 `http://127.0.0.1:8080` 啟動

## 🗄️ 數據庫管理

### 快速開始（推薦）

使用便捷腳本快速設置開發環境：

```bash
# 執行互動式數據庫管理工具
./scripts/reset_db.sh
```

### 命令行選項

```bash
# 完整重置數據庫並插入測試數據
cargo run -- --reset-db

# 僅插入種子數據（保留現有數據）
cargo run -- --seed
```

### 測試數據包含

- **測試用戶**: test@lifeup.com
- **主任務**: 學習 Vue.js、掌握 Rust、建立健康作息等
- **子任務**: 完整的學習階段劃分
- **不同狀態**: 進行中、已完成、暫停、已取消的任務範例
- **技能數據**: Vue.js、Rust、JavaScript 等技能進度
- **聊天記錄**: AI教練對話範例

### 安全特性

- ⚠️ 數據庫重置僅在開發環境執行
- 🔒 生產環境自動阻止重置操作
- 💾 自動備份功能防止數據丟失

## API 介面

### 健康檢查

```
GET /health
```

### 使用者管理

```
GET    /api/users          # 獲取使用者列表
POST   /api/users          # 建立使用者
GET    /api/users/{id}     # 獲取指定使用者
```

### 任務管理

```
GET    /api/tasks          # 獲取任務列表
POST   /api/tasks          # 建立任務
```

### 技能管理

```
GET    /api/skills         # 獲取技能列表
POST   /api/skills         # 建立技能
```

### 聊天功能

```
GET    /api/chat/messages  # 獲取聊天記錄
POST   /api/chat/send      # 發送訊息
```

## 資料模型

### User (使用者)
- `id`: 使用者唯一識別
- `name`: 使用者姓名
- `email`: 使用者信箱
- `created_at`: 建立時間
- `updated_at`: 更新時間

### Task (任務)
- `id`: 任務唯一識別
- `user_id`: 所屬使用者ID
- `title`: 任務標題
- `description`: 任務描述
- `status`: 任務狀態 (0: 待完成, 1: 進行中, 2: 已完成, 3: 已取消)
- `priority`: 優先級 (0: 低, 1: 中, 2: 高)
- `due_date`: 截止日期
- `created_at`: 建立時間
- `updated_at`: 更新時間

### Skill (技能)
- `id`: 技能唯一識別
- `user_id`: 所屬使用者ID
- `name`: 技能名稱
- `description`: 技能描述
- `level`: 技能等級 (1-10)
- `progress`: 進度 (0.0-1.0)
- `created_at`: 建立時間
- `updated_at`: 更新時間

### ChatMessage (聊天記錄)
- `id`: 訊息唯一識別
- `user_id`: 使用者ID
- `role`: 角色 ("user" 或 "assistant")
- `content`: 訊息內容
- `created_at`: 建立時間

## 環境配置

可以透過環境變數配置應用：

```bash
# 資料庫配置
export DATABASE_URL="sqlite://lifeup.db"

# 伺服器配置
export SERVER_HOST="127.0.0.1"
export SERVER_PORT="8080"

# 日誌級別
export RUST_LOG="info"

# 環境
export ENVIRONMENT="development"
```

## 資料庫支援

預設使用 SQLite，如需使用其他資料庫，請：

1. 在 `Cargo.toml` 中啟用對應的資料庫驅動
2. 修改 `DATABASE_URL` 環境變數

### MySQL
```toml
rbdc-mysql = { version = "4.6" }
```

### PostgreSQL
```toml
rbdc-pg = { version = "4.6" }
```

### SQL Server
```toml
rbdc-mssql = { version = "4.6" }
```

## 開發

### 新增新的 API 路由

1. 在 `src/routes.rs` 中新增新的處理函數
2. 在 `src/main.rs` 中註冊路由

### 新增新的資料模型

1. 在 `src/models.rs` 中定義新的結構體
2. 在 `src/main.rs` 中新增 `crud!` 巨集
3. 在 `create_tables` 函數中新增建表 SQL

## 測試

```bash
# 執行測試
cargo test

# 執行特定測試
cargo test test_name
```

## 部署

### Docker 部署

```dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/lifeup_back /usr/local/bin/
EXPOSE 8080
CMD ["lifeup_back"]
```

### 系統服務

建立 systemd 服務檔案：

```ini
[Unit]
Description=LifeUp Backend
After=network.target

[Service]
Type=simple
User=lifeup
WorkingDirectory=/opt/lifeup_back
ExecStart=/opt/lifeup_back/lifeup_back
Restart=always
Environment=DATABASE_URL=sqlite:///opt/lifeup_back/lifeup.db

[Install]
WantedBy=multi-user.target
```

## 授權

MIT License

## 貢獻

歡迎提交 Issue 和 Pull Request！ 