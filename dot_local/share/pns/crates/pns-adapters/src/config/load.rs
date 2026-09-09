use super::*;

/// Where the config lives for a given home directory. Pure, so the path rule
/// is testable without an environment.
pub fn config_path(home: &str) -> PathBuf {
    Path::new(home).join(".config/pns/config.toml")
}

/// The pure half: text in, config or a named refusal out.
pub fn parse_config(text: &str) -> Result<Config, ConfigError> {
    // The parser's Display echoes the offending source line, and this file
    // carries plugin secrets into log lines, so the refusal is rebuilt from
    // the cause and the location alone.
    let document: toml::Table = text.parse().map_err(|error: toml::de::Error| {
        let line = error
            .span()
            .map(|span| text[..span.start].matches('\n').count() + 1);
        ConfigError::Malformed(match line {
            Some(line) => format!("{} at line {line}", error.message()),
            None => error.message().to_string(),
        })
    })?;

    let mut config = Config::default();
    // The arm below is the whole schema at
    // this level, and everything outside it is still refused
    // BY NAME, so a retired table and a plural typo both say what they are.
    for (key, value) in document {
        match key.as_str() {
            "recap" => config.recap = parse_recap(value)?,
            "focus" => config.focus_silence = parse_focus(value)?,
            "daemon" => config.daemon_enabled = parse_daemon(value)?,
            "delivery" => {
                let toml::Value::Table(mut table) = value else {
                    return Err(ConfigError::Invalid("`delivery` is not a table".into()));
                };
                config.retry_limits = retry::parse_retry(&mut table)?;
                config.bypass_silence_classes = parse_delivery(toml::Value::Table(table))?;
            }
            "nag" => config.nag_after_secs = parse_nag(value)?,
            "lights" => config.lights = Some(Box::new(parse_lights(value)?)),
            "plugins" => {
                let toml::Value::Table(plugins) = value else {
                    return Err(ConfigError::Invalid("`plugins` is not a table".to_string()));
                };

                for (name, entry) in plugins {
                    let toml::Value::Table(mut settings) = entry else {
                        return Err(ConfigError::Invalid(format!(
                            "plugin `{name}` is not a table"
                        )));
                    };
                    // `enabled` is removed rather than read, so the flag
                    // reaches this layer and everything left over reaches the
                    // plugin untouched.
                    let enabled = match settings.remove("enabled") {
                        None => false,
                        Some(toml::Value::Boolean(flag)) => flag,
                        Some(_) => {
                            return Err(ConfigError::Invalid(format!(
                                "plugin `{name}` has a non-boolean `enabled`"
                            )));
                        }
                    };
                    // AND THE SETTINGS ARE JUDGED for a plugin that ships,
                    // because a near miss there is a destination that quietly
                    // never works. `enabled` is already out of the table and
                    // still listed, since it is a key the operator writes.
                    let table = format!("plugins.{name}");
                    for key in settings.keys() {
                        admits_flat(&table, key)?;
                    }
                    config
                        .plugins
                        .insert(name, PluginEntry { enabled, settings });
                }
            }
            _ => {
                // The admitted keys are listed off the roster's top-level row.
                // This is the most operator-visible typo class there is (a
                // whole table misspelled, or a table that MOVED, which refuses
                // the file whole and takes every plugin's secret with it), and
                // it was the last refusal in this file that named no
                // alternatives.
                return Err(ConfigError::Invalid(format!(
                    "unknown top-level key `{key}`; the file serves {}",
                    keys_of(TOP_LEVEL).unwrap_or_default().join(", ")
                )));
            }
        }
    }
    backstop_outlasts_the_nag(&config)?;
    Ok(config)
}

/// The IO edge: read the file at `path` and hand its text to the parser.
pub fn load_config(path: &Path) -> Result<LoadOutcome, ConfigError> {
    match read_config_text(path) {
        Ok(text) => parse_config(&text).map(Box::new).map(LoadOutcome::Loaded),
        // A dangling symlink also reads NotFound, and chezmoi deploys configs
        // as symlinks: the entry is PRESENT with a wrong target, so only an
        // absent entry is Missing and the broken link is an error.
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                && std::fs::symlink_metadata(path).is_err() =>
        {
            Ok(LoadOutcome::Missing)
        }
        Err(error) => Err(ConfigError::Unreadable(format!(
            "{}: {error}",
            path.display()
        ))),
    }
}

pub(super) fn read_config_text(path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;

    // Open before checking the descriptor, so replacing the path cannot
    // turn the subsequent read into a wait on a pipe.
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "the config path is not a regular file",
        ));
    }
    let mut text = String::new();
    file.read_to_string(&mut text)?;
    Ok(text)
}
