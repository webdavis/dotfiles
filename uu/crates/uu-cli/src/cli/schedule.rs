//! `uu schedule render`: the launchd job for the configured day and time.

use uu_adapters::{Config, DEFAULT_LABEL, config_path, render_plist};

use uu_adapters::home;

pub fn schedule_mode() -> i32 {
    let Some(home) = home() else {
        return super::no_home();
    };
    let config = match super::loaded(&config_path(&home)) {
        Ok(Some(config)) => config,
        Ok(None) => Config::default(),
        Err(code) => return code,
    };
    print!("{}", render_plist(DEFAULT_LABEL, &home, config.schedule));
    0
}
