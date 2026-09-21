use super::*;
use pns_domain::profiles::{Admits, Profile, Rule};

/// One `[profiles.<name>]` table.
///
/// AN UNWRITTEN KEY IS TODAY'S BEHAVIOUR, never off: a profile that named only
/// its hush still talks on every surface, which is the direction a mistyped
/// table has to fail in.
pub(super) fn parse_profile(name: &str, table: &toml::Table) -> Result<Profile, ConfigError> {
    let shown = format!("profiles.{name}");
    let mut profile = Profile::default();
    for (key, setting) in table {
        match key.as_str() {
            "quiet" => {
                profile.quiet = setting.as_bool().ok_or_else(|| {
                    ConfigError::Invalid(format!(
                        "`{shown}` key `quiet` has type `{}`, not boolean",
                        setting.type_str()
                    ))
                })?;
            }
            "banner" => profile.banner = surface(&shown, key, setting)?,
            "discord" => profile.discord = surface(&shown, key, setting)?,
            "phone" => profile.phone = surface(&shown, key, setting)?,
            "lights" => profile.lights = surface(&shown, key, setting)?,
            _ => return Err(unknown_key(PROFILE_KEYS, &shown, key)),
        }
    }
    // THE FLOOR, refused here rather than at delivery: see `admits_a_page`.
    if !profile.admits_a_page() {
        return Err(ConfigError::Invalid(format!(
            "profile `{name}`'s `discord` is \"none\"; a priority page has to reach the durable \
             log wherever you are, so `discord` must be \"all\" or \"priority\""
        )));
    }
    Ok(profile)
}

fn surface(shown: &str, key: &str, setting: &toml::Value) -> Result<Admits, ConfigError> {
    setting.as_str().and_then(Admits::parse).ok_or_else(|| {
        ConfigError::Invalid(format!(
            "unknown `{shown}` value for `{key}`; a surface is \"all\", \"priority\" or \"none\""
        ))
    })
}

/// `[profiles]` as a whole: the named profiles, the named networks, the
/// ordered rules and the one knob.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Profiles {
    pub profiles: BTreeMap<String, Profile>,
    pub locations: BTreeMap<String, String>,
    pub rules: Vec<Rule>,
    pub location_poll_secs: u64,
}

/// THIRTY SECONDS. A network change is observed within one poll, and a
/// `route` plus an `arp` twice a minute is two spawns nobody notices.
pub(super) const DEFAULT_LOCATION_POLL_SECS: u64 = 30;
const MIN_LOCATION_POLL_SECS: u64 = 5;
/// FIVE MINUTES at the top: past it a laptop can be on a new network for
/// longer than a meeting before the profile follows it.
const MAX_LOCATION_POLL_SECS: u64 = 300;

/// The seven weekday words, in `libc`'s own order, so the index IS `tm_wday`.
const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

pub(super) fn parse_profiles(value: toml::Value) -> Result<Profiles, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid(
            "`profiles` is not a table".to_string(),
        ));
    };
    let mut read = Profiles {
        location_poll_secs: DEFAULT_LOCATION_POLL_SECS,
        ..Profiles::default()
    };
    let mut rules = None;
    for (key, setting) in table {
        match key.as_str() {
            "location_poll" => {
                read.location_poll_secs = nonzero_duration_key(
                    PROFILES,
                    "location_poll",
                    &setting,
                    Duration::from_secs(MIN_LOCATION_POLL_SECS)
                        ..=Duration::from_secs(MAX_LOCATION_POLL_SECS),
                )?;
            }
            "locations" => read.locations = parse_locations(&setting)?,
            "rules" => rules = Some(setting),
            // EVERY OTHER TABLE IS A PROFILE NAME, which is what makes the
            // heading open: the names are the operator's own. A key that is
            // not a table names no profile, so it is refused against the
            // three keys this heading serves itself.
            name => {
                let toml::Value::Table(settings) = &setting else {
                    return Err(unknown_key(PROFILES, PROFILES, name));
                };
                read.profiles
                    .insert(name.to_string(), parse_profile(name, settings)?);
            }
        }
    }
    if let Some(rules) = rules {
        read.rules = parse_rules(&rules, &read.profiles)?;
    }
    Ok(read)
}

fn parse_locations(value: &toml::Value) -> Result<BTreeMap<String, String>, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid(
            "`profiles.locations` is not a table".to_string(),
        ));
    };
    let mut locations = BTreeMap::new();
    for (name, fingerprint) in table {
        let text = fingerprint.as_str().ok_or_else(|| {
            ConfigError::Invalid(format!(
                "`profiles.locations` key `{name}` has type `{}`, not a string",
                fingerprint.type_str()
            ))
        })?;
        locations.insert(name.clone(), text.to_string());
    }
    Ok(locations)
}

fn parse_rules(
    value: &toml::Value,
    profiles: &BTreeMap<String, Profile>,
) -> Result<Vec<Rule>, ConfigError> {
    let toml::Value::Array(rows) = value else {
        return Err(ConfigError::Invalid(
            "`profiles.rules` is not a list of rules; write `[[profiles.rules]]`".to_string(),
        ));
    };
    let mut rules = Vec::with_capacity(rows.len());
    for (offset, row) in rows.iter().enumerate() {
        rules.push(parse_rule(offset + 1, row, profiles)?);
    }
    Ok(rules)
}

/// One rule, whose refusals count from ONE: the operator reading a refusal is
/// counting rows in their own file, not indexing an array.
fn parse_rule(
    index: usize,
    value: &toml::Value,
    profiles: &BTreeMap<String, Profile>,
) -> Result<Rule, ConfigError> {
    let toml::Value::Table(table) = value else {
        return Err(ConfigError::Invalid(format!("rule {index} is not a table")));
    };
    let mut rule = Rule::default();
    for (key, setting) in table {
        match key.as_str() {
            "profile" => rule.profile = rule_text(index, "profile", setting)?,
            "location" => rule.location = Some(rule_text(index, "location", setting)?),
            "focus" => rule.focus = Some(rule_text(index, "focus", setting)?),
            "hours" => {
                let stated = rule_text(index, "hours", setting)?;
                rule.hours = Some(pns_domain::lamps::parse_window(&stated).ok_or_else(|| {
                    ConfigError::Invalid(format!(
                        "rule {index} has hours {stated:?}, which is not a HH:MM-HH:MM window"
                    ))
                })?);
            }
            "days" => rule.days = parse_days(index, setting)?,
            "calendar_busy" => {
                rule.calendar_busy = Some(setting.as_bool().ok_or_else(|| {
                    ConfigError::Invalid(format!("rule {index} has a non-boolean `calendar_busy`"))
                })?);
            }
            other => {
                return Err(ConfigError::Invalid(format!(
                    "unknown rule key `{other}` in rule {index}; a rule serves calendar_busy, \
                     days, focus, hours, location, profile"
                )));
            }
        }
    }
    if rule.profile.is_empty() {
        return Err(ConfigError::Invalid(format!(
            "rule {index} names no profile"
        )));
    }
    if !profiles.contains_key(&rule.profile) {
        return Err(ConfigError::Invalid(format!(
            "rule {index} names profile `{}`, which no `[profiles.{}]` table defines",
            rule.profile, rule.profile
        )));
    }
    Ok(rule)
}

fn rule_text(index: usize, key: &str, setting: &toml::Value) -> Result<String, ConfigError> {
    setting.as_str().map(str::to_string).ok_or_else(|| {
        ConfigError::Invalid(format!(
            "rule {index} has a `{key}` of type `{}`, not a string",
            setting.type_str()
        ))
    })
}

fn parse_days(index: usize, setting: &toml::Value) -> Result<Vec<u32>, ConfigError> {
    let toml::Value::Array(written) = setting else {
        return Err(ConfigError::Invalid(format!(
            "rule {index} has a `days` that is not a list of weekday names"
        )));
    };
    let mut days = Vec::with_capacity(written.len());
    for day in written {
        let stated = rule_text(index, "days", day)?;
        let found = DAYS
            .iter()
            .position(|known| known.eq_ignore_ascii_case(&stated))
            .ok_or_else(|| {
                ConfigError::Invalid(format!(
                    "rule {index} has day {stated:?}; a day is Mon, Tue, Wed, Thu, Fri, Sat or Sun"
                ))
            })?;
        days.push(u32::try_from(found).unwrap_or_default());
    }
    Ok(days)
}

#[cfg(test)]
mod tests;
