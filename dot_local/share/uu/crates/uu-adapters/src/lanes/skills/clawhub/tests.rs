use super::*;
use crate::lanes::skills::tests::{directory, roster, write_roster};
use crate::lanes::stubs::ScriptedRunner;
use crate::{Ran, Verdict};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::Duration;
const POLICY: &str = "policy:\n  allow_implicit_invocation: false\n";
struct Installer {
    calls: RefCell<Vec<Vec<String>>>,
    refuse: bool,
    store: PathBuf,
}
impl CommandRunner for Installer {
    fn run(&self, _: &str, _: &[&str]) -> Result<String, String> {
        panic!("installer inherited the environment")
    }
    fn run_with_input(&self, _: &str, _: &[&str], _: &str) -> Result<Ran, String> {
        Ok(Ran {
            stdout: String::new(),
            stderr: String::new(),
            verdict: Verdict::Clean,
        })
    }
    fn run_with_deadline(&self, _: &str, _: &[&str], _: Duration) -> Result<String, String> {
        unreachable!()
    }
    fn run_in(
        &self,
        program: &str,
        args: &[&str],
        env: &BTreeMap<String, String>,
    ) -> Result<String, String> {
        let mut call = vec![program.to_string()];
        call.extend(args.iter().map(|s| s.to_string()));
        self.calls.borrow_mut().push(call);
        assert_eq!(env["CLAWHUB_DISABLE_TELEMETRY"], "1");
        assert!(env["PATH"].contains("/fnm/aliases/default/bin:"));
        assert!(env["CLAWHUB_CONFIG_PATH"].starts_with(&env["HOME"]));
        if args.contains(&"install") {
            let i = args.iter().position(|s| *s == "--workdir").unwrap();
            let installed = Path::new(args[i + 1]).join("skills/@owner/gamma");
            std::fs::create_dir_all(installed.join(".clawhub")).unwrap();
            std::fs::write(installed.join("SKILL.md"), "gamma body").unwrap();
            std::fs::write(installed.join(".clawhub/origin.json"), "origin bytes").unwrap();
        }
        if self.refuse {
            let overlay = self.store.join("gamma/agents/openai.yaml");
            if self.calls.borrow().len() == 1 {
                assert!(std::fs::read_to_string(overlay).unwrap().contains(POLICY));
                return Ok("gamma: local changes (no match)".into());
            }
            assert_eq!(
                std::fs::read_to_string(&overlay).unwrap(),
                "interface: upstream\n"
            );
            std::fs::write(overlay, "interface: updated\n").unwrap();
        }
        Ok("updated".into())
    }
}
fn setup() -> (SkillsCandidate, SkillsRoster, SkillsEnvironment) {
    let root = directory();
    let c = SkillsCandidate {
        home: root.join("candidate"),
    };
    std::fs::create_dir_all(c.agents().join("skills")).unwrap();
    let path = root.join("roster");
    write_roster(&path, &roster());
    let r = SkillsRoster::read(&path).unwrap();
    let env = SkillsEnvironment::for_candidate(&root.join("real"), &c).unwrap();
    (c, r, env)
}
#[test]
fn an_absent_clawhub_skill_is_installed_in_a_throwaway_workdir_and_moved_flat() {
    let (c, r, env) = setup();
    let runner = Installer {
        calls: RefCell::new(Vec::new()),
        refuse: false,
        store: c.agents().join("skills"),
    };
    assert!(
        c.install_clawhub("/fixture/clawhub", &r, SkillsBuildMode::Full, &env, &runner)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        std::fs::read_to_string(c.agents().join("skills/gamma/SKILL.md")).unwrap(),
        "gamma body"
    );
    assert_eq!(
        std::fs::read_to_string(c.agents().join("skills/gamma/.clawhub/origin.json")).unwrap(),
        "origin bytes"
    );
    let calls = runner.calls.borrow();
    assert_eq!(
        &calls[0][..3],
        &["/fixture/clawhub", "--no-input", "--workdir"]
    );
    assert!(calls[0][3].starts_with(&c.home.join(".tmp/").to_string_lossy().into_owned()));
    assert_eq!(
        &calls[0][4..],
        &[
            "--dir",
            "skills",
            "--registry",
            "https://registry.invalid",
            "install",
            "@owner/gamma"
        ]
    );
    assert!(!c.agents().join("skills/@owner").exists());
}
#[test]
fn a_present_clawhub_skill_is_refreshed_in_place_by_bare_name() {
    let (c, r, env) = setup();
    std::fs::create_dir_all(c.agents().join("skills/gamma")).unwrap();
    let runner = ScriptedRunner::new(&[]);
    c.install_clawhub("/fixture/clawhub", &r, SkillsBuildMode::Full, &env, &runner)
        .unwrap();
    assert_eq!(
        runner.calls(),
        vec![vec![
            "/fixture/clawhub",
            "--no-input",
            "--workdir",
            c.agents().to_str().unwrap(),
            "--dir",
            "skills",
            "update",
            "gamma"
        ]]
    );
    assert_eq!(runner.environments()[0]["CLAWHUB_DISABLE_TELEMETRY"], "1");
}
#[test]
fn the_cli_refusing_over_our_own_overlay_is_retried_with_the_overlay_stripped() {
    let (c, r, env) = setup();
    let path = c.agents().join("skills/gamma/agents/openai.yaml");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let original = format!("interface: upstream\n{POLICY}");
    std::fs::write(&path, &original).unwrap();
    let runner = Installer {
        calls: RefCell::new(Vec::new()),
        refuse: true,
        store: c.agents().join("skills"),
    };
    assert!(
        c.install_clawhub("/fixture/clawhub", &r, SkillsBuildMode::Full, &env, &runner)
            .unwrap()
            .is_empty()
    );
    assert_eq!(runner.calls.borrow().len(), 2);
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        format!("interface: updated\n{POLICY}")
    );
}
