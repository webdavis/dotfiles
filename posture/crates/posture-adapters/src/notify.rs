//! How one security page reaches the operator, read from posture's own config
//! file.
//!
//! THREE MODES, ONE CHOICE, and posture names no engine. Either it signs the
//! page itself and posts it to a hermes webhook route, or it hands the page to
//! a command the operator configured and reads that command's answer back, or
//! nothing leaves the machine at all. The second mode is the producer API: a
//! JSON request on standard input, a JSON result plus an exit code out. Any
//! program may implement it, and which program does is a per-machine choice
//! rather than a property of this tool.
//!
//! IT ALSO OWNS THE `[jobs]` TABLE of that same file, which names the launchd
//! label of each job posture installs. The two are read through one schema
//! because one file declares both.
//!
//! FAIL CLOSED, NEVER SILENT. With no config file, and with one this build
//! cannot use, the choice is a hermes path holding no key at all, and a keyless
//! route refuses the page and raises the local banner. A malformed file
//! therefore costs a loud refusal per page rather than a pipeline that quietly
//! stops paging, which is the failure a security tool cannot afford. `off` is
//! the same discipline stated deliberately: it turns off DELIVERY, not the
//! page, so the finding still reaches the local banner.
//!
//! LOUD MEANS THE BANNER, not a log line. A config this build cannot use takes
//! every destination away at once, and the banner is the one that needs no
//! config to work, so a refusal is raised there as well as written to the job's
//! diagnostics; `posture doctor` answers the same question on demand.

use crate::banner_only::BannerOnly;
use crate::hermes::{CriticalCopy, HermesWebhook};
use crate::producer::ProducerCommand;
use crate::wire::Name;
use crate::{CommandRunner, UreqSignedPost};
use posture_application::{AlertSink, IndependentAlarm};
use posture_domain::{AgentLabels, DailyTimes, Severity, severity_route};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

mod schema;

/// The hermes gateway a page is posted to when the file names none: the local
/// webhook base, one path segment above the route.
pub const DEFAULT_WEBHOOK_BASE: &str = "http://127.0.0.1:8644/webhooks";

/// The route a page takes when its own tier names none: the heartbeat, the
/// digest, the cursor-reset warning and every finding below critical. A
/// critical finding overrides it; see `posture_domain::severity_route`.
///
/// ONE NAME, AND THE CONFIG FILE OWNS IT. `[notify] route` states it per
/// machine, because which channel a page lands in is the operator's gateway's
/// business rather than this tool's; this value is what they get without
/// saying.
const DEFAULT_ROUTE: &str = "posture-pages";

/// Where the rolling hour of critical-page copies is recorded, under the home
/// directory the choice was read for. Beside posture's cursor and digest
/// spool, because it is state of the same kind: small, per-machine, and worth
/// nothing to anyone who finds it.
const COPY_WINDOW: &str = ".local/state/posture-critical-copy-window.json";

/// Where posture's notify choice lives for a given home directory. Pure, so
/// the path rule is testable without an environment.
pub fn config_path(home: &Path) -> PathBuf {
    home.join(".config/posture/config.toml")
}

/// What posture's config says about its own scheduled jobs: the launchd label
/// of each, and when the two daily ones fire.
///
/// A file that cannot be read or parsed leaves every job at its shipped
/// default. The same file's notify half already reports that refusal loudly
/// and refuses to deliver, so nothing here is the one thing that tells the
/// operator their config is broken.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JobSettings {
    pub labels: AgentLabels,
    pub daily: DailyTimes,
    /// A stated value that was ignored, one line each.
    pub warnings: Vec<String>,
}

/// The `[jobs]` half of posture's config, for this home directory.
pub fn job_settings(home: &Path) -> JobSettings {
    let Some(jobs) = std::fs::read_to_string(config_path(home))
        .ok()
        .and_then(|text| toml::from_str::<schema::File>(&text).ok())
        .map(|file| file.jobs)
    else {
        return JobSettings::default();
    };
    let (daily, warnings) = jobs.times();
    JobSettings {
        labels: jobs.into_labels(),
        daily,
        warnings,
    }
}

/// The launchd label of each of posture's own jobs, for this home directory.
pub fn agent_labels(home: &Path) -> AgentLabels {
    job_settings(home).labels
}

/// The three ways one page can be raised.
#[derive(Clone, PartialEq, Eq)]
pub enum NotifyMode {
    /// The page is written as JSON to the standard input of `path`, run with
    /// `arguments` verbatim, and the result envelope read back off its output.
    Command {
        path: PathBuf,
        arguments: Vec<String>,
    },
    /// The page is signed with the route's key and posted to
    /// `<base_url>/<route>`.
    Hermes {
        base_url: String,
        keys: BTreeMap<String, String>,
        /// A second route a delivered critical page is copied to, when the
        /// file names one. `None` is one post, which is the default.
        critical_copy: Option<CriticalCopy>,
    },
    /// Nothing leaves this machine. The page is raised on the local banner,
    /// which is the one channel that needs no delivery at all.
    Off,
}

/// WRITTEN BY HAND SO NO SIGNING KEY IS EVER FORMATTED. `keys` holds one
/// webhook signing key per route and this type is public, so a derived `Debug`
/// would put every secret into whatever line formats a choice: a diagnostic
/// in a launchd-run security tool, or the message a failed `assert_eq!` prints
/// in CI. The route NAMES are the part a reader needs, and they are what this
/// prints.
impl std::fmt::Debug for NotifyMode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotifyMode::Command { path, arguments } => formatter
                .debug_struct("Command")
                .field("path", path)
                .field("arguments", arguments)
                .finish(),
            NotifyMode::Hermes {
                base_url,
                keys,
                critical_copy,
            } => formatter
                .debug_struct("Hermes")
                .field("base_url", base_url)
                .field("keys", &keys.keys().collect::<Vec<_>>())
                .field("critical_copy", critical_copy)
                .finish(),
            NotifyMode::Off => formatter.write_str("Off"),
        }
    }
}

/// The notify choice, and whatever went wrong reading it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notify {
    pub mode: NotifyMode,
    /// The route a page with no tier of its own is posted to, or handed to the
    /// producer under. Validated when the file is read, so it is a path
    /// segment exactly as written.
    pub route: String,
    /// Why the config file could not be used, when it could not. The
    /// fail-closed default stands and every page then refuses loudly, so this
    /// is what the operator is told rather than a second outcome to branch on:
    /// the job's log, the local banner and `posture doctor` all report it.
    pub refusal: Option<String>,
    /// Every key the file states that this build does not read, one line each.
    /// Delivery is configured and runs; these say what was ignored.
    pub warnings: Vec<String>,
}

/// Drop every double-quoted excerpt from a toml parse error's message. A
/// value error frames the offending value between quotes (`invalid type:
/// string "s3cret", expected a map`), and the config's own signing keys are
/// exactly the strings that land there; a structural refusal (an unknown
/// mode word, a missing table) never quotes anything, so this costs it
/// nothing. Quotes escaped inside the value (`\"`) are consumed with it
/// rather than ending the redaction early.
fn redact_quoted(message: &str) -> String {
    let mut result = String::with_capacity(message.len());
    let mut chars = message.chars();
    while let Some(c) = chars.next() {
        if c != '"' {
            result.push(c);
            continue;
        }
        result.push_str("\"<redacted>\"");
        while let Some(inner) = chars.next() {
            match inner {
                '\\' => {
                    chars.next();
                }
                '"' => break,
                _ => {}
            }
        }
    }
    result
}

impl Default for Notify {
    /// The fail-closed choice: the local gateway and no key for any route, so
    /// nothing is delivered quietly and every attempt says so.
    fn default() -> Self {
        Notify {
            mode: NotifyMode::Hermes {
                base_url: DEFAULT_WEBHOOK_BASE.to_string(),
                keys: BTreeMap::new(),
                critical_copy: None,
            },
            route: DEFAULT_ROUTE.to_string(),
            refusal: None,
            warnings: Vec::new(),
        }
    }
}

impl Notify {
    /// Read the choice for this home directory. A file that is not there is the
    /// default and no refusal; every other failure keeps the default and names
    /// itself.
    ///
    /// A DANGLING SYMLINK IS NOT AN ABSENT FILE, though the kernel reports both
    /// as NotFound. An operator may point this path at a store of their own,
    /// an external volume or a checkout, and a link whose target went away is
    /// a CONFIGURED machine whose file stopped resolving. Either way no page
    /// is delivered, because the fail-closed default holds no key; the
    /// difference is whether the operator is told WHY, and a broken link read
    /// as an unconfigured machine is told nothing.
    pub fn read(home: &Path) -> Self {
        let path = config_path(home);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    && std::fs::symlink_metadata(&path).is_err() =>
            {
                return Notify::default();
            }
            Err(error) => return Notify::refused(format!("{}: {error}", path.display())),
        };
        match Self::parse(&text, home) {
            Ok(notify) => notify,
            Err(refusal) => Notify::refused(refusal),
        }
    }

    fn refused(reason: String) -> Self {
        Notify {
            refusal: Some(reason),
            ..Notify::default()
        }
    }

    /// The pure half: text and the home directory its relative state paths
    /// hang off in, one notify choice or a named refusal out. Pure in both
    /// arguments, so the path rule is testable without an environment.
    fn parse(text: &str, home: &Path) -> Result<Notify, String> {
        let file: schema::File =
            toml::from_str(text).map_err(|error| redact_quoted(error.message().trim()))?;
        let warnings = file.unread_keys();
        let route = file.notify.route();
        Ok(Notify {
            mode: file.notify.into_mode(home)?,
            route,
            refusal: None,
            warnings,
        })
    }

    /// The routes this choice will post to that name no signing key of their
    /// own, empty for every mode but `Hermes`: `Command` and `Off` name no
    /// route to check. Two routes are asked about, the configured untiered
    /// route and the `priority` route `severity_route` hands a critical
    /// finding.
    pub fn missing_hermes_keys(&self) -> Vec<&str> {
        let NotifyMode::Hermes { keys, .. } = &self.mode else {
            return Vec::new();
        };
        let priority =
            severity_route(Some(Severity::Critical)).expect("a critical finding names a route");
        [self.route.as_str(), priority]
            .into_iter()
            .filter(|route| !keys.contains_key(*route))
            .collect()
    }

    /// Say what this config cost, wherever the operator will see it. The
    /// banner carries the refusal because a config that will not parse has
    /// already taken every other destination away.
    fn report(&self, alarm: &mut impl IndependentAlarm, diagnostics: &mut impl Write) {
        for warning in &self.warnings {
            let _ = writeln!(diagnostics, "posture: {warning}");
        }
        if let Some(refusal) = &self.refusal {
            let _ = writeln!(
                diagnostics,
                "posture: the notify config could not be used, so no page can be delivered: \
                 {refusal}"
            );
            let _ = alarm.alarm(
                "posture cannot deliver a page",
                &format!("the notify config could not be used: {refusal}"),
            );
        }
    }
}

/// The sink the configured mode calls for, with the route an untiered page
/// takes baked in, and the refusal reported once at construction.
///
/// The concrete sink is boxed because the three modes are different types and
/// every caller wants one word for "wherever a page goes". A page costs a
/// process or an HTTPS round trip, so one virtual call is free by comparison.
/// The box borrows for as long as the runner and alarm handed in do, so a
/// caller composing from borrowed collaborators needs no `'static` of its own.
pub fn alert_sink<'a, R: CommandRunner + 'a, A: IndependentAlarm + 'a>(
    notify: Notify,
    runner: R,
    mut alarm: A,
    diagnostics: &mut impl Write,
) -> Box<dyn AlertSink + 'a> {
    notify.report(&mut alarm, diagnostics);
    match notify.mode {
        NotifyMode::Command { path, arguments } => Box::new(ProducerCommand::new(
            runner,
            path,
            arguments,
            Some(route_or_default(notify.route)),
            alarm,
        )),
        NotifyMode::Hermes {
            base_url,
            keys,
            critical_copy,
        } => {
            let sink = HermesWebhook::new(
                UreqSignedPost,
                base_url,
                keys,
                route_or_default(notify.route),
                alarm,
            );
            match critical_copy {
                Some(copy) => Box::new(sink.copying(copy)),
                None => Box::new(sink),
            }
        }
        NotifyMode::Off => Box::new(BannerOnly::new(alarm)),
    }
}

/// A route read through `Notify::parse` is already validated, but `route` is
/// a public field on a public struct, so any other construction can hand in a
/// name the identifier rules refuse. Falling back to the shipped default
/// keeps a bad name from turning a config problem into a panic.
fn route_or_default(route: String) -> Name {
    Name::new(route)
        .unwrap_or_else(|_| Name::new(DEFAULT_ROUTE).expect("the shipped default route is valid"))
}

#[cfg(test)]
mod tests;
