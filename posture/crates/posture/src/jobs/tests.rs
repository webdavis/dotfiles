use super::*;
use crate::test_sandbox::Sandbox;
use posture_adapters::{CommandOutput, CommandRunner};
use posture_application::InspectionFailure;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// A launchd this test owns. NOTHING HERE TOUCHES THE REAL ONE: the units are
/// written under a temporary home and every `launchctl` call is recorded
/// instead of run, because loading a unit on the machine running the suite
/// would fight whatever installed that machine's own jobs.
#[derive(Clone, Default)]
struct Launchctl {
    calls: Rc<RefCell<Vec<String>>>,
    loaded: bool,
    bootstrap_fails: bool,
}

impl CommandRunner for Launchctl {
    fn run_completed(
        &mut self,
        program: &Path,
        args: &[&std::ffi::OsStr],
        _: posture_adapters::CommandIo<'_>,
    ) -> Result<CommandOutput, InspectionFailure> {
        let words: Vec<String> = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        self.calls
            .borrow_mut()
            .push(format!("{} {}", program.display(), words.join(" ")));
        let exit = match words.first().map(String::as_str) {
            Some("print") if !self.loaded => 1,
            Some("bootstrap") if self.bootstrap_fails => 1,
            _ => 0,
        };
        Ok(CommandOutput {
            exit,
            bytes: Vec::new(),
        })
    }
}

/// A home directory this test owns, removed with the test.
struct Home(Sandbox);
impl Home {
    fn new() -> Self {
        Self(Sandbox::new("jobs"))
    }
    fn configure(&self, text: &str) {
        let directory = self.0.join(".config/posture");
        std::fs::create_dir_all(&directory).expect("a config directory");
        std::fs::write(directory.join("config.toml"), text).expect("a config file");
    }
    fn unit(&self, label: &str) -> PathBuf {
        self.0.join(format!("Library/LaunchAgents/{label}.plist"))
    }
}
struct Run {
    status: u8,
    stdout: String,
    stderr: String,
    calls: Vec<String>,
}

fn act(verb: Verb, home: &Home, launchctl: Launchctl) -> Run {
    let mut jobs = native::Jobs::new(
        home.0.path().to_path_buf(),
        Path::new("/private/bin/posture"),
        launchctl.clone(),
        501,
    );
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let status = native::perform(verb, &mut jobs, &mut stdout, &mut stderr);
    Run {
        status,
        stdout: String::from_utf8(stdout).expect("text output"),
        stderr: String::from_utf8(stderr).expect("text diagnostics"),
        calls: launchctl.calls.borrow().clone(),
    }
}

#[test]
fn each_verb_is_named_exactly_and_anything_else_refuses_rather_than_falling_through() {
    let words = |words: &[&str]| -> Vec<OsString> { words.iter().map(OsString::from).collect() };
    assert_eq!(decode(&words(&["install"])), Some(Verb::Install));
    assert_eq!(decode(&words(&["verify"])), Some(Verb::Verify));
    assert_eq!(decode(&words(&["list"])), Some(Verb::List));
    for agent in Agent::ALL {
        assert_eq!(
            decode(&words(&["print", agent.key()])),
            Some(Verb::Print(agent)),
            "print names {}",
            agent.key()
        );
        assert!(
            USAGE.contains(agent.key()),
            "the usage names {}",
            agent.key()
        );
    }
    for refused in [
        vec![],
        vec!["INSTALL"],
        vec!["--install"],
        vec!["install", "extra"],
        vec!["print"],
        vec!["print", "converge"],
        vec!["print", "watchdog", "extra"],
        vec!["unknown"],
    ] {
        assert_eq!(decode(&words(&refused)), None, "{refused:?} must refuse");
    }
}

#[test]
fn a_refused_word_writes_usage_to_stderr_and_exits_two_without_touching_a_thing() {
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    assert_eq!(run(&[OsString::from("bogus")], &mut stdout, &mut stderr), 2);
    assert!(stdout.is_empty());
    assert!(String::from_utf8_lossy(&stderr).starts_with("usage: posture jobs "));
}

#[test]
fn print_dumps_one_unit_and_neither_writes_a_file_nor_calls_launchctl() {
    let home = Home::new();
    let run = act(Verb::Print(Agent::Poll), &home, Launchctl::default());
    assert_eq!(run.status, 0);
    assert!(run.stdout.contains("<string>dev.posture.poll</string>"));
    assert!(run.stdout.contains("<key>StartInterval</key>"));
    assert!(run.calls.is_empty(), "{:?}", run.calls);
    assert!(!home.unit("dev.posture.poll").exists());
}

#[test]
fn list_reports_every_job_without_writing_anything_and_stays_successful() {
    let home = Home::new();
    let run = act(Verb::List, &home, Launchctl::default());
    assert_eq!(run.status, 0, "listing is a reading, never a verdict");
    for agent in Agent::ALL {
        assert!(
            run.stdout
                .contains(&format!("{} dev.posture.{}", agent.key(), agent.key())),
            "{}",
            run.stdout
        );
    }
    assert!(run.stdout.contains("no unit is installed at this path"));
    assert!(run.stdout.contains("not loaded"));
    assert!(
        run.stdout
            .contains("0 of 6 jobs are installed, loaded and unchanged")
    );
    assert!(!home.0.join("Library/LaunchAgents").exists());
}

#[test]
fn install_writes_every_unit_under_the_configured_label_and_reloads_each_one() {
    let home = Home::new();
    home.configure("[notify]\nmode = \"off\"\n[jobs]\npoll = \"com.example.poll\"\n[jobs.daily]\ndigest = \"21:05\"\n");
    let run = act(Verb::Install, &home, Launchctl::default());
    assert_eq!(run.status, 0, "{}", run.stderr);
    assert!(run.stderr.is_empty(), "{}", run.stderr);
    let poll = std::fs::read_to_string(home.unit("com.example.poll")).expect("the poll unit");
    assert!(poll.contains("<string>com.example.poll</string>"), "{poll}");
    assert!(
        poll.contains("<string>/private/bin/posture</string>"),
        "{poll}"
    );
    let digest = std::fs::read_to_string(home.unit("dev.posture.digest")).expect("the digest unit");
    assert!(digest.contains("<integer>21</integer>"), "{digest}");
    assert!(digest.contains("<integer>5</integer>"), "{digest}");
    assert!(
        home.0.join(".local/log/posture").is_dir(),
        "the log directory a unit names is made before launchd needs it"
    );
    assert_eq!(
        run.calls,
        vec![
            "/bin/launchctl bootout gui/501/dev.posture.watchdog".to_string(),
            format!(
                "/bin/launchctl bootstrap gui/501 {}",
                home.unit("dev.posture.watchdog").display()
            ),
            "/bin/launchctl bootout gui/501/dev.posture.alert".to_string(),
            format!(
                "/bin/launchctl bootstrap gui/501 {}",
                home.unit("dev.posture.alert").display()
            ),
            "/bin/launchctl bootout gui/501/com.example.poll".to_string(),
            format!(
                "/bin/launchctl bootstrap gui/501 {}",
                home.unit("com.example.poll").display()
            ),
            "/bin/launchctl bootout gui/501/dev.posture.funnel".to_string(),
            format!(
                "/bin/launchctl bootstrap gui/501 {}",
                home.unit("dev.posture.funnel").display()
            ),
            "/bin/launchctl bootout gui/501/dev.posture.digest".to_string(),
            format!(
                "/bin/launchctl bootstrap gui/501 {}",
                home.unit("dev.posture.digest").display()
            ),
            "/bin/launchctl bootout gui/501/dev.posture.heartbeat".to_string(),
            format!(
                "/bin/launchctl bootstrap gui/501 {}",
                home.unit("dev.posture.heartbeat").display()
            ),
        ],
        "each job is booted out before it is bootstrapped, so a changed unit replaces the loaded one"
    );
}

#[test]
fn a_load_launchd_refuses_fails_the_install_and_names_the_job() {
    let home = Home::new();
    let run = act(
        Verb::Install,
        &home,
        Launchctl {
            bootstrap_fails: true,
            ..Launchctl::default()
        },
    );
    assert_eq!(run.status, 1);
    assert!(
        run.stderr
            .contains("posture jobs install: poll: launchctl could not load"),
        "{}",
        run.stderr
    );
}

#[test]
fn verify_fails_on_an_uninstalled_job_and_passes_once_each_one_is_written_and_loaded() {
    let home = Home::new();
    assert_eq!(act(Verb::Verify, &home, Launchctl::default()).status, 1);
    act(Verb::Install, &home, Launchctl::default());
    let run = act(
        Verb::Verify,
        &home,
        Launchctl {
            loaded: true,
            ..Launchctl::default()
        },
    );
    assert_eq!(run.status, 0, "{}", run.stdout);
    assert!(run.stdout.contains("matches what posture would write"));
    assert!(
        run.stdout
            .contains("6 of 6 jobs are installed, loaded and unchanged")
    );
}

#[test]
fn a_written_unit_that_launchd_does_not_hold_still_fails_verification() {
    let home = Home::new();
    act(Verb::Install, &home, Launchctl::default());
    let run = act(Verb::Verify, &home, Launchctl::default());
    assert_eq!(run.status, 1);
    assert!(run.stdout.contains("not loaded"));
    assert!(
        run.stdout
            .contains("0 of 6 jobs are installed, loaded and unchanged")
    );
}

#[test]
fn verify_names_the_lines_a_unit_someone_else_wrote_differs_by() {
    let home = Home::new();
    act(Verb::Install, &home, Launchctl::default());
    let path = home.unit("dev.posture.poll");
    let unit = std::fs::read_to_string(&path).expect("the poll unit");
    std::fs::write(
        &path,
        unit.replace("<integer>60</integer>", "<integer>300</integer>"),
    )
    .expect("a drifted unit");
    let run = act(
        Verb::Verify,
        &home,
        Launchctl {
            loaded: true,
            ..Launchctl::default()
        },
    );
    assert_eq!(run.status, 1);
    assert!(
        run.stdout
            .contains("differs from what posture would write in 2 line(s)"),
        "{}",
        run.stdout
    );
    assert!(
        run.stdout
            .contains("posture would write  <integer>60</integer>"),
        "{}",
        run.stdout
    );
    assert!(
        run.stdout
            .contains("on disk only         <integer>300</integer>"),
        "{}",
        run.stdout
    );
    assert!(
        run.stdout
            .contains("5 of 6 jobs are installed, loaded and unchanged"),
        "{}",
        run.stdout
    );
}

#[test]
fn a_daily_time_the_config_states_wrongly_is_named_and_the_shipped_one_installs() {
    let home = Home::new();
    home.configure("[notify]\nmode = \"off\"\n[jobs.daily]\nheartbeat = \"quarter past\"\n");
    let run = act(Verb::List, &home, Launchctl::default());
    assert_eq!(run.status, 0);
    assert!(
        run.stderr
            .contains("posture jobs: `jobs.daily.heartbeat` is ignored"),
        "{}",
        run.stderr
    );
    assert!(run.stdout.contains("daily at 09:00"), "{}", run.stdout);
}
