use super::*;
use std::os::unix::ffi::OsStringExt;

fn request(spool: &str, overrides: &[(&str, OsString)]) -> String {
    let fixture = Fixture::new(spool);
    let mut config = Configuration::read(|key| match key {
        "HOME" => Some(fixture.home.clone().into()),
        "OSQUERY_DIGEST_STORE" => Some(fixture.store.clone().into()),
        _ => overrides
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| value.clone()),
    })
    .unwrap();
    config.delivery = crate::producer_delivery(&fixture.engine());
    let runner = Runner {
        expected: fixture.engine(),
        reply: Some(Reply::Committed),
        effects: fixture.effects.clone(),
    };
    let alarm = Runner {
        expected: config.alarm.clone(),
        reply: None,
        effects: fixture.effects.clone(),
    };
    let mut stderr = Vec::new();
    assert_eq!(execute(config, Time, runner, alarm, &mut stderr), 0);
    assert!(stderr.is_empty());
    assert_eq!(fixture.kept().as_deref(), Some(spool));
    let effects = fixture.effects.borrow();
    assert!(effects.alarms.is_empty());
    assert_eq!(effects.requests.len(), 1);
    effects.requests[0].clone()
}

fn assert_body(request: &str, body: &str) {
    let escaped = body
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    assert!(request.contains(&format!("\\n{escaped}\"")), "{request}");
}

const UNICODE_ROW: &str =
    "{\"detector\":\"one\",\"identity\":\"é`界\\nxy\",\"summary\":\"summary\"}\n";

#[test]
fn the_group_and_bullet_caps_are_env_overridable_named_constants() {
    let mut spool: String = (1..=6)
        .map(|n| finding(&format!("det_{n}"), "first"))
        .collect();
    for n in 1..=4 {
        spool.push_str(&finding("det_1", &format!("extra_{n}")));
    }
    let request = request(
        &spool,
        &[
            ("DIGEST_MAX_GROUPS", "2".into()),
            ("DIGEST_MAX_BULLETS_PER_GROUP", "3".into()),
        ],
    );
    assert_eq!(request.matches("**det_").count(), 2, "{request}");
    assert_eq!(request.matches("\\n- ").count(), 4, "{request}");
    for text in ["**det_1** (5)", "… +2 more", "and 4 more detector group(s)"] {
        assert!(request.contains(text), "{request}");
    }
}

#[test]
fn an_overridden_body_cap_counts_codepoints_and_keeps_the_truncation_marker() {
    assert_body(
        &request(UNICODE_ROW, &[("DIGEST_MAX_BODY_CHARS", "16".into())]),
        "**one** (1)\n- `é\n… (truncated)",
    );
}

#[test]
fn an_overridden_field_cap_applies_after_sanitizing_each_field() {
    assert_body(
        &request(UNICODE_ROW, &[("DIGEST_MAX_FIELD_CHARS", "003".into())]),
        "**one** (1)\n- `é界 …(truncated)` - `sum…(truncated)`\n",
    );
}

#[test]
fn zero_caps_render_their_overflow_markers_instead_of_using_defaults() {
    for (key, body) in [
        (
            "DIGEST_MAX_GROUPS",
            "… and 1 more detector group(s) - see results.log",
        ),
        ("DIGEST_MAX_BULLETS_PER_GROUP", "**one** (1)\n… +1 more\n"),
        ("DIGEST_MAX_BODY_CHARS", "\n… (truncated)"),
        (
            "DIGEST_MAX_FIELD_CHARS",
            "**one** (1)\n- `…(truncated)` - `…(truncated)`\n",
        ),
    ] {
        assert_body(&request(UNICODE_ROW, &[(key, "000".into())]), body);
    }
}

#[test]
fn a_non_numeric_cap_env_value_falls_back_without_discarding_valid_other_caps() {
    for key in [
        "DIGEST_MAX_GROUPS",
        "DIGEST_MAX_BULLETS_PER_GROUP",
        "DIGEST_MAX_BODY_CHARS",
        "DIGEST_MAX_FIELD_CHARS",
    ] {
        // Two representatives here, because what this loop proves is that the
        // OTHER cap survived a neighbour's fallback. Which strings are
        // unusable is settled once, over the exact fallback values, in
        // `configuration::tests`.
        for invalid in ["abc", "٢"] {
            let overrides = if key == "DIGEST_MAX_FIELD_CHARS" {
                vec![
                    (key, invalid.into()),
                    ("DIGEST_MAX_BODY_CHARS", "16".into()),
                ]
            } else {
                vec![
                    (key, invalid.into()),
                    ("DIGEST_MAX_FIELD_CHARS", "3".into()),
                ]
            };
            let body = if key == "DIGEST_MAX_FIELD_CHARS" {
                "**one** (1)\n- `é\n… (truncated)"
            } else {
                "**one** (1)\n- `é界 …(truncated)` - `sum…(truncated)`\n"
            };
            assert_body(&request(UNICODE_ROW, &overrides), body);
        }
    }
    assert_body(
        &request(
            UNICODE_ROW,
            &[
                ("DIGEST_MAX_GROUPS", OsString::from_vec(vec![0xff])),
                ("DIGEST_MAX_FIELD_CHARS", "3".into()),
            ],
        ),
        "**one** (1)\n- `é界 …(truncated)` - `sum…(truncated)`\n",
    );
}

#[test]
fn large_digit_only_caps_remain_effectively_uncapped_without_numeric_overflow() {
    let long = "é".repeat(250);
    let mut spool: String = (0..13)
        .map(|n| finding(&format!("d{n:02}"), &long))
        .collect();
    // Distinct, so the bullet count measures the raised cap rather than the
    // repeat collapse.
    for n in 0..12 {
        spool.push_str(&finding("d00", &format!("extra{n:02}")));
    }
    let huge = "9".repeat(400);
    let request = request(
        &spool,
        &[
            ("DIGEST_MAX_GROUPS", huge.clone().into()),
            ("DIGEST_MAX_BULLETS_PER_GROUP", huge.clone().into()),
            ("DIGEST_MAX_BODY_CHARS", huge.clone().into()),
            ("DIGEST_MAX_FIELD_CHARS", huge.into()),
        ],
    );
    assert_eq!(request.matches("**d").count(), 13);
    assert_eq!(request.matches("\\n- ").count(), 25);
    assert!(request.contains(&long));
    assert!(!request.contains("truncated"));
    assert!(!request.contains("more"));
}
