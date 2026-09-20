use crate::{Config, LoadOutcome, config_path, load_config, remote_deadline};
use std::time::Duration;

/// Every install-wide setting that used to be an environment variable alone,
/// resolved once.
///
/// THE FILE FIRST AND THE VARIABLE ONLY WHEN IT NAMES NOTHING, which is the
/// direction config already wins in: an install-wide value belongs in the file
/// the operator can read, and a variable exported in a shell profile must not
/// quietly outrank it. An EMPTY value on either side is nothing written at
/// all, so an exported-but-blank variable cannot shadow anything.
pub struct InstallSettings {
    /// `[paths] state_dir`, else `PNS_STATE_DIR`: where this binary keeps
    /// what it has to remember between runs.
    pub state_dir: Option<String>,
    /// `[paths] channels_dir`, else `PNS_CHANNELS_DIR`: the directory of
    /// channel executables. NAMING IT FORCES every channel onto its
    /// executable, which is what the variable has always meant.
    pub channels_dir: Option<String>,
    /// `[plugins.hermes] url`, else `PNS_HERMES_URL`: the gateway endpoint,
    /// used verbatim for every route rather than per route.
    pub hermes_url: Option<String>,
    /// `[plugins.mobile] url`, else `PNS_MOSHI_URL`: the push endpoint.
    pub moshi_url: Option<String>,
    /// `[plugins.banner] terminal_bundle_id`, else
    /// `PNS_TERMINAL_BUNDLE_ID`: the terminal a banner click returns to.
    pub terminal_bundle_id: Option<String>,
    /// `[delivery] remote_deadline`: how long one remote call may take.
    /// CONFIG ALONE, since this one had a duplicate variable rather than a
    /// home in the file, and the file is where it belongs.
    pub remote_deadline: Option<Duration>,
    /// `[storage] busy_deadline`: how long a writer waits for the state
    /// database's write lock. CONFIG ALONE, for the same reason: the bound
    /// used to come off a variable that called itself test-only and was read
    /// by production code on every connection.
    pub busy_deadline: Duration,
}

/// The settings for one home, off the config file it holds.
///
/// A MISSING OR REFUSED FILE LEAVES EVERY SETTING TO ITS VARIABLE. The refusal
/// itself is said by the composition root that loads the same file for
/// delivery, so saying it again here would be one fault printed twice.
pub fn install_settings(home: &str) -> InstallSettings {
    let config = match load_config(&config_path(home)) {
        Ok(LoadOutcome::Loaded(config)) => Some(config),
        _ => None,
    };
    install_settings_of(config.as_deref(), home)
}

/// The same settings off a config the caller has already loaded.
pub fn install_settings_of(config: Option<&Config>, home: &str) -> InstallSettings {
    resolve(config, home, &|variable| std::env::var(variable).ok())
}

fn resolve(
    config: Option<&Config>,
    home: &str,
    environment: &dyn Fn(&str) -> Option<String>,
) -> InstallSettings {
    let paths = config.map(|config| &config.paths);
    InstallSettings {
        state_dir: setting(
            paths.and_then(|paths| paths.state_dir.as_deref()),
            "PNS_STATE_DIR",
            environment,
        )
        .map(|path| against_home(path, home)),
        channels_dir: setting(
            paths.and_then(|paths| paths.channels_dir.as_deref()),
            "PNS_CHANNELS_DIR",
            environment,
        )
        .map(|path| against_home(path, home)),
        hermes_url: setting(
            plugin_setting(config, "hermes", "url"),
            "PNS_HERMES_URL",
            environment,
        ),
        moshi_url: setting(
            plugin_setting(config, "mobile", "url"),
            "PNS_MOSHI_URL",
            environment,
        ),
        terminal_bundle_id: setting(
            plugin_setting(config, "banner", "terminal_bundle_id"),
            "PNS_TERMINAL_BUNDLE_ID",
            environment,
        ),
        remote_deadline: remote_deadline(
            config.map_or(Config::default().remote_deadline_secs, |config| {
                config.remote_deadline_secs
            }),
        ),
        busy_deadline: config.map_or(crate::config::DEFAULT_BUSY_DEADLINE, |config| {
            config.storage_busy_deadline
        }),
    }
}

/// One setting: what the file named, else what the variable named.
fn setting(
    configured: Option<&str>,
    variable: &str,
    environment: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    match configured.filter(|value| !value.is_empty()) {
        Some(value) => Some(value.to_string()),
        None => environment(variable).filter(|value| !value.is_empty()),
    }
}

/// One setting off a plugin's own table, whether or not the table is armed:
/// an endpoint is where the plugin points when it runs, not a second switch.
fn plugin_setting<'a>(config: Option<&'a Config>, plugin: &str, key: &str) -> Option<&'a str> {
    config?.plugins.get(plugin)?.settings.get(key)?.as_str()
}

/// A `~/` path against this home, the way `[phone] marker_file` is already
/// read. A path written any other way is used as it stands.
fn against_home(path: String, home: &str) -> String {
    match path.strip_prefix("~/") {
        Some(tail) if !home.is_empty() => format!("{home}/{tail}"),
        _ => path,
    }
}

#[cfg(test)]
#[path = "install/tests.rs"]
mod tests;
