/// The agent name every uu record and alert carries.
pub const AGENT: &str = "uu";

/// uu's own gateway body. The four field NAMES are the hermes webhook's
/// contract; what goes in them is uu's.
///
/// BUILT BY THE JSON WRITER, never by interpolation. Every value in here is
/// text a third party wrote (a plugin name, a herdr error), and a quote in one
/// of them would otherwise end the field early.
pub fn record_body(state: &str, host: &str, detail: &str) -> String {
    serde_json::json!({
        "agent": AGENT,
        "state": state,
        "project": host,
        "detail": detail,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_body_carries_uus_own_four_fields_and_nothing_else() {
        let body = record_body("completed", "dresden", "the whole record");
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["agent"], "uu");
        assert_eq!(parsed["state"], "completed");
        assert_eq!(parsed["project"], "dresden");
        assert_eq!(parsed["detail"], "the whole record");
        assert_eq!(parsed.as_object().unwrap().len(), 4);
    }

    #[test]
    fn a_detail_holding_json_syntax_is_encoded_rather_than_glued_into_the_body() {
        let body = record_body("failed", "dresden", "plugin \"a\": {broken}\nnext");
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["detail"], "plugin \"a\": {broken}\nnext");
    }
}
