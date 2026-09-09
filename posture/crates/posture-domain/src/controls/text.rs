// The poller has its own captured 160-character rule. Funnel text uses a different rule.
pub(crate) fn sanitize_control(value: &str) -> String {
    value
        .chars()
        .filter_map(|c| match c {
            '\\' | '`' | '$' | '\'' | '"' => None,
            '\r' | '\n' | '\t' => Some(' '),
            _ => Some(c),
        })
        .take(160)
        .collect()
}

pub(crate) fn control_span(value: &str) -> String {
    format!("`{}`", sanitize_control(value))
}
