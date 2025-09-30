#!/bin/bash

echo "===== 測試 AI 生成任務 JSON API ====="
echo ""

echo "1. 測試生成每日運動任務："
curl -X POST http://127.0.0.1:8080/api/tasks/generate-json \
  -H "Content-Type: application/json" \
  -d '{"description":"每天早上跑步30分鐘"}' \
  | python -m json.tool

echo ""
echo "----------------------------------------"
echo ""

echo "2. 測試生成學習任務："
curl -X POST http://127.0.0.1:8080/api/tasks/generate-json \
  -H "Content-Type: application/json" \
  -d '{"description":"學習 Rust 程式語言"}' \
  | python -m json.tool

echo ""
echo "----------------------------------------"
echo ""

echo "3. 測試生成工作日重複任務："
curl -X POST http://127.0.0.1:8080/api/tasks/generate-json \
  -H "Content-Type: application/json" \
  -d '{"description":"週一到週五寫工作日誌"}' \
  | python -m json.tool

echo ""
echo "----------------------------------------"
echo ""

echo "4. 測試生成週末任務："
curl -X POST http://127.0.0.1:8080/api/tasks/generate-json \
  -H "Content-Type: application/json" \
  -d '{"description":"週末整理房間和洗衣服"}' \
  | python -m json.tool

echo ""
echo "----------------------------------------"
echo ""

echo "5. 測試生成挑戰任務："
curl -X POST http://127.0.0.1:8080/api/tasks/generate-json \
  -H "Content-Type: application/json" \
  -d '{"description":"完成一個馬拉松比賽"}' \
  | python -m json.tool

echo ""
echo "----------------------------------------"
echo ""

echo "6. 測試生成每週特定日期任務："
curl -X POST http://127.0.0.1:8080/api/tasks/generate-json \
  -H "Content-Type: application/json" \
  -d '{"description":"每週三和週五做瑜伽"}' \
  | python -m json.tool

echo ""
echo "----------------------------------------"
echo ""

echo "7. 測試生成專案開發任務："
curl -X POST http://127.0.0.1:8080/api/tasks/generate-json \
  -H "Content-Type: application/json" \
  -d '{"description":"開發一個個人網站"}' \
  | python -m json.tool

echo ""
echo "----------------------------------------"
echo ""

echo "8. 測試生成健身計畫："
curl -X POST http://127.0.0.1:8080/api/tasks/generate-json \
  -H "Content-Type: application/json" \
  -d '{"description":"每天做100個伏地挺身"}' \
  | python -m json.tool

echo ""
echo "----------------------------------------"
echo ""

echo "9. 測試生成閱讀任務："
curl -X POST http://127.0.0.1:8080/api/tasks/generate-json \
  -H "Content-Type: application/json" \
  -d '{"description":"每天閱讀30頁書"}' \
  | python -m json.tool

echo ""
echo "----------------------------------------"
echo ""

echo "10. 測試生成冥想任務："
curl -X POST http://127.0.0.1:8080/api/tasks/generate-json \
  -H "Content-Type: application/json" \
  -d '{"description":"每天早晚各冥想10分鐘"}' \
  | python -m json.tool