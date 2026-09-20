use pns_domain::CertificatePin;

/// The rooms the plain pulse flashes. THE ONLY STATEMENT OF THEM, since the
/// pulse without a `[lights]` map is the bridge-and-key check rather than a
/// routing feature: which lamp says what is `[lights]`'s question.
pub const DEFAULT_ROOMS: &[&str] = &["3F - Studio", "2F - Kitchen"];

/// Everything the pulse needs from the config, or None for the not-set-up
/// silence: a bridge, a key and the certificate they answer behind.
#[derive(Debug, PartialEq)]
pub struct HueSettings {
    pub bridge: String,
    pub key: String,
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
/// finish, so it refuses by name instead of pulsing through a connection that
/// verifies nothing. A table with no bridge and key at all is still the silence
/// it always was.
pub fn hue_settings(settings: &toml::Table) -> Result<Option<HueSettings>, String> {
    let text = |key: &str| -> Option<String> {
        settings
            .get(key)?
            .as_str()
            .filter(|value| !value.is_empty())
            .map(String::from)
    };
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
    }))
}

/// Why a pulse is refused for want of a pin, with the command that produces
/// one. NAMED IN THE REFUSAL rather than left to a runbook: the operator
/// reading this line is at a terminal and one command away from the value.
fn certificate_refusal(why: &str) -> String {
    format!(
        "pns: config error (plugins.lights.certificate is {why}); run `pns lights enroll` \
and paste the line it prints; no pulse"
    )
}

/// The settings, with the refusal already complained about.
///
/// EVERY LAMP CALLER WANTS THE SAME TWO THINGS: the settings when there are
/// any, and the refusal said out loud once when there are not. Written here
/// rather than seven times, so a refusal cannot reach one path and be
/// swallowed on another.
pub fn armed_hue(settings: &toml::Table, complain: impl FnOnce(&str)) -> Option<HueSettings> {
    match hue_settings(settings) {
        Ok(hue) => hue,
        Err(refusal) => {
            complain(&refusal);
            None
        }
    }
}
