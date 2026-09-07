pub fn is_nvim_foreground(process_info_json: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(process_info_json)
        .ok()
        .and_then(|v| {
            v["result"]["process_info"]["foreground_processes"]
                .as_array()
                .map(|procs| procs.iter().any(|p| p["name"].as_str() == Some("nvim")))
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_nvim_foreground_true_when_present() {
        let j = r#"{"result":{"process_info":{"foreground_processes":[{"name":"bash"},{"name":"nvim"}]}}}"#;
        assert!(is_nvim_foreground(j));
    }

    #[test]
    fn is_nvim_foreground_false_when_absent() {
        let j = r#"{"result":{"process_info":{"foreground_processes":[{"name":"bash"}]}}}"#;
        assert!(!is_nvim_foreground(j));
    }

    #[test]
    fn is_nvim_foreground_false_on_garbage() {
        assert!(!is_nvim_foreground("not json"));
        assert!(!is_nvim_foreground("{}"));
    }
}
