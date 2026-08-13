//! The relay binary: the composition root, and the only place with a main.
//!
//! Everything here is WIRING. The registrations below are the roster, the
//! environment and the config are read once at this edge, and every decision
//! is delegated to the library. It exits 0 on every path, because a
//! notification must never fail the work it reports on.

use std::cell::OnceCell;
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
    event_mode();
}

/// One notification, end to end: read the edge, decide, render, dispatch.
fn event_mode() {
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

    let home = std::env::var("HOME").unwrap_or_default();
    let loaded = load_config(&config_path(&home));
    // Read off the config before selection consumes it: the pulse needs hue's
    // settings and the plan needs moshi's card toggle.
    let (hue_table, watch_card) = match &loaded {
        Ok(LoadOutcome::Loaded(config)) => (enabled_hue_table(config), mobile_watch_card(config)),
        _ => (None, false),
    };
    let (selection, warning) = select_plugins(&roster(), loaded);
    if let Some(warning) = warning {
        eprintln!("{warning}");
    }

    let overrides = overrides_from_env();
    let probes = SystemProbes::new(
        SystemCommandRunner,
        resolve_path(
            std::env::var("PNS_PHONE_MARKER_FILE").ok().as_deref(),
            &format!("{home}/.local/state/pns/phone-attention.marker"),
        )
        .to_string_lossy()
        .into_owned(),
    );
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|since_epoch| since_epoch.as_secs());

    let decision = decide(
        &probes,
        &selection,
        &overrides,
        event.local_only,
        event.remote_only,
        &event.pane,
        now_secs,
        event.long_running,
        watch_card,
    );

    // The pulse is part of the PLAN now, not a second invocation: the shell
    // used to call `pns pulse` alongside the notification, which meant the
    // tier was decided twice and could disagree with itself.
    if decision.pulse {
        // The state IS the exit code here: the shell notifier derives
        // --state from `$?`, and an agent turn that did not fail succeeded.
        fire_pulse(hue_table, if event.state == "failed" { "1" } else { "0" });
    }

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
    let rendered = rendered_event(&event, pane);

    let channels_dir_override = std::env::var("PNS_CHANNELS_DIR")
        .ok()
        .filter(|dir| !dir.is_empty());
    let channels_dir = resolve_path(
        channels_dir_override.as_deref(),
        &format!("{home}/.local/libexec/pns/channels"),
    );
    let auth = AuthFile {
        path: resolve_path(
            std::env::var("RELAY_AUTH_FILE").ok().as_deref(),
            &format!("{home}/.config/relay/auth.json"),
        ),
        contents: OnceCell::new(),
    };
    let banner = banner_channel();

    for leg in &decision.legs {
        let delivered = deliver_leg(
            leg,
            &rendered,
            &banner,
            &auth,
            native_first(channels_dir_override.is_some()),
            &channels_dir,
        );
        // THE ONE PLACE a delivery reaches the operator. A channel says what
        // happened; whether anyone hears it is the leg's reporting mode, and
        // that rule lives here rather than in three channels.
        if let Some(line) = delivered.line_for(leg.mode) {
            println!("{line}");
        }
    }
}

/// Every override the engine reads, out of the process environment.
fn overrides_from_env() -> Overrides {
    Overrides::from_env(
        &std::env::vars_os()
            .map(|(name, value)| {
                (
                    name.to_string_lossy().into_owned(),
                    value.to_string_lossy().into_owned(),
                )
            })
            .collect::<BTreeMap<_, _>>(),
    )
}

/// The parsed arguments plus the sanitized pane, rendered into the one event
/// every channel is handed.
fn rendered_event(event: &pns::args::EventArgs, pane: &str) -> pns::channels::Event {
    let message = render::message(&event.branch, &event.detail, &event.state);
    pns::channels::Event {
        agent: event.agent.clone(),
        state: event.state.clone(),
        project: event.project.clone(),
        branch: event.branch.clone(),
        detail: event.detail.clone(),
        title: render::title(&event.agent, &event.state, &event.project),
        preview: render::preview(&message),
        message,
        pane: pane.to_string(),
    }
}

/// Hue's settings, only when the operator enabled it explicitly.
fn enabled_hue_table(config: &pns::config::Config) -> Option<toml::Table> {
    config
        .plugins
        .get("hue")
        .filter(|hue| hue.enabled)
        .map(|hue| hue.settings.clone())
}

/// Whether a card fires while the operator is watching the pane on mobile.
///
/// DEFAULT OFF (operator ruling 2026-08-12): a card about the pane already on
/// screen is noise, and the pulse alone marks the long command finishing.
fn mobile_watch_card(config: &pns::config::Config) -> bool {
    config
        .plugins
        .get("moshi")
        .and_then(|moshi| moshi.settings.get("mobile_watch_card")?.as_bool())
        .unwrap_or(false)
}

/// The lights signal, from whichever mode asked for it.
fn fire_pulse(hue_table: Option<toml::Table>, exit_code: &str) {
    let Some(hue) = hue_table.and_then(|settings| {
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
    .run(exit_code);
}

/// The auth file, read AT MOST ONCE and only if a leg asks for it.
///
/// Eager reading made every notification wait on a file no plan might need:
/// point RELAY_AUTH_FILE at a FIFO nobody writes and an executable-only
/// delivery blocks forever. One read is still the rule, so two native legs
/// cannot sign against two different versions of the file.
struct AuthFile {
    path: std::path::PathBuf,
    contents: OnceCell<Option<String>>,
}

impl AuthFile {
    fn contents(&self) -> Option<&str> {
        self.contents
            .get_or_init(|| read_auth(&self.path))
            .as_deref()
    }
}

/// The banner, which now only needs to know where to send the click.
fn banner_channel() -> BannerChannel<SystemCommandRunner> {
    BannerChannel {
        runner: SystemCommandRunner,
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
        herdr_path: executable_in_path("herdr"),
    }
}

/// The moshi push, with the token this event already read.
fn moshi_channel(auth: Option<&str>) -> MoshiChannel<UreqPost> {
    MoshiChannel {
        http: UreqPost::default(),
        token: auth.and_then(moshi_secret),
        url: url_from_env("RELAY_MOSHI_URL", DEFAULT_MOSHI_URL),
    }
}

/// The hermes post, with the key this event already read. The path comes with
/// it only so the not-set-up line can name the file that had no key in it.
fn hermes_channel(
    auth: Option<&str>,
    auth_path: std::path::PathBuf,
) -> HermesChannel<UreqSignedPost> {
    HermesChannel {
        post: UreqSignedPost,
        key: auth.and_then(hermes_secret),
        auth_path,
        url: url_from_env("RELAY_HERMES_URL", DEFAULT_HERMES_URL),
        sync_deadline: remote_deadline(std::env::var("RELAY_REMOTE_TIMEOUT").ok().as_deref()),
    }
}

/// An endpoint override, where EMPTY means the default like every other path
/// and URL this binary reads.
fn url_from_env(variable: &str, default: &str) -> String {
    std::env::var(variable)
        .ok()
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| default.to_string())
}

/// One leg to its destination: the native plugin when it wins, else the
/// executable channel of that name.
fn deliver_leg(
    leg: &pns::routing::Leg,
    rendered: &pns::channels::Event,
    banner: &BannerChannel<SystemCommandRunner>,
    auth: &AuthFile,
    native_wins: bool,
    channels_dir: &Path,
) -> Delivery {
    if native_wins {
        // The two network channels are built HERE rather than up front,
        // because building one is what reads the auth file.
        match leg.name {
            "macos-banner" => return banner.deliver(rendered, leg.mode),
            "moshi" => return moshi_channel(auth.contents()).deliver(rendered, leg.mode),
            "hermes" => {
                return hermes_channel(auth.contents(), auth.path.clone())
                    .deliver(rendered, leg.mode);
            }
            _ => {}
        }
    }
    deliver(
        &channels_dir.join(format!("{}.sh", leg.name)),
        &rendered.to_json(leg.mode),
    )
}

/// A path from the environment, defaulting like bash's `${VAR:-default}`:
/// EMPTY means the default as much as unset does, because joining a filename
/// to an empty path resolves into the current directory and quietly delivers
/// nothing.
fn resolve_path(candidate: Option<&str>, default: &str) -> std::path::PathBuf {
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
    // FAIL CLOSED, unlike an event. The roster fallback that keeps every
    // notification working through a broken config is an EVENT-mode rule:
    // applying it here would let an unrelated typo switch a deliberately
    // disabled pulse back on. The pulse runs only when its own table says
    // enabled, explicitly.
    let config = match load_config(&config_path(&home)) {
        Ok(LoadOutcome::Loaded(config)) => config,
        // Absent is not a mistake; never opting in earns no warning.
        Ok(LoadOutcome::Missing) => return,
        Err(error) => {
            // The sanitized detail event mode prints, with the outcome THIS
            // mode had: there is no recoverable setting to fall back to, so
            // nothing pulses.
            eprintln!("pns: config error ({}); no pulse", error.detail());
            return;
        }
    };
    fire_pulse(
        enabled_hue_table(&config),
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
            .timeout_global(Some(BRIDGE_DEADLINE))
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

/// How long one bridge call may take. The pulse is decoration on a
/// notification, so it must never be what makes one slow.
const BRIDGE_DEADLINE: Duration = Duration::from_secs(10);

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
