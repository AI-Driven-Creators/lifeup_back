use chrono::{NaiveDate, Datelike};
use std::collections::HashSet;
use std::fs;
use std::sync::{Arc, RwLock};

/// 假日服務，用於判斷特定日期是否為假日
#[derive(Clone)]
pub struct CalendarService {
    holidays: Arc<RwLock<HashSet<NaiveDate>>>,
}

impl CalendarService {
    /// 創建新的日曆服務並載入假日資料
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut holidays = HashSet::new();

        // 載入所有年度的假日 CSV 文件
        let calendar_dir = "calendar";

        if let Ok(entries) = fs::read_dir(calendar_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("csv") {
                    log::info!("載入假日文件: {:?}", path);
                    match Self::load_holidays_from_csv(&path, &mut holidays) {
                        Ok(count) => log::info!("✅ 成功載入 {} 個假日", count),
                        Err(e) => log::error!("❌ 載入假日文件失敗: {}", e),
                    }
                }
            }
        } else {
            log::warn!("警告: calendar 目錄不存在或無法讀取");
        }

        log::info!("假日服務初始化完成，共載入 {} 個假日", holidays.len());

        Ok(Self {
            holidays: Arc::new(RwLock::new(holidays)),
        })
    }

    /// 從 CSV 文件載入假日資料
    fn load_holidays_from_csv(
        path: &std::path::Path,
        holidays: &mut HashSet<NaiveDate>,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let mut count = 0;

        for line in content.lines().skip(1) {
            // 跳過標題行
            if line.trim().is_empty() {
                continue;
            }

            // CSV 格式: Subject,Start Date,Start Time,End Date,End Time,All Day Event,Description,Location
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() < 2 {
                continue;
            }

            let subject = parts[0].trim();
            let date_str = parts[1].trim();

            // 跳過空行或沒有日期的行
            if subject.is_empty() || date_str.is_empty() {
                continue;
            }

            // 解析日期 (格式: YYYY/M/D 或 YYYY/MM/DD)
            if let Ok(date) = Self::parse_date(date_str) {
                holidays.insert(date);
                count += 1;
            }
        }

        Ok(count)
    }

    /// 解析日期字串 (支援 YYYY/M/D 和 YYYY/MM/DD 格式)
    fn parse_date(date_str: &str) -> Result<NaiveDate, Box<dyn std::error::Error>> {
        let parts: Vec<&str> = date_str.split('/').collect();
        if parts.len() != 3 {
            return Err("無效的日期格式".into());
        }

        let year: i32 = parts[0].parse()?;
        let month: u32 = parts[1].parse()?;
        let day: u32 = parts[2].parse()?;

        Ok(NaiveDate::from_ymd_opt(year, month, day)
            .ok_or("無效的日期")?)
    }

    /// 檢查指定日期是否為假日（包含週末和國定假日）
    pub fn is_holiday(&self, date: NaiveDate) -> bool {
        if let Ok(holidays) = self.holidays.read() {
            holidays.contains(&date)
        } else {
            log::error!("無法讀取假日資料");
            false
        }
    }

    /// 檢查指定日期是否為週末（週六或週日）
    pub fn is_weekend(&self, date: NaiveDate) -> bool {
        let weekday = date.weekday();
        matches!(weekday, chrono::Weekday::Sat | chrono::Weekday::Sun)
    }

    /// 檢查指定日期是否為工作日（非週末且非假日）
    pub fn is_workday(&self, date: NaiveDate) -> bool {
        !self.is_weekend(date) && !self.is_holiday(date)
    }

    /// 獲取假日總數
    pub fn get_holiday_count(&self) -> usize {
        if let Ok(holidays) = self.holidays.read() {
            holidays.len()
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_date() {
        assert!(CalendarService::parse_date("2025/1/1").is_ok());
        assert!(CalendarService::parse_date("2025/12/25").is_ok());
        assert!(CalendarService::parse_date("invalid").is_err());
    }

    #[test]
    fn test_is_weekend() {
        let service = CalendarService::new().unwrap();

        // 2025/11/9 是週六
        let saturday = NaiveDate::from_ymd_opt(2025, 11, 9).unwrap();
        assert!(service.is_weekend(saturday));

        // 2025/11/10 是週日
        let sunday = NaiveDate::from_ymd_opt(2025, 11, 10).unwrap();
        assert!(service.is_weekend(sunday));

        // 2025/11/11 是週一
        let monday = NaiveDate::from_ymd_opt(2025, 11, 11).unwrap();
        assert!(!service.is_weekend(monday));
    }
}
