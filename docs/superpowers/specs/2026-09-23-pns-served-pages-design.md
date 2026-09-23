# pns served pages design

Date: 2026-09-23. Status: design. The operator approved the seven pages and their build order on
2026-09-23; every decision below that the approval did not state is numbered (`D1`, `D2`, ...) and stands
until the operator changes it. The open questions at the end each carry a recommended answer.

Implementation plan: `docs/superpowers/plans/2026-09-23-pns-served-pages.md`.

Builds on: the failures page pull request (branch `feat/pns-failures-page-timeline`), which turns
`pns failures serve` into a small site with an index at `/`, the listing at `/failures` and one record at
`/failures/<id>`, in the dark visual language the operator chose. Every page here lands after it.

## Purpose

pns serves one loopback page today, the delivery failure record, and moshi's phone app opens it through
an SSH (secure shell) local forward. The operator reads the rest of pns at a terminal: who is waiting,
whether the destinations work, what the recap said, why a notification went where it went. Seven more
read-only pages put those answers on the phone:

| Build order | Page | Route | Terminal command it mirrors |
| --- | --- | --- | --- |
| 1 | Waiting on you | `/waiting` | `pns waiting` (new) |
| 2 | Health | `/health` | `pns doctor --no-send` (new flag) |
| 3 | Recap | `/recap` | `pns recap history` (new verb) |
| 4 | Right now | `/now` | `pns now` (new) |
| 5 | Sessions | `/sessions` | `pns sessions` (new) |
| 6 | Deliveries | `/deliveries` | `pns deliveries` (new) |
| 7 | Config as loaded | `/config` | `pns config show` (new) |

Each page is its own pull request, in that order.

## Rules every page keeps

These are the operator's rules, restated once so no section repeats them.

- **One formatter.** Every value a page shows is produced by the function the matching terminal command
  uses; the page only lays values out. This is the rule `2026-09-08-pns-delivery-failure-reporting-design.md`
  states for the failure record ("There must not be two formatters that can drift"), extended to every
  page. Where no terminal command exists, the page's pull request adds one, and both surfaces render from
  one extracted value builder, pinned by golden tests.
- **Read-only.** GET only, no query string, no form, no acknowledgement, nothing irreversible behind the
  tunnel.
- **Loopback only**, bound to `127.0.0.1`, single threaded, the server loop unchanged.
- **Self-contained.** No JavaScript, no external asset, system fonts only.
- **Dark only**, in the failures page's palette, whatever the phone's appearance setting.
- **Every producer string is escaped** where it lands: route, sender, project, title, detail, command,
  summary, config value.
- **`Content-Length` counts bytes.**
- **No secret reaches a page.** Page 7 carries the argument in full.

## The site

### What the failures pull request leaves in place

After the failures page merges, `pns/crates/pns/src/failures_page.rs` owns the server (bind, the request
timeout, the parent watch, the request-line parse) and `failures_page/html.rs` owns the shell (the CSS
constant, the page wrapper, the footer, `escaped()`), the index, the listing and the record. The routing
function answers `/`, `/failures`, `/failures/<id>` and 404s everything else. The daemon starts the
server as its `PAGE_JOB` child with `pns failures serve` whenever `[failures] serve = true`.

### D1. The server module becomes `site`

The module stops being about failures the day a second page lands, so the first page's pull request moves
it, as a pure move with no behavior change:

| Before | After | Holds |
| --- | --- | --- |
| `failures_page.rs` | `site.rs` | bind, serve loop, request timeout, `Target`, `answer` |
| `failures_page/parent_watch.rs` | `site/parent_watch.rs` | unchanged |
| `failures_page/html.rs` (shell half) | `site/shell.rs` | the CSS constant, `page()`, the footer, `escaped()` |
| `failures_page/html.rs` (index half) | `site/index.rs` | the index rows and their markup |
| `failures_page/html.rs` (failures half) | `site/failures.rs` | `listing_page`, `record_page` |
| `failures_page/tests.rs` | `site/tests.rs` | unchanged tests, moved |

The command stays `pns failures serve`, the job stays `PAGE_JOB`, and `[failures] serve` and
`[failures] port` stay the keys that switch and place it (open question Q1).

### D2. One route per page, exact paths

`Target` gains one arm per page: `Waiting`, `Health`, `Recap`, `Now`, `Sessions`, `Deliveries`, `Config`,
matched on the exact paths in the table above. A trailing slash, a query string, a sub-path and every
method but GET stay 404, which the existing parser tests already pin for the failures routes and each
page's pull request extends to its own path.

### D3. The pages read through a store that cannot write

`SqliteStore::viewer(state)` is a new constructor whose every connection is the existing `read_only()`
connection (`SQLITE_OPEN_READ_ONLY`, schema version checked, no directory creation, no migration, no
legacy import). `open()` and `open_existing()` return that connection for a viewer, so every existing
query method works unchanged through it and every write method fails with SQLite's own read-only refusal.
The site opens its store with `viewer` and nothing else, the failures pages included.

This makes "read-only" a property of the connection rather than of what the handlers happen to call. A
page's pull request that reached for a write method by mistake would fail its own served-page test,
because the write never lands. A database whose schema is newer or older than the binary's reads as
unreadable, and each page prints its own unreadable sentence rather than guessing.

### D4. The value builder pattern

Every page follows the same three-part shape:

1. **The value builder** lives with the terminal command, in `pns/crates/pns/src/command_<verb>.rs` or
   a submodule of it. It reads its sources and returns a view struct whose fields are finished display
   strings (an age already spelled `12m`, a time already spelled `03:20`, a status already spelled
   `HTTP 404`). Its read function has the shape `pub(crate) fn read(sources: &Sources, now: u64) ->
   Result<View, &'static str>`, where the error is the exact sentence both surfaces print when a source
   could not be read.
2. **The terminal renderer**, `pub(crate) fn render(paint: Paint, view: &View) -> String`, lays the view
   out as lines. A golden test pins its output for a fixture view.
3. **The page renderer**, `pns/crates/pns/src/site/<page>.rs`, `pub(super) fn page(view: &View,
   rendered_at: u64) -> String`, lays the same view out as HTML. Its test asserts every string field of
   the fixture view appears in the page, escaped.

`Sources` carries what every page reads: the viewer store, the state directory beside it and the home
directory. A page that also asks a live process (herdr for Sessions, the router for Right now) takes that
reading as an argument of its read function, so its tests hand it a fixture. The terminal command builds
`Sources` from `state_dir()` and `HOME`; the site builds it once when it starts. The server's `answer` arm for a
page is two calls, `read` then `page`, and nothing else.

### D5. The render time is a parameter

Every renderer takes `now` or `rendered_at` as an argument and never reads the clock itself. The site
reads the clock once per request, in `answer`, and hands the same second to the builder and to the page,
so an age and the footer's "Updated" time describe one moment and every golden test is deterministic.

### D6. Times are UTC, as on the failures page

The site's footer says "All times UTC" and every event time on every page is UTC, spelled by the date
formatter the failures pull request adds beside `utc_timestamp` in `pns-adapters/src/macos/clock.rs`
("Today", "Yesterday", "Sep 20", "September 23, 2026 at 03:32 UTC"). The exceptions are values the
operator configured as local wall-clock times (a profile rule's hours, a lamp dim window, a manual
profile's "until"), which print as the terminal prints them and carry the word "local" (open question
Q2).

### D7. The tones and what they mean

The chosen CSS has two tones on a state chip: red for failed and amber for in progress. The pages use
exactly those, plus one neutral tone for a state that asks nothing of the operator:

| Tone | Chip dot | Means | Examples |
| --- | --- | --- | --- |
| Amber (`fh-active`) | hollow | in progress, or waiting and not yet late | Waiting, Working, Retrying |
| Red | filled | failed, or needs the operator now | Overdue, Blocked, Not delivered, a `Bad` doctor row |
| Neutral (`fh-quiet`, new) | filled, muted | settled, nothing to do | Delivered, Done, Idle, a `Good` doctor row |

The neutral tone is the one addition to the chosen CSS: `.fh-quiet .fh-dot` and `.fh-quiet .fh-state`
in `--muted`, and `.fr-status.fr-quiet` and `.fr-status.fr-red` for the record card's chip, which the
chosen record file only spells in amber. Nothing else in the chosen CSS changes. Each page states which
of its states takes which tone, and the word on the chip comes from the value builder, never from the
page.

### D8. Two card shapes

A page whose content is a list of things that happened at a time uses the listing card
(`#failure-history`): day headings, the time gutter, the rail, one `<details>` per entry. A page whose
content is a state with its reasons uses the record card (`#failure-record`): the headline with its chip,
the meaning line, then the page's own parts drawn from the chosen record's pieces (the two-cell box,
labelled rows, folded blocks). The ids stay as the
chosen files spell them, because the chosen CSS is scoped by them and ships verbatim.

| Page | Card |
| --- | --- |
| Waiting on you, Recap, Sessions, Deliveries | listing |
| Health, Right now, Config as loaded | record |

### D9. The index

`/` lists one row per page: the page's name as a link, a one-line muted blurb, and on the right a live
summary in the page's own tone. Each page's pull request adds its row. The final order puts what needs
the operator first:

| Row | Blurb | Summary (right side) |
| --- | --- | --- |
| Waiting on you | sessions waiting on an answer | the count, red when any is overdue, or "none" |
| Failures | delivery legs that did not arrive | the count, or "none" (the failures pull request) |
| Right now | what pns believes and where a notification would go | the surface word and the profile, "desk, work" |
| Sessions | live agent sessions | "4 live", red ", 1 blocked" appended when any is |
| Deliveries | every delivery in the last day | "142 in 24h" (events), red ", 2 not delivered" appended when any event was not |
| Health | the destinations, the gateway and the daemon | "daemon running", red "daemon not running"; red ", 2 not delivered" appended when the ledger has dead letters |
| Recap | the latest recap for each window | the newest complete window and when it ended, "nightshift, ended 08:00" |
| Config as loaded | the settings pns is running with, secrets hidden | "loaded", "missing" or "did not load" |

**A summary never makes a network call.** The index is the page moshi opens first and the server is
single threaded, so each summary reads only the store, the local probes and herdr's local socket. The
Health summary reads only the daemon's heartbeat file and the ledger's dead-letter count, never the
report; the Right now summary never runs the router probe. Each summary is a function in the page's
value builder module, `pub(crate) fn summary(sources: &Sources, now: u64) -> Summary` (the clock dropped where a page has no use for it), where
`Summary { text: String, tone: Tone }`, so the index shows the value the page itself would show.

A summary whose source is unreadable reads "unreadable" in the neutral tone. The index never fails as a
whole because one source did.

### D10. Empty, unreadable and not found

Every page has three non-happy states, and each prints a sentence the terminal command prints too:

- **Empty**: the header, one line saying there is nothing ("Nothing is waiting on you."), the footer.
- **Unreadable**: the header, the value builder's error sentence, the footer, still a `200 OK`, because
  the page answered and what it answered is that the source could not be read.
- **Not found**: unchanged from the failures page, a 404 inside the shell.

## Page 1: Waiting on you

Route `/waiting`. Terminal command `pns waiting`. Listing card.

### What it shows

Every open wait, one entry per session, newest first, grouped by the day it opened. It is the live form
of the list the return card carries after `fix/pns-return-card-lists-open-waits` (pull request 924): the
same store query, the same wait line, plus the age the card has no room for.

### Source

`SqliteStore::open_waits(0, now)` from pull request 924, which lists every session whose `sessions` row
has `blocked_since` set, joined to its newest `blocked`, `asked` or `asking` activity event, with the
count of its unanswered waits. The hook word `waiting` (the sandbox network approval) is recorded as
`blocked`, so it arrives here under that word.

**D11. `OpenWait` gains `since`.** The row already selects `blocked_since`; the struct carries it now,
because an age is the one thing the page adds and a second query for it would be a second definition of
"open". The return card ignores the new field.

**D12. The recap's `open` section is a different list and stays one.** `pns recap open` lists sessions
whose LAST EVENT is in `NEEDS_YOU` (so it includes `denied` and `failed` and excludes `asking`). This page
lists open WAITS, from the `sessions` row, which is what the return card lists. The two answer different
questions and neither page claims the other's.

### The values, all from the value builder

| Field | Produced by | Example |
| --- | --- | --- |
| title | `pns_domain::render::title(agent, state, project)` | `codex · blocked · dotfiles` |
| asks | `OpenWait.asks`, verbatim | `Bash: git push origin feat/x` |
| count | `"{count} unanswered"` when count is above one, else empty | `8 unanswered` |
| age | `pns_domain::remind::waited(now - since)` | `21m` |
| clock, day | the failures pull request's UTC formatters | `03:20`, `Today` |
| since | the full UTC date formatter | `September 23, 2026 at 03:20 UTC` |
| chip | `pns_domain::stale::wait_chip(since, now, window)` (new) | `Waiting` or `Overdue` |
| escalation | `pns_domain::stale::escalation_line(since, window)` rendered with the UTC clock | `after 1h, at 04:20 UTC`, or `off` |

The terminal line is the return card's own `waiting()` line (made `pub` in `pns_domain::missed`), with
the age and the chip word in front:

```
pns: 2 sessions waiting on you
  ● 3m   Waiting  claude · asked · pns: Which branch should the fix land on?
  ● 1h   Overdue  codex · blocked · dotfiles ×8: Bash: git push origin feat/x
```

### Tones

`Waiting` is amber with the hollow dot while the wait is younger than the stale escalation window
(`[stale] escalate_after`, default one hour). `Overdue` is red at or past it, the same predicate the
escalation's own query uses (`blocked_since <= now - window`). With the escalation off, a wait is never
overdue.

### Layout

Header "Waiting on you". Day headings. Each entry: the gutter shows the clock the wait opened; the title
line carries the title and the chip; the subtitle is `asks`; the next line reads "Waiting 21m" with the
count after it in the muted tone. The folded "Details" holds "Waiting since" and "Escalation".

### Empty and unreadable

"Nothing is waiting on you." / "pns: the session store could not be read".

## Page 2: Health

Route `/health`. Terminal command `pns doctor --no-send` (new flag). Record card.

### What it shows

The doctor's report without its sends: each destination and when it last delivered, the hermes
gateway's routes, moshi's pairing and daemon version, the lights bridge, the room sensor, the daemon's
last tick, and the rest of the report's sections, each row with its mark.

### Why the doctor needs a no-send form first

`pns doctor` today sends one test notification through every enabled destination (the phone buzzes),
fires a real lamp pulse, runs the configured recap summarizer once, and reads the router through
`ReadHomeProbe::run`, which can raise a stale-reading notification and write a staleness record. None of
that may happen behind a tunnel, and the operator approved `pns doctor --no-send` on 2026-09-23, not yet built, for the same reason at the terminal. This page's pull request builds that flag, and the page is
its phone form.

**D13. What `--no-send` withholds, and what it still reads.**

| Check | Plain `pns doctor` | `pns doctor --no-send` |
| --- | --- | --- |
| each send destination | one test event | not sent; the row says when that destination last delivered, from the ledger |
| lights | one real pulse | not pulsed |
| recap summarizer | runs the real binary once | not run |
| home network | `ReadHomeProbe::run` (may alert, may write) | `read_home` and `home_report::rows`, which never alert or write |
| hermes routes | unsigned probe POST | unchanged: an unsigned POST cannot become a delivery |
| lights bridge, certificate | a read of the bridge | unchanged |
| moshi pairing, Back Tap, Focus, room sensor, daemon, decisions, imports, delivery health | reads | unchanged |

The withholding is enforced where the report is run, not where it is printed: `RunDoctor` gains a
`sending: Sending` field (`Send` or `Withhold`), and under `Withhold` it never calls its `deliver`, `pulse`
or `summarizer` actions. The application-layer test hands it actions that panic when called.

**D14. "Last delivered" comes from the ledger.** A new read, `SqliteStore::last_delivered_each() ->
Result<Vec<(String, u64)>, LedgerFailure>`, answers the newest `finished` of an acknowledging attempt
(`outcome = 1`) per destination. A withheld row reads `phone: not sent (--no-send); last delivered
September 23, 2026 at 03:20 UTC, 1h ago`, or `never delivered`. It is a `Note`, never a grade: an
old delivery on a quiet night is not a fault.

### The values, all from the doctor

The report is the doctor's own `Vec<pns_domain::doctor::Item>`, produced by one extracted function both
surfaces call: `command_doctor::run_report(sources, detail, sending, emit) -> i32`. The terminal prints
each item as it arrives; the site collects them. The row text passes through `doctor_style`'s
`unattributed` (the `pns doctor: ` prefix removed), and the closing summary (`nothing to act on`, or `1
issue to fix, 2 warnings to look at:` and the numbered list) comes from `doctor_style::closing`, extracted
from `Report::close` so both surfaces count the same rows.

### Tones

A `Good` row is neutral, `Warn` amber, `Bad` red, `Note`, `Detail` and `Aside` carry no chip. The page's
headline chip is `All good` (neutral), `N to look at` (amber, warnings only) or `N to fix` (red, any
issue), from the same counts as the closing summary.

### Layout

Headline "Health" with the chip. The meaning line is the closing summary's headline, and the closing
list, when there is one, sits under it. Then one block per report section, in the report's order, with
its title and blurb, each row a line with a small dot in its mark's tone; `Detail` and `Aside` rows are
indented and muted. The daemon's age and the last deliveries are rows of their sections, as at the
terminal, rather than a separate box whose values the report would have to compute twice.

**D15. The page is slow and says so.** The route probes, the bridge read and the router read each carry
their own deadline (three seconds per route, five for the router), so the page can take several seconds.
The single-threaded server is unchanged; the index never runs the report (see D9), and the Health index
row reads only the daemon's heartbeat and the ledger's dead-letter count.

**D16. The doctor's pre-report warnings stay on standard error.** Three sentences print before the report
today (a switched-off backend, a route with no signing key, a room sensor refusal) and are not report
rows. The page does not show them (open question Q3).

### Empty and unreadable

Never empty. A config that cannot be read is the doctor's own report of that, with its marks.

## Page 3: Recap

Route `/recap`. Terminal command `pns recap history` (new verb). Listing card.

### What it shows

The latest instance of each recap window (nightshift, morning, afternoon, evening), in progress or
complete, and the instance before each: the last two days, newest first, each with its counts and its
stored summary when one exists.

### Source

A recap is computed, not stored: `pns recap` assembles one from the activity store over a window's
bounds. Only the summary paragraph is stored, in `recap_summaries`, ONE ROW PER WINDOW NAME replaced in
place, with `covers` (the newest event it summarized) and `at` (when it was written).

**D17. The page computes counts and never runs the recap's sources.** A full recap also runs the
`[recap.sources]` commands (git, gh, dam) and can run the summarizer. The page and `pns recap history`
read only the activity store (`activity_between(since, until)` grouped by
`pns_domain::recap::activity::by_project`) and the stored summaries, so a page load spawns nothing.

**D18. Instances come from the recap's own window arithmetic.** For each of the four windows,
`pns_domain::recap::window::resolve(window, previous, ...)` with `previous` false and then true gives the
latest instance and the one before, through `command_recap::window::named`, which already resolves a
named window through the local zone. An instance whose `until` is after now is in progress.

**D19. A stored summary belongs to the instance it covers.** The one stored row for a window attaches to
the instance whose `since < covers <= until`. Every other instance reads "no summary kept". Keeping every
instance's summary is a schema change (open question Q4).

### Values

| Field | Produced by | Example |
| --- | --- | --- |
| title | `Window::as_str`, capitalized by the view builder | `Morning` |
| bounds | the local civil times `resolve` returned, spelled `08:00-12:00 local` | `08:00-12:00 local` |
| day | the local civil date of `since`: `Today`, `Yesterday`, `Sep 21` | `Today` |
| chip | `In progress` (amber) when `until > now`, else `Complete` (neutral) | `Complete` |
| tally | `pns_domain::recap::history::tally_line(&projects)` (new) | `42 events, 5 sessions in 3 projects` |
| summary | the stored `text`, verbatim | the paragraph |
| written | `pns_adapters::local_timestamp(at)` and `source`, as `summary::attach` already spells them | `written 12:01 by claude` |
| per project | `pns_domain::recap::history::project_line(&project)` (new) | `dotfiles: 3 sessions, 30 events` |

**D20. This page's times are local.** A recap window is defined in local civil time (`[recap] morning =
"08:00-12:00"`), and its bounds only mean something in that zone. The page's day headings and bounds are
local and say so, and its footer reads "Window times local" where the other pages read "All times UTC".

### Layout

Header "Recap". Day headings (local). Each entry: the gutter shows the instance's start; the title is the
window name with its chip; the subtitle is the bounds; the next line is the tally; the summary paragraph
follows in the body text color, or "no summary kept" muted. "Details" folds one line per project.

### Empty and unreadable

An instance with no events reads "nothing recorded". An unreadable activity store: "pns: the activity
store could not be opened, so there is no recap to give" (the terminal's existing sentence).

## Page 4: Right now

Route `/now`. Terminal command `pns now`. Record card.

### What it shows

What pns believes at this second and why: where the operator is, the screen lock, the home network, the
active profile, every hush in force, what the lamps are holding or have muted, which waits are about to
escalate, and, for each event state, where a notification of that state would be delivered now.

### What the code believes today, stated honestly

Three facts shape this page, and it shows each of them rather than papering over it:

- **Delivery reads the surface, not the router.** The delivery decision uses `Surface { Desk, Mobile,
  Away }` from `pns_domain::surface::surface` (desk input age, phone input age, Back Tap marker age, screen
  lock). The router's home reading (`pns_domain::home::HomeReading`) is computed only by `pns doctor`, and
  the event path passes `HomePresence::Unknown` (`presence_runtime::home_presence`). The page's headline is
  therefore the surface, and the router reading is a row of its own that says it is not used for delivery
  yet. The pull request that wires the router into delivery removes that clause.
- **A profile is resolved but not yet applied.** `profile_runtime::active` resolves and `pns profile`
  prints it, but no delivery path reads it. The per-state table below is computed by the delivery decision
  itself, so it shows what delivery does today and picks up the profile's mask the day the mask lands in
  `Overrides`, with no change to this page.
- **There is no single "quiet hours".** Four things hush pns: `pns mute`, a macOS Focus named in
  `[focus]`, the lamp dim window, and a profile's `quiet` flag. The page lists each on its own row.

### Sources and values

| Row | Source | Produced by |
| --- | --- | --- |
| headline | `pns_application::operator_surface_reading` | `pns_domain::surface::Surface::headline` (new): `At the desk`, `On the phone`, `Away` |
| chip | the same reading's `screen_locked` | `Locked` (amber), `Unlocked` (neutral), `Lock unknown` (neutral) |
| desk input, phone input, Back Tap | the same reading's three ages | `pns_domain::doctor::ago`, and `fresh within 2m` from `desk_fresh_secs` via `pns_domain::duration::spelled` |
| home network | `pns_application::read_home(router, device)`, never `ReadHomeProbe::run` | `home_report::rows(reading, None)`, the doctor's own rows |
| profile | `profile_runtime::active` | `pns_domain::profiles::report::because` and `surfaces_line` |
| mute | `SqliteStore::mute_expiry` | `pns_domain::mute::status` (the clause `status_line` prints after `pns: `) |
| Focus | `pns_adapters::focus_now` | `pns_application::doctor_focus`, the doctor's own row |
| lamp dimming | `[lights] dim_window` | `pns_domain::lamps::window::quiet_now` spelled `22:00-07:00 local, outside now` |
| lamp mutes | `SqliteStore::read_muted` | `pns_domain::lights::mute::muted_report` |
| loop lamp | `pns_adapters::live_leases` (new, read-only) | `held by wW:p21 for 2h` via `pns_domain::remind::waited` |
| escalations | `SqliteStore::stale_blocks(now)` and `Sources::stale_window` | `pns_domain::stale::pending_line(blocked, due_clock)` (new) |
| would deliver | the delivery decision, once per state | `pns_domain::routing::legs_line` (new) |

**D21. The router probe runs on `/now` only.** It is an HTTP read of the UniFi router with a five-second
deadline, so the index summary never runs it. `ReadHomeProbe::run` is never called by this page because it
can raise a stale-reading notification and write a staleness record; `read_home` is the pure read beneath
it. The operator approved a daemon poll that stores the router's answer about once a minute
and makes "not home" count as away (approved 2026-09-23, not yet built). When that lands, this row reads the stored
answer and its age instead of probing, and loses its "not used for delivery yet" clause; that pull
request carries the change to this page.

**D22. The loop lamp is read without sweeping.** `sweep_leases` removes expired lease files on the way
through, which is the daemon tick's job. `live_leases(state, now, timeout)` is a new read-only sibling
that lists `(pane, epoch)` for every lease inside its bound using the sweep's own liveness predicate, and
renames or removes nothing.

**D23. Pending escalations are the un-escalated waits.** `stale_blocks(now)` lists every wait whose
`escalated_at` is still null; each one's due time is `since + window`. The line names the session and
when it pages: `claude in pns pages at 05:41 UTC`. It says nothing about the away and locked skips,
which the operator ruled out on 2026-09-23 (the stale page fires regardless) and which that change
removes from the gate.

### The per-state table

**D24. The table is the real decision, run once per state.** The event path's override assembly
(`event_flow/execution.rs`, the `Overrides { muted, focus_active, ..overrides_from_env() }` block and the
class and silence policy read beside it) is extracted into one function, `live_overrides(home,
focus_silence, now)`, which the event path and this page both call. The page then calls
`pns_application::decide` with that override, the plugin selection `select_plugins(&roster(), loaded)`
already makes, the default delivery class, no pane, the automatic scope and `long_running = false`, once
for each word of `PREVIEW_STATES = ["done", "failed", "blocked", "asked", "observation"]`. The durable
legs name the route `route_for(class_route, state)` resolves, else `[routes] default`.

A row reads `blocked  banner, hermes (priority), lights`, or `nothing (muted)` when every leg is
suppressed. The table's own caveat, printed under it: it describes an event with no pane, so the
visibility rule (a pane on screen suppresses its own banner) is not part of it.

### Layout

Headline and chip. Meaning line: `Delivery treats you as at the desk.` (the view builder's sentence for
the surface). The two-cell box: "Desk input" and "Phone input", each with its age and a meta line (the
freshness window, the Back Tap age). Labelled rows: Home network, Profile, Mute, Focus, Lamp dimming, Lamp
mutes, Loop lamp, Escalations. Then "Would deliver" as a two-column grid, one row per state. "Technical
details" folds the raw inputs: every age in seconds, the lock reading, the freshness window, the override
flags, and the words `visibility: unknown (no pane)`.

### Empty and unreadable

The page is never empty. A probe that cannot answer shows its own `unknown` value, which is what the
delivery decision reads too. An unreadable store shows `unreadable` on the rows that read it and the rest
of the page renders.

## Page 5: Sessions

Route `/sessions`. Terminal command `pns sessions`. Listing card.

### What it shows

One row per live agent session: its state, the time of its last event, its pane, and what it is working
on.

### Source

**D25. herdr decides what is live and what state it is in.** `herdr pane list` answers one object per
pane with `agent`, `agent_session.value` (the harness's own session id), `agent_status` (`working`,
`blocked`, `done`, `idle`, `unknown`), `pane_id` and `workspace_id`. Those four status words are exactly
the operator's four, and herdr is the process that knows whether a pane still exists. pns keeps no
liveness of its own: its `sessions` rows are never swept and no hook marks a session ended. A pane herdr
lists with an agent session is live; nothing else is.

The store adds what herdr does not know, joined on the session id: the session's title, project and
branch from `sessions`, and its newest `activity_events` row's time and state. A live pane pns has never
heard from (a harness with no hooks) is still listed, with "no events recorded".

New pieces:

- `pns_adapters::herdr::parse_agent_panes(json) -> Option<Vec<AgentPane>>`, where `AgentPane { pane,
  workspace, agent, session, status, terminal_title }`. A pane with no `agent_session` is skipped; a
  document that is not herdr's shape is `None`.
- `SqliteStore::session_facts(ids: &[String]) -> Result<Vec<SessionFacts>, StoreError>`, one read over
  `sessions` left-joined to each session's newest activity row.

### Values

| Field | Produced by | Example |
| --- | --- | --- |
| status chip | herdr's word, capitalized by `pns_domain::sessions::status_chip` (new) | `Blocked` |
| title | the `sessions` title, else herdr's `terminal_title`, clipped by `render::clipped` | `ship the served pages plan` |
| project, branch | `sessions` | `dotfiles`, `docs/pns-served-pages-plan` |
| agent | herdr's `agent` | `claude` |
| pane | herdr's `pane_id` | `wW:p21` |
| last event | `doctor::ago(now - at)` and the event's state word | `2m ago, asked` |
| clock | the UTC clock of the last event | `04:58` |

### Tones and order

Blocked is red, Working amber with the hollow dot, Done, Idle and Unknown neutral. **D26.** The list is
grouped by status in that order (Blocked, Working, Done, Idle, Unknown), each group under a heading in
the day-heading style, and newest last event first within a group, because the question the page answers
is "which session needs me", not "what happened when".

### Empty and unreadable

"No agent is running in herdr." / "pns: herdr did not answer, so which sessions are live is unknown". An
unreadable store still lists herdr's panes, with "session store unreadable" in place of the stored facts.

## Page 6: Deliveries

Route `/deliveries`. Terminal command `pns deliveries`. Listing card.

### What it shows

Every delivery leg the ledger recorded in the last twenty-four hours, delivered or not, grouped by the
event that produced it, with a per-destination summary at the top: how many were delivered and how fast.

### Source

**D27. Latency is `finished` of the acknowledging attempt minus `started` of the first attempt.** The
ledger has no event timestamp column, but `prepare` writes a generation-1 attempt for every leg, with
`started` set to the moment the event was accepted, in the same transaction as the event. The attempt that
acknowledged (`outcome = 1`) carries `finished`. Both are whole epoch seconds, so latency is whole
seconds. `acknowledged = 1` alone is not "delivered": the dead-letter drain sets it too, so the query keys
on `outcome = 1`.

New pieces:

- `SqliteStore::deliveries_since(cutoff: u64, limit: u32) -> Result<Vec<StoredDelivery>, LedgerFailure>`
  in `ledger/deliveries.rs`, read through the ledger's existing read-only `failing` helper.
- `StoredDelivery { id, event, destination, route, agent, state, project, accepted_at, settled_at, outcome,
  retries, deadlettered }` in `pns-application/src/ports/ledger.rs`, where `outcome` is a new enum
  `LegOutcome { Delivered, Pending, Failed(TransportOutcome) }`.

**D28. The window is the last twenty-four hours, capped at 500 legs**, newest first, and the page says
"showing the newest 500" when the cap cut anything. The ledger is never pruned and has no index on
`started`, so the cap is what bounds a single-threaded page's work.

### Values

| Field | Produced by | Example |
| --- | --- | --- |
| event title | `render::title(agent, state, project)` | `codex · done · dotfiles` |
| event chip | `pns_domain::retry::event_verdict(&[LegOutcome])` (new) | `Delivered`, `Retrying`, `Not delivered` |
| leg status | `LegOutcome::word` (new), whose failure arm is today's `short_status` wording | `delivered`, `pending`, `HTTP 404`, `no response`, `bad URL` |
| latency | `pns_domain::remind::waited(settled - accepted)` | `1s` |
| destination summary | `pns_domain::retry::destination_summary` (new) | `phone  41 of 43 delivered, median 1s` |
| clock, day | the failures pull request's UTC formatters | `03:20`, `Today` |

`short_status` in `command_failures.rs` moves onto `TransportOutcome::short(self)` in `pns-domain`, so the
failures listing and this page spell a failure the same way from one function.

### Tones

Delivered neutral, Retrying amber, Not delivered red. An event's chip is its worst leg: any dead-lettered
leg makes it Not delivered, else any pending or failing leg makes it Retrying, else Delivered.

### Layout

Header "Deliveries". A tinted box (the listing's `.fh-detail` style) with one row per destination: name,
"41 of 43 delivered", "median 1s". Then the day-grouped timeline, one entry per event: the gutter clock,
the event title with its chip, a subtitle listing each leg as `banner 0s · phone 1s · hermes HTTP 404`,
and "Details" folding one line per leg with its id (linked to `/failures/<id>` when the leg is failing),
destination, route, status, retries and latency.

### Empty and unreadable

"Nothing was delivered in the last day." / "pns: the delivery ledger could not be read" (the failures
page's sentence).

## Page 7: Config as loaded

Route `/config`. Terminal command `pns config show`. Record card.

### What it shows

The settings the running pns resolved from `~/.config/pns/config.toml`, defaults filled in, section by
section, with every secret replaced by the word `hidden`.

### D29. Redaction by construction

The page is built from the typed, resolved `pns_adapters::Config`, never from the file's text and never
from `Debug`, through one function in the crate that owns config:

```rust
// pns-adapters/src/config/disclosure.rs
pub fn disclosed(config: &Config) -> Disclosure
pub struct Disclosure { pub sections: Vec<Section> }
pub struct Section { pub name: String, pub rows: Vec<Row> }
pub struct Row { pub key: String, pub value: Shown }
pub enum Shown { Value(String), Hidden, Unset }
```

Three rules make it airtight:

1. **Typed settings are spelled field by field.** Each section's rows are written out by hand from typed
   fields (`routes.default`, `stale_escalate_after_secs` spelled through `duration::spelled`, the recap's
   window times, and so on). A typed field nobody spelled does not appear. The one typed secret, the Google
   calendar's `client_id`, `client_secret` and `refresh_token`, is spelled as `Hidden` or `Unset` and never
   as a value.
2. **Plugin tables are shown by allowlist.** A plugin's settings are a `toml::Table`, so its rows come
   from walking the table, and a key's value is shown only when the key is on `SHOWN_PLUGIN_KEYS`, a
   `(plugin, key)` list of the schema keys that carry no secret (`enabled`, `type`, `ack_deadline`,
   `poll_interval`, `webhook_port`, `rooms`, and so on). Every other key renders `Hidden` when present. A
   key added to the schema tomorrow is hidden until somebody allowlists it. The open tables (`keys` and
   `channels`) show their key names, which are route and project names, and never their values.
3. **A load failure shows no error text.** A TOML parse error quotes the offending line, and that line can
   be a secret. The page says only "the config did not load; run `pns doctor` at the terminal for the
   reason", and the terminal command, which the operator runs locally, prints the full error.

### D30. One list of secret keys, and three tests that prove the construction

`SECRET_BEARING_KEYS` and `SECRET_BEARING_TABLES` move from the private `pns-config-render` dev binary to
`pns_adapters::config::SECRET_KEYS` and `SECRET_TABLES`, completed with the two schema secrets the shipped
file does not arm (`plugins.github.webhook_secret`, `quiet.calendar.client_id`, `client_secret`,
`refresh_token`), and both the dev binary's refusal and the tests below read it.

1. **The sentinel test** (in `pns-adapters`): a config the test builds sets every schema key of every
   plugin table, every key of the open tables and every calendar credential to a distinct sentinel
   string, runs `disclosed`, and asserts that a sentinel appears in the lines exactly when its key is on
   `SHOWN_PLUGIN_KEYS`. A control asserts an allowlisted sentinel does appear, so the test cannot pass by
   rendering nothing, and the page's own test repeats the check through the page renderer.
2. **The classification test**: every key on `SHOWN_PLUGIN_KEYS` is a real schema key of its plugin
   table, and none is on `SECRET_KEYS` or inside a `SECRET_TABLES` table.
3. **The shipped-config check**: `pns-config-render --check` already renders the committed values with
   every vault secret replaced by a `from-the-vault:<entry>:<field>` placeholder and parses the result.
   It also renders `disclosed` of that config and refuses when any `from-the-vault:` text appears in it.
   `test/unit/pns-config-template.test.sh` runs that check over the committed values file, which pins the
   real shipped config from outside the crate.

### What the construction does not cover

A secret an operator pastes into a non-secret key (an API token typed into a route name) is shown,
because nothing in pns can tell it is a secret. The page's header says "secrets hidden by key", which is
the promise it can keep. Page 7 ships at full scope on that basis; no narrower scope makes the promise
stronger.

### Values and layout

Headline "Config as loaded" with the chip `Loaded` (neutral), `Missing` (amber) or `Did not load` (red).
Meaning line: the path, with the home directory written as `~`. Then one `<details>` per section, folded
except the first, named as the config file writes its tables: `routes`, `delivery_class`, `recap`,
`stale`, `retry`, `failures`, `storage`, `focus`, `remind`, `gateway`, `profiles`, `quiet.calendar`,
`lights`, and one `plugins.<name>` per plugin. Each is a two-column grid of key
and value; a hidden value is the muted word `hidden`, an unset one the muted word `unset`.

### Empty and unreadable

Missing: "No config file; pns is running on its defaults." and the defaults' sections. Did not load: the
sentence in rule 3.

## Out of scope

- Any action from a page: acknowledging, draining, retrying, muting, answering a wait. The site stays
  read-only; the terminal is where state changes.
- Automatic refresh. moshi's own reload refreshes a page; a `<meta http-equiv="refresh">` would keep the
  single-threaded server busy on a phone nobody is looking at.
- A light theme, authentication, and any interface but loopback.
- Deep links from a notification into a page, which moshi has no documented way to carry (the failure
  reporting design's own reason).

## Verification

- Each page's value builder is pinned by a golden test of its terminal form, and each page's renderer by
  a test that every string the view carries appears on the page, escaped.
- Every served-page test runs against a sandbox state directory and an OS-assigned port, and the
  database it read is byte-identical afterwards.
- The Config page's three tests (spec D30) pass, including the shipped-config check over the committed
  values file.
- After the operator's own apply: each route answers at `127.0.0.1:8646` through moshi, the index shows
  eight rows, and `/health` produced no notification anywhere (no banner, no phone card, no Discord post,
  no lamp pulse).

## Open questions

Each carries the recommended answer; the plan builds the recommendation until the operator says
otherwise.

1. **Q1. Rename the server now that it is a site?** `pns failures serve`, the daemon's `PAGE_JOB` and the
   `[failures] serve` and `port` keys all name failures while serving eight pages. Recommended: keep all
   of them for this program. moshi's `scan-ports`, the shipped config and the daemon all name them, and a
   rename is its own small pull request whenever the operator wants one.
2. **Q2. UTC or local time?** The failures page chose UTC, so every event time here is UTC; recap windows
   and configured clock times are local and say so. Recommended: keep UTC for event times, matching the
   ledger and the failures page. Switching the whole site to local later is one change to the three UTC
   formatters' callers.
3. **Q3. Move the doctor's three pre-report warnings into the report?** A switched-off backend, a route
   with no signing key and a room sensor refusal print to standard error before the report, so the Health
   page cannot show them. Recommended: yes, as a follow-up pull request after page 2, which makes them
   rows of a leading "Config" section on both surfaces.
4. **Q4. Keep a recap summary per window instance?** Today one row per window name is replaced in place,
   so only the latest instance of each window has a summary. Recommended: yes, as its own pull request
   after page 3: key `recap_summaries` by window and instance end, pruned by the retention that already
   prunes it.
5. **Q5. Show plugin URLs on the Config page?** They are hidden, because a webhook URL can carry a token
   in its path. Recommended: keep them hidden in page 7, and add an origin-only row (scheme, host and port) as a
   follow-up if the endpoint is worth naming; an origin cannot carry a path token.
6. **Q6. Is herdr the only source of "live" for Sessions?** A session running outside herdr never appears.
   Recommended: yes; herdr is the multiplexer of record on this machine, and pns has no liveness of its
   own to fall back on.
7. **Q7. Show the router reading on Right now before it drives delivery?** Recommended: yes, with its "not
   used for delivery yet" clause, until the approved daemon router poll lands and removes it.
8. **Q8. Does the Health page's pull request build `pns doctor --no-send`, approved and not yet built,
   rather than a separate pull request?** Recommended: yes. The page cannot ship without it, and building it once avoids two
   branches editing `command_doctor.rs` at the same time.
9. **Q9. Approve the neutral tone?** The chosen CSS has red and amber only; settled states (Delivered,
   Done, Idle, a passing health row) use the muted color with a filled dot. Recommended: yes.
