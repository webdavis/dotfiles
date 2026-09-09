use super::{Answers, compose_config};
use crate::config::{DEFAULT_SUBMIT_DEADLINE_SECS, Recap, parse_config};

/// Every table a walk can decline, spelled as a heading standing at the
/// head of a line: what the two ends of the walk are checked for.
const DECLINABLE_TABLES: [&str; 5] = [
    "[plugins.hermes]",
    "[plugins.hue]",
    "[plugins.router]",
    "[focus]",
    "[nag]",
];

/// A walk that armed everything it was offered.
fn every_feature_armed() -> Answers {
    Answers {
        mobile_token: "moshi-secret".to_string(),
        hermes_key: "hermes-secret".to_string(),
        hue_bridge: "192.168.1.9".to_string(),
        hue_key: "hue-secret".to_string(),
        hue_rooms: vec!["Studio".to_string(), "Kitchen".to_string()],
        router_type: "unifi".to_string(),
        router_url: "https://192.168.1.1".to_string(),
        router_api_key: "router-secret".to_string(),
        router_device_hostname: "phone".to_string(),
        focus_modes: vec!["Sleep".to_string()],
        nag: true,
    }
}

/// The config the text composes to, or the refusal as a panic naming it.
fn parsed(text: &str) -> crate::config::Config {
    parse_config(text).unwrap_or_else(|error| panic!("it must load: {error:?}\n{text}"))
}

mod defaults;
mod features;
mod rendering;
