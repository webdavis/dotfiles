use super::fixtures::*;

#[test]
fn a_line_names_the_event_and_every_gate_input_behind_one_epoch_second() {
    // EVERY VALUE IS A NUMBER, A BOOLEAN OR AN ENUM NAME, so the only
    // reader this file has can print it without interpreting it, and a
    // reading nobody could take stays absent instead of becoming a zero.
    let plain = decision(inputs());
    let overrides = Overrides {
        skip_phone: true,
        ..Overrides::default()
    };
    assert_eq!(
        line(&Record {
            event: &event(),
            decision: &plain,
            overrides: &overrides,
            legs: &[],
            nag: false,
            permission_mode: "",
            agent_id: "",
            tool_name: "",
        }),
        "1756500000 claude/blocked mode=none agent=none tool=none surface=Mobile visibility=Hidden \
         session_visibility=Visible desk_age=none phone_age=12 tap_age=none locked=no \
         fresh_window=120 long_running=no nag=no local_only=no remote_only=no pane=present \
         pane_dropped=no watch_card=no muted=no focus=no skip_phone=yes force_phone=no \
         idle_invalid=no desk_invalid=no phone_invalid=no \
         plan=banner:no,card:no,pulse:no legs=none"
    );

    // AN UNREAD LOCK IS ITS OWN ROW, byte for byte. A `locked=no` here
    // would be the line claiming the display was awake on a reading the
    // decision never took, which is the one thing `tri` exists to stop.
    let unread_lock = decision(GateInputs {
        screen_locked: None,
        ..inputs()
    });
    assert_eq!(
        line(&Record {
            event: &event(),
            decision: &unread_lock,
            overrides: &overrides,
            legs: &[],
            nag: false,
            permission_mode: "",
            agent_id: "",
            tool_name: "",
        }),
        "1756500000 claude/blocked mode=none agent=none tool=none surface=Mobile visibility=Hidden \
         session_visibility=Visible desk_age=none phone_age=12 tap_age=none locked=none \
         fresh_window=120 long_running=no nag=no local_only=no remote_only=no pane=present \
         pane_dropped=no watch_card=no muted=no focus=no skip_phone=yes force_phone=no \
         idle_invalid=no desk_invalid=no phone_invalid=no \
         plan=banner:no,card:no,pulse:no legs=none"
    );
}

#[test]
fn a_line_carries_the_payloads_mode_agent_and_tool_or_says_none() {
    // WHY: three `claude/blocked` events lined up with subagent
    // hand-offs, not with any prompt the operator saw (OBS-4), and the
    // decision log had no field that could ever tell those apart from an
    // ordinary approval.
    let plain = decision(inputs());
    let recorded = line(&Record {
        event: &event(),
        decision: &plain,
        overrides: &Overrides::default(),
        legs: &[],
        nag: false,
        permission_mode: "bypassPermissions",
        agent_id: "agent_01",
        tool_name: "Bash",
    });
    assert!(
        recorded.contains(" mode=bypassPermissions agent=agent_01 tool=Bash "),
        "got {recorded}"
    );

    // AND EVERY FIELD A PAYLOAD DID NOT STATE READS `none`, never a blank:
    // an empty field in the middle of a space-delimited line is
    // indistinguishable from a line one field short.
    let recorded = line(&Record {
        event: &event(),
        decision: &plain,
        overrides: &Overrides::default(),
        legs: &[],
        nag: false,
        permission_mode: "",
        agent_id: "",
        tool_name: "",
    });
    assert!(
        recorded.contains(" mode=none agent=none tool=none "),
        "got {recorded}"
    );
}

#[test]
fn a_payload_field_outside_the_printable_allowlist_is_recorded_as_unprintable() {
    // THE THREE PAYLOAD FIELDS ARE HARNESS TEXT, and `tool_name` is remote
    // text a connected MCP server chose. A NEWLINE forges a second entry
    // in a one-record-per-line file; an escape sequence reaches the
    // terminal `pns doctor` prints to; a space is what the reader splits
    // on. Each is refused the way `agent` and `state` refuse it.
    let recorded = line(&Record {
        event: &event(),
        decision: &decision(inputs()),
        overrides: &Overrides::default(),
        legs: &[],
        nag: false,
        permission_mode: "bypass Permissions",
        agent_id: "\u{1b}[31magent_01",
        tool_name: "Bash\n1756500000 claude/done",
    });
    assert!(
        recorded.contains(" mode=unprintable agent=unprintable tool=unprintable "),
        "got {recorded}"
    );
    assert!(!recorded.contains('\n'), "a newline forged a second entry");
}

#[test]
fn a_line_with_no_readable_clock_leads_with_a_dash_rather_than_epoch_zero() {
    // A RECOGNIZED VALUE, so the reader can tell it from a line it could
    // not parse. Epoch zero would parse cleanly and render as 56 years
    // ago, which is a claim nobody measured.
    let decision = decision(GateInputs {
        now_secs: None,
        ..inputs()
    });
    let recorded = line(&Record {
        event: &event(),
        decision: &decision,
        overrides: &Overrides::default(),
        legs: &[],
        nag: false,
        permission_mode: "",
        agent_id: "",
        tool_name: "",
    });
    assert!(
        recorded.starts_with("- claude/blocked "),
        "got {recorded:?}"
    );
}

#[test]
fn no_free_text_reaches_a_line_and_the_pane_appears_only_as_two_booleans() {
    // THE OPERATOR'S OWN CONTENT: a tool call, a reply, a working
    // directory, a branch name. `pns doctor` PRINTS this file, so anything
    // recorded here lands in a state file and then on a terminal. Every
    // field of `EventArgs` outside `agent` and `state` is checked, the
    // narrowing flags included, because they reach the line through the
    // decision's own inputs and never through the event.
    let event = EventArgs {
        agent: "claude".to_string(),
        state: "blocked".to_string(),
        project: "SECRETPROJECT".to_string(),
        branch: "SECRETBRANCH".to_string(),
        detail: "SECRETDETAIL".to_string(),
        pane: "wW:pSECRETPANE".to_string(),
        channel: "SECRETCHANNEL".to_string(),
        local_only: true,
        remote_only: true,
        long_running: true,
        help: false,
    };
    let decision = decision(GateInputs {
        pane_present: true,
        ..inputs()
    });
    let recorded = line(&Record {
        event: &event,
        decision: &decision,
        overrides: &Overrides::default(),
        legs: &[],
        nag: false,
        permission_mode: "",
        agent_id: "",
        tool_name: "",
    });
    for content in [
        "SECRETPROJECT",
        "SECRETBRANCH",
        "SECRETDETAIL",
        "SECRETPANE",
        "wW",
        "SECRETCHANNEL",
    ] {
        assert!(
            !recorded.contains(content),
            "{content} reached the record: {recorded}"
        );
    }
    assert!(
        recorded.contains(" pane=present pane_dropped=no "),
        "{recorded}"
    );
}

#[test]
fn an_agent_or_state_outside_the_printable_allowlist_is_recorded_as_unprintable() {
    // THE TWO VALUES THAT COME FROM ARGV, and the only text in a line.
    // A NEWLINE IS THE DANGEROUS ONE: this file is one record per line, so
    // a value carrying one forges a second entry. An escape sequence is the
    // other: `pns doctor` prints these to a terminal.
    let identity = |agent: &str, state: &str| {
        let event = EventArgs {
            agent: agent.to_string(),
            state: state.to_string(),
            ..EventArgs::default()
        };
        let decision = decision(inputs());
        let recorded = line(&Record {
            event: &event,
            decision: &decision,
            overrides: &Overrides::default(),
            legs: &[],
            nag: false,
            permission_mode: "",
            agent_id: "",
            tool_name: "",
        });
        recorded
            .split_whitespace()
            .nth(1)
            .unwrap_or_default()
            .to_string()
    };

    assert_eq!(identity("claude", "blocked"), "claude/blocked");
    assert_eq!(identity("codex-2.tui_1", "done"), "codex-2.tui_1/done");
    // NO SPACE IN EITHER: a space is refused on its own, which would
    // mask the newline this row is actually about.
    for forged in ["claude\n1756500000", "claude\rfake"] {
        assert_eq!(
            identity(forged, "done"),
            "unprintable/done",
            "a newline forges a second entry"
        );
    }
    assert_eq!(identity("claude", "\u{1b}[31mred"), "claude/unprintable");
    assert_eq!(identity("cl aude", "done"), "unprintable/done");
    assert_eq!(identity("clau\u{00e9}de", "done"), "unprintable/done");
    // AN EMPTY VALUE is absent, not unprintable: a bare `pns` names no
    // agent and no state, and there is nothing there to refuse.
    assert_eq!(identity("", ""), "none/none");
    // AN OVER-LONG ONE IS TRUNCATED, and the allowlist runs over the whole
    // value first, so a truncation can never land mid-character.
    assert_eq!(
        identity(&"a".repeat(40), "done"),
        format!("{}/done", "a".repeat(32))
    );
    // AND THE ORDER IS JUDGE THEN TRUNCATE, never the reverse. A clean
    // 32-character head with a newline at position 40 passes any check
    // that runs on the cut value, and the cut value is then written as a
    // real agent name while the forged entry rides in behind it. Cutting
    // first is also a panic hazard the moment a cut lands mid-character.
    assert_eq!(
        identity(
            &format!("{}\n1756500000 forged/entry", "a".repeat(40)),
            "done"
        ),
        "unprintable/done",
        "the tail is judged too, not only the 32 characters that survive"
    );
}

#[test]
fn a_line_carries_the_arbitrated_plan_and_each_legs_verdict() {
    // WITHOUT THE LEGS the log says pns decided to card the operator while
    // their question is why no card appeared. THE VERDICT IS THE VARIANT
    // NAME, never the channel's sentence, which can carry a status code or
    // a URL.
    let carded = decision(inputs());
    let carded = Decision {
        plan: DeliveryPlan {
            banner: false,
            phone_card: true,
            pulse: false,
        },
        ..carded
    };
    // THE DECORATION FLAG IS THE ROSTER'S OWN: the phone and the banner
    // show the operator something, the durable log and an unknown channel
    // do not. Nothing in a ring line reads it, which is exactly why it is
    // stated honestly here rather than defaulted.
    let legs = [
        (
            Leg {
                name: "mobile",
                mode: ReportMode::Silent,
                decorative: true,
            },
            Delivery::Failed("the gateway answered 502 at https://example.invalid".to_string()),
        ),
        (
            Leg {
                name: "hermes",
                mode: ReportMode::Silent,
                decorative: false,
            },
            Delivery::Delivered("posted".to_string()),
        ),
        (
            Leg {
                name: "macos-banner",
                mode: ReportMode::Silent,
                decorative: true,
            },
            Delivery::Silent,
        ),
        (
            Leg {
                name: "kitchen",
                mode: ReportMode::Silent,
                decorative: false,
            },
            Delivery::Unlaunched("no such channel".to_string()),
        ),
    ];
    let recorded = line(&Record {
        event: &event(),
        decision: &carded,
        overrides: &Overrides::default(),
        legs: &legs,
        nag: false,
        permission_mode: "",
        agent_id: "",
        tool_name: "",
    });
    assert!(
        recorded.ends_with(
            " plan=banner:no,card:yes,pulse:no \
             legs=mobile:failed,hermes:delivered,macos-banner:silent,kitchen:unlaunched"
        ),
        "got {recorded}"
    );
    assert!(
        !recorded.contains("502"),
        "the sentence stays out: {recorded}"
    );
    assert!(
        !recorded.contains("example.invalid"),
        "and its URL with it: {recorded}"
    );

    // A PLAN THAT REACHED NO CHANNEL still records, and says so.
    let recorded = line(&Record {
        event: &event(),
        decision: &decision(inputs()),
        overrides: &Overrides::default(),
        legs: &[],
        nag: false,
        permission_mode: "",
        agent_id: "",
        tool_name: "",
    });
    assert!(
        recorded.ends_with(" plan=banner:no,card:no,pulse:no legs=none"),
        "got {recorded}"
    );
}
