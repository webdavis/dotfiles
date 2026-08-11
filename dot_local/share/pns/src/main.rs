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
use std::time::{SystemTime, UNIX_EPOCH};

use pns::args::parse_args;
use pns::config::{config_path, load_config};
use pns::engine::{Overrides, decide, event_json, resolve_path, select_plugins};
use pns::registry::{Registry, Routing};
use pns::render;
use pns::system::{SystemCommandRunner, SystemProbes};

fn main() {
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
    let channels_dir = resolve_path(
        std::env::var("PNS_CHANNELS_DIR").ok().as_deref(),
        &format!("{home}/.local/libexec/pns/channels"),
    );

    for leg in &decision.legs {
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
