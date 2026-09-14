use super::*;
use crate::{InspectionFailure, SshCommandResult, SshCompleted, SshFile, SshScanFailure};
use posture_domain::{SshAttributes, SshRecord};
use std::{
    cell::RefCell,
    collections::VecDeque,
    io::{self, Write},
    path::PathBuf,
    rc::Rc,
};
pub(super) type Trace = Rc<RefCell<Vec<String>>>;
pub(super) fn good(text: &str) -> SshCommandResult {
    Ok(SshCompleted {
        status: 0,
        output: text.as_bytes().to_vec(),
    })
}
pub(super) fn record(checksum: u8) -> Vec<SshRecord> {
    vec![SshRecord {
        path: b"/fixture/main".to_vec(),
        attributes: SshAttributes {
            mode: 0o644,
            uid: 0,
            gid: 0,
        },
        checksum: [checksum; 32],
    }]
}
pub(super) struct Fixture {
    pub(super) trace: Trace,
    pub(super) prime: SshCommandResult,
    pub(super) syntax: SshCommandResult,
    pub(super) ports: SshCommandResult,
    pub(super) loaded: VecDeque<SshCommandResult>,
    pub(super) restart: SshCommandResult,
    pub(super) trees: RefCell<VecDeque<Result<Vec<SshRecord>, SshScanFailure>>>,
    pub(super) banners: VecDeque<SshCommandResult>,
    pub(super) available: bool,
    pub(super) verification: SshVerification,
}
impl Fixture {
    pub(super) fn new() -> Self {
        Self {
            trace: Rc::new(RefCell::new(vec![])),
            prime: good(""),
            syntax: good(""),
            ports: good("port 22\nport 2222\nport 22\n"),
            loaded: VecDeque::from([good("loaded"), good("loaded")]),
            restart: good(""),
            trees: RefCell::new(VecDeque::from([
                Ok(record(0)),
                Ok(record(0)),
                Ok(record(0)),
            ])),
            banners: VecDeque::from([good("localhost ssh-ed25519 key\n")]),
            available: true,
            verification: SshVerification::Verified,
        }
    }
    pub(super) fn run(
        &mut self,
        ready: Result<SshReadiness, ReadinessRefusal>,
        broken_output: bool,
    ) -> (u8, String, String) {
        let mut files = Files {
            trace: self.trace.clone(),
            prime: std::mem::replace(&mut self.prime, good("")),
        };
        let mut ssh = Ssh {
            trace: self.trace.clone(),
            syntax: std::mem::replace(&mut self.syntax, good("")),
            ports: std::mem::replace(&mut self.ports, good("")),
        };
        let mut service = Service {
            trace: self.trace.clone(),
            loaded: std::mem::take(&mut self.loaded),
            restart: std::mem::replace(&mut self.restart, good("")),
        };
        let mut banners = Banners {
            trace: self.trace.clone(),
            available: self.available,
            answers: std::mem::take(&mut self.banners),
        };
        let verification =
            std::mem::replace(&mut self.verification, SshVerification::Failed(vec![]));
        let mut verification = Some(verification);
        let trace = self.trace.clone();
        let mut verify = || {
            trace.borrow_mut().push("verify".into());
            verification.take().unwrap()
        };
        let mut pause = |duration: Duration| {
            trace
                .borrow_mut()
                .push(format!("pause:{}", duration.as_millis()))
        };
        let mut out = Output {
            trace: self.trace.clone(),
            bytes: vec![],
            broken: broken_output,
        };
        let mut err = Vec::new();
        let status = reload_ssh(
            &mut SshReload {
                files: &mut files,
                sshd: &mut ssh,
                tree: self,
                launchctl: &mut service,
                banners: &mut banners,
                verify: &mut verify,
                pause: &mut pause,
            },
            ready,
            &mut SshOutput {
                stdout: &mut out,
                stderr: &mut err,
            },
        );
        (
            status,
            String::from_utf8(out.bytes).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }
}
impl SshTree for Fixture {
    fn scan(&self) -> Vec<SshScanFailure> {
        panic!("verification owns the scan")
    }
    fn observe(&self) -> Result<Vec<SshRecord>, SshScanFailure> {
        self.trace.borrow_mut().push("observe".into());
        self.trees.borrow_mut().pop_front().unwrap()
    }
}
struct Files {
    trace: Trace,
    prime: SshCommandResult,
}
impl SshInstallFiles for Files {
    fn path(&self, _: SshFile) -> PathBuf {
        PathBuf::from("/fixture/dropins/000-ssh-hardening.conf")
    }
    fn prime(&mut self) -> SshCommandResult {
        self.trace.borrow_mut().push("prime".into());
        std::mem::replace(&mut self.prime, good(""))
    }
    fn directory_exists(&self) -> bool {
        panic!("reload cannot install")
    }
    fn exists(&self, _: SshFile) -> Result<bool, InspectionFailure> {
        panic!("reload cannot install")
    }
    fn stage(&mut self, _: &[u8]) -> SshCommandResult {
        panic!("reload cannot write")
    }
    fn chmod(&mut self) -> SshCommandResult {
        panic!("reload cannot write")
    }
    fn save(&mut self) -> SshCommandResult {
        panic!("reload cannot write")
    }
    fn rename(&mut self, _: SshFile, _: SshFile) -> SshCommandResult {
        panic!("reload cannot write")
    }
    fn remove(&mut self, _: &[SshFile]) -> SshCommandResult {
        panic!("reload never rolls back")
    }
}
struct Ssh {
    trace: Trace,
    syntax: SshCommandResult,
    ports: SshCommandResult,
}
impl Sshd for Ssh {
    fn available(&self) -> bool {
        true
    }
    fn global(&mut self) -> SshCommandResult {
        self.trace.borrow_mut().push("ports".into());
        std::mem::replace(&mut self.ports, good(""))
    }
    fn syntax(&mut self) -> SshCommandResult {
        self.trace.borrow_mut().push("syntax".into());
        std::mem::replace(&mut self.syntax, good(""))
    }
    fn connection(&mut self, _: &str) -> SshCommandResult {
        panic!("separate verifier")
    }
}
struct Service {
    trace: Trace,
    loaded: VecDeque<SshCommandResult>,
    restart: SshCommandResult,
}
impl SshLaunchctl for Service {
    fn probe(&mut self) -> SshCommandResult {
        self.trace.borrow_mut().push("probe".into());
        self.loaded.pop_front().unwrap()
    }
    fn restart(&mut self) -> SshCommandResult {
        self.trace.borrow_mut().push("restart".into());
        std::mem::replace(&mut self.restart, good(""))
    }
}
struct Banners {
    trace: Trace,
    available: bool,
    answers: VecDeque<SshCommandResult>,
}
impl SshBannerProbe for Banners {
    fn available(&self) -> bool {
        self.trace.borrow_mut().push("prover".into());
        self.available
    }
    fn probe(&mut self, port: u16, timeout: u32) -> SshCommandResult {
        self.trace
            .borrow_mut()
            .push(format!("banner:{port}:{timeout}"));
        self.answers.pop_front().unwrap_or_else(|| good(""))
    }
}
struct Output {
    trace: Trace,
    bytes: Vec<u8>,
    broken: bool,
}
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        if self
            .bytes
            .windows(b"about to restart".len())
            .any(|s| s == b"about to restart")
        {
            self.trace.borrow_mut().push("warning-flush".into());
            if self.broken {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
        }
        Ok(())
    }
}
pub(super) fn ready() -> Result<SshReadiness, ReadinessRefusal> {
    SshReadiness::parse("2", ".005", "1")
}
