use crate::{Mobile, executable_in_path};
use pns_adapters::{
    BannerChannel, DEFAULT_HERMES_URL, DEFAULT_MOSHI_URL, DiscordChannel, DiscordSettings,
    HermesChannel, HermesKeys, MoshiChannel, SystemCommandRunner, UreqDiscordPost, UreqPost,
    channel_url, refused_backend_line, refused_discord_line, remote_deadline, resolve_path,
};
use pns_application::{Destinations, NotificationDestination};
use pns_domain::{Event, EventArgs, registry::Selection, render, routes::Routes};
use pns_hermes::UreqSignedPost;
use std::time::Duration;

mod registration;

const EXECUTABLE_DEADLINE: Duration = Duration::from_secs(5);

pub(crate) fn destinations(
    selection: &Selection,
    route: &str,
    home: &str,
    mobile: &Mobile,
    hermes_keys: &HermesKeys,
    discord: &DiscordSettings,
    routes: &Routes,
) -> Destinations<Box<dyn NotificationDestination>> {
    destinations_with_output(
        selection,
        route,
        home,
        mobile,
        hermes_keys,
        discord,
        routes,
        false,
    )
}

// EIGHT ARGUMENTS, and the lint is wrong here: every one is a separate
// reading the composition root already took off one config, and the
// alternative is a struct that exists only to be destructured on the next
// line. The grouping that would earn its keep (one "what the config said"
// value) is a change to every caller of the event path, not to this signature.
#[allow(clippy::too_many_arguments)]
pub(crate) fn destinations_with_output(
    selection: &Selection,
    route: &str,
    home: &str,
    mobile: &Mobile,
    hermes_keys: &HermesKeys,
    discord: &DiscordSettings,
    routes: &Routes,
    json: bool,
) -> Destinations<Box<dyn NotificationDestination>> {
    destinations_for_override(
        std::env::var("PNS_CHANNELS_DIR").ok().as_deref(),
        selection,
        route,
        home,
        mobile,
        hermes_keys,
        discord,
        routes,
        json,
    )
}

// THE ENVIRONMENT READ IS THE ONLY THING ABOVE THIS LINE, so the decision the
// override drives (a blank value falls through to the default directory, a set
// one forces every channel onto its executable, and a refused backend precedes
// both) is reachable with the value handed in. It is what keeps the test of
// that decision a plain call instead of a re-exec of the test binary with a
// scrubbed environment, which could only be bounded by a wall-clock deadline
// and reddened `main` under load when a spawn outran it.
#[allow(clippy::too_many_arguments)]
fn destinations_for_override(
    channels_override: Option<&str>,
    selection: &Selection,
    route: &str,
    home: &str,
    mobile: &Mobile,
    hermes_keys: &HermesKeys,
    discord: &DiscordSettings,
    routes: &Routes,
    json: bool,
) -> Destinations<Box<dyn NotificationDestination>> {
    let override_dir = channels_override.filter(|dir| !dir.is_empty());
    let channels = resolve_path(override_dir, &format!("{home}/.local/libexec/pns/channels"));
    let forced = override_dir.map(|_| channels.as_path());
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
                hermes_keys,
                hermes_target(
                    route,
                    std::env::var("PNS_HERMES_URL").ok().as_deref(),
                    routes,
                ),
            ),
            forced,
            None,
            json,
        ),
        registration::choose(
            discord_channel(discord, route, routes),
            forced,
            discord.refusal().map(refused_discord_line),
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
        // THE CARD TITLE IS UNTOUCHED BY THE SENDER HEADER. An iOS
        // notification title shows about forty characters, and
        // `agent · state · project` is what the card exists to deliver, so
        // the sender's own fields travel UNCOMPOSED beside it: the one
        // destination that renders a header composes it there, and the
        // branch already reaches the card through `message`.
        title: render::title(&event.agent, &event.state, &event.project),
        session: event.session.clone(),
        session_title: event.session_title.clone(),
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
pub(crate) fn moshi_channel(token: Option<String>) -> MoshiChannel<UreqPost> {
    MoshiChannel {
        http: UreqPost::default(),
        token,
        url: url_from_env("PNS_MOSHI_URL", DEFAULT_MOSHI_URL),
    }
}
/// The hermes post, signed with the key the config named FOR THIS ROUTE.
///
/// THE LOOKUP IS HERE, at the one place that already knows both the route and
/// the config, so the route in the channel, the route in its URL and the route
/// the key was granted to are one value rather than three that agree by habit.
fn hermes_channel(
    keys: &HermesKeys,
    (route, url): (String, String),
) -> HermesChannel<UreqSignedPost> {
    HermesChannel {
        post: UreqSignedPost,
        key: keys.key_for(&route).map(str::to_string),
        route,
        url,
        sync_deadline: remote_deadline(std::env::var("PNS_REMOTE_TIMEOUT").ok().as_deref()),
    }
}
/// The bot post, with the token and the channel the config already provided.
///
/// NO ENV OVERRIDE FOR THE ENDPOINT, unlike moshi and hermes beside it: those
/// two point at a local gateway and a pairing service a test can stand up,
/// while this one names discord.com, and a variable that could repoint an
/// authenticated bot post is a credential-exfiltration lever for no gain. The
/// seam is the test seam.
fn discord_channel(
    settings: &DiscordSettings,
    route: &str,
    routes: &Routes,
) -> DiscordChannel<UreqDiscordPost> {
    DiscordChannel {
        post: UreqDiscordPost,
        token: settings.token().map(str::to_string),
        channels: settings.channels().clone(),
        // THE LEG'S OWN ROUTE, taken here for `hermes_channel`'s reason above:
        // the submission and the retry both build their destinations from it,
        // and it is what a severity override has already written.
        route: route.to_string(),
        // AND THE NAME THIS MACHINE GIVES ITS DEFAULT ROUTE, which is the map
        // key an event with no project lands on. Taken here for the same
        // reason: the config is read at the root, never in the destination.
        default_route: routes.default_route().to_string(),
        // THE STORE IS BUILT HERE rather than threaded through every caller
        // of `destinations`: it holds a path and opens its connection per
        // transaction, which is how `recap_delivery_runtime` already builds
        // one beside the destinations it hands them to.
        threads: Box::new(pns_adapters::SqliteStore::new(pns_adapters::state_dir())),
    }
}
/// The route one event posts to and the endpoint that route answers at.
///
/// BOTH AT ONCE, never separately: the route names the signing key and the URL
/// names the gateway, and a function that answered only the second let the key
/// be chosen from a name the URL had already fallen away from.
///
/// The env override wins for the URL (an explicit URL, the tests' escape
/// hatch); the route is the `--channel` name, and the DEFAULT ROUTE THE CONFIG
/// NAMED when nothing named one. The URL's final segment is swapped for that
/// route, so renaming the default route moves the path and not the gateway.
/// The gateway has no route named "alert"; the default is where an event
/// with no route named goes. An unusable name is said out loud and falls
/// back LOUD-WARD to the default route, key and all: a misrouted
/// notification on the default route beats a silently dropped one.
fn hermes_target(channel: &str, env_override: Option<&str>, routes: &Routes) -> (String, String) {
    let route = if channel.is_empty() {
        routes.default_route()
    } else if pns_domain::safety::route_name_is_usable(channel) {
        channel
    } else {
        eprintln!(
            "pns: --channel {channel:?} is not a usable route name; posting to the default route"
        );
        routes.default_route()
    };
    let url = match env_override.filter(|url| !url.is_empty()) {
        Some(url) => url.to_string(),
        // The route is already usable and the default URL already carries a
        // path, so this fallback is unreachable; it stands rather than an
        // `expect`, because a notification path must not panic.
        None => {
            channel_url(DEFAULT_HERMES_URL, route).unwrap_or_else(|| DEFAULT_HERMES_URL.to_string())
        }
    };
    (route.to_string(), url)
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
