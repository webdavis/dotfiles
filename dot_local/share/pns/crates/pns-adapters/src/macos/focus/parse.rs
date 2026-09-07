//! macOS Focus, read off the Do Not Disturb store's own bytes.
//!
//! THREE TOTAL FUNCTIONS AND NO IO. The store is two private, undocumented
//! Apple files, so every shape they could hold is answered here rather than
//! at the read: nothing panics, nothing errors, and every unreadable shape
//! answers "no Focus is silencing anything".
//!
//! FAIL OPEN, which is `quiet::is_muted`'s direction and deliberately the
//! opposite of `hue::quiet_now`'s. A schema Apple changes on any macOS update
//! would, failing closed, silence every banner, card and pulse on the morning
//! after an upgrade with nothing on screen to say why. Failing open costs one
//! interruption the operator asked not to have, and `pns doctor` is where the
//! unreadable store is said out loud.
//!
//! POLICY IS PER MODE AND NEVER "a Focus is on". Measured on this operator's
//! own machine, a Focus was asserted for 95% of one day, so a gate that fired
//! on any Focus at all would be a mute with no expiry. `[focus] silence` names
//! the modes that mean it, and a mode nobody named silences nothing.

use std::collections::{BTreeMap, BTreeSet};

/// The Focus modes asserted right now, by mode identifier.
///
/// A LIVE ASSERTION IS THE WHOLE ANSWER. `data[0].storeAssertionRecords` holds
/// one record per Focus currently asserted; ending a Focus MOVES its record
/// into `storeInvalidationRecords`, which nothing here reads. Both spellings
/// of "no Focus" that macOS is documented to write, the key absent and the key
/// present as an empty array, answer an empty set without a special case.
///
/// A SET RATHER THAN A LIST, because the live store on this machine carries
/// the SAME assertion record twice. Uniqueness is not a property macOS
/// maintains, so nothing downstream may count these.
///
/// NO TIMESTAMP IS READ, deliberately. `header.timestamp` moves for writes
/// that are not Focus transitions (cloud sync and record pruning, both
/// measured), so a freshness gate built on it would be a guess dressed as a
/// check.
///
/// TOTAL, AND THE DOCTOR INHERITS THAT. Bytes that are not JSON at all answer
/// an empty set rather than an error, exactly as a schema Apple moved would,
/// so nothing about a file's CONTENTS can ever reach the doctor's
/// could-not-be-read sentence: only a failed read of the file itself does.
/// That is the fail-open direction on purpose, and the accepted limit is
/// stated where the sentence is written.
pub fn active_modes(assertions_json: &str) -> BTreeSet<String> {
    // EVERY MISSING OR MISTYPED STEP READS AS `Null` through this indexing,
    // which is why there is not one explicit error arm below: not JSON, no
    // `data`, `data` not an array, an empty `data`, no records key, that key
    // not an array and a record naming no mode all end at the same empty set.
    let Ok(store) = serde_json::from_str::<serde_json::Value>(assertions_json) else {
        return BTreeSet::new();
    };
    store["data"][0]["storeAssertionRecords"]
        .as_array()
        .map(|records| {
            records
                .iter()
                .filter_map(|record| {
                    record["assertionDetails"]["assertionDetailsModeIdentifier"]
                        .as_str()
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Mode identifier to the display name the operator sees in Control Center,
/// from the mode catalog beside the assertion store.
///
/// KEYED ON `mode.modeIdentifier` RATHER THAN ON THE MAP KEY. The two are
/// equal for all ten modes on this machine, and only the field is the one an
/// assertion's `assertionDetailsModeIdentifier` is named after; the map key is
/// a convention Apple documents nowhere.
///
/// AN UNREADABLE CATALOG IS AN EMPTY MAP, never an error, and that is fail
/// open: with no names resolved only a raw identifier in the config can match,
/// so a broken catalog silences less rather than more.
pub fn mode_names(configurations_json: &str) -> BTreeMap<String, String> {
    let Ok(catalog) = serde_json::from_str::<serde_json::Value>(configurations_json) else {
        return BTreeMap::new();
    };
    catalog["data"][0]["modeConfigurations"]
        .as_object()
        .map(|modes| {
            modes
                .values()
                .filter_map(|entry| {
                    let mode = &entry["mode"];
                    Some((
                        mode["modeIdentifier"].as_str()?.to_string(),
                        mode["name"].as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}
