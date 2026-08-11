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
use pns::channels::hermes::{DEFAULT_HERMES_URL, HermesChannel, UreqSignedPost, remote_deadline};
use pns::channels::hue::{Bridge, HuePulse, Sleeper, hue_settings};
use pns::channels::moshi::{DEFAULT_MOSHI_URL, MoshiChannel, UreqPost};
use pns::channels::{Channel, native_first};
use pns::config::{LoadOutcome, config_path, load_config};
use pns::engine::{Overrides, decide, event_json, resolve_path, select_plugins};
use pns::registry::{Registry, Routing};
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

    // THE ROSTER, and the only statement of delivery order. A destination is
    // added here, never in policy.
    let mut registry = Registry::new();
    for (name, routing) in [
        (
            "moshi",
            Routing {
                local: false,
                presence_gated: true,
                durable: false,
            },
        ),
        (
            "hermes",
            Routing {
                local: false,
                presence_gated: false,
                durable: true,
            },
        ),
        (
            "macos-banner",
            Routing {
                local: true,
                presence_gated: false,
                durable: false,
            },
        ),
    ] {
        if let Err(error) = registry.register(name, routing) {
            eprintln!("pns: {error:?}");
        }
    }

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

    let title = render::title(&event.agent, &event.state, &event.project);
    let message = render::message(&event.branch, &event.detail, &event.state);
    let preview = render::preview(&message);
    let channels_dir_override = std::env::var("PNS_CHANNELS_DIR")
        .ok()
        .filter(|dir| !dir.is_empty());
    let channels_dir = resolve_path(
        channels_dir_override.as_deref(),
        &format!("{home}/.local/libexec/pns/channels"),
    );

    let banner = BannerChannel {
        runner: SystemCommandRunner,
        probes: SystemProbes::new(SystemCommandRunner, String::new()),
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
        title: title.clone(),
        message: message.clone(),
        preview: preview.clone(),
        pane: pane.to_string(),
    };

    let auth_path = resolve_path(
        std::env::var("RELAY_AUTH_FILE").ok().as_deref(),
        &format!("{home}/.config/relay/auth.json"),
    );
    let moshi = MoshiChannel {
        http: UreqPost::default(),
        auth_path: auth_path.clone(),
        url: std::env::var("RELAY_MOSHI_URL")
            .ok()
            .filter(|url| !url.is_empty())
            .unwrap_or_else(|| DEFAULT_MOSHI_URL.to_string()),
    };

    let hermes = HermesChannel {
        post: UreqSignedPost,
        auth_path,
        url: std::env::var("RELAY_HERMES_URL")
            .ok()
            .filter(|url| !url.is_empty())
            .unwrap_or_else(|| DEFAULT_HERMES_URL.to_string()),
        sync_deadline: remote_deadline(std::env::var("RELAY_REMOTE_TIMEOUT").ok().as_deref()),
    };

    for leg in &decision.legs {
        if native_first(channels_dir_override.is_some()) {
            match leg.name {
                "macos-banner" => {
                    banner.deliver(&native, leg.mode);
                    continue;
                }
                "moshi" => {
                    moshi.deliver(&native, leg.mode);
                    continue;
                }
                "hermes" => {
                    hermes.deliver(&native, leg.mode);
                    continue;
                }
                _ => {}
            }
        }
        deliver(
            &channels_dir.join(format!("{}.sh", leg.name)),
            &event_json(
                &event.agent,
                &event.state,
                &event.project,
                &event.branch,
                &event.detail,
                &title,
                &message,
                &preview,
                pane,
                leg.mode.as_str(),
            ),
        );
    }
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
fn deliver(channel: &Path, event: &str) {
    let Ok(mut child) = Command::new(channel).stdin(Stdio::piped()).spawn() else {
        return;
    };
    if let Some(mut stdin) = child.stdin.take() {
        // Newline-terminated, as the bash's `jq -cn` emitted it: a channel
        // reading one line with `read -r` gets nothing without it.
        let _ = stdin.write_all(event.as_bytes());
        let _ = stdin.write_all(b"\n");
    }
    let _ = child.wait();
}

/// The `pulse` mode: read the hue table, take the single-pulse lock, and run
/// the sequence against the bridge. Every absence is a silent exit 0.
fn pulse_mode() {
    let home = std::env::var("HOME").unwrap_or_default();
    let Ok(LoadOutcome::Loaded(config)) = load_config(&config_path(&home)) else {
        return;
    };
    let Some(hue) = config
        .plugins
        .get("hue")
        .filter(|table| table.enabled)
        .and_then(|table| {
            hue_settings(
                &table.settings,
                std::env::var("HUE_PULSE_ROOMS").ok().as_deref(),
            )
        })
    else {
        return;
    };

    // Two pulses at once would interleave their writes and restore each
    // other's transient state. The kernel drops this lock on any exit, so it
    // cannot go stale; a concurrent pulse is skipped, never queued.
    let lock_path = resolve_path(None, &format!("{home}/.local/state/pns/hue-pulse.lock"));
    if let Some(parent) = lock_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(lock) = std::fs::File::create(&lock_path) else {
        return;
    };
    if lock.try_lock().is_err() {
        return;
    }

    HuePulse {
        bridge: UreqBridge {
            base: format!("https://{}/clip/v2/resource", hue.bridge),
            key: hue.key,
        },
        sleeper: RealSleeper,
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

    fn put(&self, path: &str, body: &str) -> bool {
        self.agent()
            .put(format!("{}/{path}", self.base))
            .header("hue-application-key", &self.key)
            .content_type("application/json")
            .send(body)
            .is_ok()
    }
}

struct RealSleeper;

impl Sleeper for RealSleeper {
    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}
