//! Which way a page leaves this machine, read from posture's own config file.
//!
//! TWO PATHS, ONE CHOICE, and posture names no engine. Either it signs the page
//! itself and posts it to a hermes webhook route, or it hands the page to a
//! command the operator configured and reads that command's answer back. The
//! second path is the producer API: a JSON request on standard input, a JSON
//! result plus an exit code out. Any program may implement it, and which
//! program does is a per-machine choice rather than a property of this tool.
//!
//! FAIL CLOSED, NEVER SILENT. With no config file, and with one this build
//! cannot use, the choice is a hermes path holding no key at all, and a keyless
//! route refuses the page and raises the local banner. A malformed file
//! therefore costs a loud refusal per page rather than a pipeline that quietly
//! stops paging, which is the failure a security tool cannot afford.

use crate::hermes::HermesWebhook;
use crate::producer::ProducerCommand;
use crate::{CommandRunner, UreqSignedPost};
use posture_application::{AlertSink, IndependentAlarm};
use posture_producer_wire::Name;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

mod schema;

/// The hermes gateway a page is posted to when the file names none: the local
/// webhook base, one path segment above the route.
pub const DEFAULT_WEBHOOK_BASE: &str = "http://127.0.0.1:8644/webhooks";

/// The route a page takes when its own tier names none: the heartbeat, the
/// digest and the cursor-reset warning. A tiered finding overrides it; see
/// `posture_domain::severity_route`.
const UNTIERED_ROUTE: &str = "posture";

/// Where posture's delivery choice lives for a given home directory. Pure, so
/// the path rule is testable without an environment.
pub fn config_path(home: &Path) -> PathBuf {
    home.join(".config/posture/config.toml")
}

/// The two ways one page can leave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryPath {
    /// The page is written as JSON to the standard input of `command`, run with
    /// `arguments` verbatim, and the result envelope read back off its output.
    Producer {
        command: PathBuf,
        arguments: Vec<String>,
    },
    /// The page is signed with the route's key and posted to
    /// `<base_url>/<route>`.
    Hermes {
        base_url: String,
        keys: BTreeMap<String, String>,
    },
}

/// The delivery choice, and whatever went wrong reading it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivery {
    pub path: DeliveryPath,
    /// Why the config file could not be used, when it could not. The
    /// fail-closed default stands and every page then refuses loudly, so this
    /// is a line for the operator's log rather than a second outcome to branch
    /// on.
    pub refusal: Option<String>,
}

impl Default for Delivery {
    /// The fail-closed choice: the local gateway and no key for any route, so
    /// nothing is delivered quietly and every attempt says so.
    fn default() -> Self {
        Delivery {
            path: DeliveryPath::Hermes {
                base_url: DEFAULT_WEBHOOK_BASE.to_string(),
                keys: BTreeMap::new(),
            },
            refusal: None,
        }
    }
}

impl Delivery {
    /// Read the choice for this home directory. A file that is not there is the
    /// default and no refusal; every other failure keeps the default and names
    /// itself.
    ///
    /// A DANGLING SYMLINK IS NOT AN ABSENT FILE, though the kernel reports both
    /// as NotFound. chezmoi deploys configs as symlinks, so a broken link is a
    /// CONFIGURED machine whose file stopped resolving, and reading that as an
    /// unconfigured one would hide the breakage behind a default.
    pub fn read(home: &Path) -> Self {
        let path = config_path(home);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    && std::fs::symlink_metadata(&path).is_err() =>
            {
                return Delivery::default();
            }
            Err(error) => return Delivery::refused(format!("{}: {error}", path.display())),
        };
        match Self::parse(&text) {
            Ok(path) => Delivery {
                path,
                refusal: None,
            },
            Err(refusal) => Delivery::refused(refusal),
        }
    }

    fn refused(reason: String) -> Self {
        Delivery {
            refusal: Some(reason),
            ..Delivery::default()
        }
    }

    /// The pure half: text in, one delivery path or a named refusal out.
    fn parse(text: &str) -> Result<DeliveryPath, String> {
        let file: schema::File =
            toml::from_str(text).map_err(|error| error.message().trim().to_string())?;
        file.delivery.into_path()
    }
}

/// The sink the configured path calls for, with the route an untiered page
/// takes baked in, and the refusal reported once at construction.
///
/// The concrete sink is boxed because the two paths are different types and
/// every caller wants one word for "wherever a page goes". A page costs a
/// process or an HTTPS round trip, so one virtual call is free by comparison.
/// The box borrows for as long as the runner and alarm handed in do, so a
/// caller composing from borrowed collaborators needs no `'static` of its own.
pub fn alert_sink<'a, R: CommandRunner + 'a, A: IndependentAlarm + 'a>(
    delivery: Delivery,
    runner: R,
    alarm: A,
    diagnostics: &mut impl Write,
) -> Box<dyn AlertSink + 'a> {
    if let Some(refusal) = &delivery.refusal {
        let _ = writeln!(
            diagnostics,
            "posture: the delivery config could not be used, so no page can be delivered: {refusal}"
        );
    }
    let route = Name::new(UNTIERED_ROUTE).expect("the fixed untiered route is valid");
    match delivery.path {
        DeliveryPath::Producer { command, arguments } => Box::new(ProducerCommand::new(
            runner,
            command,
            arguments,
            Some(route),
            alarm,
        )),
        DeliveryPath::Hermes { base_url, keys } => Box::new(HermesWebhook::new(
            UreqSignedPost,
            base_url,
            keys,
            route,
            alarm,
        )),
    }
}

#[cfg(test)]
mod tests;
