use super::Sandbox;
use std::process::Command;

/// What a recording stub backend was spawned with: its argument words and the
/// two variables a summarizer's isolation sets, `unset` when absent.
pub struct SummarizerSpawn {
    pub argv: Vec<String>,
    pub codex_home: String,
    pub summarizing: String,
}

impl SummarizerSpawn {
    /// The word right after the first `flag`, or None when `flag` was not
    /// passed.
    pub fn after(&self, flag: &str) -> Option<&str> {
        let at = self.argv.iter().position(|word| word == flag)?;
        self.argv.get(at + 1).map(String::as_str)
    }

    /// Whether `flag` was passed with `value` anywhere, for a flag given more
    /// than once.
    pub fn passed(&self, flag: &str, value: &str) -> bool {
        self.argv
            .windows(2)
            .any(|pair| pair[0] == flag && pair[1] == value)
    }

    /// Every part of the stripped Codex home's isolation, rooted at `home`:
    /// no session file, no writes, no shell, no connector, plugin or hook, no
    /// sub-agent, goal, image read or web search.
    pub fn assert_codex_isolated(&self, home: &str) {
        let argv = &self.argv;
        assert!(argv.contains(&"--ephemeral".to_string()), "{argv:?}");
        assert_eq!(self.after("-s"), Some("read-only"), "{argv:?}");
        assert_eq!(self.after("-C"), Some(home), "{argv:?}");
        for feature in [
            "shell_tool",
            "unified_exec",
            "apps",
            "plugins",
            "hooks",
            "multi_agent",
            "goals",
            "view_image",
        ] {
            assert!(self.passed("--disable", feature), "{feature}: {argv:?}");
        }
        assert!(
            self.passed("-c", "web_search=\"disabled\""),
            "web search: {argv:?}"
        );
        assert_eq!(self.codex_home, home, "CODEX_HOME is the stripped home");
        assert_eq!(self.summarizing, "1", "the re-entry guard is set");
    }
}

impl Sandbox {
    /// A stub `name` first on PATH that answers `answer` and records what it
    /// was spawned with, for `recorded_spawn` to read back.
    pub fn stub_recording_backend(&self, command: &mut Command, name: &str, answer: &str) {
        self.stub_on_path(
            command,
            name,
            &format!(
                "cat >/dev/null\n\
                 for word in \"$@\"; do printf '%s\\n' \"$word\"; done >\"{root}/backend.argv\"\n\
                 printf '%s\\n%s\\n' \"${{CODEX_HOME-unset}}\" \"${{PNS_SUMMARIZING-unset}}\" \
                 >\"{root}/backend.env\"\n\
                 printf '%s\\n' '{answer}'",
                root = self.display()
            ),
        );
    }

    /// What the recording stub was spawned with. Panics naming `context` when
    /// the stub never ran.
    pub fn recorded_spawn(&self, context: &str) -> SummarizerSpawn {
        let recorded = |name: &str| -> Vec<String> {
            std::fs::read_to_string(self.path(name))
                .unwrap_or_else(|error| panic!("the stub never ran ({error}): {context}"))
                .lines()
                .map(str::to_string)
                .collect()
        };
        let env = recorded("backend.env");
        SummarizerSpawn {
            argv: recorded("backend.argv"),
            codex_home: env[0].clone(),
            summarizing: env[1].clone(),
        }
    }
}
