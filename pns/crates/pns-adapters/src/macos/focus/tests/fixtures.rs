use super::*;

/// The live store off dresden on 2026-08-29, trimmed to one record per
/// array and carrying the DUPLICATE assertion the real file holds. Both
/// invalidation arrays are populated, because a store with a live Focus in
/// it also carries the history of the ones that ended.
pub(super) const LIVE_ACTIVE: &str = r#"{
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
pub(super) const TWO_AT_ONCE: &str = r#"{
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
pub(super) const EMPTIED_ARRAY: &str = r#"{
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
pub(super) const KEY_ABSENT: &str = r#"{
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
pub(super) const LIVE_CATALOG: &str = r#"{
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
pub(super) const KEY_DISAGREES: &str = "com.apple.donotdisturb.mode.moonfill";

pub(super) const CASUALLY_CONCERNED: &str = "com.apple.donotdisturb.mode.graduationcapfill";

pub(super) fn named(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_string()).collect()
}

pub(super) fn asserted(identifiers: &[&str]) -> BTreeSet<String> {
    identifiers
        .iter()
        .map(|identifier| (*identifier).to_string())
        .collect()
}
