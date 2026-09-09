#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatWindow {
    text: String,
    seconds: u64,
    invalid_literal: bool,
}
impl HeartbeatWindow {
    pub fn from_override(input: Option<&str>) -> Self {
        let default = || Self {
            text: "1800".into(),
            seconds: 1800,
            invalid_literal: false,
        };
        let Some(text) =
            input.filter(|text| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()))
        else {
            return default();
        };
        let radix = if text.starts_with('0') { 8 } else { 10 };
        match u64::from_str_radix(text, radix) {
            Ok(seconds) => Self {
                text: text.into(),
                seconds,
                invalid_literal: false,
            },
            Err(_) => Self {
                invalid_literal: true,
                ..default()
            },
        }
    }
    pub fn seconds(&self) -> u64 {
        self.seconds
    }
    pub fn display(&self) -> &str {
        &self.text
    }
    pub fn invalid_literal(&self) -> bool {
        self.invalid_literal
    }
}
#[cfg(test)]
mod tests;
