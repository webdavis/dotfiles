# The stale-block escalation: one page about a session nobody came back to

The reminder's sibling, and the second path in pns that speaks about an event long after it happened. When a
session is waiting on the operator, the event that started that wait records the second it began on the
session's own row and registers one leased job with the daemon. An hour later the daemon re-executes this
binary as `pns stale`, and that run (the "fire") pages ONCE about every session still waiting, on
`priority`, the route reserved for things that need a human.

Four rules decide everything below. First, one block earns at most one page, because the fire stamps the
row it pages about and the stamp is the claim. Second, a page is only sent where the operator could act
on it: nothing goes out while they are away from both the desk and the phone, or while the screen has
been locked for the whole window. Third, the ROW is the authority and not a marker file, because the
blocked marker is written only where the lamps are configured and an escalation built on it would be
silently dead on a machine with no `[lights]` table. Fourth, the wait's start and end are
`lights::phase::blocked_marker_action` over `pulse::LAMP_BLOCKED`, the list the lamps already carry, read
rather than copied.

Vocabulary. A **block** is one session's wait on the operator, `blocked_since` on its `sessions` row. The
**window** is `[stale] escalate_after`, "how long a session stays blocked before ONE page about it goes
to the priority route". The **fire** is one run of `pns stale`. The **claim** is the `escalated_at` stamp,
taken under `escalated_at IS NULL` inside the write, which is a compare-and-swap the database arbitrates.
The **gate** is `stale::gate`, the total function that says whether this moment earns a page.

## State

Two columns on the `sessions` table, created by migration step 9 with the table itself:
`blocked_since INTEGER` and `escalated_at INTEGER`, both null when the session is not waiting. The wait
is recorded whether or not this feature is on, because the return card lists open waits off the same row
(`missed-notifications.md`); this feature owns the job and the `escalated_at` stamp. Nothing else is
added: the design rejected `ledger_events`, which has no session column and no timestamp and is never
pruned, and rejected a file per session, which would need a sweeper where a row replaced in place needs
none.

## 1. The window is the schedule and `[stale] enabled` is the switch

Given an operator who wants to hear about a session nobody came back to

When `[stale] escalate_after` is read out of the configuration file

Then 1m to 24h arms the feature at that window, `enabled = false` is the feature off, and every other
value, `"0s"` included, is refused by name.

- Success: an armed `[stale]` table with nothing said carries the default, 3600, and the switch over it
  defaults true
  (`config/tests/stale.rs:the_escalation_window_defaults_to_an_hour_and_the_switch_is_what_turns_it_off`),
  and the shipped template writes both uncommented at those defaults, per the defaults-visible ruling of
  2026-08-31. `Config::stale_window_secs` is what the page reads: the window while the switch is on and
  zero while it is off, which is `WINDOW_OFF`'s own reading.
- Failure sources: a negative number, an integer, 59, and 86401, each refused with the offender named
  (`config/tests/stale.rs:an_escalation_window_that_is_not_a_duration_is_refused_by_name`).
- Fail direction: an unreadable config reads as OFF (`wait_runtime.rs:stale_settings`), the same
  direction `remind_delay_secs` takes and for its reason.
- Thresholds: 60 admitted, 59 refused; 86400 admitted, 86401 refused; zero refused with a sentence
  naming the key and pointing at the switch, because an unset window is an hour rather than off and
  there is nothing absence could say.
- Compatibility contract: DEFAULT ON at an hour, where `[remind] delay` beside it is default off. The two
  defaults make different mistakes: a nudge nobody asked for interrupts a session the operator is
  already watching, and a page nobody asked for arrives about a session stuck for an hour, which is the
  one thing they would want to know.

## 2. Recording the wait, and the job that times it

Given a session that has just started waiting on the operator

When the event path writes its records

Then `blocked_since` is stamped on that session's row and one leased job `stale:<session>` is registered
with `due = now + window`, `until = due + window`, no `unless_marker` and args `["stale"]`.

- Success: every state in `pulse::LAMP_BLOCKED` arms it, for EVERY harness
  (`track_wait/tests.rs`, `tests/hooks/stale_arming.rs`). The reminder's claude-only gate does not carry
  over: it exists because a five-minute nudge would be wrong in the common case for a Codex turn that
  runs tens of minutes, and an hour is past any normal turn.
- Where it happens: inside the same call as the blocked marker (`SessionWait` beside `BlockedMarker` in
  the record tail), because the row and the job are one fact stated to two readers. Arming from the hook
  arms instead would put them in different places, where an event reaching one and not the other leaves
  a row nothing pages about or a job with no row to find.
- No `unless_marker`: the reminder's answered marker is written by every Stop and StopFailure, so sharing it
  would cancel almost every escalation before it fired. The row is the authority instead, and the cost is
  one no-op spawn per answered block, an hour after it was answered.
- Failure sources: a window of zero registers no job and still stamps `blocked_since`, with
  `escalated_at` stamped at the same second, so the wait is recorded and no fire can ever claim it:
  switching the escalation on later pages only waits begun after that
  (`sqlite/tests/sessions.rs:a_wait_begun_with_the_escalation_off_is_never_paged_about`,
  `tests/hooks/stale_arming.rs:a_blocked_wait_is_recorded_for_the_return_card_whether_or_not_the_escalation_is_on`);
  no clock arms nothing, never a wait at epoch zero; a session id that cannot be a filename records
  nothing at all; a row that cannot be written schedules no job and says so on stderr.
- Required side effects: none beyond the row and the spool entry. Both are local disk writes on a
  synchronous hook path, in `ArmRemind`'s budget: no network, no subprocess, no wait.
- Forbidden side effects: nothing here delivers, and no free text reaches the spool. The fire reads the
  row, so the argv is the subcommand and nothing else.
- Idempotency: the job id is the spool filename, so a second wait in one session REPLACES the job rather
  than stacking a second one, and the new wait clears the previous `escalated_at`.

## 3. Ending the wait

Given a session whose wait is over

When any state outside `pulse::LAMP_BLOCKED` reaches the event path, or the `prompt` or `resolved` hook
arm ends the wait directly

Then `blocked_since` and `escalated_at` are both cleared, unless the wait began after the moment being
cleared for.

- The three call sites are the record tail's own `SessionWait::track` and `wait_runtime`'s
  `end_blocked_wait`, which ends the marker and the row in one call and is what both hook arms reach.
- Fail direction: clearing is UNCONDITIONAL of the window, like the start, so an open row always means
  a wait nobody has ended.
- A late clear keeps a newer wait: every answer arm is async, so a batch's clear can land after the next
  approval began its wait, and the row compares `blocked_since` against the caller's moment exactly as
  the blocked marker compares its epoch. No clock is an unconditional clear
  (`sqlite/tests/sessions.rs:a_late_clear_leaves_a_wait_begun_after_its_own_moment`). A clear that
  started after the new wait began still takes it, the same residual the marker names.
- An observation (`model-switch`, `quota`, `config-change`) changes nothing, because the record tail
  returns before either write for any attempt that is not the first. That is the blocked marker's own
  neutrality, deliberately shared.

## 4. The fire: what it selects, and the claim

Given a daemon tick that woke a `stale:<session>` job

When `pns stale` runs

Then it selects `blocked_since IS NOT NULL AND blocked_since <= now - window AND escalated_at IS NULL`,
oldest first, stamps each row it is about to page, and pages once per row it stamped.

- Thresholds: a block as old as the window is selected; one second short of it is not
  (`sqlite/tests/sessions.rs:a_block_as_old_as_the_window_is_selected_and_one_second_short_of_it_is_not`).
- The claim is the stamp. Two fires woken in one tick produce one page between them, because the write
  carries its own `escalated_at IS NULL`; no lock file of its own is taken, which is where this differs
  from the reminder's `fire.lock`.
- Stamped on ATTEMPT, never on success, which matches the reminder's honesty: a mute, a Focus or an empty plan
  can suppress delivery, and a page that retried every hour because the first one was muted is the
  failure mode worth avoiding.
- `pns stale` takes no argument, and one is a refusal with exit 2: one fire covers every stuck session,
  so an argument is a value it would have to ignore.

## 5. The gate: only where the operator can act

Given a fire that found something stuck

When it reads the surface, once, off the same probe set every event path uses

Then `Surface::Away` is silent, a `screen_locked` of `Some(true)` with a desk idle age at or past the
window is silent, and anything else pages.

- Away is a skip because the thing a blocked agent needs is a keyboard, and `Away` means neither the desk
  nor the phone has a fresh reading.
- One instantaneous lock reading answers a question about an hour: unlocking a Mac takes input and input
  resets the idle clock, so a desk idle for the whole window cannot have been unlocked inside it. pns
  keeps no history of the lock.
- A screen locked for PART of the window still pages, which is the case the feature exists for: the
  operator was there, stepped away, and a session is stuck.
- Only `Some(true)` locks, matching `surface`'s own rule, so an `ioreg` that stops answering costs the
  suppression rather than the page; an unknown idle age is the same direction.
- The gate is read BEFORE any claim, so a suppressed fire leaves every row as it found it and a later
  fire can still escalate the block. It says how many it held back, and why, on stderr, because that is
  the stream the daemon keeps.
- ACCEPTED LIMIT: the job is a one-shot, the shape `arm_remind` already uses and for its reason (a
  held-back remind is lost rather than queued), so a fire suppressed while the operator is away does not
  fire again by itself. What reaches that row is the NEXT wait-starting event of that session, another
  session's fire sweeping every row at once, or `pns stale` typed at the desk, and nothing at all until
  one of those happens: a block that stands through a night on an idle machine is never paged about. The
  recap section the design names for this is not built yet.

## 6. The page

Given a row the fire has stamped

When the page is raised

Then it is one ordinary event on the `priority` route, state `blocked`, detail
`blocked <n> minutes, no answer`, carrying the row's project, branch, session and title.

- The header and subheader compose exactly as every other event's do, at the hermes destination, so a
  page reads as the same line the `pns` channel already carries for that session.
- The route is fixed and not configurable: `priority` is defined as machine health and security plus this
  escalation (operator ruling, 2026-09-14), so a key naming another route would be a config that
  contradicts the definition.
- No pane: the fire is a daemon child with no `HERDR_PANE_ID`, and the row is not where a pane belongs. A
  page that focuses nothing is honest; an invented pane id would focus somebody else's.
- It is an `Attempt::Nudge`, so it journals no miss, counts as no activity, claims no return moment and
  pulses no lamp: it is a second card about an event already recorded.
- A page the gateway refuses is recorded in the delivery ledger, where `pns failures` reads it, and said
  on stderr naming the route. Nothing about a refused page is silent.

## Gaps

- No test drives a real daemon tick through to a page an hour later: this binary has no clock override,
  so the job's registration and the fire are pinned separately instead.
- No test pins the `priority` route's own gateway configuration, and none can: the committed hermes
  config is age-encrypted, so a test would need an identity CI never supplies. The route now carries the
  key pns signs with and a prompt in the pns body's shape, and `run_after_68` compares every route's
  secret against pns's on each apply, which is the only gate that can see the value.
- The recap section that would carry a suppressed escalation is designed and not built.
