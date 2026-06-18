use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureLevel {
    None,
    #[default]
    MetadataOnly,
    Full,
}

impl CaptureLevel {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "none" => CaptureLevel::None,
            "full" => CaptureLevel::Full,
            _ => CaptureLevel::MetadataOnly,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_level_from_str_none() {
        assert_eq!(CaptureLevel::parse("none"), CaptureLevel::None);
        assert_eq!(CaptureLevel::parse("NONE"), CaptureLevel::None);
        assert_eq!(CaptureLevel::parse("None"), CaptureLevel::None);
    }

    #[test]
    fn test_capture_level_from_str_full() {
        assert_eq!(CaptureLevel::parse("full"), CaptureLevel::Full);
        assert_eq!(CaptureLevel::parse("FULL"), CaptureLevel::Full);
        assert_eq!(CaptureLevel::parse("Full"), CaptureLevel::Full);
    }

    #[test]
    fn test_capture_level_from_str_default() {
        assert_eq!(CaptureLevel::parse(""), CaptureLevel::MetadataOnly);
        assert_eq!(CaptureLevel::parse("invalid"), CaptureLevel::MetadataOnly);
        assert_eq!(CaptureLevel::parse("metadata"), CaptureLevel::MetadataOnly);
    }

    #[test]
    fn test_capture_level_default() {
        assert_eq!(CaptureLevel::default(), CaptureLevel::MetadataOnly);
    }
}
