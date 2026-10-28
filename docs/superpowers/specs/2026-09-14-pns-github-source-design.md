# The GitHub source: one event, two transports, a lamp and a channel per repo

Status: design, written 2026-09-14 (evening) with the operator asleep. Nothing built, nothing
changed, no code written by this lane. Every choice made in the operator's place is listed under
"Assumptions" with the alternative it displaced, and the questions only the operator can answer are
the last section before the build plan.

## What the operator decided

Recorded 2026-09-14 (evening), in the same sitting as the direct Discord destination ruling and the
uu triage channel:

> one Discord channel per REPO, `#github-<repo>`, plus a `#github` catch-all, mapped in pns config
> and delivered by pns's direct Discord bot (not hermes routes); until the bot ships, one hermes
> `github` route feeds the catch-all. GitHub colours configurable in pns config (purple pass /
> orange fail defaults), a `github` lamp behaviour selectable per lamp via `shows`.

and, in the brief that produced this document: pns gains a GitHub SOURCE so the operator knows when
Actions runs and other GitHub things finish; ONE event shape for every GitHub thing, whether it
arrived by poll or by push; TWO transports, poll first because push depends on it for catch-up; a
token in pns config by KeePassXC entry name, never a dependency on the operator's `gh` login; no
acting on events and no GitHub write of any kind.

## Where this sits, and what changed since the May research

`docs/research/2026-05-18-github-workflow-notification-trigger.md` answered the same question four
months ago and its conclusions have aged in four separate directions. This section is the honest
accounting, because three of the four reverse a recommendation that document made.

1. **The 60 second poller it characterized is gone.** `dot_local/bin/executable_gha-watcher.sh`,
   `executable_gha-notify.sh` and `com.webdavis.gha-watcher.plist.tmpl` are absent from this
   checkout; `dot_local/bin/` holds two files, neither of them a watcher, and the only mentions of
   `gha-watcher` anywhere in the tree are in six documents. There is nothing to upgrade. This
   design builds a poller rather than modernizing one.
1. **The queue problem it solved no longer exists.** Its headline recommendation was a launchd
   `QueueDirectories` chokepoint to serialize Hue pulses, because `hue-pulse.sh` had no locking and
   two pulses inside 50 ms corrupted each other's restore. That whole apparatus is obsolete: pns is
   the single writer to the lamps now, the bridge restores the lamp itself (measured on a real lamp
   2026-09-01, `pns-domain/src/pulse.rs`), and the daemon owns the animation upkeep. A GitHub event
   entering through `pns submit` is already serialized by construction. **The queue is the one part
   of the May design that is finished, by something else, without anyone building it.**
1. **Tailscale Funnel is no longer the low-friction inbound path. It is a critical page.**
   `posture funnel` reads the tailnet's funnel configuration on every tick and pages CRIT on any
   exposure that its baseline does not already carry (`posture-domain/src/funnel.rs`,
   `FunnelAlert::Exposure`). Turning on Funnel to receive webhooks means either living with a
   critical page or teaching the baseline to expect it, and the second is exactly the shape of
   change the posture work has spent a month refusing. The May document's "recommended" transport
   is now the one the security posture is built to complain about.
1. **A Cloudflare tunnel exists now, and it is already carrying one hostname.** `~/.cloudflared/`
   holds a named tunnel, `agentmail-receiver`, whose ingress routes `agentmail.webdavis.io` to
   `http://localhost:8787` with a `http_status:404` catch-all beneath it. The May document put
   Cloudflare Tunnel at "15 to 30 minutes of setup" against Funnel's 60 seconds. That cost is
   already paid. A second ingress hostname on an existing tunnel is a two line change to a file
   that is already deployed and already running.

The fifth May finding, the purple and orange colour pair, survives as the operator's own choice and
is carried forward here. It does not survive contact with the lamp vocabulary pns built afterwards,
which is measured in Decision 4 and is the single most consequential finding in this document.

## Constraints that bind this design

1. **The operator runs applies. Agents do not.** Every config file, LaunchAgent and template below
   reaches the machine only through an operator-run full apply.
1. **The shipped pns config template is a GENERATED file.** `dot_config/pns/private_config.toml.tmpl`
   is `just pns-config-render`'s output over `dot_config/pns/config-values.toml`, and a hand edit
   fails a byte-equality test in `just test-rust`. Every config change below is a values-file edit
   plus a layout edit plus a regeneration.
1. **Defaults visible in config** (2026-08-31), **opt-in tables ship commented** (2026-08-31), **no
   dead knobs**: only the knobs that apply to a behaviour exist.
1. **Secrets never enter the repo.** A secret-bearing key in the values file holds
   `{ keepassxc = "<entry>", field = "Password" }` and the generator refuses anything else.
1. **No removal mechanisms** (2026-08-02). Nothing below has a retirement script; a deployed
   leftover is listed for the operator to trash by hand.
1. **We test the behavior of tools we wrote, and nothing else** (2026-08-05). A test asserting that
   a Cloudflare ingress stanza exists, or that a LaunchAgent is wired, would be deleted on sight.
1. **No workspace may depend on another** (2026-09-10). This design lives entirely inside pns's
   workspace, so that rule is not engaged, but the corollary is: posture gains its own control for
   the tunnel, in posture's own workspace, and posture never learns that pns has a GitHub source.
1. **Tools are pns-agnostic** (2026-09-14). This one cuts the other way and is worth stating: the
   GitHub source is pns's OWN feature, not a third-party tool handing pns an event. pns is allowed
   to know about GitHub the way it knows about moshi and hermes.
1. **Optimal over cheap** (2026-09-05) and **no manual-intervention designs**: a design whose
   failure mode is "the operator notices the lamp stopped" is not finished.
1. **Rust files target 300 lines and never exceed 500**, unit tests included (2026-09-02), and the
   clean-code Rust bindings govern the crate split.

## What is actually built today

Everything in this section was read or run in this checkout on 2026-09-14. Nothing is recalled.

### The daemon already schedules a self-renewing 60 second job, and the pattern has a name

`pns-application/src/daemon.rs` runs a loop that, once every `SWITCH_TICKS`, re-reads the config
switch and then calls `ensure_presence_poll(&jobs, self.settings.presence_interval(), now)`.
`pns-application/src/presence_registration.rs` is thirty lines and does exactly what a GitHub poll
needs: it registers a leased `Job` under a fixed spool name with `every: Some(interval)`, keeps the
PENDING due rather than pushing it out on every sweep, refreshes only the lease, cancels the job
when the interval is `None`, and drops every failure because the next sweep tries again. Its
docblock states why the sensor is the daemon's own job: "no event asks for a room reading, so
nothing else would ever register it, and a job registered once at startup would die with its lease
on the first daemon that outran it." That sentence is true of a GitHub poll word for word.

A `Job` (`pns-domain/src/jobs.rs`) carries `id`, `due`, `until`, `every`, `unless_marker` and
`args`, where `args` is "the argv THIS binary is re-executed with. Never another program." So the
poll is `pns github poll --daemon`, run by the daemon as a child of itself, bounded by a lease.

### The lamp vocabulary is a closed set of five words and five compile-time colours

`pns-domain/src/lamps/config/behaviour.rs` defines `Behaviour` as `Done`, `Failed`, `Blocked`,
`Unread`, `Looping`, with `BEHAVIOUR_WORDS` giving the five config spellings. Its docblock says why
the set is closed: "a `shows` list holding a word nothing matches is a lamp that stays dark while
the operator is sure they routed it, with no message anywhere." It also records the precedent this
design needs: "`Unread` IS ONE WORD AND CARRIES TWO COLOURS."

The colours are constants in `pns-domain/src/pulse.rs`, and **none of them is configurable**:

| Constant | xy | What it says |
| --- | --- | --- |
| `SUCCESS_COLOR` | 0.17, 0.70 | deep green, done |
| `FAILURE_COLOR` | 0.675, 0.322 | red, failed, and the unread failure flavour |
| `BLOCKED_COLOR` | 0.3395, 0.1379 | magenta, an agent waiting on the operator |
| `UNREAD_SUCCESS_COLOR` | 0.50, 0.40 | daylight, a run that finished while away |
| `LOOP_COLOR` | 0.1532, 0.0475 | deepest blue, long work in flight |

Every one carries the same claim: operator-locked on a real lamp under the observe, adjust, lock
protocol, on a dated trial. `PulseColor` is `{ x: f64, y: f64 }` and deliberately not RGB, because
"the bridge clamps RGB into its gamut and desaturates hard; xy bypasses that conversion."

`BLOCKED_COLOR`'s docblock carries the separation metric this design is judged by. Blocked and loop
once sat **0.067 apart** in xy and "in daylight the two lamps read as one", which is why the blue
moved. The pair that replaced it is 0.207 apart, and the docblock names the closest remaining pair
in the whole vocabulary: failure and unread success, at **0.192**.

`[lights.done]` and `[lights.failed]` each carry exactly `duration_ms` and `brightness`. There is no
colour key anywhere in the lamp tables.

### The config renderer walks a static layout and refuses a float

`pns-adapters/src/config/render/layout.rs` holds `LAYOUT`, a table of every table and key the schema
serves, in file order, each key carrying its own comment and either a `Default` (written live) or an
`Example` (written commented). `render` consumes the values table as it walks and refuses by name
anything left over.

`pns-adapters/src/config/render/values.rs:render_value` renders a bool, an integer, a string, an
array of strings, and the keepassxc secret table. Its final arm is
`other => Err(format!("type `{}` does not render", other.type_str()))`. **A float does not render
today, and neither does an array of floats.** Any colour knob spelled as a coordinate pair needs one
new arm in that function, and that is a hard error rather than a silent omission, which is the right
failure.

### Per-route hermes keys land in the branch this design waits on

`feat/pns-per-route-keys` replaces the single `[plugins.hermes] key` with a `[plugins.hermes.keys]`
table, one key per route, already carrying `pns`, `pns-recap`, `posture` and `priority` entries
shaped `pns = { keepassxc = "Hermes :: Webhook Secret (#pns)", field = "Password" }`. Its own
comment states the property: "a key leaked from any route cannot post to every route. A route left
out here posts nothing and says which key is missing, rather than borrowing another route's secret."
A `github` route is one line in that table. **This is why PR 1 of the build plan waits: both
branches edit `config-values.toml`, the generated template and the layout.**

### The hermes destination posts to a route by name

`pns-adapters/src/destinations/hermes.rs` posts one signed JSON body to
`http://127.0.0.1:8644/webhooks/pns` by default, with `channel_url` swapping the final path segment
for a named route. The body carries `agent`, `state`, `project`, `detail`, `header`, `subheader`,
`body`, `thread_id` and `request_id`, and every key is always present because the gateway renders
`{key}` with no conditionals. `pns-hermes/src/post.rs:sign` is HMAC-SHA256 (hash-based message
authentication code over SHA-256) producing a lowercase hex digest, from the `hmac` and `sha2`
crates already in the lock file.

### The producer API, and what an HTTP listener in this workspace already looks like

`pns submit --json` reads one `pns.request` version 1 envelope on stdin and answers a
`pns.result` envelope (`pns/src/submit.rs`, `pns-protocol/src/request.rs`). The request's fields are
`request_id`, `producer`, `session`, `event`, `signal`, `occurred_at`, `elapsed_secs`, `detail`,
`context`, `scope`, `route`, `class`, `interaction` and `extensions`, where `extensions` is
"producer-specific data, carried verbatim and never read here". `Signal` is a closed set:
`Succeeded`, `Failed`, `NeedsAttention`, `ApprovalRequested`, `Resolved`, `Observation`, `Progress`.

`pns/src/bin/http-capture.rs` is a std-only `TcpListener` HTTP server of about 150 lines, built as a
dev binary behind `required-features = ["dev-tools"]`. It is proof that an HTTP listener in this
workspace needs no web framework and no new dependency.

### The tunnel, and the posture control that does not exist yet

The `agentmail-receiver` tunnel routes one hostname to one localhost port with a 404 catch-all.
posture watches Tailscale Funnel and pages on exposure; it watches **nothing** about cloudflared.
`.chezmoidata/macos_posture_controls.yaml` records verify-tier controls, each naming a `reader` from
a closed list of poller reader functions (`fdesetup_status`, `csrutil_status`, `pgrep_oversight`,
`lulu_rule_present` and five more), and a record with an unknown reader aborts the render. **So a
cloudflared control is not a data-only change.** It is either a new reader function in the poller or
a new subcommand shaped like `posture funnel`, with its own baseline. Decision 2b says which.

## Facts verified against GitHub's documentation today

Every row was fetched on 2026-09-14 and is quoted or paraphrased from the cited page. Re-verify any
row this design is built on before building it; three of them decide the shape.

| Fact | Source |
| --- | --- |
| "These endpoints only support authentication using a personal access token (classic)." A fine-grained personal access token **cannot** call the notifications endpoints. | REST notifications, `docs.github.com/en/rest/activity/notifications` |
| "All calls to these endpoints require the `notifications` or `repo` scopes." | same |
| "There is an `X-Poll-Interval` header that specifies how often (in seconds) you are allowed to poll. In times of high server load, the time may increase. Please obey the header." Today it is 60. | same |
| "Notifications are optimized for polling with the `Last-Modified` header. If there are no new notifications, you will see a `304 Not Modified` response, leaving your current rate limit untouched." | same |
| `ci_activity`: "A GitHub Actions workflow run that you triggered was completed." | notification reasons, same page |
| `review_requested`: "You, or a team you're a member of, were requested to review a pull request." `mention`: "You were specifically @mentioned in the content." `security_alert`: "GitHub discovered a security vulnerability in your repository." | same |
| GitHub Actions notifications offer "On GitHub" (web) and "Email" delivery, plus an "Only notify for failed workflows" filter, for "repositories that are set up with GitHub Actions and that you are watching". | `docs.github.com/en/subscriptions-and-notifications/how-tos/managing-github-actions-notifications` |
| "GitHub does not automatically redeliver failed deliveries." Manual redelivery exists in the web interface and through the REST API. | `docs.github.com/en/webhooks/using-webhooks/handling-failed-webhook-deliveries` |
| A GitHub App lists, reads and redelivers its own deliveries with `GET /app/hook/deliveries`, `GET /app/hook/deliveries/{delivery_id}` and `POST /app/hook/deliveries/{delivery_id}/attempts`, authenticating with a JWT (JSON Web Token) signed by the app's private key, not an installation token. | `docs.github.com/en/rest/apps/webhooks` |
| `workflow_run` needs the App's `Actions` repository permission at read. `check_run` and `check_suite` need `Checks` at read. `pull_request` needs `Pull requests`. `release` needs `Contents`. `dependabot_alert` needs `Dependabot alerts`. `workflow_run` actions are `completed`, `in_progress`, `requested`, and the payload carries `workflow_run.conclusion`, `repository.full_name` and `workflow_run.html_url`. | `docs.github.com/en/webhooks/webhook-events-and-payloads` |
| Discord's GitHub-compatible webhook endpoint supports `commit_comment`, `create`, `delete`, `fork`, `issue_comment`, `issues`, `member`, `public`, `pull_request`, `pull_request_review`, `pull_request_review_comment`, `push`, `release`, `watch`, `check_run`, `check_suite`, `discussion`, `discussion_comment`. **`workflow_run` is not on that list.** | `docs.discord.com/developers/resources/webhook` |
| GitHub's webhook source ranges, read from `GET https://api.github.com/meta` today: `192.30.252.0/22`, `185.199.108.0/22`, `140.82.112.0/20`, `143.55.64.0/20`, `2a0a:a440::/29`, `2606:50c0::/32`. | live API call |
| Cloudflare Access actions are Allow, Block, Bypass and Service Auth. Bypass "disables any Access enforcement for traffic that meets the defined rule criteria", supports IP range selectors, cannot use identity selectors, and warns that "Bypass does not enforce any Access security controls and requests are not logged". | `developers.cloudflare.com/cloudflare-one/access-controls/policies/` |

Two of those rows are load-bearing and deserve calling out.

**The notifications API refuses a fine-grained token.** The design's token is therefore a **classic
personal access token with the `notifications` scope**, and there is no way around it short of
abandoning the poll transport. A classic token is coarse by construction: `notifications` is the
narrowest scope that works, and it still reads every notification on the account.

**Discord drops `workflow_run`.** So the naive shortcut, pointing a GitHub webhook straight at a
Discord `/github` endpoint and skipping pns entirely, cannot deliver the one event the operator
asked for. It would deliver `check_suite`, which is adjacent and not the same thing, and it would
deliver nothing to the lamps or the phone. The receiver is not incidental; it is the only path.

## Decision 1: one event shape

**One `GithubEvent`, produced by both transports, is the whole design.** Everything downstream
(lamp, Discord channel, phone, banner, deduplication) reads that one value and never learns which
transport produced it.

```
GithubEvent
  repo         "webdavis/dotfiles"     the full name, owner included, always present
  kind         WorkflowRun | Check | ReviewRequest | Mention | Release | SecurityAlert
  outcome      Passed | Failed | Neutral
  title        "lint"                  the workflow, check, pull request or release name
  url          "https://github.com/..." where tapping the notification lands
  identity     a stable key for this event, see below
  occurred_at  epoch seconds
```

**`kind` is a closed enum for `Behaviour`'s reason.** An open string is a value that reaches a
config-driven map, matches nothing, and produces silence nobody can see. The six variants come from
the operator's own list and they map cleanly onto both transports: `ci_activity` and `workflow_run`
both become `WorkflowRun`, `check_run` and `check_suite` and the `CheckSuite` subject type become
`Check`, `review_requested` and `pull_request` with `review_requested` become `ReviewRequest`, and
so on. **A notification reason or webhook event with no variant is DROPPED and counted**, never
mapped to a catch-all, because a catch-all kind is how `assign`, `subscribed` and `state_change`
turn a useful lamp into a lamp that is always on.

**`outcome` is three-valued and not two.** `Neutral` exists because a review request, a mention and
a release have no pass or fail, and forcing them into `Passed` would flash the pass colour at a
human asking for something. A `Neutral` event delivers to Discord and never touches the lamp. That
is stated here rather than in the lamp section because it is a property of the event shape: a lamp
that cannot represent an event is the event's problem to declare.

**`identity` is what makes the same event arrive twice and be delivered once.** It is the tuple
`(repo, kind, provider id)`, where the provider id is the workflow run id, the check run id, the
pull request number plus reviewer, the notification thread id, or the release tag, in that order of
preference. Both transports can compute it: a webhook payload carries the numeric id directly, and a
notification's `subject.url` ends in it. The receiver and the poller each look the identity up in a
small seen-set before submitting, and the set is the ONE piece of state both transports share.

**The seen-set lives in pns's state directory, holds an identity and the time it was first seen, and
expires entries after 24 hours.** Twenty-four hours is longer than any plausible transport skew (a
webhook is seconds, a poll is a minute, a laptop asleep overnight catches up on wake) and short
enough that the file stays small. An expired entry costs one duplicate notification, which is the
cheap failure; an unbounded file costs a growing read on every tick, which is the expensive one.

**`extensions` carries the GitHub parts across `pns submit`.** The request envelope already has a
field for exactly this, documented as "producer-specific data, carried verbatim and never read
here", so the GitHub event rides in `extensions.github` and pns's own policy reads `signal`,
`route` and `class` the way it does for every other producer. Nothing about the envelope changes.

## Decision 2: two transports

### The candidates

| Approach | Latency | Sees Dependabot and other people's pushes | Survives sleep and outage | Inbound exposure | New moving parts |
| --- | --- | --- | --- | --- | --- |
| (a) Poll only | up to 60 s | **no**, `ci_activity` is "a workflow run that you triggered" | **yes**, that is what the cursor is for | none | one daemon job |
| (b) Push only | ~1 s | yes | **no**, "GitHub does not automatically redeliver failed deliveries" | one hostname | App, tunnel ingress, Access policy, receiver, LaunchAgent, posture control |
| (c) Push with API redelivery as the catch-up | ~1 s | yes | partly, and only by adding a second poller against `GET /app/hook/deliveries` plus JWT signing | one hostname | everything in (b) plus a JWT signer and a delivery reconciler |
| **(d) Both, poll as the baseline, push for speed** | ~1 s typical, 60 s worst | yes | yes | one hostname | everything in (b) plus (a) |

**(a) alone is the operator's stated gap.** The verified `ci_activity` wording is decisive: a
Dependabot-triggered run and a collaborator's push produce a run the operator did not trigger, so no
notification is generated, so no poll can ever see it. This is not a latency argument.

**(b) alone fails the machine it runs on.** dresden is a laptop. It sleeps, it changes networks, the
tunnel goes down, and GitHub does not retry. Every event in that window is lost permanently, with no
local signal that anything was missed.

**(c) is the interesting trap.** It looks like it removes the poller: the App can list its own
deliveries and redeliver failures through the API. It does not. Reconciling deliveries needs its own
scheduled job, its own state, JWT signing with the app's private key (a second secret, and one whose
compromise is worse than the token's), and it only ever sees events the App was subscribed to, so it
still misses anything the webhook was not configured for. It is a poller with more secrets and a
narrower view. **Rejected, and the redelivery API stays in this document as an operator recovery
tool rather than a mechanism.**

**Recommendation: (d), and poll is built first.** Not because it is smaller but because push depends
on it: without a catch-up path, the push transport's first sleep is a silent data loss, and shipping
a notification source whose failure mode is silence is the thing "no manual-intervention designs"
forbids. The order is a correctness order.

### 2a: the poll transport

**One conditional request, once per `X-Poll-Interval` seconds, from the daemon.**

`ensure_github_poll` sits beside `ensure_presence_poll` in `pns-application`, registered on the same
`SWITCH_TICKS` sweep, cancelled when `[plugins.github]` is absent or disabled, with `every` set from
the interval the last response reported. The job's argv is `["github", "poll", "--daemon"]`.

One tick does exactly this:

1. `GET https://api.github.com/notifications?participating=false` with
   `Authorization: Bearer <token>`, `Accept: application/vnd.github+json`,
   `X-GitHub-Api-Version: 2022-11-28`, and `If-Modified-Since: <the stored Last-Modified>`.
1. `304` means nothing happened. Store the new `X-Poll-Interval` if it moved, and exit. **This costs
   no rate limit at all**, which is the verified property that makes a 60 second poll free.
1. `200` means new threads. For each, map `reason` plus `subject.type` to a `kind`, skip what has no
   variant, compute the identity, drop what the seen-set already holds, and submit the rest oldest
   first, the way the old watcher ordered its notifications.
1. Store the response's `Last-Modified` as the new cursor and the `X-Poll-Interval` as the new
   `every`.

**The cursor is `Last-Modified`, not a timestamp we invent, and not a mark-as-read.** Marking a
notification read is a GitHub write and is out of scope, so unread threads keep coming back forever
and the seen-set is what makes that harmless. This is a deliberate trade: the design spends a small
local file to avoid touching the operator's notification inbox, which is theirs.

**The rate budget, computed rather than assumed.** One request per 60 seconds is 60 per hour against
an authenticated limit of 5000, and the 304s among them are free. The May document measured the old
watcher at 660 per hour because it asked per repository. Asking the notifications endpoint instead
of the runs endpoint is what collapses that number, and it is also what buys review requests,
mentions, releases and security alerts for the same one call.

**The known wrinkle, and it is a build-time verification rather than a design choice.** The
documentation marks `subject.url` required, but a `CheckSuite` subject has historically carried a
null `url`, which would leave a `WorkflowRun` event with no link. Before PR 2 is written, make one
real call and look. If the field is null, the fallback is `repository.html_url` plus `/actions`,
which lands the operator in the right place with one more click, and the event's `url` field is
never empty.

**Two behaviors worth pinning, both behavior of a tool we wrote:** a 304 response produces no
submission and advances no cursor except the interval; a 200 carrying a thread whose identity is
already in the seen-set produces no submission.

### 2b: the push transport

**A GitHub App on the account, a second hostname on the existing tunnel, a Cloudflare Access bypass
limited to GitHub's ranges, and a receiver process that is not the daemon.**

**Why a GitHub App and not a per-repository webhook.** One installation on the account covers every
repository, including ones created later, with one secret and one endpoint. Per-repository webhooks
mean N secrets, N settings pages, and a new repository that silently notifies nothing. The App is
also what makes the redelivery API available as a recovery tool. Its permissions are read-only and
exactly the five the verified table names: Actions, Checks, Pull requests, Contents, Dependabot
alerts. It has no write permission of any kind, which is the out-of-scope rule enforced at the place
it is actually enforceable.

**The ingress is two lines in a file that already exists.** `~/.cloudflared/config.yml` gains a
second hostname above the 404 catch-all, pointing at the receiver's loopback port. The tunnel keeps
its name and its credentials file; nothing about `agentmail.webdavis.io` changes. The catch-all
underneath is what makes an unknown hostname a 404 rather than a route to something.

**Cloudflare Access fails closed by default, and the Bypass policy is the only opening.** The
application is the new hostname. Its policy set is exactly one Bypass rule whose selector is
GitHub's six webhook ranges, and no Allow rule. Anything from any other address meets no policy and
is refused by Access before it reaches the tunnel. The verified warning applies and is accepted
knowingly: "Bypass does not enforce any Access security controls and requests are not logged", so
the bypassed traffic is unauthenticated and unlogged at the edge. **That is fine because the edge is
not the boundary.** The signature check is, and it runs on every request regardless of source
address. Access is the outer filter that keeps internet background noise off the receiver; the
HMAC is what decides whether a payload is real.

**The receiver is its own process, and `pns github receive` is its argv.** It binds
`127.0.0.1:<port>` only, reads one request, and:

1. Refuses anything that is not `POST` to the one path, anything over a body ceiling, and anything
   without `X-Hub-Signature-256`, `X-GitHub-Event` and `X-GitHub-Delivery`.
1. Computes HMAC-SHA256 over the exact body bytes with the webhook secret and compares against the
   header's `sha256=` digest **in constant time**, through `hmac`'s own `verify_slice`, which is
   constant time by construction and is already a dependency of this workspace.
1. Rings the doorbell: runs the same poll `pns github poll` runs, rather than mapping the payload
   itself, because a webhook body carries neither the notification thread's id nor its
   `updated_at`, so parsing it here would give the two transports two different spellings of the
   same identity.
1. Answers `204` and forgets the connection.

**Why a subcommand of the existing binary rather than a second binary.** `cargo install` installs
every binary a package declares, so a second one is a second artifact in everyone's `~/.cargo/bin`
for a feature only this machine's tunnel uses. A subcommand is one artifact, one build, one version.
The operator's constraint is satisfied either way: the receiver is a **different process** from
`pns daemon run`, started by its own LaunchAgent, so the always-on daemon still never listens on a
socket. Its environment holds the webhook secret and nothing else, because it needs nothing else:
it verifies, maps, and hands off.

**The LaunchAgent is `com.webdavis.pns-github-receiver`**, shaped like `com.webdavis.pns-daemon`:
`RunAtLoad`, `KeepAlive` as `{ SuccessfulExit: false }` so a clean exit from a disabled config stays
exited, a stated `ThrottleInterval`, and logs under `~/.local/log/`.

**The posture control, and why it is not a data record.** The operator's requirement is that posture
verifies the tunnel exposes exactly the declared hostnames. No existing reader in
`.chezmoidata/macos_posture_controls.yaml` reads cloudflared, and an unknown reader aborts the
render, so this is new posture code. Its natural shape is `posture funnel`'s, which is the closest
thing already built: read the live ingress, compare against a declared set, page on any hostname
that is not declared, keep a baseline so a known state does not page twice. **It is posture's work,
in posture's workspace, and posture learns nothing about pns or GitHub from it**: the declared
hostname set is configuration, and `agentmail.webdavis.io` is already in it on day one.

## Decision 3: the token

**A classic personal access token with the `notifications` scope, in pns config by KeePassXC entry
name, in the values-file pattern.**

```toml
[plugins.github]
token = { keepassxc = "GitHub :: Personal Access Token (pns notifications)", field = "Password" }
```

**Not the operator's `gh` login**, which was the brief's explicit instruction and is right for three
reasons that are worth recording. `gh`'s token is interactive, refreshable and scoped to whatever
the operator last authorized; a daemon reading it is a daemon whose credentials change without
anyone deciding they should. pns already spawns `gh` for the recap's merged pull requests and the
Cargo.toml comment states the boundary precisely: "`gh` carries its own auth and pns never touches
it." Reading `gh`'s token would break a boundary the codebase wrote down. And a token that lives in
the vault can be scoped to `notifications` alone, which `gh`'s cannot.

**The scope is `notifications`, not `repo`.** Both are documented as sufficient and `repo` is read
and write on every repository the operator can reach. A notification source has no business holding
a write scope.

**The fine-grained token question is settled and the answer is no.** The documentation is explicit:
"These endpoints only support authentication using a personal access token (classic)." So the design
requires a classic token, and the residual risk is stated rather than mitigated: `notifications` is
the narrowest scope available and it still reads every notification on the account. The webhook
secret is a separate value with a separate entry, held only by the receiver.

## Decision 4: the `github` lamp behaviour, and the colour problem the May pair has

**A sixth `Behaviour` word, `Github`, carrying two colours the way `Unread` already does**, one
`[lights.github]` table with `duration_ms` and `brightness` like `done` and `failed`, and two colour
keys. Selectable per lamp through the existing `shows` list, so a dedicated GitHub lamp is
`shows = ["github"]` and nothing else. Dim-window rules unchanged: `github` is a behaviour like any
other, so a target's `dim_behaviours` may name it and a target with a window that does not is
suppressed inside the window, exactly as today.

```toml
[lights.github]
duration_ms = 4000
brightness = 100
pass = [0.2725, 0.1283]
fail = [0.5562, 0.4084]

[lights.lamp."3F - Studio - HCL1"]
shows = ["github"]
```

**Three things about that table are new to the config, and each is a real cost.**

1. **It is the first configurable colour anywhere in pns.** The other five behaviours' colours are
   compile-time constants, each locked on a real lamp. Making one behaviour's colour a knob creates
   an asymmetry a reader will notice, and the honest answer is that it is justified here and not
   elsewhere: the other five were locked by observation and these two have never been on a lamp.
1. **The renderer refuses a float today.** `render_value` handles bool, integer, string, string
   array and the secret table, and answers "type `float` does not render" for anything else. One new
   arm for a float and one for an array of floats is the whole change, and it fails loudly if
   forgotten.
1. **The values file gains two colour lines and the resolved-config snapshot moves.** That snapshot
   test is the guard against a values file silently switching settings off; when it fails on this
   change, the failure prints the diff and the accept command, and the diff is the thing to read.

**An xy pair, not a hex colour.** `pulse.rs` states the reason and it is not a preference: "Not RGB,
because the bridge clamps RGB into its gamut and desaturates hard; xy bypasses that conversion." A
hex knob would be friendlier to type and would silently render a different colour than it names.

### The colour measurement, which is the finding this document exists to record

The May pair is purple at xy (0.2725, 0.1283) for pass and orange at xy (0.5562, 0.4084) for fail.
Both sit inside the gamut C triangle the research verified, so the research's own test passes. But
pns built its lamp vocabulary **after** that research, and `BLOCKED_COLOR`'s docblock records the
separation metric that vocabulary was tuned against: two colours 0.067 apart in xy "read as one" in
daylight, the swap that fixed it produced a 0.207 separation, and the closest surviving pair in the
whole set is failure against unread success at 0.192. Measured in the same metric:

| Proposed colour | Nearest shipped colour | Distance | Verdict by the vocabulary's own bar |
| --- | --- | --- | --- |
| purple (0.2725, 0.1283) | `BLOCKED_COLOR` magenta (0.3395, 0.1379) | **0.068** | the exact distance already rejected as "read as one" |
| orange (0.5562, 0.4084) | `UNREAD_SUCCESS_COLOR` daylight (0.50, 0.40) | **0.057** | closer still |
| orange (0.5562, 0.4084) | `FAILURE_COLOR` red (0.675, 0.322) | 0.147 | below the 0.192 worst existing pair |

**And there is no orange that fixes it.** A grid search over the whole warm region inside gamut C
finds a best-case separation of **0.117** from the shipped set, because red at (0.675, 0.322) and
daylight at (0.50, 0.40) already bracket that corner. The warm end of the vocabulary is spent. A
purple can be improved (a violet around (0.20, 0.295) clears 0.21 from everything), but the fail
colour cannot, as long as it is orange.

**The resolution is the operator's own design, not a different colour.** The ruling asks for a
DEDICATED GitHub lamp, one config line. On a lamp that carries `shows = ["github"]` and nothing
else, cross-behaviour separation stops mattering: the lamp's identity is where it is, and the only
two colours it ever shows are purple and orange, which are 0.399 apart and unmistakable. The
collision is real only where the `github` behaviour rides a lamp that also carries `blocked` or
`unread`.

So the recommendation is precise: **ship the May pair as the default, and document that `github`
belongs on a lamp of its own.** The design should make that the easy path rather than a warning
nobody reads, and the cheapest way is prose in the shipped config comment for `[lights.github]`
saying the pair is chosen for a dedicated lamp and naming what it collides with on a shared one.

**Neither colour has passed the one test a colour can pass.** Every other constant in `pulse.rs`
carries a dated real-lamp trial. These two carry a desk research document. That is exactly why they
are configurable, and it is the reason "pick the GitHub lamp" is an operator step: the trial is
observe, adjust, lock, and it cannot be done from here.

**One thing the gamut check is not.** `FAILURE_COLOR` at (0.675, 0.322) is itself marginally outside
the community gamut C triangle, 0.021 from its red vertex, and it ships and is operator-locked. So
gamut containment is a sanity check on a proposed colour, never a gate: the bridge clamps, and the
lamp is the judge.

## Decision 5: Discord, one channel per repo

**SUPERSEDED on 2026-09-15 by `docs/superpowers/specs/2026-09-15-pns-discord-destination-design.md`,
and the table below is not to be built.** The operator's ruling is one `repo -> channel entry` map in
the pns config, shared by the GitHub source and by session events, so `[plugins.github.channels]` is
cancelled in favour of `[plugins.discord.channels]`: one repository resolves to one channel whichever
producer named it, GitHub spelling it `webdavis/dotfiles` and a session spelling it `dotfiles`. The
channels are `#<project>-dev` rather than `#github-<repo>`, the catch-all is `#github-notifications`
rather than `#github`, the interim hermes `github` route below is cancelled outright, and a config
still holding `[plugins.github]` is refused naming it as a plugin nothing registered. What survives
from this decision is its reasoning, which the replacement keeps: the full name is tried first because
the extraction is coming, and the catch-all is a required key because a map without one silently
swallows the first event from every new repository. Everything after this paragraph is the cancelled
original, kept for that reasoning.

**A `[plugins.github.channels]` map keyed on the full repository name, plus a catch-all.**

```toml
[plugins.github.channels]
"webdavis/dotfiles" = { keepassxc = "Discord (Uriel) :: Channel ID (#github-dotfiles)", field = "Password" }
default = { keepassxc = "Discord (Uriel) :: Channel ID (#github)", field = "Password" }
```

**Keyed on the FULL name, owner included, because the extraction is coming.** pns, uu and posture
live inside the dotfiles repository today and are being lifted into their own repositories with
`git subtree split`. GitHub only ever knows repositories, so the day `webdavis/pns` exists it starts
producing events under a name this map has never seen, lands in the catch-all, and the fix is one
line. Keying on a short name would make that day a collision instead of an addition.

**Delivered by pns's direct Discord destination**, the one the routing ruling approved so pns can
create a thread per session and read back a message id. A channel id is what that destination takes,
so a per-repo map is a map of channel ids, and the bot token is a single separate entry. This is why
the channels are ids and not names: hermes routes address a channel by route name through the
gateway's own table, and the direct destination addresses it by id.

**Until that destination exists, one hermes `github` route feeds the catch-all.** That is one row in
`[plugins.hermes.keys]`, one route in the hermes modify template, and two KeePassXC entries the
operator creates (`Hermes :: Webhook Secret (#github)` and the channel id the gateway posts to).
Every repository lands in `#github` in that interim and the per-repo map is inert, which is the
right degradation: the events arrive, in one place, and the split is a later configuration change
rather than a later feature.

**An unmapped repository is a delivered event, never a dropped one.** The catch-all is the whole
answer to "a webhook for a repo with no channel mapping", and it is why `default` is a required key
rather than an optional one: a map with no catch-all is a map that can silently swallow the first
event from every new repository.

## Decision 6: the phone and the banner

**GitHub is work, not machine health, so it never uses the `priority` class.** The 2026-09-14
morning ruling is exact: `priority` is posture CRIT, a failed unattended upgrade, a dead daemon.
A failing lint job is none of those. A GitHub event therefore submits with no `class` at all and
takes pns's ordinary path.

**A failure earns the phone when away, through the normal presence gate.** No new mechanism: the
submitted request carries `Signal::Failed`, the delivery plan keeps the presence-gated mobile leg
when `delivery.phone_card` is true, and the operator at their desk gets a banner instead. This is
the same treatment a long command's failure gets and for the same reason.

**A pass earns the banner and the durable log, and never the phone.** This is the question the brief
asked to be answered explicitly, and the answer is no: a green check is information the operator
wants to be able to find, not an interruption worth a device in another room. The pass reaches the
lamp (which is the glanceable surface, and the whole reason for a dedicated GitHub lamp), the
Discord channel (which is the durable record), and a banner if the operator is at the machine.
`Signal::Succeeded` with `scope` left at `Automatic` produces exactly that today.

**A `Neutral` outcome reaches Discord and stops there.** A review request or a release has no pass
or fail to pulse, and a lamp that lights for "someone asked you to review something" is a lamp that
is on all day.

## Decision 7: failure modes

| Failure | Behavior | Distinguishable |
| --- | --- | --- |
| the laptop sleeps | the daemon's job is leased and re-registered on wake; the first poll after wake carries the stored `Last-Modified` and returns everything missed | yes, the events arrive late rather than never |
| the tunnel is down | pushes are lost permanently ("GitHub does not automatically redeliver failed deliveries"); the poll catches everything the operator triggered, within 60 s of the tunnel coming back | **partly**: a run triggered by Dependabot during the outage is never seen by either transport |
| the rate limit is exhausted | a 403 with `x-ratelimit-remaining: 0`; the poll backs off to the reset time, and one poll per minute of free 304s makes this near-unreachable in practice | yes, in the daemon log |
| the token expired or was revoked | 401; the poll stops and says which config key names the entry | yes, and it must be loud: a silent 401 is a source that looks alive |
| the same event arrives on both transports | the seen-set refuses the second, whichever arrives second | yes, by counting refusals |
| a webhook arrives for a repo with no channel mapping | delivered to the `#github` catch-all | yes, the channel it lands in says so |
| Cloudflare Access is misconfigured to allow everything | the receiver still refuses every unsigned payload; Access is the outer filter, not the boundary | **no**, and this is the accepted residual: a Bypass policy is unlogged by design |
| Cloudflare Access is misconfigured to allow nothing | GitHub's deliveries fail at the edge; the App's delivery list shows the failures and the poll keeps working | yes, in the App's delivery list |
| a forged payload | refused at the signature check, constant-time, before any parsing; counted, not delivered | yes, by counting refusals |
| GitHub's webhook ranges change | deliveries start failing at Access; the ranges are re-read from `GET /meta` and the policy updated | yes, same as above, and it is an operator step |
| the receiver is not running | the tunnel returns a connection error to GitHub; the poll is unaffected | yes, at the App's delivery list and in launchd |
| a notification reason with no `kind` variant | dropped and counted | yes, by the count |
| `subject.url` is null on a CheckSuite | the event's url falls back to the repository's actions page | yes, visibly, in the link |
| the seen-set file is corrupt or unreadable | treated as empty, which costs duplicate notifications and never loses one | yes, in the daemon log |

**The silent state is the tunnel down during a Dependabot run.** Nothing local knows that a webhook
GitHub tried to send did not arrive, because the poll cannot see that run at all. The honest
mitigation is the App's own delivery list, which the operator can read and redeliver from, and the
honest statement is that a mechanical detector for "a delivery failed while we were offline" would
be the (c) reconciler this design rejected. If that window turns out to matter in practice, (c) is
the upgrade path and it is a whole PR of its own.

## Out of scope

- **Every GitHub write.** No rerunning a workflow, no merging, no approving, no commenting, no
  marking a notification read. The App's permissions are read-only, which is where this is enforced.
- **Acting on an event from the notification**, including a Discord button that reruns a job. The
  buttons research is a separate lane and this design deliberately does not anticipate it.
- **The (c) delivery reconciler**: JWT signing, `GET /app/hook/deliveries`, and automatic redelivery.
  Named as an upgrade path, not built.
- **Making the other five lamp colours configurable.** The asymmetry is acknowledged in Decision 4
  and left standing; those five are locked on real lamps and nobody has asked to move them.
- **Threads per repository in Discord.** The direct destination's thread work is the session
  attribution design's, and a GitHub event is not a session.
- **Marking notifications read to shrink the poll**, which is a write and would also change what the
  operator sees in their own inbox.
- **Any `.chezmoiremove`, retirement script or removal mechanism**, per the standing ruling. If the
  interim hermes `github` route is retired after the direct destination ships, the leftover is listed
  for the operator to trash by hand.
- **Per-repository webhooks** as an alternative to the App, and organization-level webhooks.

## Assumptions made in the operator's place

1. **The token is a classic personal access token with the `notifications` scope.** Forced by the
   documented refusal of fine-grained tokens. *Alternative:* the `repo` scope, also documented as
   sufficient and carrying write access to every repository, which a notification source must not
   hold.
1. **The poll's cursor is `Last-Modified` plus a local seen-set, and notifications are never marked
   read.** *Alternative:* mark each notification read after delivering it, which makes the cursor
   trivial and is a GitHub write against the operator's own inbox.
1. **`kind` is a closed enum of six, and an unmapped reason is dropped and counted.**
   *Alternative:* an open string kind with a catch-all, which delivers `assign`, `subscribed` and
   `state_change` too and turns the lamp into a lamp that is always on.
1. **`outcome` has a third value, `Neutral`, and a `Neutral` event never touches the lamp.**
   *Alternative:* two values, which flashes the pass colour when a human asks for a review.
1. **The seen-set expires after 24 hours.** *Alternative:* any other window, or no expiry and an
   unbounded file. Twenty-four hours is a judgement sized to "a laptop asleep overnight", not a
   measurement.
1. **The receiver is `pns github receive`, a subcommand run by its own LaunchAgent**, so the
   always-on daemon never listens. *Alternative:* a second binary in the pns package, which
   `cargo install` would put in every installing user's `~/.cargo/bin` for a feature only this
   machine's tunnel uses.
1. **The receiver holds the webhook secret and nothing else, and hands events to `pns submit`
   rather than delivering them itself.** *Alternative:* a receiver that delivers directly, which
   would need the Discord token, the hermes keys and the lamp config in a process that is reachable
   from the internet.
1. **Cloudflare Access carries exactly one Bypass policy scoped to GitHub's six webhook ranges, and
   no Allow policy.** *Alternative:* a Service Auth policy with a service token, which Access does
   log, and which GitHub cannot send because a webhook carries no configurable request headers
   beyond its own.
1. **The ingress is a second hostname on the existing `agentmail-receiver` tunnel.**
   *Alternative:* a second named tunnel, which is a second credentials file, a second process and a
   second thing to keep running, for one more hostname.
1. **posture's tunnel control is shaped like `posture funnel`: live ingress against a declared set,
   with a baseline.** *Alternative:* a new reader in `macos_posture_controls.yaml`, which is the
   cheaper shape and cannot express "exactly these hostnames and no others" against a list.
1. **The `github` lamp behaviour carries two colours and one brightness**, on `Unread`'s precedent.
   *Alternative:* a brightness per colour, which the May research asked for (80 for purple, 100 for
   orange) and which adds a knob no other pulse behaviour has.
1. **The shipped colour defaults are the May pair, documented as chosen for a dedicated lamp.**
   *Alternative:* pick a better-separated pair now, which is possible for the purple and provably
   not possible for the orange, and which would override the operator's own stated choice on a
   question only a real lamp can settle.
1. **A colour is an xy pair of floats, and `render_value` gains a float arm.** *Alternative:* two
   scalar keys per colour, or a `"x,y"` string, both of which avoid the renderer change and read
   worse in the config the operator edits.
1. **The Discord map is keyed on the full repository name and requires a `default` catch-all.**
   *Alternative:* a short-name key, which collides the day the extraction produces `webdavis/pns`.
1. **A pass never reaches the phone.** *Alternative:* card every completed run, which is the old
   watcher's behavior and the reason the operator stopped wanting it.
1. **GitHub events carry no `class`, so they never bypass a mute or a Focus.** *Alternative:* a
   `github` class in `delivery.bypass_silence_classes`, which makes a lint failure interrupt a Focus
   session and contradicts the priority ruling in spirit.

## Open questions

1. **Is a classic personal access token acceptable at all?** It is the only token the notifications
   API takes, its narrowest useful scope still reads every notification on the account, and it lives
   in the vault next to everything else. If the answer is no, the poll transport cannot be built and
   the whole design collapses to push-only, which loses the catch-up.
1. **Does the purple survive a real lamp beside the blocked magenta, at 0.068?** Decision 4 says it
   does not need to, because the GitHub lamp is dedicated. That holds only if the operator really
   does give it a lamp of its own rather than adding `github` to a lamp already showing `blocked`.
1. **Is orange the right fail colour given that nothing orange clears 0.117 from the shipped set?**
   The warm corner is spent on red and daylight. Accepting orange means accepting that a shared lamp
   cannot distinguish a GitHub failure from unread success news.
1. **Should the six `kind` variants all be built at once, or does PR 1 ship `WorkflowRun` alone?**
   The event shape is cheaper to get right in one pass; the lamp and the channel only care about
   `outcome`. Shipping all six means mapping reasons nobody has seen fire yet.
1. **Which repositories get their own channel on day one?** The catch-all makes this reversible, and
   the answer affects how many KeePassXC entries the operator creates before PR 4.
1. **Does the interim hermes `github` route get created at all, or does PR 3 wait for the direct
   Discord destination?** Creating it is two vault entries and a route block, and retiring it later
   leaves a deployed leftover this repository has no mechanism to remove.
1. **Should the receiver refuse a delivery whose `X-GitHub-Delivery` it has already processed?**
   The seen-set already deduplicates on event identity, which covers it, but a redelivery the
   operator triggers by hand from the App's delivery list would then be silently ignored, which is
   the opposite of what they wanted when they pressed the button.
1. **Is the posture tunnel control a prerequisite for PR 3, or does it follow?** This document
   places it inside PR 3 because a hostname that reaches the internet with nothing watching it is
   the exact gap `posture funnel` exists to close. The alternative is to ship the ingress first and
   add the control after, which is a window.
1. **Does `subject.url` come back null for a CheckSuite?** One real call answers it and it decides
   whether the fallback in 2a is dead code or load-bearing.

## Build plan

Four pull requests, in this order. **PR 1 waits for `feat/pns-per-route-keys` to merge**: both
touch `dot_config/pns/config-values.toml`, the generated template and the render layout, and the
conflict is mechanical but real.

**PR 1: the event kind, the lamp behaviour and the colour config.** The `GithubEvent` type and its
`kind`/`outcome` enums in `pns-domain`; `Behaviour::Github` added to the closed set and to
`BEHAVIOUR_WORDS`; the `[lights.github]` table in the layout with `duration_ms`, `brightness`,
`pass` and `fail`; the float and float-array arms in `render_value`; the colour pair resolved from
config with the May pair as the shipped default; the regenerated template and the accepted
resolved-config snapshot. **Testable by hand the day it merges**, with no transport at all: hand
`pns submit --json` an event carrying a `github` extension and watch the lamp. Behaviors worth a
test, all behavior of a tool we wrote: a `shows` list naming `github` routes a GitHub event to that
lamp and a list that does not leaves it dark; a `Neutral` outcome produces no pulse; a configured
colour pair reaches the driver and an absent one falls back to the shipped default; a colour outside
the unit range is refused by name; `render_value` writes a float pair and still refuses a type that
does not render.

**PR 2: the poll transport.** `[plugins.github]` with `enabled` and `token`; `ensure_github_poll`
beside `ensure_presence_poll`; the `github poll` subcommand; the conditional request with the
`Last-Modified` cursor and the `X-Poll-Interval` honored as the job's `every`; the reason-to-kind
map; the seen-set with its 24 hour expiry. Behaviors worth a test: a 304 submits nothing and
advances only the interval; a 200 submits oldest first; an identity already in the seen-set submits
nothing; an unmapped reason is dropped and counted; a 401 stops the poll and names the config key;
an absent or disabled table cancels the registered job.

**PR 3: the push receiver, the tunnel ingress, the LaunchAgent and the posture control.** The
`github receive` subcommand binding loopback; constant-time signature verification; the second
ingress hostname in `~/.cloudflared/config.yml`; `com.webdavis.pns-github-receiver` and its loader;
posture's declared-hostname control with its baseline. The operator creates the GitHub App, its
webhook secret entry, and the Cloudflare Access Bypass policy. Behaviors worth a test: a payload
with a valid signature is mapped and submitted; a payload with an invalid signature is refused with
no submission and no parse; a payload with no signature header is refused; an oversized body is
refused; an event whose identity the poll already delivered is refused; a `workflow_run` with action
`in_progress` produces no event. The posture control's own behaviors are posture's tests: an
undeclared hostname pages, a declared one does not, a second tick on a known state pages once.

**PR 4: documentation, and the operator's mapping.** The ledger entry, a runbook paragraph naming
the App, the token scope, the Access policy and the receiver's LaunchAgent, plus the per-repo
channel map filled in with the entries the operator created. This is where "put `github` on a lamp
of its own" gets written down where a reader will find it.
