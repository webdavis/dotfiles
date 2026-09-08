use super::*;
#[test]
fn the_lamp_map_starter_is_always_offered_and_is_wholly_commented_out() {
    // ALWAYS PRESENT AND COMMENTED, whether or not hue is armed: this
    // wizard never asks about the lamp map at all, so the starter reads
    // as inert documentation rather than a question the walk answered.
    // AN EMPTY `[lights]` IS A DISTINCT STATE, the operator asking for the
    // lamps and naming none, so the starter is an example to fill in and
    // never a heading standing on its own.
    for text in [
        compose_config(&every_feature_armed()),
        compose_config(&Answers::default()),
    ] {
        assert!(text.contains("# [lights]"), "{text}");
        assert!(!text.contains("\n[lights"), "{text}");
        assert!(parsed(&text).lights.is_none());
    }
}

#[test]
fn a_credential_carrying_quotes_and_backslashes_reaches_the_config_as_itself() {
    // A PASTED SECRET IS UNTRUSTED TEXT. Interpolating it raw composes a
    // file that will not parse at best, and at worst one whose value stops
    // where the operator's own quote did.
    let answers = Answers {
        hermes_key: "a\"b\\c".to_string(),
        ..every_feature_armed()
    };
    let config = parsed(&compose_config(&answers));
    assert_eq!(
        config.plugins["hermes"].settings["key"].as_str(),
        Some("a\"b\\c")
    );
}

/// Every line SHAPED like a documented key, however it is spelled: the
/// loose reading the strict scan is held against.
///
/// THE STRICT SCAN IS WHITESPACE-EXACT AND LOWERCASE-ONLY, which is what
/// makes it silent rather than wrong: a line it does not recognise as a key
/// is not a line it complains about, it is a line it never sees. This
/// reader recognises the shape alone, so the two disagreeing is the
/// wizard documenting something the roster was never asked about.
fn key_shaped_lines(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| {
            let bare = line.strip_prefix("# ").unwrap_or(line);
            let Some((name, _)) = bare.split_once('=') else {
                return false;
            };
            let name = name.trim();
            !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        })
        .collect()
}

#[test]
fn every_key_it_writes_is_a_key_the_roster_serves_however_the_walk_was_answered() {
    // THE ANTI-DRIFT FENCE. The text is compiled into the binary, so
    // nothing else reads it and a key renamed in the schema would leave
    // the wizard writing a line that refuses the whole file the moment it
    // is uncommented. The scan is the shipped template's own, run over
    // both ends of the walk, and it reads the commented lines too.
    for text in [
        compose_config(&Answers::default()),
        compose_config(&every_feature_armed()),
    ] {
        parsed(&text);
        let found = crate::config::documented_keys_the_roster_serves(&text);
        // EVERY KEY-SHAPED LINE REACHED THE SCAN, which is the half a bare
        // count cannot state. The scan checks what it recognises and says
        // nothing about the rest, so a key misspelled past it (`apiKey`
        // for `api_key`, or `enabled=true` without the spaces the scan
        // splits on) is documented, unserved, and silently unchecked: the
        // operator uncomments it and the whole file stops loading.
        let shaped = key_shaped_lines(&text);
        assert!(
            !shaped.is_empty(),
            "the text documents no key at all: {text}"
        );
        assert_eq!(
            found,
            shaped.len(),
            "a key-shaped line never reached the roster scan; the scan read {found} of these {}:\n{}",
            shaped.len(),
            shaped.join("\n")
        );
    }
}

#[test]
fn a_wizard_render_carries_no_chezmoi_action_because_every_answer_is_a_literal() {
    // `Answers` NEVER PRODUCE A SECRET MARKER: every credential the walk
    // collects is a plain string handed straight to `values()`, so a
    // wizard's own composed text is real TOML from the first line, never
    // a template `render` fills in only after chezmoi runs.
    for text in [
        compose_config(&Answers::default()),
        compose_config(&every_feature_armed()),
    ] {
        assert!(!text.contains("{{"), "{text}");
    }
}
