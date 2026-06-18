use chrono::{DateTime, Timelike, Utc};
use zorch_shared::AppError;

pub struct AccessWindow {
    pub allowed_hours_start: Option<u8>,
    pub allowed_hours_end: Option<u8>,
    pub window_timezone: Option<String>,
}

impl AccessWindow {
    pub fn is_within_window(&self) -> Result<bool, AppError> {
        self.is_within_window_at(Utc::now())
    }

    pub fn is_within_window_at(&self, now: DateTime<Utc>) -> Result<bool, AppError> {
        let (start, end) = match (self.allowed_hours_start, self.allowed_hours_end) {
            (Some(s), Some(e)) => (s, e),
            _ => return Ok(true),
        };

        if start > 23 || end > 23 {
            return Err(AppError::Validation(
                "Access window hours must be 0-23".to_string(),
            ));
        }

        let tz_str = self.window_timezone.as_deref().unwrap_or("UTC");
        let tz: chrono_tz::Tz = tz_str.parse().map_err(|_| {
            AppError::Validation(format!("Invalid timezone: {}", tz_str))
        })?;

        let now_in_tz = now.with_timezone(&tz);
        let current_hour = now_in_tz.hour() as u8;

        let within = if start <= end {
            current_hour >= start && current_hour <= end
        } else {
            current_hour >= start || current_hour <= end
        };

        Ok(within)
    }

    pub fn window_description(&self) -> String {
        match (self.allowed_hours_start, self.allowed_hours_end) {
            (Some(s), Some(e)) => format!(
                "{:02}:00-{:02}:00 {}",
                s,
                e,
                self.window_timezone.as_deref().unwrap_or("UTC")
            ),
            _ => "No restriction".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_window(start: Option<u8>, end: Option<u8>, tz: Option<&str>) -> AccessWindow {
        AccessWindow {
            allowed_hours_start: start,
            allowed_hours_end: end,
            window_timezone: tz.map(String::from),
        }
    }

    fn hour_utc(h: u32) -> DateTime<Utc> {
        chrono::NaiveDate::from_ymd_opt(2024, 1, 15)
            .unwrap()
            .and_hms_opt(h, 0, 0)
            .unwrap()
            .and_utc()
    }

    #[test]
    fn no_window_always_allows() {
        let w = make_window(None, None, None);
        assert!(w.is_within_window().unwrap());
    }

    #[test]
    fn same_day_within_window() {
        let w = make_window(Some(9), Some(18), None);
        assert!(w.is_within_window_at(hour_utc(12)).unwrap());
    }

    #[test]
    fn same_day_outside_window() {
        let w = make_window(Some(9), Some(18), None);
        assert!(!w.is_within_window_at(hour_utc(6)).unwrap());
    }

    #[test]
    fn wrap_around_within_evening() {
        let w = make_window(Some(22), Some(6), None);
        assert!(w.is_within_window_at(hour_utc(23)).unwrap());
    }

    #[test]
    fn wrap_around_within_early_morning() {
        let w = make_window(Some(22), Some(6), None);
        assert!(w.is_within_window_at(hour_utc(3)).unwrap());
    }

    #[test]
    fn wrap_around_outside() {
        let w = make_window(Some(22), Some(6), None);
        assert!(!w.is_within_window_at(hour_utc(12)).unwrap());
    }

    #[test]
    fn boundary_at_start_hour() {
        let w = make_window(Some(9), Some(18), None);
        assert!(w.is_within_window_at(hour_utc(9)).unwrap());
    }

    #[test]
    fn boundary_at_end_hour() {
        let w = make_window(Some(9), Some(18), None);
        assert!(w.is_within_window_at(hour_utc(18)).unwrap());
    }

    #[test]
    fn invalid_timezone_returns_error() {
        let w = make_window(Some(9), Some(18), Some("Invalid/Zone"));
        assert!(w.is_within_window().is_err());
    }

    #[test]
    fn utc_timezone_works() {
        let w = make_window(Some(0), Some(23), Some("UTC"));
        assert!(w.is_within_window().unwrap());
    }

    #[test]
    fn window_description_no_window() {
        let w = make_window(None, None, None);
        assert_eq!(w.window_description(), "No restriction");
    }

    #[test]
    fn window_description_with_window() {
        let w = make_window(Some(9), Some(18), Some("Asia/Bangkok"));
        assert_eq!(w.window_description(), "09:00-18:00 Asia/Bangkok");
    }

    #[test]
    fn timezone_conversion_affects_result() {
        // 2024-01-15 03:00 UTC = 2024-01-15 12:00 Asia/Bangkok
        let utc_time = chrono::NaiveDate::from_ymd_opt(2024, 1, 15)
            .unwrap()
            .and_hms_opt(3, 0, 0)
            .unwrap()
            .and_utc();
        let w = make_window(Some(9), Some(18), Some("Asia/Bangkok"));
        // 12:00 Bangkok is within 9-18 window
        assert!(w.is_within_window_at(utc_time).unwrap());
    }

    #[test]
    fn full_day_window_always_allows() {
        let w = make_window(Some(0), Some(23), None);
        assert!(w.is_within_window_at(hour_utc(0)).unwrap());
        assert!(w.is_within_window_at(hour_utc(12)).unwrap());
        assert!(w.is_within_window_at(hour_utc(23)).unwrap());
    }

    #[test]
    fn single_hour_window() {
        let w = make_window(Some(12), Some(12), None);
        assert!(w.is_within_window_at(hour_utc(12)).unwrap());
        assert!(!w.is_within_window_at(hour_utc(11)).unwrap());
        assert!(!w.is_within_window_at(hour_utc(13)).unwrap());
    }
}
