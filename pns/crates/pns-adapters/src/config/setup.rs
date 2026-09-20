use pns_domain::{Answers, hue_is_armed, router_is_armed};

/// One plugin table with its switch on.
fn armed(mut settings: toml::Table) -> toml::Value {
    settings.insert("enabled".to_string(), toml::Value::Boolean(true));
    toml::Value::Table(settings)
}

/// This walk's answers, as the values table `config_text::render` walks.
///
/// ONLY WHAT WAS ARMED IS HERE. A table this method never inserts is one
/// `render` writes at its layout default, commented for an opt-in table
/// and live at the CORE default for `daemon` and `recap`, neither of which
/// this wizard even asks about.
///
/// EVERY PLUGIN TABLE STATES ITS SWITCH, because `[plugins.*] enabled`
/// defaults false: a table written without it is a destination that renders
/// live and delivers nothing. The two this machine has by sitting at it,
/// `banner` and `phone`, are armed whether the walk asked anything about
/// them or not.
fn values(answers: &Answers) -> toml::Table {
    let mut plugins = toml::Table::new();
    plugins.insert("banner".to_string(), armed(toml::Table::new()));
    let mut mobile = toml::Table::new();
    if !answers.mobile_token.is_empty() {
        mobile.insert(
            "device_token".to_string(),
            toml::Value::String(answers.mobile_token.clone()),
        );
    }
    plugins.insert("phone".to_string(), armed(mobile));
    if !answers.hermes_key.is_empty() {
        // THE WALK'S ONE ANSWER IS THE DEFAULT ROUTE'S KEY, under its SHIPPED
        // name because the wizard does not ask about `[routes]`: every route
        // has its own key, and the default route is the one a machine
        // reaching this wizard has prepared; the rest are written by hand as
        // their routes are prepared.
        let mut keys = toml::Table::new();
        keys.insert(
            pns_domain::routes::Routes::default()
                .default_route()
                .to_string(),
            toml::Value::String(answers.hermes_key.clone()),
        );
        // UNDER THE DURABLE LOG'S OWN HEADING. The wizard asks only about
        // hermes; `type` is the layout's own default, written by the render.
        let mut log = toml::Table::new();
        log.insert("keys".to_string(), toml::Value::Table(keys));
        plugins.insert("log".to_string(), armed(log));
    }
    if hue_is_armed(answers) {
        let mut hue = toml::Table::new();
        hue.insert(
            "bridge_host".to_string(),
            toml::Value::String(answers.hue_bridge.clone()),
        );
        hue.insert(
            "api_key".to_string(),
            toml::Value::String(answers.hue_key.clone()),
        );
        hue.insert(
            "certificate".to_string(),
            toml::Value::String(answers.hue_certificate.clone()),
        );
        plugins.insert("lights".to_string(), armed(hue));
    }
    if router_is_armed(answers) {
        let mut router = toml::Table::new();
        router.insert(
            "type".to_string(),
            toml::Value::String(answers.router_type.clone()),
        );
        router.insert(
            "url".to_string(),
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
        plugins.insert("home_presence".to_string(), armed(router));
    }

    let mut values = toml::Table::new();
    values.insert("plugins".to_string(), toml::Value::Table(plugins));
    if !answers.focus_modes.is_empty() {
        let mut focus = toml::Table::new();
        focus.insert(
            "modes".to_string(),
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
    if answers.remind {
        // THE DELAY IS WRITTEN OUT. An empty `[remind]` states no setting and
        // renders exactly like no table at all, so a walk that armed the
        // reminder says what it armed it at.
        let mut remind = toml::Table::new();
        remind.insert(
            "delay".to_string(),
            toml::Value::String(
                crate::config::render::REMIND_DELAY
                    .trim_matches('"')
                    .to_string(),
            ),
        );
        values.insert("remind".to_string(), toml::Value::Table(remind));
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
