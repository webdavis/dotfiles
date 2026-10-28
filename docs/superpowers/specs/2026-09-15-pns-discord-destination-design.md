# The direct Discord destination and the one project channel map

Status: design, written 2026-09-15. Nothing built. Every choice made in the operator's place is under
"Assumptions", and "Open questions" holds only what the operator must answer. Every Discord HTTP
claim carries the documentation URL it was fetched from on 2026-09-15; every pns claim carries the
file it was read in, in this checkout.

## What the operator ruled, and what it displaces

Rulings of 2026-09-14 and 2026-09-15, which this design implements rather than revisits:

- Every channel is `#<project>-<stream>`, never bare. Each project has `#<project>-dev` carrying that
  repository's CI, pull requests, GitHub notifications and agent session threads, for `dotfiles`,
  `pns`, `uu`, `posture`, `homelab`, `justdavis-ansible`, `essential-feed-case-study`, `scalebar`,
  `netpulse`, `plantpulse` and `casually-concerned`. `#github-notifications` is the catch-all for a
  repository with no channel, and `#priority` is the severity channel for anything critical from any
  source, severity outranking subject.
- The notification channels are `#pns-events`, `#uu-runs`, `#posture-pages` and `#general`.
  `#pns-recap` and `#uu-failures` are deleted.
- Repository and project channels are delivered by pns's OWN Discord bot, never by a hermes route,
  and no interim hermes `github` route is to be added.
- ONE `repo -> channel entry` map in the pns config, shared by the GitHub source and by session
  events. The kind of a message lives in its header, not in a second channel.
- Recaps move from `#pns-events` to the project's own channel once the bot ships.

This supersedes two things already written down. The GitHub source design
(`docs/superpowers/specs/2026-09-14-pns-github-source-design.md`, merged as #620) names its channels
`#github-<repo>` and `#github`, and puts the map at `[plugins.github.channels]`; both are replaced
here, and its Decision 5 interim hermes `github` route is cancelled outright. Its polling, event
shape and colour sections stand untouched. The session attribution design
(`docs/superpowers/specs/2026-09-14-pns-session-attribution-and-threads-design.md`, merged as #593)
concluded that one thread per session is "not buildable against hermes v0.17.0 through supported
configuration"; the bot is exactly the transport that makes it buildable, and its section 2 becomes
this document's section 3.

## The bot, as it exists today

A Discord application named `pns`, created and OFFLINE. Permissions: View Channels, Send Messages,
Create Public Threads, Send Messages in Threads, Embed Links, Read Message History. Create Private
Threads unchecked, no Manage Messages, no Manage Threads, no privileged intents, Public Bot off,
Install Link None, guild install only. Its vault entries, by exact title:
`Discord (Uriel) :: Bot Token (pns)`, `Discord (Uriel) :: Public Key (pns)` and
`Discord (Uriel) :: Application/User ID (pns)`. Channel ids are
`Discord (Uriel) :: Channel ID (#<name>)` and already exist for every `#<project>-dev` and for
`#github-notifications`.

Two consequences bind the design. **No Manage Threads means the bot cannot unarchive or unlock a
thread it did not create**, which decides section 3's failure path. **No gateway intent means the bot
never connects a websocket**: it is an HTTP client that posts and reads nothing back except its own
responses, which is why no daemon-resident connection appears anywhere below.

## 1. The destination plugin

### Its table and its shape

```toml
[plugins.discord]
enabled = true
type = "bot"
token = { keepassxc = "Discord (Uriel) :: Bot Token (pns)", field = "Password" }

[plugins.discord.channels]
default = { keepassxc = "Discord (Uriel) :: Channel ID (#github-notifications)", field = "Password" }
"pns-events" = { keepassxc = "Discord (Uriel) :: Channel ID (#pns-events)", field = "Password" }
priority = { keepassxc = "Discord (Uriel) :: Channel ID (#priority)", field = "Password" }
dotfiles = { keepassxc = "Discord (Uriel) :: Channel ID (#dotfiles-dev)", field = "Password" }
```

`type = "bot"` mirrors `[plugins.mobile] type`, whose layout comment already states the rule: a table
naming no backend, or naming one nothing answers, is refused out loud rather than read as the only
one there is (`pns-adapters/src/config/render/layout/destinations.rs`). The registry name is
`discord`, added to `ROSTER` in `pns-domain/src/registry/roster.rs` with hermes's declaration exactly,
`Routing { local: false, presence_gated: false, durable: true, event_dispatched: true }`: not local,
not presence gated, the durable log, reached by every event.

### Beside `[plugins.hermes]`, and what happens when both are on

**Both enabled at once must be refused at config load, naming both tables.** This is not caution, it
is a measured duplicate. `channel_plan` (`pns-domain/src/routing.rs:74-125`) filters the selection on
declarations and keeps EVERY plugin whose declaration passes, so two durable channels produce two
legs and one event reaches Discord twice. `DeliveryScope::RemoteOnly` narrows to `routing.durable`,
which is both of them. Worse quietly: `Destinations::durable()`
(`pns-application/src/destinations.rs:120-124`) answers the FIRST durable entry in registration
order, and `deliver_recap` (`pns/src/recap_delivery_runtime.rs`) posts the recap through exactly that
call, so the recap would silently pick whichever of the two was registered first while every other
event doubled.

So the cutover is a two line config edit in one apply: `[plugins.hermes] enabled = false` and
`[plugins.discord] enabled = true`. The refusal belongs with the roster's existing ones, which already
refuse an unknown plugin name and block the whole file (`pns-domain/src/registry.rs`). pns keeps
compiling both destinations, so a rollback is the same two lines the other way.

Nothing about hermes is deleted: posture and uu post to hermes routes with their own clients and know
nothing about pns (2026-09-14), so `#posture-pages` and `#uu-runs` keep arriving through the gateway
however pns's own durable leg is configured.

### The token

The values file names the vault entry; `just pns-config-render` writes
`token = {{ (keepassxc "Discord (Uriel) :: Bot Token (pns)").Password | toToml }}` into
`dot_config/pns/private_config.toml.tmpl`, exactly as `[plugins.mobile] token` and the four hermes
keys already do (lines 30 and 54 to 58 of that file). The loaded value lives in a type deriving no
`Debug`, for `HermesKeys`'s stated reason: "the keys never enter a type that derives one, so they
cannot ride a formatted dump into a log line" (`pns-adapters/src/config/hermes.rs`), and it never
reaches argv, a child environment or a printed line. A missing or empty token is the not-set-up state
and produces `Delivery::Failed` naming the config key, which is `HermesChannel`'s `skipped_line`
behaviour and reasoning (`pns-adapters/src/destinations/hermes.rs:139-152`): an empty Discord channel
otherwise looks like the jobs stopped.

### The HTTP calls

Base `https://discord.com/api/v10`, version 10 being the recommended one
(`docs.discord.com/developers/reference`).

- **Post a message:** `POST /channels/{channel.id}/messages`, body `{"content": ...}`
  (`docs.discord.com/developers/resources/message`).
- **Open a session thread:** `POST /channels/{channel.id}/messages/{message.id}/threads`, body
  `{"name": ..., "auto_archive_duration": 1440}` (`docs.discord.com/developers/resources/channel`).
- **Post into a thread:** `POST /channels/{thread.id}/messages`, because a thread IS a channel and
  carries its own id.

Headers on every call: `Authorization: Bot <token>`, `Content-Type: application/json`, and
`User-Agent: DiscordBot (https://github.com/webdavis/dotfiles, <pns version>)`. The User-Agent is
**mandatory**: "requests without a valid User-Agent may be blocked and return Cloudflare errors"
(`docs.discord.com/developers/reference`). `allowed_mentions` is sent as `{"parse": []}` on every
post, because the default for a regular message "parses all mention types (users, roles, everyone)"
(same message page) and an agent pasting a branch name containing `@everyone` must not page the
guild.

`auto_archive_duration` takes 60, 1440, 4320 or 10080 minutes (channel page). 1440 is chosen in
section 3.

### 429, 5xx and a dead network

**The existing classification is already correct and gains nothing.** `DeliveryOutcome::class`
(`pns-domain/src/retry.rs:64-82`) puts 408 and 429 and every 5xx in `Temporary` and everything else
in `Permanent`, and `RetryLimits::verdict` dead-letters a permanent failure on the FIRST attempt
rather than the twentieth. Mapped onto Discord's own table
(`docs.discord.com/developers/topics/opcodes-and-status-codes`):

| Answer | Class | What the operator sees |
| --- | --- | --- |
| 401, the header missing or invalid | Permanent | dead-lettered at once, naming the token's key |
| 403, or 404 code 10003 unknown channel | Permanent | dead-lettered at once, naming the channel key |
| 429 rate limited | Temporary | retried on the ledger's linear backoff |
| 5xx, or nothing answered (`NoResponse`) | Temporary | same |
| a malformed URL (`NoStatus`) | Permanent | dead-lettered at once |

A dead-lettered leg is a row in `pns failures`, which is the refusal-and-ledger pattern this engine
already reports every other destination's refusals through (`pns/src/command_failures.rs`).

**No `Retry-After` scheduler is built.** A 429 carries a `Retry-After` header and a body with
`retry_after`, `global` and `message` (`docs.discord.com/developers/topics/rate-limits`). The ledger's
schedule is linear at 60 seconds per attempt (`RetryBackoff`, same file), which already exceeds every
per-route `retry_after` this traffic can earn, and honouring the header would mean a second scheduler
disagreeing with the ledger about when a leg is due, so the value is logged and not obeyed. The
number worth recording is the ban threshold: an IP is temporarily restricted after 10,000 invalid
requests (401, 403 or 429) per 10 minutes, and "429 errors returned with `X-RateLimit-Scope: shared`
are not counted against you" (same page). A few hundred notifications a day cannot reach that, and a
permanent 401 dead-letters on its first attempt rather than retrying into it.

## 2. The channel map

### Lookup order

One table, `[plugins.discord.channels]`, consulted in this order, first hit wins:

1. **The event's route, when it named one that is not `DEFAULT_ROUTE`.** An event submitted with
   `--channel priority`, or by a producer whose `severity_route` says `priority`
   (`pns-domain/src/routes.rs`, `pns-domain/src/stale.rs`), takes the `priority` entry. This is the
   severity override, and it is not new machinery: the route already crosses the CLI, the submitted
   request and the composition root (`pns/src/channel_dispatch.rs:141-166`).
1. **`owner/name`**, the full repository name, when the event carries one.
1. **The bare project name**, which is `event.project`: the repository basename, computed from
   `git rev-parse --git-common-dir` for every event by the session attribution design, falling back
   to the cwd basename outside a repository.
1. **`pns-events`** when the event has no project at all.
1. **`default`**, which is `#github-notifications`.

Steps 2 and 3 are one rule with two spellings: GitHub only ever knows `webdavis/dotfiles`, and pns's
own events only ever know `dotfiles`. Trying the full name first means the day `git subtree split`
produces `webdavis/pns` beside a `pns` directory in this checkout, the two are separable by writing
one key; until somebody writes one, both spellings land on the same bare entry.

Steps 4 and 5 are deliberately different channels. "I know which repository and have no channel for
it" is a GitHub-shaped miss and belongs in the catch-all; "there is no repository" is an engine event
and belongs in `#pns-events`. Two failures, two places to look. `default` is a REQUIRED key and a map
without it is refused at load, for the reason the GitHub design gave and this one keeps: a map with no
catch-all silently swallows the first event from every new repository.

### Why the map holds entry names and not ids

The ruling is explicit that secrets reach the config only as keepassxc references, never a value and
never a printed id. The values file is COMMITTED, so an id written there is a permanent record of a
private guild's layout in a public repository, and the generator already refuses anything but
`{ keepassxc = ..., field = ... }` in a secret-bearing key (`dot_config/pns/config-values.toml`,
header comment). Rotation then stays a vault edit plus an apply rather than a commit, the way the
four hermes secrets already work.

### Schema

`config/schema.rs` lists allowed keys per table and refuses the rest by name. Repository names are
open, so `plugins.discord.channels` joins `TARGET_KEYS` as a wildcard row rather than an enumerated
one; `[plugins.hermes.keys]` cannot be the model, because its vocabulary is
`pns_domain::routes::ROUTES`, a closed roster. The layout table
(`config/render/layout/destinations.rs`) declares `plugins.discord` with `plugins.discord.channels`
as a child, the way `PLUGINS_HERMES` declares `PLUGINS_HERMES_KEYS`.

## 3. Session threads

### Create versus reuse

The first event of a session that reaches a given channel posts to the CHANNEL, reads the message id
out of the returned message object, and calls `POST /channels/{channel}/messages/{message}/threads`
with `name` set to the header's first line (`dotfiles · feat/x · blocked`, capped at Discord's 100
character channel name limit) and `auto_archive_duration: 1440`. Every later event of that session in
that channel posts to `POST /channels/{thread}/messages`.

### Where the id lives

A new table at migration step 10 (`pns-adapters/src/persistence/sqlite/migrations.rs`, `VERSION` 9
today, step 9 being `sessions::create`):

```sql
CREATE TABLE session_threads (
  session TEXT NOT NULL,
  channel TEXT NOT NULL,
  thread  TEXT NOT NULL,
  PRIMARY KEY (session, channel));
```

**Keyed on the pair, not on the session**, which is what makes a session spanning two repositories
correct rather than surprising: an agent that starts in `dotfiles` and posts an event from a `pns`
worktree resolves a different channel, finds no row for that pair, and opens a second thread there.
One session, two threads, each in the channel its events were actually about. A single column on
`sessions` would have posted the second repository's events into the first repository's channel. It
is a table rather than a state-directory file for `sessions`'s own recorded reason: every marker
family there carries a sweeper, and a row replaced in place needs none
(`pns-adapters/src/persistence/sqlite/sessions.rs`). Rows are never swept, on that file's own
accepted-cost argument.

### When the thread is gone

**Archival needs no handling at all.** "Sending a message will automatically unarchive the thread,
unless the thread has been locked by a moderator" (`docs.discord.com/developers/topics/threads`). The
24 hour age heuristic the hermes-era design carried is therefore deleted rather than ported: it would
be code guarding a case Discord already handles.

A deleted thread answers 404 with code 10003 "Unknown channel"; a locked one answers 50083 or 160005
"Thread is locked" (opcodes page), and the bot holds no Manage Threads permission, so it cannot
unlock one. All three take one path: **clear the stored row, repost that event to the channel once,
and open a fresh thread from it.** The retry happens inside the destination, before the ledger records
an outcome, so the event is delivered rather than dead-lettered. It is the trade `channel_url`
already makes for an unusable route name, in that function's own words: a message in the wrong place
beats a message nowhere (`pns-adapters/src/destinations/hermes.rs:82-90`).

## 4. Recap migration

`pns recap agent --stdin` fits the body and hands it to `post`, which builds an `EventArgs` with
`agent: "pns"`, `state: "recap"` and no project, then posts through `Destinations::durable()`
(`pns/src/command_recap.rs`, `pns/src/recap_delivery_runtime.rs`). The whole change is one field:
`deliver_recap` fills `project` from the cwd, which it is already positioned to do because
`git_recap` spawns `pns_adapters::git_facts(&cwd)` two functions away. The Discord destination then
resolves the channel through section 2's ordinary lookup, with no recap-specific branch anywhere.

A recap composed outside a repository has no project and lands on the `pns-events` entry, which is
`#pns-events`: the same channel it goes to today, so the no-project case is unchanged rather than
degraded.

**A recap opens no thread**: it is a window of time rather than a session, and threading it would bury
the one message a day the operator most wants at channel level.

## 5. Message shape

Plain `content`, not an embed.

```
**dotfiles · feat/posture-alert-cutover · blocked**
-# claude · a1b2 · arm posture alert and retire the Bash alerter
Bash(git push --force-with-lease) needs approval
```

Line one is `render::header(project, branch, state)` and line two is
`render::subheader(agent, short_session(session), session_title)`, both already written and already
posted as fields on every hermes body (`pns-domain/src/render.rs:38-73`,
`pns-adapters/src/destinations/hermes.rs:48-74`). The bot composes the three lines itself instead of
posting fields a gateway template assembles, which is the one simplification the direct transport
buys: there is no second renderer to drift from this one, and `joined`'s rule that an empty part takes
its separator with it already prevents `dotfiles ·  · done`.

**The kind lives in the header, per the ruling.** A GitHub event's first line reads
`dotfiles · lint · failed` and a session event's reads `dotfiles · feat/x · blocked`: same three
slots, same scan order, one channel.

**Not an embed**, though the bot holds Embed Links. An embed costs a colour decision this design has
no locked answer for, does not render in the mobile push preview where the first line matters most,
and would abandon the layout the session design already settled. It stays the upgrade path for the
GitHub source's purple and orange pair, which is where a colour belongs.

**The ceiling is 2000 characters for `content`** (message page). pns already budgets at 1800
(`pns-domain/src/recap/budget.rs`, `MAX_CHARS`, quoted in `recap/agent.rs`), so that number stays and
the 200 character margin stays with it. The shed rule mirrors the recap's: header and subheader are
never shed, whole lines are dropped from the END of the body and the count of dropped lines is
appended, and no line is ever cut in half. `recap::agent::fitted` is NOT reused for events: it sheds
by recap section heading, which an event body has none of. It is unchanged and keeps serving the
recap.

## Assumptions

1. **`[plugins.discord]` with `type = "bot"`**, on `[plugins.mobile]`'s precedent. *Alternative:* no
   `type` key, which saves a line and makes a second Discord transport a new plugin name.
1. **Both durable destinations enabled at once is refused at load.** *Alternative:* let the first
   registered win, which is what `Destinations::durable()` does today and which silently doubles
   every event that is not a recap.
1. **The route beats the project when an event names one.** *Alternative:* a `priority_channel` key,
   a second mechanism for a lookup the route already performs.
1. **Full name, then bare name, then `pns-events`, then `default`.** *Alternative:* full name only,
   which no pns event can ever satisfy, or bare name only, which collides after the extraction.
1. **No project and unmapped project reach different channels.** *Alternative:* one catch-all, which
   loses the distinction between a repository nobody mapped and no repository at all.
1. **One thread per (session, channel) pair.** *Alternative:* one per session, which posts a second
   repository's events into the first repository's channel.
1. **No archive age heuristic, and a missing or locked thread reposts to the channel inside the
   destination.** *Alternatives:* the hermes-era 24 hour rule, which guards a case Discord's own
   unarchive-on-send already handles; and dead-lettering, which loses the event to a tidied thread.
1. **The 429 `Retry-After` is logged, not obeyed.** *Alternative:* a per-destination scheduler that
   can disagree with the ledger about when a leg is due.
1. **`allowed_mentions: {"parse": []}` on every post.** *Alternative:* Discord's default, which
   parses `@everyone` out of any text an agent quotes.
1. **Plain content, not an embed, and `auto_archive_duration: 1440`.** *Alternatives:* an embed, which
   needs a colour ruling and renders worse in a push preview; and 10080 minutes, which keeps a week of
   sessions in every project channel's active list.

## Open questions

1. **Does the hermes destination stay enabled after the cutover, or does the bot serve `#pns-events`
   too?** The map already has a `pns-events` entry, so the bot can; keeping hermes on as well is the
   one configuration this design refuses.
1. **Which projects get a full `owner/name` key on day one?** Only needed where a bare name will
   collide after the extraction; the answer sizes PR 2's map rows.
1. **Do agent session threads belong in `#<project>-dev` alongside CI, or does one thread per session
   per worktree crowd it?** The ruling says one channel; this is what would reopen it after a week.

## Build ladder

Four pull requests, each small, each independently testable. Tests are behavior of tools we wrote,
each under a second, and none of them talks to Discord: the HTTP client is behind the same kind of
seam `SignedPost` already puts in front of hermes (`pns-hermes/src/post.rs`).

**PR 1: the destination and its table.** `pns-adapters/src/destinations/discord.rs` plus a `discord/`
directory for its private parts; `pns-adapters/src/config/discord.rs` for the settings reader; the
`discord` row in `pns-domain/src/registry/roster.rs`; the `plugins.discord` rows in
`config/schema.rs` and `config/render/layout/destinations.rs`; the wiring in
`pns/src/channel_dispatch.rs`; `dot_config/pns/config-values.toml` and the regenerated template.
Behaviors: a token-less table refuses by name and posts nothing; a 401 dead-letters on its first
attempt while 429 and 5xx stay retryable; a config enabling both hermes and discord is refused naming
both; the token appears in no rendered failure line; the composed request carries the bot
authorization, the User-Agent and empty `allowed_mentions`.

**PR 2: the channel map.** A pure lookup in `pns-domain`, the wildcard schema row, the map rows in the
values file, the regenerated template. Behaviors: a named non-default route beats the project; a full
`owner/name` key beats the bare name; an unmapped project reaches `default`; an event with no project
reaches `pns-events`; a map with no `default` is refused at load.

**PR 3: session threads.** Migration step 10 and `session_threads`; create-or-reuse in the
destination. Behaviors: a session's first event to a channel posts to the channel and stores the
thread the response names; a later event to the same channel posts to the stored thread; the same
session in a second project opens a second thread; each of 10003, 50083 and 160005 clears the row and
reposts to the channel once; the thread name is the header's first line, capped at 100 characters.

**PR 4: the recap, the GitHub source and the runbook.** `deliver_recap` fills `project`;
`[plugins.github.channels]` is deleted in favour of the one map and the GitHub source resolves its
channel through it; `docs/runbooks/local-daemons.md` gains the bot's paragraph beside the gateway's,
naming the three vault entries and the two channels an operator adds when a project is created.
Behaviors: a recap with a project resolves that project's channel and opens no thread; a recap with
none reaches `pns-events`; a GitHub event and a session event about the same repository resolve the
same channel.

## Deliberately out

**The slash commands `/pns pending|approve|reject`.** When their turn comes they need, and the
operator has to provide: an Interactions Endpoint URL, "a public endpoint for your app where Discord
can send your app HTTP-based interactions" (`docs.discord.com/developers/interactions/overview`),
which means a hostname on the existing `agentmail-receiver` Cloudflare tunnel; Ed25519 verification
of the `X-Signature-Ed25519` and `X-Signature-Timestamp` headers against
`Discord (Uriel) :: Public Key (pns)`, answering 401 when it fails, and a `PONG` with `type: 1` to
Discord's PING (same page); guild-scoped command registration under
`Discord (Uriel) :: Application/User ID (pns)`; and, on pns's side, a WRITE path that can approve a
pending permission prompt, which does not exist today. `from training, not verified:` an interaction
must be acknowledged within about three seconds or Discord discards it; the overview page does not
state a limit, so measure it before designing around it.

**The push receiver through the Cloudflare tunnel.** Unchanged from the GitHub source design's PR 3
(a GitHub App, its webhook secret, a second ingress hostname, a Cloudflare Access Bypass policy over
GitHub's published source ranges, a LaunchAgent, and posture's own declared-hostname control). That
design owns it; this one only cancels the interim hermes `github` route it proposed.

**Any removal mechanism.** The hermes `pns-events` route and its vault entries stay deployed after the
cutover and are listed for the operator to trash by hand.
