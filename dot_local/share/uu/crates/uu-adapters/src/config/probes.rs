use super::{Config, ConfigError};
use crate::{
    BrewLane, ClaudePluginsLane, CommandLane, HerdrLane, LaneAdapter, LaneRegistration, NpmLane,
    NvimMasonLane, NvimParsersLane, NvimPluginsLane, NvimSmokeTestLane, UvLane,
};

pub(crate) const REGISTRATIONS: &[LaneRegistration] = &[
    LaneRegistration::new::<BrewLane>("brew"),
    LaneRegistration::new::<ClaudePluginsLane>("claude-plugins"),
    LaneRegistration::new::<CommandLane>("command"),
    LaneRegistration::new::<HerdrLane>("herdr"),
    LaneRegistration::new::<NpmLane>("npm"),
    LaneRegistration::new::<NvimMasonLane>("nvim-mason"),
    LaneRegistration::new::<NvimParsersLane>("nvim-parsers"),
    LaneRegistration::new::<NvimPluginsLane>("nvim-plugins"),
    LaneRegistration::new::<NvimSmokeTestLane>("nvim-smoke-test"),
    LaneRegistration::new::<UvLane>("uv"),
];

pub(crate) fn parse_config(text: &str) -> Result<Config, ConfigError> {
    super::parse_config(text, REGISTRATIONS)
}

pub(crate) fn parsed(text: &str) -> Config {
    parse_config(text).expect("this config is valid")
}

pub(crate) fn refusal(text: &str) -> String {
    match parse_config(text) {
        Err(error) => error.detail().to_string(),
        Ok(config) => panic!("this config should have been refused, got {config:?}"),
    }
}

pub(crate) fn checked_text(text: &str) -> &str {
    parsed(text);
    text
}

// Assert typed fields through their owning parser after the shared load path accepts the block.
// Registration execution tests separately prove which typed parser composition selects.
pub(crate) fn typed<T: LaneAdapter>(text: &str, name: &str) -> Option<T> {
    let document: toml::Table = text.parse().expect("fixture document");
    let fields = document
        .get("lanes")?
        .as_table()?
        .get(name)?
        .as_table()?
        .clone();
    Some(T::parse(&format!("lanes.{name}"), fields).expect("typed parser fixture"))
}
