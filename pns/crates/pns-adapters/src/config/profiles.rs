use super::*;
use pns_domain::profiles::{Admits, Profile};

/// One `[profiles.<name>]` table.
///
/// AN UNWRITTEN KEY IS TODAY'S BEHAVIOUR, never off: a profile that named only
/// its hush still talks on every surface, which is the direction a mistyped
/// table has to fail in.
pub(super) fn parse_profile(name: &str, table: &toml::Table) -> Result<Profile, ConfigError> {
    let shown = format!("profiles.{name}");
    let mut profile = Profile::default();
    for (key, setting) in table {
        admits(PROFILE_KEYS, &shown, key)?;
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

#[cfg(test)]
mod tests;
