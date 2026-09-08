use pns_domain::{Answers, hue_is_armed, router_is_armed};

/// This walk's answers, as the values table `config_text::render` walks.
///
/// ONLY WHAT WAS ARMED IS HERE. A table this method never inserts is one
/// `render` writes at its layout default, commented for an opt-in table
/// and live at the CORE default for `mobile`, `macos-banner`, `daemon`
/// and `recap`, none of which this wizard even asks about.
fn values(answers: &Answers) -> toml::Table {
    let mut plugins = toml::Table::new();
    if !answers.mobile_token.is_empty() {
        let mut mobile = toml::Table::new();
        mobile.insert(
            "token".to_string(),
            toml::Value::String(answers.mobile_token.clone()),
        );
        plugins.insert("mobile".to_string(), toml::Value::Table(mobile));
    }
    if !answers.hermes_key.is_empty() {
        let mut hermes = toml::Table::new();
        hermes.insert(
            "key".to_string(),
            toml::Value::String(answers.hermes_key.clone()),
        );
        plugins.insert("hermes".to_string(), toml::Value::Table(hermes));
    }
    if hue_is_armed(answers) {
        let mut hue = toml::Table::new();
        hue.insert(
            "bridge".to_string(),
            toml::Value::String(answers.hue_bridge.clone()),
        );
        hue.insert(
            "key".to_string(),
            toml::Value::String(answers.hue_key.clone()),
        );
        hue.insert(
            "rooms".to_string(),
            toml::Value::Array(
                answers
                    .hue_rooms
                    .iter()
                    .cloned()
                    .map(toml::Value::String)
                    .collect(),
            ),
        );
        plugins.insert("hue".to_string(), toml::Value::Table(hue));
    }
    if router_is_armed(answers) {
        let mut router = toml::Table::new();
        router.insert(
            "type".to_string(),
            toml::Value::String(answers.router_type.clone()),
        );
        router.insert(
            "router_url".to_string(),
            toml::Value::String(answers.router_url.clone()),
        );
        router.insert(
            "api_key".to_string(),
            toml::Value::String(answers.router_api_key.clone()),
        );
        router.insert(
            "device_hostname".to_string(),
            toml::Value::String(answers.router_device_hostname.clone()),
        );
        plugins.insert("router".to_string(), toml::Value::Table(router));
    }

    let mut values = toml::Table::new();
    if !plugins.is_empty() {
        values.insert("plugins".to_string(), toml::Value::Table(plugins));
    }
    if !answers.focus_modes.is_empty() {
        let mut focus = toml::Table::new();
        focus.insert(
            "silence".to_string(),
            toml::Value::Array(
                answers
                    .focus_modes
                    .iter()
                    .cloned()
                    .map(toml::Value::String)
                    .collect(),
            ),
        );
        values.insert("focus".to_string(), toml::Value::Table(focus));
    }
    if answers.nag {
        values.insert("nag".to_string(), toml::Value::Table(toml::Table::new()));
    }
    values
}
/// The whole config file, composed from one walk's answers.
///
/// EVERY DEFAULT IT RELIES ON IS WRITTEN OUT, because a loaded config is
/// authoritative and an absent `enabled` reads false: a wizard that left the
/// core implicit would hand a fresh machine a file that turns the banner and
/// the card off. A declined feature is present too, as a commented block with
/// empty values, so the file says what it could carry as well as what it does.
///
/// `crate::config_text::render` NEVER REFUSES A WALK'S OWN ANSWERS: every
/// value this method's `values()` composes is a plain literal off the roster
/// this wizard's own layout serves, so the only way this expect fires is a
/// bug in `values()` itself, not an operator's input.
pub fn compose_config(answers: &Answers) -> String {
    super::render(&values(answers)).expect("a wizard's own answers always render")
}

pub struct SetupRenderer;
impl pns_application::ConfigRenderer for SetupRenderer {
    fn compose(&self, answers: &Answers) -> String {
        compose_config(answers)
    }
    fn validate(&self, composed: &str) -> Result<(), String> {
        super::parse_config(composed)
            .map(|_| ())
            .map_err(|error| error.detail().to_string())
    }
}

#[cfg(test)]
mod tests;
