use crate::*;

/// Every leg to its destination, in the registry's delivery order, each
/// paired with what its channel had to say for itself.
///
/// IT RETURNS ITS OUTCOMES RATHER THAN PRINTING THEM. An event prints only what
/// a reporting leg said; a hand-run check labels every outcome with its
/// plugin's name and prints the lot. Two callers spelling one report two ways
/// is exactly what a returned value is for.
///
/// THE LEGS AND THE SCRUB ARRIVE AS VALUES, not as a `Decision`: a caller that
/// took no decision has none to hand over.
pub(crate) fn dispatch_legs(
    legs: &[pns::routing::Leg],
    pane_dropped: bool,
    event: &pns::args::EventArgs,
    home: &str,
    mobile: &Mobile,
    hermes_key: Option<String>,
) -> Vec<(pns::routing::Leg, Delivery)> {
    // Sanitized ONCE here rather than per channel: a channel may be written in
    // any language and cannot be expected to share the guard. Warned about
    // only now, because a scrub nobody was going to receive is not news.
    let pane = if pane_dropped {
        eprintln!("pns: dropped a pane id with shell metacharacters; no channel will focus a pane");
        ""
    } else {
        event.pane.as_str()
    };
    let rendered = rendered_event(event, pane);

    let channels_dir_override = std::env::var("PNS_CHANNELS_DIR")
        .ok()
        .filter(|dir| !dir.is_empty());
    let channels_dir = resolve_path(
        channels_dir_override.as_deref(),
        &format!("{home}/.local/libexec/pns/channels"),
    );
    let banner = banner_channel();
    let moshi = moshi_channel(mobile.token.clone());
    let hermes = hermes_channel(hermes_key, hermes_url_for(&event.channel));

    // NO `?` AND NO EARLY RETURN: one channel's failure costs the others
    // nothing, and every channel above was constructed before the first
    // delivery, so a leg cannot be lost to a sibling's refusal.
    legs.iter()
        .map(|leg| {
            // THE MOBILE LEG IS GATED ON THE BACKEND VERDICT, ahead of the
            // dispatch that picks a seam and so ahead of BOTH of them. The
            // gate used to sit on the TOKEN, which only feeds the native
            // channel: with an executable channel of the same name installed,
            // the card went out under a backend nobody named while stderr
            // said "no card is pushed". A sentence that is printed has to be
            // true wherever the leg is dispatched.
            //
            // IT SITS HERE RATHER THAN IN `deliver_leg` because this is the
            // one site that dispatches any leg at all, so the two are the
            // same fence; a refused leg also runs nothing, so there is no
            // panic to catch below and nothing to unwind.
            if leg.name == "mobile"
                && let Some(reason) = mobile.refusal.as_deref()
            {
                return (*leg, Delivery::Failed(refused_backend_line(reason)));
            }
            // A PANIC IS ONE LEG'S FAILURE, never the run's. Without this an
            // unwinding channel takes the remaining legs and, in a hand-run
            // check, the rest of the census with it, and a census that ended
            // early is read as a report that finished. The default hook still
            // prints its own trace to stderr, which is left alone: silencing
            // it process-wide would hide every other panic in the binary.
            let delivered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                deliver_leg(
                    leg,
                    &rendered,
                    &banner,
                    &moshi,
                    &hermes,
                    native_first(channels_dir_override.is_some()),
                    &channels_dir,
                )
            }))
            .unwrap_or_else(|_| {
                // NO PAYLOAD TEXT: a panic message is written for a developer
                // and may quote anything the channel was holding.
                Delivery::Failed(format!(
                    "the {} channel PANICKED; nothing was sent",
                    leg.name
                ))
            });
            (*leg, delivered)
        })
        .collect()
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
fn hermes_url_for(channel: &str) -> String {
    let env_override = std::env::var("PNS_HERMES_URL")
        .ok()
        .filter(|url| !url.is_empty());
    if let Some(url) = env_override {
        return url;
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
/// One leg to its destination: the native plugin when it wins, else the
/// executable channel of that name.
fn deliver_leg(
    leg: &pns::routing::Leg,
    rendered: &pns::channels::Event,
    banner: &BannerChannel<SystemCommandRunner>,
    moshi: &MoshiChannel<UreqPost>,
    hermes: &HermesChannel<UreqSignedPost>,
    native_wins: bool,
    channels_dir: &Path,
) -> Delivery {
    if native_wins {
        match leg.name {
            "macos-banner" => return banner.deliver(rendered, leg.mode),
            "mobile" => return moshi.deliver(rendered, leg.mode),
            "hermes" => return hermes.deliver(rendered, leg.mode),
            _ => {}
        }
    }
    deliver(
        &channels_dir.join(format!("{}.sh", leg.name)),
        &rendered.to_json(leg.mode),
    )
}
/// Hand one channel its event on stdin. A channel that is missing, is not
/// executable, or fails is not an error: it is simply not installed, or it
/// declined, and neither may take down the siblings or the caller.
///
/// SILENT ON THE NOTIFICATION PATH whichever verdict it answers with: the
/// common failure here is a channel nobody installed, and reporting that on
/// every event would be noise. THE TWO ARE STILL DIFFERENT VERDICTS. A channel
/// that ran and said nothing is `Silent`; one that never started is
/// `Unlaunched`, which prints nowhere an event can see and is what lets a
/// hand-run check tell a delivery from a spawn that never happened. The exit
/// status of a channel that DID run is still dropped, because a channel
/// declining is its own business.
fn deliver(channel: &Path, event: &str) -> Delivery {
    let mut child = match Command::new(channel).stdin(Stdio::piped()).spawn() {
        Ok(child) => child,
        Err(error) => {
            return Delivery::Unlaunched(format!(
                "could not launch the channel at {} ({error}); nothing was sent",
                channel.display()
            ));
        }
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
