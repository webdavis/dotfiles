# Local daemons: atuin, happy, tailscaled, the hermes gateway

Four long-running services on dresden, each with its own failure mode and diagnostic ladder. The first
three are chezmoi-tracked LaunchAgents: the plists live under `Library/LaunchAgents/` and the loaders
that bootstrap them are `.chezmoiscripts/run_onchange_after_*` scripts keyed on the plist's own hash, so
a loader re-runs when its plist changes rather than on every apply. The hermes gateway is not a
LaunchAgent; `hermes gateway` owns its lifecycle and this repository owns only its configuration.

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

## Happy daemon (remote agent control)

[happy](https://happy.engineering/) bridges Claude Code sessions to the Happy mobile and web apps for
remote control; the local daemon is that bridge. Its lifecycle is managed by
`~/Library/LaunchAgents/com.webdavis.happy-daemon.plist` (`KeepAlive=true`, `RunAtLoad=true`), loaded by
`.chezmoiscripts/run_onchange_after_62-load-happy-daemon-launchagent.sh.tmpl` (`bootout` plus `bootstrap`
with a 3-try retry loop, mirroring the atuin loader). `happy` itself is an npm global tracked under
`npm:` in `.chezmoidata/system_packages_autoinstall.yaml`, and logs go to
`~/.local/log/happy-daemon.log`.

**The one gotcha: use `start-sync`, not `start`.** The plist runs `happy daemon start-sync`, which keeps
the daemon in the foreground. The documented command, `happy daemon start`, detaches (forks, then
returns), which under `KeepAlive` looks like an instant exit and restart-loops, orphaning a daemon each
cycle. `start-sync` is the foreground entry point that `start` spawns internally, and happy ships no
documented `--foreground` flag, so the plist comment is where the reason is recorded. launchd then
supervises a two-process tree: the `start-sync` process it keeps alive, which in turn manages the real
daemon.

**Diagnostic ladder** when remote control stops connecting:

```bash
happy daemon status                        # 'Daemon is running' + PID, port, version
launchctl list | grep happy                # col 1 = live PID, col 2 = last exit status
ps aux | grep '[h]appy daemon'             # supervised start-sync process + the daemon it spawns
tail ~/.local/log/happy-daemon.log         # crash messages
happy doctor                               # full diagnostics ('happy doctor clean' kills runaways)
```

## Hermes gateway (webhook routes)

The gateway is the hermes agent's webhook platform, switched on by `WEBHOOK_ENABLED` and `WEBHOOK_PORT`
in `~/.hermes/.env` (rendered from `private_dot_hermes/private_dot_env.tmpl`). It listens on
`127.0.0.1:8644` and every notification this machine sends to Discord arrives as a signed POST to
`http://127.0.0.1:8644/webhooks/<route>`.

### The routes

Six routes, and the template that owns them declares exactly these. A live route it does not name is
REMOVED by the next apply, which is how `unattended-upgrades` (superseded by `uu`) and `osquery`
(superseded by `posture`) leave.

| Route       | Who posts                 | What                                                                                                       |
| ----------- | ------------------------- | ---------------------------------------------------------------------------------------------------------- |
| `general`   | nothing in this repo yet  | The catch-all channel. Declared so an ad-hoc POST has a signed route of its own rather than borrowing one. |
| `pns`       | pns hook and daemon paths | Every routine agent event. The default route when nothing names one.                                       |
| `priority`  | the alert drainer         | Machine health and security ONLY (operator ruling 2026-09-14). Posture is held off it by `severity_route`. |
| `uu`        | uu                        | The weekly unattended-upgrades record. Renamed from `unattended-upgrades`.                                 |
| `posture`   | posture                   | Every page it raises, including the critical ones, plus the daily digest, the heartbeat, poll and funnel.  |
| `pns-recap` | pns                       | The return recap.                                                                                          |

Route names are not URLs: a producer names a route and the gateway's own table decides where it lands.
posture picks its route from the finding's tier in one place (`severity_route`,
`posture/crates/posture-domain/src/severity.rs`), and that one place holds EVERY tier on `posture`,
critical included, so `priority` is held out of posture's tier map by posture rather than by anything the
gateway does; uu's default is `DEFAULT_RECORD_URL` (`uu/crates/uu-adapters/src/config/records.rs`); pns's
recap route is `RECAP_ROUTE` (`pns/crates/pns-domain/src/routes.rs`). pns keeps a ROSTER of the routes it
posts to, `pns_domain::routes::ROUTES`, because each one is signed with its own key and a route with no
key is a post pns refuses rather than signs with somebody else's. So adding a route pns posts to is two
registrations, the gateway and that roster, and a route only uu or posture posts to needs the gateway
alone.

### Where they live, and what a route carries

`~/.hermes/config.yaml` is a chezmoi **modify-template**,
`private_dot_hermes/modify_private_config.yaml.tmpl`, and it owns exactly two things:
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

Every route carries four things: a `secret`, `deliver: discord`, `deliver_only: true` so the body is
posted verbatim instead of being fed to an agent, and a `deliver_extra.chat_id` naming its channel.
`general`, `pns` and `priority` share the three-line prompt (`**{header}**`, `-# {subheader}`, `{body}`);
`posture`, `pns-recap` and `uu` keep the event-shaped prompt they already had.

**Every secret and every channel id comes from KeePassXC by entry name, two entries per route.** Twelve
entries, named off the route:

| Route       | Secret entry                            | Channel entry                                |
| ----------- | --------------------------------------- | -------------------------------------------- |
| `general`   | `Hermes :: Webhook Secret (#general)`   | `Discord (Uriel) :: Channel ID (#general)`   |
| `pns`       | `Hermes :: Webhook Secret (#pns)`       | `Discord (Uriel) :: Channel ID (#pns)`       |
| `pns-recap` | `Hermes :: Webhook Secret (#pns-recap)` | `Discord (Uriel) :: Channel ID (#pns-recap)` |
| `posture`   | `Hermes :: Webhook Secret (#posture)`   | `Discord (Uriel) :: Channel ID (#posture)`   |
| `priority`  | `Hermes :: Webhook Secret (#priority)`  | `Discord (Uriel) :: Channel ID (#priority)`  |
| `uu`        | `Hermes :: Webhook Secret (#uu)`        | `Discord (Uriel) :: Channel ID (#uu)`        |

The parentheses in the secret titles are load-bearing: the retired single shared key lived at
`Hermes :: Webhook Secret :: #pns`, which is a different entry. The ElevenLabs voice is one more lookup,
`ElevenLabs :: Voice ID`, whose Password field holds the voice id.

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

| Route       | Sender and the key it reads                                                    |
| ----------- | ------------------------------------------------------------------------------ |
| `pns`       | `[plugins.hermes.keys] pns` in `dot_config/pns/config-values.toml`             |
| `pns-recap` | `[plugins.hermes.keys] pns-recap`                                              |
| `posture`   | `[plugins.hermes.keys] posture` (posture pipes its pages through `pns submit`) |
| `priority`  | `[plugins.hermes.keys] priority`                                               |
| `uu`        | `[records] key` in `dot_config/uu/private_config.toml.tmpl`                    |
| `general`   | no sender in this repository; an ad-hoc signed POST                            |

pns refuses to post to a route its table names no key for, and records the refusal the way every other
refused hermes post is recorded; `pns doctor` names every route left without a key. Until the apply that
carries both sides has run, the senders still sign with the retired shared key and the gateway answers
401 on five of the six routes, so apply once, with KeePassXC unlocked, then `hermes gateway restart`.

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
had to carry the ONE secret pns signs with, and `run_after_68` compared the others against the `pns`
route's value. Six separate vault entries leave nothing to compare a route against: agreement is now
between the entry a route renders from and the key its sender holds, and neither of those is in
`config.yaml`. What the check still sees is PRESENCE, by shape and never by value, which is why it
reports a route with no secret and a `chat_id` that is not a 17-to-20-digit Discord snowflake.

What that leaves. Nothing signs with the old key any more. The alert-drainer LaunchAgent and
`drain-undelivered-alerts.sh` retired once their queue read empty (`select count(*) from pending_alerts`
was 0) and every osquery agent ran a `posture` subcommand, so nothing writes that store and nothing reads
that key. `~/.config/osquery/webhook-secret` is dead weight, and it is watched by the
agent-attack-surface pack, so trash it alongside the LaunchAgent and the queue rather than on its own.
`severity_route` still holds EVERY posture tier on `posture`, critical included; flipping its critical
arm back to `priority` is now a one-line change in posture's own workspace, and the test named for the
hold is what makes that flip deliberate.

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
`general`, `pns`, `pns-recap`, `posture`, `priority`, `uu`. What to expect from the apply itself:
`run_after_68` prints NOTHING when all six routes are present with a secret, a snowflake `chat_id` and
`deliver_only: true`, no undeclared route is left, `tts.elevenlabs.voice_id` is set, and the gateway
already answers each route. Every line it does print names the route, the condition, and the KeePassXC
entry or command that fixes it, and never a value.

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
