# Run orchestration

`uu run [<lane>]` follows the command, record and state contracts below. `uu-application` owns
sequencing; adapters parse configuration, run commands and deliver records and alarms.

## Entering a run

- **Given** no home directory in the environment, **when** entering the command, **then** report that no
  configuration can be read and exit 1 before composing the use case.
- **Given** no configuration, **when** invoked without a lane, **then** report that nothing is enabled
  and exit 0 without running lanes. A named lane in that situation is an unmet request: diagnose the
  missing declaration and exit 1. Configuration errors retain their existing diagnostics and exits.
- **Given** configured lanes, **when** the run lock is contended or unavailable, **then** return the
  corresponding typed refusal before pruning, sampling time, executing a lane or delivering anything. The
  command exits 1. Only contention claims another run holds the lock; both failures retain their cause.
- **Given** an acquired lock, **when** running all lanes or one selected lane, **then** keep the guard
  through every run effect and release it on return. Request pruning with the complete declared name set
  before sampling the header. Selection must not prune other configured lanes.
- **Given** an acquired lock, **when** preparing lane facts, **then** sample the start time and previous
  marker before lane work or marker publication. The header and every lane receive those same facts. An
  unreadable start clock renders epoch zero; it must not invent a plausible current time.
- **Given** a selected name that yields no report, **when** execution returns, **then** return
  `UndeclaredLane`, release the guard and publish no run record or success marker. The command exits 1.

## Executing lanes

- **Given** declared lanes, **when** executing them, **then** visit names in sorted order and continue
  after a failed report. A selection executes only its matching name.
- **Given** the monotonic clock started under the lock immediately before the lane loop, **when** each
  lane starts, **then** pass both its declared deadline and its actual remaining budget to the executor.
  The budget is `min(declared, 24 hours - elapsed)`, with subtraction saturating at zero. One second
  before the run limit leaves at most one second; at the limit and one second after it, the budget is
  zero. A declared deadline below, equal to or above the remaining time yields the smaller duration.
- **Given** a lane with no explicit deadline, **when** configuration resolves it, **then** its default
  remains six hours. The application passes the resolved duration and does not parse configuration.
  Process enforcement stays with the executor and watchdog. The 24-hour value bounds lane budgets; it is
  not a new timeout around alerting, record delivery, state access or the whole function.

## Reporting and retaining history

- **Given** lane reports, **when** composing the run, **then** print the record detail before attempting
  per-lane failure alerts, process staleness after those alerts, and deliver the run record afterward.
  Pending escalation follows staleness, before record delivery. Count failed operations, deferred lanes
  and pending lanes separately. The outbound state is `failed` if any operation failed, otherwise
  `deferred` if any lane deferred, otherwise `pending` if any lane is pending, otherwise `completed`.
  Pending lanes are named `pending` in the detail and counted in its closing line.

- **Given** an alert channel, **when** delivery fails, **then** report its cause and continue the run. An
  unconfigured channel produces the existing log notice and owes no external delivery. Neither case is
  silently reported as a successful external send.

- **Given** per-lane history, **when** a lane completes or reports pending work, **then** reset its
  non-success streak to zero. A failure or deferral increments it with saturation at the maximum `u32`
  value. Counts one and two do not trip; reaching three trips once; advancing from three to four does not
  alert again. A later success permits a new streak to trip. Lanes retain independent histories.

- **Given** absent streak history, **when** counting the current attempt, **then** start from zero.
  Unreadable history is a different state: attempt a diagnostic alert and treat it as two, one below the
  threshold. A failed or deferred attempt can then trip instead of silently forgiving its history.

- **Given** a trip at three, **when** attempting its alert, **then** do so before writing the new streak.
  If the configured alert fails, write two so the next non-success retries. A delivered or unconfigured
  alert permits writing three. Repeated failed delivery may therefore repeat the attempt; this is not an
  exactly-once delivery guarantee.

- **Given** a streak write failure, **when** the adapter returns its location and cause, **then** emit
  the bookkeeping diagnostic and attempt a lane alert. Do not report a successful write. This does not
  add a separate condition to the run's marker policy.

- **Given** a command child that exits 100, **when** collecting its result, **then** retain stdout and
  its stderr-derived reason as pending work. Exit 75 remains deferred; other unsuccessful exits remain
  failures. A report's typed verdict gives failure precedence over deferral, then pending, then
  completed.

- **Given** a configured lane, **when** parsing `escalate_after_runs`, **then** accept a positive whole
  number through the maximum `u32` value, defaulting to three. Resolve it before type-specific parsing
  and pass it to the application with the lane's deadline.

- **Given** consecutive pending runs, **when** the count reaches that lane's threshold, **then** attempt
  one pending alarm before publishing the count. Below the threshold and beyond it, do not trip. Counts
  saturate; any non-pending verdict resets the count. Unreadable history is reported and treated as one
  below the configured threshold. A refused alarm keeps that value so the next pending run retries. Write
  failures are reported as pending bookkeeping failures and alerted. The state adapter keeps this count
  in `lanes/<name>/pending`, separately from `streak`; existing whole-directory pruning covers both.

- **Given** a configured failure webhook, **when** any failed, stale, pending or record-lost alarm is
  raised, **then** post its kind, sampled host and detail using the existing four-field record body and
  records signing key. Attempt the webhook and configured pns engine independently, once each. Any
  configured refusal makes the combined alarm unsuccessful and retains one-shot retry eligibility. A
  later retry may reach a destination that accepted the earlier attempt. It does not change the run exit
  or add a condition to marker eligibility. With the webhook absent, delivery remains pns-only.

## Delivering the record and advancing the marker

- **Given** an unconfigured record channel, **when** reporting a run, **then** log that nothing was
  posted and treat the record as not owed. A configured channel that cannot sign is a lost record:
  diagnose it, do not post, and do not advance the success marker.
- **Given** a rejected record, **when** the adapter returns its destination, description and typed cause,
  **then** print the result and attempt a whole-run alert. Status rejection, no response and no status
  remain distinguishable. Even successful delivery of that alert cannot make the record count as
  received. The existing record adapter retains its ten-second POST deadline.
- **Given** zero failures, zero deferred lanes and a delivered or unconfigured record, **when**
  finishing, **then** sample the finish clock and request a marker write using that time. Do not reuse
  the header time. Any failed lane, deferred lane or lost record forbids this marker write.
- **Given** an unreadable finish clock, **when** a clean run ends, **then** keep the old marker and name
  its location in the diagnostic. A guessed timestamp must not shorten the next reported gap.
- **Given** a marker write failure, **when** it is returned, **then** report the location and cause. The
  existing filesystem writer does not promise preservation of the previous bytes on every write failure.
  `Completed` still means orchestration completed, and maps to exit 0; lane, delivery and bookkeeping
  failures are reported through their existing channels rather than a new exit policy.

## Boundaries retained by this extraction

The application receives typed values and makes no filesystem, environment, process or network calls. Its
ports do not receive signing keys or configuration tables. Existing adapters own record encoding,
argument vectors, diagnostics and process cleanup. Alert arguments remain separate opaque values, passed
with `Command::args`; the use case must not reinterpret report text as shell syntax. Existing payload
contents and diagnostic disclosure are preserved, with no new redaction or logging policy.

Lane watchdogs retain their process-group termination and escaped-process diagnostics. The alert adapter
still waits with `Command::status` and has no deadline of its own. Both signed-post paths retain the
ten-second deadline. No new cancellation mechanism or stronger cleanup guarantee for escaped or stuck
spawns is introduced. The new pending file uses the existing count encoding and atomic writer; no store
migration or deduplication is introduced. See [the ownership decision](../decisions/run-application.md)
for these retained limits.

## Neovim plugin pins

- **Given** a `nvim-plugins` lane, **when** resolving configuration, **then** require an absolute
  `config` directory and default its executable to `nvim`. Run it headless with that directory's
  `init.lua` and `lua/uu/plugins.lua`, preserving paths containing spaces. Retain child output for
  completed, pending and failed results. Exit 100 is pending; every other nonzero exit is failed.
- **Given** report-only mode, **when** checking plugins, **then** wait for Lazy's check, list each
  pending or failed plugin by name and count current plugins without listing them. Any plugin error makes
  the report failed; otherwise updates make it pending. Exit through Neovim so the completed headless
  instance releases its server socket.
- **Given** `auto_commit = true`, **when** resolving configuration, **then** require an absolute `repo`.
  Its default is false; a non-boolean value is refused with the key and written value.
- **Given** an enabled writeback with no open recovery, **when** its branch or lock preflight is refused,
  **then** report the reason and check only. Require a branch, a clean source lock, equality of
  committed, indexed and deployed lock bytes, and installed lock-managed revisions matching the lock.
  Local plugins are outside lock management.
- **Given** an allowed writeback, **when** updating, **then** durably save the repository, config,
  branch, starting commit and old lock before any update. Save the candidate lock after the update,
  including a failed update. Immediately before copying, recheck branch, commit, source and index lock
  bytes. Preserve intervening edits and retain recovery on any refusal.
- **Given** changed plugin pins, **when** committing, **then** copy and commit only
  `dot_config/nvim/lazy-lock.json` with ordinary hooks, preserving unrelated staged paths. Report
  completion only after committed, deployed and installed pins agree. An unchanged candidate closes
  recovery without an empty commit. Update, copy, hook and commit failures remain failed and retain both
  old and candidate lock bytes.
- **Given** open recovery, **when** the lane runs again, **then** refuse checks and updates until the
  operator's clean committed, deployed and installed pins agree, even if auto-commit is now off. After
  that agreement, archive the recovery record and resume the requested mode.
- **Given** a successful changed update, **when** other Neovim sockets are present under the per-user
  runtime root, **then** count other process identifiers once, excluding our own, and print
  `N Neovim instance(s) were running during this update; restart them to load the new versions`. With no
  other socket, print no notice.

## Mason tools

- **Given** a `nvim-mason` lane, **when** it runs, **then** invoke its required absolute config's
  `lua/uu/mason.lua` through the shared headless host, retaining its own lane name and child failure.
- **Given** the tool roster, **when** updating, **then** refresh the registry first and subscribe to each
  package's success and failure events before `MasonToolsUpdateSync`. Treat the completion event only as
  completion. Any failed package fails the lane with its reason, including when the command subsequently
  throws. Report updated and current tools and the language-server sentence on every run.
- **Given** either generated job, **when** an installer launches an interpreter, **then** search the
  home's managed Node directory before system tools, and include the home's Cargo binary directory.
  Preserve existing home and property-list escaping.

## Parser reconciliation

- **Given** a `nvim-parsers` lane, **when** it runs, **then** invoke its required absolute config's
  `lua/uu/parsers.lua` through the shared headless host, retaining its own lane name and child failure.
- **Given** installed parsers, **when** reconciling the locked installer revisions, **then** call
  `update(nil, { summary = true }):wait()`. A false result or exception fails the lane. Name updated and
  current parsers and retain each failed compiler's output tail.
- **Given** either lane changed installed tools or parsers, **when** reporting, **then** append the
  existing restart notice if another Neovim instance's socket is present.

## Candidate startup verification

- **Given** an enabled `nvim-smoke-test` lane, **when** it runs, **then** require an absolute cache, copy
  config and Mason, and run prepare then a fresh verifier with private config, data, state, cache, HOME
  and Claude discovery roots. A failed prepare never reaches verification.
- **Given** a candidate, **when** verification ends, **then** require this run's completion and exact
  lock, actual VimEnter, no startup diagnostics and no stderr, with child failure and timeout counted.
  Retain raw diagnostics and report paths. Health error and warning counts do not change startup status.
- **Given** a completed keymap capture, **when** recording, **then** write mode, left-hand side and
  right-hand side or description as three tab-separated fields, and compare additions and removals by
  mode and left-hand side. A first dump explicitly has no previous comparison.

## Bootstrap

- **Given** `uu bootstrap <lane>`, **when** its config and registered capability are available, **then**
  hold the same run lock while calling that capability and print its report lines. Return zero only for
  completion. Never post a record or alert, stamp success, prune state or change either streak.
- **Given** an undeclared lane, an absent config or a type without bootstrap capability, **when**
  requested by name, **then** refuse with exit one and name the missing declaration or unsupported type.
  A refused lock never invokes the capability. Usage lists bootstrap beside the other commands.

## Claude Code plugin records

- **Given** a `claude-plugins` lane, **when** loading config, **then** require an absolute `inventory`
  path. Read one JSON document with a nonempty `plugins` object; validate each plugin's array and each
  record's object and scope string before filtering to user scope. A malformed record fails the whole
  reading. Other scopes alone form a valid empty reading.
- **Given** a user-scope record, **when** reading its fingerprint, **then** prefer a nonempty version
  other than `unknown`, then a nonempty commit, then `unknown`. Preserve the retired reporter's escaped
  tab-separated fields. Report only identifiers and fingerprints with the shared quoting and caveat.
- **Given** snapshot history, **when** running or bootstrapping, **then** keep a valid uu snapshot first.
  Otherwise import the validated sorted legacy name/fingerprint pairs atomically, including an empty
  file, preserving its exact bytes and leaving the legacy file alone. Seed only when both are absent.
  Failed reads, validation and publication fail the lane without reseeding.
- **Given** a first reading, **when** running, **then** save a baseline and compare nothing. Later runs
  report changes and atomically replace the snapshot through an owner-only sibling temporary file. The
  snapshot advances during the lane, before delivery of the combined record. A refused delivery can
  therefore leave that comparison absent from the next record, unlike the retired bash job.
- **Given** an existing or imported baseline, **when** bootstrapping repeatedly, **then** keep it without
  comparing or advancing it. A non-regular inventory is refused, and failed publication preserves the
  previous snapshot. State uses the declared lane name under `~/.local/state/uu/lanes/`.

## Skills roster

- **Given** skills settings, **when** parsing the callable component, **then** require every path to be
  absolute and name invalid or unknown fields. The skills type remains unavailable to run and bootstrap
  until its complete cutover; the shipped block stays commented.
- **Given** the custom roster, **when** capturing it, **then** require one version 2 document with typed
  tables and a nonempty npx/clawhub union. Refuse conflicting Hermes registry/profile ownership and
  refuse publication if the original roster bytes changed.

## Skills generations

- **Given** a candidate build, **when** starting, **then** capture the updater executable digest before
  mutation. Ready metadata records its directory id, creation time, roster digest, updater digest and
  full/additive mode. Recovery offers only compatible full candidates for weekly validation; missing or
  incompatible metadata stays retained.
- **Given** an interrupted exchange, **when** recovering, **then** finish the atomic directory swap and
  retain the outgoing generation and its skill names until pruning finishes. A retention failure keeps
  the marker and workspace. After pruning, reclaim the owned installer workspace, resume interrupted
  garbage removal and keep exactly one previous generation.

## Skills npx installs

- **Given** a candidate and npx roster, **when** installing, **then** run one explicit skills add command
  per repository group, name every failed skill and continue other groups. Reconcile single-document
  candidate and installer locks; a full refresh drops delisted keys and additive builds preserve existing
  entries.
- **Given** an installer child, **when** spawning, **then** clear inherited environment and keep
  candidate HOME, base directories, temporary files, npm cache and ClawHub config. Capture the real fnm
  interpreter directory before redirecting HOME. The existing lane deadline still bounds the child.

## Skills ClawHub installs

- **Given** an absent ClawHub skill, **when** installing, **then** use an owned throwaway workdir and
  move its nested directory flat with origin metadata. Refresh a present skill by bare name. Retry
  local-change refusals only after stripping our own policy block, then reassert it while preserving
  updated upstream metadata.

## Skills candidate validation

- **Given** candidate skills, **when** asserting Codex tiers, **then** add the on-demand policy and
  remove it from core skills while preserving upstream metadata. Refuse overlay symlinks.
- **Given** a candidate, **when** validating, **then** require every tracked directory and SKILL.md,
  ClawHub origin metadata, one npx lock document and correct overlays. Full lock keys must equal the npx
  roster; additive candidates may retain delisted keys. Refuse the whole candidate on any failure and
  retain its renamed HOME for diagnosis, leaving the current generation alone.

## Skills publication

- **Given** a fresh or recovered candidate, **when** publishing, **then** validate its E9 content and
  captured roster before the atomic first rename or generation exchange. Recovery validates a published
  generation before retaining the outgoing copy or reconciling store entries.
- **Given** a full publication, **when** reconciling the store, **then** remove only exact managed
  delisted links and quarantine only outgoing-owned delisted real directories. Foreign entries survive.
  Replace a tracked real directory only after its recorded content was absorbed, and report a writer
  whose content changed or was never recorded. Additive publication preserves existing entries.
- **Given** an interrupted publication, **when** pruning or workspace cleanup fails, **then** retain the
  journal and outgoing ownership until a retry finishes both. Quarantined store content remains available
  under `.agents/.skills-quarantine/<generation>/<name>`.

## Skills delivery links

- **Given** the reconciled store, **when** delivering skills, **then** Claude receives each surviving
  skill unless its delivery row says `none`. Hermes receives only its mapped profiles, excluding
  `humanizer` and `hyperframes`. Preserved foreign skills remain eligible.
- **Given** a missing destination, **when** either mode delivers links, **then** create its parents
  first. Refuse a Hermes profile parent or skills child that is a symlink before writing through it.
- **Given** mapped and existing Hermes profiles, **when** full convergence runs, **then** repair
  incorrect owned links and remove stale owned links, including in demapped profiles. Additive
  convergence preserves existing entries. Both modes preserve foreign links and real entries.

## Skills follow-up phases

- **Given** Hermes registry entries, **when** refreshing profiles, **then** update every entry by its
  lock key in each declared profile, skip held entries visibly, and continue after failures. Output
  containing `blocked` or `refused`, ignoring case, counts as failure even at exit zero.
- **Given** a fork watch, **when** comparing upstream trees, **then** preserve all ten advisory states:
  drift, missing path, broken lock, missing lock, absent table, unreachable upstream, headless clone,
  unstageable clone, clone timeout and incomplete walk. Name old and new hashes for drift. Use an owned
  clone, clear inherited Git configuration and bound each clone to five minutes within the lane budget.
  Remove the owned clone after either outcome. These states remain pending and do not count as failure.
- **Given** an app-owned pack link, **when** refreshing it, **then** count a failed refresh and preserve
  its output. Check routing first, leave clean routing alone and count a failed repair with its output.

## Weekly skills composition

- **Given** a weekly skills execution, **when** building a candidate, **then** recover first, migrate a
  flat store when needed and recover again. Capture before fingerprints, build or reuse a full candidate,
  validate and publish it, and prune outgoing ownership before fan-out. A failed recovery withholds
  another build and retains its journal. A failed build leaves the current generation alone.
- **Given** a failed candidate phase, **when** continuing the weekly run, **then** still attempt the app
  pack, full fan-out, live overlays, routing, Hermes registry and fork watch. Compare after fingerprints
  with the captured before values; an unreadable fingerprint says `NOT COMPARED`. All required failures
  remain in the report. Live managed overlays are read from the generation without writing through store
  links; owned vendored directories may have their policy reasserted.
- **Given** the inactive skills component, **when** later composing its cutover, **then** capture the
  updater digest at process startup before earlier lanes can run. Sessions reuse that immutable digest.
  This component does not register or enable weekly execution or bootstrap.

## Additive skills bootstrap

- **Given** a roster skill, **when** bootstrapping, **then** repair absent content, wrong generation
  links, missing `SKILL.md` or missing npx lock entries by reinstalling only that skill. Overlay-only
  drift rebuilds without installing; remove an obsolete owned policy from core skills as well as adding
  missing on-demand policy. A healthy store skips publication, then still performs additive fan-out, live
  overlay checks and routing.
- **Given** an additive publication, **when** cloning and delivering it, **then** retain delisted
  generation names, npx keys, store entries and existing delivery links. Fill missing Hermes destinations
  under the same parent and child guards, both with and without publication. Do not migrate a flat store
  or refresh a healthy skill. Required phase failures remain failures.
