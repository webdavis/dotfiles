# pns session attribution, Discord threads and the stale-block escalation

Design for the four operator rulings of 2026-09-14, in the order they are to be built: the sender
header, the stale-block escalation, the route decisions, and one Discord thread per session.

Every file and line reference below was read in the worktree at
`~/.herdr/worktrees/dotfiles/docs-pns-session-attribution-design` on 2026-09-14, and every claim
about hermes was read out of the installed checkout at `~/.hermes/hermes-agent` (Hermes Agent
v0.17.0, upstream `a4091e49`).

## Problem

The `pns` Discord channel says `claude · blocked · dotfiles` and then the message. With four agents
running in four worktrees of the same repository, that line names none of them: it cannot say which
branch, which session, or what the session was asked to do. On a phone the channel reads as a
column of identical lines, so the operator opens the laptop to find out which pane is waiting.

Three things follow from that, and the rulings name all three. Say who sent it. Group a session's
events so the channel shows one line per agent instead of one per event. And when a session has
been blocked long enough that nobody is coming, escalate it once, to a route reserved for things
that need a human.

## Constraints

- The operator runs chezmoi applies. Nothing here may require an agent to run one.
- Secrets never enter the repository. The hermes signing key reaches the pns config by a keepassxc
  reference (`dot_config/pns/private_config.toml.tmpl:48`), and the hermes gateway config is
  age-encrypted at `private_dot_hermes/encrypted_private_config.yaml.age`.
- hermes is a third party. It is configured through its own config file and its supported options,
  never patched.
- No removal mechanisms. A route or file that stops being used is listed for the operator, never
  swept by a script this repository ships.
- Tests cover the behavior of tools we wrote. A test that only checks that two declarations agree
  (a route list against a config, a plist field) is out of scope under the 2026-08-05 ruling.
- pns is a shippable product installed with `cargo install`. Nothing in its workspace may assume
  this checkout exists, and it may not depend on another workspace.
- Rust files target 300 lines and never exceed 500, tests included.

## Current state

### What an event carries

`pns_domain::EventArgs` (`pns/crates/pns-domain/src/notification.rs:34-50`) is the whole of what an
event knows: `agent`, `state`, `project`, `branch`, `detail`, `pane`, `channel`, `scope`,
`long_running`. There is no session id and no session title on it.

The hook path fills those fields in `pns/crates/pns/src/turn_lifecycle.rs:28-45` and `:73-90`, and
in `pns/crates/pns/src/hook_dispatch.rs:79-91`, `:105-120`, `:149`, `:191`. `project` comes from
`project_of` (`turn_lifecycle.rs:93-98`), which is the last non-empty segment of the hook payload's
`cwd`. **In a linked worktree that is the branch slug, not the repository name**: this worktree's
`cwd` ends in `docs-pns-session-attribution-design`, so today's card would call the project that.

`branch` comes from `git_branch` (`pns/crates/pns-adapters/src/git.rs:7-16`), which runs
`git -C <cwd> branch --show-current` under a five second deadline. It is empty on a detached head,
and it is set on only two of the six event paths (the two in `turn_lifecycle.rs`); the four arms in
`hook_dispatch.rs` leave it at the default.

`pane` is `std::env::var("HERDR_PANE_ID")` at each of those call sites. That is the correct source
and the only correct one: `herdr pane current` resolves against the caller's own pane, which is
always the pane the event fired from, and using it makes every desk notification suppress itself
(`pns/crates/pns-domain/src/surface.rs:33-42`, drill D4, 2026-08-13).

The submitted-request path drops more. `pns/crates/pns/src/event_flow/submit/mapping.rs:16-38` maps
a decoded `Request` onto `EventArgs`, and the envelope's `session` field
(`pns/crates/pns-protocol/src/request.rs:86-91`) has nowhere to land, so it is discarded. `agent`
becomes the producer name, which is how `posture` and `uu` events are labelled.

### What the hook payload actually carries

`HookPayload` (`pns/crates/pns-adapters/src/harness/payload.rs:8-71`) parses sixteen fields off the
harness JSON (JavaScript Object Notation) object. `session_id` and `cwd` are among them. **The
user's prompt text is not**, and neither is the harness's own session title.

Both are available. Read out of the installed Claude Code 2.1.270 binary, the `UserPromptSubmit`
payload is built as `hook_event_name:"UserPromptSubmit", prompt:<text>, ..., session_title:<title>`,
and `session_title` also rides on `SessionStart`. Its schema comment in the same binary says
"Payloads may omit it while the field rolls out", so it is optional and can be absent on a session
that has not been titled yet. Codex sends neither field.

The `prompt` hook arm (`hook_dispatch.rs:32-35`) currently marks the turn's start and ends any
blocked wait. It records nothing about what was asked.

### What reaches hermes

`hermes_body` (`pns/crates/pns-adapters/src/destinations/hermes.rs:25-40`) posts five keys:

```json
{"agent": "claude", "state": "blocked", "project": "dotfiles", "detail": "...", "request_id": "..."}
```

`detail` is `event.message`, which is `render::message` (`pns/crates/pns-domain/src/render.rs:42-53`),
the bare detail with the branch prefixed as `branch: body`. The card title,
`render::title` (`render.rs:15-23`), is `agent · state · project` and is not sent to hermes at all.
Both are composed in `rendered_event_quiet` (`pns/crates/pns/src/channel_dispatch.rs:72-90`).

A route name is turned into a URL (uniform resource locator) by swapping the last path segment of
the gateway address (`hermes.rs:50-56`), so a route is a path and the gateway's own table is the
only place routes are declared.

The live `pns` route in `~/.hermes/config.yaml` is:

```yaml
pns:
  secret: <the pns signing secret>
  deliver: discord
  deliver_only: true
  prompt: '{agent} · {state} · {project}


    {detail}'
  deliver_extra:
    chat_id: '<the #pns channel id>'
```

### What the ledger records

`ledger_events` (`pns/crates/pns-adapters/src/persistence/sqlite/ledger/schema.rs:8-14`) has
`seq, producer, request_id, agent, state, project, branch, detail, title, message, preview, pane`,
plus `producer_request` from migration step 5. **It has no session column and no timestamp.** It is
a delivery ledger: the times live on `ledger_legs.due` and `ledger_attempts.started`, and nothing
in the crate ever deletes from it, so it grows for the life of the machine. The schema is at
version 8 with a linear migration ladder
(`pns/crates/pns-adapters/src/persistence/sqlite/migrations.rs:4,41-60`); the database is
`~/.local/state/pns/pns.db` (`persistence/state_dir.rs:3-11`, `sqlite/store.rs:64`).

### The clock and the leased-job primitive

`pns_domain::jobs::Job` (`pns/crates/pns-domain/src/jobs.rs:24-40`) is one leased job: an id that is
its own spool filename, a `due`, a lease `until`, an optional `every`, an optional `unless_marker`
that cancels it, and the argv this binary is re-executed with. `decide`
(`jobs.rs:117-131`) checks the lease first, then the marker, then whether a child is still running,
then the due second. `rearm` (`jobs.rs:150-153`) uses `now + every`, never `due + every`.

The nag is the existing rider on that primitive and the closest thing to ruling 3 already built.
`ArmNag` (`pns/crates/pns-application/src/arm_nag.rs:37-140`) clears the session's answered marker,
publishes a record, and schedules a one-shot job whose lease is one more window past its due second.
`nag_mode` (`pns/crates/pns/src/command_nag.rs:26-67`) is what the job runs: it claims the window,
claims each record by rename, and raises one coalesced card. Two limits matter here. It arms for
`claude` only (`arm_nag.rs:52-54`), and it is armed from one call site,
`moshi_submission.rs:113`, which is the `blocked` arm; `asked`, `plan-ready` and `denied` arm
nothing.

The daemon is `com.webdavis.pns-daemon` (`Library/LaunchAgents/com.webdavis.pns-daemon.plist.tmpl`),
running `pns daemon run`, restarted only on a crash.

### What a session's wait already looks like on disk

`update_blocked_marker` (`pns/crates/pns-adapters/src/protocols/markers/blocked.rs:52-82`) writes
one file per waiting session, named by session id, holding the second the wait started, and removes
it on any event that is not a wait. Which events are waits is
`blocked_marker_action` (`pns/crates/pns-domain/src/lights/phase.rs:148-154`) over
`pulse::LAMP_BLOCKED` (`pns/crates/pns-domain/src/pulse.rs:127`), which is
`["blocked", "asked", "plan-ready", "denied", "asking"]`.

That file is exactly the state ruling 3 needs, and it cannot be used as it stands: writing it is
gated on `lamps_live` (`blocked.rs:65-66`, wired at `pns/crates/pns/src/event_flow/records.rs:47-50`),
so a machine with no `[lights]` table accumulates no markers at all.

### Presence

`surface` (`pns/crates/pns-domain/src/surface.rs:141-169`) answers `Desk`, `Mobile` or `Away` from
the desk input age, the phone input age, the phone marker age, a freshness window and the screen
lock. Only `Some(true)` locks. Both desk readings come from one `ioreg` call:
`parse_idle_nanoseconds` and `parse_screen_locked`
(`pns/crates/pns-adapters/src/macos/desk.rs:25-32,49-82`). **The lock reading is instantaneous.**
pns keeps no history of it, so "locked for the whole window" is not a value anything can read today.

### Routes

Three routes exist in the gateway: `priority`, `pns`, `unattended-upgrades`.
`.chezmoiscripts/run_after_68-hermes-log-route-status.sh.tmpl:30` expects two of them, `pns` and
`unattended-upgrades`, and checks each for presence, `deliver_only`, and a non-404 from the running
gateway.

- `uu` posts to `unattended-upgrades` by default
  (`uu/crates/uu-adapters/src/config/records.rs:17`).
- `posture` already names the route `posture` on every one of its six commands
  (`posture/crates/posture/src/{alert,digest,funnel,heartbeat,poll,watchdog}.rs`, each around line
  40 to 120), with no branch on severity. **That route does not exist in the gateway**, so every
  posture page is refused today.
- posture's watchdog probes `priority` for gateway health
  (`posture/crates/posture/src/watchdog/configuration.rs:80`).
- pns already names `pns-recap` for the return recap
  (`pns/crates/pns-application/src/post_return_recap.rs:47,68`), behind `[recap] digest_as_thread`
  (`dot_config/pns/private_config.toml.tmpl:189-194`), and falls back to the default route with a
  line saying why when the route refuses. That route does not exist in the gateway either.

One signing key covers every route (`[plugins.hermes] key`), so a new route is configured with the
same secret as the existing ones.

## Design

### 1. The sender header

#### The fields, and exactly where each comes from

**project** is the repository name, from
`git -C <cwd> rev-parse --path-format=absolute --git-common-dir`, then dirname, then basename.
Measured in this worktree the common dir is `/Users/stephen/workspaces/Ivy/webdavis/dotfiles/.git`,
so the answer is `dotfiles` where `project_of(cwd)` answers `docs-pns-session-attribution-design`.
Outside a repository it falls back to `project_of(cwd)`.

**branch** is `git -C <cwd> branch --show-current`, the existing `git_branch`. It is empty on a
detached head.

**worktree** is the basename of `git -C <cwd> rev-parse --show-toplevel`, used in the branch slot
only when the branch is empty.

**harness** is `PNS_PRODUCER`, defaulting to `claude` (`hook_dispatch.rs:23`), and the producer name on
the submitted-request path (`mapping.rs:18`). Today that is `claude`, `codex`, and the producer
names `posture`, `uu` and `pns`. `pi` and `omp` reach `pns gate`, which forwards to moshi and raises
no pns event (`moshi_submission.rs:13-26`), so neither can appear in a header until it produces
events.

**session** is the first four characters of `HookPayload.session_id`, or of the envelope's
`session.id`. Four characters of a hexadecimal identifier separate the handful of sessions alive at
once, and a collision costs one ambiguous line, never a misrouted message.

**title** is, in order: the harness's `session_title`, the session's first `prompt` flattened to one
line and cut to 60 characters, the worktree basename. It is stored once per session and read by
every later event.

`/goal` needs no source of its own. A `/goal <text>` invocation is a user prompt, so it arrives
through `UserPromptSubmit` as the prompt text and is captured by the same rule as any other first
prompt.

`pane` stays `HERDR_PANE_ID` from the hook environment at each call site, unchanged, for the reason
`surface.rs:33-42` records.

#### Two layouts

Layout A, two lines, using Discord's bold and its native subtext primitive (`-# `, which hermes
itself documents at `gateway/display_config.py:40,120`):

```
**dotfiles · feat/posture-alert-cutover · blocked**
-# claude · a1b2 · arm posture alert and retire the Bash alerter
Bash(git push --force-with-lease) needs approval
```

Layout B, one line, fixed-width columns inside a code span so they align down the channel:

```
`dotfiles     │ feat/posture-alert-cut… │ claude │ a1b2` arm posture alert and retire the Bash…
Bash(git push --force-with-lease) needs approval
```

#### Recommendation: layout A

Scan order is the whole argument. The question a channel full of agent traffic has to answer at a
glance is "which of my checkouts, on which branch", and layout A puts exactly that in bold on line
one, at the largest weight Discord gives a message. The session title answers the second question,
"which of the three sessions in that checkout", and it sits on the dim line where it does not
compete with the first. The identifiers, which are needed only when the first two lines tie, come
last and smallest.

Layout B fails on the surface that matters most. A code span does not reflow, so on a phone the
columns either scroll horizontally or shrink to unreadable, and the padding that buys the alignment
spends the width the title needs. It also ties the rendering to a monospaced assumption that
Discord honours on the desktop client and not in every notification preview.

#### Where the header is composed

**pns composes both lines and sends them as fields; the gateway template only places them.** The
gateway's renderer substitutes `{key}` from the posted body and has no conditionals, and a key the
body omits renders as the literal text `{key}`
(`~/.hermes/hermes-agent/gateway/platforms/webhook.py:864-879`). A template that assembled
`{project} · {branch}` itself would therefore print a dangling separator for an event with no
branch, and the literal `{branch}` for an older pns that does not send one.

So `hermes_body` (`destinations/hermes.rs:25-40`) gains four keys and keeps its existing five:

```json
{"agent": "claude", "state": "blocked", "project": "dotfiles", "detail": "...",
 "request_id": "...", "header": "dotfiles · feat/posture-alert-cutover · blocked",
 "subheader": "claude · a1b2 · arm posture alert and retire the Bash alerter",
 "body": "Bash(git push --force-with-lease) needs approval", "thread_id": ""}
```

`header` and `subheader` are built by two new total functions beside `render::title`, each dropping
an empty part along with its separator. `body` is `event.detail`, the bare text, because `detail`
stays branch-prefixed for the banner and the branch is now in the header. `thread_id` is always
present and empty until section 4 fills it, for the reason the failure modes below give. The old
keys stay so that a gateway route nobody has updated keeps rendering.

The route's prompt becomes:

```yaml
prompt: |-
  **{header}**
  -# {subheader}
  {body}
```

#### Storage

Migration step 9 adds one table:

```sql
CREATE TABLE sessions (
  id TEXT PRIMARY KEY,
  harness TEXT NOT NULL,
  project TEXT NOT NULL,
  branch TEXT NOT NULL,
  title TEXT NOT NULL,
  first_seen INTEGER NOT NULL,
  last_seen INTEGER NOT NULL);
```

The `prompt` hook upserts the row, writing `title` only when the row has none, so the FIRST prompt
of a session is the one that names it and a later prompt does not rewrite the label. Every other
event touches `last_seen` and reads `title`.

A per-session file under the state directory was the alternative, and it loses on sweeping: the
marker families that live there each carry a sweeper, and one row replaced in place needs none.
The cost of the table is one migration step and one upsert on the synchronous prompt hook. That
hook's budget is the same one `ArmNag` measured and documented (`arm_nag.rs:19-31`): local disk
work only, no network, no spawn. The header pull request measures it the same way, 500 runs each
side.

#### Failure modes

- **No repository.** `git` answers nothing, the project falls back to the cwd basename and the
  branch slot is empty. The header degrades to `dotfiles · done`.
- **Detached head.** `branch --show-current` is empty, the worktree basename takes the slot.
- **A wedged git.** Both reads run under the existing five second deadline (`git.rs:17-19`), and a
  timeout is an empty string, never a held notification.
- **No session row.** A session whose prompt hook never ran (a Codex session, a hook installed
  mid-session) has no title. The subheader drops the title and its separator and reads
  `codex · 7f31`.
- **A title with a newline or a control byte.** The title is flattened with the existing
  `harness::flattened` before it is stored, so a pasted multi-line prompt cannot become two Discord
  lines or two state-file lines.
- **A route template that names a key pns does not send.** Renders as the literal `{key}` in the
  channel. This is why every key is always present and empty rather than conditionally omitted.

#### What the phone card shows

Unchanged. The moshi card is `{"token", "title", "message"}`
(`pns/crates/pns-adapters/src/destinations/moshi.rs:72-96`) where `title` is `render::title` and
`message` is the 260 character preview (`render.rs:8`). An iOS notification title shows roughly
forty characters before it truncates, and `dotfiles · feat/posture-alert-cutover · blocked` is
already past that, so putting the header in the card title would cut the state off the end, which
is the one word the card exists to deliver.

The card therefore keeps `agent · state · project` as its title. The card's MESSAGE already begins
with the branch, through `render::message`, so the phone gains the branch and loses nothing. The
session title is a Discord-only field: the phone card is a glance, the Discord line is a log.

#### What tests pin

One behavior per test, red first.

1. `render::header` orders project, branch, state.
2. `render::header` drops an empty branch with its separator rather than leaving `dotfiles ·  · done`.
3. `render::subheader` drops an empty title with its separator.
4. The short session is the first four characters of the session id.
5. `project_name` reads the git common directory, so a linked worktree reports `dotfiles` and not
   the branch slug.
6. The branch slot falls back to the worktree basename when the head is detached.
7. `parse_payload` reads `prompt` and `session_title`.
8. The prompt hook stores the FIRST prompt as the title and a later prompt does not overwrite it.
9. A Stop with no prompt in its payload still carries the title, because it is read from the store.
10. The hermes body carries `header`, `subheader`, `body` and `thread_id` as empty strings rather
    than omitting them, for an event that knows none of them.
11. The moshi card title is byte for byte what it was before the header existed.

### 2. One Discord thread per session

#### What the installed hermes can and cannot do

Verified by reading the gateway and the Discord plugin, not by inference:

- **Posting into a thread whose id is already known: supported.** A route's `deliver_extra` may
  carry `thread_id`, which the gateway passes to the adapter as
  `metadata={"thread_id": ...}` (`gateway/platforms/webhook.py:1016-1022`), and the Discord
  adapter's `send` fetches that thread and posts into it in preference to the channel
  (`plugins/platforms/discord/adapter.py:1684,1703-1715`).
- **Driving that per message: supported.** `deliver_extra` values are template-rendered against the
  posted body before delivery (`webhook.py:617-619,881-891`), so a route declaring
  `thread_id: '{thread_id}'` takes the thread from each POST rather than from static config. An
  empty string is falsy at `webhook.py:1017`, so the same route falls back to the channel.
- **Creating a thread: NOT supported on this path.** For a normal text channel the adapter's
  `send` has no create branch at all. A forum channel (type 15) auto-creates one thread per message,
  named from the message's first line (`adapter.py:1725,1801-1831,6699-6706`), which is one thread
  per EVENT, not per session.
- **Learning a created thread's id: NOT supported.** A `deliver_only` route answers
  `{"status", "route", "target", "delivery_id"}` and drops the adapter's `raw_response`, which is
  where the thread id would be (`webhook.py:643-652`). The same gap rules out the edit-in-place
  alternative, which needs a message id back for exactly the same reason.
- hermes does have a `create_thread` action, but only as an AGENT tool
  (`tools/discord_tool.py:427,486,513`), reachable by running a model over the payload. That is a
  model call in front of every notification, which the `deliver_only` design exists to avoid.

#### Verdict: threads wait on hermes

pns can address a thread it already knows and can never obtain one. One thread per session is
therefore **not buildable against hermes v0.17.0 through supported configuration**, and hermes is
not ours to patch.

Two upstream asks would unblock it, either one alone being enough:

1. Return the adapter's `raw_response` on a `deliver_only` response, so a POST that created a
   thread tells the caller its id.
2. Accept `deliver_extra.thread_name`, creating or reusing a thread of that name in the target
   channel, so the caller never needs an id at all. This is the better ask: it is idempotent, it
   survives pns losing its mapping, and it needs no round trip.

#### The pns side, ready for either

The work that does not depend on hermes is already in the header: `thread_id` is a key pns sends on
every POST, empty today. When hermes gains either ask, pull request 4 (PR 4) adds one column and
one rule.

```sql
ALTER TABLE sessions ADD COLUMN thread TEXT NOT NULL DEFAULT '';
ALTER TABLE sessions ADD COLUMN thread_seen INTEGER;
```

- The first event of a session posts with an empty `thread_id` and a `thread_name` equal to the
  header's first line. The response names the thread; pns stores it and stamps `thread_seen`.
- Every later event posts with the stored `thread_id`.
- **Archival.** Discord auto-archives a thread after its configured idle window, and hermes's own
  `create_thread` action defaults that to 1440 minutes (`tools/discord_tool.py:713`). pns treats a
  thread as archived when `thread_seen` is older than the same 24 hours, and posts to the channel
  with the thread name again rather than to the id. Posting into an archived thread is not an error
  in Discord (it unarchives), so the age rule is an optimisation, not a correctness gate.
- **A thread that vanished.** The adapter answers "Thread N not found" (`adapter.py:1715`), the
  gateway turns that into a 502, and the existing retry classification treats it as a rejection. The
  rule: on a rejection naming a thread, clear the stored thread and retry once against the channel.
  A message in the wrong place beats a message nowhere, which is the same trade `channel_url`
  already makes for an unusable route name (`hermes.rs:44-49`).

#### The nearest thing that works today, and why not to build it

One channel per project is supported with no hermes change: a route per project, each with its own
`chat_id`, and pns naming the route on `EventArgs.channel`, which the submitted-request path already
carries end to end (`mapping.rs:24-27`). A route the gateway does not have answers 404, and the
recap's existing fallback shape (`post_return_recap.rs:47`) is the pattern for landing on `pns`
instead with a line saying why.

It is not recommended. It costs an encrypted-config edit and a hand-made Discord channel per
repository, it needs `run_after_68`'s expectation updated on every new project, and it buys grouping
by PROJECT when the thing being grouped is a SESSION: four agents in four worktrees of `dotfiles`
still land in one channel, which is the problem being solved. The header alone already turns that
channel into four distinguishable streams.

Recap delivery is unchanged by all of this: it keeps its own `pns-recap` route, and agent events
never go to `priority`.

#### What tests pin (PR 4, when hermes allows it)

1. A session's first event posts with an empty thread and the header's first line as the name.
2. The id the response names is stored against that session.
3. A later event of the same session posts with the stored id.
4. A session whose `thread_seen` is older than the archive window posts by name again.
5. A rejection naming a missing thread clears the stored id and retries against the channel.

### 3. The stale-block escalation

A session blocked for sixty minutes with nothing resolving it gets ONE message on `priority`, once
per block, and only when the operator could act on it.

#### Why not a query over `ledger_events`

The ruling asks for the exact query. There is not one, and the reason is structural rather than a
missing index: `ledger_events` has no session column and no timestamp
(`ledger/schema.rs:8-14`). It is a delivery ledger, its clock lives on its legs and attempts, and
nothing prunes it, so it grows for the life of the machine. Making it answer this question would
cost two new columns, a backfill that cannot be done, and a growing scan on every tick.

The question is about SESSIONS, and section 1 builds the sessions table for a different reason. Two
columns on it answer it exactly:

```sql
ALTER TABLE sessions ADD COLUMN blocked_since INTEGER;
ALTER TABLE sessions ADD COLUMN escalated_at INTEGER;
```

The query, run by the job described below:

```sql
SELECT id, harness, project, branch, title, blocked_since
  FROM sessions
 WHERE blocked_since IS NOT NULL
   AND blocked_since <= :now - :window
   AND escalated_at IS NULL;
```

`blocked_since` is set by the same event that starts a wait and cleared by the same event that ends
one, which is the rule `blocked_marker_action` already states
(`lights/phase.rs:148-154`) over `pulse::LAMP_BLOCKED` (`pulse.rs:127`). **Reuse that constant
rather than writing `blocked`, `asked`, `denied` a second time**: `plan-ready` and `asking` are
waits by the same definition, and a second list is a second thing to keep in step.

The existing blocked MARKER is deliberately not reused: it is written only when the lights are
configured (`blocked.rs:65-66`, `event_flow/records.rs:47-50`), so an escalation built on it would
be silently dead on a machine with no `[lights]` table.

#### One leased job on the existing clock

Mirror `ArmNag` exactly, as a sibling and not a parameter on it. `ArmStaleEscalation` in
`pns-application`:

- Arms on every wait-starting state, from `hook_dispatch.rs`'s `asked | plan-ready | denied` arm and
  from `moshi_submission::blocking_event`, so all five wait states are covered where the nag covers
  one.
- Arms for every harness. The nag's claude-only gate (`arm_nag.rs:46-54`) exists because Codex has
  no batch-level clearing signal and its turns run for tens of minutes, which would make a
  five-minute nudge wrong in the common case. A sixty-minute window is past any normal turn, so the
  gate does not carry over.
- Schedules `Job { id: "stale:<session>", due: now + window, until: due + window, every: None,
  unless_marker: Some(nag::marker_name(session)), args: vec!["stale"] }`. The lease is one more
  window past due, for the reason `arm_nag.rs:112-118` gives: a machine that slept through the
  window never spawns at all.
- Shares the nag's ANSWERED MARKER rather than minting a second one. Both timers are armed by the
  same event and cancelled by the same answer, and `ArmNag` already clears that marker before it
  arms (`arm_nag.rs:71-90`), which is correct for both.

The job runs `pns stale`, a new subcommand beside `pns nag`, taking no arguments for the same reason
(`command_nag.rs:31-34`): coalescing means a session argument could not be honoured.

#### The gate: only when the operator can act

`pns stale` reads the surface once, from the same probe set every event path uses
(`surface.rs:141-169`), and stays silent when either holds:

- **Away.** `Surface::Away` means neither the desk nor the phone has a fresh reading. The operator
  is not at a keyboard, and the thing a blocked agent needs is a keyboard. The morning recap is what
  carries it instead.
- **Locked for the whole window.** `screen_locked == Some(true)` AND the desk idle age is at least
  the window. One instantaneous reading answers a question about an hour, because unlocking a Mac
  takes input and input resets the idle clock: a desk idle for sixty minutes cannot have been
  unlocked inside them. Only `Some(true)` locks, matching `surface.rs:155`, so an `ioreg` that
  stops answering costs the suppression rather than the escalation.

A screen locked for part of the window still escalates, which is the intended direction: the
operator was there, stepped away, and the page is what tells them a session is stuck.

#### The dedup marker

`escalated_at` on the session row is the marker, and the column is the whole rule. It is stamped
when the page is attempted, and cleared by the same event that clears `blocked_since`. So:

- One page per block. The job is a one-shot (`every: None`), and a second tick that somehow reached
  the same row finds `escalated_at` set and skips it.
- Silent until the block resolves. Nothing re-arms while `escalated_at` stands.
- A new block after a resolve arms a fresh job, whose id replaces any leftover by rename
  (`jobs.rs:26-28`).

**Stamped on attempt, not on success**, which matches the nag's own honesty
(`command_nag.rs:60-64`): a mute, a Focus or an empty plan can suppress delivery, and a page that
retried every hour because the first one was muted is the failure mode worth avoiding.

#### The page

Route `priority`, fixed and not configurable: ruling 4 defines `priority` as machine health and
security plus this escalation, so naming another route would be a config that contradicts the
definition. The body is the ordinary event body of section 1, with the header's own fields, so the
`priority` route needs the same prompt template as `pns`. The detail reads
`blocked 63 minutes, no answer` with the elapsed time computed from `blocked_since`.

#### Failure modes

- **No clock.** `now_secs()` answers nothing, the job does nothing and says so on stderr, exactly as
  `nag_mode` does (`command_nag.rs:46-51`).
- **The daemon is stopped.** No job fires. That is the stated cost of the clock across every rider,
  and the session still appears in the morning recap.
- **A session that ended without an event.** Its row keeps `blocked_since` and it pages once. The
  page is correct: nobody answered it. The lease then prevents a second.
- **The window shortened in config between arm and fire.** The job's own `due` is what fires, so a
  shortened window takes effect on the next block, not retroactively. A window set to zero means
  off, and the fire drops any row it finds, matching `nag_mode`'s reading of a feature turned off
  between arming and firing (`command_nag.rs:37-45`).
- **Two jobs woken in one tick.** The window claim `nag_mode` already uses
  (`command_nag.rs:12-19`) is the pattern: one claim, then each row claimed by its own `escalated_at`
  write inside the transaction.

#### The recap

`pns recap` gains one section listing every session with `blocked_since` set, ordered oldest first,
each as its header line plus how long it has waited. That is what makes "skip when away" safe: the
escalation is suppressed, never lost.

#### What tests pin

One behavior per test, red first.

1. A wait-starting event stamps `blocked_since` and schedules a `stale:<session>` job.
2. Every state in `LAMP_BLOCKED` arms it, and a `done` event clears `blocked_since`.
3. It arms for codex, where the nag does not.
4. A block older than the window with `escalated_at` unset is selected by the query.
5. A block younger than the window is not selected.
6. A session whose wait was answered is not selected, because the marker drops the job.
7. The fire stamps `escalated_at`, so a second fire selects nothing.
8. A new block after a resolve clears `escalated_at` and is selected again.
9. `Surface::Away` suppresses the page and leaves `escalated_at` unset, so it pages once the
   operator is back.
10. A screen locked with a desk idle at or past the window suppresses the page.
11. A screen locked with a desk idle inside the window still pages.
12. The page names the `priority` route.
13. The page's detail states how long the block has stood.
14. The recap lists a session still waiting, oldest first.

### 4. Routes

The route decisions of 2026-09-14, recorded here for the implementation that follows.

| Route | Who posts | What |
| ----- | --------- | ---- |
| `pns` | pns hook and daemon paths | Every routine agent event. |
| `priority` | posture, pns | Machine health and security, plus the stale-block escalation. |
| `posture` | posture | Non-critical pages and the digest. |
| `uu` | uu | The weekly record. Renamed from `unattended-upgrades`, already in flight. |
| `pns-recap` | pns | The return recap. |

Changes this implies:

- **`unattended-upgrades` becomes `uu`.** One constant
  (`uu/crates/uu-adapters/src/config/records.rs:17`), one route in the gateway config, and
  `run_after_68`'s list and its comment, which names the old spelling twice
  (`run_after_68-hermes-log-route-status.sh.tmpl:17,21-22`). The old route is left in the gateway
  config for the operator to delete by hand; this repository ships no removal mechanism.
- **`posture` and `pns-recap` are added to the gateway**, each with the pns signing secret,
  `deliver_only: true`, the section 1 prompt template, and its own `chat_id`. Both are named by code
  that already shipped, so both are refused today.
- **`run_after_68`'s `expected_routes` becomes `(pns priority posture uu pns-recap)`.** Nothing
  tests that list against the gateway config or against the code that names the routes: that is
  declaration-consistency checking, out of scope since 2026-08-05, and the script itself is the
  check that runs at apply time.
- **posture branches on severity.** Today every command hard-codes `posture`
  (`alert.rs:65`, `digest.rs:54`, `funnel.rs:37`, `heartbeat.rs:40`, `poll.rs:119`,
  `watchdog.rs:50`). The alert path takes the finding's severity and names `priority` for a critical
  finding and `posture` for everything else; the digest, heartbeat, poll and funnel paths name
  `posture` unconditionally. The watchdog keeps probing `priority`, since that is the route whose
  health it is reporting on.

## Implementation plan

Four small pull requests, in this order. Each is independently shippable and each leaves the channel
in a working state.

### PR 1: the sender header

pns sends attribution; the `pns` route renders it.

- `HookPayload` gains `prompt` and `session_title`; `parse_payload` reads both.
- Migration step 9 adds the `sessions` table; the prompt hook upserts the row.
- `project_name`, `worktree_name` and `session_short` beside the existing `git_branch`.
- `render::header` and `render::subheader`; `EventArgs` and `Event` gain the two composed lines and
  the session id; `hermes_body` gains `header`, `subheader`, `body` and an always-empty `thread_id`.
- The four `hook_dispatch.rs` arms gain the branch they do not set today.
- Operator step: the `pns` route's prompt in the encrypted hermes config, then an apply.

Tests: the eleven listed in section 1.

### PR 2: the stale-block escalation

- Two columns on `sessions`; `ArmStaleEscalation`; the `pns stale` subcommand; the surface gate; the
  recap section.
- `[nag] stale_after_secs`, default 3600, zero meaning off, shipped uncommented at its default per
  the config hygiene ruling.
- Operator step: the `priority` route gains the section 1 prompt template, then an apply.

Tests: the fourteen listed in section 3.

### PR 3: the routes

- `uu`'s default record URL; posture's severity branch; `run_after_68`'s expectation and comment.
- Operator step: add `posture`, `pns-recap` and `uu` to the encrypted hermes config, each with the
  pns signing secret and the section 1 template, then an apply. The old `unattended-upgrades` route
  is listed for the operator to remove by hand.

Tests: posture sends a critical finding to `priority`; posture sends a non-critical finding to
`posture`; posture sends the digest to `posture`; uu's default record URL names `uu`. No test pins
`run_after_68`'s list.

### PR 4: one thread per session

**Blocked on hermes.** Ships when either upstream ask in section 2 lands. Until then the `thread_id`
key pns already sends stays empty and the channel reads as one headed line per event.

Tests: the five listed in section 2.

## Out of scope

- Patching hermes. The two upstream asks are filed, not implemented here.
- Per-project Discord channels. Designed in section 2, deliberately not built.
- Adding `session` or `occurred_at` columns to `ledger_events`. The sessions table answers the
  question the escalation asks, and the ledger keeps being a delivery ledger.
- Removing the `unattended-upgrades` route, or any other deployed leftover. Listed for the operator.
- Making `pi` and `omp` produce pns events. They reach `pns gate`, which forwards to moshi.
- A second nag cadence, a snooze, or a per-session mute for the escalation.

## Open questions for the operator

1. **The state in the header's first line.** Layout A reads
   `**dotfiles · feat/posture-alert-cutover · blocked**`, putting the one word that says what
   happened beside the project and branch. The ruling's example line showed project and branch
   alone. Confirm the state belongs there, or say where it should go instead.
2. **The title cap.** Sixty characters is the working default: it fits a Discord line on a phone
   without wrapping and holds a typical first prompt. Say if a longer or shorter cut reads better
   once it is in front of you.
3. **Which upstream ask to file first.** `thread_name` on `deliver_extra` is the better design and
   the larger change; returning `raw_response` on a `deliver_only` response is three lines and
   unblocks the same feature with more bookkeeping on the pns side.
