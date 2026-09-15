use pns_adapters::{MarkerReading, SystemCommandRunner, SystemProbes, TapFailure};
use pns_protocol::{TapMarker, TapOperation, TapResult, TapWriteStatus};
use std::io::Write;

const USAGE: &str = "pns tap [--info | --install] [--json]";

pub(crate) fn tap_mode() -> i32 {
    let args = crate::arguments_after_subcommand();
    let json = args.iter().any(|arg| arg == "--json");
    let (mut result, code) = match operation(&args) {
        Ok(operation) => {
            let mut result = TapResult::new(operation);
            let code = match execute(&mut result) {
                Ok(()) => 0,
                Err(error) => {
                    result.fail(error.code, &error.message);
                    1
                }
            };
            (result, code)
        }
        Err(()) => {
            let mut result = TapResult::new(TapOperation::Tap);
            result.fail("invalid_arguments", &format!("usage: {USAGE}"));
            (result, 2)
        }
    };
    let output = if json {
        match result.encode() {
            Ok(text) => text,
            Err(_) => {
                // Do not lose whether the marker was recorded when a report exceeds wire bounds.
                result.marker = None;
                result.install = None;
                result.surface = None;
                result.fail("output_failed", "the tap report exceeds the output limits");
                let Ok(text) = result.encode() else {
                    return 1;
                };
                let _ = writeln!(std::io::stdout().lock(), "{text}");
                return 1;
            }
        }
    } else {
        crate::tap_report::render(&result).join("\n")
    };
    let written = if !json && code != 0 {
        writeln!(std::io::stderr().lock(), "{output}")
    } else {
        writeln!(std::io::stdout().lock(), "{output}")
    };
    if written.is_err() { 1 } else { code }
}

fn operation(args: &[String]) -> Result<TapOperation, ()> {
    let mut operation = TapOperation::Tap;
    let mut json = false;
    for arg in args {
        match arg.as_str() {
            "--json" if !json => json = true,
            "--info" if operation == TapOperation::Tap => operation = TapOperation::Info,
            "--install" if operation == TapOperation::Tap => operation = TapOperation::Install,
            _ => return Err(()),
        }
    }
    Ok(operation)
}

fn execute(result: &mut TapResult) -> Result<(), TapFailure> {
    if result.operation == TapOperation::Install {
        result.install = Some(pns_adapters::tap_install()?);
        result.message = "Follow the setup steps, then verify a tap on your devices.".into();
        return Ok(());
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let resolved = pns_adapters::phone_marker_path(
        &home,
        std::env::var_os("PNS_PHONE_MARKER_FILE").as_deref(),
    )?;
    result.marker = Some(TapMarker {
        path: resolved.path.to_string_lossy().into_owned(),
        source: resolved.source.into(),
        config_file: resolved.config_file.to_string_lossy().into_owned(),
        exists: None,
        mtime_epoch_secs: None,
        touched_at: None,
        age_secs: None,
        fresh: None,
    });
    if result.operation == TapOperation::Tap {
        result.write_status = TapWriteStatus::Failed;
        pns_adapters::record_phone_tap(&resolved.path)?;
        result.write_status = TapWriteStatus::Recorded;
    }
    // A fresh set after the write prevents a memoized pre-tap timestamp from winning.
    let probes = SystemProbes::new(SystemCommandRunner, String::new())
        .with_phone_marker(Some(resolved.path));
    let metadata = probes.marker_reading();
    let now = probes.now_secs();
    if let Some(marker) = result.marker.as_mut() {
        marker.exists = metadata.exists();
        marker.mtime_epoch_secs = metadata.mtime();
        marker.touched_at = metadata.mtime().and_then(pns_adapters::utc_timestamp);
        marker.age_secs = now
            .zip(metadata.mtime())
            .map(|(now, time)| now.saturating_sub(time));
    }
    if matches!(
        metadata,
        MarkerReading::Unreadable(_) | MarkerReading::InvalidTimestamp
    ) {
        return Err(TapFailure::new(
            "marker_unreadable",
            "the marker timestamp could not be read",
        ));
    }
    let reading =
        pns_application::operator_surface_reading(&probes, &crate::overrides_from_env(), now);
    if let Some(marker) = result.marker.as_mut() {
        marker.fresh = reading.desk_fresh_secs.and_then(|window| {
            if marker.exists == Some(false) {
                Some(false)
            } else {
                marker
                    .age_secs
                    .map(|age| pns_domain::surface::is_fresh(Some(age), window))
            }
        });
    }
    let surface = match reading.surface {
        pns_domain::surface::Surface::Desk => "desk",
        pns_domain::surface::Surface::Mobile => "mobile",
        pns_domain::surface::Surface::Away => "away",
    };
    result.surface = Some(surface.into());
    let prefix = if result.operation == TapOperation::Tap {
        "Tap recorded. "
    } else {
        ""
    };
    result.message = format!("{prefix}Current surface: {:?}.", reading.surface);
    Ok(())
}
