use super::{Invocation, Kind, Settings};

fn settings(kind: Kind, model: &str) -> Settings {
    Settings {
        kind,
        model: model.to_string(),
        ..Settings::default()
    }
}

/// THE GOLDEN VECTOR PER KNOWN TYPE. pns owns these words, so this is what
/// catches pns drifting on its own; the day a harness changes its own flags is
/// caught by the doctor row, which runs the real binary.
#[test]
fn every_known_type_composes_the_exact_words_pns_runs() {
    let words = |argv: &[&str]| {
        argv.iter()
            .map(|word| (*word).to_string())
            .collect::<Vec<_>>()
    };
    for (kind, model, argv, in_argv) in [
        (
            Kind::Claude,
            "",
            words(&["claude", "-p", "--safe-mode", "--tools", ""]),
            false,
        ),
        (
            Kind::Claude,
            "haiku",
            words(&[
                "claude",
                "-p",
                "--safe-mode",
                "--tools",
                "",
                "--model",
                "haiku",
            ]),
            false,
        ),
        (
            Kind::Codex,
            "",
            words(&["codex", "exec", "--color", "never", "--skip-git-repo-check"]),
            false,
        ),
        (
            Kind::Codex,
            "gpt-5.1-codex",
            words(&[
                "codex",
                "exec",
                "--color",
                "never",
                "--skip-git-repo-check",
                "-m",
                "gpt-5.1-codex",
            ]),
            false,
        ),
        (
            Kind::Ollama,
            "qwen3.5:4b",
            words(&[
                "ollama",
                "run",
                "qwen3.5:4b",
                "--hidethinking",
                "--nowordwrap",
                "--think=false",
            ]),
            false,
        ),
        (
            Kind::Hermes,
            "",
            words(&["hermes", "chat", "-Q", "-t", "", "-q"]),
            true,
        ),
        (
            Kind::Hermes,
            "anthropic/claude-sonnet-4",
            words(&[
                "hermes",
                "chat",
                "-Q",
                "-t",
                "",
                "-m",
                "anthropic/claude-sonnet-4",
                "-q",
            ]),
            true,
        ),
    ] {
        let invocation = settings(kind, model).invocation().expect("a known type");
        assert_eq!(
            (invocation.argv, invocation.prompt_in_argv),
            (argv, in_argv),
            "the vector for {} with model {model:?}",
            kind.word()
        );
    }
}

/// CODEX ALONE RUNS IN THE STRIPPED HOME, which the adapter supplies because
/// its path is a runtime fact rather than a word pns composes.
#[test]
fn only_codex_runs_in_the_stripped_codex_home() {
    for kind in super::WORDS {
        let mut stated = settings(*kind, "m");
        stated.command = vec!["my-model".to_string()];
        assert_eq!(
            stated.invocation().expect("configured").stripped_codex_home,
            *kind == Kind::Codex,
            "{}",
            kind.word()
        );
    }
}

/// THE EFFORT IS EACH HARNESS'S OWN FLAG, and only the two that have one take
/// it. An empty effort passes nothing, which leaves the backend's default.
#[test]
fn the_effort_is_passed_in_each_harnesss_own_flag() {
    let mut codex = settings(Kind::Codex, "gpt-6-luna");
    codex.effort = "low".to_string();
    assert_eq!(
        codex.invocation().unwrap().argv[5..],
        ["-m", "gpt-6-luna", "-c", "model_reasoning_effort=\"low\""]
    );
    let mut claude = settings(Kind::Claude, "");
    claude.effort = "high".to_string();
    assert_eq!(claude.invocation().unwrap().argv[5..], ["--effort", "high"]);
    let takers: Vec<Kind> = super::WORDS
        .iter()
        .copied()
        .filter(|kind| kind.takes_effort())
        .collect();
    assert_eq!(takers, [Kind::Claude, Kind::Codex]);
}

/// `custom` IS THE OPERATOR'S OWN WORDS, and an empty one is no summarizer at
/// all rather than a command that cannot spawn.
#[test]
fn a_custom_summarizer_is_the_operators_words_and_an_empty_one_is_no_summarizer() {
    let mut custom = Settings::default();
    assert!(
        !custom.configured(),
        "the shipped default has no summarizer"
    );
    assert_eq!(custom.invocation(), None);
    custom.command = vec!["my-model".to_string(), "--quiet".to_string()];
    assert!(custom.configured());
    assert_eq!(
        custom.invocation(),
        Some(Invocation {
            argv: custom.command.clone(),
            prompt_in_argv: false,
            stripped_codex_home: false,
        })
    );
    // A KNOWN TYPE IS CONFIGURED WITHOUT A COMMAND, which is the whole point
    // of naming a harness instead of writing its flags out.
    assert!(settings(Kind::Claude, "").configured());
}

/// The five words, and nothing else.
#[test]
fn the_type_words_round_trip_and_anything_else_names_no_summarizer() {
    for kind in super::WORDS {
        assert_eq!(Kind::of(kind.word()), Some(*kind));
    }
    for word in ["", "Claude", "gpt", "ollama run"] {
        assert_eq!(Kind::of(word), None, "case: {word}");
    }
}
