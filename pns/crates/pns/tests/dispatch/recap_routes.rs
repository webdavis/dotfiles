use super::*;

#[test]
fn a_machine_with_no_durable_route_never_points_a_card_at_a_recap_nothing_can_carry() {
    // "recap in #<route>" IS A PROMISE, and a spawn alone cannot back it. A
    // started child still posts nothing when there is no durable channel: the
    // hermes leg answers Failed before it touches the network and the child
    // exits 0, so the phone pointed at a recap and the channel stayed empty.
    //
    // ASKED OF THE SELECTION, which is the one reading dispatch takes too, so
    // the promise on the card and the channel behind it cannot disagree. TWO
    // MACHINES HAVE NOWHERE FOR A RECAP TO GO and both are honest: one whose
    // config NAMES a roster without hermes, which is this test, and one with
    // no usable config at all, since hermes needs a route signed for before
    // it can carry anything and so is not in the core.
    let sandbox = Sandbox::new("recap-no-durable-route");
    record_every_event(&sandbox);
    sandbox.write_config(
        "[plugins.phone]\nenabled = true\ntype = \"moshi\"\n[plugins.banner]\nenabled = true\n",
    );
    loud_window(&sandbox);

    run(&mut present_event(&sandbox));

    let raised = events(&sandbox, "banner");
    assert_eq!(raised.len(), 2, "the live event and one card: {raised:?}");
    let body = raised[1]["detail"].as_str().expect("a detail");
    assert!(
        !body.contains("recap in #"),
        "the card pointed at a recap no channel could carry: {body}"
    );
    assert!(
        body.starts_with("2 missed notifications. "),
        "and what it delivered instead is slice 13's card, unchanged: {body}"
    );
    assert!(
        !sandbox.fired("hermes"),
        "a recap reached a route the config turned off: {:?}",
        events(&sandbox, "hermes")
    );
}

#[test]
fn the_marker_advances_so_a_second_present_event_recaps_nothing() {
    // IDEMPOTENCE, as locked. Without the advance the second event counts the
    // same loud window and posts the same recap again, which is the exact
    // failure the marker exists to prevent. The two events run back to back
    // over ONE window, and exactly one recap may come out of it.
    let sandbox = Sandbox::new("recap-idempotent");
    record_every_event(&sandbox);
    loud_window(&sandbox);

    run(&mut present_event(&sandbox));
    run(&mut present_event(&sandbox));

    let raised = events(&sandbox, "banner");
    assert_eq!(
        raised.len(),
        3,
        "two live events and ONE card between them: {raised:?}"
    );
    assert_eq!(
        raised
            .iter()
            .filter(|event| event["state"] == "missed")
            .count(),
        1,
        "the second event carded the same window again: {raised:?}"
    );
    // AND THE DISCORD HALF IS COUNTED THE SAME WAY, polled so a second child
    // that was slower than the first still fails this.
    poll_until(|| {
        events(&sandbox, "hermes")
            .into_iter()
            .find(|event| event["state"] == "recap")
    })
    .unwrap_or_else(|| panic!("the first event posted no recap at all"));
    assert_eq!(
        events(&sandbox, "hermes")
            .iter()
            .filter(|event| event["state"] == "recap")
            .count(),
        1,
        "the same window was recapped twice: {:?}",
        events(&sandbox, "hermes")
    );
}

#[test]
fn a_recap_told_a_window_it_cannot_read_prints_usage_exits_two_and_posts_nothing() {
    // A MODE, NOT A HOOK, so a typo is a refusal rather than a silent exit 0:
    // this is hand-runnable, and a recap the operator believes was posted is
    // worse than one that said it could not be. `event_mode` is what it used to
    // fall through to, which would have sent a notification about nothing.
    let sandbox = Sandbox::new("recap-usage");
    let output = logged_event(&sandbox)
        .args([
            "recap",
            "--since-epoch",
            "yesterday",
            "--until-epoch",
            "1756500000",
        ])
        .output()
        .expect("the engine runs");

    assert_eq!(output.status.code(), Some(2), "stderr: {}", stderr(&output));
    assert!(
        stderr(&output).contains("pns recap --since-epoch <epoch> --until-epoch <epoch>"),
        "the usage names both bounds: {}",
        stderr(&output)
    );
    for channel in ["hermes", "phone", "banner"] {
        assert!(
            !sandbox.fired(channel),
            "{channel} was handed a recap over a window nobody could read"
        );
    }
}

/// A log carried by the native Discord bot, with the two keys config load
/// requires and nothing else.
pub(super) const DISCORD_LOG: &str = "[plugins.phone]\nenabled = true\ntype = \"moshi\"\n\
    [plugins.log]\nenabled = true\ntype = \"discord\"\nbot_token = \"token\"\n\
    [plugins.log.channels]\ndefault = \"1\"\npriority = \"2\"\n\
    [plugins.banner]\nenabled = true\n[failures]\npage_enabled = false\n";

#[test]
fn a_discord_log_carries_the_return_recap_and_hermes_is_never_handed_it() {
    // `--to durable` NAMES THE ROLE, and `[plugins.log] type` is what fills
    // it: a child that took the first durable plugin registered posted every
    // recap through hermes whatever the config chose.
    let sandbox = Sandbox::new("recap-discord-log");
    record_every_event(&sandbox);
    sandbox.stub_channel(
        "discord",
        &format!("cat >>\"{}/discord.events\"", sandbox.display()),
    );
    sandbox.write_config(DISCORD_LOG);
    loud_window(&sandbox);

    run(&mut present_event(&sandbox));

    poll_until(|| {
        events(&sandbox, "discord")
            .into_iter()
            .find(|event| event["state"] == "recap")
    })
    .unwrap_or_else(|| {
        panic!(
            "no recap reached discord: {:?}",
            events(&sandbox, "discord")
        )
    });
    assert!(
        events(&sandbox, "hermes").is_empty(),
        "the recap went to a transport the config did not select: {:?}",
        events(&sandbox, "hermes")
    );
}

#[test]
fn a_discord_card_names_the_channel_key_its_recap_posts_under() {
    // THE RECAP TAKES ITS PROJECT FROM THE CHECKOUT IT WAS COMPOSED IN, and
    // the Discord map tries that project's key before the default route's, so
    // the card names the key the recap actually landed under.
    let sandbox = Sandbox::new("recap-discord-pointer");
    record_every_event(&sandbox);
    sandbox.stub_channel(
        "discord",
        &format!("cat >>\"{}/discord.events\"", sandbox.display()),
    );
    sandbox.write_config(&DISCORD_LOG.replace(
        "priority = \"2\"\n",
        "priority = \"2\"\ndotfiles = \"3\"\npns-events = \"4\"\n",
    ));
    loud_window(&sandbox);
    let checkout = checkout_named(&sandbox, "dotfiles");

    run(present_event(&sandbox).current_dir(&checkout));

    let (card, raised) = carded_recap(&sandbox);
    assert!(
        card["detail"]
            .as_str()
            .is_some_and(|detail| detail.ends_with("recap in #dotfiles")),
        "{raised:?}"
    );
    let recap = poll_until(|| {
        events(&sandbox, "discord")
            .into_iter()
            .find(|event| event["state"] == "recap")
    })
    .unwrap_or_else(|| {
        panic!(
            "no recap reached discord: {:?}",
            events(&sandbox, "discord")
        )
    });
    assert_eq!(recap["project"], "dotfiles", "{recap:?}");
}

/// A repository with one empty commit, in a directory named `name` inside
/// the sandbox.
fn checkout_named(sandbox: &Sandbox, name: &str) -> std::path::PathBuf {
    let checkout = sandbox.path(name);
    std::fs::create_dir_all(&checkout).expect("the checkout directory");
    for arguments in [
        &["init", "--quiet"][..],
        &["commit", "--quiet", "--allow-empty", "-m", "root"][..],
    ] {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(&checkout)
            .args(arguments)
            // A HOOK EXPORTS GIT_DIR, which would point these at the real
            // repository instead of the sandbox.
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .status()
            .expect("git runs");
        assert!(status.success(), "git {arguments:?}");
    }
    checkout
}
