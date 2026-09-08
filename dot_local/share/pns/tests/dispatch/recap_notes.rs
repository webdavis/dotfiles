use super::*;

#[test]
fn only_the_notes_the_glob_names_and_the_window_covers_are_ever_read() {
    // THE GLOB IS THE WHOLE PERMISSION, and the window is the whole selection.
    // A note that changed before the operator left was already read by them; a
    // file the pattern does not name is not a review note at all; and a
    // directory the pattern does not name is not pns's to open. All three are
    // planted here, and only one of them may reach Discord.
    let sandbox = Sandbox::new("recap-notes");
    record_every_event(&sandbox);
    sandbox.write_config(&format!(
        "{EVERY_DISPATCHED_CHANNEL}[recap]\nreview_notes = \"~/notes/checklist-*.md\"\n"
    ));
    loud_window(&sandbox);
    let now = epoch_now();
    write_note(
        &sandbox,
        "notes/checklist-inside.md",
        "# the claim protocol raced itself\n",
        now - 1800,
    );
    write_note(
        &sandbox,
        "notes/checklist-before.md",
        "# read before the operator left\n",
        now - 7200,
    );
    write_note(
        &sandbox,
        "notes/other.txt",
        "# not a checklist\n",
        now - 1800,
    );
    write_note(
        &sandbox,
        "elsewhere/checklist-outside.md",
        "# in a directory nobody named\n",
        now - 1800,
    );

    run(&mut present_event(&sandbox));

    let body = posted_recap(&sandbox);
    assert!(
        body.contains("- checklist-inside.md: the claim protocol raced itself"),
        "the one note inside the window is not the section: {body}"
    );
    for missed in ["checklist-before", "other.txt", "checklist-outside"] {
        assert!(!body.contains(missed), "{missed} was read: {body}");
    }
}

#[test]
fn a_glob_that_matches_nothing_says_so_and_one_pointing_nowhere_says_something_else() {
    // THREE STATES, THREE SENTENCES, and the operator calibrating a glob is the
    // one who needs them apart: "nobody told pns where to look", "the directory
    // you named is not there" and "it is there and held nothing in this window"
    // send them to three different places, and only the last one means the
    // night really had no findings in it.
    let sandbox = Sandbox::new("recap-notes-empty");
    record_every_event(&sandbox);
    sandbox.write_config(&format!(
        "{EVERY_DISPATCHED_CHANNEL}[recap]\nreview_notes = \"~/notes/checklist-*.md\"\n"
    ));
    loud_window(&sandbox);
    // THE DIRECTORY IS THERE AND THE PATTERN MATCHES NOTHING IN IT.
    write_note(
        &sandbox,
        "notes/other.txt",
        "# not a checklist\n",
        epoch_now(),
    );

    run(&mut present_event(&sandbox));

    let body = posted_recap(&sandbox);
    assert!(
        body.contains("CAUGHT BY REVIEW, AND IMPLEMENTED: nothing was noted"),
        "a glob that matched nothing read as something else: {body}"
    );

    // AND A GLOB POINTING AT A DIRECTORY NOBODY MADE is a config the operator
    // has to fix, not a quiet night.
    let nowhere = Sandbox::new("recap-notes-nowhere");
    record_every_event(&nowhere);
    nowhere.write_config(&format!(
        "{EVERY_DISPATCHED_CHANNEL}[recap]\nreview_notes = \"~/no-such-dir/checklist-*.md\"\n"
    ));
    loud_window(&nowhere);

    run(&mut present_event(&nowhere));

    let missing = posted_recap(&nowhere);
    assert!(
        missing.contains("CAUGHT BY REVIEW, AND IMPLEMENTED: unavailable"),
        "a directory nobody made read as a quiet night: {missing}"
    );
}

#[test]
fn a_note_that_matched_and_would_not_open_says_so_rather_than_vanishing() {
    // A NOTE PNS CANNOT READ IS STILL NEWS. It matched the operator's own
    // pattern and its clock puts it in the window, so dropping it renders a
    // night in which that finding never existed, which is exactly the claim
    // this section is not allowed to make.
    //
    // AND THE CAP CUTS THE OLDEST, NOT THE ALPHABETICALLY LAST: the newer note
    // comes first here, where sorting by name would have put `checklist-locked`
    // above `checklist-open`.
    let sandbox = Sandbox::new("recap-note-unreadable");
    record_every_event(&sandbox);
    sandbox.write_config(&format!(
        "{EVERY_DISPATCHED_CHANNEL}[recap]\nreview_notes = \"~/notes/checklist-*.md\"\n"
    ));
    loud_window(&sandbox);
    let now = epoch_now();
    write_note(
        &sandbox,
        "notes/checklist-locked.md",
        "# a finding nobody can open\n",
        now - 1800,
    );
    std::fs::set_permissions(
        sandbox.path("notes/checklist-locked.md"),
        std::os::unix::fs::PermissionsExt::from_mode(0o000),
    )
    .expect("the mode");
    write_note(
        &sandbox,
        "notes/checklist-open.md",
        "# a finding anybody can\n",
        now - 900,
    );

    run(&mut present_event(&sandbox));

    let body = posted_recap(&sandbox);
    let lines: Vec<&str> = body.lines().collect();
    let noted = lines
        .iter()
        .position(|line| line.starts_with("CAUGHT BY REVIEW"))
        .unwrap_or_else(|| panic!("no review section at all: {body}"));
    assert_eq!(
        lines[noted + 1..noted + 3],
        [
            "- checklist-open.md: a finding anybody can",
            "- checklist-locked.md: could not be read",
        ],
        "{body}"
    );
}

#[test]
fn a_summarized_merge_section_keeps_only_the_lines_its_own_sources_vouch_for() {
    // THE WHOLE PATH IN ONE TEST: a listing off `gh`, the merge prompt handed
    // to a real process, and the receipts check run over what that process
    // actually said. Every piece of it was covered on its own and the WIRING
    // between them was not, so forcing both external answers to None left the
    // suite green: no test proved a configured summarizer ever reached these
    // two sections at all.
    let sandbox = Sandbox::new("recap-merges-summarized");
    record_every_event(&sandbox);
    sandbox.write_config(&recap_summarized_by("repos = [\"webdavis/dotfiles\"]\n"));
    loud_window(&sandbox);

    let mut command = present_event(&sandbox);
    let listing = serde_json::json!([
        { "number": 213, "title": "a subject", "body": "## Summary\n\nthe first.\n" },
        { "number": 212, "title": "another subject", "body": "## Summary\n\nthe second.\n" },
    ]);
    stub_gh(&sandbox, &mut command, &format!("printf '%s' '{listing}'"));
    // ONE STUB, THREE QUESTIONS, ANSWERED APART. It replies to the merge
    // prompt only when it is handed the merge instruction, which is what
    // proves `merge_prompt` is what this section asked with rather than the
    // night's prompt reaching it by accident.
    sandbox.stub_on_path(
        &mut command,
        SUMMARIZER,
        "case \"$(cat)\" in\n  *'pull requests merged'*) printf '%s\\n' \
         '#213 the recap names what shipped' 'and this line cites nothing' ;;\n  \
         *) printf '%s\\n' 'the night, in one line' ;;\nesac",
    );
    run(&mut command);

    let body = posted_recap(&sandbox);
    let lines: Vec<&str> = body.lines().collect();
    let shipped = lines
        .iter()
        .position(|line| *line == "NEW BEHAVIOR")
        .unwrap_or_else(|| panic!("no NEW BEHAVIOR section at all: {body}"));
    assert_eq!(
        lines[shipped..shipped + 3],
        [
            "NEW BEHAVIOR",
            "- #213 the recap names what shipped",
            "...and 1 more",
        ],
        "{body}"
    );
    // AND THE NIGHT GOT ITS OWN ANSWER, so the two questions really were two.
    assert!(
        body.contains("- the night, in one line"),
        "the night was answered with the merge section's lines: {body}"
    );
}

#[test]
fn one_recap_spends_one_summarizer_budget_however_many_questions_it_asks() {
    // ONE EPISODE, ONE BUDGET. `summarizer_deadline_secs` is what the whole
    // return moment may spend, not what each question may: three per-call
    // deadlines at the default key held two processes for twelve minutes after
    // the card had already said the recap was in #pns.
    //
    // COUNTED RATHER THAN TIMED, deliberately. The first call parks past the
    // whole budget, so a shared one leaves the other two nothing to spend and
    // neither of them records a run; a per-call budget hands all three a full
    // key and all three record. Counting the runs says that in one assertion
    // and costs the suite one deadline instead of three.
    //
    // IT PINS THE BUDGET AND NOT THE GUARD. `summarize`'s zero-deadline return
    // is SPAWN AVOIDANCE, and it is not what this count sees: MEASURED with
    // that guard deleted, all three calls fork, and the two behind the parked
    // one are killed on a zero-length window before their stub reaches its own
    // record line, so the count is still one and this test is still green. What
    // the guard saves is three forked-and-instantly-killed children, which is
    // not something a non-flaky test can pin at the process boundary, so it is
    // left to review rather than claimed here.
    let sandbox = Sandbox::new("recap-one-budget");
    // STRUCTURAL: the parked call has to hold the whole budget for the count
    // to mean anything, and summarizer_deadline_secs is whole seconds, so 1
    // is the smallest value that still spawns.
    sandbox.allow_slow(
        "summarizer_deadline_secs is whole seconds; 1s is the smallest budget that still spawns",
    );
    record_every_event(&sandbox);
    sandbox.write_config(&recap_summarized_by(
        "summarizer_deadline_secs = 1\nrepos = [\"webdavis/dotfiles\"]\n\
         review_notes = \"~/notes/checklist-*.md\"\n",
    ));
    loud_window(&sandbox);
    write_note(
        &sandbox,
        "notes/checklist-inside.md",
        "# a finding\n",
        epoch_now() - 1800,
    );

    let mut command = present_event(&sandbox);
    stub_gh_listing(
        &sandbox,
        &mut command,
        213,
        "a subject",
        "## Summary\n\nsomething shipped.\n",
    );
    // RECORDED BEFORE IT PARKS, so a call the deadline killed still counts as
    // a call that was started.
    sandbox.stub_on_path(
        &mut command,
        SUMMARIZER,
        &format!(
            "cat >/dev/null\nprintf 'x\\n' >>\"{}/summarizer.runs\"\nsleep 30",
            sandbox.display()
        ),
    );
    run(&mut command);

    let body = posted_recap(&sandbox);
    assert!(
        body.contains("(The summarizer did not answer"),
        "the parked summarizer answered anyway: {body}"
    );
    assert_eq!(
        std::fs::read_to_string(sandbox.path("summarizer.runs"))
            .expect("the summarizer ran")
            .lines()
            .count(),
        1,
        "one recap started more than one summarizer on one budget"
    );
    // AND BOTH SECTIONS STILL POSTED, off the lines pns holds with no model at
    // all: a spent budget costs the wording and never the facts.
    assert!(
        body.contains("- #213 something shipped.")
            && body.contains("- checklist-inside.md: a finding"),
        "a spent budget cost the sections their mechanical lines: {body}"
    );
}
