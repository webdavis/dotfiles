use crate::config::{NvimHost, NvimPluginsLane};
use crate::lanes::stubs::stub_facts;
use crate::lanes::{CommandRunner, LaneAdapter, Ran, Verdict};
use std::cell::RefCell;
use std::time::Duration;
use uu_domain::LaneVerdict;

struct Child {
    verdict: Verdict,
    calls: RefCell<Vec<Vec<String>>>,
}
impl Child {
    fn new(verdict: Verdict) -> Self {
        Self {
            verdict,
            calls: RefCell::new(Vec::new()),
        }
    }
}
impl CommandRunner for Child {
    fn run(&self, _: &str, _: &[&str]) -> Result<String, String> {
        panic!("Neovim output needs its typed exit verdict")
    }
    fn run_with_deadline(&self, _: &str, _: &[&str], _: Duration) -> Result<String, String> {
        panic!("the existing whole-lane deadline owns Neovim")
    }
    fn run_with_input(&self, program: &str, args: &[&str], input: &str) -> Result<Ran, String> {
        assert_eq!(input, "");
        self.calls.borrow_mut().push(
            std::iter::once(program)
                .chain(args.iter().copied())
                .map(String::from)
                .collect(),
        );
        Ok(Ran {
            stdout: "finder: updates available\n".into(),
            verdict: self.verdict.clone(),
        })
    }
}
fn lane(auto_commit: bool) -> NvimPluginsLane {
    NvimPluginsLane {
        host: NvimHost {
            nvim: "/fixture/nvim".into(),
            config: "/fixture/config with spaces/".into(),
        },
        auto_commit,
        repo: Some("/fixture/source with spaces".into()),
    }
}

#[test]
fn the_plugins_lane_runs_nvim_headless_with_the_configs_init_and_the_uu_module() {
    let child = Child::new(Verdict::Clean);
    let report = lane(false).run("editor", &stub_facts(), &child);
    assert_eq!(report.verdict(), LaneVerdict::Completed);
    assert_eq!(
        *child.calls.borrow(),
        vec![vec![
            "/fixture/nvim",
            "--headless",
            "-u",
            "/fixture/config with spaces/init.lua",
            "-l",
            "/fixture/config with spaces/lua/uu/plugins.lua"
        ]]
    );
}

#[test]
fn an_nvim_lane_names_its_own_lane_not_its_type() {
    let child = Child::new(Verdict::Clean);
    assert_eq!(
        lane(false).run("my-editor", &stub_facts(), &child).name,
        "my-editor"
    );
}

#[test]
fn a_plugins_child_exiting_pending_is_a_pending_lane_carrying_its_lines() {
    let child = Child::new(Verdict::Pending("exit 100: pins need review".into()));
    let report = lane(false).run("editor", &stub_facts(), &child);
    assert_eq!(report.verdict(), LaneVerdict::Pending);
    assert_eq!(report.failures(), 0);
    assert_eq!(
        report.lines,
        [
            "finder: updates available",
            "/fixture/nvim: pending (exit 100: pins need review)"
        ]
    );
}

#[test]
fn a_plugins_child_exiting_non_zero_is_a_counted_failure_carrying_its_stderr_tail() {
    for verdict in [
        Verdict::Failed("exit 1: fetch tail".into()),
        Verdict::Deferred("exit 75: fetch tail".into()),
    ] {
        let child = Child::new(verdict);
        let report = lane(false).run("editor", &stub_facts(), &child);
        assert_eq!(report.verdict(), LaneVerdict::Failed);
        assert_eq!(report.failures(), 1);
        assert_eq!(report.lines[0], "finder: updates available");
        assert!(report.lines[1].ends_with("fetch tail"));
    }
}

#[test]
fn a_plugins_lane_with_auto_commit_on_hands_the_module_the_repo() {
    let child = Child::new(Verdict::Clean);
    lane(true).run("editor", &stub_facts(), &child);
    assert_eq!(
        &child.calls.borrow()[0][6..],
        &["--auto-commit", "--repo", "/fixture/source with spaces"]
    );
}

#[test]
fn a_plugins_lane_with_auto_commit_off_passes_no_commit_flag() {
    let child = Child::new(Verdict::Clean);
    lane(false).run("editor", &stub_facts(), &child);
    assert_eq!(child.calls.borrow()[0].len(), 6);
    assert!(
        !child.calls.borrow()[0]
            .iter()
            .any(|arg| arg == "--auto-commit" || arg == "--repo")
    );
}
