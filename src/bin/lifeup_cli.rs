use std::fs;
use std::io::{self, Read};

// 本地定義必要的資料結構，避免依賴 lib 目標
#[derive(serde::Deserialize)]
struct SkillTag { name: String, category: String }

#[derive(serde::Deserialize)]
struct GeneratedTask {
    title: String,
    description: String,
    difficulty: serde_json::Number,
    estimated_hours: serde_json::Number,
    skill_tags: Vec<SkillTag>,
    resources: Vec<String>,
    #[allow(dead_code)]
    personality_match: Option<String>,
}

#[derive(serde::Deserialize)]
struct GeneratedTasksResponse {
    learning_summary: String,
    estimated_months: serde_json::Number,
    personality_insights: String,
    main_tasks: Vec<GeneratedTask>,
    daily_tasks: Vec<GeneratedTask>,
    project_tasks: Vec<GeneratedTask>,
}

#[tokio::main]
async fn main() {
    // 支援兩個子命令：
    // 1) validate-career [--file <path>]  解析並驗證 career JSON（AI 產生的多任務結構）
    // 2) import-task [--file <path>]      讀入單任務 JSON，呼叫後端 HTTP API 寫入 DB

    let mut args = std::env::args().skip(1);
    let sub = args.next().unwrap_or_else(|| {
        eprintln!(
            "用法：\n  \
  cargo run --bin lifeup_cli -- validate-career [--file <path>] \\\n+  cargo run --bin lifeup_cli -- import-task [--file <path>] [--career <name>] [--user-id <id>]\n\n示例：\n  cargo run --bin lifeup_cli -- validate-career --file career.json\n  cargo run --bin lifeup_cli -- import-task --file career.json --career \"UI 設計師\" --user-id 123e4567-e89b-12d3-a456-426614174000\n\n也可從 stdin 讀取：\n  type career.json | cargo run --bin lifeup_cli -- validate-career\n  type career.json | cargo run --bin lifeup_cli -- import-task --career \"UI 設計師\" --user-id <id>"
        );
        std::process::exit(1);
    });

    match sub.as_str() {
        "validate-career" => {
            let content = read_input_or_file(&mut args);
            match parse_ai_tasks_response_local(&content) {
                Ok(parsed) => {
                    let total = parsed.main_tasks.len() + parsed.daily_tasks.len() + parsed.project_tasks.len();
                    println!(
                        "✅ 驗證通過：main={} daily={} project={}，estimated_months={}，summary_len={}",
                        parsed.main_tasks.len(),
                        parsed.daily_tasks.len(),
                        parsed.project_tasks.len(),
                        parsed.estimated_months,
                        parsed.learning_summary.len()
                    );
                    // 額外輸出每個任務的 title 與難度，方便人工檢查
                    for (idx, t) in parsed.main_tasks.iter().enumerate() {
                        println!("[main:{}] {} (diff {}, {}h)", idx + 1, t.title, t.difficulty, t.estimated_hours);
                    }
                    for (idx, t) in parsed.daily_tasks.iter().enumerate() {
                        println!("[daily:{}] {} (diff {}, {}h)", idx + 1, t.title, t.difficulty, t.estimated_hours);
                    }
                    for (idx, t) in parsed.project_tasks.iter().enumerate() {
                        println!("[project:{}] {} (diff {}, {}h)", idx + 1, t.title, t.difficulty, t.estimated_hours);
                    }
                    println!("總任務數：{}", total);
                }
                Err(e) => {
                    eprintln!("❌ 驗證失敗：{}", e);
                    std::process::exit(2);
                }
            }
        }
        "import-task" => {
            // 期待的是 career 任務總表 JSON（多任務），CLI 先清理並嘗試本地解析，確保格式正確後再送後端
            // 支援選項：--career <name>  --user-id <id>
            let mut career_name: Option<String> = None;
            let mut user_id: Option<String> = None;
            let mut rest: Vec<String> = Vec::new();
            while let Some(arg) = args.next() {
                if arg == "--career" { career_name = args.next(); }
                else if arg == "--user-id" { user_id = args.next(); }
                else { rest.push(arg); }
            }

            let raw = read_input_or_file(&mut rest.into_iter());
            let cleaned = clean_json_markdown(&raw);

            // 本地解析與驗證（同 validate-career）
            if let Err(e) = parse_ai_tasks_response_local(&cleaned) {
                eprintln!("❌ JSON 格式錯誤：{}", e);
                std::process::exit(2);
            }

            // 呼叫後端統一匯入端點，由伺服器進一步解析、建表與寫 DB
            let api = std::env::var("CLI_API_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
            let url = format!("{}/api/career/import", api);
            let payload = serde_json::json!({
                "selected_career": career_name,
                "user_id": user_id,
                "raw_json": cleaned,
            });
            match reqwest::Client::new()
                .post(url)
                .json(&payload)
                .send()
                .await {
                Ok(resp) => {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    if status.is_success() {
                        println!("✅ 匯入成功：{}", text);
                    } else {
                        eprintln!("❌ 匯入失敗（狀態碼 {}）：{}", status, text);
                        std::process::exit(3);
                    }
                }
                Err(err) => {
                    eprintln!("❌ 無法連線到 API：{}", err);
                    eprintln!("請確認後端已啟動（cargo run）或設定 CLI_API_BASE_URL");
                    std::process::exit(4);
                }
            }
        }
        _ => {
            eprintln!("未知指令：{}", sub);
            std::process::exit(1);
        }
    }
}

fn read_input_or_file(args: &mut impl Iterator<Item = String>) -> String {
    let mut next_opt = args.next();
    if matches!(next_opt.as_deref(), Some("--file")) {
        let path = args.next().unwrap_or_else(|| {
            eprintln!("--file 需要提供檔案路徑");
            std::process::exit(1);
        });
        return fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("讀檔失敗：{}", e);
            std::process::exit(1);
        });
    }

    // 若未提供 --file，則讀 stdin
    let mut buffer = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut buffer) {
        eprintln!("讀取標準輸入失敗：{}", e);
        std::process::exit(1);
    }
    buffer
}

fn parse_ai_tasks_response_local(ai_response: &str) -> Result<GeneratedTasksResponse, String> {
    let cleaned = ai_response
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string();
    let parsed: GeneratedTasksResponse = serde_json::from_str(&cleaned)
        .map_err(|e| format!("JSON 解析失敗: {}", e))?;

    // 基本驗證
    let mut check_task = |t: &GeneratedTask| -> Result<(), String> {
        if t.title.trim().is_empty() { return Err("title 不可為空".into()); }
        if t.description.trim().is_empty() { return Err("description 不可為空".into()); }
        let diff = if let Some(i) = t.difficulty.as_i64() { i as i32 } else if let Some(f) = t.difficulty.as_f64() { f.round() as i32 } else { return Err("difficulty 必須為數字".into()) };
        if diff < 1 || diff > 5 { return Err(format!("difficulty 超出範圍: {}", diff)); }
        let hours = if let Some(i) = t.estimated_hours.as_i64() { i as i32 } else if let Some(f) = t.estimated_hours.as_f64() { f.round() as i32 } else { return Err("estimated_hours 必須為數字".into()) };
        if hours <= 0 { return Err(format!("estimated_hours 需為正整數: {}", hours)); }
        if t.skill_tags.is_empty() { return Err("skill_tags 不可為空".into()); }
        for tag in &t.skill_tags {
            if tag.name.trim().is_empty() { return Err("skill_tag.name 不可為空".into()); }
            if tag.category != "technical" && tag.category != "soft" { return Err(format!("skill_tag.category 僅限 technical/soft: {}", tag.category)); }
        }
        Ok(())
    };

    for t in &parsed.main_tasks { check_task(t)?; }
    for t in &parsed.daily_tasks { check_task(t)?; }
    for t in &parsed.project_tasks { check_task(t)?; }

    Ok(parsed)
}

fn clean_json_markdown(input: &str) -> String {
    input
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string()
}


