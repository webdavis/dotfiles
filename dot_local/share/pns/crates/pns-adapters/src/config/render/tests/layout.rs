use super::*;

#[test]
fn render_walks_every_layout_table_and_writes_no_heading_outside_it() {
    // LAYOUT IS THE SOURCE OF TRUTH: every table it declares must show up
    // in the render, commented or live, and every heading the render
    // writes must be one LAYOUT actually declares. A `render` that
    // enumerates tables by hand rather than walking LAYOUT can drift from
    // this without any test noticing.
    let text = render(&toml::Table::new()).expect("an empty walk still renders");
    let layout_names: std::collections::HashSet<&str> =
        super::LAYOUT.iter().map(|table| table.name).collect();

    for table in super::LAYOUT {
        if table.name.starts_with("lights.") {
            continue; // governed by the single [lights] presence flag
        }
        let live = format!("[{}]\n", table.name);
        let commented = format!("# [{}]\n", table.name);
        assert!(
            text.contains(&live) || text.contains(&commented),
            "`{}` from LAYOUT never appears in the render: {text}",
            table.name
        );
    }

    for line in text.lines() {
        let heading = line.strip_prefix("# [").or_else(|| line.strip_prefix('['));
        let Some(heading) = heading.and_then(|rest| rest.strip_suffix(']')) else {
            continue;
        };
        if heading.contains('"') {
            continue; // a lamp/room/zone target declaration, not a LAYOUT table
        }
        assert!(
            layout_names.contains(heading),
            "the render wrote a heading `{heading}` LAYOUT never declares"
        );
    }
}

#[test]
fn every_layout_table_matches_the_config_roster_exactly_in_both_directions() {
    // EVERY LAYOUT TABLE IS ONE THE ROSTER SERVES, and with the SAME key
    // set: the layout is a second statement of `config`'s own vocabulary,
    // and this is what stops the two from drifting apart.
    //
    // `lights` ITSELF IS THE ONE EXCEPTION, and only because `config`
    // reads it as one flat table where this layout writes seven headings:
    // the roster's `done`, `failed`, `blocked`, `unread`, `loop` and `dim`
    // are each a SEPARATE `lights.<name>` entry here, not a `Key` of
    // `lights`, and `lamp`, `room` and `zone` are the hardcoded
    // declaration branch. So `lights`'s effective key set is its own
    // `refresh_secs` plus the leaf name of every `lights.<x>` table this
    // layout declares, plus the three declaration levels.
    for table in super::LAYOUT {
        let (_, roster_keys) = crate::config::TABLE_KEYS
            .iter()
            .find(|(name, _)| *name == table.name)
            .unwrap_or_else(|| panic!("`{}` is not a table the roster serves", table.name));
        let mut layout_keys: Vec<&str> = table.keys.iter().map(|key| key.name).collect();
        if table.name == "lights" {
            layout_keys.extend(["lamp", "room", "zone"]);
            layout_keys.extend(
                super::LAYOUT
                    .iter()
                    .filter_map(|entry| entry.name.strip_prefix("lights.")),
            );
        }
        layout_keys.sort_unstable();
        layout_keys.dedup();
        let mut roster_keys = roster_keys.to_vec();
        roster_keys.sort_unstable();
        assert_eq!(
            layout_keys, roster_keys,
            "`{}` disagrees between the layout and the roster",
            table.name
        );
    }

    // AND EVERY ROSTER TABLE THE LAYOUT CAN REACH IS WRITTEN BY SOME
    // PATH: `TOP_LEVEL` has no heading of its own to write, and
    // `lights.<level>` is written by the hardcoded target-declaration
    // branch rather than a `Key` list, so both are the two named
    // exceptions rather than gaps.
    for (table, _) in crate::config::TABLE_KEYS.iter().copied() {
        if table == crate::config::TOP_LEVEL || table == crate::config::TARGET_KEYS {
            continue;
        }
        assert!(
            super::LAYOUT.iter().any(|entry| entry.name == table),
            "the roster serves `{table}` and the layout never writes it"
        );
    }
}

#[test]
fn the_hardcoded_target_declaration_branch_writes_every_target_key() {
    // THE HALF THE LAYOUT WALK CANNOT SEE: `lights.<level>` has no `Key`
    // list, so its coverage is proven by exercising the render path
    // directly rather than by a table lookup.
    let mut target = toml::Table::new();
    target.insert(
        "shows".to_string(),
        toml::Value::Array(vec![toml::Value::String("done".to_string())]),
    );
    target.insert(
        "dim_window".to_string(),
        toml::Value::String("22:00-07:00".to_string()),
    );
    target.insert(
        "dim_behaviours".to_string(),
        toml::Value::Array(vec![toml::Value::String("done".to_string())]),
    );
    let mut rooms = toml::Table::new();
    rooms.insert("Studio".to_string(), toml::Value::Table(target));
    let mut lights = toml::Table::new();
    lights.insert("room".to_string(), toml::Value::Table(rooms));
    let mut values = toml::Table::new();
    values.insert("lights".to_string(), toml::Value::Table(lights));

    let text = render(&values).expect("a full target declaration renders");
    let config = parse_config(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
    let studio = &config.lights.expect("lights was armed").rooms["Studio"];
    assert_eq!(studio.shows, Some(vec![crate::config::Behaviour::Done]));
    assert_eq!(studio.dim_window.as_deref(), Some("22:00-07:00"));
    assert_eq!(studio.dim_behaviours, vec![crate::config::Behaviour::Done]);
}

#[test]
fn the_target_declaration_key_roster_is_exactly_shows_dim_window_and_dim_behaviours() {
    // THE EXACT KEY SET, not merely "an unknown key is refused": a fourth
    // key added to `render_target`'s own hardcoded list would pass every
    // existing test without ever being asserted as belonging.
    let (_, roster_keys) = crate::config::TABLE_KEYS
        .iter()
        .find(|(name, _)| *name == crate::config::TARGET_KEYS)
        .expect("TARGET_KEYS is declared in the roster");
    let mut roster_keys = roster_keys.to_vec();
    roster_keys.sort_unstable();
    assert_eq!(roster_keys, ["dim_behaviours", "dim_window", "shows"]);
}
