use super::*;

// --- the routing grammar -------------------------------------------------

#[test]
fn a_declaration_at_any_of_the_three_levels_reads_the_same_three_keys() {
    // ONE VOCABULARY FOR THREE LEVELS, because a lamp, a room and a zone
    // answer the same questions and differ only in how specific they are.
    for level in ["lamp", "room", "zone"] {
        let held = lights(&format!(
            "[lights.{level}.\"3F - Studio\"]\n\
                 shows = [\"done\", \"failed\"]\n\
                 dim_window = \"22:00-07:00\"\n\
                 dim_behaviours = [\"blocked\", \"unread\", \"loop\"]\n"
        ));
        let table = match level {
            "lamp" => &held.lamps,
            "room" => &held.rooms,
            _ => &held.zones,
        };
        assert_eq!(
            table.get("3F - Studio"),
            Some(&Target {
                shows: Some(vec![Behaviour::Done, Behaviour::Failed]),
                dim_window: Some("22:00-07:00".to_string()),
                dim_behaviours: vec![Behaviour::Blocked, Behaviour::Unread, Behaviour::Looping],
            }),
            "at the {level} level"
        );
    }
}

#[test]
fn a_declaration_that_states_nothing_states_nothing_rather_than_defaulting() {
    // `None` IS "SAID NOTHING", which a plain `Vec` could not spell: it is
    // what lets a lamp state which behaviours it carries and inherit its
    // room's window, and what tells a deliberate empty list from silence.
    let silent = lights("[lights.lamp.\"HCL1\"]\n");
    assert_eq!(silent.lamps["HCL1"], Target::default());
    assert_eq!(silent.lamps["HCL1"].shows, None);
    let emptied = lights("[lights.lamp.\"HCL1\"]\nshows = []\n");
    assert_eq!(
        emptied.lamps["HCL1"].shows,
        Some(Vec::new()),
        "an empty list is an OVERRIDE, which is how one lamp is taken out of a \
             routed room"
    );
}

#[test]
fn a_behaviour_word_the_lamps_do_not_speak_is_refused_with_the_closed_set_named() {
    // THE REFUSAL LISTS THE WHOLE SET, which is worth the extra words here:
    // the failure it prevents is a lamp that stays dark while the operator
    // is sure they routed it, and their only evidence is a lamp doing
    // nothing.
    for key in ["shows", "dim_behaviours"] {
        let said = refusal(&format!(
            "[lights.room.\"3F - Studio\"]\n{key} = [\"breathing\"]\n"
        ));
        assert_eq!(
            said,
            format!(
                "`lights.room.3F - Studio` key `{key}` names `breathing`, which is \
                     no behaviour; the lamps say done, failed, blocked, unread, loop"
            ),
        );
    }
}

#[test]
fn dim_behaviours_with_no_window_to_run_them_in_is_refused_rather_than_read_and_dropped() {
    // NO DEAD KNOBS, which is the config ruling applied to the one pair of
    // keys that can be half written. The enables RIDE the window, so a
    // declaration naming which behaviours run dimmed and never saying when
    // is a list nothing will ever read: the operator gets a lamp that
    // strobes all night and a file that says it should not.
    for stated in ["[\"blocked\"]", "[]"] {
        assert_eq!(
            refusal(&format!(
                "[lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n\
                     dim_behaviours = {stated}\n"
            )),
            "`lights.room.3F - Studio` states `dim_behaviours` with no \
                 `dim_window` for them to run in, so nothing would ever read them",
            "dim_behaviours = {stated}"
        );
    }
    // AN EMPTY LIST BESIDE A WINDOW IS THE BEDROOM RULE and stays legal:
    // the refusal is about a missing window, never about an empty list.
    assert!(
        parse_config(
            "[lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n\
                 dim_window = \"22:00-07:00\"\ndim_behaviours = []\n"
        )
        .is_ok()
    );
}

#[test]
fn an_unknown_declaration_key_is_refused_by_name_with_the_path_the_operator_wrote() {
    // THE PATH THEY WROTE, not the roster's own row: an operator told
    // `lights.<level>` has no `dim_hours` would go looking for a table they
    // never typed.
    let said = refusal("[lights.room.\"3F - Studio\"]\ndim_hours = \"22:00-07:00\"\n");
    assert!(
        said.contains("`lights.room.3F - Studio` key `dim_hours`"),
        "{said}"
    );
    assert!(
        said.contains("dim_behaviours, dim_window, shows"),
        "and it lists what the level does serve: {said}"
    );
}

#[test]
fn a_declaration_that_is_not_a_table_of_settings_is_refused_by_name() {
    for written in [
        "[lights]\nlamp = { \"HCL1\" = 3 }\n",
        "[lights]\nroom = 3\n",
        "[lights]\nzone = \"Upstairs\"\n",
    ] {
        let said = refusal(written);
        assert!(!said.is_empty(), "{written:?} must be refused: {said}");
    }
}
