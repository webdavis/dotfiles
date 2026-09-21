use crate::*;
use pns_adapters::SqliteStore;
use pns_domain::profiles::{Override, because, surfaces_line};

pub(crate) const PROFILE_USAGE: &str =
    "usage: pns profile [<name> [--for <duration> | --until HH:MM] | clear]";

/// The `profile` mode: which bundle of delivery settings is active.
///
/// TYPED BY HAND AND NEVER A HOOK, so a typo is a refusal rather than a
/// silent fallthrough, exactly as `pns mute` argues for itself.
///
/// THE REPORT IS READ BACK from the store after whatever was asked for, so the
/// line cannot claim a profile that never landed.
pub(crate) fn profile_mode() -> i32 {
    let argv: Vec<String> = crate::arguments_after_subcommand();
    let records = SqliteStore::for_records(state_dir());
    match argv.split_first() {
        None => report(&records),
        Some((word, rest)) if word == "clear" && rest.is_empty() => {
            if let Err(error) = records.set_profile_override(None) {
                eprintln!(
                    "pns: state error (the profile override could not be written: {error}); \
                     the profile was not changed"
                );
                return 1;
            }
            report(&records)
        }
        Some((name, rest)) => select(&records, name, rest),
    }
}

fn select(records: &SqliteStore, name: &str, rest: &[String]) -> i32 {
    let defined = crate::profile_runtime::defined_profiles();
    if !defined.iter().any(|known| known == name) {
        eprintln!(
            "pns profile: no profile named `{name}`; this config defines {}",
            defined.join(", ")
        );
        return 2;
    }
    let until = match parse_bound(rest, now_secs(), crate::profile_runtime::minutes_now()) {
        Ok(until) => until,
        Err(refusal) => {
            eprintln!("{refusal}");
            eprintln!("{PROFILE_USAGE}");
            return 2;
        }
    };
    let standing = Override {
        profile: name.to_string(),
        until,
    };
    if let Err(error) = records.set_profile_override(Some(&standing)) {
        eprintln!(
            "pns: state error (the profile override could not be written: {error}); \
             the profile was not changed"
        );
        return 1;
    }
    report(records)
}

/// The two lines `pns profile` prints: the active profile with what chose it,
/// and what that profile admits.
fn report(records: &SqliteStore) -> i32 {
    let reading = crate::profile_runtime::active(records);
    println!(
        "pns: profile `{}` ({})",
        reading.resolved.profile,
        because(
            &reading.resolved.chose,
            &reading.resolved.matched,
            reading.until_clock.as_deref()
        )
    );
    println!("     {}", surfaces_line(&reading.profile));
    0
}

/// `--for <duration>` or `--until HH:MM`, or no bound at all.
///
/// `--until` PAST TODAY'S CLOCK MEANS TOMORROW. Asked at 17:00 to run until
/// 09:00, the operator means nine in the morning; refusing it would make them
/// do date arithmetic to say something unambiguous.
pub(crate) fn parse_bound(
    words: &[String],
    now_secs: Option<u64>,
    minutes_now: Option<u16>,
) -> Result<Option<u64>, String> {
    let [flag, value] = words else {
        if words.is_empty() {
            return Ok(None);
        }
        return Err(format!("pns profile: {PROFILE_USAGE}"));
    };
    let (Some(now), Some(minutes)) = (now_secs, minutes_now) else {
        return Err("pns: state error (the clock cannot be read); no bound was set".to_string());
    };
    match flag.as_str() {
        "--for" => {
            let held = pns_domain::duration::parse_duration(
                "profile duration",
                value,
                pns_domain::mute::MUTE_RANGE,
            )?;
            Ok(Some(now.saturating_add(held.as_secs())))
        }
        "--until" => {
            let wanted = pns_domain::profiles::minute_of_clock(value)
                .ok_or_else(|| format!("pns profile: {value:?} is not an HH:MM time of day"))?;
            let ahead = if wanted > minutes {
                u64::from(wanted - minutes)
            } else {
                u64::from(1440 + u32::from(wanted) - u32::from(minutes))
            };
            Ok(Some(now.saturating_add(ahead * 60)))
        }
        _ => Err(format!("pns profile: {PROFILE_USAGE}")),
    }
}

#[cfg(test)]
mod tests;
