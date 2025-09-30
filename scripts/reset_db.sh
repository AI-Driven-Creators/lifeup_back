#!/bin/bash

# 人生升級系統 - 數據庫重置腳本
# 此腳本提供便捷的數據庫重置和種子數據插入功能

set -e  # 遇到錯誤立即退出

# 顏色定義
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 獲取腳本所在目錄
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo -e "${BLUE}=== 人生升級系統 - 數據庫管理工具 ===${NC}"
echo

# 檢查是否在正確目錄
if [ ! -f "$PROJECT_DIR/Cargo.toml" ]; then
    echo -e "${RED}錯誤: 無法找到 Cargo.toml 文件${NC}"
    echo "請確保腳本在正確的專案目錄中執行"
    exit 1
fi

# 切換到專案目錄
cd "$PROJECT_DIR"

# 檢查環境變數
if [ ! -f ".env" ]; then
    echo -e "${YELLOW}警告: 找不到 .env 文件，將使用預設配置${NC}"
fi

# 顯示選項菜單
show_menu() {
    echo -e "${YELLOW}請選擇操作:${NC}"
    echo "1) 完整重置數據庫 (刪除所有數據並插入測試數據)"
    echo "2) 僅插入種子數據 (保留現有數據，添加測試數據)"
    echo "3) 備份當前數據庫"
    echo "4) 顯示數據庫信息"
    echo "5) 退出"
    echo
}

# 備份數據庫
backup_database() {
    local backup_name="lifeup_backup_$(date +%Y%m%d_%H%M%S).db"
    if [ -f "lifeup.db" ]; then
        cp "lifeup.db" "backups/$backup_name"
        echo -e "${GREEN}數據庫已備份到: backups/$backup_name${NC}"
    else
        echo -e "${YELLOW}數據庫文件不存在，無需備份${NC}"
    fi
}

# 顯示數據庫信息
show_db_info() {
    if [ -f "lifeup.db" ]; then
        local size=$(du -h "lifeup.db" | cut -f1)
        local modified=$(stat -c %y "lifeup.db" 2>/dev/null || stat -f %Sm "lifeup.db" 2>/dev/null || echo "Unknown")
        echo -e "${BLUE}數據庫信息:${NC}"
        echo "文件: lifeup.db"
        echo "大小: $size"
        echo "修改時間: $modified"
        
        # 顯示表數量（如果 sqlite3 可用）
        if command -v sqlite3 &> /dev/null; then
            local table_count=$(sqlite3 lifeup.db "SELECT COUNT(*) FROM sqlite_master WHERE type='table';" 2>/dev/null || echo "0")
            echo "表數量: $table_count"
        fi
    else
        echo -e "${YELLOW}數據庫文件不存在${NC}"
    fi
}

# 確認操作
confirm_action() {
    local message="$1"
    echo -e "${YELLOW}$message${NC}"
    read -p "確定要繼續嗎? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo -e "${BLUE}操作已取消${NC}"
        return 1
    fi
    return 0
}

# 創建備份目錄
mkdir -p backups

# 主菜單循環
while true; do
    show_menu
    read -p "請輸入選項 (1-5): " choice
    echo
    
    case $choice in
        1)
            if confirm_action "這將刪除所有現有數據並重新創建數據庫！"; then
                echo -e "${BLUE}正在執行完整數據庫重置...${NC}"
                
                # 備份現有數據庫
                backup_database
                
                # 執行重置
                cargo run -- --reset-db
                
                if [ $? -eq 0 ]; then
                    echo -e "${GREEN}✅ 數據庫重置完成！${NC}"
                    echo -e "${GREEN}✅ 測試數據已插入！${NC}"
                    echo
                    echo -e "${BLUE}你現在可以:${NC}"
                    echo "- 啟動後端服務: cargo run"
                    echo "- 使用測試用戶登入: test@lifeup.com"
                    echo "- 查看預設任務和技能數據"
                else
                    echo -e "${RED}❌ 數據庫重置失敗${NC}"
                fi
            fi
            ;;
        2)
            if confirm_action "這將在現有數據庫中插入測試數據"; then
                echo -e "${BLUE}正在插入種子數據...${NC}"
                
                cargo run -- --seed
                
                if [ $? -eq 0 ]; then
                    echo -e "${GREEN}✅ 種子數據插入完成！${NC}"
                else
                    echo -e "${RED}❌ 種子數據插入失敗${NC}"
                fi
            fi
            ;;
        3)
            echo -e "${BLUE}正在備份數據庫...${NC}"
            backup_database
            ;;
        4)
            show_db_info
            ;;
        5)
            echo -e "${BLUE}再見！${NC}"
            exit 0
            ;;
        *)
            echo -e "${RED}無效選項，請重新選擇${NC}"
            ;;
    esac
    
    echo
    read -p "按 Enter 鍵繼續..."
    echo
done