use crate::{Mobile, executable_in_path};
use pns_adapters::{
    BannerChannel, DEFAULT_HERMES_URL, DEFAULT_MOSHI_URL, HermesChannel, MoshiChannel,
    SystemCommandRunner, UreqPost, channel_url, refused_backend_line, remote_deadline,
    resolve_path,
};
use pns_application::{Destinations, NotificationDestination};
use pns_domain::{Event, EventArgs, registry::Selection, render};
use pns_hermes::UreqSignedPost;
use std::time::Duration;

mod registration;

const EXECUTABLE_DEADLINE: Duration = Duration::from_secs(5);

pub(crate) fn destinations(
    selection: &Selection,
    route: &str,
    home: &str,
    mobile: &Mobile,
    hermes_key: Option<String>,
) -> Destinations<Box<dyn NotificationDestination>> {
    destinations_with_output(selection, route, home, mobile, hermes_key, false)
}

pub(crate) fn destinations_with_output(
    selection: &Selection,
    route: &str,
    home: &str,
    mobile: &Mobile,
    hermes_key: Option<String>,
    json: bool,
) -> Destinations<Box<dyn NotificationDestination>> {
    let override_dir = std::env::var("PNS_CHANNELS_DIR")
        .ok()
        .filter(|dir| !dir.is_empty());
    let channels = resolve_path(
        override_dir.as_deref(),
        &format!("{home}/.local/libexec/pns/channels"),
    );
    let forced = override_dir.as_ref().map(|_| channels.as_path());
    let native = vec![
        registration::choose(
            moshi_channel(mobile.token.clone()),
            forced,
            mobile.refusal.as_deref().map(refused_backend_line),
            json,
        ),
        registration::choose(banner_channel(), forced, None, json),
        registration::choose(
            hermes_channel(
                hermes_key,
                hermes_url_for(route, std::env::var("PNS_HERMES_URL").ok().as_deref()),
            ),
            forced,
            None,
            json,
        ),
    ];
    registration::assemble(selection, native, &channels, json)
}

/// The parsed arguments plus the sanitized pane, rendered into the one event
/// every channel is handed.
pub(crate) fn rendered_event(event: &EventArgs, pane_dropped: bool) -> Event {
    if pane_dropped {
        eprintln!("pns: dropped a pane id with shell metacharacters; no channel will focus a pane");
    }
    rendered_event_quiet(event, pane_dropped)
}

pub(crate) fn rendered_event_quiet(event: &EventArgs, pane_dropped: bool) -> Event {
    let pane = if pane_dropped {
        ""
    } else {
        event.pane.as_str()
    };
    let message = render::message(&event.branch, &event.detail, &event.state);
    Event {
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
/// The moshi push, with the token the config already provided.
fn moshi_channel(token: Option<String>) -> MoshiChannel<UreqPost> {
    MoshiChannel {
        http: UreqPost::default(),
        token,
        url: url_from_env("PNS_MOSHI_URL", DEFAULT_MOSHI_URL),
    }
}
/// The hermes post, with the key the config already provided.
fn hermes_channel(key: Option<String>, url: String) -> HermesChannel<UreqSignedPost> {
    HermesChannel {
        post: UreqSignedPost,
        key,
        url,
        sync_deadline: remote_deadline(std::env::var("PNS_REMOTE_TIMEOUT").ok().as_deref()),
    }
}
/// The hermes endpoint one event posts to. The env override wins (an explicit
/// URL, the tests' escape hatch), then a `--channel` route name derived from
/// the default gateway, then the default route (`/webhooks/pns`) itself. The
/// gateway has no route named "alert"; the default is where an event with no
/// route named goes. An unusable name is said out loud and falls back
/// LOUD-WARD: a misrouted notification on the default route beats a silently
/// dropped one.
fn hermes_url_for(channel: &str, env_override: Option<&str>) -> String {
    let env_override = env_override.filter(|url| !url.is_empty());
    if let Some(url) = env_override {
        return url.to_string();
    }
    if channel.is_empty() {
        return DEFAULT_HERMES_URL.to_string();
    }
    channel_url(DEFAULT_HERMES_URL, channel).unwrap_or_else(|| {
        eprintln!(
            "pns: --channel {channel:?} is not a usable route name; posting to the default route"
        );
        DEFAULT_HERMES_URL.to_string()
    })
}
/// An endpoint override, where EMPTY means the default like every other path
/// and URL this binary reads.
fn url_from_env(variable: &str, default: &str) -> String {
    std::env::var(variable)
        .ok()
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| default.to_string())
}

#[cfg(test)]
mod tests;
