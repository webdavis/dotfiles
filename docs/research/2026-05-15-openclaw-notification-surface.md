# OpenClaw → macOS Workstation Notification Architecture

**Date:** 2026-05-15 **Audience:** Stephen — runs the homelab himself, sole user, skeptical of
speculative claims. **Mode:** Deep research (8-phase pipeline; 3 parallel agents). **Scope decision
driver:** The dotfiles audit produced two Todoist tasks (`#dotfiles` project) that wait on this
resolution — P10 (`6gfVJ7VwcFQvg7xM` — build OpenClaw long-running-task notifications) and P11
(`6gfVJ9P5vpX64JhM` — automate gh-notify + hue-pulse blue + OpenClaw notifications). The shape of
OpenClaw's notification surface determines whether either phase is "hook into existing surface" or "build
the surface upstream first."

______________________________________________________________________

## Executive Summary

OpenClaw exposes exactly one first-class outbound notification surface today: **cron jobs configured with
`delivery.mode = "webhook"`**. The Gateway POSTs a structured `CronEvent` payload (jobId, action, status,
durationMs, summary, error, runId) to a user-configured URL with an optional
`Authorization: Bearer <cron.webhookToken>` header and an SSRF-guarded fetch with a 10-second timeout
[1][2]. There is no `/v1/events` SSE stream, no per-request `callback_url` field, no global task-complete
event bus, and no documented plugin SDK event-subscription API. Three open GitHub issues — #69186
("completion/success notification sound for agent turns"), #20237 ("WebUI notification system, cron job
management popups"), and #44925 ("Subagent completion silently lost — no retry, no notification") —
confirm community demand for completion notifications beyond the cron-webhook path [3].

**Recommended architecture: Option A — Cron-job webhook → Tailscale → Mac-side LaunchAgent webhook
receiver → existing alerter + hue-pulse stack.** Two facts make this the clear winner: (1) the user
already runs Tailscale on `dresden` (the Mac) and `mister` (iPhone), and adding the Raspberry Pi is one
curl-pipe install; (2) the cron-webhook surface is documented, first-class, and trivially fits the
existing `com.webdavis.atuin-daemon.plist` LaunchAgent pattern. Long-running agent tasks get wrapped as
cron jobs with `delivery.mode=webhook` pointing at `http://dresden:9999/notify` via MagicDNS, and a
40-line Python receiver fires `alerter` + `hue-pulse.sh` on arrival.

**Single biggest reason:** OpenClaw's cron-webhook surface is the documented, supported, first-class
notification API. Building on it requires no upstream contribution to OpenClaw, no log-tailing fragility,
and no SaaS dependency. The Tailscale piece is already 80% deployed; the remaining 20% is one install on
the Pi.

**Three options presented and ranked** at the end of this report. Option A is the recommendation. Option
B (cron-webhook → MQTT-over-Tailscale → mosquitto_sub) is reserved for events the user can't afford to
drop while asleep. Option C (JSONL log tail) is the escape hatch for events that fall outside cron's
scope, with the caveat that log-message strings are not a stable API.

**Confidence: High** on the OpenClaw surface analysis (every claim has a verbatim docs quote or
source-file reference). High on the transport analysis (existing Tailscale install was empirically
verified on disk). Medium on whether all of Stephen's intended "long-running task" use cases fit the cron
model — that's a workflow question the spec needs to settle.

______________________________________________________________________

## Introduction

### Research Question

How should the user wire OpenClaw, an always-on agent-gateway service running on a Raspberry Pi in the
user's homelab, to notify the macOS workstation when long-running tasks complete? The notification needs
to land in the existing local notification stack: `alerter` for macOS-native notifications and
`~/.local/bin/hue-pulse.sh` for Philips Hue color pulses, both already integrated via the
`__cmd_notify_*` hooks in `dot_bashrc.tmpl`.

### Scope & Methodology

Three parallel research agents covered three orthogonal dimensions:

- **Agent 1 — OpenClaw event/webhook surface deep dive.** Read https://docs.openclaw.ai/ and the public
  GitHub repo for any webhook, hook, event, observability, or plugin-lifecycle surface. Verbatim quotes
  required; speculative claims forbidden. Negative findings (e.g., "no `/v1/events` endpoint") treated as
  valuable signal.
- **Agent 2 — AI/agent gateway notification pattern survey.** Surveyed LiteLLM, Portkey, Helicone,
  Langfuse, MCP, AnythingLLM, and OpenWebUI for their task-complete notification mechanisms. Categorized
  into five patterns (A: outbound webhook; B: streaming subscription; C: per-request `callback_url`; D:
  observability sink; E: plugin/middleware).
- **Agent 3 — Pi → Mac transport options.** Evaluated HTTP webhook receiver on the Mac, Tailscale direct
  connect, MQTT, SSE/long-polling, Pushover, SSH back-channel, and Cloudflare Tunnel. Cross-referenced
  against the user's existing chezmoi-managed infrastructure to identify what's already deployed (key
  finding: Tailscale is on the Mac and iPhone, not yet on the Pi; `cloudflared` installed but no tunnel
  configured).

After triangulation, three candidate solution architectures emerged. The recommendation is defended
against the alternatives at the end.

### Key Assumptions

- The user's "long-running tasks" are agent workflows the user kicks off and walks away from — not
  interactive request/response calls where the caller holds an SSE socket open. (If they're the latter,
  the OpenResponses SSE stream's `response.completed`/`response.failed` events are sufficient and no new
  infrastructure is needed.)
- The user is willing to wrap long-running agent work in OpenClaw cron jobs. OpenClaw's cron model
  supports `--session isolated` jobs that run agents to completion, with `--message "..."` as the initial
  prompt — this maps cleanly to "fire and forget" tasks [1].
- Tailscale is acceptable infrastructure on the Pi. The user already runs it on the Mac (`v1.96.5`) and
  iPhone (`mister` on the tailnet) and added `tailscale-app` to the package autoinstall yaml on line 182,
  so adopting it on the Pi is consistent with existing tooling.
- The user has explicitly rejected `ntfy.sh` as a transport in earlier brainstorming. Pushover is in a
  different category (it leverages APNs rather than competing with the local webhook stack) but is also
  flagged as a SaaS dependency the user prefers to avoid.

______________________________________________________________________

## Main Analysis

### Finding 1: OpenClaw's only first-class outbound notification surface is the cron-job webhook

OpenClaw exposes outbound HTTP notifications via exactly one mechanism documented at
https://docs.openclaw.ai/automation/cron-jobs in the "Delivery and output" section: cron jobs configured
with `delivery: { mode: "webhook", to: "https://..." }` POST a finished-event payload to the configured
URL [1]. The payload type is `CronEvent` per `src/cron/service/state.ts`:

```typescript
export type CronEvent = {
  jobId: string;
  action: "added" | "updated" | "removed" | "started" | "finished";
  job?: CronJob;
  runAtMs?: number;
  durationMs?: number;
  status?: CronRunStatus;
  error?: string;
  summary?: string;
  diagnostics?: CronRunDiagnostics;
  delivered?: boolean;
  deliveryStatus?: CronDeliveryStatus;
  runId?: string;
  nextRunAtMs?: number;
} & CronRunTelemetry;
```

The dispatcher in `src/gateway/server-cron-notifications.ts` only fires the webhook when
`params.evt.summary` is set (i.e., the `finished` action with a result), per
`dispatchGatewayCronFinishedNotifications`. Auth is bearer-only via `cron.webhookToken` configured
globally; the docs are explicit: *"`webhookToken`: bearer token used for cron webhook POST delivery
(`delivery.mode = "webhook"`), if omitted no auth header is sent"* [2]. No HMAC signing of the request
body is supported. Fetch is SSRF-guarded and times out at 10 seconds
(`CRON_WEBHOOK_TIMEOUT_MS = 10_000`).

A separate `failureDestination` setting routes failure events to a different endpoint, useful for
separating success and failure notifications. The failure payload is a smaller
`{ jobId, jobName, message }` shape per `sendGatewayCronFailureAlert`. The docs caveat that
`delivery.failureDestination` is only supported on `sessionTarget="isolated"` jobs unless the primary
delivery mode is also `webhook` [1].

**This is the cleanest fit for Stephen's use case.** Wrap long-running agent work as cron jobs with
`--session isolated --message "..."`, configure `delivery.mode=webhook`, point at a Mac-side receiver,
and the rest is wiring.

**What it does NOT cover:** ad-hoc completions that don't go through cron. For interactive agent runs
invoked directly by the user, the OpenResponses SSE stream emits per-request lifecycle events
(`response.completed`, `response.failed`) but these are useful only to the caller holding the stream open
— they're not subscribable from a separate watcher process [4]. Three GitHub issues confirm community
demand for a more general completion notification: #69186 ("Add completion/success notification sound for
agent turns"), #20237 ("WebUI notification system, cron job management popups, and context monitor
integration"), and #44925 ("Subagent completion silently lost — no retry, no notification, no
auto-restart on timeout") [3]. None of these are resolved.

Other surfaces investigated but deemed unsuitable:

- **`/hooks/*` HTTP endpoint** (`hooks.enabled=true`): this is *inbound* — external systems POST here to
  wake the Gateway. Wrong direction for the user's need [5].
- **`@openclaw/plugins-webhooks` plugin** (TaskFlow webhooks): same inbound topology. External systems
  invoke TaskFlow operations (`create_flow`, `finish_flow`, etc.) [6]. Not a subscription model.
- **OpenAI-compat `POST /v1/chat/completions` SSE**: emits token deltas and `[DONE]`, no task-lifecycle
  event types [7].
- **`diagnostics-otel` plugin** (OTLP push): exports spans for model usage, harness lifecycle, tool
  execution. Workable via a downstream collector that webhooks on `openclaw_run_completed_total`, but a
  layered indirection [8].
- **`diagnostics-prometheus` plugin** (Prometheus pull): scrapes `openclaw_run_completed_total` at
  `GET /api/diagnostics/prometheus` with operator-scope auth. Alertmanager could fire on
  `increase(... [1m]) > 0` but the docs explicitly warn against exposing this endpoint publicly [9].
  Heavy infra for one user.
- **JSONL log file** at `/tmp/openclaw/openclaw-YYYY-MM-DD.log` with structured `{msg, ...}` entries
  [10]. Tailable via `tail -F | jq`. Real but fragile — log message strings are not a stable API. Used in
  OpenClaw's own e2e tests via patterns like `"cron finished event"`.
- **Plugin SDK runtime event listeners** (`onRunFinished` and similar): unverified. The
  `diagnostics-otel` and `diagnostics-prometheus` plugins internally subscribe to a diagnostics event
  bus, implying one exists, but `/plugins/quickstart` and `/plugins/sdk` doc paths return 404 and no
  public API for end-user plugins to register lifecycle listeners is documented.

**Implications:** Build on the cron-webhook surface. Treat anything else as an escape hatch.

### Finding 2: Per-request `callback_url` is conspicuously absent across the AI-gateway industry; outbound webhook is the dominant convention

Surveying six gateways (LiteLLM, Portkey, Helicone, Langfuse, MCP, OpenWebUI plus AnythingLLM as a
community comparator), no gateway exposes a per-request `callback_url` field on the chat/agent request
itself [11–14]. Stripe-style callback-per-request, despite being common in payments and telephony APIs,
has not been adopted in the AI-gateway space. The de facto industry pattern for completion notifications
is **outbound webhook on a configured URL** (Pattern A): Helicone POSTs to a per-account webhook with
`request_id`, `user_id`, `request_body`, `response_body`, and HMAC-SHA256 in a `helicone-signature`
header [15]. OpenWebUI POSTs `chat_response` with `chat.id`, `title`, `last_message` to a `WEBHOOK_URL`
env-configured endpoint, with a recent feature (open-webui #16428) introducing 28+ distinct event types
[16][17]. Langfuse webhooks are scoped to prompt-version CRUD only — trace completion is *not* a webhook
event, only an observability sink [18]. LiteLLM and Portkey notably do not offer fire-and-forget
per-completion webhooks: LiteLLM's webhooks are scoped to budget alerts (`budget_crossed`,
`threshold_crossed`); Portkey's only outbound HTTP surface is a synchronous in-band guardrail webhook
with a 3-second timeout that fails open [19].

MCP is the lone gateway in the survey to use streaming subscription (Pattern B): `notifications/progress`
(opt-in via `progressToken` in request `_meta`) and `notifications/message` (logging severity) flow over
the same long-lived stdio/SSE/Streamable HTTP session that delivered the request [20]. This works because
MCP is stateful by design; the other surveyed gateways are stateless HTTP and would need to bolt on a
separate SSE/WebSocket endpoint, which none have shipped.

The universal patterns are D (observability sink — LiteLLM `StandardLoggingPayload` to a generic API, all
of LiteLLM's plug-in integrations into Langfuse/Helicone/Datadog/Sentry/Slack) and E (plugin/middleware —
LiteLLM Custom Callbacks, OpenWebUI `__event_emitter__` and inlet/outlet filters, AnythingLLM custom
agent skills). These map cleanly to the OpenTelemetry-style export model and the Python/JS middleware
idioms gateway codebases were already built around, which is why they're table stakes.

**Implications for OpenClaw:** the cron-webhook surface OpenClaw already has matches Pattern A — the
industry's dominant convention. The user's integration shouldn't try to force OpenClaw into Pattern C
(per-request callback) — that would be a green-field upstream contribution with no precedent in the
space. Build the Mac-side webhook receiver to match Pattern A conventions (POST + JSON + 2xx ack + bearer
or HMAC auth + exponential backoff retry on the sender) and the integration will feel native to anyone
familiar with Helicone/OpenWebUI.

### Finding 3: Tailscale is already 80% deployed; one Pi install completes the transport

The user's existing infrastructure was verified empirically:

- `/opt/homebrew/bin/tailscale` v1.96.5 installed on the Mac (verified). Tailnet members include
  `dresden` (the Mac) and `mister` (the iPhone). **The Raspberry Pi is not yet on the tailnet.**
- `tailscale-app` listed at line 182 of `.chezmoidata/system_packages_autoinstall.yaml`, confirming
  Tailscale is a declared part of the workstation toolchain.
- `cloudflared` v2026.5.0 installed via Homebrew (verified) but no tunnel currently configured in any
  chezmoi template. Recent commit `9c7fb33` (2026-05-04) added the formula.
- The `com.webdavis.*.plist.tmpl` LaunchAgent pattern is already used five times (atuin-daemon,
  yt-dlp-pot reload, GHA watcher, claude launchagent, plus one more), so adding a sixth for a webhook
  receiver follows established convention.
- Notification sinks are operational: `alerter` from Homebrew handles macOS NSUserNotification;
  `hue-pulse.sh` accepts an exit code and pulses the user's Philips Hue lights via `openhue` + `jq`. Both
  are called inline from `dot_bashrc.tmpl:257-260` today.

The transport landscape ranks as follows for this user's specific topology:

**Tailscale direct connect (best fit).** Install Tailscale on the Pi
(`curl -fsSL https://tailscale.com/install.sh | sh; sudo tailscale up`). MagicDNS resolves `dresden` from
anywhere on the tailnet; the Pi calls `http://dresden:9999/notify` regardless of which physical network
the Mac is currently on. WireGuard handles NAT traversal automatically; DERP relays cover symmetric-NAT
cases with an extra 50–200ms. Free plan supports 100 devices and 3 users — well within personal-homelab
scope. The Mac runs a small HTTP server bound to the tailnet interface (`100.x.y.z:9999`); the
LaunchAgent wraps it with `KeepAlive=true` mirroring the `com.webdavis.atuin-daemon.plist` shape. Latency
20–150ms direct, sub-second worst case. Failure mode: Mac asleep → events drop unless the sender retries
or the receiver queues. Setup complexity 2/5.

**MQTT-over-Tailscale (best for durability).** Install `mosquitto` on the Pi
(`apt install mosquitto mosquitto-clients`). Pi-side adapter receives the cron webhook, publishes to a
topic. Mac runs `mosquitto_sub -h dresden-tailnet-ip -t notify/# -q 1 -c -i mac-notify` in a LaunchAgent
loop that fires `alerter` and `hue-pulse.sh` per message. The killer feature is QoS 1 + persistent
session: messages enqueue server-side while the Mac is asleep, then drain on reconnect. This is the
cheapest reliable queue you'll ever set up. Trade-off: a second always-on service on the Pi and a
slightly fussier subscriber loop on the Mac. Setup complexity 3/5. Note: don't expose port 1883 publicly;
constrain to the tailnet.

**Pushover (different category, flagged but not recommended).** A `curl` to
`https://api.pushover.net/1/messages.json` with a `token` and `user` key (one-time $5 per platform).
Delivers via APNs to the Mac/iOS Pushover app — works across cell networks, on locked screens, on closed
laptops. Latency 1–3 seconds through APN. Strictly speaking, Pushover lives in a different category than
`ntfy.sh`: ntfy.sh competes with the local webhook stack as a transport-layer SaaS, while Pushover
leverages Apple's push infrastructure which the other options can't replicate. If a notification is
genuinely critical and a sleeping Mac shouldn't miss it, Pushover or an APNs-via-Shortcuts equivalent is
the right answer — but the user's stated SaaS aversion suggests holding this in reserve for true
emergencies, not as the default channel.

**Dominated options** (not recommended):

- **SSE / long-polling**: strictly inferior to MQTT for this use case. Reconnect logic and event-replay
  must be designed and tested; MQTT gives this for free.
- **SSH back-channel via autossh**: Tailscale on the Mac obviates this. Building an SSH reverse tunnel
  when WireGuard is already there is reinventing a worse Tailscale.
- **Cloudflare Tunnel**: solves the inverse problem (exposing Pi services to the public internet, e.g., a
  webhook receiver for IoT devices). Wrong direction for Pi → roaming Mac. Keep `cloudflared` in reserve
  for actual Pi-ingress use cases.
- **Bare HTTP webhook on LAN (no Tailscale)**: works only on the home LAN; the moment the Mac travels,
  it's unreachable. Tailscale solves this with one install.
- **`ntfy.sh`**: explicitly rejected by the user in earlier brainstorming.

### Finding 4: The cron-webhook payload maps cleanly onto the existing notification primitives

`alerter` accepts a title, message, and optional sound. `hue-pulse.sh` accepts an exit code (0 = green
pulse, non-zero = red). The `CronEvent` payload supplies more than enough to drive both:

```python
# Mac-side receiver, pseudo-code
def on_cron_event(evt):
    if evt['action'] != 'finished':
        return  # only act on completions
    title = f"OpenClaw cron: {evt.get('job', {}).get('name', evt['jobId'])}"
    duration = f"{evt.get('durationMs', 0) / 1000:.1f}s"
    summary = (evt.get('summary') or '')[:140]  # truncate
    msg = f"{evt['status']} in {duration} — {summary}"
    subprocess.run(['alerter', '--title', title, '--message', msg, '--sound', 'Glass'])
    exit_code = 0 if evt['status'] == 'success' else 1
    subprocess.run([str(HOME / '.local/bin/hue-pulse.sh'), str(exit_code)])
```

The hue-pulse `<exit_code>` semantic was added in commit `00e670b` (2026-04-25 — 4-phase wave with deeper
color) and refined in `a1e40b6` (2026-04-24 — double-pulse). The `CronEvent.status` field maps directly:
`"success"` → green pulse, `"failure"` / `"timeout"` / `"error"` → red pulse.

For the **gh-notify side of P11**, the integration story is different. `gh-notify` fetches GitHub
notifications via `gh` CLI; it's not an OpenClaw event source. The "OpenClaw agent notifications for
gh-notify events" bullet in the Todoist task description (`6gfVJ9P5vpX64JhM`) needs clarification: the
most defensible interpretation is **inbound to OpenClaw** — when `gh-notify` fires, also notify OpenClaw
via its `/hooks/agent` or `/hooks/wake` endpoint so OpenClaw can take action on the GitHub event. That
uses the *opposite* surface from this research's focus (OpenClaw's inbound webhook endpoints). The
"hue-pulse blue" half of P11 is straightforward: extend `hue-pulse.sh` to accept a color argument and
call `hue-pulse.sh blue` from the gh-notify hook. No new transport needed for that half.

**Implication for the spec:** the spec should split P11 into two clearer sub-goals: (a) gh-notify
integration with hue-pulse (color blue) — purely Mac-side, no OpenClaw needed; (b) gh-notify → OpenClaw
inbound notification — uses `/hooks/*`, an entirely different mechanism from the cron-webhook surface
that P10 builds on.

______________________________________________________________________

## Three most promising options

### Option A — Cron-webhook → Tailscale → Mac LaunchAgent webhook receiver (RECOMMENDED)

**Architecture sketch:**

```
[ Pi: OpenClaw Gateway ]
   long-running cron job (--session isolated --message "...")
   delivery.mode = "webhook", delivery.to = "http://dresden:9999/notify"
   webhookToken = bearer (KeePassXC entry)
                            |
                            v  POST /notify  (Authorization: Bearer ...)
[ Tailscale tailnet ]  ──  100.x.y.z:9999  (MagicDNS: dresden)
                            |
                            v
[ Mac: com.webdavis.openclaw-notify-receiver.plist (LaunchAgent) ]
   tiny Python HTTP server (40 lines)
   verifies bearer, parses CronEvent JSON
   subprocess: alerter --title ... --message ... --sound Glass
   subprocess: ~/.local/bin/hue-pulse.sh <exit_code>
```

**Effort/complexity:** 2/5. Tailscale install on Pi: one curl-pipe command. Python receiver: ~40 lines.
LaunchAgent plist: copy `com.webdavis.atuin-daemon.plist.tmpl` and adjust. Cron job config: per-job YAML
or one-time per template.

**Failure modes:**

- Mac asleep → cron webhook POST gets retry behavior per OpenClaw's HTTP client (verify exact retry
  policy in `server-cron-notifications.ts`); events lost if retries exhaust.
- Tailnet partition → same as Mac-asleep behavior.
- Bearer token leak → attacker can spoof completion notifications (not high-impact but worth HMAC if
  OpenClaw adds it later).
- 10-second timeout on Pi side — Mac receiver must respond within 10s; trivial for a passthrough script.

**Fit with existing stack:** perfect. Mirrors `com.webdavis.atuin-daemon.plist` shape. Uses existing
`alerter` + `hue-pulse.sh` sinks. No new sinks. No new transports beyond Tailscale, which is already 80%
deployed.

**Specific concerns:**

- Bearer-only auth (no HMAC body signing). Adequate for personal homelab over Tailscale.
- Requires wrapping intended notification triggers as cron jobs. If the user wants notifications for
  events that aren't cron-driven (e.g., interactive `openclaw chat` sessions), Option A doesn't cover
  them — falls back to Option C.
- The receiver is the new attack surface. Bind to the tailnet interface only, not 0.0.0.0.

### Option B — Cron-webhook → MQTT-over-Tailscale → Mac mosquitto_sub LaunchAgent

**Architecture sketch:**

```
[ Pi: OpenClaw cron webhook ]
   delivery.to = "http://localhost:9998/cron-to-mqtt"  (loopback adapter)
                            |
                            v
[ Pi: tiny adapter (Python or bash) ]
   receives webhook → publishes to mosquitto topic "openclaw/cron/finished"
   QoS 1, retain false
                            |
                            v
[ Pi: mosquitto broker (over Tailscale; bind to 100.x.y.z:1883) ]
                            |
                            v  persistent session, QoS 1
[ Mac: com.webdavis.mqtt-notify.plist (LaunchAgent) ]
   mosquitto_sub -h <pi-tailnet> -t "openclaw/#" -q 1 -c -i mac-notify
   while read line; do alerter; hue-pulse; done
```

**Effort/complexity:** 3/5. Adds mosquitto install + config on Pi, the cron-to-MQTT adapter (~50 lines),
and the Mac-side subscriber LaunchAgent. Roughly double the moving parts of Option A.

**Failure modes:**

- Mac asleep for hours → broker queues messages, drains on Mac wake (the whole point).
- Mosquitto config errors → silent drops. Test thoroughly.
- Adapter goes down → Pi → Mac path broken even if Tailscale and broker are healthy.

**Fit with existing stack:** good but more layers. The Mac-side `while read; do alerter; hue-pulse; done`
slots into the LaunchAgent pattern. Adds a category of failure (broker queue grew unbounded) that Option
A doesn't have.

**Specific concerns:**

- The Pi-side adapter is a new component to maintain. Could be eliminated by configuring OpenClaw to POST
  directly to a webhook that the broker accepts (mosquitto plugins like `mosquitto_dynamic_security` or
  HTTP-bridge), but those are themselves more infrastructure.
- For one user with maybe a handful of cron jobs per day, the queue durability is a feature with no real
  cost — but the build cost is real.

### Option C — Gateway log tail → Pi-side filter → existing Option A/B transport

**Architecture sketch:**

```
[ Pi: OpenClaw Gateway ]
   writes JSONL log: /tmp/openclaw/openclaw-2026-05-15.log
                            |
                            v
[ Pi: tail -F | jq 'select(...) ' (systemd service or supervisor) ]
   filters interesting events (e.g., msg == "run finished")
   formats as notification event
                            |
                            v  (same Option A or B transport from here)
```

**Effort/complexity:** 3/5. Adds the log-tail sidecar on the Pi and reuses one of the other transports
for delivery.

**Failure modes:**

- Log message strings are not a stable API — an OpenClaw upgrade changes `"cron finished event"` to
  `"cron job finished"` and silently breaks the filter.
- Log rotation timing — the daily rotation could miss the tail's last reads.
- Filtering false positives if log message vocabulary expands.

**Fit with existing stack:** OK. The downstream transport reuses Option A or B unchanged.

**Specific concerns:**

- Brittle by design. OpenClaw's own e2e tests reference the same log message strings
  (`"cron finished event"` in `scripts/e2e/cron-mcp-cleanup-docker-client.ts`), so the strings are de
  facto stable in the short term — but that's not a contract.
- Only use this for events that fall outside the cron-webhook scope (ad-hoc agent runs, interactive
  sessions). If you find yourself filtering for cron events here, you've reinvented Option A worse.

______________________________________________________________________

## Recommendation

**Pick Option A.**

The single most important fact: OpenClaw's cron-webhook surface is the documented, supported, first-class
notification API. It matches the industry-dominant Pattern A convention (Helicone, OpenWebUI), and
bolting on Pattern B (streaming) or Pattern C (per-request `callback_url`) would be a green-field
upstream contribution with no precedent in the AI-gateway space. Building on what OpenClaw already
provides is dramatically cheaper than building what it doesn't.

The Tailscale piece tips the decision further. The user already runs Tailscale on `dresden` and `mister`,
has `tailscale-app` in the package autoinstall yaml, and is one curl-pipe install on the Pi away from
MagicDNS-based any-network reachability. The Mac-side webhook receiver is a 40-line Python script wrapped
in a LaunchAgent plist that copies `com.webdavis.atuin-daemon.plist.tmpl` line-for-line in shape. Total
effort: about an afternoon, end-to-end.

**Why not Option B (MQTT) by default?** Because the failure-mode delta — "Mac asleep for an hour and I
miss notifications" — isn't a problem for the user's stated use case ("alerter + hue-pulse for
long-running command completion"). These are advisory notifications. The user is at the Mac when work is
happening; if a notification fires while they're asleep, it's noise the next morning, not lost work. The
durability cost (extra service on Pi, more moving parts, broker tuning) doesn't pay back at this scale.
**Reserve Option B for events the user can't afford to drop** — for example, backup completions,
monitoring threshold crossings, certificate-expiry warnings, OpenClaw cron jobs that produce summaries
the user actually reads. The recommendation is: ship Option A first; promote specific event classes to
Option B (a topic suffix in MQTT) if dropped notifications start to bite.

**Why not Option C (log tail)?** Because it's the right answer to a different question. Option C is the
escape hatch for events that aren't cron-driven — interactive agent sessions, ad-hoc completions. If the
user's primary "long-running tasks" turn out to live outside the cron model, Option C becomes essential.
But for the documented use case (long-running tasks the user kicks off and walks away from — a perfect
cron fit), Option A is direct and Option C is indirect.

**Specific implementation guidance for the dotfiles spec (P10/P11):**

1. **P10 (`6gfVJ7VwcFQvg7xM`) becomes concrete:** "Install Tailscale on the OpenClaw Pi. Configure
   long-running OpenClaw cron jobs with `delivery.mode=webhook` + `cron.webhookToken` from KeePassXC. Add
   a `com.webdavis.openclaw-notify-receiver.plist` LaunchAgent on the Mac running a Python HTTP server
   bound to the Mac's tailnet IP. Receiver verifies bearer, parses CronEvent, fires `alerter` +
   `hue-pulse.sh`."
1. **P11 (`6gfVJ9P5vpX64JhM`) needs splitting:** the gh-notify-and-hue-blue half is a Mac-side change to
   `hue-pulse.sh` (add color arg) and a gh-notify hook. The OpenClaw-notification half is inbound to
   OpenClaw via `/hooks/wake` or `/hooks/agent` — different surface from P10's outbound cron-webhook. The
   spec should rename or split this task.
1. **Karl's wkflw-ntfy review (P10's secondary goal):** the relevant adoptable ideas are the marker-based
   atomic-claim escalation pattern and the bats-based test approach for notification scripts. Reject
   ntfy.sh as transport; reject nushell-specific implementation. Apply the test pattern to the new Python
   receiver and to the existing `hue-pulse.sh`.

**Open follow-up if Option A proves insufficient:** if the user discovers that "long-running task"
includes events the cron model can't accommodate, the spec should add a Todoist task for adopting Option
B for those specific event classes, OR an upstream contribution to OpenClaw to add a general
task-complete event API (per issue #69186). Don't speculatively build this now.

______________________________________________________________________

## Synthesis & Insights

### Pattern 1: The "first-class surface" question is the load-bearing one

This research's primary work was answering one question: does OpenClaw already expose a notification
surface for this use case? Once that was answered (yes, the cron-webhook), the rest of the architecture
flowed: the cleanest transport (Tailscale, already 80% deployed), the cleanest receiver shape
(LaunchAgent, already a pattern), and the cleanest integration (alerter + hue-pulse, already wired).
Every option that doesn't build on the cron-webhook surface (log tail, observability sinks, plugin SDK
speculation) is strictly more work for strictly less integration depth.

The lesson generalizes: when integrating with an existing service, the first research question is always
"what surface exists?" rather than "what surface should we build?" Building from absence is much more
expensive than building from presence.

### Pattern 2: Industry-convention compliance pays back

OpenClaw's cron-webhook follows Pattern A, the AI-gateway industry default. The Mac-side receiver should
follow the same convention: POST + JSON + 2xx ack + optional bearer or HMAC, with the sender expected to
retry on 5xx. By matching the convention, the receiver is trivially portable to other gateways (Helicone,
OpenWebUI) if the user ever broadens to them. By contrast, building a per-request callback model (Pattern
C) — even though it's interesting and arguably cleaner — would be a green-field design with no
operational precedent. Picking the dominant convention is rarely the most elegant choice, but it's almost
always the most maintainable.

### Pattern 3: Latent infrastructure is a powerful signal

The Tailscale-already-on-the-Mac fact reshaped the transport ranking entirely. Without it, the analysis
would have spent more time on Cloudflare Tunnel or autossh; with it, Tailscale dominates trivially. The
lesson: before designing a new piece of infrastructure, audit what's already running. The audit in this
case was a one-line `command -v tailscale cloudflared` check that saved hours of architectural debate.

### Implications for the dotfiles tasks plan

- P10 becomes concrete enough to estimate: install Tailscale on the Pi (~5 min), write the receiver
  script (~30 min), write the LaunchAgent plist (~15 min), test end-to-end (~30 min). Realistic
  single-session work.
- P11 needs splitting in the spec. The gh-notify-and-hue-blue half is a different problem from the
  OpenClaw-notifications half; conflating them in one Todoist task created confusion.
- An open question remains: do all the user's intended "long-running tasks" fit OpenClaw's cron model? If
  interactive agent runs are also in scope, Option C (log tail) becomes a needed addition. The spec
  should ask the user to confirm before P10 starts.

______________________________________________________________________

## Limitations & Caveats

### Counterevidence Register

**Contradictory finding 1: OpenClaw cron-webhook retry policy is undocumented.** The Agent 1 evidence
quote covered the dispatcher and the bearer auth, but did not quote a retry policy. The 10-second timeout
is documented; whether OpenClaw retries on 5xx and with what backoff is not. If the answer is "no
retries," then Mac-asleep events are dropped without recovery, weakening Option A's reliability story.
**Mitigation:** verify by inspecting `server-cron-notifications.ts` or testing empirically. If no
retries, consider promoting to Option B sooner.

**Contradictory finding 2: Pushover is not necessarily in the same category as ntfy.sh.** The user
rejected ntfy.sh as a SaaS transport. Pushover *is* a SaaS, but it leverages APN — Apple's first-party
push infrastructure — and is the only realistic answer for "notification while Mac is asleep and not on
AC power with 'wake for network access.'" If the user's stated rejection of ntfy.sh extends to all SaaS
transports universally, fine; if the rejection is specifically about ntfy.sh-style "custom HTTP push
transports that duplicate what Apple already provides," Pushover may be in a different bucket. The user
should make this distinction explicit if Pushover is on the table.

### Known Gaps

**Gap 1: The retry policy mentioned above.** Should be resolved before relying on Option A for critical
events.

**Gap 2: Plugin SDK event listener surface.** Agent 1 reported the OpenClaw `diagnostics-otel` and
`diagnostics-prometheus` plugins subscribe to an internal diagnostics event bus, implying one exists. The
`/plugins/quickstart` and `/plugins/sdk` doc paths return 404, leaving the public API undocumented.
**If** OpenClaw exposes a plugin lifecycle hook (`onRunFinished`), it could be a cleaner alternative to
log-tailing for Option C. Resolving requires source-code investigation, deferred.

**Gap 3: "Long-running tasks" workflow shape unverified.** This research assumed long-running tasks are
cron-driven. If interactive agent runs are also in scope, Option C is needed in addition. Spec should ask
the user to confirm.

### Assumptions Revisited

- **"User runs Tailscale already on Mac"**: verified empirically (`tailscale v1.96.5` on
  `/opt/homebrew/bin/tailscale`).
- **"User has rejected ntfy.sh"**: stated by user in prior brainstorming. Not re-verified in this
  research; carried forward.
- **"Pi is on the home LAN with reliable internet"**: not verified — inferred from "always-on homelab Pi"
  framing.

### Areas of Uncertainty

**Uncertainty 1: P11's OpenClaw integration scope.** The Todoist description ("OpenClaw agent
notifications for gh-notify events") is ambiguous. This research interpreted it as inbound-to-OpenClaw
(using `/hooks/agent`) but could alternatively mean "when gh-notify fires, also notify users via
OpenClaw's agent channel" (different mechanism entirely). Resolve before starting P11.

**Uncertainty 2: Whether the cron-webhook receives `summary` for all cron jobs or only ones with
`--message`.** Agent 1 noted the webhook only fires when `params.evt.summary` is set. If cron jobs
without `--message` don't produce a summary, those jobs are silent to the webhook. Verify when
implementing.

______________________________________________________________________

## Recommendations

### Immediate Actions (within the dotfiles tasks plan)

1. **Update P10 description in Todoist** (`td task update id:6gfVJ7VwcFQvg7xM`) to reflect concrete
   scope: "Install Tailscale on the OpenClaw Pi. Wrap long-running agent work as cron jobs with
   `delivery.mode=webhook` + bearer auth (`cron.webhookToken` from KeePassXC). Add
   `com.webdavis.openclaw-notify-receiver.plist` LaunchAgent on the Mac with a Python webhook receiver
   bound to the tailnet IP; receiver fires `alerter` + `hue-pulse.sh` per CronEvent." Link this research
   file in the description.
1. **Split P11** (`td task update id:6gfVJ9P5vpX64JhM`) into two clearer halves: (a) gh-notify hook →
   blue hue-pulse (Mac-only); (b) gh-notify → OpenClaw inbound notification via `/hooks/agent` (uses
   OpenClaw's inbound webhook surface — different from P10's outbound).
1. **Verify the cron-webhook retry policy** by reading `src/gateway/server-cron-notifications.ts` in the
   OpenClaw repo before relying on Option A for critical events. If no retries, escalate the question of
   Option B promotion.

### Next Steps (after the audit cycle completes)

1. **Decide whether to upstream a general task-complete API to OpenClaw** (GitHub issues #69186 and
   #44925 already request this). A small PR adding `events.onTaskComplete` config that POSTs the existing
   `RunCompletedTelemetry` to a webhook would address all three open issues simultaneously. Defer until
   the dotfiles tasks plan is complete.
1. **Document the receiver protocol** in the chezmoi repo's README or CLAUDE.md so future-you remembers
   the shape (bearer auth, expected payload, what triggers alerter vs hue-pulse).

### Further Research Needs

1. **Plugin SDK lifecycle surface**: if OpenClaw documents this in a future release, it may obsolete
   Option C. Watch.
1. **Mac wake-on-network reliability**: empirical question — does the user's Mac wake reliably for
   tailnet traffic when on AC power with "wake for network access" enabled? If yes, the Option A
   failure-mode story improves materially.

______________________________________________________________________

## Bibliography

[1] OpenClaw docs — "Cron Jobs" (Automation), "Delivery and output" section.
https://docs.openclaw.ai/automation/cron-jobs (Retrieved: 2026-05-15)

[2] OpenClaw docs — "Configuration Reference" (Gateway), `cron.webhookToken` entry.
https://docs.openclaw.ai/gateway/configuration-reference (Retrieved: 2026-05-15)

[3] OpenClaw GitHub repo open issues: #69186, #20237, #44925 (completion notification gaps).
https://github.com/openclaw/openclaw/issues (Retrieved: 2026-05-15)

[4] OpenClaw docs — "OpenResponses HTTP API" (Gateway), event types `response.completed`,
`response.failed`, etc. https://docs.openclaw.ai/gateway/openresponses-http-api (Retrieved: 2026-05-15)

[5] OpenClaw docs — "Web" section, `/hooks/*` HTTP endpoint. https://docs.openclaw.ai/web/ (Retrieved:
2026-05-15)

[6] OpenClaw docs — "Plugins: Webhooks". https://docs.openclaw.ai/plugins/webhooks (Retrieved:
2026-05-15)

[7] OpenClaw docs — "OpenAI HTTP API" (Gateway), SSE behavior.
https://docs.openclaw.ai/gateway/openai-http-api (Retrieved: 2026-05-15)

[8] OpenClaw docs — "OpenTelemetry" (Gateway), span coverage.
https://docs.openclaw.ai/gateway/opentelemetry (Retrieved: 2026-05-15)

[9] OpenClaw docs — "Prometheus" (Gateway), `openclaw_run_completed_total` and
`/api/diagnostics/prometheus`. https://docs.openclaw.ai/gateway/prometheus (Retrieved: 2026-05-15)

[10] OpenClaw docs — "Logging", JSONL log shape and `logs tail` RPC. https://docs.openclaw.ai/logging
(Retrieved: 2026-05-15)

[11] LiteLLM docs — "Generic API Callback". https://docs.litellm.ai/docs/observability/generic_api
(Retrieved: 2026-05-15)

[12] LiteLLM docs — "Custom Callbacks". https://docs.litellm.ai/docs/observability/custom_callback
(Retrieved: 2026-05-15)

[13] LiteLLM docs — "Proxy Logging". https://docs.litellm.ai/docs/proxy/logging (Retrieved: 2026-05-15)

[14] LiteLLM docs — "Proxy Alerting/Webhooks". https://docs.litellm.ai/docs/proxy/alerting (Retrieved:
2026-05-15)

[15] Helicone docs — "Webhooks", HMAC-SHA256 in `helicone-signature` header.
https://docs.helicone.ai/features/webhooks (Retrieved: 2026-05-15)

[16] OpenWebUI docs — "Webhook Integrations" (Administration).
https://docs.openwebui.com/features/administration/webhooks/ (Retrieved: 2026-05-15)

[17] open-webui GitHub discussion #16428 — "28 distinct event types".
https://github.com/open-webui/open-webui/discussions/16428 (Retrieved: 2026-05-15)

[18] Langfuse docs — "Prompt Webhooks and Slack Integrations".
https://langfuse.com/docs/prompt-management/features/webhooks-slack-integrations (Retrieved: 2026-05-15)

[19] Portkey docs — "Bring Your Own Guardrails" webhook semantics.
https://portkey.ai/docs/product/guardrails/list-of-guardrail-checks/bring-your-own-guardrails (Retrieved:
2026-05-15)

[20] MCP specification (2025-06-18) — base protocol notifications + Progress + Logging utilities.
https://modelcontextprotocol.io/specification/2025-06-18/basic (Retrieved: 2026-05-15)

[21] Tailscale docs — installation and MagicDNS. https://tailscale.com/kb/ (Retrieved: 2026-05-15)

[22] Eclipse Mosquitto MQTT broker — QoS levels and persistent sessions.
https://mosquitto.org/man/mosquitto-conf-5.html (Retrieved: 2026-05-15)

[23] Helicone docs — "Alerts". https://docs.helicone.ai/features/alerts (Retrieved: 2026-05-15)

[24] OpenWebUI docs — "Event Emitters" (Plugin development).
https://docs.openwebui.com/features/extensibility/plugin/development/events/ (Retrieved: 2026-05-15)

[25] AnythingLLM custom agent skill tutorial — webhooks.
https://dev.to/drunnells/writing-an-anythingllm-custom-agent-skill-to-trigger-makecom-webhooks-1dn0
(Retrieved: 2026-05-15)

[26] MCP Progress Notifications spec.
https://modelcontextprotocol.io/specification/2025-06-18/basic/utilities/progress (Retrieved: 2026-05-15)

[27] MCP Logging Notifications spec.
https://modelcontextprotocol.io/specification/2025-06-18/server/utilities/logging (Retrieved: 2026-05-15)

______________________________________________________________________

## Appendix: Methodology

### Research Process

Executed deep mode (8-phase pipeline). Three parallel general-purpose research agents covered orthogonal
dimensions of the question.

**Phase Execution:**

- **Phase 1 (SCOPE):** Defined per the user's `/brainstorming`-driven request. Three solution
  architectures expected.
- **Phase 2 (PLAN):** Decomposed into three independent agent prompts (OpenClaw surface, AI-gateway
  patterns, Pi → Mac transports). Each agent received a structured brief with citation requirements.
- **Phase 3 (RETRIEVE):** Agents ran in parallel in background. Total wall time ~5 minutes. Each agent
  independently consulted primary sources (OpenClaw docs + GitHub issues, gateway docs, transport docs,
  the user's local chezmoi repo for state verification).
- **Phase 4 (TRIANGULATE):** Cross-referenced findings — Agent 3's discovery that `cloudflared` is
  installed but not configured (per commit `9c7fb33`) was independently corroborated; Agent 1's claim
  that OpenClaw has only one outbound webhook surface was independently consistent with Agent 2's
  industry context (Pattern A is industry-standard, OpenClaw's cron-webhook fits the pattern).
- **Phase 4.5 (OUTLINE REFINEMENT):** No major outline shifts. Original outline (executive summary →
  per-dimension findings → three options → recommendation) held up against the evidence.
- **Phase 5 (SYNTHESIZE):** Pattern recognition across agents — "first-class surface" question is
  load-bearing; latent infrastructure (Tailscale) reshapes ranking; industry-convention compliance pays
  back.
- **Phase 6 (CRITIQUE):** Adversarial passes — "would the user push back on this?" yielded two
  counterevidence items (Pushover-vs-ntfy categorization, cron-webhook retry policy uncertainty) and
  three named gaps.
- **Phase 7 (REFINE):** Surfaced the gh-notify/P11 ambiguity as a spec-level recommendation. Tightened
  the recommendation rationale.
- **Phase 8 (PACKAGE):** This document.

### Sources Consulted

**Total cited:** 27.

**Source Types:**

- OpenClaw documentation pages: 8
- OpenClaw GitHub issues + source files: 1 issues batch + 3 source files referenced via Agent 1
- AI-gateway documentation (LiteLLM, Portkey, Helicone, Langfuse, MCP, OpenWebUI, AnythingLLM): 14
- Transport documentation (Tailscale, Mosquitto): 2
- The user's local chezmoi repository (state verification): paths cited inline

**Temporal Coverage:** All citations retrieved 2026-05-15. Recency-critical claims (existing installed
binary versions, recent commit references) date-stamped to that day.

### Verification Approach

**Triangulation:** Core claims required either (a) a verbatim docs quote, (b) a primary source-file
reference, or (c) empirical verification on the user's machine. Examples: OpenClaw cron-webhook existence
is verified by quoting the "Delivery and output" docs section AND citing
`src/gateway/server-cron-notifications.ts`. Tailscale-already-on-Mac is verified by
`command -v tailscale` returning a real path.

**Credibility:** Official OpenClaw + Tailscale documentation scored highest. Tutorial/blog sources (e.g.,
AnythingLLM Make.com webhook tutorial) used only for color and code examples, never as sole source for a
core claim. Negative findings ("no per-request `callback_url`") triangulated across multiple gateways
before concluding the absence is industry-wide.

**Quality Control:** Every URL retrieved during the research session. Where claims rest on quotes, the
quotes are verbatim from the cited source. Counterevidence section explicitly surfaces the two findings
that don't fit the recommendation's narrative.

### Claims-Evidence Table

| ID  | Claim                                                                                         | Evidence                                                                                    | Sources           | Confidence                                                                     |
| --- | --------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- | ----------------- | ------------------------------------------------------------------------------ |
| C1  | OpenClaw exposes outbound webhook only via cron jobs                                          | Verbatim docs quote + source-file reference                                                 | [1][2]            | High                                                                           |
| C2  | No per-request `callback_url` in any of 6 surveyed gateways                                   | Negative finding triangulated across LiteLLM, Portkey, Helicone, Langfuse, MCP, OpenWebUI   | [11–20]           | High                                                                           |
| C3  | Tailscale already installed on Mac, not on Pi                                                 | Empirical: `command -v tailscale` + chezmoi grep                                            | [21] + local repo | High                                                                           |
| C4  | `cloudflared` installed but no tunnel configured                                              | Empirical + recent commit `9c7fb33` (2026-05-04)                                            | local repo        | High                                                                           |
| C5  | Cron-webhook retry policy is undocumented                                                     | Absence of retry mention in `cron.webhookToken` docs entry                                  | [2]               | Medium (negative finding; source-code verification needed for full confidence) |
| C6  | OpenClaw has open issues requesting general completion notifications (#69186, #20237, #44925) | Issue titles + states from GitHub API                                                       | [3]               | High                                                                           |
| C7  | Pattern A (outbound webhook) is the AI-gateway industry default                               | Survey of 6 gateways; convention emerges across Helicone + OpenWebUI + Langfuse-prompt-CRUD | [11–20]           | High                                                                           |

**Confidence Levels:**

- **High:** 3+ independent sources or verbatim primary quote + corroboration.
- **Medium:** Single primary source OR negative finding without exhaustive verification.
- **Low:** (none in this report)

______________________________________________________________________

## Report Metadata

**Research Mode:** Deep (8-phase, 3 parallel agents) **Total Sources:** 27 cited **Word Count:** ~7,500
**Research Duration:** ~10 minutes (parallel agents) + synthesis **Generated:** 2026-05-15
**Validation:** All citations retrieved during the research session; counterevidence explicitly
registered; no fabricated sources.
