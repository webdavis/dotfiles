use super::*;

// --- the catch-up replay ----------------------------------------------------

/// An event the operator is PRESENT for: at the desk with the origin pane out
/// of sight, which is the matrix row that earns a banner and so the row a
/// replay rides on.
///
/// AWAY IS DELIBERATELY NOT IT, and away is what the bare sandbox gives:
/// away is where misses are made and never where they are delivered, so an
/// away event is the one row that must NOT flush the queue.
pub(super) fn present_event(sandbox: &Sandbox) -> std::process::Command {
    let mut command = logged_event(sandbox);
    command.env("PNS_IDLE_SECS", "0");
    sandbox.stub_herdr(&mut command, false);
    command
        .args(["--agent", "claude", "--state", "done"])
        .args(["--detail", "the live turn", "--pane", "t1:p2"]);
    command
}

/// Channels that record EVERY event they are handed rather than only the last.
///
/// THE SANDBOX'S OWN STUBS TRUNCATE, which is right for a suite asking whether
/// a channel fired at all and useless here: a replay is a SECOND notification
/// on the same channel, and a truncating stub shows one file either way.
pub(super) fn record_every_event(sandbox: &Sandbox) {
    for channel in ["mobile", "hermes", "macos-banner"] {
        sandbox.stub_channel(
            channel,
            &format!("cat >>\"{}/{channel}.events\"", sandbox.display()),
        );
    }
}

/// Everything the state directory holds, sorted. A claim file the run left
/// behind shows up here and nowhere else.
pub(super) fn state_files(sandbox: &Sandbox) -> Vec<String> {
    let mut held: Vec<String> = std::fs::read_dir(sandbox.path("state"))
        .expect("the state dir")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    held.sort();
    held
}

/// The three dispatched channels with the catch-up card switched off, which
/// is the only `[recap]` key PR 1 reads.
pub(super) fn card_switched_off() -> String {
    format!("{EVERY_DISPATCHED_CHANNEL}[recap]\nreplay_card = false\n")
}

// --- the operating system's own mute ----------------------------------------

/// The mode this operator really leaves on, and the name Control Center shows
/// for it. A CUSTOM MODE, deliberately: the identifier says nothing about the
/// name, so a test using `Sleep` for both would pass with the catalog read
/// deleted.
pub(super) const A_CUSTOM_FOCUS: &str = "com.apple.donotdisturb.mode.graduationcapfill";

pub(super) const ITS_NAME: &str = "Casually Concerned";

/// The three stub channels plus the Focus policy, which is the only shape
/// these two tests differ in.
pub(super) fn focus_config(silence: &str) -> String {
    format!(
        "[plugins.mobile]\nenabled = true\ntype = \"moshi\"\n[plugins.hermes]\nenabled = true\n\
         [plugins.macos-banner]\nenabled = true\n[focus]\nsilence = [{silence}]\n"
    )
}

/// A present event WITH ALL THREE DECORATIONS REALLY ON THE TABLE, which is
/// what makes "the Focus held them" a claim that can fail.
///
/// AT THE DESK THE CARD IS OFF AND A SHORT COMMAND RAISES NO PULSE, so a bare
/// `present_event` asserted against a silenced card and a silenced pulse is
/// asserting what the surface had already decided: the Focus clause could be
/// deleted outright and both would still read as held. `PNS_FORCE_PHONE` puts
/// the card back on the plan and `--long-running` puts the pulse there, and
/// the sibling test below shows all three firing in this same world.
///
/// THE FORCE IS ALSO THE POINT, not just the setup. It is a producer's opinion
/// set in the environment, and a Focus the operator named has to beat it: that
/// arbitration order lived only in an engine unit test until this world
/// existed to run it through the process.
pub(super) fn focus_event(sandbox: &Sandbox) -> std::process::Command {
    let mut command = present_event(sandbox);
    command.env("PNS_FORCE_PHONE", "1").arg("--long-running");
    command
}

/// How many present events race for one planted journal.
///
/// WHAT THIS PIN IS AND IS NOT, measured rather than claimed. On the build
/// below it is a hard assertion: every run delivers exactly one replay. As a
/// hunt for a build that claims by READING AND THEN REMOVING it is a
/// probability, because that mutant only loses when two runs land inside the
/// same few microseconds: it died in one run of five here at eight racers and
/// two of eight at twenty four, so raising the count does not buy
/// determinism (the spawn spread grows with it, and the arrivals stay just as
/// thin). Eight is kept because it is the cheaper of two equal odds, and
/// because the standing assertion, not the mutant, is what this test is for.
pub(super) const RACERS: usize = 8;

/// The three dispatched channels with the RECAP switched off, which is how a
/// test about something else keeps a loud fixture from earning one.
///
/// EIGHT SIMULTANEOUS EVENTS ARE NOT A WINDOW. The racing tests below stamp
/// every event with one second, so the last racer to run can count all eight
/// inside a window a few milliseconds wide and earn a recap for an absence
/// that never happened. Sequentially the marker moves on every present event
/// and the count never leaves single figures, which is what
/// `the_marker_advances_when_the_recap_fires_so_a_second_event_recaps_nothing`
/// pins; here the recap is simply not what is being measured.
pub(super) fn recap_switched_off() -> String {
    format!("{EVERY_DISPATCHED_CHANNEL}[recap]\ndigest = false\n")
}

/// MORE RACERS THAN THE JOURNAL TEST USES: they all take the SAME adoption
/// path, and the window this hunts is a few microseconds wide, so the odds a
/// pair lands inside it come from the number of pairs.
pub(super) const ADOPTERS: usize = 24;

// --- the activity window and the recap --------------------------------------

/// The activity ring's own depth, stated here rather than imported: a test that
/// read the constant it is checking would agree with any value the source held.
pub(super) const ACTIVITY_KEPT: usize = 150;

/// The activity ring, oldest first, which is the order an append leaves it in.
pub(super) fn activity(sandbox: &Sandbox) -> Vec<String> {
    stored_records::lines(sandbox, "activity")
}

/// The activity ring's own field cap, stated here for the same reason its
/// depth is.
pub(super) const ACTIVITY_MAX_CHARS: usize = 120;

/// The shared read ceiling the decision ring and the journal use, stated here
/// so the fixture below can prove it is exceeded.
pub(super) const SHARED_READ_MAX: u64 = 256 * 1024;

/// The activity ring's path, with the state directory that holds it already
/// made, so a test can plant something there before the first event runs.
pub(super) fn activity_path(sandbox: &Sandbox) -> std::path::PathBuf {
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    sandbox.path("state/activity")
}

/// The WORST-CASE ring the depth was sized against: every text field but the
/// index is the full field cap of CONTROL BYTES, which the writer escapes at
/// six bytes each. Built through `serde_json` rather than by hand, so the
/// fixture is escaped exactly the way the engine escapes what it writes.
pub(super) fn escape_heavy_activity(count: usize) -> String {
    let padding = "\u{1b}".repeat(ACTIVITY_MAX_CHARS);
    (0..count)
        .map(|which| {
            format!(
                "{}\n",
                serde_json::json!({
                    "at": 1_756_499_000_u64,
                    "agent": padding,
                    "state": padding,
                    "project": format!("planted {which}"),
                    "branch": padding,
                    "detail": padding,
                })
            )
        })
        .collect()
}
