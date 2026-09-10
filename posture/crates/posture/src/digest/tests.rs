use super::*;
use posture_adapters::{CommandIo, CommandOutput};
use posture_application::{ClockUnavailable, InspectionFailure, WallTime};
use std::sync::atomic::{AtomicU64, Ordering};
use std::{
    cell::RefCell,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    rc::Rc,
};

#[derive(Default)]
struct Effects {
    requests: Vec<String>,
    alarms: Vec<Vec<OsString>>,
}

#[derive(Clone, Copy)]
enum Reply {
    Committed,
    Refused,
}

struct Runner {
    expected: PathBuf,
    reply: Option<Reply>,
    effects: Rc<RefCell<Effects>>,
}

impl CommandRunner for Runner {
    fn run_completed(
        &mut self,
        program: &Path,
        args: &[&OsStr],
        io: CommandIo<'_>,
    ) -> Result<CommandOutput, InspectionFailure> {
        assert_eq!(program, self.expected);
        let Some(reply) = self.reply else {
            self.effects
                .borrow_mut()
                .alarms
                .push(args.iter().map(|word| word.to_os_string()).collect());
            return Err(InspectionFailure::Failed);
        };
        let CommandIo::Input(input) = io else {
            panic!("request must be stdin")
        };
        let request = String::from_utf8(input.to_vec()).unwrap();
        // The codec owns canonical JSON. This fixture extracts only its identity.
        let identity = request
            .split("\"request_id\":\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .to_owned();
        self.effects.borrow_mut().requests.push(request);
        let (status, diagnostics, exit) = match reply {
            Reply::Committed => ("accepted", "\"ledger_committed\"", 0),
            Reply::Refused => ("rejected", "", 2),
        };
        Ok(CommandOutput {
            bytes: format!(
                "{{\"schema\":\"pns.result/1\",\"request_id\":\"{identity}\",\"status\":\"{status}\",\"diagnostics\":[{diagnostics}]}}\n"
            )
            .into_bytes(),
            exit,
        })
    }
}

struct Time;
impl Clock for Time {
    fn now(&mut self) -> Result<WallTime, ClockUnavailable> {
        Ok(WallTime {
            seconds: 10000,
            utc_day: "2026-09-08".into(),
        })
    }
}

struct StoppedClock;
impl Clock for StoppedClock {
    fn now(&mut self) -> Result<WallTime, ClockUnavailable> {
        Err(ClockUnavailable)
    }
}

struct Fixture {
    home: PathBuf,
    store: PathBuf,
    effects: Rc<RefCell<Effects>>,
}

impl Fixture {
    fn new(spool: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let home = std::env::temp_dir().join(format!(
            "posture-digest-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let store = home.join("state").join("digest.ndjson");
        prepare_spool_directory(&store).unwrap();
        if !spool.is_empty() {
            std::fs::write(&store, spool).unwrap();
        }
        Self {
            home,
            store,
            effects: Rc::default(),
        }
    }

    fn config(&self) -> Configuration {
        Configuration {
            store: self.store.clone(),
            pns: self.home.join("pns"),
            alarm: self.home.join("osascript"),
        }
    }

    fn run(&self, reply: Reply, stderr: &mut Vec<u8>) -> u8 {
        let config = self.config();
        let runner = Runner {
            expected: config.pns.clone(),
            reply: Some(reply),
            effects: self.effects.clone(),
        };
        let alarm = Runner {
            expected: config.alarm.clone(),
            reply: None,
            effects: self.effects.clone(),
        };
        execute(config, Time, runner, alarm, stderr)
    }

    fn spool_contents(&self) -> String {
        std::fs::read_to_string(&self.store).unwrap_or_default()
    }

    fn kept(&self) -> Option<String> {
        std::fs::read_to_string(self.store.with_extension("ndjson.last")).ok()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.home);
    }
}

fn finding(detector: &str, identity: &str) -> String {
    format!(
        "{{\"detector\":\"{detector}\",\"identity\":\"{identity}\",\"summary\":\"{detector} saw {identity}\"}}\n"
    )
}

#[test]
fn a_day_of_findings_becomes_one_silent_grouped_observation_and_a_forensic_copy() {
    let fixture = Fixture::new(&format!(
        "{}{}",
        finding("launchd", "com.example.agent"),
        finding("launchd", "com.example.other")
    ));
    let mut stderr = vec![];
    assert_eq!(fixture.run(Reply::Committed, &mut stderr), 0);
    let effects = fixture.effects.borrow();
    assert_eq!(effects.requests.len(), 1);
    assert!(effects.alarms.is_empty());
    let request = &effects.requests[0];
    for field in [
        "\"producer\":\"posture\"",
        "\"event\":\"digest\"",
        // AN OBSERVATION, NEVER A PAGE: the digest is by definition everything
        // that did not earn one.
        "\"kind\":\"observation\"",
        "\"occurred_at\":10000",
        "\"route\":\"posture\"",
        "2026-09-08",
        "2 item(s)",
        "launchd",
        "com.example.agent",
    ] {
        assert!(request.contains(field), "{field}: {request}");
    }
    assert!(stderr.is_empty());
    assert_eq!(fixture.spool_contents(), "");
    assert!(fixture.kept().unwrap().contains("com.example.other"));
}

#[test]
fn a_quiet_day_sends_nothing_at_all() {
    // A daily message that says "nothing happened" every day is one the
    // operator stops reading.
    let fixture = Fixture::new("");
    let mut stderr = vec![];
    assert_eq!(fixture.run(Reply::Committed, &mut stderr), 0);
    assert!(fixture.effects.borrow().requests.is_empty());
    assert!(fixture.kept().is_none());
    assert!(stderr.is_empty());
}

#[test]
fn a_refused_send_leaves_the_batch_in_the_spool_for_tomorrow_and_still_exits_zero() {
    // A LOST DAILY DIGEST IS LOW STAKES. The batch is already back, and the
    // engine raises its own alarm when the pipeline itself broke, so a nonzero
    // exit here would only put a red mark in launchd's log.
    let fixture = Fixture::new(&finding("launchd", "com.example.agent"));
    let mut stderr = vec![];
    assert_eq!(fixture.run(Reply::Refused, &mut stderr), 0);
    assert!(fixture.spool_contents().contains("com.example.agent"));
    assert!(fixture.kept().is_none());
    assert!(stderr.is_empty());
}

#[test]
fn a_clock_that_cannot_answer_leaves_the_batch_untouched_and_says_so() {
    // The clock names the day in the title and stamps the claim, so claiming a
    // batch this run cannot finish naming would only risk it.
    let fixture = Fixture::new(&finding("launchd", "com.example.agent"));
    let config = fixture.config();
    let runner = Runner {
        expected: config.pns.clone(),
        reply: Some(Reply::Committed),
        effects: fixture.effects.clone(),
    };
    let alarm = Runner {
        expected: config.alarm.clone(),
        reply: None,
        effects: fixture.effects.clone(),
    };
    let mut stderr = vec![];
    assert_eq!(execute(config, StoppedClock, runner, alarm, &mut stderr), 1);
    assert_eq!(stderr, b"posture digest: the clock did not answer\n");
    assert!(fixture.spool_contents().contains("com.example.agent"));
    assert!(fixture.effects.borrow().requests.is_empty());
}
