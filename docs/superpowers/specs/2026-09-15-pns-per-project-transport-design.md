# Per-project transport: the bot or a hermes route

Status: design, written 2026-09-15. Nothing built. Every choice made in the operator's place is under
"Assumptions", and "Open questions" holds only what the operator must answer. Every claim about pns
carries the file it was read in, in this checkout.

## What the operator ruled, 2026-09-15

1. **The two transports are not the same job.** The bot posts. A hermes route can hand the
   notification to an agent that reads and interprets it, a capability the bot will never have. Both
   must be enabled at once, so the both-durable refusal has to go.
2. **The choice is per project.** Some repositories post straight to the bot; others route through a
   hermes webhook for interpretation. One transport for everything is not the model.
3. **One table, not two.** The transport is stated on the same entry that already names where a
   project's news goes, so the reader is never debugging which of two tables won:

   ```toml
   dotfiles = { keepassxc = "... Channel ID (#dotfiles-dev)" }
   netpulse = { route = "netpulse" }
   ```

4. **The table is renamed and moves.** It stops being "which Discord channel" and becomes "where this
   project's news goes", so it no longer belongs under `[plugins.discord]` when half its entries are
   not Discord.
5. **The bot's identity is its token, and nothing more.** No source file names a bot; the vault entry
   title `Discord (Uriel) :: Bot Token (pns)` is the operator's own filing, not a name pns knows. No
   bot name, application id or display name is introduced anywhere in this design.
6. **No mapping means no posting.** With no table at all, pns posts to Discord nothing. But an enabled
   destination with an empty or default-less map still refuses at load, because the operator had to
   write `enabled = true` to get there. The refusal stays.

This supersedes the "Beside `[plugins.hermes]`, and what happens when both are on" section of
`docs/superpowers/specs/2026-09-15-pns-discord-destination-design.md` (merged as #654), which refused
both durable plugins enabled at once. Everything else in that document, and in
`docs/superpowers/specs/2026-09-14-pns-session-attribution-and-threads-design.md` (merged as #593, its
session-thread verdict quoted in section 4 below), stands untouched.

## Dependency: routes must become configuration first

A lane on `fix/pns-routes-are-configuration` is moving hermes route names out of a compiled roster and
into configuration, because a compiled `POSTURE_ROUTE` constant meant pns knew posture existed. As
read today, `pns/crates/pns-domain/src/routes.rs` still compiles the roster: `ROUTES` is a fixed array
of three names (`DEFAULT_ROUTE`, `POSTURE_ROUTE`, `crate::stale::PRIORITY_ROUTE`), each with its own
signing key at `[plugins.hermes.keys]`, and the only way to add a fourth is a source change and a
recompile.

**This design depends on that lane landing before it can ship**, because Decision 2 below writes an
arbitrary `route = "netpulse"` value into a project's transport entry, and that value is only sane to
accept once a route is something the operator can name in configuration rather than something the
roster already had to know about at compile time. Shipping Decision 2 against the compiled roster
would mean `netpulse` never becoming usable as a route until a second, unrelated pull request added it
to `ROUTES` by hand, which defeats the point of a per-project table an operator edits alone.

## What is built today

**The both-durable refusal exists and blocks exactly what this design lifts.**
`pns/crates/pns-adapters/src/config/plugins.rs:159-176` (`refuse_two_durable_logs`) reads every plugin
the roster marks `Routing { durable: true, .. }` (`durable_plugin_names`, same file, lines 133-142)
and refuses the config at load if more than one is enabled, naming every table. Today that is exactly
`hermes` and `discord`, both declared `durable: true` in
`pns/crates/pns-domain/src/registry/roster.rs:58-85`, and the roster's own comment calls the bot "the
SECOND durable log, and the alternative to hermes rather than a companion" (lines 67-71). A test pins
the refusal directly: `a_config_enabling_both_durable_logs_is_refused_naming_both`
(`pns-adapters/src/config/plugins.rs:214-228`).

**The channel map exists, pure and shared, but only Discord reads it.**
`pns/crates/pns-domain/src/channel_map.rs` is a `BTreeMap<String, String>` plus one lookup function,
`channel_for`, tried in this order (`keys_tried`, lines 51-69): the event's named route (when it is
not the default route), then `owner/name`, then the bare project name, then `pns-events` for no
project, then `default`. `pns/crates/pns-adapters/src/destinations/discord.rs:159` is the one call
site in the whole binary: `channel_for(&self.channels, &self.route, &request.event.project)`. The
hermes destination (`pns-adapters/src/destinations/hermes.rs`) never calls it; grepped and confirmed
zero occurrences.

**Hermes route selection is global per invocation, not per project.** The route a hermes post carries
is resolved once, ahead of destination construction, from `--channel` or a producer's declared kind
(`pns_domain::routes::Kind::route()`, `routes.rs:66-72`; a health event takes `PRIORITY_ROUTE`, an
agent event takes the default), and the same resolved string is handed to every destination in
`pns/src/channel_dispatch.rs:16-33` (`destinations(selection, route, ...)`). Discord's `channel_for`
call then treats that same string as the FIRST key in its own lookup (the severity override), and only
Discord ever narrows it further by project. There is no code path today that reads a project name and
decides which hermes route to post to; the hermes destination knows one route per event, handed to it
from outside, and posts to whichever URL and signing key that route names.

**`[plugins.discord.channels]` is the map, an open table keyed by project.**
`pns-adapters/src/config/discord.rs:94-108` (`channel_map`) reads it out of `[plugins.discord]`
verbatim: every entry that states a non-empty string, and nothing else. `states_default_channel`
(same file, lines 111-113) is what the load-time refusal below reads. The schema admits it as an open
table (`config/schema.rs:113`, `DISCORD_CHANNELS`), and the render layout documents its lookup order
in the shipped comment (`config/render/layout/destinations.rs:76-89`).

**The refusal for a map with no catch-all exists and is unaffected by this design.**
`refuse_a_map_without_a_catch_all` (`pns-adapters/src/config/plugins.rs:189-202`) refuses an ARMED
discord table whose `channels` states no `default` key, at load, beside the two-durable-logs refusal.
Ruling 6 keeps this behavior; it moves to read the renamed table instead.

**`channel_plan` computes legs from config-load-time selection, not from an event's project.**
`pns/crates/pns-domain/src/routing.rs:72-126` filters the roster's selection on each plugin's
`Routing` declaration and the requested `DeliveryScope`; nothing in its filters, nor in
`Destinations::durable()` (`pns-application/src/destinations.rs:120-124`, "the FIRST durable entry in
registration order"), reads `event.project`. Which durable plugins are even candidates is decided once
when the config loads and the registry selects a roster, not per event.

**Session threads are bot-only, and hermes cannot build them.** The `session_threads` table
(`pns-adapters/src/persistence/sqlite/session_threads.rs`, created at migration step 10 per
`pns-adapters/src/persistence/sqlite/migrations.rs:67`) keys a Discord thread id on `(session,
channel)` and is written and read only by the bot's own destination code
(`pns-adapters/src/destinations/discord/threads.rs`). The session attribution design's verdict, quoted
verbatim: "pns can address a thread it already knows and can never obtain one. One thread per session
is therefore **not buildable against hermes v0.17.0 through supported configuration**, and hermes is
not ours to patch"
(`docs/superpowers/specs/2026-09-14-pns-session-attribution-and-threads-design.md`, "Verdict: threads
wait on hermes"). Nothing since has changed that; this design does not attempt to.

## Decisions

### 1. The table is named `[plugins.destinations]` and its entries are `[plugins.destinations.projects]`

`[plugins.discord.channels]` stops existing as a name; a project's entry no longer lives under either
transport's own table, because an entry can now point at either one. `[plugins.destinations]` holds no
transport's own settings (token, type, hermes keys all stay where they are, under `[plugins.discord]`
and `[plugins.hermes]`), only the one routing map every project entry lives in:

```toml
[plugins.destinations.projects]
default = { keepassxc = "Discord (Uriel) :: Channel ID (#github-notifications)", field = "Password" }
"pns-events" = { keepassxc = "Discord (Uriel) :: Channel ID (#pns-events)", field = "Password" }
priority = { keepassxc = "Discord (Uriel) :: Channel ID (#priority)", field = "Password" }
dotfiles = { keepassxc = "Discord (Uriel) :: Channel ID (#dotfiles-dev)", field = "Password" }
netpulse = { route = "netpulse" }
```

`destinations` names what the table decides (where a project's news goes), never a transport, which is
the same naming discipline `channel_map.rs`'s own doc comment already states for the values inside it:
"its values are opaque to this crate". `projects` as the child table's name, rather than repeating
`channels`, states plainly that the key space is project names, the way `PLUGINS_DISCORD_CHANNELS`'s
own layout comment already reads the keys today (`config/render/layout/destinations.rs:76-89`), and
avoids implying every entry names a Discord channel when some now name a hermes route instead.

### 2. Two entry shapes, judged by which key is present, and nothing else admitted

An entry is exactly one of:

- **A Discord entry**: a `{ keepassxc = ..., field = ... }` secret table, read as a Discord channel id,
  exactly `[plugins.discord.channels]`'s entries are read today (`config/discord.rs:94-108`).
- **A hermes entry**: `{ route = "<name>" }`, a bare string naming one of the routes
  `pns_domain::routes` grants a signing key to (Decision 2 depends on that roster becoming
  configuration, per the dependency section above; until it does, the only names a `route` value can
  usefully take are the ones already compiled in).

**Both keys on one entry is refused at load, naming the project.** A design consideration follows this
decision (see "Whether a project can name both"), and its answer is no: an entry stating both a
`keepassxc` reference and a `route` is ambiguous about which leg fires, and this repository already
refuses rather than guesses (`ConfigError::Invalid`, the same posture `refuse_a_map_without_a_catch_all`
and `discord_backend` take for every other malformed table). **Neither key on a present entry is
refused the same way `stated` already reads an entry that states nothing** (`config/discord.rs:112-116`):
an entry present with an empty string, the wrong type, or no key the schema recognizes reads as
malformed and is refused, naming the project and the entry, rather than silently falling through to
`default` as if the project had no entry at all. An operator who mistyped a key deserves to be told,
not routed to the catch-all as if they had written nothing.

The `default` and `pns-events` catch-all keys keep exactly the reading `channel_map.rs` already gives
them (`NO_PROJECT_KEY`, `DEFAULT_KEY`, lines 26-33), and each is free to be either shape: `default`
could name a hermes route just as easily as a Discord channel, since the fallback is a destination
like any other project entry.

### 3. The five-step lookup keeps its five steps; the transport travels with the resolved entry

`channel_map::channel_for` (`channel_map.rs:36-42`) and `keys_tried` (lines 51-69) are unchanged in
shape: route, then `owner/name`, then bare project, then `pns-events`, then `default`, first hit wins.
What changes is what the lookup returns. Today it returns `&str`, a bare channel id string, read by
exactly one caller that already knows it means Discord. The lookup instead has to return the resolved
entry's SHAPE along with its value, an enum of the two forms from Decision 2, so a caller reading
`netpulse`'s `{ route = "netpulse" }` entry knows to hand the event to the hermes destination with that
route, rather than trying to post a route name to Discord as if it were a channel id.

**The two fallbacks (`pns-events` for no project, `default` for an unmapped one) take whichever
transport their own entry names**, exactly like every other key: there is no rule that a fallback must
be Discord or must be hermes. An operator who writes `default = { route = "pns-events" }` gets every
unmapped project routed through hermes; one who writes it as a Discord channel id gets the behavior
this repository ships today. Nothing in this design privileges one transport as the "real" default;
the catch-all is a project entry like any other, it is just the one that answers when no more specific
key does.

**This is new plumbing, not a reading change**, because today's `channel_plan`
(`routing.rs:72-126`) decides which durable plugin fires once, at config-load time, off which plugin
is `enabled`, before any event exists to read a project from. Making the choice per project means the
durable leg can no longer be selected purely from the roster's `Selection`; the event's project has to
be read and looked up in `[plugins.destinations.projects]` BEFORE the durable leg is chosen, and the
looked-up entry's shape is what decides whether the discord or the hermes destination receives this
particular event. That lookup has no code to sit in yet; Decision 5 below (the build plan) is where it
lands as its own pull request, because it changes `channel_plan`'s contract, not just the table it
reads. `EventArgs.channel`'s existing doc comment ("resolved through the config's `[plugins.hermes]`
channels table", `pns-domain/src/notification.rs:44-46`) already names this resolution as an intended
place for it to happen; it just is not built yet, and hermes has never had a `channels` table under it
to resolve through.

### 4. The both-durable refusal is replaced by a table that dissolves the duplication it guarded against

`refuse_two_durable_logs` exists because `channel_plan` used to keep every plugin whose declaration
passed as a leg (`routing.rs:82-125`, no project filter anywhere in it), so two durable plugins both
enabled produced two legs for one event, and `Destinations::durable()`
(`destinations.rs:120-124`) silently answered whichever registered first for the recap.

**Per-project routing dissolves the duplication, because one project's event now resolves to exactly
one entry in `[plugins.destinations.projects]`, and that entry names exactly one transport.**
`channel_plan` (as amended by Decision 3's plumbing) filters the durable candidates down to the ONE
the project's resolved entry names, not to every plugin the roster marks durable. There is no longer a
step where "every enabled durable plugin receives every event": the durable leg IS the resolved entry,
singular, by construction. Two durable plugins being ENABLED in config (both `[plugins.hermes]` and
`[plugins.discord]` armed with a token) stops being the ambiguous state the refusal existed to name,
because which one actually fires is decided per event by the table, not by which tables are switched
on.

**The recap's question ("which is the durable one?") gets its answer from the same lookup, using the
recap's own project.** `pns recap agent --stdin` posts with `agent: "pns"`, `state: "recap"` and no
project (`docs/superpowers/specs/2026-09-15-pns-discord-destination-design.md`, section 4, read there
as background and unchanged by this design), so it resolves through the `pns-events` fallback key like
any other project-less event, landing on whichever transport that entry names.
`Destinations::durable()`'s "first in registration order" answer is no longer read on this path at
all: the caller that used to ask "which durable destination is there" instead asks "what does the
resolved entry for this event's project name", and gets a single answer because the table can only
ever name one transport per entry (Decision 2's refusal on an entry naming both is what keeps that
true).

**What replaces the refusal, concretely: nothing refuses "both plugins enabled" any more.** Enabling
`[plugins.hermes]` and `[plugins.discord]` together stops being a config error; a project that names
neither in its own entry falls through to `default`, whose SHAPE decides which of the two enabled
plugins actually receives that project's events. `refuse_a_map_without_a_catch_all` keeps blocking the
one failure that per-project routing does not dissolve: an armed table (Discord's `token` set, or
hermes's `keys` set) with no `default` entry, because a project nobody wrote an entry for still needs
somewhere to land.

### 5. A session routed through hermes has no thread; the bot's session is unaffected

Session threads stay exactly what they are today: a feature of the bot's own destination, keyed on
`(session, channel)`, unreachable through hermes because hermes offers no way to create or learn a
thread id (the quoted verdict above). Nothing in per-project routing changes that finding; it only
changes which projects ever reach the bot leg at all.

A session whose project resolves to a hermes entry posts every event of that session as a plain
message to that route, with no thread grouping, exactly as every hermes post does today. A session
whose project resolves to a Discord entry gets the thread behavior described in the discord
destination design's section 3, unchanged. **A session that crosses both** (an agent starts in
`dotfiles`, mapped to the bot, and later posts an event attributed to `netpulse`, mapped to hermes) is
not a new case this design has to solve: it already exists today for two Discord channels, and the
session-thread design's own answer holds without change, "one session, two threads, each in the
channel its events were actually about"
(`docs/superpowers/specs/2026-09-15-pns-discord-destination-design.md`, "Where the id lives"). Here it
becomes "one session, one thread and one threadless stream", which is the same rule read across a
transport boundary instead of a channel boundary: each project entry decides delivery for its own
events, and a session is only ever the sum of wherever its events individually landed.

### 6. A project cannot name both transports; that is a trap, not a want

Decision 2 already refuses an entry stating both `keepassxc` and `route`. The want behind asking for
both is real: an operator might want a project's events to reach a human glancing at Discord AND an
agent watching a hermes route. But granting it at the entry level reintroduces exactly the duplication
Decision 4 just dissolved, one event producing two legs again, with no severity-outranks-subject rule
to arbitrate which leg is "the" durable one when a report or a recap asks. The `priority` route already
exists as the escalation path SEVERITY takes across every project (ruling: "severity outranks
subject", `channel_map.rs`'s own module doc), so an operator wanting both audiences for one project's
routine events is asking for a second dimension this table was not built to hold, project times
audience, not project alone. That is a real want, but it is a different table (or a list value per
project) than the one this design's mandate covers, and speculating its shape here would be exactly
the premature two-table problem ruling 3 already rejected once. It is out of scope, not solved by
fiat.

## Assumptions

- **`route` names a route already granted a key today**, because `fix/pns-routes-are-configuration`
  has not landed as read in this checkout; once it has, a `route` value in
  `[plugins.destinations.projects]` should be validated against whatever that lane's config surface
  becomes, not against a compiled `ROUTES` array. This design does not attempt to spell out that
  lane's own schema; it only states that its landing is a precondition.
- **The lookup's return type becomes an enum over the two entry shapes** rather than a bare string,
  because a caller has to know which destination to hand the event to; the exact type and its home
  crate (`pns-domain::channel_map` is the natural fit, since it already owns `ChannelMap` and
  `channel_for`) is an implementation decision for the build, not this design.
- **`channel_plan`'s signature changes to accept the resolved entry** rather than just a
  `DeliveryScope`, because the durable leg it used to compute from the roster's `Selection` alone now
  depends on a per-event lookup; this design does not draft the new signature, only states that the
  dependency moves from "which plugins are enabled" to "which plugins are enabled, narrowed to the one
  this project's entry names".
- **Migration of existing `[plugins.discord.channels]` entries to `[plugins.destinations.projects]`
  is a config-values edit, not a code migration**: nothing reads the old table name once the rename
  ships, so there is no compatibility shim to build and none to later remove, consistent with this
  repository building no removal mechanisms.
- **A project entry naming neither key, present but empty, is refused rather than treated as absent**,
  matching `stated`'s existing reading of a malformed value one level down; this keeps the same posture
  the repository already takes on every other config table, refuse rather than guess.

## Open questions

- Should the hermes-route entry shape (`{ route = "..." }`) also accept a plain string
  (`netpulse = "netpulse"`) rather than requiring the table form, given a route name carries no
  secret and the `{ keepassxc = ..., field = ... }` shape exists specifically because a Discord channel
  id is one? The operator has not stated a preference between the two spellings.
- Once `fix/pns-routes-are-configuration` lands, does naming a route in
  `[plugins.destinations.projects]` that the hermes config never granted a signing key to get refused
  at load (the same posture `discord_backend` takes for an unanswered `type`), or does it fall back
  silently to the default route? This design assumes refusal, consistent with ruling 6's "refuse rather
  than guess" posture, but the operator has not confirmed it.

## Build plan: the pull request ladder

1. **Blocked on `fix/pns-routes-are-configuration` merging first.** No work below can land against a
   compiled route roster without immediately needing a second edit the moment that lane lands.
2. **Rename the table.** `[plugins.discord.channels]` becomes `[plugins.destinations.projects]` in the
   schema (`config/schema.rs`), the render layout (`config/render/layout/destinations.rs`), and
   `config-values.toml`; `channel_map.rs`'s reader moves off `[plugins.discord]` onto the new table
   name. No behavior change: every existing entry stays a Discord channel id, and the both-durable
   refusal stays in place. This is the smallest possible diff that proves the rename compiles and every
   existing test still passes with the table read from its new location.
3. **Teach the lookup the second entry shape.** `channel_map::channel_for` (or its replacement) returns
   an enum over "Discord channel id" and "hermes route name", `discord_backend`'s reading pattern
   extended to judge an entry the same way `[plugins.discord] type` is judged today: named, recognized,
   or refused. The both-durable refusal is untouched in this step; only entries are polymorphic so far.
4. **Wire the per-event choice into `channel_plan`.** The durable leg selection reads the resolved
   entry for the event's project and narrows to the one transport it names, instead of keeping every
   durable plugin the roster selected. `Destinations::durable()`'s "first in registration order"
   callers (the recap) move onto the per-event lookup.
5. **Drop the both-durable refusal, and its two tests, replacing them with the entry-shape refusal from
   step 3** (both `keepassxc` and `route` on one entry) and a positive-control test that both plugins
   enabled together now loads.
6. **Documentation and the shipped config template.** `just pns-config-render`'s output changes shape;
   regenerate it and confirm the byte-equality test it carries still passes.

## Verification

Every claim about what exists today was read directly in this checkout rather than assumed: the
refusal and its tests in `pns-adapters/src/config/plugins.rs`, the roster declarations in
`pns-domain/src/registry/roster.rs`, the pure lookup and its one caller in `pns-domain/src/channel_map.rs`
and `pns-adapters/src/destinations/discord.rs`, the route resolution path in `pns-domain/src/routes.rs`
and `pns/src/channel_dispatch.rs`, the leg-selection policy in `pns-domain/src/routing.rs` and
`pns-application/src/destinations.rs`, the session-thread table and its migration step in
`pns-adapters/src/persistence/sqlite/session_threads.rs` and `migrations.rs`, and the session-thread
hermes verdict quoted from `docs/superpowers/specs/2026-09-14-pns-session-attribution-and-threads-design.md`.
`fix/pns-routes-are-configuration`'s current state was read from `pns-domain/src/routes.rs` as it
stands in this checkout, which still compiles `ROUTES` as a fixed array; the lane's own changes were
not available to read, only their absence.
