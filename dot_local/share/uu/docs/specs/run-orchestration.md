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
