use super::*;

/// The epoch the marker holds, or None when there is no marker at all.
pub(super) fn last_present(sandbox: &Sandbox) -> Option<u64> {
    stored_records::present(sandbox)
}

/// This machine's clock, which the fixtures below place themselves against.
pub(super) fn epoch_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock past 1970")
        .as_secs()
}

/// The last-present marker planted `ago` seconds back, which is the only thing
/// that opens a window at all.
pub(super) fn plant_marker(sandbox: &Sandbox, ago: u64) {
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    std::fs::write(
        sandbox.path("state/last-present"),
        format!("{}\n", epoch_now() - ago),
    )
    .expect("the marker");
}

/// An activity ring `count` entries deep, every one of them stamped `ago`
/// seconds back and carrying its own index. `urgent` names the index that is
/// `blocked` rather than `done`, which is how a test plants something that
/// still needs the operator.
pub(super) fn planted_activity(count: usize, ago: u64, urgent: Option<usize>) -> String {
    let at = epoch_now() - ago;
    (0..count)
        .map(|which| {
            let state = if urgent == Some(which) {
                "blocked"
            } else {
                "done"
            };
            format!(
                "{{\"at\":{at},\"agent\":\"claude\",\"state\":\"{state}\",\
                 \"project\":\"p{which}\",\"branch\":\"b\",\"detail\":\"planted {which}\"}}\n"
            )
        })
        .collect()
}

/// The engine's stated volume threshold, which every fixture below is built
/// around. Stated rather than imported, for the reason the depths are.
pub(super) const MIN_EVENTS: usize = 8;

/// A window loud enough to earn a recap: the marker an hour back, twelve
/// events half an hour back with one of them blocked, and two misses queued.
/// THE LIVE EVENT MAKES IT THIRTEEN, because the activity ring records every
/// event and this one is inside the window it opened.
pub(super) fn loud_window(sandbox: &Sandbox) {
    plant_marker(sandbox, 3600);
    std::fs::write(activity_path(sandbox), planted_activity(12, 1800, Some(4))).expect("the ring");
    std::fs::write(journal_path(sandbox), planted_journal(2)).expect("the journal");
}

/// The file the blocked recap stub waits on, so a test releases it rather than
/// timing it. Named on the sandbox so a panic can still let the stub go.
pub(super) const RELEASE: &str = "the.test.is.over";

/// A hermes stub that PARKS on a recap and answers everything else at once.
///
/// THE OBSERVATION, and the reason it is a block rather than a sleep. The old
/// test ran the parent to completion and then polled, which a recap rendered
/// IN the parent satisfies just as well: `poll_until` returns immediately when
/// the answer is already there. VERIFIED by running the brief's own mutation
/// (the child's work done in-process instead of spawned): the whole suite
/// stayed green. A stub that will not return until this test says so cannot be
/// satisfied that way, because the parent's own exit is what gets asserted
/// while the recap is still parked inside the stub.
///
/// BOUNDED ANYWAY, at ten seconds, so a broken build fails rather than hangs.
pub(super) fn hermes_parks_on_the_recap(sandbox: &Sandbox) {
    sandbox.stub_channel(
        "hermes",
        &format!(
            "payload=$(cat)\ncase \"$payload\" in\n  *'\"state\":\"recap\"'*) \
             for _ in $(seq 1 200); do [ -e \"{root}/{RELEASE}\" ] && break; sleep 0.05; done ;;\n\
             esac\nprintf '%s\\n' \"$payload\" >>\"{root}/hermes.events\"",
            root = sandbox.display()
        ),
    );
}

/// The window's own claim, planted by hand for an owner this test chooses.
/// THE MARKER ITSELF IS NOT PLANTED BESIDE IT: a claim exists exactly when the
/// marker has been renamed out of the way, and a fixture holding both would be
/// a state the engine cannot produce.
pub(super) fn plant_window_claim(sandbox: &Sandbox, owner: u32, ago: u64) {
    std::fs::create_dir_all(sandbox.path("state")).expect("state dir");
    std::fs::write(
        sandbox.path(&format!("state/last-present.claim.{owner}")),
        format!("{}\n", epoch_now() - ago),
    )
    .expect("the claim");
}

/// A process id nothing is using: a child run to completion and reaped, so the
/// kernel has already answered for it. STATED BY THE MACHINE rather than
/// guessed at, because a made-up number can be live.
pub(super) fn a_reaped_pid() -> u32 {
    let mut child = std::process::Command::new("/usr/bin/true")
        .spawn()
        .expect("a child");
    let gone = child.id();
    child.wait().expect("the child is waitable");
    gone
}
