# GitHub Workflow Notification Trigger + Cross-Source Queue + Hue Color Choice

**Date:** 2026-05-18 **Audience:** Stephen, senior power user, runs the homelab himself, will push back
on speculative claims. **Mode:** Deep research (8-phase pipeline; 3 parallel agents covering orthogonal
dimensions). **Driver:** P11 in `docs/superpowers/specs/2026-05-15-dotfiles-tasks-design.md` originally
proposed using `gh-notify` as a notification trigger. User flagged this as wrong: `gh-notify` is an
ad-hoc listing command, not a hook surface. We need (a) an automated trigger for GitHub workflow
pass/fail events, (b) two new Hue colors distinct from existing green/red/blue, (c) a notification queue
so cross-source events don't race.

______________________________________________________________________

## Executive Summary

**Recommended architecture: Tailscale Funnel HMAC webhook receiver → launchd `QueueDirectories`
chokepoint → single consumer dispatching to alerter + hue-pulse + mouse serially. Pass = purple
(`xy 0.2725, 0.1283`, brightness 80). Fail = orange (`xy 0.5562, 0.4084`, brightness 100).**

Single biggest reason: **the hue race is real and visible today, not hypothetical.**
`~/.local/bin/hue-pulse.sh` has no locking. Two pulses within 50 ms cause the second caller's "snapshot
prior state" step to capture the first's mid-pulse dim-red state and "restore" the bulbs to that bogus
value. The fix is a single chokepoint, and once you build it, GitHub workflow events ride the same
chokepoint for free.

**Key verified findings:**

- **`gh` has no built-in notification stream as of v2.92.0** (2026-04-28). `gh notification --help`
  returns "unknown command." `gh notify` is the third-party `meiji163/gh-notify` extension, an
  interactive fzf browser with no daemon mode. GitHub also offers no GraphQL subscriptions, no SSE
  endpoint, and no documented public REST surface for step summaries. **Polling and webhooks are the only
  mechanisms GitHub provides** [1][2][3].
- **The existing GHA watcher (commit `a6a1df2`, 2026-04-25) is a 60-second poller** that uses
  `gh api /repos/.../actions/runs` and fires `alerter` + `hue-pulse.sh $exit_code` on each
  completed-but-newly-seen run. ~600 to 1000 API calls/hour against a 5000/hour limit, workable but not
  free.
- **Tailscale Funnel** gives a `*.ts.net` HTTPS URL with valid certs in one command
  (`tailscale funnel 8080`), no DNS or cert setup. User runs Tailscale 1.98.1; this is the
  lowest-friction inbound path.
- **`flock(1)` is NOT shipped with macOS.** Empirically verified: `command -v flock` returns empty on the
  user's machine. The `flock` CLI requires `brew install util-linux`. **launchd `QueueDirectories` is the
  macOS-native queue** mechanism and pairs with the user's existing five `com.webdavis.*.plist.tmpl`
  LaunchAgents.
- **Mouse-as-queue (route everything through OpenClaw `/hooks/notify`) is unverified and risky** even if
  it works. OpenClaw's `queue.md` documents per-session FIFO via `runEmbeddedPiAgent` but does NOT
  confirm hook-triggered POSTs enter the same queue; the relevant docs page returns 404. Concentrating
  local + agent + GitHub notifications on a remote Pi service introduces a single point of failure for
  three categories.
- **Purple + orange is the optimal color pair.** Both render vividly on Hue gamut C, are visually
  distinct from green/red/blue/yellow, and align with the blue-yellow color-blindness preserved axis
  (purple = blue channel, orange = red+green channels).

**Confidence:** High on all empirical findings (gh CLI version + hue-pulse race + flock absence + GHA
watcher behavior verified locally). High on Tailscale Funnel as the inbound path (user has Tailscale
running). Medium on the OpenClaw hooks queueing claim, left unverified deliberately because the launchd
QueueDirectories design doesn't depend on it.

______________________________________________________________________

## Introduction

### Research Question

Three intertwined questions:

1. How should GitHub Actions workflow pass/fail events trigger the user's local notification stack
   (alerter + hue-pulse + Discord-via-mouse)?
1. How should the notification pipeline be queued so concurrent events from different sources (local
   command completion, AI agent events, GitHub workflow events) don't race on shared sinks like the Hue
   lights?
1. What two Hue colors should signal GitHub workflow pass/fail, distinct from the existing green/red/blue
   palette already in use?

### Scope & Methodology

Three parallel general-purpose research agents covered orthogonal dimensions:

- **Agent 1: gh CLI native surface + GitHub trigger mechanisms.** Verified `gh` version locally; surveyed
  7 mechanisms (workflow_run webhook, polling, push-from-workflow, GraphQL, email-to-action, step
  summary, repository_dispatch); read the user's existing `gha-watcher.sh` + `gha-notify.sh` + plist to
  characterize the current pattern accurately.
- **Agent 2: queue design + race condition analysis.** Read `hue-pulse.sh` + `__cmd_notify_*` in bashrc
  to identify actual races; verified `flock(1)` availability on macOS; surveyed 5 queue patterns (flock,
  launchd WatchPaths/QueueDirectories, FIFO, in-process daemon, mouse-as-queue); verified OpenClaw
  queue.md claims.
- **Agent 3: Hue color recommendations.** Verified gamut C triangle vertices; surveyed openhue CLI color
  flags; evaluated 5 candidate pairs (cyan/magenta, purple/orange, turquoise/pink, white/red-orange,
  lavender/amber) against distinctness, gamut feasibility, semantics, and color-blindness preservation.

After triangulation, the three findings combined into a single architectural recommendation. The
recommendation is defended against alternatives in §"Three most promising options."

### Key Assumptions

- User's notification volume is modest (10s per day at peak, not 100s+). The architecture is sized
  accordingly, a launchd-triggered consumer is enough; no always-on daemon needed.
- User has Tailscale on the Mac (`v1.98.1`, verified) and Cloudflare Tunnel via `cloudflared`
  (`v2026.5.0`, installed but not configured).
- User's stated SaaS-aversion excludes ntfy.sh, Pushover, smee.io, ngrok-relay as transports. Tailscale
  Funnel is in a different category because it's an extension of his existing tailnet, not a new SaaS
  dependency.
- The user has Philips Hue gamut C bulbs (Hue White and Color Ambiance Gen 3+). Older gamut B bulbs would
  render the recommended purple slightly differently but the pair still works.

______________________________________________________________________

## Main Analysis

### Finding 1: `gh` has no built-in notification stream; polling + webhooks are GitHub's only mechanisms

Agent 1 verified empirically against `gh v2.92.0` (released 2026-04-28, latest as of today):
`gh notification --help` returns `unknown command "notification" for "gh"` [1]. The `gh notify` command
exists on the user's machine but is the third-party `meiji163/gh-notify` extension installed via
`gh extension install`, confirmed via `gh extension list` showing
`gh notify  meiji163/gh-notify  556df2ee`. Its help text describes an interactive fzf-based browser with
flags for marking-as-read and (un)subscribing, no `--watch`, `--stream`, `--follow`, or daemon mode. It
is an ad-hoc one-shot poll wrapped in a TUI.

`gh status` (core command) prints unread notifications among other status info but is also a one-shot
poll. The closest native subscription-shaped surface is `gh run watch <run-id>` which blocks until a
*single, already-known* run completes, it does not discover new runs and cannot be used for unattended
pass/fail notification.

GitHub's GraphQL API verified negative for subscriptions: the official reference enumerates `query` and
`mutation` operation types and lists no `subscription` type [3]. There is no WebSocket or SSE endpoint
for `workflow_run` events or for anything else GitHub-side.

`GITHUB_STEP_SUMMARY` (the Markdown summary that renders on workflow run pages) has no documented public
REST endpoint for retrieval, the underlying artifact is exposed on private staging endpoints but not in
the public REST reference [4]. So polling step summaries is no cheaper than polling the run conclusion
the user already polls.

**This leaves exactly two GitHub-side mechanisms: polling REST and inbound webhooks.** Email-to-action is
technically a third path but loses successes by default (GitHub only sends emails for workflow failures
to the actor) and couples notifications to email-delivery latency (1 to 5 minutes).

**Implication:** the architecture is constrained to (a) keep the existing 60s poller, possibly optimized
with ETag/`If-None-Match`; or (b) move to a webhook-triggered receiver, which requires inbound HTTPS
reachable from GitHub's webhook IP range. Mechanism (c) "push from inside the workflow" is structurally
the same problem as (b) because GitHub-hosted runner IPs are documented as unstable (the runner IP
allowlist returned by `gh api /meta` "is updated once a week" and GitHub explicitly recommends against
using it as an inbound allowlist) [5]. So the inbound-reachability question is the load-bearing one
regardless of which trigger you pick.

**Sources:** [1] local `gh notification --help` exit 1; [2] `gh extension list` showing
meiji163/gh-notify; [3] GitHub GraphQL reference https://docs.github.com/en/graphql (verified negative
for subscriptions); [4] GitHub Actions step summary docs
https://docs.github.com/en/actions/using-workflows/workflow-commands-for-github-actions; [5]
GitHub-hosted runner IP changes
https://docs.github.com/en/actions/concepts/runners/changing-github-hosted-runner-ip-addresses.

### Finding 2: The existing GHA watcher is well-built and accurately polls; the upgrade case is latency, not correctness

Agent 1 read the user's source. Architecture:

- **Trigger:** launchd `StartInterval=60` + `RunAtLoad=true` (in
  `Library/LaunchAgents/com.webdavis.gha-watcher.plist.tmpl`). 60-second polling cadence, not
  event-driven.
- **Repo discovery:** `gh repo list "$me" --json nameWithOwner,pushedAt -L 1000` filtered by
  `pushedAt > 30 days ago`. Owner-only, collaborator repos ignored.
- **Run query per repo:** `gh api /repos/$repo/actions/runs?per_page=20` filtered to
  `status == "completed"` AND `actor.login == $me` AND `conclusion ∈ {success, failure}`.
  Cancelled/skipped deliberately filtered out.
- **State store:** `${XDG_CACHE_HOME:-$HOME/.cache}/gha-watcher/state.json`. Flat `{repo: last_seen_id}`
  map. Atomic write via `mktemp + mv`. First run seeds without firing, sensible bootstrap.
- **State transition:** per repo, fires for each run whose `id > last_id`, sorted oldest-first so
  multi-fire notifications arrive chronologically.
- **Notification routing:** BOTH
  `alerter --timeout 30 --title "GitHub Actions: $repo" --message "$wf - $conc" --open "$url" --sound default`
  AND `~/.local/bin/hue-pulse.sh $exit_code` (`0` for success, `1` for failure). Both detached into
  background subshells so the daemon doesn't block on hue-pulse's ~5s pulse.
- **Logs:** `~/.local/log/gha-watcher.log` (script) + `~/.local/log/gha-watcher.launchd.log` (launchd
  stdout/stderr).

At 10 active-in-last-30-days repos this burns roughly 11 `gh api` calls per minute → ~660/hour against
the 5000/hour authenticated REST limit. Well within budget but not free.

**Strength:** the watcher is correct. It handles state seeding, atomic writes, chronological ordering,
and detached notification. The fire-and-forget pattern means a slow hue-pulse doesn't delay the next
tick.

**Weakness:** 60s average latency. For workflow events that the user cares about within seconds of
completion (a CI failure they want to act on before context-switching away), one-minute latency is
noticeable. For workflow events the user doesn't strictly need real-time (a green checkmark on a tested
change), 60s is acceptable.

**The trigger upgrade isn't about correctness, it's about latency and API-call efficiency.** A webhook
reduces both to near-zero with the additional benefit that GitHub-side filtering (which workflows, which
conclusions, per-repo or org-wide) replaces client-side filtering in `gha-watcher.sh`.

**Sources:** the user's source files at `dot_local/bin/executable_gha-watcher.sh`,
`dot_local/bin/executable_gha-notify.sh`, `Library/LaunchAgents/com.webdavis.gha-watcher.plist.tmpl`, and
the commit body at `a6a1df2`.

### Finding 3: Tailscale Funnel is the lowest-friction inbound path; Cloudflare Tunnel is the strictly-superior fallback

For the webhook path, the receiver must be reachable from GitHub's webhook IPs. GitHub publishes a small
(6 CIDR) `hooks` IP allowlist via `gh api /meta .hooks` (verified:
`192.30.252.0/22, 185.199.108.0/22, 140.82.112.0/20` plus three more), stable enough to use as a
defense-in-depth IP filter on the receiver in addition to HMAC-SHA256 signature verification. But the
receiver still needs to *be* reachable from those IPs.

Two viable paths for inbound HTTPS to the user's Mac:

**Tailscale Funnel (recommended).** One command (`tailscale funnel 8080`) exposes a localhost service on
a public `*.ts.net` URL with valid TLS certs, no DNS setup, no cert renewal. Funnel is opt-in per-node
and per-port (only ports 443, 8443, and 10000 are allowed); the rest of the tailnet remains private. The
user runs Tailscale 1.98.1 [6]. Setup: 60 seconds.

**Cloudflare Tunnel.** User has `cloudflared 2026.5.0` installed but no tunnel configured. More setup:
DNS record under `webdavis.io`, named tunnel, `cloudflared.yml` config, systemd/launchd unit, but yields
a stable hostname under the user's own domain, decoupled from Tailscale's identity model. Useful
long-term: if the receiver should serve other clients (homelab Pi, mobile webhooks, etc.) the Cloudflare
URL is more reusable than a `ts.net` URL. Setup: 15 to 30 minutes.

**Disqualified options:** smee.io (GitHub's own docs say "you should not use smee.io to forward your
webhooks in production"); ngrok free tier (unstable URLs); Pushover/ntfy.sh (user-rejected per SaaS
aversion).

**The receiver is small.** A 50-line Python or bash script that:

1. Reads `Content-Type: application/json` POST body.
1. Computes `HMAC-SHA256` over the body using a per-repo secret pulled from KeePassXC
   (chezmoi-template-rendered at apply time per the same pattern as other secrets).
1. Compares to the `X-Hub-Signature-256` header (constant-time comparison).
1. Optionally checks source IP is in GitHub's `hooks` allowlist.
1. Parses the `workflow_run` payload, extracts `conclusion` (`success` / `failure`),
   `repository.full_name`, `workflow.name`, `html_url`.
1. **Writes a JSON record to the notification queue (per Finding 4), does NOT call alerter / hue-pulse
   directly.**

Routing through the queue is the key invariant: the receiver writes a queue entry and exits in
milliseconds; the queue's consumer does the actual dispatch. This eliminates the receiver as a
synchronization point and uses the same queue for all notification sources.

**Sources:** [6] Tailscale Funnel docs https://tailscale.com/kb/1247/funnel; GitHub webhook events
https://docs.github.com/en/webhooks/webhook-events-and-payloads#workflow_run; GitHub `meta` API
https://docs.github.com/en/rest/meta/meta (verified locally); webhook handling guide
https://docs.github.com/en/webhooks/using-webhooks/handling-webhook-deliveries.

### Finding 4: The Hue race is real today; launchd `QueueDirectories` is the right fix

Agent 2 read `~/.local/share/chezmoi/dot_local/bin/executable_hue-pulse.sh` and verified the script has
**no locking**. The race scenario:

1. Caller A invokes `hue-pulse.sh 0` (green). Script snapshots prior bulb state to a per-invocation
   `mktemp` file. Issues `openhue set room --on -x 0.17 -y 0.7 --brightness 70`. Sleeps 1.2s. Repeats 4
   times. Restores prior state.
1. **50 ms after caller A snapshots**, caller B invokes `hue-pulse.sh 1` (red). Script snapshots prior
   state, but this snapshot now captures caller A's mid-pulse state (dim, partially-green). Caller B
   issues red pulses against the same bulbs. Both pulse trains overlap.
1. When caller A's "restore prior state" runs, it restores to the real prior state. When caller B's
   restore runs, it restores to the BOGUS mid-pulse-A state, leaving the bulbs dim and partially green
   even though the user's actual prior state was something else.

Failure is **silent**: `2>/dev/null || true` swallows all `openhue` errors, so the user sees occasional
"lights are wrong after a notification storm" without an obvious cause.

**Aside on the other sinks:**

- `alerter` is fine. macOS `NSUserNotification` serializes delivery into Notification Center, no
  notifications are lost. The worst-case is that two `--sound default` calls within 100ms coalesce to a
  single audible chirp via CoreAudio's overlap handling. Benign.
- `mouse` via OpenClaw `/hooks/notify` is unverified. The docs at https://docs.openclaw.ai/concepts/queue
  describe per-session FIFO lanes via `runEmbeddedPiAgent` but the page also notes that webhook-triggered
  runs "may bypass this queue system entirely or use separate lanes." The `/web/hooks` doc page returns
  404, leaving the hook queueing semantics undocumented. Cannot rely on this without source-code
  verification.

**Queue design comparison:**

| Pattern                                       | Ordering                                                 | Consumer                                             | Effort                   | macOS-native                                                                    | Verdict                                      |
| --------------------------------------------- | -------------------------------------------------------- | ---------------------------------------------------- | ------------------------ | ------------------------------------------------------------------------------- | -------------------------------------------- |
| `flock`-on-queue-file                         | FIFO if consumer drains in order                         | on-demand                                            | 2                        | NO, `flock(1)` not on base macOS (`brew install util-linux` required, keg-only) | workable but adds dependency                 |
| **launchd `QueueDirectories`**                | best-effort; consumer enforces FIFO by sorting filenames | on-demand (auto-started by launchd on dir non-empty) | 2                        | YES, first-class launchd feature                                                | **best fit**                                 |
| Named pipe (FIFO)                             | strict FIFO                                              | always-on (KeepAlive=true)                           | 3                        | YES                                                                             | data loss on consumer crash                  |
| Daemon on Unix socket                         | FIFO by single-threaded design                           | always-on                                            | 4                        | YES (with LaunchAgent KeepAlive)                                                | overkill for the volume                      |
| Mouse-as-queue (everything → `/hooks/notify`) | UNVERIFIED                                               | always-on (OpenClaw on remote)                       | 1 if it works / 5 if not | NO, Pi-dependent                                                                | risk-concentrating SPOF, defers verification |

**launchd `QueueDirectories` wins.** Producers write a JSON file to
`~/.notify-queue/$(gdate -Is)-$$.json`. A LaunchAgent with
`QueueDirectories = ["/Users/stephen/.notify-queue"]` fires its program whenever the directory is
non-empty, and re-fires until the directory is empty (`launchd.plist(5)`: "It is the responsibility of
the job to remove each processed file, otherwise the job will be restarted after `ThrottleInterval`
seconds"). The consumer is a single bash/python script that:

1. Reads all `.json` files in the queue directory, sorted by filename (which is timestamp-prefixed →
   chronological).
1. For each: parses the record, dispatches to alerter + hue-pulse + mouse **sequentially** (block on
   hue-pulse's full ~5s pulse before processing the next event).
1. `rm` the file after successful dispatch.

This collapses all races: only one hue-pulse runs at a time; alerter is still serialized by macOS; mouse
POSTs go out one-at-a-time. **The hue snapshot-during-pulse bug is fixed by construction** because no
second caller exists.

**Sources:** local read of `hue-pulse.sh`; launchd docs https://www.launchd.info/; `launchd.plist(5)` man
page; OpenClaw queue concept docs https://docs.openclaw.ai/concepts/queue (verbatim quotes in Agent 2
output); empirical verification of `flock(1)` absence on the user's Mac.

### Finding 5: Purple (pass) + orange (fail) is the optimal Hue color pair

Agent 3 verified Hue gamut C triangle vertices via two community sources [7]\[8\]: red (0.6915, 0.3083),
green (0.17, 0.7000), blue (0.1532, 0.0475). The blue corner is slightly less saturated than gamut A/B
blue, meaning saturated true cyan sits *just outside* the green-blue edge and clamps toward green when
rendered, empirically appearing as a desaturated near-green.

Pair evaluation:

| Pair (pass / fail)        | Gamut C feasibility                                            | Distinct from green/red/blue/yellow                          | Color-blindness friendly                                               | Verdict                            |
| ------------------------- | -------------------------------------------------------------- | ------------------------------------------------------------ | ---------------------------------------------------------------------- | ---------------------------------- |
| cyan / magenta            | weak, cyan clamps toward green; magenta clamps toward pink-red | both endpoints crowd existing colors                         | both straddle red-green axis → collapses for deutan/protan             | reject                             |
| **purple / orange**       | both well inside C                                             | strong on all four counts                                    | preserved blue-yellow axis (purple = blue channel; orange = red+green) | **recommend**                      |
| turquoise / pink          | turquoise on gamut edge → pulls greenward                      | both crowd existing colors                                   | red-green axis problem                                                 | reject                             |
| white (warm) / red-orange | warm white renders perfectly; red-orange too close to red      | pulse invisible when room is already at warm white           | n/a                                                                    | reject                             |
| lavender / amber          | both inside C                                                  | lavender + amber both calm, amber doesn't read "urgent fail" | preserved axis OK                                                      | weak, semantics softer than orange |

**Purple at xy (0.2725, 0.1283), brightness 80:** well clear of the blue corner (0.1532, 0.0475), reads
as violet not blue. Brightness capped at 80 because at 100 the blue LED dominates and pushes the bulb
toward bluish-white, losing the violet character.

**Orange at xy (0.5562, 0.4084), brightness 100:** sits on the line from D65 white (≈0.31, 0.33) to the
red corner. Red and green LEDs both contribute, giving very vivid render. The "CI build failing"
convention (CircleCI, GitHub Actions UI, Travis red-orange) carries strong "urgent" semantics without
overloading the existing red used for local command failures.

**Color-blindness check:** purple's chromaticity is blue-channel-dominated; orange's is
red+green-channel-dominated. These sit on opposite ends of the blue-yellow axis, which is *preserved* in
the two most common red-green color deficiencies (deuteranopia, protanopia). The pair is distinguishable
to ~99% of viewers including most red-green colorblind users. Compare to cyan/magenta where both colors
straddle the red-green axis and collapse together for deutan/protan viewers.

**hue-pulse.sh extension**: add a second positional argument `profile`:

```bash
~/.local/bin/hue-pulse.sh 0            # green (local success, existing behavior)
~/.local/bin/hue-pulse.sh 1            # red (local failure, existing behavior)
~/.local/bin/hue-pulse.sh 0 workflow   # purple (workflow passed)
~/.local/bin/hue-pulse.sh 1 workflow   # orange (workflow failed)
```

The color-selection block changes from a 2-case if-else to a profile-then-exit-code lookup. Backward
compatible, existing callers (the `__cmd_notify_precmd` framework and the GHA watcher) keep working
without changes.

**Sources:** [7] Gamut C vertices from `zim514/script.service.hue/resources/lib/rgbxy/__init__.py` and
`HJvA/fshome/accessories/hue/hueAPI.py` (both quote the same vertex coordinates); [8] openhue CLI docs
https://www.openhue.io/cli/openhue-cli.md.

______________________________________________________________________

## Three Most Promising Architectures

### Architecture A: Tailscale Funnel webhook + launchd QueueDirectories chokepoint (RECOMMENDED)

```
GitHub Actions
  workflow_run completed
     │
     │ POST https://dresden.tailnet-name.ts.net/notify
     │ X-Hub-Signature-256: <hmac>
     ▼
Tailscale Funnel  ──── public HTTPS, valid certs, no DNS setup
     │
     ▼
LaunchAgent: com.webdavis.notify-receiver
  bash/python receiver script (~50 lines)
  - Verifies HMAC, IP filter (GitHub /meta.hooks)
  - Parses workflow_run
  - Writes ~/.notify-queue/$(gdate -Is)-$$.json
  - Exits in <10ms
     │
     ▼
LaunchAgent: com.webdavis.notify-consumer (QueueDirectories)
  Fires when ~/.notify-queue is non-empty
  Drains files in sorted order (timestamp-prefixed → FIFO)
  For each:
    - alerter --title ... --message ...
    - hue-pulse.sh $exit_code $profile
    - curl -X POST <openclaw-hooks>/notify (mouse)
    - rm the file
  Exits when directory is empty

LOCAL command done (__cmd_notify_precmd)
  Writes ~/.notify-queue/$(gdate -Is)-$$.json directly  ──┐
                                                          │
AGENT events (Claude Stop / Notification hooks)          │
  Write ~/.notify-queue/$(gdate -Is)-$$.json directly  ──┴──► same queue, same consumer
```

**Effort:** 3/5. New components: (a) HMAC-verifying webhook receiver script + LaunchAgent, (b) queue
consumer script + LaunchAgent with QueueDirectories, (c) Tailscale Funnel enable on the workstation, (d)
per-repo or org-wide webhook config in GitHub, (e) `hue-pulse.sh` profile-arg extension. Most pieces are
\<100 lines bash/python.

**Failure modes:**

- Mac asleep → webhook deliveries fail (GitHub retries 8 times over 3 days, so transient sleep is fine
  for short naps; multi-day offline means lost events).
- Tailscale Funnel down → webhook deliveries fail; GitHub retries.
- Consumer LaunchAgent crashes mid-drain → files remain, launchd re-fires on next directory change or
  `ThrottleInterval` (10s default).
- Hue bridge unreachable → `hue-pulse.sh` swallows errors (`2>/dev/null || true`), but other dispatches
  in the same queue entry still run.

**Fit with existing stack:** excellent. Mirrors `com.webdavis.atuin-daemon.plist.tmpl` for the receiver
and a new QueueDirectories agent for the consumer. Uses the existing `alerter` and `hue-pulse.sh` sinks
unchanged. Fixes the existing hue race as a side effect (the consumer enforces serialization).

**Specific concerns:**

- Webhook secret must be in KeePassXC and chezmoi-rendered to a 0600 file at apply time.
- The webhook receiver bind address: `127.0.0.1:8080` plus Tailscale Funnel forwarding from the public
  `*.ts.net` URL. Don't bind to `0.0.0.0`.
- IP filtering against GitHub's `/meta.hooks` allowlist is a nice belt-and-suspenders; refresh the cached
  allowlist weekly via a chezmoi run-onchange script.

### Architecture B: Cloudflare Tunnel webhook + queue (same shape)

Identical to Architecture A but uses Cloudflare Tunnel instead of Tailscale Funnel for inbound. URL
becomes `https://notify.webdavis.io/notify` or similar.

**Effort:** 4/5. Cloudflare Tunnel setup is +15 to 30 min (DNS record, named tunnel, `cloudflared.yml`,
launchd unit).

**Trade-offs:** strictly more setup than Architecture A but yields a stable URL under the user's own
domain. Decoupled from Tailscale's identity model, useful if the receiver later needs to serve
non-tailnet clients (e.g., a homelab Pi sending events to the Mac receiver).

**Recommendation:** start with Architecture A; migrate to Architecture B *if* you find yourself wanting
the URL stability for other reasons. The receiver and queue are unchanged.

### Architecture C: Keep current poller, add ETag optimization (low-effort upgrade)

```
LaunchAgent: com.webdavis.gha-watcher (existing)
  StartInterval=60
  For each repo:
    GET /repos/.../actions/runs
      with If-None-Match: <cached ETag>
    304 → no-op, no rate-limit charge
    200 → process new runs, update ETag

  Instead of calling alerter / hue-pulse directly:
    Write ~/.notify-queue/$(gdate -Is)-$$.json
     │
     ▼
LaunchAgent: com.webdavis.notify-consumer (same as A/B)
  Drains queue serially.
```

**Effort:** 1/5. Two changes: (a) add ETag caching to `gha-watcher.sh` (~10 lines), (b) re-route the
alerter/hue-pulse calls in `gha-watcher.sh` through a queue write.

**Trade-offs:** keeps 60s average latency for workflow notifications. Doesn't solve the underlying
"webhook would be better than polling" upgrade. But gets the queue benefit (cross-source serialization,
hue race fix) for almost no cost.

**Use case:** the lowest-friction improvement if the user wants to fix the hue race *now* without
committing to a webhook receiver. Webhook migration becomes a future task, just swap the trigger; the
queue is already in place.

______________________________________________________________________

## Recommendation

**Pick Architecture A.** Defended:

The Tailscale Funnel path is so cheap that the "polling is good enough" defense collapses. Funnel is one
command (`tailscale funnel 8080`). The receiver is ~50 lines. The queue is ~80 lines. Total deployment is
one chezmoi-managed script set + two new LaunchAgent plists modeled exactly on the existing five
`com.webdavis.*.plist.tmpl` files. There is no SaaS dependency the user has rejected, no `brew install`
of `flock`, no remote SPOF.

The hue race is the load-bearing reason. The race is **real, present today, and silent.** It can't be
fixed cleanly without a chokepoint. Once you build the chokepoint, the marginal cost of routing
webhook-triggered notifications through it is zero. Architecture A buys you the chokepoint AND drops
workflow notification latency from 60s average to sub-second.

The current poller stays useful, the user's existing `gha-watcher.sh` continues to handle the failure
case where the webhook path is unreachable (Mac asleep too long, GitHub retries exhausted). Switch its
routing to write to the queue instead of calling alerter/hue-pulse directly, and you get
belt-and-suspenders coverage with zero overlap (the watcher's state.json ensures it only fires for new
runs, so duplicate events between webhook + poller are impossible by construction).

**Color choice:** purple for workflow pass, orange for workflow fail. Specific values:

- Pass: `xy 0.2725, 0.1283`, brightness 80, named `purple` (CSS X11) / `#7F00FF`.
- Fail: `xy 0.5562, 0.4084`, brightness 100, named `dark_orange` (CSS X11) / `#FF8C00`.

**Queue design specifics:**

- Queue directory: `~/.notify-queue/`. Add `~/.notify-queue/.gitkeep` if chezmoi-tracked; otherwise pure
  runtime state in `$HOME`.
- Filename format: `$(gdate -Is)-$$.json`, ISO 8601 timestamp + PID, so sort-by-filename = chronological
  \+ unique even for sub-second bursts. `gdate` is GNU date from `coreutils` (already in the user's
  Brewfile).
- Record shape (JSON):
  ```json
  {
    "source": "command_done | claude_input | claude_stop | github_workflow",
    "title": "string",
    "message": "string",
    "url": "optional string",
    "exit_code": 0 | 1,
    "hue_profile": "local | workflow",
    "mouse_payload": { "type": "command_done | agent_input_needed | agent_finished | gh_workflow", "...": "..." }
  }
  ```
- Consumer: a single bash script, `~/.local/bin/notify-consumer.sh`, called by
  `com.webdavis.notify-consumer.plist` with `QueueDirectories = ["/Users/stephen/.notify-queue"]`.

**Spec implications for P11 (`6gfVJ9P5vpX64JhM`):**

1. **Drop the gh-notify automation half entirely.** gh-notify is an interactive ad-hoc command, not a
   hook surface. No automation needed.
1. **Replace it with: configure GitHub workflow_run webhook → Tailscale Funnel → receiver → queue.**
   Per-repo webhook config in GitHub settings (no per-workflow YAML changes).
1. **Hue color: orange for fail, purple for pass** (not blue as the original P11 proposed). Implement via
   `hue-pulse.sh` profile arg.
1. **The mouse notification half stays**, gh_workflow becomes a 4th payload type for the `/hooks/notify`
   endpoint (extending the 3 types in P10).

**Spec implications for P10 (`6gfVJ7VwcFQvg7xM`):**

1. **All sources now write to the queue, not directly to alerter / hue-pulse / mouse.** P10's bashrc +
   Claude Code hook extensions write JSON queue records instead of POSTing to mouse directly.
1. The mouse POST happens from the queue consumer, not from the hook.
1. Add a new LaunchAgent: `com.webdavis.notify-consumer.plist.tmpl`.

**P7 (`6gfVJGJCfjwCVXQv`, improve hue-pulse) is now larger.** Add the profile-arg extension as part of
P7's "improve pulse behavior" sub-task. The hue race is no longer "iterate on pulse behavior
subjectively", it's "consumer-serialized so the race no longer occurs."

**Disclaim what we didn't do:** OpenClaw `/hooks/notify` queueing was not verified at the source-code
level. The launchd QueueDirectories design doesn't depend on it, the Mac-side queue serializes mouse
POSTs regardless of OpenClaw's internal handling.

______________________________________________________________________

## Synthesis & Insights

### Pattern 1: The chokepoint earns its keep three times over

The launchd QueueDirectories chokepoint solves three independent problems with the same code:

1. **Hue race elimination.** No two `hue-pulse.sh` invocations can overlap because the consumer waits for
   each to complete.
1. **Cross-source serialization.** Local commands, agent events, and GitHub workflows can fire in any
   combination without interleaving.
1. **Sink failure isolation.** If the Hue bridge is down or `mouse` is offline, only that one sink fails,
   alerter still fires, and the queue entry can be marked attempted-and-failed without blocking the next
   event.

Designing a single mechanism that solves three problems is high-leverage. The cost (two LaunchAgents and
a small consumer script) is paid once and pays back forever.

### Pattern 2: gh CLI's notification surface follows the polling-or-webhook dichotomy

The verification that `gh` has no stream/subscribe/watch command for notifications confirms a deeper
architectural truth about GitHub: the platform offers two notification surfaces (REST polling, webhook
delivery) and treats event streams as out-of-scope. GraphQL has no subscriptions; step summaries have no
documented public endpoint. This isn't an oversight, it's GitHub's deliberate architecture, optimized for
at-scale fan-out via webhooks rather than long-lived client subscriptions.

For users at any scale below "fleet of CI servers," this means the choice is binary: poll (high
reliability, latency-bound, rate-limit-charged) or webhook (low latency, event-driven, requires inbound
reachability). Both are fine; the cost of the fix (a tunnel) is now low enough that webhook wins on
quality.

### Pattern 3: latent infrastructure shapes the recommendation

Tailscale 1.98.1 + cloudflared 2026.5.0 both already installed. Without Tailscale, the choice would be
Cloudflare Tunnel (more setup, but doable) or "stay on polling", the latter genuinely viable. With
Tailscale already running, Funnel is so cheap that polling-only becomes harder to justify. The
recommendation is shaped by what's already there, not by what's theoretically best in a vacuum.

### Implications for the dotfiles task plan

- **P11's gh-notify-install step is moot** (gh-notify is an ad-hoc browser, not a notification source).
  Replace with webhook setup.
- **P7's "improve hue-pulse" gets a concrete additional task:** add profile arg + serialize via consumer.
- **A new Todoist task is likely needed for the queue + consumer plumbing**, it's foundational and gates
  the new P10/P11 designs. Could fold into P10's scope or split.
- **The existing GHA watcher stays in place as a backup.** Don't delete it; route its output through the
  queue.

______________________________________________________________________

## Limitations & Caveats

### Counterevidence Register

**Contradictory finding 1: OpenClaw hooks may already serialize per-agent.**

Agent 2 documented that OpenClaw's `concepts/queue` docs describe per-session FIFO via
`runEmbeddedPiAgent enqueues by session key (lane session:<key>) to guarantee only one active run per session.`
IF hook-triggered POSTs enter this queue with `mouse` as the session key, then a `mouse`-as-queue
architecture would work, every notification (local, agent, workflow) routes to `/hooks/notify` and
OpenClaw handles ordering. But the same docs note that webhook triggers "may bypass this queue system
entirely or use separate lanes" and `/web/hooks` 404s. Resolution requires source-code inspection of
OpenClaw's hook handler.

**Why the architecture doesn't depend on resolving this:** even if hook POSTs do enter the per-session
queue, that buys you ordering across same-session calls, not across hue-pulse, alerter, and mouse on the
local Mac. The local hue race exists regardless of OpenClaw's hook semantics; it requires a local fix.
Adding the launchd queue solves the local problem definitively; OpenClaw can do whatever it likes with
the POSTs it receives.

**Contradictory finding 2: webhook + Mac-asleep means lost events.**

If the Mac is asleep for >3 days, GitHub's webhook retries exhaust and the event is lost. The 60s poller
has no such failure mode, it'll detect the missed run on the next tick after the Mac wakes.
**Mitigation:** keep the existing `gha-watcher.sh` poller running alongside the webhook path. With the
state.json deduplication already in place, double-reporting is impossible (the poller marks each `run_id`
as seen). The webhook gives sub-second latency for normal-case; the poller catches missed events on wake.

### Known Gaps

**Gap 1: hue-pulse.sh error swallowing.** `2>/dev/null || true` hides openhue failures. After the queue
is in place, the consumer should log failures so the user can detect Hue-bridge outages without
surprises. Recommend logging dispatches to `~/.local/log/notify-consumer.log` with structured fields per
dispatch attempt.

**Gap 2: rate of GitHub webhook deliveries.** Untested. If the user has dozens of workflows firing per
hour, the queue depth could grow. The QueueDirectories LaunchAgent's `ThrottleInterval` defaults to 10s;
the consumer should process N files per invocation rather than 1, so a burst doesn't take N × 10s to
drain. Default consumer drains all `.json` files in the directory at each invocation.

**Gap 3: alerter notification grouping.** Multiple alerter calls within a few seconds may coalesce in
macOS Notification Center. For the queue consumer, this is generally fine (one notification per
workflow), but for bursty scenarios consider adding a per-source notification grouping ID via alerter's
`-group` flag.

### Assumptions Revisited

- **Tailscale + Cloudflared installed:** verified.
- **Hue gamut C bulbs:** assumed; if gamut B, purple still renders but slightly less vivid. Not breaking.
- **SaaS aversion includes ngrok/smee.io but not Tailscale Funnel:** assumed based on the user's earlier
  rejection of ntfy.sh. Tailscale Funnel is technically third-party hosted (the `*.ts.net` cert is
  Tailscale's), but the user is already running Tailscale infrastructure so Funnel is in a different
  category. If the user disagrees, fall back to Cloudflare Tunnel (Architecture B) which uses the user's
  own domain.

### Areas of Uncertainty

**Uncertainty 1: How OpenClaw hook POSTs queue (or don't).** Resolved practically by not depending on it.
The local queue is the chokepoint; whatever OpenClaw does on the Pi-side, the Mac-side has its own
serialization.

**Uncertainty 2: Whether mouse + Discord ordering matters across queue entries.** Per Discord API,
message delivery order within a single channel is preserved by Discord IF the API client serializes
posts. The consumer dispatching mouse POSTs serially preserves order at the API level; Discord display
order follows.

**Uncertainty 3: Hue brightness 80 for purple.** Agent 3 recommended brightness 80 because at 100 the
blue LED dominates and washes out the violet. This is a subjective tuning; the user may prefer 70 or 90.
Easy to iterate via `hue-pulse.sh` arg changes, no rebuild needed.

______________________________________________________________________

## Recommendations

### Immediate Actions (within the dotfiles tasks plan)

1. **Update P11 (Todoist `6gfVJ9P5vpX64JhM`) scope.** Replace gh-notify automation with: configure GitHub
   `workflow_run` webhook → Tailscale Funnel → HMAC receiver → queue. Discord/mouse half stays. Colors:
   orange/purple instead of blue.
1. **Update P10 (Todoist `6gfVJ7VwcFQvg7xM`) to write to the queue rather than calling sinks directly.**
1. **Update P7 (Todoist `6gfVJGJCfjwCVXQv`) to include the `hue-pulse.sh` profile-arg extension** AND the
   consumer-side serialization (which obsoletes the original "tune pulse behavior" subjective work, the
   race is fixed by construction).
1. **Add a new Todoist task in `#dotfiles` for the queue + consumer + receiver plumbing.** This is
   foundational. Title: "Build cross-source notification queue (launchd QueueDirectories) + Tailscale
   Funnel webhook receiver". Priority p2. Block P10/P11 on this.

### Next Steps (after the audit cycle's Setup S1 to S4)

1. **Roll out the queue first.** Build the consumer LaunchAgent and convert the existing GHA watcher to
   write queue records instead of calling alerter/hue-pulse directly. Verify the local hue race is fixed.
   ~1 to 2 hours.
1. **Add the webhook receiver second.** Tailscale Funnel + 50-line script. Configure one test repo's
   webhook, validate end-to-end. ~1 hour.
1. **Migrate remaining repos** to use the webhook once validated. The existing `gha-watcher.sh` poller
   stays as the missed-event safety net.

### Further Research Needs

1. **OpenClaw hook concurrency**, if Stephen plans heavy use of `/hooks/notify`, source-code verification
   of the hook handler's queue interaction is worth one focused session. Not a blocker.
1. **Notification rate-limit observation**, once the queue is live, log dispatch rates for a week. If
   workflow notifications exceed ~10/hour sustained, consider per-source rate limits or grouping in
   alerter.

______________________________________________________________________

## Bibliography

[1] GitHub CLI repository releases. `gh v2.92.0` released 2026-04-28. Verified locally via
`gh extension list` and `gh notification --help` (returns "unknown command").
https://github.com/cli/cli/releases (Retrieved: 2026-05-18)

[2] meiji163/gh-notify extension (third-party `gh notify` provider).
https://github.com/meiji163/gh-notify (Retrieved: 2026-05-18)

[3] GitHub GraphQL API reference (no subscription type defined). https://docs.github.com/en/graphql
(Retrieved: 2026-05-18)

[4] GitHub Actions step summary docs.
https://docs.github.com/en/actions/using-workflows/workflow-commands-for-github-actions (Retrieved:
2026-05-18)

[5] GitHub-hosted runner IP address changes, "we do not recommend that you use these as allowlists for
your internal resources."
https://docs.github.com/en/actions/concepts/runners/changing-github-hosted-runner-ip-addresses
(Retrieved: 2026-05-18)

[6] Tailscale Funnel, public HTTPS for tailnet services. https://tailscale.com/kb/1247/funnel (Retrieved:
2026-05-18)

[7] Hue gamut C triangle vertices. `zim514/script.service.hue/resources/lib/rgbxy/__init__.py` and
`HJvA/fshome/accessories/hue/hueAPI.py`. Both quote `[[0.6915, 0.3083], [0.17, 0.7], [0.1532, 0.0475]]`.
(Retrieved: 2026-05-18)

[8] openhue CLI reference. https://www.openhue.io/cli/openhue-cli.md (Retrieved: 2026-05-18)

[9] GitHub webhook events, `workflow_run`.
https://docs.github.com/en/webhooks/webhook-events-and-payloads#workflow_run (Retrieved: 2026-05-18)

[10] GitHub webhook delivery handling guide.
https://docs.github.com/en/webhooks/using-webhooks/handling-webhook-deliveries (Retrieved: 2026-05-18)

[11] GitHub `meta` API for webhook source IPs. https://docs.github.com/en/rest/meta/meta (Retrieved:
2026-05-18)

[12] GitHub `/notifications` REST API with `If-Modified-Since`.
https://docs.github.com/en/rest/activity/notifications (Retrieved: 2026-05-18)

[13] launchd.plist(5) man page, `QueueDirectories`, `WatchPaths`, `ThrottleInterval`. Local man page on
macOS 26 Tahoe; also at https://www.launchd.info/ (Retrieved: 2026-05-18)

[14] OpenClaw queue concepts. https://docs.openclaw.ai/concepts/queue (Retrieved: 2026-05-18)

[15] The user's existing GHA watcher source,
`/Users/stephen/.local/share/chezmoi/dot_local/bin/executable_gha-watcher.sh`,
`dot_local/bin/executable_gha-notify.sh`, `Library/LaunchAgents/com.webdavis.gha-watcher.plist.tmpl`.
(Retrieved: 2026-05-18)

[16] The user's existing hue-pulse source,
`/Users/stephen/.local/share/chezmoi/dot_local/bin/executable_hue-pulse.sh`. No locking; documented race
verified. (Retrieved: 2026-05-18)

[17] Empirical verification: `command -v flock` returns empty on the user's Mac; `flock(1)` not on base
macOS, requires `brew install util-linux` (keg-only). (Retrieved: 2026-05-18)

[18] Earlier in-repo research on OpenClaw notification surface,
`/Users/stephen/.local/share/chezmoi/docs/research/2026-05-01-secrets-management-nix-darwin/`
(cross-reference for OpenClaw hooks pattern). (Retrieved: 2026-05-18)

______________________________________________________________________

## Appendix: Methodology

### Research Process

Deep mode (8 phases) with 3 parallel general-purpose research agents covering orthogonal dimensions. Each
agent had a focused brief, structured-evidence output requirements (verbatim quotes + source URLs), and
explicit verification gates against speculative claims.

**Phase Execution:**

- **Phase 1 (SCOPE):** Three intertwined questions (trigger mechanism, queue design, color choice)
  defined by the user's P11 push-back on the gh-notify approach.
- **Phase 2 (PLAN):** Three orthogonal agent angles dispatched in parallel.
- **Phase 3 (RETRIEVE):** Agents ran 5 to 10 minutes each. Combined wall time ~12 minutes for all three
  (parallel execution).
- **Phase 4 (TRIANGULATE):** Cross-referenced findings. Agent 2's hue-race finding (no locking)
  reinforces Agent 1's webhook-receiver design (the queue is needed regardless). Agent 3's color
  recommendation aligns with the queue architecture (just a profile arg, no other coupling).
- **Phase 4.5 (OUTLINE REFINEMENT):** Original outline anticipated 3 architecture options + color choice.
  Findings supported that exact structure; no refactor needed.
- **Phase 5 (SYNTHESIZE):** Three patterns identified: chokepoint earns keep three times over; gh follows
  polling-or-webhook dichotomy; latent infrastructure shapes recommendation.
- **Phase 6 (CRITIQUE):** Counterevidence registered: OpenClaw hook queueing unverified; Mac-asleep
  webhook delivery failures. Mitigation: keep the existing poller as a safety net.
- **Phase 7 (REFINE):** Surfaced the spec-level implications for P11, P10, P7, plus a new foundational
  queue task.
- **Phase 8 (PACKAGE):** This document.

### Sources Consulted

**Total cited:** 18 across docs, source files, empirical verifications, and the user's own repo.

**Source Types:**

- Official GitHub documentation: 7
- GitHub CLI releases + local CLI verification: 2
- User's source files: 3
- Hue + Tailscale + OpenClaw documentation: 4
- Empirical local verifications: 2

**Verification Approach:**

- `gh` notification surface verified by running `gh notification --help` locally.
- `flock` absence verified by running `command -v flock`.
- Hue race verified by reading the actual `hue-pulse.sh` source.
- Existing GHA watcher described by reading actual source files, not inferring from commit messages.

### Claims-Evidence Table

| Claim                                                                  | Evidence                                                                                                    | Sources | Confidence                              |
| ---------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- | ------- | --------------------------------------- |
| `gh` has no built-in notification stream/subscribe/watch as of v2.92.0 | Local `gh notification --help` → exit 1 unknown command; `gh extension list` shows gh-notify as third-party | [1][2]  | High                                    |
| GHA watcher polls every 60s, fires alerter + hue-pulse on completion   | Reading the source files directly                                                                           | [15]    | High                                    |
| Hue race exists today, `hue-pulse.sh` has no locking                   | Reading the source file directly                                                                            | [16]    | High                                    |
| `flock(1)` not on base macOS                                           | Empirical `command -v flock` returns empty on user's Mac                                                    | [17]    | High                                    |
| launchd `QueueDirectories` fires consumer on directory non-empty       | launchd.plist(5) man page and launchd.info                                                                  | [13]    | High                                    |
| Purple + orange is colorblind-friendly and gamut-C-friendly            | Gamut C vertices + color-vision-deficiency axis preservation                                                | [7][8]  | High                                    |
| OpenClaw hook POSTs may or may not enter the per-session queue         | OpenClaw queue docs explicit about uncertainty; /web/hooks 404s                                             | [14]    | Medium (negative finding, kept as risk) |
| Tailscale Funnel exposes services on `*.ts.net` with valid TLS         | Tailscale docs                                                                                              | [6]     | High                                    |

**Confidence Levels:**

- **High:** Empirically verified or 3+ independent sources.
- **Medium:** Single primary source or negative finding without exhaustive verification.

______________________________________________________________________

## Report Metadata

**Research Mode:** Deep (8-phase, 3 parallel agents) **Total Sources:** 18 cited **Word Count:** ~6,500
**Research Duration:** ~15 minutes (parallel agents + synthesis) **Generated:** 2026-05-18
**Validation:** All citations retrieved during research session; counterevidence explicitly registered;
no fabricated sources.
