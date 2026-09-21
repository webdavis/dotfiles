use super::*;

#[test]
fn a_configured_summarizers_lines_become_the_night_in_order() {
    // THE ONE THING THE MODEL IS ALLOWED TO CHANGE. The window is handed to the
    // configured command on stdin and what comes back is the timeline, in place
    // of the mechanical line-per-event the same window would have rendered.
    let sandbox = Sandbox::new("recap-summarizer-lines");
    record_every_event(&sandbox);
    sandbox.write_config(&recap_summarized_by(""));
    loud_window(&sandbox);

    let mut command = present_event(&sandbox);
    // THE STUB KEEPS EVERY PROMPT, appended, so the test can say what the
    // model was actually handed rather than trusting the writer. A DELIVERED
    // RECAP ASKS IT TWICE: once for this timeline and once for the summary
    // paragraph above it.
    sandbox.stub_on_path(
        &mut command,
        SUMMARIZER,
        &format!(
            "cat >> '{}'\nprintf '%s\\n' 'the branch landed' 'the suite went red' 'a review is waiting'",
            sandbox.path("prompt.captured").display()
        ),
    );
    run(&mut command);

    let body = posted_recap(&sandbox);
    let night = body
        .lines()
        .position(|line| line == "AGENTS")
        .unwrap_or_else(|| panic!("no timeline at all: {body}"));
    assert_eq!(
        body.lines().skip(night + 1).take(3).collect::<Vec<_>>(),
        [
            "- the branch landed",
            "- the suite went red",
            "- a review is waiting"
        ],
        "the summarizer's lines are not the timeline: {body}"
    );
    assert!(
        !body.contains("planted 0"),
        "the mechanical lines were posted as well: {body}"
    );
    // AN ANSWERED NIGHT CARRIES NO NOTE ABOUT SILENCE.
    assert!(
        !body.contains("(The summarizer did not answer"),
        "a note about silence on an answered night: {body}"
    );
    // AND THE MODEL WAS HANDED THE REAL WINDOW: the instruction in front, the
    // window's own entries behind it. A gutted prompt would summarize nothing
    // and every other assertion here would still pass.
    let prompt =
        std::fs::read_to_string(sandbox.path("prompt.captured")).expect("the captured prompt");
    assert!(
        prompt.contains("Below are the events"),
        "the timeline instruction never reached the model: {prompt:?}"
    );
    assert!(
        prompt.contains("planted 1"),
        "the window's entries never reached the model: {prompt:?}"
    );
}

#[test]
fn the_windows_own_count_and_what_needs_you_survive_whatever_the_model_says() {
    // WHAT THE MODEL IS NOT ALLOWED TO CHANGE, and the reason the substitution
    // is a type rather than a prompt: this stub answers with a header of its
    // own carrying a false count, and with nothing urgent in it at all. The
    // count in the message stays the length of the window pns read, and the
    // line naming what is still waiting stays where it was composed.
    let sandbox = Sandbox::new("recap-summarizer-count");
    record_every_event(&sandbox);
    sandbox.write_config(&recap_summarized_by(""));
    loud_window(&sandbox);

    let mut command = present_event(&sandbox);
    stub_summarizer(
        &sandbox,
        &mut command,
        "printf '%s\\n' 'While you were away, 00:00-00:00 · 999 events' \
         'a quiet night, nothing needed anybody'",
    );
    run(&mut command);

    let body = posted_recap(&sandbox);
    let lines: Vec<&str> = body.lines().collect();
    // TWELVE, NOT THE CARD'S THIRTEEN, and the two are each honest about what
    // they read: the card counts the ring, which the live event joined, and
    // the recap counts the durable activity table, which argv events never
    // reach. `header`'s own comment states why nothing reconciles them.
    assert!(
        lines[0].starts_with("While you were away, ") && lines[0].ends_with("· 12 events"),
        "the model's header was posted as the recap's own: {body}"
    );
    let urgent = lines
        .iter()
        .position(|line| line.contains("claude/blocked p4: planted 4"))
        .unwrap_or_else(|| panic!("the model summarized away what needs the operator: {body}"));
    let night = lines
        .iter()
        .position(|line| *line == "AGENTS")
        .unwrap_or_else(|| panic!("no timeline at all: {body}"));
    // `open` IS LAST NOW, which is the design's own section order, and it is
    // still the section a model cannot touch: the line is composed here and
    // the summarizer only ever writes the agents list above it.
    assert!(
        night < urgent,
        "what needs the operator moved out of its own section: {body}"
    );
    assert!(
        lines.contains(&"- a quiet night, nothing needed anybody"),
        "the model still wrote the timeline: {body}"
    );
    // AND ITS OWN HEADER IS A LINE OF THE NIGHT, never a line of structure: it
    // is carried, prefixed, under the real one rather than dropped, so nothing
    // is censored and nothing is a heading that pns did not write.
    assert_eq!(
        lines
            .iter()
            .skip(night)
            .filter(|line| line.starts_with("While you were away, "))
            .count(),
        0,
        "the model's own header line reads as a header: {body}"
    );
    assert!(
        lines[0].starts_with("While you were away, "),
        "the page's own header is the first line: {body}"
    );
}
