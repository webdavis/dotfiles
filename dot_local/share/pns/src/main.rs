//! The relay binary: the composition root, and the only place with a main.
//!
//! Everything here is WIRING. The registrations below are the roster, the
//! environment and the config are read once at this edge, and every decision
//! is delegated to the library. It exits 0 on every path, because a
//! notification must never fail the work it reports on.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use pns::args::parse_args;
use pns::channels::banner::BannerChannel;
use pns::channels::hermes::{
    DEFAULT_HERMES_URL, HermesChannel, UreqSignedPost, hermes_secret, remote_deadline,
};
use pns::channels::hue::{Bridge, HuePulse, hue_settings};
use pns::channels::moshi::{DEFAULT_MOSHI_URL, MoshiChannel, UreqPost, moshi_secret, read_auth};
use pns::channels::{Delivery, native_first};
use pns::config::{LoadOutcome, config_path, load_config};
use pns::engine::{Overrides, decide};
use pns::registry::{Registry, select_plugins};
use pns::render;
use pns::system::{SystemCommandRunner, SystemProbes};

fn main() {
    // The pulse is a MODE, not a leg: it fires on a long command's exit code
    // rather than on an event, so it leaves before any of the event wiring.
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("pulse")) {
        pulse_mode();
        return;
    }

    // Lossy rather than validating: a stray byte in argv degrades into an
    // unknown token, which the lenient contract already skips, instead of
    // aborting an always-exit-0 notification.
    let (event, warnings) = parse_args(
        std::env::args_os()
            .skip(1)
            .map(|argument| argument.to_string_lossy().into_owned()),
    );
    for warning in &warnings {
        eprintln!("relay: {warning}");
    }

    let registry = roster();

    let home = std::env::var("HOME").unwrap_or_default();
    let (selection, warning) = select_plugins(&registry, load_config(&config_path(&home)));
    if let Some(warning) = warning {
        eprintln!("{warning}");
    }

    let overrides = Overrides::from_env(
        &std::env::vars_os()
            .map(|(name, value)| {
                (
                    name.to_string_lossy().into_owned(),
                    value.to_string_lossy().into_owned(),
                )
            })
            .collect::<BTreeMap<_, _>>(),
    );
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|since_epoch| since_epoch.as_secs());
    let probes = SystemProbes::new(
        SystemCommandRunner,
        resolve_path(
            std::env::var("PNS_PHONE_MARKER_FILE").ok().as_deref(),
            &format!("{home}/.local/state/pns/phone-attention.marker"),
        )
        .to_string_lossy()
        .into_owned(),
    );

    let decision = decide(
        &probes,
        &selection,
        &overrides,
        event.local_only,
        event.remote_only,
        &event.pane,
        now_secs,
    );

    if decision.legs.is_empty() {
        // A verdict that must be SAID, but only for the contradiction the
        // caller asked for: a silent exit is indistinguishable from delivery.
        if event.local_only && event.remote_only {
            println!(
                "relay: post SKIPPED -- --local-only and --remote-only were both given, which suppresses every channel; nothing was sent"
            );
        }
        return;
    }

    // Sanitized ONCE here rather than per channel: a channel may be written in
    // any language and cannot be expected to share the guard. Warned about
    // only now, because a scrub nobody was going to receive is not news.
    let pane = if decision.pane_dropped {
        eprintln!(
            "relay: dropped a pane id with shell metacharacters; no channel will focus a pane"
        );
        ""
    } else {
        event.pane.as_str()
    };

    let message = render::message(&event.branch, &event.detail, &event.state);
    let channels_dir_override = std::env::var("PNS_CHANNELS_DIR")
        .ok()
        .filter(|dir| !dir.is_empty());
    let channels_dir = resolve_path(
        channels_dir_override.as_deref(),
        &format!("{home}/.local/libexec/pns/channels"),
    );

    let banner = BannerChannel {
        runner: SystemCommandRunner,
        // THE SAME probe set the engine read, by reference: a second one
        // would take its own idle and focus readings a few milliseconds
        // later, so the suppression could disagree with the routing that
        // just ran on the same event.
        probes: &probes,
        // An EMPTY override falls through, so an exported-but-blank variable
        // cannot shadow the inherited bundle id.
        terminal_id: std::env::var("PNS_TERMINAL_BUNDLE_ID")
            .ok()
            .filter(|id| !id.is_empty())
            .or_else(|| {
                std::env::var("__CFBundleIdentifier")
                    .ok()
                    .filter(|id| !id.is_empty())
            })
            .unwrap_or_default(),
        // A garbled idle override leaves the THRESHOLD unknown, which is what
        // makes the banner unsuppressable without a third override state: the
        // bash keeps the garbled string, fails its numeric test and fires, so
        // consulting the live probe here could drop a banner instead.
        desk_idle_secs: if overrides.desk_invalid || overrides.idle_invalid {
            None
        } else {
            Some(
                overrides
                    .desk_idle_secs
                    .unwrap_or(pns::engine::DEFAULT_DESK_IDLE_SECS),
            )
        },
        herdr_path: executable_in_path("herdr"),
        idle_override: overrides.idle_secs,
        focused_override: overrides.focused_pane.clone(),
    };
    let native = pns::channels::Event {
        agent: event.agent.clone(),
        state: event.state.clone(),
        project: event.project.clone(),
        branch: event.branch.clone(),
        detail: event.detail.clone(),
        title: render::title(&event.agent, &event.state, &event.project),
        preview: render::preview(&message),
        message,
        pane: pane.to_string(),
    };

    let auth_path = resolve_path(
        std::env::var("RELAY_AUTH_FILE").ok().as_deref(),
        &format!("{home}/.config/relay/auth.json"),
    );
    // ONE READ of the auth file for the whole event: two channels wanting two
    // secrets out of one file is not two reasons to open it, and a file that
    // changed between them would sign one leg with a key the other did not
    // have.
    let auth = read_auth(&auth_path);
    let moshi = MoshiChannel {
        http: UreqPost::default(),
        token: auth.as_deref().and_then(moshi_secret),
        url: std::env::var("RELAY_MOSHI_URL")
            .ok()
            .filter(|url| !url.is_empty())
            .unwrap_or_else(|| DEFAULT_MOSHI_URL.to_string()),
    };

    let hermes = HermesChannel {
        post: UreqSignedPost,
        key: auth.as_deref().and_then(hermes_secret),
        auth_path,
        url: std::env::var("RELAY_HERMES_URL")
            .ok()
            .filter(|url| !url.is_empty())
            .unwrap_or_else(|| DEFAULT_HERMES_URL.to_string()),
        sync_deadline: remote_deadline(std::env::var("RELAY_REMOTE_TIMEOUT").ok().as_deref()),
    };

    for leg in &decision.legs {
        let delivered = if native_first(channels_dir_override.is_some()) {
            match leg.name {
                "macos-banner" => Some(banner.deliver(&native, leg.mode)),
                "moshi" => Some(moshi.deliver(&native, leg.mode)),
                "hermes" => Some(hermes.deliver(&native, leg.mode)),
                _ => None,
            }
        } else {
            None
        }
        .unwrap_or_else(|| {
            deliver(
                &channels_dir.join(format!("{}.sh", leg.name)),
                &native.to_json(leg.mode),
            )
        });
        // THE ONE PLACE a delivery reaches the operator. A channel says what
        // happened; whether anyone hears it is the leg's reporting mode, and
        // that rule lives here rather than in three channels.
        if let Some(line) = delivered.line_for(leg.mode) {
            println!("{line}");
        }
    }
}

/// A path from the environment, defaulting like bash's `${VAR:-default}`:
/// EMPTY means the default as much as unset does, because joining a filename
/// to an empty path resolves into the current directory and quietly delivers
/// nothing.
pub fn resolve_path(candidate: Option<&str>, default: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(
        candidate
            .filter(|value| !value.is_empty())
            .unwrap_or(default),
    )
}

/// THE ROSTER, and the only statement of delivery order. A destination is
/// added to `registry::ROSTER`, never to policy; this turns those
/// declarations into the registry both modes select from.
fn roster() -> Registry {
    let mut registry = Registry::new();
    for (name, routing) in pns::registry::ROSTER {
        if let Err(error) = registry.register(name, routing) {
            eprintln!("pns: {error:?}");
        }
    }
    registry
}

/// The first executable of that name on PATH, absolute, or None. The click
/// string bakes it in because the click runs in a bare launchd context whose
/// PATH cannot find `~/.local/bin`.
fn executable_in_path(name: &str) -> Option<String> {
    use std::os::unix::fs::PermissionsExt;
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|directory| directory.join(name))
        .find(|candidate| {
            std::fs::metadata(candidate)
                .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        })
        .map(|path| path.to_string_lossy().into_owned())
}

/// Hand one channel its event on stdin. A channel that is missing, is not
/// executable, or fails is not an error: it is simply not installed, or it
/// declined, and neither may take down the siblings or the caller.
///
/// SILENT BY DESIGN, and deliberately so now that it says so in the type: the
/// common failure here is a channel nobody installed, and reporting that on
/// every event would be noise. The status is still dropped; what changed is
/// that it is dropped in one visible place instead of implicitly.
fn deliver(channel: &Path, event: &str) -> Delivery {
    let Ok(mut child) = Command::new(channel).stdin(Stdio::piped()).spawn() else {
        return Delivery::Silent;
    };
    if let Some(mut stdin) = child.stdin.take() {
        // Newline-terminated, as the bash's `jq -cn` emitted it: a channel
        // reading one line with `read -r` gets nothing without it.
        let _ = stdin.write_all(event.as_bytes());
        let _ = stdin.write_all(b"\n");
    }
    let _ = child.wait();
    Delivery::Silent
}

/// The `pulse` mode: read the hue table, take the single-pulse lock, and run
/// the sequence against the bridge. Every absence is a silent exit 0.
fn pulse_mode() {
    let home = std::env::var("HOME").unwrap_or_default();
    let loaded = load_config(&config_path(&home));
    // The settings come off the config, the ENABLED verdict comes off the
    // same selection an event uses: hue is a registered plugin that happens
    // not to be an event leg, not a special case wired past the registry.
    let settings = match &loaded {
        Ok(LoadOutcome::Loaded(config)) => {
            config.plugins.get("hue").map(|hue| hue.settings.clone())
        }
        _ => None,
    };
    let (selection, warning) = select_plugins(&roster(), loaded);
    // The SAME sanitized warning event mode prints. A config that is merely
    // absent stays silent, because never opting in is not a mistake; one that
    // could not be read is the operator's to know about, whichever mode found
    // it.
    if let Some(warning) = warning {
        eprintln!("{warning}");
    }
    if !selection.iter().any(|entry| entry.name == "hue") {
        return;
    }
    let Some(hue) = settings.and_then(|settings| {
        hue_settings(&settings, std::env::var("HUE_PULSE_ROOMS").ok().as_deref())
    }) else {
        return;
    };

    HuePulse {
        bridge: UreqBridge {
            base: format!("https://{}/clip/v2/resource", hue.bridge),
            key: hue.key,
        },
        rooms: hue.rooms,
    }
    .run(
        &std::env::args_os()
            .nth(2)
            .map(|code| code.to_string_lossy().into_owned())
            .unwrap_or_else(|| "0".to_string()),
    );
}

/// The CLIP v2 bridge over ureq.
struct UreqBridge {
    base: String,
    key: String,
}

impl UreqBridge {
    fn agent(&self) -> ureq::Agent {
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(10)))
            .max_redirects(0)
            // The bridge serves a self-signed certificate for its own LAN
            // address, so verification is disabled here exactly as openhue
            // does it; there is no CA that could vouch for a Hue bridge.
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .disable_verification(true)
                    .build(),
            )
            .build()
            .new_agent()
    }
}

impl Bridge for UreqBridge {
    fn get(&self, path: &str) -> Option<String> {
        self.agent()
            .get(format!("{}/{path}", self.base))
            .header("hue-application-key", &self.key)
            .call()
            .ok()?
            .body_mut()
            .read_to_string()
            .ok()
    }

    fn put(&self, path: &str, body: &str) {
        // Nothing reads the outcome: a pulse that did not land is not worth
        // failing, reporting or retrying on a notification path.
        let _ = self
            .agent()
            .put(format!("{}/{path}", self.base))
            .header("hue-application-key", &self.key)
            .content_type("application/json")
            .send(body);
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_path;

    #[test]
    fn an_empty_channels_dir_variable_means_the_default_not_the_current_dir() {
        // Bash's ${VAR:-default} defaults on EMPTY as well as unset; joining
        // a filename to an empty path would quietly deliver nothing.
        assert_eq!(
            resolve_path(Some(""), "/fallback/channels"),
            std::path::PathBuf::from("/fallback/channels")
        );
        assert_eq!(
            resolve_path(None, "/fallback/channels"),
            std::path::PathBuf::from("/fallback/channels")
        );
        assert_eq!(
            resolve_path(Some("/set/dir"), "/fallback/channels"),
            std::path::PathBuf::from("/set/dir")
        );
    }
}
