# Local daemons: atuin, tailscaled, the hermes gateway, the pns Discord bot

Four long-running services on dresden, each with its own failure mode and diagnostic ladder. Atuin runs
as a chezmoi-tracked LaunchAgent: its plist lives under `Library/LaunchAgents/` and its loader is a
`.chezmoiscripts/run_onchange_after_*` script keyed on the plist's own hash, so the loader re-runs when
the plist changes rather than on every apply. Tailscaled is a launchd system daemon (see its section
below), not a chezmoi-tracked LaunchAgent. The hermes gateway is not a LaunchAgent either;
`hermes gateway` owns its lifecycle and this repository owns only its configuration. The pns Discord bot
at the end is a fourth thing again, neither a service nor a daemon: it is pns's own HTTP client, and it
is here because it delivers the same notifications the gateway does and is configured the same way.

## Shell history (atuin)

Atuin daemon mode is enabled (`[daemon] enabled = true; autostart = false`, in
`dot_config/atuin/private_config.toml.tmpl`). The daemon's lifecycle is managed by
`~/Library/LaunchAgents/com.webdavis.atuin-daemon.plist` (`KeepAlive=true`, `RunAtLoad=true`, running
`atuin daemon start --force` so a stale socket from a prior crash auto-cleans on restart). Command
recording is decoupled from `PROMPT_COMMAND` via the daemon.

History is stored in SQLite under `~/.local/share/atuin/` (atuin's default location beneath
`XDG_DATA_HOME`, which `~/.bashrc` exports as `$HOME/.local/share`; no repo file sets `db_path`). Sync v2
records are opt-in (`[sync] records = true`), which future-proofs the local schema even though
`auto_sync = false`. `filter_mode = "host"` restricts Ctrl-R to the current machine's history, and
`filter_mode_shell_up_key_binding = "session"` is moot in practice because bashrc runs
`atuin init bash --disable-up-arrow`. Bash's built-in history is fully removed (no `HISTFILE`,
`HISTSIZE`, `histappend`, `HISTCONTROL` or `HISTIGNORE` anywhere in the bashrc); atuin owns all
recording.

**Diagnostic ladder** when history stops recording:

```bash
atuin doctor                              # built-in: socket, db, env, shell hooks
launchctl list | grep atuin               # status: '0' = healthy, '-' = not running
ps aux | grep '[a]tuin daemon'            # daemon process
tail ~/.local/log/atuin-daemon.log        # crash messages
atuin daemon status; atuin --version      # 'Version' line should equal 'atuin <ver>'
```

`atuin status` is for *sync* status only and errors when not logged in. It is not a "is the daemon
working" check; use `atuin daemon status` (reports `Version`, `Protocol`, `Healthy`) for daemon health.

**Past failures**, each now self-healing:

- A stale daemon socket under `~/.local/share/atuin/` caused `EADDRINUSE` restart loops. Fixed by
  `--force` in the plist.
- `bash-preexec` went missing after atuin 18.x dropped its bundle. Fixed by sourcing
  `${HOMEBREW_PREFIX}/etc/profile.d/bash-preexec.sh` in the bashrc before `atuin init`.
- `brew` upgrading atuin in-place while the daemon kept running stale code silently broke recording via
  gRPC schema drift. Two independent guards now catch it:
  `.chezmoiscripts/run_after_45-bounce-atuin-daemon-on-upgrade.sh.tmpl` compares the version recorded in
  `~/.local/share/atuin/atuin-daemon.pid` against `atuin --version`, and `dot_bashrc.tmpl` compares the
  binary's mtime against that same pid file right after `atuin init`. Either one triggers
  `launchctl kickstart -k gui/$(id -u)/com.webdavis.atuin-daemon`.

## Hermes gateway (webhook routes)

The gateway is the hermes agent's webhook platform, switched on by `WEBHOOK_ENABLED` and `WEBHOOK_PORT`
in `~/.hermes/.env` (rendered from `private_dot_hermes/private_dot_env.tmpl`). It listens on
`127.0.0.1:8644` and every notification this machine sends to Discord arrives as a signed POST to
`http://127.0.0.1:8644/webhooks/<route>`.

### The routes

Six routes, and the template that owns them declares exactly these. A live route it does not name is
REMOVED by the next apply, which is how `unattended-upgrades` (superseded by `uu-runs`), `osquery`
(superseded by `posture-pages`) and `pns-recap` (retired with its channel on 2026-09-15) leave.

**A route is named for its Discord channel**, which is also the name both of its KeePassXC entries are
built from, so route name == channel name == entry name. The channels were renamed on 2026-09-14 and the
routes followed: `pns` became `pns-events`, `uu` became `uu-runs` and `posture` became `posture-pages`,
because the bare tool names are now GitHub repo channels (a future pns GitHub source posts there) and no
webhook route may take one. `explain`, added 2026-09-17, is the one exception: it posts into `#priority`
rather than a channel of its own, so it names `priority` as its `channel` and reads that channel's
existing entry instead of getting a new one.

| Route           | Who posts                                           | What                                                                                                                                                                                                                                |
| --------------- | --------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `general`       | nothing in this repo yet                            | The catch-all channel. Declared so an ad-hoc POST has a signed route of its own rather than borrowing one.                                                                                                                          |
| `pns-events`    | pns hook and daemon paths                           | Every routine agent event, the return recap included. The default route when nothing names one.                                                                                                                                     |
| `priority`      | the alert drainer; pns on `--delivery-class health` | Machine health and security ONLY (operator ruling 2026-09-14). Posture is held off it by `severity_route`.                                                                                                                          |
| `uu-runs`       | uu                                                  | The weekly unattended-upgrades record. Renamed from `unattended-upgrades`, then from `uu`.                                                                                                                                          |
| `posture-pages` | posture                                             | Every page it raises, including the critical ones, plus the daily digest, the heartbeat, poll and funnel.                                                                                                                           |
| `explain`       | posture, copying a delivered critical page          | The one AGENT route on this gateway. It carries no `deliver_only` key, so the gateway hands the posted body to an agent and delivers the agent's reply instead of the body itself. See "The posture critical-page explainer" below. |

Route names are not URLs: a producer names a route and the gateway's own table decides where it lands.
posture picks its route from the finding's tier in one place (`severity_route`,
`posture/crates/posture-domain/src/severity.rs`), and that one place holds EVERY tier on `posture-pages`,
critical included, so `priority` is held out of posture's tier map by posture rather than by anything the
gateway does; uu's default is `DEFAULT_RECORD_URL` (`uu/crates/uu-adapters/src/config/records.rs`) for
the weekly record, while its ALERT names no route at all: it sends `--delivery-class health` and this
machine's `[delivery_class.health] route` names `priority`, so a failed unattended upgrade pages without
uu knowing a channel exists; pns's recap takes `DEFAULT_ROUTE` like every other session event, because
the `pns-recap` route and its channel retired on 2026-09-15. pns keeps a ROSTER of the routes it posts
to, `pns_domain::routes::ROUTES`, because each one is signed with its own key and a route with no key is
a post pns refuses rather than signs with somebody else's. So adding a route pns posts to is two
registrations, the gateway and that roster, and a route only uu or posture posts to needs the gateway
alone.

### Where they live, and what a route carries

`~/.hermes/config.yaml` is a chezmoi **modify-template**,
`private_dot_hermes/modify_private_config.yaml`, and it owns exactly two things:
`platforms.webhook.extra.routes` (the whole map) and `tts.elevenlabs.voice_id` (one key). **Hermes owns
every other line of that file.** It rewrites the config from its own model at runtime, so the age capture
that used to hold this target turned every runtime write into source drift and pushed a snapshot back
over whatever hermes had just decided; the modify-template reads the live file, overlays those two
things, and hands the rest back untouched. When the overlays change nothing the live file is emitted back
byte for byte, so a no-op apply prints nothing.

Read the routes without changing anything:

```bash
yq -r '.platforms.webhook.extra.routes | keys' ~/.hermes/config.yaml
```

Every route carries a `secret`, `deliver: discord`, a `prompt` and a `deliver_extra.chat_id` naming its
channel. Five of the six also carry `deliver_only: true`, so their body is posted verbatim instead of
being fed to an agent; `explain` is the one route without it, which is what makes it an agent route.
`general`, `pns-events` and `priority` share the three-line prompt (`**{header}**`, `-# {subheader}`,
`{body}`); `posture-pages` and `uu-runs` keep the event-shaped prompt they already had; `explain` carries
its own explainer instruction, framing the posted page as untrusted data.

**Every secret and every channel id comes from KeePassXC by entry name, one secret entry per route and
one channel entry per channel.** Eleven entries, named off the route, except `explain`, whose channel
entry is `priority`'s:

| Route           | Secret entry                                | Channel entry                                    |
| --------------- | ------------------------------------------- | ------------------------------------------------ |
| `general`       | `Hermes :: Webhook Secret (#general)`       | `Discord (Uriel) :: Channel ID (#general)`       |
| `pns-events`    | `Hermes :: Webhook Secret (#pns-events)`    | `Discord (Uriel) :: Channel ID (#pns-events)`    |
| `posture-pages` | `Hermes :: Webhook Secret (#posture-pages)` | `Discord (Uriel) :: Channel ID (#posture-pages)` |
| `priority`      | `Hermes :: Webhook Secret (#priority)`      | `Discord (Uriel) :: Channel ID (#priority)`      |
| `uu-runs`       | `Hermes :: Webhook Secret (#uu-runs)`       | `Discord (Uriel) :: Channel ID (#uu-runs)`       |
| `explain`       | `Hermes :: Webhook Secret (#explain)`       | `Discord (Uriel) :: Channel ID (#priority)`      |

The parentheses in the secret titles are load-bearing: the retired single shared key lived at
`Hermes :: Webhook Secret :: #pns`, which is a different entry. The ElevenLabs voice is one more lookup,
`ElevenLabs :: Voice ID`, whose Password field holds the voice id.

A `Discord (Uriel) :: Channel ID (#explain)` entry and an `#explain` Discord channel both exist and are
both unused: the route deliberately reads `priority`'s channel entry instead, so one channel id lives in
one vault entry rather than two that could drift apart. Do not populate or wire the unused entry; it is
not a leftover to clean up, it is the cost of the route name not matching its channel.

**A missing KeePassXC entry aborts the whole apply. An empty one does not.** keepassxc-cli exits non-zero
on a title it cannot find, chezmoi fails the template on that, and a failed modify-template takes every
later target and every `run_after_` script with it, which is the safe direction to fail. An entry that
EXISTS with an empty Password field is the case that gets through: keepassxc-cli prints the empty field
and exits 0, and chezmoi renders an empty string (measured 2026-09-14 against a throwaway database). A
route rendered with an empty secret takes the entire webhook platform down at the next gateway start,
while looking healthy in the file, and `run_after_68` is what reports it, on the same apply, by presence
and never by value. Create both entries, populated, before naming a route in the template.

**Agents edit route NAMES and prompts, never values.** Nothing in this repo holds a secret or a channel
id, and the deployed copy is denied to Claude Code's file tools by a `Read(~/.hermes/config.yaml)` rule
in `private_dot_claude/modify_settings.json` (see `docs/runbooks/claude-code-settings.md`). Rotating a
secret or moving a channel is a KeePassXC edit plus an apply.

### Which sender reads which secret

The gateway and the senders read the SAME entries, so one apply lands both sides together and nothing is
left signing with a key the gateway no longer holds:

| Route           | Sender and the key it reads                                                                                                                                                                                                              |
| --------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `pns-events`    | `[plugins.log.keys] pns-events` in `dot_config/pns/config-values.toml`                                                                                                                                                                   |
| `posture-pages` | `[notify.hermes.keys] posture-pages` in `dot_config/posture/private_config.toml.tmpl`, and the same route in pns's own table                                                                                                             |
| `priority`      | `[plugins.log.keys] priority`, and `[notify.hermes.keys] priority` on posture's own side                                                                                                                                                 |
| `uu-runs`       | `[records] key` in `dot_config/uu/private_config.toml.tmpl`                                                                                                                                                                              |
| `general`       | no sender in this repository; an ad-hoc signed POST                                                                                                                                                                                      |
| `explain`       | posture only, `[notify.hermes.keys] explain` in `dot_config/posture/private_config.toml.tmpl`, and only for the copy of a delivered critical page (`notify.hermes.critical_copy_route`, `posture/crates/posture-adapters/src/hermes.rs`) |

pns refuses to post to a route its table names no key for, and records the refusal the way every other
refused hermes post is recorded; `pns doctor` names every route left without a key. Until the apply that
carries both sides has run, the senders still sign with the retired shared key and the gateway answers
401 on the routes that are not that key, so apply once, with KeePassXC unlocked, then
`hermes gateway restart`.

### The posture critical-page explainer

posture is `explain`'s only sender today, and it is a SECOND sender: `explain` never carries a page on
its own, only a copy of one `priority` already delivered. `AlertSink::submit`
(`posture/crates/posture-adapters/src/hermes.rs`) posts a critical finding to its own tier route first;
once that post comes back delivered, `copy_delivered_page` signs the same body a second time with the
`explain` key and posts it to `explain`, carrying a request id derived from the page's own id but never
equal to it (`request_id::derive_copy`). A Notice-tier page, a page whose own post failed, or a machine
with no `critical_copy_route` configured never reaches that second post at all.

The copy is rationed per distinct finding, not per page:
`posture/crates/posture-adapters/src/hermes/window.rs` keeps a rolling hour of claims in one timestamp
file under `~/.local/state/` (`posture-critical-copy-window.json`), a finding already claimed inside the
hour earns no second copy, and the hour holds at most twenty distinct findings before a further one is
withheld and reported on the local banner. The window fails open: a file it cannot read or write grants
the claim rather than silently switching the leg off.

Because `explain` is an agent route, `hermes` hands the copy to a model rather than delivering it
verbatim, and the prompt (`private_dot_hermes/modify_private_config.yaml`) instructs it to treat the page
as untrusted data and reply with a fixed four-part explanation, under 1200 characters, naming no command
of its own. The webhook agent sandbox is empty (`platform_toolsets.webhook: ["no_mcp"]`, same template),
so the model that reads a posture finding has no tool to misuse. The reply lands in the same `#priority`
channel as the page it explains, directly under it; it moves into the page's own Discord thread once the
pns Discord bot (task 82 in `docs/remaining-work.md`) exists to open one.

Operator step after the apply that first declares this route: `hermes gateway restart`, the same restart
"When a route changes" below already calls for, so the gateway loads `explain` rather than answering it
404\.

### Three gotchas

**The gateway does not expand `${VAR}` in its platform config.** `gateway/config.py` loads `config.yaml`
with a bare `yaml.safe_load` and merges `platforms` straight through, so a `chat_id` written as
`${DISCORD_HOME_CHANNEL}` reaches Discord as that literal string. A route's secret and channel id have to
be literals in the rendered file and cannot be reached from the `.env`, which is why they come from
KeePassXC through the modify-template and why a new route gets no `.env` line of its own.
`DISCORD_HOME_CHANNEL` is still in the `.env` because hermes itself reads it for the fallback channel;
`DISCORD_OSQUERY_CHANNEL` was removed on 2026-09-14 after a grep of the installed hermes source and of
this repo found no reader for it at all.

**A prompt template renders an unknown placeholder as itself.** `_render_prompt` substitutes a missing
key with `{the.key}` rather than failing, so a route whose template does not match its producers' body
shape delivers literal placeholders and no content. pns-shaped bodies carry `agent`, `state`, `project`
and `detail`, plus the composed `header`, `subheader` and `body`; the bash osquery alerter's carried
`alert.title` and `alert.detail`. Every route is templated for the pns shape now, `priority` and
`general` included, so its old producer is the one that would deliver placeholders.

**Route keys are now per route, and the comparison that used to police them is gone.** Every route once
had to carry the ONE secret pns signs with, and `run_after_68` compared the others against the default
route's value. Five separate vault entries leave nothing to compare a route against: agreement is now
between the entry a route renders from and the key its sender holds, and neither of those is in
`config.yaml`. What the check still sees is PRESENCE, by shape and never by value, which is why it
reports a route with no secret and a `chat_id` that is not a 17-to-20-digit Discord snowflake.

What that leaves. Nothing signs with the old key any more. The alert-drainer LaunchAgent and
`drain-undelivered-alerts.sh` retired once their queue read empty (`select count(*) from pending_alerts`
was 0) and every osquery agent ran a `posture` subcommand, so nothing writes that store and nothing reads
that key. `~/.config/osquery/webhook-secret` is dead weight, and it is watched by the
agent-attack-surface pack, so trash it alongside the LaunchAgent and the queue rather than on its own.
`severity_route` still holds EVERY posture tier on `posture-pages`, critical included; flipping its
critical arm back to `priority` is now a one-line change in posture's own workspace, and the test named
for the hold is what makes that flip deliberate.

**An empty `platform_toolsets.webhook` list is not the same thing as `["no_mcp"]`.** `[]` reads as "no
extra Model Context Protocol allowlist for this platform", which leaves every globally enabled Model
Context Protocol server live for a webhook agent run, cua-driver (which drives this Mac's graphical
interface) among them. `["no_mcp"]` is the documented sentinel `hermes_cli/tools_config.py` special-cases
to mean no tools at all, and it is what `explain`'s sandbox depends on: the route feeds a model text read
off a machine it does not control, so the boundary that matters is that the model has nothing to act
with. Saving a change from the `hermes tools` picker in the terminal UI drops the sentinel, because the
picker has no checkbox for it, so a picker session re-arms every server until the next apply puts
`["no_mcp"]` back.

**The gateway's duplicate cache is keyed on the delivery id alone, across every route.** Two posts
carrying the same `X-Request-ID` (or `X-GitHub-Delivery` or `svix-id`) within its one-hour window are one
delivery even when they target different routes: the second answers `{"status": "duplicate"}` and never
runs, gateway-wide rather than per route. This is why posture's copy of a critical page carries a request
id derived from the page's own id but distinct from it (`request_id::derive_copy`,
`posture/crates/posture-adapters/src/request_id.rs`): reusing the page's id would make the copy read as
the page arriving a second time and the gateway would drop it silently.

**An absent placeholder renders as its own literal braces, and that reaches Discord.** `_render_prompt`
degrades a missing key to `{the.key}` rather than failing, so a `deliver_extra` value built from a key a
sender never sends is not empty, it is the literal text `{that_key}` posted to the channel. This is why
`explain`'s route carries no `deliver_extra.thread_id`: posture's body has no `thread_id` field yet, so
an absent key there would send the literal string `{thread_id}` to Discord as a thread id rather than
degrading cleanly. A thread id is only added to a route once its sender always emits the key, which is
also true of any future field added to a route's `deliver_extra`.

### When a route changes

An apply writes the new `~/.hermes/config.yaml`, but the running gateway loaded the old one, so a new or
renamed route answers 404 until `hermes gateway restart`. That restart drains in-flight runs for up to
180 seconds, so it is deliberate rather than automatic, and `run_after_68` nudges rather than restarts.

Two operator commands after an apply, in this order:

```bash
hermes gateway restart
yq -r '.platforms.webhook.extra.routes | keys' ~/.hermes/config.yaml
```

The first loads the new table. The second is the read-only confirmation, and it should list exactly
`explain`, `general`, `pns-events`, `posture-pages`, `priority`, `uu-runs`. What to expect from the apply
itself: `run_after_68` prints NOTHING when all six routes are present with a secret and a snowflake
`chat_id`, `deliver_only: true` on the five delivery routes and absent on `explain`, no undeclared route
is left, `tts.elevenlabs.voice_id` is set, and the gateway already answers each route. Every line it does
print names the route, the condition, and the KeePassXC entry or command that fixes it, and never a
value.

## The pns Discord bot (project channels)

The bot is pns's OWN durable destination, one of the two transports `[plugins.log] type` names, and the
alternative to the hermes gateway above rather than a companion to it: there is ONE durable-log table, so
two of them cannot be declared at all. It holds no gateway intent and opens no websocket. It is an HTTP
client that posts to `discord.com/api/v10` and reads nothing back except its own responses, which is why
nothing restarts and nothing listens.

**It ships unselected.** `[plugins.log] type = "hermes"` is the live durable log today while the bot's
own token and channel map sit in the same table ready, so the cutover is that one line changed to
`"discord"` in one apply, and the rollback is the same line back.

**The bot's own three vault entries**, created once and never per project:

| Entry                                          | What reads it                                                       |
| ---------------------------------------------- | ------------------------------------------------------------------- |
| `Discord (Uriel) :: Bot Token (pns)`           | `[plugins.log] token`, in the `Authorization` header                |
| `Discord (Uriel) :: Application/User ID (pns)` | guild-scoped command registration, when slash commands are built    |
| `Discord (Uriel) :: Public Key (pns)`          | Ed25519 verification of interactions, when slash commands are built |

The last two are held for the deliberately-unbuilt `/pns` slash commands
(`docs/superpowers/specs/2026-09-15-pns-discord-destination-design.md`, "Deliberately out"). Only the
token is read today.

### Where a message lands

ONE MAP, `[plugins.log.channels]` in `dot_config/pns/config-values.toml`, shared by every producer: the
GitHub source and an agent session about the same repository resolve the same channel, because the lookup
tries the event's route, then `owner/name`, then the bare project name, then `pns-events` when there is
no project at all, then `default`. That is why the GitHub source design's own `[plugins.github.channels]`
map is cancelled rather than built: one repository, one channel, whichever producer named it. A config
that still holds `[plugins.github]` is refused out loud, naming `github` as a plugin nothing registered.

Two of those keys are the fallbacks and are never per project. `default` is `#github-notifications`, the
catch-all for a repository nobody mapped; `pns-events` is where an event with no repository at all lands,
which is every engine event and every recap composed outside a checkout. A recap composed INSIDE one
carries that repository's project and lands in its channel, and it opens no thread: it is a window of
time rather than a session, and the one message a day the operator most wants belongs at channel level.

### When a project is created

Three edits, in this order, and none of them writes an id into this repository:

1. Create `#<project>-dev` in the guild, the one channel that project gets. It carries that repository's
   CI, pull requests, GitHub notifications and agent session threads.
1. Create `Discord (Uriel) :: Channel ID (#<project>-dev)` in KeePassXC, holding the channel id in its
   Password field.
1. Add one line to `[plugins.log.channels]` keyed on the repository's bare name, naming that entry and
   field, exactly as every line already there does. A repository whose bare name will collide after a
   `git subtree split` takes a second line keyed `owner/name`, which the lookup tries first.

A channel id is a secret like every other id in that file, so the map holds ENTRY NAMES and never a
value: rotation is a vault edit plus an apply rather than a commit, and a committed file never records a
private guild's layout. Then apply with KeePassXC unlocked. Nothing restarts.

### What a failure says

Every line names the status and the config key to fix, never the token and never a channel id. A missing
or empty token posts nothing and says `[plugins.log] token`; a map that answered nothing says
`[plugins.log.channels] default`, because every lookup ends at the catch-all. A 401, a 403 or a 404
dead-letters on its first attempt and shows up in `pns failures`; a 429 or any 5xx is retried on the
ledger's own linear backoff, and the `Retry-After` header is logged rather than obeyed so a second
schedule cannot disagree with the ledger about when a leg is due.

## Tailscale (headless daemon)

Tailscale runs as the open-source `tailscale` **formula** (not the `tailscale-app` GUI cask) as a launchd
**system daemon** via `sudo tailscaled install-system-daemon`, which places a root-owned copy in
`/usr/local/bin` while the brew formula stays user-owned so `brew upgrade` runs unattended. It boots
before login and uses the `utun` interface, so there is no Network or System Extension to re-approve
after updates (the GUI variants' weakness on a headless host). State persists at `/Library/Tailscale`
across reboots.

Auth is a one-time manual `sudo tailscale up --accept-dns=true` plus flipping **Disable Key Expiry** on
the node in the admin console, after which node-key expiry will not force reauthentication (no auth keys,
no rotation, no KeePassXC). `run_onchange_after_66-tailscaled-status.sh.tmpl` is a sudo-free reminder: it
reads `tailscale status --json`, branches on `.BackendState`, and prints those one-time steps when the
daemon is starting, unauthenticated, awaiting machine auth or stopped. It is silent when the daemon is
running, exits 0 on every path, and never runs sudo or authenticates.

### DNS

Always `--accept-dns=true`, never a static `100.100.100.100` global resolver (that breaks off-tailnet).
The weak spot on the open-source macOS build is the resolver registration layer
(`tailscale/tailscale#13461`, `#19139`): tailscaled's internal MagicDNS resolver stays healthy, but its
registration of the `<tailnet>.ts.net` suffix route with macOS can silently half-fail (search-domain
fragment written, no nameserver route), including at home, so tailnet names stop resolving through the
system resolver while all other DNS works.

Remedy:

```bash
sudo tailscale set --accept-dns=false && sudo tailscale set --accept-dns=true
sudo dscacheutil -flushcache; sudo killall -HUP mDNSResponder
dscacheutil -q host -a name <peer>.<tailnet>.ts.net   # not dig, which bypasses /etc/resolver
```

Durable fallback: needed peers are pinned in `/etc/hosts` declaratively, from structured `tailnet_pins`
data in `.chezmoidata/macos_system_setup.yaml`. The Tier 2 sudo runner hands one pin per line to
`~/.cargo/bin/tailnet-pin`, which converges the file to exactly one line per pin. Tailscaled never
manages `/etc/hosts`, so the entries coexist, and tailnet IPs are stable per node.

### Updates

`brew upgrade` updates the user-owned formula (no extension re-approval needed), but the running daemon
is a separate root-owned copy a formula upgrade does not touch. After upgrading the `tailscale` formula,
re-run `sudo /opt/homebrew/opt/tailscale/bin/tailscaled install-system-daemon` to refresh the daemon
copy. On dresden `sudo` is passwordless (the operator's `!authenticate` sudoers config, not managed by
this repo), so the re-copy is a single command; on a fresh machine expect a password prompt.

### Daemon-host role

When an always-home Mac exists and takes over the daemon-host role, dresden (which is carried) cuts back
to the GUI `tailscale-app` cask for better roaming DNS, and the always-home Mac runs this daemon. Make
the chezmoi config machine-conditional then.
