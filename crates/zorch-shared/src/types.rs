newtype_uuid!(RequestId);
newtype_uuid!(OrgId);
newtype_uuid!(ApiKeyId);

newtype_string!(ProviderId);
newtype_string!(ModelId);
newtype_string!(VirtualModelId);

newtype_numeric!(TokenCount, u64);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyTag {
    pub key: String,
    pub value: String,
}

pub fn validate_tags(tags: &[ApiKeyTag]) -> Result<Vec<ApiKeyTag>, String> {
    if tags.len() > 16 {
        return Err(format!("Too many tags: {} (max 16)", tags.len()));
    }
    let mut seen_keys = std::collections::HashSet::new();
    for tag in tags {
        if tag.key.is_empty() {
            return Err("Tag key must not be empty".to_string());
        }
        if tag.key.len() > 32 {
            return Err(format!("Tag key too long: '{}' (max 32 chars)", tag.key));
        }
        if !tag.key.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-') {
            return Err(format!("Tag key contains invalid characters: '{}' (only lowercase a-z, 0-9, _, -)", tag.key));
        }
        if tag.value.is_empty() {
            return Err(format!("Tag value must not be empty for key '{}'", tag.key));
        }
        if tag.value.len() > 128 {
            return Err(format!("Tag value too long for key '{}' (max 128 chars)", tag.key));
        }
        if !seen_keys.insert(tag.key.clone()) {
            return Err(format!("Duplicate tag key: '{}'", tag.key));
        }
    }
    Ok(tags.to_vec())
}

impl TokenCount {
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_id_generation() {
        let id = RequestId::new();
        assert!(!id.to_string().is_empty());
    }

    #[test]
    fn test_request_id_from_uuid() {
        let uuid = ::uuid::Uuid::new_v4();
        let id = RequestId::from_uuid(uuid);
        assert_eq!(*id, uuid);
    }

    #[test]
    fn test_request_id_display() {
        let uuid = ::uuid::Uuid::new_v4();
        let id = RequestId::from_uuid(uuid);
        assert_eq!(id.to_string(), uuid.to_string());
    }

    #[test]
    fn test_request_id_deref() {
        let uuid = ::uuid::Uuid::new_v4();
        let id = RequestId::from_uuid(uuid);
        assert_eq!(id.as_bytes(), uuid.as_bytes());
    }

    #[test]
    fn test_org_id_generation() {
        let id = OrgId::new();
        assert!(!id.to_string().is_empty());
    }

    #[test]
    fn test_org_id_from_uuid() {
        let uuid = ::uuid::Uuid::new_v4();
        let id = OrgId::from_uuid(uuid);
        assert_eq!(*id, uuid);
    }

    #[test]
    fn test_api_key_id_generation() {
        let id = ApiKeyId::new();
        assert!(!id.to_string().is_empty());
    }

    #[test]
    fn test_api_key_id_from_uuid() {
        let uuid = ::uuid::Uuid::new_v4();
        let id = ApiKeyId::from_uuid(uuid);
        assert_eq!(*id, uuid);
    }

    #[test]
    fn test_provider_id_from_string() {
        let id = ProviderId::from("openai");
        assert_eq!(id.to_string(), "openai");
    }

    #[test]
    fn test_provider_id_from_str() {
        let id = ProviderId::from("anthropic");
        assert_eq!(id.to_string(), "anthropic");
    }

    #[test]
    fn test_provider_id_deref() {
        let id = ProviderId::from("google");
        assert_eq!(id.as_str(), "google");
    }

    #[test]
    fn test_model_id_from_string() {
        let id = ModelId::from("gpt-4");
        assert_eq!(id.to_string(), "gpt-4");
    }

    #[test]
    fn test_model_id_deref() {
        let id = ModelId::from("claude-3");
        assert_eq!(id.as_str(), "claude-3");
    }

    #[test]
    fn test_token_count_new() {
        let tc = TokenCount::new(100);
        assert_eq!(tc.as_u64(), 100);
    }

    #[test]
    fn test_token_count_from_u64() {
        let tc = TokenCount::from(250u64);
        assert_eq!(*tc, 250);
    }

    #[test]
    fn test_token_count_display() {
        let tc = TokenCount::new(500);
        assert_eq!(tc.to_string(), "500");
    }

    #[test]
    fn test_token_count_deref() {
        let tc = TokenCount::new(750);
        assert_eq!(*tc, 750);
    }

    #[test]
    fn test_virtual_model_id_from_string() {
        let id = VirtualModelId::from("fast-model");
        assert_eq!(id.to_string(), "fast-model");
    }

    #[test]
    fn test_virtual_model_id_deref() {
        let id = VirtualModelId::from("cheap-model");
        assert_eq!(id.as_str(), "cheap-model");
    }

    #[test]
    fn test_types_serialize() {
        let request_id = RequestId::new();
        let json = serde_json::to_string(&request_id).unwrap();
        assert!(!json.is_empty());
    }

    #[test]
    fn test_types_deserialize() {
        let uuid = ::uuid::Uuid::new_v4();
        let json = serde_json::to_string(&uuid).unwrap();
        let request_id: RequestId = serde_json::from_str(&json).unwrap();
        assert_eq!(*request_id, uuid);
    }
}
