use super::*;
use posture_domain::CuratedLine;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

type Events = Rc<RefCell<Vec<&'static str>>>;
struct Guard(Events);
impl Drop for Guard {
    fn drop(&mut self) {
        self.0.borrow_mut().push("unlock");
    }
}
struct Lock {
    events: Events,
    fails: bool,
}
impl WriteLock for Lock {
    type Guard = Guard;
    fn acquire(&self) -> Result<Guard, LockRefusal> {
        self.events.borrow_mut().push("lock");
        if self.fails {
            Err(LockRefusal)
        } else {
            Ok(Guard(self.events.clone()))
        }
    }
}
struct Launch {
    events: Events,
    answer: Result<CapturedAgent, CaptureRefusal>,
}
impl LaunchdTable for Launch {
    fn capture(&mut self, label: &str) -> Result<CapturedAgent, CaptureRefusal> {
        assert_eq!(label, "my.alpha");
        self.events.borrow_mut().push("capture");
        self.answer.clone()
    }
}
struct Source {
    events: Events,
    path: Result<PathBuf, SourceRefusal>,
    lines: Result<Vec<SourceLine>, SourceRefusal>,
    contains: bool,
    listed: Vec<u8>,
}
impl SourceAllowlist for Source {
    fn source_path(&mut self) -> Result<PathBuf, SourceRefusal> {
        self.events.borrow_mut().push("resolve");
        self.path.clone()
    }
    fn contains_label_text(&self, path: &Path, label: &str) -> bool {
        assert_eq!(path, Path::new("/source"));
        assert_eq!(label, "my.alpha");
        self.events.borrow_mut().push("contains");
        self.contains
    }
    fn read(&self, path: &Path) -> Result<Vec<SourceLine>, SourceRefusal> {
        assert_eq!(path, Path::new("/source"));
        self.events.borrow_mut().push("read");
        self.lines.clone()
    }
    fn list(&self) -> Result<Vec<u8>, SourceRefusal> {
        self.events.borrow_mut().push("list");
        Ok(self.listed.clone())
    }
}
struct Publish {
    events: Events,
    error: Option<PublicationRefusal>,
    preserved: Vec<Vec<u8>>,
    entries: Vec<(String, String, String, String)>,
}
impl Publisher for Publish {
    fn publish(
        &mut self,
        path: &Path,
        lines: &[CuratedLine<'_, &[u8]>],
    ) -> Result<(), PublicationRefusal> {
        assert_eq!(path, Path::new("/source"));
        assert!(
            !self.events.borrow().contains(&"unlock"),
            "publication holds the write lock"
        );
        self.events.borrow_mut().push("publish");
        for line in lines {
            match line {
                CuratedLine::Preserved(raw) => self.preserved.push(raw.to_vec()),
                CuratedLine::Added(entry) => self.entries.push((
                    entry.identity.label.into(),
                    entry.identity.path.into(),
                    entry.identity.program.into(),
                    entry.sha256.into(),
                )),
            }
        }
        self.error.clone().map_or(Ok(()), Err)
    }
}
struct Fixture {
    events: Events,
    lock: Lock,
    launch: Launch,
    source: Source,
    publish: Publish,
}
impl Fixture {
    fn new() -> Self {
        let events = Rc::new(RefCell::new(Vec::new()));
        Self {
            lock: Lock {
                events: events.clone(),
                fails: false,
            },
            launch: Launch {
                events: events.clone(),
                answer: Ok(CapturedAgent {
                    path: "/home/agent.plist".into(),
                    program: "/home/runner /home/arg".into(),
                    sha256: "a".repeat(64),
                }),
            },
            source: Source {
                events: events.clone(),
                path: Ok("/source".into()),
                lines: Ok(vec![
                    SourceLine::Preserved(b"# retained".to_vec()),
                    SourceLine::Object {
                        label: Some("my.alpha".into()),
                        raw: b"old alpha".to_vec(),
                    },
                    SourceLine::Object {
                        label: Some("my.beta".into()),
                        raw: b"beta".to_vec(),
                    },
                ]),
                contains: true,
                listed: b"  # not a comment\n{bad\n".to_vec(),
            },
            publish: Publish {
                events: events.clone(),
                error: None,
                preserved: vec![],
                entries: vec![],
            },
            events,
        }
    }
    fn run(&mut self, command: AllowlistCommand<'_>) -> Result<CurationOutcome, CurationFailure> {
        CurateAllowlist {
            home: Path::new("/home"),
            launchd: &mut self.launch,
            source: &mut self.source,
            publisher: &mut self.publish,
            lock: &self.lock,
        }
        .run(command)
    }
    fn events(&self) -> Vec<&'static str> {
        self.events.borrow().clone()
    }
}

#[test]
fn add_captures_before_source_and_holds_the_lock_through_publication() {
    let mut f = Fixture::new();
    assert_eq!(
        f.run(AllowlistCommand::Add("my.alpha")),
        Ok(CurationOutcome::Allowed {
            label: "my.alpha".into(),
            program: "/home/runner /home/arg".into()
        })
    );
    assert_eq!(
        f.events(),
        ["lock", "capture", "resolve", "read", "publish", "unlock"]
    );
    assert_eq!(
        f.publish.preserved,
        [b"# retained".to_vec(), b"beta".to_vec()]
    );
    assert_eq!(
        f.publish.entries,
        [(
            "my.alpha".into(),
            "~/agent.plist".into(),
            "~/runner ~/arg".into(),
            "a".repeat(64)
        )]
    );
}
#[test]
fn deny_publishes_only_retained_lines_without_capturing_an_agent() {
    let mut f = Fixture::new();
    assert_eq!(
        f.run(AllowlistCommand::Deny("my.alpha")),
        Ok(CurationOutcome::Denied("my.alpha".into()))
    );
    assert_eq!(
        f.events(),
        ["lock", "resolve", "contains", "read", "publish", "unlock"]
    );
    assert_eq!(
        f.publish.preserved,
        [b"# retained".to_vec(), b"beta".to_vec()]
    );
    assert!(f.publish.entries.is_empty());
}
#[test]
fn a_literal_deny_miss_skips_even_a_corrupt_source_and_publication() {
    let mut f = Fixture::new();
    f.source.contains = false;
    f.source.lines = Ok(vec![SourceLine::Invalid]);
    assert_eq!(
        f.run(AllowlistCommand::Deny("my.alpha")),
        Ok(CurationOutcome::NotPresent("my.alpha".into()))
    );
    assert_eq!(f.events(), ["lock", "resolve", "contains", "unlock"]);
}
#[test]
fn list_returns_deployed_raw_entries_without_a_lock_or_source_lookup() {
    let mut f = Fixture::new();
    f.lock.fails = true;
    assert_eq!(
        f.run(AllowlistCommand::List),
        Ok(CurationOutcome::Listed(f.source.listed.clone()))
    );
    assert_eq!(f.events(), ["list"]);
}
#[test]
fn lock_setup_failure_refuses_before_validation_or_any_source_effect() {
    let mut f = Fixture::new();
    f.lock.fails = true;
    assert_eq!(
        f.run(AllowlistCommand::Add("com.apple.bad")),
        Err(CurationFailure::Lock)
    );
    assert_eq!(f.events(), ["lock"]);
}
#[test]
fn an_invalid_label_releases_its_lock_without_capture_or_publication() {
    let mut f = Fixture::new();
    assert_eq!(
        f.run(AllowlistCommand::Add("com.apple.bad")),
        Err(CurationFailure::InvalidLabel("com.apple.bad".into()))
    );
    assert_eq!(f.events(), ["lock", "unlock"]);
}
#[test]
fn capture_failure_never_resolves_or_writes_source() {
    for error in [
        CaptureRefusal::NoAgent,
        CaptureRefusal::Hash("/missing".into()),
    ] {
        let mut f = Fixture::new();
        f.launch.answer = Err(error.clone());
        assert_eq!(
            f.run(AllowlistCommand::Add("my.alpha")),
            Err(CurationFailure::Capture(error))
        );
        assert_eq!(f.events(), ["lock", "capture", "unlock"]);
    }
}
#[test]
fn source_resolution_failure_does_not_publish_or_fall_back_to_deployed_state() {
    let mut f = Fixture::new();
    f.source.path = Err(SourceRefusal::Resolve);
    assert_eq!(
        f.run(AllowlistCommand::Deny("my.alpha")),
        Err(CurationFailure::Source(SourceRefusal::Resolve))
    );
    assert_eq!(f.events(), ["lock", "resolve", "unlock"]);
}
#[test]
fn an_invalid_source_line_refuses_the_entire_change() {
    let mut f = Fixture::new();
    f.source.lines.as_mut().unwrap().push(SourceLine::Invalid);
    assert_eq!(
        f.run(AllowlistCommand::Deny("my.alpha")),
        Err(CurationFailure::InvalidLine("/source".into()))
    );
    assert_eq!(
        f.events(),
        ["lock", "resolve", "contains", "read", "unlock"]
    );
}
#[test]
fn source_read_failure_cannot_be_reported_as_a_successful_deny() {
    let mut f = Fixture::new();
    f.source.lines = Err(SourceRefusal::Read);
    assert_eq!(
        f.run(AllowlistCommand::Deny("my.alpha")),
        Err(CurationFailure::Source(SourceRefusal::Read))
    );
    assert_eq!(
        f.events(),
        ["lock", "resolve", "contains", "read", "unlock"]
    );
}
#[test]
fn publication_failure_is_preserved_and_releases_the_guard() {
    let mut f = Fixture::new();
    let error = PublicationRefusal::ManifestRefresh("/runner".into());
    f.publish.error = Some(error.clone());
    assert_eq!(
        f.run(AllowlistCommand::Deny("my.alpha")),
        Err(CurationFailure::Publication(error))
    );
    assert_eq!(
        f.events(),
        ["lock", "resolve", "contains", "read", "publish", "unlock"]
    );
}
#[test]
fn unrelated_entry_bytes_survive_curation_without_utf8_replacement() {
    let mut f = Fixture::new();
    f.source.lines = Ok(vec![SourceLine::Object {
        label: Some("other".into()),
        raw: vec![b'"', 255, b'"'],
    }]);
    assert_eq!(
        f.run(AllowlistCommand::Deny("my.alpha")),
        Ok(CurationOutcome::Denied("my.alpha".into()))
    );
    assert_eq!(f.publish.preserved, [vec![b'"', 255, b'"']]);
}
