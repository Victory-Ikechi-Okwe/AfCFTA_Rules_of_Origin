pub fn store(d: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    Some(uuid::Uuid::new_v4().hyphenated().to_string())
}
