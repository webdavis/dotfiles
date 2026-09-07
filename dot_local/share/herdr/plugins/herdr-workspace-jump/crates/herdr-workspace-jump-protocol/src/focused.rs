pub fn parse_focused_id(event_json: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(event_json)
        .ok()
        .and_then(|event| event["data"]["workspace_id"].as_str().map(str::to_string))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_focused_id_reads_the_workspace_id_and_survives_garbage() {
        let event = r#"{"event":"workspace_focused","data":{"workspace_id":"w18"}}"#;
        assert_eq!(parse_focused_id(event), Some("w18".to_string()));
        assert_eq!(parse_focused_id("not json"), None);
        assert_eq!(parse_focused_id("{}"), None);
    }
}
