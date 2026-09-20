use super::*;
use crate::Ran;
use crate::lanes::skills::tests::{directory, write_roster};
use std::cell::RefCell;
use std::time::Duration;
mod failures;
struct Git {
    failure: &'static str,
    calls: RefCell<Vec<Vec<String>>>,
    homes: RefCell<Vec<std::path::PathBuf>>,
}
impl Git {
    fn new(failure: &'static str) -> Self {
        Self {
            failure,
            calls: RefCell::new(Vec::new()),
            homes: RefCell::new(Vec::new()),
        }
    }
}
impl CommandRunner for Git {
    fn run(&self, _: &str, _: &[&str]) -> Result<String, String> {
        panic!("every git operation must have a deadline")
    }
    fn run_with_input(&self, _: &str, _: &[&str], _: &str) -> Result<Ran, String> {
        unreachable!()
    }
    fn run_with_deadline(&self, _: &str, _: &[&str], _: Duration) -> Result<String, String> {
        panic!("the comparison runs git in an environment of its own")
    }
    fn run_in(
        &self,
        program: &str,
        args: &[&str],
        environment: &crate::lanes::Environment,
        most: Option<Duration>,
    ) -> Result<String, String> {
        // NO HELPER PROCESS: git itself is the program, and the isolated
        // environment carries what `env -i VAR=...` used to spell in argv.
        assert_eq!(program, "/usr/bin/git");
        assert_eq!(most, Some(Duration::from_secs(300)));
        assert!(environment.only_these);
        assert_eq!(environment.variables["PATH"], "/usr/bin:/bin");
        for (key, value) in [
            ("GIT_CONFIG_GLOBAL", "/dev/null"),
            ("GIT_CONFIG_SYSTEM", "/dev/null"),
            ("GIT_CONFIG_COUNT", "0"),
            ("GIT_CONFIG_PARAMETERS", ""),
            ("GIT_TERMINAL_PROMPT", "0"),
        ] {
            assert_eq!(environment.variables[key], value, "{key}");
        }
        let home = std::path::PathBuf::from(&environment.variables["HOME"]);
        self.homes.borrow_mut().push(home.clone());
        self.calls
            .borrow_mut()
            .push(args.iter().map(|s| s.to_string()).collect());
        if args.contains(&"clone") {
            let end = args
                .iter()
                .position(|s| *s == "--")
                .expect("URL is separated from options");
            assert_eq!(args[end + 1], "--public-fixture");
            std::fs::create_dir(home.join("repo")).unwrap();
            std::fs::write(home.join("repo/owned"), "clone").unwrap();
            return match self.failure { "unreachable"=>Err("exit 128: fixture remote rejected".into()), "timeout"=>Err("lane `skills step git` exceeded its 300s deadline, so its process group was killed".into()), _=>Ok(String::new()) };
        }
        match args.last().copied().unwrap() {
            "HEAD^{commit}" if self.failure == "headless" => Err("exit 128".into()),
            "HEAD:SKILL.md" if self.failure == "path" => Err("HEAD:SKILL.md\nexit 128".into()),
            "HEAD^{tree}" | "HEAD:SKILL.md" => Ok("new-tree\n".into()),
            "HEAD^{commit}" => Ok("commit\n".into()),
            other => panic!("unexpected revision {other}"),
        }
    }
}
fn setup() -> (std::path::PathBuf, std::path::PathBuf) {
    let root = directory();
    let lock = root.join("lock");
    write_roster(
        &lock,
        &serde_json::json!({"forks":{"fork\nname":{"sourceUrl":"--public-fixture","skillPath":"SKILL.md","lastComparedTreeHash":"old-tree"}}}),
    );
    (root, lock)
}
#[test]
fn a_fork_whose_upstream_tree_hash_moved_is_pending_with_both_hashes() {
    let (root, lock) = setup();
    let git = Git::new("");
    let mut report = LaneReport::new("skills");
    SkillsForkWatch::read(&lock).report(&root, &git, &mut report);
    assert_eq!(report.verdict(), uu_domain::LaneVerdict::Pending);
    assert_eq!(report.failures(), 0);
    let said = report.lines.join("\n");
    for expected in ["fork-drift", "old-tree", "new-tree", "compare", "by hand"] {
        assert!(said.contains(expected), "{said}");
    }
    assert_eq!(git.calls.borrow().len(), 3);
}
#[test]
fn a_whole_repository_comparison_uses_the_root_tree_and_unchanged_is_not_pending() {
    let (root, lock) = setup();
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&lock).unwrap()).unwrap();
    value["forks"]["fork\nname"]["skillPath"] = ".".into();
    value["forks"]["fork\nname"]["lastComparedTreeHash"] = "new-tree".into();
    write_roster(&lock, &value);
    let git = Git::new("");
    let mut report = LaneReport::new("skills");
    SkillsForkWatch::read(&lock).report(&root, &git, &mut report);
    assert_eq!(report.verdict(), uu_domain::LaneVerdict::Completed);
    assert!(report.lines.iter().any(|s| s.contains("unchanged")));
    assert_eq!(
        git.calls.borrow().last().unwrap().last().unwrap(),
        "HEAD^{tree}"
    );
}
#[test]
fn the_temp_clone_is_removed_whatever_the_outcome() {
    let (root, lock) = setup();
    for failure in ["", "unreachable", "timeout", "headless", "path"] {
        let git = Git::new(failure);
        let mut report = LaneReport::new("skills");
        SkillsForkWatch::read(&lock).report(&root, &git, &mut report);
        assert!(!git.homes.borrow().is_empty(), "clone must be attempted");
        assert!(git.homes.borrow().iter().all(|p| !p.exists()), "{failure}");
        assert!(lock.exists());
    }
}
