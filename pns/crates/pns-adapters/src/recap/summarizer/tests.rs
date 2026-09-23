use super::{Failure, run_summarizer};
use pns_domain::recap::summarizer::Invocation;
use std::time::Duration;

/// A scripted summarizer under the test's own directory. NEVER `claude`,
/// `codex`, `ollama` OR THE LIVE HERMES: this proves the seam, and a test that
/// reached a real model would prove the network instead.
fn scripted(directory: &std::path::Path, name: &str, body: &str) -> Invocation {
    let path = directory.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::os::unix::fs::PermissionsExt::set_mode(
        &mut std::fs::metadata(&path).unwrap().permissions().clone(),
        0o755,
    );
    std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    Invocation {
        argv: vec![path.display().to_string()],
        prompt_in_argv: false,
        stripped_codex_home: false,
    }
}

fn temporary(name: &str) -> std::path::PathBuf {
    let directory =
        std::env::temp_dir().join(format!("pns-summarizer-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

/// THE PROMPT REACHES THE BACKEND AND ITS ANSWER COMES BACK, which is the one
/// thing the seam owes its caller.
#[test]
fn the_prompt_is_written_to_the_child_and_its_answer_read_back() {
    let directory = temporary("answers");
    let invocation = scripted(&directory, "answering", "cat");
    assert_eq!(
        run_summarizer(&invocation, Duration::from_secs(5), "what moved").unwrap(),
        "what moved"
    );
    std::fs::remove_dir_all(&directory).ok();
}

/// A PROMPT IN ARGV IS THE OTHER PLACEMENT, which is how hermes is reached.
#[test]
fn a_backend_that_takes_the_prompt_in_argv_is_handed_it_as_the_last_word() {
    let directory = temporary("argv");
    let mut invocation = scripted(&directory, "echoing", "printf '%s' \"$1\"");
    invocation.prompt_in_argv = true;
    assert_eq!(
        run_summarizer(&invocation, Duration::from_secs(5), "in the words").unwrap(),
        "in the words"
    );
    std::fs::remove_dir_all(&directory).ok();
}

/// EVERY WAY OF NOT ANSWERING IS ITS OWN FAILURE, and the hung one is bounded
/// by the deadline rather than by whoever called it: this is what keeps a
/// wedged backend from holding the gateway's own tick.
#[test]
fn a_missing_silent_refusing_or_hung_summarizer_each_names_its_own_failure() {
    let directory = temporary("failures");
    let missing = Invocation {
        argv: vec![directory.join("not-installed").display().to_string()],
        prompt_in_argv: false,
        stripped_codex_home: false,
    };
    assert_eq!(
        run_summarizer(&missing, Duration::from_secs(5), "x"),
        Err(Failure::Unstarted)
    );
    assert_eq!(
        run_summarizer(
            &scripted(&directory, "quiet", "exit 0"),
            Duration::from_secs(5),
            "x"
        ),
        Err(Failure::Silent)
    );
    assert_eq!(
        run_summarizer(
            &scripted(&directory, "refusing", "echo nope; exit 3"),
            Duration::from_secs(5),
            "x"
        ),
        Err(Failure::Unanswered)
    );
    let started = std::time::Instant::now();
    assert_eq!(
        run_summarizer(
            &scripted(&directory, "hung", "sleep 30"),
            Duration::from_millis(200),
            "x"
        ),
        Err(Failure::Unanswered),
        "a hung backend is killed at the deadline rather than waited on"
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the deadline bounds the call: {:?}",
        started.elapsed()
    );
    // AND A SPENT BUDGET STARTS NOTHING AT ALL.
    assert_eq!(
        run_summarizer(
            &scripted(&directory, "never-run", "echo ran"),
            Duration::ZERO,
            "x"
        ),
        Err(Failure::Silent)
    );
    std::fs::remove_dir_all(&directory).ok();
}

/// The one visible line each failure leaves in the summary's place.
#[test]
fn every_failure_leaves_one_line_naming_the_summarizer_and_what_happened() {
    let deadline = Duration::from_secs(240);
    assert_eq!(
        Failure::Unstarted.line("claude", deadline),
        "the claude summarizer could not be started"
    );
    assert_eq!(
        Failure::Unanswered.line("ollama", deadline),
        "the ollama summarizer gave no answer within 4m"
    );
    assert_eq!(
        Failure::Silent.line("custom", deadline),
        "the custom summarizer answered nothing"
    );
}
