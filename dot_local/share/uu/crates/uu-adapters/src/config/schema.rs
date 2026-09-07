//! The schema this file is judged against: what every table serves, and what
//! shape one setting has to have.
//!
//! EVERY UNKNOWN KEY IS REFUSED BY NAME. The failure that buys is the one a
//! silent pass-through cannot report: a lane spelled `[lanes.hedr]` is a week
//! that quietly updates nothing while the operator reads a config that looks
//! right.

use super::ConfigError;

/// The shared tables' keys. Each typed lane owns its own vocabulary; both
/// paths refuse mistyped keys and list what to write instead.
pub const TABLE_KEYS: &[(&str, &[&str])] = &[
    (TOP_LEVEL, &["alerts", "lanes", "records", "schedule"]),
    ("schedule", &["day", "time"]),
    ("records", &["failure_webhook", "key", "url"]),
    ("alerts", &["binary"]),
];

/// The roster row for the file's own top level. THE EMPTY NAME, because that
/// level has no heading an operator writes.
pub const TOP_LEVEL: &str = "";

/// What one table serves, or `None` for a table this schema has no vocabulary
/// for.
pub fn keys_of(table: &str) -> Option<&'static [&'static str]> {
    TABLE_KEYS
        .iter()
        .find(|(name, _)| *name == table)
        .map(|(_, keys)| *keys)
}

/// Whether a table admits a key, refusing it BY NAME and with the whole
/// vocabulary spelled out when it does not. `table` is what the refusal NAMES
/// as the offender; `vocabulary` is which shared `TABLE_KEYS` row judges it.
/// Lane callers instead pass their typed vocabulary to `admits_lane`.
///
/// THIS IS THE ONE GATE, and every table's match then has a fallthrough that
/// does nothing. The alternative shape, a gate here AND a refusal in each
/// fallthrough, spells the same rule twice: removing either leaves the suite
/// green, so neither is pinned and the roster stops being load-bearing. The
/// drift this arrangement could still admit, a key declared in the roster with
/// no arm to read it, is what `every_key_the_roster_declares_is_actually_read`
/// walks.
pub fn admits(table: &str, vocabulary: &str, key: &str) -> Result<(), ConfigError> {
    let serves = keys_of(vocabulary)
        .ok_or_else(|| ConfigError::Invalid(format!("no vocabulary for `{vocabulary}`")))?;
    admit_key(table, "the table", serves, key)
}

pub fn admits_lane(table: &str, kind: &str, serves: &[&str], key: &str) -> Result<(), ConfigError> {
    admit_key(table, &format!("a `{kind}` lane"), serves, key)
}

fn admit_key(table: &str, whose: &str, serves: &[&str], key: &str) -> Result<(), ConfigError> {
    if serves.contains(&key) {
        return Ok(());
    }
    Err(ConfigError::Invalid(format!(
        "unknown `{table}` key `{key}`; {whose} serves {}",
        serves.join(", ")
    )))
}

/// One table, refused BY NAME when the operator wrote something else there.
pub fn table_of(table: &str, value: toml::Value) -> Result<toml::Table, ConfigError> {
    match value {
        toml::Value::Table(entries) => Ok(entries),
        other => Err(ConfigError::Invalid(format!(
            "`{table}` has type `{}`, not a table",
            other.type_str()
        ))),
    }
}

/// One key that has to be a string with something in it. A value that is
/// empty, or nothing but whitespace, names nothing, so it is refused rather
/// than read as "use the default": the operator wrote a value, and silently
/// substituting another is a setting they believe they set. A blank one is the
/// worse of the two, because the file reads as though the setting was made.
pub fn non_empty(table: &str, key: &str, setting: &toml::Value) -> Result<String, ConfigError> {
    match setting.as_str() {
        Some(blank) if blank.trim().is_empty() => Err(ConfigError::Invalid(format!(
            "`{table}` key `{key}` is empty or only whitespace, so it names nothing"
        ))),
        Some(value) => Ok(value.to_string()),
        None => Err(ConfigError::Invalid(format!(
            "`{table}` key `{key}` has type `{}`, not a string",
            setting.type_str()
        ))),
    }
}

/// A `non_empty` string that also has to be an ABSOLUTE path, for a key whose
/// whole job is to name a file whose directory the lane then derives.
pub fn absolute(table: &str, key: &str, setting: &toml::Value) -> Result<String, ConfigError> {
    let stated = non_empty(table, key, setting)?;
    if !stated.starts_with('/') {
        return Err(ConfigError::Invalid(format!(
            "`{table}` key `{key}` is `{stated}`, which is not an absolute path; the lane runs it \
             with its own directory first on PATH, so it must name the file in full"
        )));
    }
    Ok(stated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::probes::refusal;
    use crate::config::probes::{REGISTRATIONS, parse_config};

    #[test]
    fn every_table_refuses_its_own_near_miss_and_lists_its_vocabulary() {
        for (text, table, key, serves) in [
            (
                "[schedule]\ndays = \"sunday\"\n",
                "schedule",
                "days",
                "day, time",
            ),
            (
                "[records]\nkey = \"k\"\nurls = \"u\"\n",
                "records",
                "urls",
                "key, url",
            ),
            ("[alerts]\nbin = \"pns\"\n", "alerts", "bin", "binary"),
            (
                "[lanes.herdr]\nplugin = []\n",
                "lanes.herdr",
                "plugin",
                "binary, deadline_secs, escalate_after_runs, plugins, type",
            ),
        ] {
            let detail = refusal(text);
            assert!(
                detail.contains(&format!("unknown `{table}` key `{key}`")),
                "{detail}"
            );
            assert!(detail.contains(serves), "{detail}");
        }
    }

    #[test]
    fn a_table_written_as_a_scalar_is_refused_by_name() {
        for (text, table) in [
            ("schedule = 3\n", "schedule"),
            ("records = \"x\"\n", "records"),
            ("alerts = true\n", "alerts"),
            ("lanes = 1\n", "lanes"),
        ] {
            let detail = refusal(text);
            assert!(detail.contains(&format!("`{table}` has type")), "{detail}");
        }
    }

    #[test]
    fn every_lane_type_refuses_a_key_it_does_not_serve_and_so_does_a_user_named_lane() {
        // The built-in roster, judged by its own name...
        for registration in REGISTRATIONS {
            let kind = registration.type_name();
            let detail = refusal(&format!("[lanes.{kind}]\nbogus = 1\n"));
            assert!(
                detail.contains(&format!("unknown `lanes.{kind}` key `bogus`")),
                "{detail}"
            );
        }
        // ...and an operator-chosen name of that same type, judged the same
        // way: the table named in the refusal is the CHOSEN name, and the
        // vocabulary spelled out is still the TYPE's.
        let detail = refusal("[lanes.mine]\ntype = \"herdr\"\nbogus = 1\n");
        assert!(
            detail.contains("unknown `lanes.mine` key `bogus`"),
            "{detail}"
        );
        assert!(detail.contains("a `herdr` lane serves"), "{detail}");
    }

    #[test]
    fn every_key_the_roster_declares_is_actually_read() {
        // The one drift a single admission gate can still admit: a key added to
        // a shared or typed lane vocabulary with no arm to read it is ignored, so
        // the operator writes a setting that does nothing and no refusal ever
        // mentions it. Every key here is handed a value of the wrong type; an
        // arm refuses it, and a key with no arm accepts anything.
        //
        // A BOOLEAN IS THE PROBE, because no key this schema serves admits
        // one. An integer probe would be a LEGAL value for `deadline_secs`,
        // and this test would then read the key it is meant to walk as unread.
        let tables = TABLE_KEYS
            .iter()
            .map(|(name, keys)| (name.to_string(), *keys))
            .chain(
                REGISTRATIONS
                    .iter()
                    .map(|entry| (format!("lanes.{}", entry.type_name()), entry.keys())),
            );
        for (table, keys) in tables {
            for key in keys {
                let text = if table == TOP_LEVEL {
                    format!("{key} = true\n")
                } else {
                    let probe = if *key == "auto_commit" { "42" } else { "true" };
                    format!("[{table}]\n{key} = {probe}\n")
                };
                let detail = match parse_config(&text) {
                    Err(error) => error.detail().to_string(),
                    Ok(_) => panic!("`{table}` declares `{key}` and nothing reads it"),
                };
                // AND THE REFUSAL MUST NOT BE "UNKNOWN". A key nothing reads is
                // refused too, by the arm that catches everything else, so the
                // reason it gives is the whole difference between a key that is
                // read and one the roster merely advertises.
                assert!(
                    !detail.contains("unknown"),
                    "`{table}` declares `{key}` and nothing reads it: {detail}"
                );
            }
        }
    }

    #[test]
    fn an_empty_string_setting_is_refused_rather_than_read_as_the_default() {
        for (text, table, key) in [
            ("[records]\nkey = \"\"\n", "records", "key"),
            ("[records]\nkey = \"k\"\nurl = \"\"\n", "records", "url"),
            ("[alerts]\nbinary = \"\"\n", "alerts", "binary"),
            ("[lanes.herdr]\nbinary = \"\"\n", "lanes.herdr", "binary"),
            ("[schedule]\nday = \"\"\n", "schedule", "day"),
            ("[schedule]\ntime = \"\"\n", "schedule", "time"),
        ] {
            let detail = refusal(text);
            assert!(
                detail.contains(&format!("`{table}` key `{key}` is empty")),
                "{detail}"
            );
        }
    }

    #[test]
    fn a_setting_holding_only_whitespace_is_refused_the_same_way_an_empty_one_is() {
        // A space is not a value. `binary = " "` is a command nothing can run
        // and `url = " "` is a route nothing can post to, and both of them
        // LOOK set in the file, which is worse than a key that is not there:
        // the operator reads a config that says the setting is made.
        for (text, table, key) in [
            ("[records]\nkey = \" \"\n", "records", "key"),
            ("[records]\nkey = \"k\"\nurl = \" \"\n", "records", "url"),
            ("[alerts]\nbinary = \"\t\"\n", "alerts", "binary"),
            ("[lanes.herdr]\nbinary = \"  \"\n", "lanes.herdr", "binary"),
            ("[schedule]\nday = \" \"\n", "schedule", "day"),
            ("[schedule]\ntime = \"\t \"\n", "schedule", "time"),
        ] {
            let detail = refusal(text);
            assert!(
                detail.contains(&format!("`{table}` key `{key}` is empty")),
                "{detail}"
            );
        }
    }
}
