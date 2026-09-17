use pns_domain::CertificatePin;
use pns_domain::lamps::{QuietWindow, parse_window};

/// The rooms the bash pulsed when `HUE_PULSE_ROOMS` said nothing.
pub const DEFAULT_ROOMS: &[&str] = &["3F - Studio", "2F - Kitchen"];

/// Everything the pulse needs from the config, or None for the not-set-up
/// silence: a bridge and key are required, rooms come from the environment
/// override (newline-separated, room names carry spaces), else the settings
/// array, else the defaults.
#[derive(Debug, PartialEq)]
pub struct HueSettings {
    pub bridge: String,
    pub key: String,
    pub rooms: Vec<String>,
    /// The one certificate the bridge may present. REQUIRED, which is why it
    /// is a value here rather than an option: the bridge's certificate carries
    /// no name a verifier can check, so its fingerprint is the whole of what
    /// makes the address the operator typed the device they meant.
    pub certificate: CertificatePin,
}

/// The settings, the not-set-up silence, or the refusal that names the key.
///
/// THREE ANSWERS RATHER THAN TWO, and the third is the fail-closed pin. A
/// bridge and key with no `certificate` is a table somebody armed and did not
/// finish, so it refuses by name in the `quiet_hours` grammar instead of
/// pulsing through a connection that verifies nothing. A table with no bridge
/// and key at all is still the silence it always was.
pub fn hue_settings(
    settings: &toml::Table,
    rooms_env: Option<&str>,
) -> Result<Option<HueSettings>, String> {
    let text = |key: &str| -> Option<String> {
        settings
            .get(key)?
            .as_str()
            .filter(|value| !value.is_empty())
            .map(String::from)
    };
    let from_env: Vec<String> = rooms_env
        .unwrap_or_default()
        .lines()
        .filter(|room| !room.is_empty())
        .map(String::from)
        .collect();
    let (Some(bridge), Some(key)) = (text("bridge"), text("key")) else {
        return Ok(None);
    };
    let Some(stated) = text("certificate") else {
        return Err(certificate_refusal("unset"));
    };
    let certificate = CertificatePin::parse(&stated).map_err(|why| certificate_refusal(&why))?;
    Ok(Some(HueSettings {
        bridge,
        key,
        certificate,
        rooms: if from_env.is_empty() {
            settings
                .get("rooms")
                .and_then(|rooms| rooms.as_array())
                .map(|rooms| {
                    rooms
                        .iter()
                        .filter_map(|room| room.as_str())
                        .map(String::from)
                        .collect::<Vec<_>>()
                })
                .filter(|rooms| !rooms.is_empty())
                .unwrap_or_else(|| DEFAULT_ROOMS.iter().map(|room| room.to_string()).collect())
        } else {
            from_env
        },
    }))
}

/// Why a pulse is refused for want of a pin, with the command that produces
/// one. NAMED IN THE REFUSAL rather than left to a runbook: the operator
/// reading this line is at a terminal and one command away from the value.
fn certificate_refusal(why: &str) -> String {
    format!(
        "pns: config error (plugins.hue.certificate is {why}); run `pns lights enroll` \
and paste the line it prints; no pulse"
    )
}

/// The settings, with the refusal already complained about.
///
/// EVERY LAMP CALLER WANTS THE SAME TWO THINGS: the settings when there are
/// any, and the refusal said out loud once when there are not. Written here
/// rather than seven times, so a refusal cannot reach one path and be
/// swallowed on another.
pub fn armed_hue(
    settings: &toml::Table,
    rooms_env: Option<&str>,
    complain: impl FnOnce(&str),
) -> Option<HueSettings> {
    match hue_settings(settings, rooms_env) {
        Ok(hue) => hue,
        Err(refusal) => {
            complain(&refusal);
            None
        }
    }
}

/// The window the operator configured, or None for no window at all.
///
/// A value that is not a `HH:MM-HH:MM` string is a REFUSAL rather than a
/// silent no-window: an operator who asked for quiet hours and mistyped them
/// would otherwise be flashed at 3am and told nothing.
pub fn quiet_window(settings: &toml::Table) -> Result<Option<QuietWindow>, String> {
    let Some(stated) = settings.get("quiet_hours") else {
        return Ok(None);
    };
    let Some(text) = stated.as_str() else {
        return Err(quiet_hours_refusal(stated.type_str()));
    };
    // EMPTY IS ABSENT, the rule the bridge and key beside it already follow.
    if text.is_empty() {
        return Ok(None);
    }
    parse_window(text)
        .map(Some)
        .ok_or_else(|| quiet_hours_refusal(&format!("{text:?}")))
}

/// The refusal, in the shape the config layer already refuses a setting by
/// name: what was written, and what it cost.
fn quiet_hours_refusal(offender: &str) -> String {
    format!("pns: config error (hue.quiet_hours is {offender}, not a HH:MM-HH:MM window); no pulse")
}
