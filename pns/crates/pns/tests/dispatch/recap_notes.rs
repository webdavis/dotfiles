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
        "{EVERY_DISPATCHED_CHANNEL}[recap]\nreview_notes_glob = \"~/notes/checklist-*.md\"\n"
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
        "{EVERY_DISPATCHED_CHANNEL}[recap]\nreview_notes_glob = \"~/notes/checklist-*.md\"\n"
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
    // AN EMPTY WINDOWED SECTION IS OMITTED unless `-v` asks for it, which is
    // the design's own ruling: a page of "nothing in this window" lines
    // buries the sections that had news. What must never happen is the empty
    // state reading as one of the other two.
    assert!(
        !body.contains("REVIEW NOTES"),
        "a glob that matched nothing printed a section nobody can act on: {body}"
    );

    // AND A GLOB POINTING AT A DIRECTORY NOBODY MADE is a config the operator
    // has to fix, not a quiet night.
    let nowhere = Sandbox::new("recap-notes-nowhere");
    record_every_event(&nowhere);
    nowhere.write_config(&format!(
        "{EVERY_DISPATCHED_CHANNEL}[recap]\nreview_notes_glob = \"~/no-such-dir/checklist-*.md\"\n"
    ));
    loud_window(&nowhere);

    run(&mut present_event(&nowhere));

    let missing = posted_recap(&nowhere);
    assert!(
        missing.contains("REVIEW NOTES: unavailable"),
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
        "{EVERY_DISPATCHED_CHANNEL}[recap]\nreview_notes_glob = \"~/notes/checklist-*.md\"\n"
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
        .position(|line| line.starts_with("REVIEW NOTES"))
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
fn a_summarized_review_section_keeps_only_the_lines_its_own_sources_vouch_for() {
    // THE WHOLE PATH IN ONE TEST: two notes off the glob, the note prompt
    // handed to a real process, and the receipts check run over what that
    // process actually said. Every piece of it was covered on its own and the
    // WIRING between them was not, so forcing the answer to None left the
    // suite green: no test proved a configured summarizer ever reached this
    // section at all.
    let sandbox = Sandbox::new("recap-notes-summarized");
    record_every_event(&sandbox);
    sandbox.write_config(&recap_summarized_by(
        "review_notes_glob = \"~/notes/checklist-*.md\"\n",
    ));
    loud_window(&sandbox);
    write_note(
        &sandbox,
        "notes/checklist-first.md",
        "# the first finding\n",
        epoch_now(),
    );
    write_note(
        &sandbox,
        "notes/checklist-second.md",
        "# the second finding\n",
        epoch_now(),
    );

    let mut command = present_event(&sandbox);
    // ONE STUB, TWO QUESTIONS, ANSWERED APART. It replies to the note prompt
    // only when it is handed the note instruction, which is what proves
    // `note_prompt` is what this section asked with rather than the window's
    // own prompt reaching it by accident.
    sandbox.stub_on_path(
        &mut command,
        SUMMARIZER,
        "case \"$(cat)\" in\n  *'review notes written'*) printf '%s\\n' \
         'checklist-first.md the review named it' 'and this line cites nothing' ;;\n  \
         *) printf '%s\\n' 'the window, in one line' ;;\nesac",
    );
    run(&mut command);

    let body = posted_recap(&sandbox);
    let lines: Vec<&str> = body.lines().collect();
    let reviewed = lines
        .iter()
        .position(|line| *line == "REVIEW NOTES")
        .unwrap_or_else(|| panic!("no REVIEW NOTES section at all: {body}"));
    assert_eq!(
        lines[reviewed..reviewed + 3],
        [
            "REVIEW NOTES",
            "- checklist-first.md the review named it",
            "...and 1 more",
        ],
        "{body}"
    );
    // AND THE WINDOW GOT ITS OWN ANSWER, so the two questions really were two.
    assert!(
        body.contains("- the window, in one line"),
        "the window was answered with the review section's lines: {body}"
    );
}
