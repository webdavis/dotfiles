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

/// Whether any asserted mode is one the config named.
///
/// TWO SPELLINGS ARE ACCEPTED for each entry in the list: the display name out
/// of the catalog ("Casually Concerned"), which is what the operator reads in
/// Control Center, and the raw `modeIdentifier`, which is the only handle a
/// mode with no name in the catalog has.
///
/// CASE-INSENSITIVE, because the name is transcribed by hand out of a user
/// interface and "sleep" is not a different Focus from "Sleep". See `same` for
/// exactly how far that goes, which is not as far as "any script".
///
/// AN IDENTIFIER ENTRY IS MATCHED WITH NO CATALOG AT ALL, and that is the
/// DESIGN rather than an oversight of the catalog's failure path. The raw
/// `modeIdentifier` is the ONLY handle a mode the catalog does not name has,
/// so it cannot be made to depend on the catalog without deleting the escape
/// hatch that exists for exactly the case where the catalog says nothing. The
/// consequence, stated so nobody has to rediscover it: a catalog that is
/// absent, gated or garbled leaves identifier entries working and NAME entries
/// inert. That silences LESS rather than more, which is this module's
/// direction, and `pns doctor` says the catalog failed in a clause of its own
/// so the inert half is never mistaken for health.
///
/// AN EMPTY LIST SILENCES NOTHING, which is the feature switched off and the
/// default state of every machine that never wrote a `[focus]` table.
pub fn silenced(
    active: &BTreeSet<String>,
    names: &BTreeMap<String, String>,
    silence: &[String],
) -> bool {
    active.iter().any(|identifier| {
        silence.iter().any(|listed| {
            same(listed, identifier) || names.get(identifier).is_some_and(|name| same(listed, name))
        })
    })
}

/// Two spellings of one Focus mode, compared the way a hand transcribing a
/// name out of Control Center would mean them.
///
/// FOLDED BOTH WAYS, because neither direction alone is enough. MEASURED:
/// "Straße" lowercases to "straße" while "STRASSE" lowercases to "strasse", so
/// a lowercase-only compare misses a name the operator typed in capitals;
/// upper-casing both agrees on "STRASSE". Agreement in EITHER direction is
/// taken as the same name.
///
/// AND THAT IS THE WHOLE OF IT, stated rather than overclaimed: this is case
/// mapping, NOT full Unicode case folding and NOT normalization. MEASURED,
/// both still false: "İstanbul" against "istanbul" (the dotted capital maps to
/// i plus a combining dot), and a decomposed "Cafe\u{301}" against a composed
/// "café". A name that either of those describes must be listed by its raw
/// `modeIdentifier`, which is compared by the same rule but is ASCII on every
/// mode macOS writes. Doing better needs a case-folding and normalization
/// dependency, which this crate does not carry for one config key.
fn same(left: &str, right: &str) -> bool {
    left.to_lowercase() == right.to_lowercase() || left.to_uppercase() == right.to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::{active_modes, mode_names, silenced};
    use std::collections::{BTreeMap, BTreeSet};

    /// The live store off dresden on 2026-08-29, trimmed to one record per
    /// array and carrying the DUPLICATE assertion the real file holds. Both
    /// invalidation arrays are populated, because a store with a live Focus in
    /// it also carries the history of the ones that ended.
    const LIVE_ACTIVE: &str = r#"{
      "data": [
        {
          "storeInvalidationRecords": [
            {
              "invalidationAssertion": {
                "assertionUUID": "05F40259-C504-49F4-B906-C44C46F73B4D",
                "assertionSource": {
                  "assertionClientIdentifier": "com.apple.donotdisturb.private.workout-trigger",
                  "assertionSourceDeviceIdentifier": "BDC180B7-721C-4B89-B886-2D1552A33D04"
                },
                "assertionStartDateTimestamp": 721321214.042869,
                "assertionDetails": {
                  "assertionDetailsIdentifier": "com.apple.donotdisturb.trigger.workout.68CBE540-D150-41FD-BCFD-D58ADA5F0BD1",
                  "assertionDetailsModeIdentifier": "com.apple.donotdisturb.mode.workout",
                  "assertionDetailsReason": "user-action"
                }
              },
              "invalidationSource": {
                "assertionClientIdentifier": "com.apple.focus.activity-manager",
                "assertionSourceDeviceIdentifier": "F422EBF7-6BE9-42BE-8230-B687E19007BA"
              },
              "invalidationDateTimestamp": 806942866.743962,
              "invalidationReason": "user-changed-state"
            }
          ],
          "storeInvalidationRequestRecords": [
            {
              "invalidationRequestPredicate": { "invalidationPredicateType": "any" },
              "invalidationRequestReason": "user-changed-state",
              "invalidationRequestUUID": "2B4215BB-7E19-4D57-AEE2-3328D0499D5E",
              "invalidationRequestSource": {
                "assertionClientIdentifier": "com.apple.focus.activity-manager",
                "assertionSourceDeviceIdentifier": "F422EBF7-6BE9-42BE-8230-B687E19007BA"
              },
              "invalidationRequestDateTimestamp": 809713980.03135
            }
          ],
          "storeAssertionRecords": [
            {
              "assertionUUID": "3CC0682F-2B5C-4C9D-95EB-93E0B5B2677A",
              "assertionSource": {
                "assertionClientIdentifier": "com.apple.focus.activity-manager",
                "assertionSourceDeviceIdentifier": "F422EBF7-6BE9-42BE-8230-B687E19007BA"
              },
              "assertionStartDateTimestamp": 809713980.03135,
              "assertionDetails": {
                "assertionDetailsIdentifier": "com.apple.focus.activity-manager",
                "assertionDetailsModeIdentifier": "com.apple.donotdisturb.mode.graduationcapfill",
                "assertionDetailsLifetime": {
                  "assertionDetailsScheduleLifetimeScheduleIdentifier": "com.apple.donotdisturb.schedule.default",
                  "assertionDetailsLifetimeType": "schedule",
                  "assertionDetailsScheduleLifetimeBehavior": "expire-on-end"
                },
                "assertionDetailsReason": "user-action"
              }
            },
            {
              "assertionUUID": "3CC0682F-2B5C-4C9D-95EB-93E0B5B2677A",
              "assertionSource": {
                "assertionClientIdentifier": "com.apple.focus.activity-manager",
                "assertionSourceDeviceIdentifier": "F422EBF7-6BE9-42BE-8230-B687E19007BA"
              },
              "assertionStartDateTimestamp": 809713980.03135,
              "assertionDetails": {
                "assertionDetailsIdentifier": "com.apple.focus.activity-manager",
                "assertionDetailsModeIdentifier": "com.apple.donotdisturb.mode.graduationcapfill",
                "assertionDetailsLifetime": {
                  "assertionDetailsScheduleLifetimeScheduleIdentifier": "com.apple.donotdisturb.schedule.default",
                  "assertionDetailsLifetimeType": "schedule",
                  "assertionDetailsScheduleLifetimeBehavior": "expire-on-end"
                },
                "assertionDetailsReason": "user-action"
              }
            }
          ]
        }
      ],
      "header": { "version": 8, "timestamp": 809744069.273127 }
    }"#;

    /// TWO MODES ASSERTED AT ONCE, which the operator's own history really
    /// does: reconstructed off this store, a Sleep schedule ran from 23:45 to
    /// 05:00 INSIDE an eighteen-hour Casually Concerned span. The array is a
    /// list of everything asserted, not the one thing that is on.
    const TWO_AT_ONCE: &str = r#"{
      "data": [
        {
          "storeInvalidationRecords": [],
          "storeInvalidationRequestRecords": [],
          "storeAssertionRecords": [
            {
              "assertionUUID": "3CC0682F-2B5C-4C9D-95EB-93E0B5B2677A",
              "assertionStartDateTimestamp": 809713980.03135,
              "assertionDetails": {
                "assertionDetailsIdentifier": "com.apple.focus.activity-manager",
                "assertionDetailsModeIdentifier": "com.apple.donotdisturb.mode.graduationcapfill",
                "assertionDetailsReason": "user-action"
              }
            },
            {
              "assertionUUID": "8F2A1C55-9D0B-42E7-BC31-7A0E4D9F6612",
              "assertionStartDateTimestamp": 809739903.11,
              "assertionDetails": {
                "assertionDetailsIdentifier": "com.apple.sleep.sleep-mode",
                "assertionDetailsModeIdentifier": "com.apple.sleep.sleep-mode",
                "assertionDetailsReason": "schedule"
              }
            }
          ]
        }
      ],
      "header": { "version": 8, "timestamp": 809744069.273127 }
    }"#;

    /// THE FIRST DOCUMENTED SPELLING OF "no Focus": the key is present and
    /// holds an empty array, with the invalidation history left in place.
    const EMPTIED_ARRAY: &str = r#"{
      "data": [
        {
          "storeInvalidationRecords": [
            {
              "invalidationAssertion": {
                "assertionUUID": "05F40259-C504-49F4-B906-C44C46F73B4D",
                "assertionDetails": {
                  "assertionDetailsModeIdentifier": "com.apple.donotdisturb.mode.workout"
                }
              },
              "invalidationDateTimestamp": 806942866.743962,
              "invalidationReason": "user-changed-state"
            }
          ],
          "storeInvalidationRequestRecords": [],
          "storeAssertionRecords": []
        }
      ],
      "header": { "version": 8, "timestamp": 809744069.273127 }
    }"#;

    /// THE SECOND DOCUMENTED SPELLING, which disagrees with the first: the key
    /// is not written at all. Two independent third-party readings of this
    /// store describe different empty shapes, so both are pinned and neither
    /// is assumed.
    const KEY_ABSENT: &str = r#"{
      "data": [
        {
          "storeInvalidationRecords": [
            {
              "invalidationAssertion": {
                "assertionUUID": "05F40259-C504-49F4-B906-C44C46F73B4D",
                "assertionDetails": {
                  "assertionDetailsModeIdentifier": "com.apple.donotdisturb.mode.graduationcapfill"
                }
              },
              "invalidationDateTimestamp": 806942866.743962,
              "invalidationReason": "client-ended"
            }
          ],
          "storeInvalidationRequestRecords": []
        }
      ],
      "header": { "version": 8, "timestamp": 809744069.273127 }
    }"#;

    /// The mode catalog off dresden the same day, trimmed to two of its ten
    /// entries and to the sibling keys a reader has to navigate past.
    ///
    /// PLUS ONE SYNTHETIC THIRD, and it is synthetic in both of its details.
    /// Its MAP KEY DISAGREES with its own `mode.modeIdentifier`, which no
    /// entry on this machine does today: without it the choice of which of the
    /// two to key on is unfalsifiable, and a reader keyed on the map key
    /// passes every test. Its NAME IS NON-ASCII for the second reason: every
    /// other fixture here is ASCII, so nothing would notice `same` dropping to
    /// a single case fold. Apple's own ten are all ASCII; a custom mode's name
    /// is whatever the operator typed.
    const LIVE_CATALOG: &str = r#"{
      "data": [
        {
          "modeConfigurations": {
            "com.apple.donotdisturb.mode.graduationcapfill": {
              "dimsLockScreen": false,
              "impactsAvailability": false,
              "lastModifiedByVersion": 2,
              "compatibilityVersion": 2,
              "mode": {
                "name": "Casually Concerned",
                "tintColorName": "systemRedColor",
                "symbolDescriptorTintStyle": 0,
                "identifier": "586E30E1-1C59-45D9-B531-838B7759C1E2",
                "semanticType": -1,
                "symbolImageName": "graduationcap.fill",
                "modeIdentifier": "com.apple.donotdisturb.mode.graduationcapfill",
                "visibility": 0
              },
              "automaticallyGenerated": false,
              "hasSecureData": true
            },
            "com.apple.sleep.sleep-mode": {
              "dimsLockScreen": true,
              "impactsAvailability": true,
              "lastModifiedByVersion": 2,
              "compatibilityVersion": 2,
              "mode": {
                "name": "Sleep",
                "tintColorName": "systemIndigoColor",
                "symbolDescriptorTintStyle": 0,
                "identifier": "C02A0910-6FAD-463E-95BB-2D38D85C88C4",
                "semanticType": 1,
                "symbolImageName": "bed.double.fill",
                "modeIdentifier": "com.apple.sleep.sleep-mode",
                "visibility": 0
              },
              "automaticallyGenerated": true,
              "hasSecureData": true
            },
            "com.apple.donotdisturb.mode.a-key-that-disagrees": {
              "dimsLockScreen": false,
              "mode": {
                "name": "Straße",
                "identifier": "0F1B4C77-3A62-4E10-9C5D-1E7A9B2F4D08",
                "modeIdentifier": "com.apple.donotdisturb.mode.moonfill",
                "visibility": 0
              },
              "automaticallyGenerated": false,
              "hasSecureData": true
            }
          }
        }
      ],
      "header": { "version": 3, "timestamp": 809128539.244755 }
    }"#;

    /// The mode identifier of the synthetic entry, which is the FIELD and not
    /// the map key it sits under.
    const KEY_DISAGREES: &str = "com.apple.donotdisturb.mode.moonfill";

    const CASUALLY_CONCERNED: &str = "com.apple.donotdisturb.mode.graduationcapfill";

    fn named(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }

    fn asserted(identifiers: &[&str]) -> BTreeSet<String> {
        identifiers
            .iter()
            .map(|identifier| (*identifier).to_string())
            .collect()
    }

    // --- reading the assertion store ----------------------------------------

    #[test]
    fn a_store_holding_a_live_assertion_names_the_mode_that_is_on() {
        assert_eq!(
            active_modes(LIVE_ACTIVE),
            asserted(&[CASUALLY_CONCERNED]),
            "the live record's own mode identifier is the answer"
        );
    }

    #[test]
    fn the_same_record_written_twice_is_one_mode_and_not_two() {
        // THE LIVE FILE REALLY DOES THIS: two records carry one
        // `assertionUUID`. Uniqueness is not a property macOS maintains, so a
        // reader that counted these would be counting its own duplicate.
        assert_eq!(active_modes(LIVE_ACTIVE).len(), 1);
    }

    #[test]
    fn every_mode_asserted_at_once_is_named_and_not_just_the_first() {
        // A reader that assumed one live Focus, or that took `[0]` because the
        // records "should" be unique, would silence on one of these and miss
        // the other. The operator's own history really overlaps two.
        assert_eq!(
            active_modes(TWO_AT_ONCE),
            asserted(&[CASUALLY_CONCERNED, "com.apple.sleep.sleep-mode"])
        );
        for listed in ["Casually Concerned", "Sleep"] {
            assert!(
                silenced(
                    &active_modes(TWO_AT_ONCE),
                    &mode_names(LIVE_CATALOG),
                    &named(&[listed])
                ),
                "naming either one is enough: {listed}"
            );
        }
    }

    #[test]
    fn both_documented_spellings_of_no_focus_name_no_mode() {
        // The two third-party descriptions of the empty state DISAGREE with
        // each other, so both are pinned. This is also the pair that catches a
        // substring grep for `storeAssertionRecords`, which is the
        // implementation shipped elsewhere and is right about only one of them.
        assert!(
            active_modes(EMPTIED_ARRAY).is_empty(),
            "an emptied array is no Focus"
        );
        assert!(
            active_modes(KEY_ABSENT).is_empty(),
            "an absent key is no Focus"
        );
    }

    #[test]
    fn an_ended_focus_in_the_invalidation_history_is_never_an_active_one() {
        // Deactivation MOVES the record rather than deleting it, so both empty
        // fixtures still carry a mode identifier further down the file. A
        // reader pointed at the wrong array reports a Focus that ended weeks
        // ago as the one that is on now.
        assert!(
            !active_modes(EMPTIED_ARRAY).contains("com.apple.donotdisturb.mode.workout"),
            "the workout Focus in the history had already ended"
        );
        assert!(
            !active_modes(KEY_ABSENT).contains(CASUALLY_CONCERNED),
            "and so had this one"
        );
    }

    #[test]
    fn nothing_readable_names_no_mode_one_row_per_failure_shape() {
        for (shape, why) in [
            ("", "an empty file"),
            ("{", "a truncated document"),
            ("null", "a JSON null"),
            ("[]", "a top level array"),
            (r#"{"data":[]}"#, "no store in the data"),
            (r#"{"data":[{}]}"#, "a store with no arrays"),
            (
                r#"{"data":[{"storeAssertionRecords":{}}]}"#,
                "the records as an object",
            ),
            (
                r#"{"data":[{"storeAssertionRecords":["not a record"]}]}"#,
                "a record that is not an object",
            ),
            (
                r#"{"data":[{"storeAssertionRecords":[{"assertionDetails":{}}]}]}"#,
                "a record naming no mode",
            ),
            (
                // A JSON STREAM, restated for serde: `from_str` refuses
                // trailing content, and this row is what proves the refusal
                // lands on the SAFE side rather than reading the first
                // document and ignoring the rest.
                r#"{"data":[{"storeAssertionRecords":[]}]} {"data":[{"storeAssertionRecords":[{"assertionDetails":{"assertionDetailsModeIdentifier":"x"}}]}]}"#,
                "two concatenated documents",
            ),
        ] {
            assert!(
                active_modes(shape).is_empty(),
                "{why} is not a Focus: {shape}"
            );
        }
    }

    // --- reading the mode catalog -------------------------------------------

    #[test]
    fn the_catalog_turns_an_identifier_into_the_name_control_center_shows() {
        let names = mode_names(LIVE_CATALOG);
        assert_eq!(
            names.get(CASUALLY_CONCERNED).map(String::as_str),
            Some("Casually Concerned")
        );
        assert_eq!(
            names.get("com.apple.sleep.sleep-mode").map(String::as_str),
            Some("Sleep")
        );
    }

    #[test]
    fn a_mode_is_named_by_its_own_identifier_field_and_never_by_the_map_key() {
        // THE MAP KEY IS A CONVENTION APPLE DOCUMENTS NOWHERE, and only the
        // field is the one an assertion's `assertionDetailsModeIdentifier` is
        // named after. Every entry on this machine spells the two the same, so
        // this synthetic pair is the only thing standing between the choice
        // and a reader that keyed on the map key and passed anyway.
        let names = mode_names(LIVE_CATALOG);
        assert_eq!(
            names.get(KEY_DISAGREES).map(String::as_str),
            Some("Straße"),
            "the record's own field is the key"
        );
        assert!(
            !names.contains_key("com.apple.donotdisturb.mode.a-key-that-disagrees"),
            "and the map key it sat under is not: {names:?}"
        );
    }

    #[test]
    fn a_catalog_nothing_can_read_resolves_no_names_at_all() {
        // FAIL OPEN AGAIN, and in the same direction: with no names, only a
        // raw identifier in the config can match, so a broken catalog silences
        // less rather than more.
        for shape in ["", "{", "null", r#"{"data":[]}"#, r#"{"data":[{}]}"#] {
            assert!(
                mode_names(shape).is_empty(),
                "an unreadable catalog names nothing: {shape}"
            );
        }
    }

    // --- the policy ---------------------------------------------------------

    #[test]
    fn a_mode_the_config_names_by_its_display_name_is_silenced() {
        assert!(silenced(
            &active_modes(LIVE_ACTIVE),
            &mode_names(LIVE_CATALOG),
            &named(&["Casually Concerned"])
        ));
    }

    #[test]
    fn the_name_is_matched_however_the_operator_capitalised_it() {
        // Transcribed by hand out of a user interface: "sleep" is not a
        // different Focus from "Sleep".
        assert!(silenced(
            &active_modes(LIVE_ACTIVE),
            &mode_names(LIVE_CATALOG),
            &named(&["casually CONCERNED"])
        ));
    }

    #[test]
    fn a_name_whose_lowercase_disagrees_with_itself_is_still_the_same_name() {
        // MEASURED, and the reason `same` folds both ways: "Straße"
        // lowercases to "straße" and "STRASSE" lowercases to "strasse", so a
        // lowercase-only compare reads a name the operator typed in capitals
        // as a different Focus and silently silences nothing.
        assert!(silenced(
            &asserted(&[KEY_DISAGREES]),
            &mode_names(LIVE_CATALOG),
            &named(&["STRASSE"])
        ));
    }

    #[test]
    fn a_raw_mode_identifier_is_accepted_for_a_mode_the_catalog_does_not_name() {
        // The identifier is the ONLY handle an unnamed mode has, so it is
        // matched with no catalog at all.
        assert!(silenced(
            &active_modes(LIVE_ACTIVE),
            &BTreeMap::new(),
            &named(&[CASUALLY_CONCERNED])
        ));
    }

    #[test]
    fn a_focus_nobody_named_silences_nothing() {
        // THE WHOLE POINT OF PER-MODE POLICY. A Focus was asserted for 95% of
        // one measured day on this machine, so "a Focus is on" is not a
        // question worth acting on; "is it one of these" is.
        assert!(!silenced(
            &active_modes(LIVE_ACTIVE),
            &mode_names(LIVE_CATALOG),
            &named(&["Sleep", "Coding"])
        ));
    }

    #[test]
    fn an_empty_list_is_the_feature_switched_off() {
        assert!(!silenced(
            &active_modes(LIVE_ACTIVE),
            &mode_names(LIVE_CATALOG),
            &[]
        ));
    }

    #[test]
    fn a_named_mode_that_is_not_asserted_silences_nothing() {
        assert!(!silenced(
            &asserted(&[]),
            &mode_names(LIVE_CATALOG),
            &named(&["Casually Concerned"])
        ));
    }
}
