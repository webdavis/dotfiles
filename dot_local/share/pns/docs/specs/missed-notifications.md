# Missed-notification recording and replay

## Scope

This specification covers what `pns` writes down when a notification could not have reached the operator,
and what happens when the operator comes back. It covers the journal (the file `missed-notifications`,
one JSON object per line, oldest first), the predicate that decides an event belongs in it, the claim
protocol that arbitrates between several short-lived processes reaching for the same journal at the same
moment, the `Moment` model that makes one event the owner of a whole return, the window's near edge
(`last-present`) and the stranded window claims left behind when a run dies holding it, the staleness and
abandonment rules that let a later run adopt a hold, and what a replay actually delivers. It does not
cover the decision ring (`decisions`), the surface and visibility plan that decides whether a live event
decorates at all, the lights, or the Discord recap's own rendering beyond the point where the replay path
spawns it. The activity ring (`activity`) appears here only because the return moment counts its window;
it is never claimed and never consumed (`src/main.rs:record_activity`).

______________________________________________________________________

### 1. Ownership is taken by rename or by exclusive create, never by removal

Given two or more short-lived `pns` processes reaching for the same state file at the same moment

When one of them has to become the single owner of that file's contents

Then ownership is decided by `rename(2)` onto a name carrying the claimant's process id, or by an exclusive `create_new` open, and never by unlinking the contended path.

This is the invariant the whole area is built on, and it is measured rather than assumed. The code states
it in five places:

- `src/main.rs:take_claim`: "THE RENAME IS THE OWNERSHIP TEST, and the remove is no longer one. It used
  to be, on the premise that only one of two runs reading a stranded claim could unlink it. MEASURED on
  macOS 26.2 (APFS), that premise is false: eight processes unlinking ONE path were every one of them
  told they had succeeded, and two racing runs that both read one claim both delivered it (reproduced
  twice in 1500 rounds). A rename does arbitrate, measured in the same run: 40 rounds of eight racers,
  one winner every time."
- `src/main.rs:claim_journal`: "The unlink used to hold that job and MEASURED it cannot: on macOS 26.2
  (APFS) eight processes unlinking ONE path were every one of them told they had succeeded."
- `src/main.rs:claim_moment`: "An unlink cannot stand in: MEASURED on macOS 26.2 (APFS), eight processes
  unlinking one path were every one of them told they had succeeded."
- `src/main.rs:claim_lock`: "THE DEAD LOCK IS TAKEN BY RENAME AND NEVER BY REMOVE, which is the one place
  arbitration is still needed on this path: a remove reports success to EVERY racer on APFS (measured,
  eight racers all told they had succeeded), so two processes clearing one dead lock would each then
  create a fresh one and both would own the window."
- `src/main.rs:update_blocked_marker`: "Unlink cannot arbitrate on this filesystem (concurrent unlink
  reports success to every caller on APFS)."

Removal still appears in this area, but only after ownership has already been decided by a rename, and
only on a path named for the removing process (`src/main.rs:take_claim` removes the held file it itself
renamed into place; `src/main.rs:claim_moment` removes the window claim it itself took).

- Success: exactly one racer delivers a given batch, and exactly one racer owns a given return moment.
  Pinned deterministically by
  `tests/dispatch.rs:a_journal_this_run_could_not_read_is_left_on_disk_rather_than_consumed`, which
  asserts the leftover sits under the `missed-notifications.held.` name and comments "A build that reads
  the claim where it lies and owns it by unlinking leaves the claim name here instead, and owns nothing";
  and by `tests/dispatch.rs:an_event_inside_another_runs_return_moment_delivers_no_card_of_any_kind`.
  Corroborated under load by
  `tests/dispatch.rs:racing_present_events_deliver_exactly_one_replay_between_them` (8 racers, hard
  assertion) and the ignored soak
  `tests/dispatch.rs:racing_present_events_adopt_one_stranded_claim_exactly_once` (24 racers, "roughly
  one catch in 200 rounds").
- Failure sources: a rename that fails for any reason (the source is gone because another racer won, the
  directory is unwritable, the path is on another filesystem). Every one of them is read as "this run
  does not own it" and returns `Claimed::Nothing` or `Moment::Busy`.
- Fail direction: safe. A failed rename never delivers and never destroys. `src/main.rs:Claimed` spells
  out the four outcomes and states that only `Taken` may hand entries onward.
- Thresholds: not applicable to the invariant itself. The two age thresholds that decide when a claim may
  be taken from a process that may still exist are in behaviors 9 and 13.
- Required side effects: the contended path is left with no file at it for the width of the critical
  section, which is why every reader of these files treats "not found" as "no state" and never as an
  error (`src/main.rs:republish_after`, `src/main.rs:read_epoch`).
- Forbidden side effects: no `remove_file` on a contended path may stand in for arbitration. No check
  taken before a rename may be treated as a guard on what the rename carried
  (`src/main.rs:claim_by_rename`: "VERIFIED AFTER THE RENAME AND NOT BEFORE").
- Timeout and cancellation: not applicable. `rename(2)` and `create_new` do not block.
- Idempotency and duplicates: this invariant is the mechanism that makes replay non-duplicating. See
  behaviors 11, 12 and 13 for what is left over when a rename wins and the run then dies.
- Privacy: not applicable at this layer. The rename moves a file within one 0700 state directory.
- Process ownership and cleanup: every claim name carries `std::process::id()`, which is what lets a
  later run tell "held by somebody still running" from "left by somebody gone"
  (`src/main.rs:owner_is_gone`).
- Compatibility contract: the claim and hold names are internal. `src/main.rs:owner_is_gone` documents
  that it parses "the segment before the first dot (held.<pid>.<seq>); a bare held.<pid> from an older
  build, and the marker's claim.<pid>, both parse the same way", which is the one backward-reading
  promise made here.

______________________________________________________________________

### 2. An event the operator could not have perceived is journalled

Given an event whose plan called for neither a banner nor a phone card, on which nobody was watching the origin pane, and which was not skipped because another route already carried it

When the event path reaches the record site

Then one entry is appended to `missed-notifications`, and nothing is printed.

`src/missed_notifications.rs:was_missed` is the whole predicate, and it is three clauses over values the
record site already holds:
`!overrides.skip_phone && !watching && !decision.plan.banner && !decision.plan.phone_card`, where
`watching` is `visibility == Visible && surface != Away`. The surface half is what saves the Away row:
"an away operator is watching nothing, and a desk display showing the origin pane to an empty chair is
exactly the reading that must not suppress" (`tests/dispatch.rs` counterpart in the module's own unit
tests:
`src/missed_notifications.rs:an_away_event_is_missed_even_when_the_session_reported_the_pane_visible`).
`PNS_SKIP_PHONE` is set exactly when a moshi approval forward really happened, so that card is already on
the phone and replaying it later "would be actively wrong"
(`src/missed_notifications.rs:a_card_skipped_because_another_route_already_raised_one_is_not_missed`).

The predicate is deliberately plan-level and not delivery-level. `src/missed_notifications.rs:was_missed`
names the two limits that follow: an event narrowed with both `--local-only` and `--remote-only` reaches
no channel while its plan still says banner, so it is not journalled; and an event whose plan called for
a card on a machine with no phone channel configured is not journalled either.

- Success:
  `tests/dispatch.rs:a_missed_event_appends_exactly_one_entry_carrying_what_a_card_would_have_shown` (a
  muted event: `hermes` still fired, `mobile` did not, exactly one entry, all five fields present, `at`
  past 1700000000). The negative half is `tests/dispatch.rs:a_delivered_event_journals_nothing_at_all`,
  which asserts the file does not exist at all on a machine that never missed one.
- Failure sources: an unwritable state directory; something at the journal's path that is not a regular
  file; a ring lock held past every attempt; a read-back that fails. All are swallowed.
- Fail direction: the notification still goes out. `src/main.rs:record_missed` is fail-quiet by design:
  "An event path whose stdout a harness hook reads must not gain a line about the state directory, and a
  journal entry that did not land costs a replay, never a card."
  `tests/dispatch.rs:a_state_directory_that_cannot_be_written_costs_a_missed_event_nothing` asserts exit
  0, `hermes` fired, empty stdout and empty stderr (the whole stream, not a substring).
  `tests/dispatch.rs:a_fifo_at_the_journals_path_is_refused_untouched_and_never_parks_the_event` asserts
  the same and that the FIFO is still a FIFO.
- Thresholds: not applicable. The predicate is boolean over already-held values, with no clock and no
  count in it.
- Required side effects: exactly one appended line (the separator rides in the same write, so two racing
  appends cannot interleave, `src/main.rs:append_ring_line`). The file is created 0600.
- Forbidden side effects: nothing is printed on stdout or stderr; no second clock read is taken (the
  epoch is `decision.inputs.now_secs`, "two readings of one moment can disagree",
  `src/main.rs:record_missed`); the record site never learns that `[recap] replay_card` exists (see
  behavior 17).
- Timeout and cancellation: the ring lock is bounded at `RING_LOCK_ATTEMPTS` (200) attempts with a 1 ms
  sleep between them, and a holder older than `RING_LOCK_STALE_SECS` (5 seconds) is read as an orphan
  (`src/main.rs:claim_ring_lock`). Giving up returns `WouldBlock` and costs the one entry.
- Idempotency and duplicates: one event writes at most one entry, at one site, reached only on
  `Attempt::First` (behavior 3). A miss and a replay are mutually exclusive by construction: the replay
  predicate requires `plan.banner || plan.phone_card`, which the miss predicate negates
  (`src/missed_notifications.rs:should_replay`: "a run whose plan decorated nothing is exactly a run that
  JOURNALS, so a miss and a replay are mutually exclusive by construction: no event can deliver the entry
  it just wrote").
- Privacy: the entry holds the operator's own text. The file is created 0600
  (`src/main.rs:STATE_FILE_MODE`), the module's own header states "no pns command ever prints an entry,
  and the only thing that reads an entry back is the replayer", and
  `tests/dispatch.rs:the_journal_is_created_readable_and_writable_by_its_owner_alone` asserts the mode
  after both the create and the prune.
- Process ownership and cleanup: the append leaves the ring lock removed on drop (`src/main.rs:HeldLock`)
  and leaves no pending file behind (`src/main.rs:publish_state_line` removes its pending file if the
  rename fails).
- Compatibility contract: `src/missed_notifications.rs:entries` parses by key and never by position, and
  a missing field reads as empty, so a shorter entry from another build degrades to a thinner card rather
  than to no card.

______________________________________________________________________

### 3. Only the first delivery attempt journals; a nudge and an observation never do

Given a nudge (`Attempt::Nudge`) or an observation (`Attempt::Observation`) rather than a first delivery

When the event path passes the decision record

Then it returns before the journal, before the activity ring, before `mark_present`, before `replay_missed` and before the pulse.

`src/main.rs` states the reason at the gate: "The recap counts activity-ring lines toward `min_events`,
so a nudge or an observation that rang would inflate the operator's own recap with pns's noise; neither
is evidence of presence, so neither must move the last-present marker." And: "A SUPPRESSED NUDGE IS
THEREFORE LOST, deliberately ... a 'still waiting' card replayed hours later, about a question answered
long ago, is worse than silence."

- Success: the contiguous tail below `if attempt != Attempt::First { return; }` is the whole set of
  affected writes (`src/main.rs`, immediately above `record_missed`).
- Failure sources: not applicable. This is a branch, not an operation that can fail.
- Fail direction: not applicable.
- Thresholds: not applicable.
- Required side effects: the decision ring line is still written for a nudge (it sits above the gate).
- Forbidden side effects: no journal entry, no activity line, no lease renewal, no lamp arming, no
  return-moment claim, no pulse.
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: this is one of the two rules that keep a replay from feeding itself. The
  other is behavior 16.
- Privacy: a suppressed nudge's text never reaches disk.
- Process ownership and cleanup: not applicable.
- Compatibility contract: not applicable.

______________________________________________________________________

### 4. One journal entry holds five capped text fields plus the decision's own epoch

Given an event being journalled

When `src/missed_notifications.rs:entry` builds the line

Then it is a single JSON object on one line carrying `at`, `agent`, `state`, `project`, `branch` and `detail`, each text field flattened and capped at the caller's `max_chars`, and nothing else.

The journal passes `render::PREVIEW_MAX_CHARS` (260) because "what a card renders without a cut is
exactly what a replay needs"; the activity ring passes `ACTIVITY_MAX_CHARS` (120) because "a recap line
is one line among a hundred" (`src/missed_notifications.rs:entry`, `src/main.rs:ACTIVITY_MAX_CHARS`).
Deliberately absent, each with its own stated reason: the pane ("an id from an hour ago may name a pane
that no longer exists"), the channel, the tier, and every leg verdict
(`src/missed_notifications.rs:Entry`).

The line is built with `serde_json::json!` and never with `format!`, which
`src/missed_notifications.rs:entry` names as "the Rust spelling of this repo's 'build JSON with
`jq -n --arg`' rule: interpolation is exactly how a newline in a detail would forge an entry."

- Success:
  `tests/dispatch.rs:a_missed_event_appends_exactly_one_entry_carrying_what_a_card_would_have_shown`
  reads all five fields back and asserts `at` parses as a `u64` past 1700000000.
- Failure sources: a clock that cannot be read. `at` is then JSON `null`, "which is honest and which a
  reader can tell from an absent field" (`src/missed_notifications.rs:entry`).
- Fail direction: not applicable on the delivery path. `entry` is a total function with no input or
  output.
- Thresholds: `max_chars` is 260 for the journal and 120 for the activity ring. A field of exactly the
  cap is written whole; one character more is cut by `render::flatten_reply`.
- Required side effects: none. `src/missed_notifications.rs` is policy only: "no config, no clock, no
  environment, no file and no printing."
- Forbidden side effects: no second `SystemTime` call at the record site.
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: `entry` is pure, so the same inputs always produce the same line.
- Privacy: the entry is the operator's own text by design. This is why the journal exists as a separate
  file from the decision ring: "The ring is read by a human through `pns doctor` and therefore admits no
  free text at all; the journal is read by the replayer and is useless without the event's own text"
  (`src/missed_notifications.rs` module header).
- Process ownership and cleanup: not applicable.
- Compatibility contract: `src/missed_notifications.rs:entries` is the read side, kept beside the write
  side "so the pair changes together", parses by key, and skips a line that is not a JSON object.
  `tests/dispatch.rs:a_line_nothing_can_parse_costs_the_entries_around_it_nothing` pins that a torn line
  is not counted and costs the entries around it nothing.

______________________________________________________________________

### 5. The journal is bounded at twenty five and prunes the oldest first

Given a journal already at `missed_notifications::KEPT` (25) entries

When one more miss is appended

Then the file is republished holding the newest 25, with the oldest gone.

`src/missed_notifications.rs:KEPT` argues 25 against the decision ring's 5: "Five is argued from one
intervening Stop hook, which is a scale of seconds; this file has to survive an absence of hours, and
twenty five covers an evening at a few notifiable events an hour."

The append, the read-back, the prune and the publish all happen inside one hold of the ring's own lock
(`src/main.rs:append_ring_line`: "THE WHOLE OPERATION IS ONE CLAIM"), which is what retired an earlier
bug where "a racer that read before a sibling's append could still publish its stale, smaller window
AFTER the sibling published a newer one".

- Success: `tests/dispatch.rs:the_journal_keeps_only_the_most_recent_misses_with_the_oldest_gone`
  (planted at the cap, one more event, oldest dropped and newest last).
  `tests/dispatch.rs:the_shared_append_prunes_each_ring_to_its_own_callers_depth` pins that the journal
  prunes to 25 while the decision ring prunes to 5 in the same run.
- Failure sources: a read-back that fails. `src/main.rs:append_ring_line` then heals by republishing the
  one line it just wrote, which is the known-good part, unless the error is `NotFound`.
- Fail direction: the notification still goes out; the prune is entirely off the delivery path.
- Thresholds: `KEPT` is 25. `RING_READ_MAX` is 256 KiB. `src/missed_notifications.rs:KEPT` does the
  arithmetic and names a hard ceiling: a worst-case entry measures 7,876 bytes and a full journal 196,900
  bytes (75% of the read cap), so "past a depth of 33 a full journal no longer reads back at all, and the
  append answers a file it cannot read by republishing the one line it just wrote: the journal would
  collapse to a single entry exactly when it is fullest, and silently. Raising this past 33 means raising
  that read cap in the same change." A depth of 33 still reads back; 34 does not.
- Required side effects: the prune publishes by writing a pending file `missed-notifications.new.<pid>`
  at 0600 and renaming it over the journal (`src/main.rs:publish_state_line`), so a reader landing
  mid-prune never sees an empty file.
  `tests/dispatch.rs:the_journal_is_created_readable_and_writable_by_its_owner_alone` asserts the mode
  survives the prune.
- Forbidden side effects: the prune never truncates in place, and never widens the mode (the pending
  file's permissions are set on the open handle after the create, "so nothing can be swapped in
  underneath between the two", `src/main.rs:publish_state_line`).
- Timeout and cancellation: bounded by the same ring lock as behavior 2.
- Idempotency and duplicates: the `NotFound` arm is the one that matters here. If a claim renamed the
  journal away between this append and its read-back, the just-written line went with it and is already
  on its way to the operator; republishing would show it twice, so nothing is done
  (`src/main.rs:republish_after`: "NotFound is the exception and the only one").
- Privacy: unchanged from behavior 2.
- Process ownership and cleanup: the pending file carries this process's id so two runs publishing at
  once cannot share one, and it is removed if the rename fails.
- Compatibility contract: `src/main.rs:MISSED_NOTIFICATIONS` states the file is "Bounded state that
  prunes itself, not a log stream and not rotate-logs' business", which is the contract with the
  machine's log rotation.

______________________________________________________________________

### 6. Anything at the journal's path that is not a regular file is refused, never repaired

Given a FIFO, a directory, or a symlink at `missed-notifications`

When an append, a read, or a claim reaches it

Then the operation is refused, the path is left exactly as it was found, and the event proceeds.

Three separate guards implement this. `src/main.rs:append_ring_line` checks `symlink_metadata` before the
open, "so a state directory that does not exist yet fails the lock's own exclusive create" and an
irregular file is "Refused and never repaired: deleting something this tool did not put there, on a path
it only ever appends to, is a bigger action than skipping one record." `src/main.rs:readable_ring`
refuses a non-regular file and a file over `read_max` without reading it, because "A FIFO parks the open
forever, for READING as much as for writing, which wedges the hook that appended or the command a human
is waiting on." `src/main.rs:claim_by_rename` verifies after the rename and renames back: "anything that
is not a regular file goes straight back to the journal's own path, untouched and unread."

- Success:
  `tests/dispatch.rs:a_fifo_at_the_journals_path_is_refused_untouched_and_never_parks_the_event`,
  `tests/dispatch.rs:a_fifo_at_the_journals_path_is_refused_untouched_and_never_parks_the_replay`,
  `tests/dispatch.rs:a_fifo_at_the_journals_path_never_parks_the_doctor_and_is_named_by_its_kind`, and
  `tests/dispatch.rs:a_directory_at_the_journals_path_is_put_back_exactly_where_it_was_found` (which
  plants a marker file inside the directory so the assertion is that this directory came back).
- Failure sources: a rename-back that itself fails. `src/main.rs:claim_by_rename` states the outcome: "A
  RENAME BACK THAT FAILS LEAVES IT AT THE CLAIM PATH, which is a state nothing here can improve on: the
  guarded reader refuses a non-regular file without opening it, so a later adoption leaves it alone as
  well. It is never read and never removed."
- Fail direction: the notification still goes out. All three FIFO tests assert exit 0, the live event
  delivered, and empty stdout and stderr.
- Thresholds: `readable_ring` refuses at `found.len() > read_max`, so a file of exactly `RING_READ_MAX`
  (262,144 bytes) is read and one byte more is refused.
- Required side effects: the path still holds what it held. Every FIFO test asserts
  `symlink_metadata(...).file_type().is_fifo()` afterwards.
- Forbidden side effects: no unlink of a path this tool did not write. No `chmod` of a file found in
  place (`src/main.rs:STATE_FILE_MODE` names this as an accepted limit: a ring an earlier build left
  keeps its umask mode until it is next created).
- Timeout and cancellation: the FIFO tests run under `output_before_the_deadline`, so a build that parks
  on the open fails rather than hanging the suite.
- Idempotency and duplicates: a refusal delivers nothing and destroys nothing, so it can be repeated
  indefinitely.
- Privacy: a refused file is never read, so its bytes never reach a channel.
- Process ownership and cleanup:
  `tests/dispatch.rs:a_directory_at_the_journals_path_is_put_back_exactly_where_it_was_found` asserts the
  whole state directory listing afterwards, so a claim path left standing fails it.
- Compatibility contract: `symlink_metadata` is used everywhere rather than `metadata`, so the link
  itself is judged rather than its target, in the append, the reader and the adoption scan
  (`src/main.rs:stranded_claims` says the same about `DirEntry::metadata`).

______________________________________________________________________

### 7. The doctor counts the journal and never renders an entry

Given a journal on disk

When `pns doctor` runs

Then it prints one line naming how many notifications are waiting, and it leaves the file byte for byte as it found it.

`src/main.rs:missed_line` reads through `readable_ring` and hands the contents to
`src/missed_notifications.rs:waiting_line`, which "COUNTS AND NEVER PARSES, and that is the privacy rule
made structural rather than promised: there is no code path in here that could emit a field, because
nothing in here ever looks inside a line."

The sentences, verbatim:

```text
nothing waiting (either state of the switch):
pns doctor: no missed notification is recorded.

one waiting, card on:
pns doctor: 1 missed notification is waiting to be replayed; the next event that raises a banner or a card while the operator is not away delivers it.

many waiting, card on:
pns doctor: {many} missed notifications are waiting to be replayed; the next event that raises a banner or a card while the operator is not away delivers them.

one waiting, card off:
pns doctor: 1 missed notification is recorded; the catch-up card is switched off (`[recap] replay_card = false`), so nothing delivers it until the card is switched back on.

many waiting, card off:
pns doctor: {many} missed notifications are recorded; the catch-up card is switched off (`[recap] replay_card = false`), so nothing delivers them until the card is switched back on.

present and unreadable:
pns doctor: the missed-notification journal could not be read ({kind}).
```

The zero case is the same sentence with the switch either way, and it is deliberately about what is
RECORDED, because an empty journal is "either nothing was missed or a write did not land", and the line
claims neither (`src/missed_notifications.rs:NONE_WAITING`). The unreadable sentence is
`src/main.rs:MISSED_UNREADABLE` with the error kind in parentheses after it.

The promise the "waiting to be replayed" sentence makes is exact and was narrowed twice.
`src/missed_notifications.rs:waiting_line`: "The sentence used to end 'nothing replays them yet', which
the replay made false the moment it shipped, and then 'the next event the operator is present for', which
promises more than the binary does: presence alone delivers nothing. Three things have to be true at
once, and the sentence says all three."

- Success: `tests/dispatch.rs:the_doctor_counts_the_journal_last_and_never_moves_its_exit_code_for_it`
  (the count is the last line, one decision then the count and nothing else, exit 0);
  `tests/dispatch.rs:the_doctor_leaves_the_journal_exactly_as_it_found_it` (the bytes are identical and
  the whole state directory holds only `missed-notifications`).
- Failure sources: an absent journal, a journal that cannot be read (a directory or a FIFO at the path).
- Fail direction: not on a delivery path. The count sits below the section that already cannot move the
  exit code, so an unreplayed or unreadable journal never fails `pns doctor`
  (`tests/dispatch.rs:a_journal_the_doctor_cannot_read_is_named_by_its_error_kind_and_moves_no_exit_code`,
  `tests/dispatch.rs:a_fifo_at_the_journals_path_never_parks_the_doctor_and_is_named_by_its_kind`).
- Thresholds: the sentence branches at 0, 1 and many. There is no time threshold here.
- Required side effects: none at all. The doctor is read-only over this file.
- Forbidden side effects: the doctor must never append. `src/main.rs:missed_line`: "a doctor that
  journaled would file a miss for the act of going to look for one, and its own test send is the last
  event anything should ever replay." Nothing in this path may render an entry;
  `src/missed_notifications.rs:waiting_line` warns that "Anyone tempted to make this 'more helpful' by
  rendering the newest entry is about to print the operator's own text to a terminal."
- Timeout and cancellation: the read goes through `readable_ring`, which refuses a FIFO by
  `symlink_metadata` before opening it, so the doctor cannot park.
- Idempotency and duplicates: the doctor is a pure read, so running it any number of times changes
  nothing and delivers nothing.
- Privacy: this is the strongest privacy statement in the area. The count is a line count over non-empty
  lines, with no parse anywhere in the path.
- Process ownership and cleanup: not applicable.
- Compatibility contract: absent and unreadable are two different states with two different sentences,
  and `src/main.rs:missed_line` keeps them apart by matching `ErrorKind::NotFound` explicitly. KNOWN GAP:
  the doctor's count reads the journal's own name, so a batch sitting under a claim name or a held name
  is invisible to it (`src/main.rs:claim_journal`: "the doctor's count could not even see it, because
  that count reads the journal's own name").

______________________________________________________________________

### 8. A returning event claims the whole return moment before it counts anything

Given an event whose surface is not Away and whose plan raised a banner or a phone card

When the replay path runs

Then it takes one claim covering both halves of the return (the window's near edge and the journal), and a racer that finds the moment held says nothing at all.

`src/main.rs:Moment` is the model, and it is deliberately one value rather than a claim per file: "ONE
ARBITRATION OVER BOTH HALVES of what a return delivers ... with a claim each the loser of one could still
win the other: MEASURED at roughly one run in three with eight racers, a racer that found the marker held
read no window, fell through to the journal, and put its catch-up card on the phone beside the winner's
recap card."

`Moment::Owned { since, waiting }` carries the near edge the marker held (absent when there was no
marker) and the journal claimed inside the same critical section. `Moment::Busy` means "A run that still
exists holds the moment right now, so this event is inside somebody else's return and has claimed
nothing."

The ordering inside `src/main.rs:claim_moment` is the point of the function:

1. rename `last-present` to `last-present.claim.<pid>[.<epoch>]`;
1. on failure, look for a stranded window claim (behavior 9) and adopt it by a second rename;
1. read the near edge off what was claimed, never off a second read of the marker;
1. claim the journal if asked (behavior 11);
1. restore the edge through `advance_marker` (behavior 10);
1. remove the claim this run took.

"Reading the marker first and renaming it afterwards claims whatever marker is there BY THEN, which is
not the one the window was counted from, because the winner republishes inside that gap. Both racers then
post the same window."

- Success: `tests/dispatch.rs:an_event_inside_another_runs_return_moment_delivers_no_card_of_any_kind`
  (the test's own process id is planted as the holder, so the holder is provably alive: one banner, the
  journal byte identical, no recap in `hermes`, and `last-present` still absent because the standing down
  run must not republish the edge somebody else is holding).
  `tests/dispatch.rs:racing_present_events_recap_one_loud_window_exactly_once_between_them` asserts eight
  live events plus exactly one card of any kind, exactly one recap in `hermes`, and no claim left behind.
- Failure sources: the state directory cannot be read (`stranded_window_claim` returns
  `StrandedWindow::None`); the marker does not exist (a machine that has never published one).
- Fail direction: the notification still goes out. `replay_missed` is fail-quiet in `record_missed`'s
  style, and a `Moment::Busy` returns before any delivery without touching the queue.
- Thresholds: `STALE_WINDOW_CLAIM_SECS` is 300 (behavior 9). No other threshold here.
- Required side effects: the near edge is restored immediately, "before the window is counted and long
  before anything is dispatched, so the marker's absence is bounded by two renames rather than by a
  delivery" (`src/main.rs:claim_moment`).
  `tests/dispatch.rs:the_claim_never_survives_the_run_whether_the_replay_delivered_or_not` kills a run
  while it is inside the replay's own dispatch and asserts both that no claim is left and that
  `last-present` is already back holding this event's own clock read.
- Forbidden side effects: a run that stood down must not publish `last-present`.
  `src/main.rs:mark_present` records why: "MEASURED at one run in sixty with eight racers: a run that
  found the moment held republished the marker here anyway, out from under the holder, and a third run
  then renamed that fresh marker and became a SECOND owner alongside the first. The two then raced on the
  journal, and the pair of them put a recap card and a catch-up card on the phone at one moment." This is
  why `mark_present` also goes through `claim_moment`, with `take_journal` false: "NOTHING IS TAKEN AND
  NOTHING IS DELIVERED: the claim is asked for the edge alone."
- Timeout and cancellation: the claim itself has no deadline; it is two renames and a small read. A
  process killed mid-claim leaves one file the next return adopts by name.
- Idempotency and duplicates: this behavior is the whole idempotence rule for a return. Exactly one card
  of any kind per return moment.
- Privacy: the journal read happens inside the claim, in memory, and is never written anywhere else.
- Process ownership and cleanup: "NOTHING IS LEFT BEHIND on any path this run completes, and a run killed
  mid-claim leaves ONE file that the next return adopts by name. The adoption is also the recovery: the
  edge that run was holding comes back with it rather than being lost."
- Compatibility contract: the claim name shape is internal, and is parsed only by
  `src/main.rs:stranded_window_claim` and `src/main.rs:window_claim_is_free`.

______________________________________________________________________

### 9. A stranded window claim is live, abandoned, or absent

Given the rename of `last-present` failed

When `src/main.rs:stranded_window_claim` scans the state directory

Then it answers `Live` for the first claim whose owner is neither this process, nor gone, nor older than the staleness bound; `Abandoned(path)` for the last free one it found; and `None` when there is no claim at all.

The scan matches `last-present.claim.` and nothing looser, "which is `stranded_claims`' rule: the journal
and the turn marker claim themselves in this directory too, and a wider match would hand one of their
values back as a window's near edge" (`src/main.rs:StrandedWindow`).

`src/main.rs:window_claim_is_free` gives three ways a claim is free, and names them: "It is THIS RUN'S,
so nothing else can be inside it; or its owner has EXITED, so nothing is; or it is far OLDER than any run
could still be holding it." The age test is the one a process id cannot answer: "a claim minutes old is
one whose owner died mid-claim and whose id the machine has since handed to something long-lived. Without
it that claim reads as live for as long as the new process runs, and every return moment on the machine
stands down behind it: no card, no recap and no edge, until that process happens to exit."

`src/main.rs:window_claim_suffix` records the epoch in the claim's own name rather than relying on the
file: "a rename carries the marker's mtime, which is the time of the last PRESENT event and can be hours
before the claim was made."

- Success:
  `tests/dispatch.rs:a_window_claim_whose_owner_is_gone_is_adopted_rather_than_lost_or_left_behind`
  plants a claim owned by a reaped process id (obtained by running `/usr/bin/true` to completion, "STATED
  BY THE MACHINE rather than guessed at, because a made-up number can be live") with NO marker beside it,
  and asserts the recap counts 13 events off the adopted edge and the claim is swept.
  `tests/dispatch.rs:an_event_inside_another_runs_return_moment_delivers_no_card_of_any_kind` is the
  `Live` half.
- Failure sources: `read_dir` fails (answers `None`, so the run treats it as a machine that never
  published a marker and still owes its catch-up card); the second rename of an abandoned claim fails
  (`taken` is `None`, so `since` is `None`).
- Fail direction: the notification still goes out. Nothing in this scan can suppress the live event.
- Thresholds: `STALE_WINDOW_CLAIM_SECS` is 300 seconds, tested as
  `now.saturating_sub(taken) > STALE_WINDOW_CLAIM_SECS`. A claim taken exactly 300 seconds ago is NOT
  free and the arriving run stands down; at 301 seconds it is free and may be adopted. The bound is
  "deliberately five minutes, four orders of magnitude past what holding one costs, so a real holder can
  never be stolen from and a stranded one can never wedge for long." A claim carrying no epoch, or a run
  with no readable clock, falls back on the process id alone and the age test never fires
  (`src/main.rs:window_claim_is_free`, the `_ => false` arm).
- Required side effects: an abandoned claim is adopted by a second rename onto this run's own claim name,
  "which is `take_claim`'s idiom: two runs that both reach one stranded claim still cannot both take it,
  because only one rename can win" (`src/main.rs:claim_moment`). The adopted file is removed by the
  adopting run at the end of its claim.
- Forbidden side effects: the scan must not match a wider prefix, and must not take a claim from a live
  owner.
- Timeout and cancellation: not applicable; one directory scan.
- Idempotency and duplicates: at most one window claim can exist at a time, "because a claim is only ever
  made by renaming the ONE marker or by renaming an existing claim, and a run that finds one live makes
  none of its own. The loop still answers `Live` for the first live one it meets rather than assuming
  that, because the directory is a plain directory another hand can reach"
  (`src/main.rs:StrandedWindow`).
- Privacy: a window claim holds one epoch and no operator text.
- Process ownership and cleanup: the pid segment is judged by `src/main.rs:owner_is_gone`, shared with
  the journal's holds so the two cannot drift.
- Compatibility contract: `window_claim_suffix` emits `claim.<pid>.<epoch>` with a clock and
  `claim.<pid>` without one, and `window_claim_is_free` reads both.
- NOT ESTABLISHED: no test in this tree exercises `STALE_WINDOW_CLAIM_SECS`. The two window-claim tests
  plant `last-present.claim.<owner>` with no epoch segment (`tests/dispatch.rs:plant_window_claim`), so
  both take the pid-only path. What happens when the age test frees a claim whose owner is genuinely
  still inside its critical section is not stated in the code and not covered by a test.

______________________________________________________________________

### 10. The window's near edge only ever moves forward

Given a near edge already on disk, or one carried in from a claim

When a run publishes `last-present`

Then it publishes only when the value it holds is newer than the one already there.

`src/main.rs:advance_marker` states the measurement: "a slow event that read epoch 100 and a quick one
that read 101 both publish at the end of their own run, so the slow one used to land last and put the
edge back to 100. Everything the quick event covered then reads as absence activity on the next return,
and a long enough tail of it crosses the threshold and posts a recap of a window that never happened."

`advance_marker` is "CALLED ONLY FROM INSIDE A CLAIM, which is what makes the read and the publish safe
as a pair: the caller holds the marker, so nothing else is writing this path between them."
`src/main.rs:claim_moment` publishes `since.max(now)`, so "a claim taken with no readable clock puts back
exactly what it took."

`src/main.rs:read_epoch` refuses anything it will not vouch for: "AN UNPARSEABLE MARKER IS NO EDGE AT
ALL, never an edge at epoch zero. A marker some other hand rewrote is not a near edge this can trust, and
reading one as zero would recap the whole ring."

- Success: `tests/dispatch.rs:the_windows_near_edge_never_moves_backward_however_late_an_event_publishes`
  plants a marker an hour in the future and asserts it survives the run untouched.
  `tests/dispatch.rs:a_marker_no_reader_can_parse_opens_no_window_rather_than_one_from_epoch_zero`
  asserts an unparseable marker opens no window and is healed to this event's own clock read.
  `tests/dispatch.rs:a_present_event_moves_the_last_present_marker_and_an_away_event_does_not` is the
  base case.
- Failure sources: an unwritable state directory; an unparseable marker.
- Fail direction: the notification still goes out. "A marker that did not land costs one window's near
  edge, which the next present event moves anyway" (`src/main.rs:advance_marker`).
- Thresholds: the comparison is `held >= now`, so a marker holding exactly this event's epoch is left
  alone and a marker one second older is replaced.
- Required side effects: the publish is a pending-file-plus-rename at 0600
  (`src/main.rs:publish_state_line`).
- Forbidden side effects: `src/main.rs:mark_present` must not publish outside a claim, for the reason in
  behavior 8.
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: `mark_present` runs after the card site, deliberately: "The window a recap
  covers ends where this event is, so moving the edge before `replay_missed` counted the window would
  leave every count at one and no recap could ever fire."
  `tests/dispatch.rs:the_marker_advances_so_a_second_present_event_recaps_nothing` is the idempotence
  assertion (two back-to-back present events over one window, exactly one card and exactly one recap).
- Privacy: `last-present` holds one epoch and no operator text.
- Process ownership and cleanup: the pending file is named for this process and removed if the rename
  fails.
- Compatibility contract: the file is one line holding a decimal epoch (`src/main.rs:LAST_PRESENT`).
  "Absent means no window at all, so a fresh install cannot recap the whole ring."

______________________________________________________________________

### 11. The journal is claimed by rename and consumed only after a successful read

Given a journal at `missed-notifications` and a run that owns the return moment

When `src/main.rs:claim_journal` runs

Then any stranded claim is adopted first (behavior 13), then the journal itself is renamed to `missed-notifications.claim.<pid>`, verified, held (behavior 12), read, and only then given up.

`src/main.rs:claim_journal` names the property the ordering exists for: "NOTHING UNDELIVERED IS EVER
DESTROYED ... What this run cannot read, it leaves; what it cannot give up, it leaves; what it leaves
sits under its claim name or a held name, and one of the returns that follow goes looking for both."

`src/main.rs:Claimed` is four outcomes rather than one empty vector, "because they are four different
things to have happened and only one of them may destroy anything. This used to collapse into
`Vec::new()`, and that is exactly how a journal whose read failed came to be deleted with nothing
delivered":

- `Nothing`: nothing was there, or another run took it first.
- `Refused`: the path holds something this tool never wrote. Put back where it was found, and not read.
- `Taken(entries)`: this run owns them, and the claim they came from is gone.
- `LeftForAdoption`: the claim could not be read or could not be given up. It is still on disk, whole.

`Claimed::entries` returns entries only for `Taken`: "an unread claim is still on disk, and delivering
from it as well would show the operator the same batch twice."

`src/main.rs:claim_by_rename` refuses to rename over a claim already at its own name: "a rename
overwrites: the journal would land on top of a batch nobody has seen. Both are left where they are, and
the next return tries both again." That guard "IS NOT PINNED BY A TEST, and cannot be: no test can plant
a claim named for a process id the engine has not been given yet."

- Success: `tests/dispatch.rs:a_present_event_delivers_one_extra_notification_carrying_the_whole_journal`
  asserts the journal is gone after a delivering run.
  `tests/dispatch.rs:the_claim_never_survives_the_run_whether_the_replay_delivered_or_not` asserts no
  claim survives, delivered or killed mid-dispatch.
- Failure sources: the rename fails (`Nothing`); the claimed path turns out not to be a regular file
  (`Refused`, renamed back); a claim already exists at this run's own name (`LeftForAdoption`); the read
  fails (`LeftForAdoption`, behavior 12).
- Fail direction: the notification still goes out.
  `tests/dispatch.rs:a_fifo_at_the_journals_path_is_refused_untouched_and_never_parks_the_replay` asserts
  exit 0, the live event alone, empty stdout and stderr, and a FIFO still at the path.
- Thresholds: `RING_READ_MAX` (256 KiB) is the read ceiling for the claimed batch
  (`src/main.rs:take_claim` passes it).
- Required side effects: the claim is removed before any delivery, never after, "so a channel that hangs
  to its deadline and takes the process with it cannot leave an orphan in the state directory for the
  next run to trip over"
  (`tests/dispatch.rs:the_claim_never_survives_the_run_whether_the_replay_delivered_or_not`).
- Forbidden side effects: nothing undelivered is destroyed.
  `tests/dispatch.rs:a_journal_this_run_could_not_read_is_left_on_disk_rather_than_consumed` plants a
  journal with an undecodable byte and asserts exactly one leftover, byte for byte what was waiting.
- Timeout and cancellation: `src/main.rs:claim_journal` states that "ALL of it before any delivery" is
  what makes a hang safe: "The entries are in memory from the moment this returns, so a channel that
  hangs to its deadline and takes the process with it leaves no claim behind."
- Idempotency and duplicates: exactly one run may deliver a given batch. The one named race:
  `src/main.rs:claim_journal` states "an append that opened the journal path before the rename writes
  into the claimed inode, and is replayed or lost depending on which side of the read it lands. That is
  ONE entry at a rare boundary, the same bound `append_ring_line` already names and accepts."
  `tests/dispatch.rs:racing_present_events_deliver_exactly_one_replay_between_them` asserts eight live
  events plus exactly one replay per channel.
- Privacy: the batch is read into memory and handed only to the same legs the live event reached
  (behavior 15).
- Process ownership and cleanup: the claim name carries `std::process::id()`, which is what makes
  `claim_by_rename`'s refusal a non-race: "only the process holding this id writes this name, and it is
  this one."
- Compatibility contract: `missed-notifications.claim.<pid>` and `missed-notifications.held.<pid>.<seq>`
  are the two names a batch can wait under, and `tests/dispatch.rs:claim_files` documents both.

______________________________________________________________________

### 12. A claim is held by a second rename before a byte of it is read

Given a claim this run has taken

When `src/main.rs:take_claim` runs

Then the claim is renamed to `missed-notifications.held.<pid>.<seq>` first, read second, and removed third, in that order.

The hold name deliberately sits outside the prefix the adoption scan matches, "so nothing can take this
batch a second time while it is being read. It comes back into that scan only once the process named in
it is gone" (`src/main.rs:take_claim`).

The read comes before the remove, "which is the older half of this and unchanged. Removing first, or
removing whatever the read answered, throws away a batch nobody has seen the moment the read fails:
MEASURED as a journal with one undecodable byte in it coming back empty, with the file already gone."

The sequence number in the hold name is per claim, not per process, and that is a fix rather than
decoration: "A single per-process name coupled every stranded claim in a run to the first one, and an
UNREADABLE first claim then occupied the name, was migrated to a fresh name by every later run's
adoption, always sorted oldest, and so STARVED every good batch behind it forever. The sequence dissolves
the coupling; the adoption parses the pid segment alone."

- Success: `tests/dispatch.rs:a_journal_this_run_could_not_read_is_left_on_disk_rather_than_consumed`
  asserts the leftover is under the `missed-notifications.held.` name, and explains why that name is the
  load-bearing evidence: "A build that reads the claim where it lies and owns it by unlinking leaves the
  claim name here instead, and owns nothing."
  `tests/dispatch.rs:an_unreadable_old_claim_cannot_starve_the_good_batch_behind_it` plants an unreadable
  claim and a good one in the same run and asserts the good one delivers while exactly one held file
  parks.
- Failure sources: a hold already exists at this name (`LeftForAdoption`); the rename fails (`Nothing`);
  the read fails (`LeftForAdoption`); the remove fails (`LeftForAdoption`, with the entries deliberately
  not returned).
- Fail direction: the notification still goes out; a failed hold only costs the replay.
- Thresholds: `RING_READ_MAX` (256 KiB).
- Required side effects: on the success path the held file is gone and the entries are in memory.
- Forbidden side effects: no read before the rename lands. No remove of a file whose read failed.
- Timeout and cancellation: not applicable; two renames and one bounded read.
- Idempotency and duplicates: the hold is what makes a second delivery impossible while a live owner is
  reading. `tests/dispatch.rs:a_held_batch_whose_owner_is_still_running_is_left_exactly_where_it_is`
  plants two live holds (this test's own process id, which answers success, and pid 1, which answers
  `EPERM`) and asserts both are untouched and no replay fired.
- Privacy: the batch never leaves memory except through the delivery legs.
- Process ownership and cleanup: `HELD_SEQ` is a process-local `AtomicU32` starting at 0, so one run's
  holds are `held.<pid>.0`, `held.<pid>.1` and so on.
- Compatibility contract: `src/main.rs:owner_is_gone` parses the segment before the first dot, so "a bare
  held.<pid> from an older build" is still recognized. That bare spelling is a compatibility artifact,
  not something this build writes.

______________________________________________________________________

### 13. A stranded claim or an abandoned hold is adopted by a later return

Given a claim or a hold left in the state directory by a run that did not finish

When the next return moment claims the journal

Then `src/main.rs:stranded_claims` collects them oldest first, and each is taken through `take_claim` before the journal's own name is claimed.

Two different admission rules apply, and the difference matters:

- A `missed-notifications.claim.<pid>` file is admitted regardless of its owner's liveness
  (`src/main.rs:stranded_claims` matches the prefix alone). It is safe because the claim name is held for
  only one rename before `take_claim` moves it to a held name, and a rename arbitrates.
- A `missed-notifications.held.<pid>[.<seq>]` file is admitted only once `src/main.rs:owner_is_gone` says
  the owner has exited (`src/main.rs:abandoned_hold`). "Nothing else may touch one while its owner lives,
  which is the whole reason the name sits outside the claim prefix: an owner that is still reading cannot
  have its batch taken a second time."

`src/main.rs:owner_is_gone` is one answer for every claim in the directory, shared between the journal's
holds and the window's claims "because two copies of this test would drift the day one of them learns
something." Only `ESRCH` counts as gone: `kill(pid, 0)` answering `EPERM` "is still a process that
exists". A reused process id therefore reads as alive, and what that costs is "a batch that waits for the
first return after the process wearing its number exits ... a replay deferred, never a replay destroyed
and never one delivered twice." A non-positive owner is refused outright, because "kill() reads
non-positive values as the GROUP and BROADCAST forms".

Ordering is by last write time, which a rename does not touch, "so a claim still carries the moment its
last entry was appended. A time that cannot be read sorts oldest, which costs an ordering and never a
delivery." Adopting oldest first is correct because "a stranded claim WAS the journal on an earlier
return, so it is older than anything in the file now" (`src/main.rs:claim_journal`).

- Success: `tests/dispatch.rs:a_claim_an_earlier_run_never_finished_is_adopted_by_the_next_return` (a
  planted `claim.999999`, both entries delivered, nothing left behind);
  `tests/dispatch.rs:a_held_batch_whose_owner_is_gone_is_adopted_exactly_once` (a planted `held.999999`,
  both entries delivered, nothing left behind);
  `tests/dispatch.rs:a_hand_planted_negative_hold_name_is_never_read_as_a_pid` (a planted `held.-99999`
  is left exactly where it was found and delivered by nobody).
- Failure sources: `read_dir` fails (returns an empty list, so nothing is adopted this round); `metadata`
  fails (that entry sorts oldest).
- Fail direction: the notification still goes out; a failed adoption defers a batch to the next return.
- Thresholds: no age threshold applies to a journal claim or a hold. Liveness alone gates a hold, and
  nothing gates a claim. This is the deliberate difference from the window claim in behavior 9, which
  does carry `STALE_WINDOW_CLAIM_SECS`.
- Required side effects: an adopted claim is renamed to this run's own held name before it is read, which
  is what makes two simultaneous adopters safe.
- Forbidden side effects: a hold whose owner is alive must never be touched. A negative or unparseable
  owner segment must never reach `kill`.
- Timeout and cancellation: not applicable; one directory scan plus one rename and one read per
  candidate.
- Idempotency and duplicates: exactly once.
  `tests/dispatch.rs:racing_present_events_adopt_one_stranded_claim_exactly_once` is the soak (24 racers,
  ignored by default, "a probabilistic hunt, roughly one catch in 200 rounds"), and the two held-file
  tests above are the deterministic statements of the same invariant.
- Privacy: an adopted batch is the operator's own text and travels the same path as a fresh one.
- Process ownership and cleanup: adoption is also the sweep. Every adoption test asserts the final state
  directory listing is exactly `["activity", "decisions", "last-present", "lights-news"]`.
- Compatibility contract: the pid-segment parse tolerates the older bare `held.<pid>` spelling.

______________________________________________________________________

### 14. The window is counted on a half-open interval off the claimed edge

Given a claimed near edge `since` and this event's clock `until`

When `src/main.rs:activity_in` counts the window

Then it returns every activity entry whose `at` satisfies `at > since && at <= until`, oldest first.

"THE NEAR EDGE IS EXCLUSIVE and the far edge is not, which is the difference between 'since you were last
here' and 'including the moment you were'. MEASURED: with it inclusive, the event that MOVED the marker
is counted inside the next window, and every event sharing that same second with it is too. Eight events
in one second then read as a loud window opening at the instant it closed, so a burst at the desk earned
a recap of an absence that never happened."

The window comes off what was claimed, never off a second read: "`since` is the value that was renamed
out from under every other racer, so a racer holding a republished marker computes the empty window it
deserves rather than the one somebody else already posted" (`src/main.rs:replay_missed`). A marker ahead
of `now` is no window either, because "A clock that moved backwards is not a bracket."

- Success:
  `tests/dispatch.rs:events_stamped_at_the_markers_own_second_belong_to_it_and_not_to_the_window_after`
  plants twelve activity entries at exactly the marker's own epoch and asserts the catch-up card, not a
  recap card.
  `tests/dispatch.rs:an_activity_window_with_no_marker_to_open_it_recaps_nothing_and_still_catches_up`
  asserts a full ring with no marker recaps nothing and still delivers the queued card, which is what
  tells "no window" apart from "somebody else holds the moment".
- Failure sources: the activity ring cannot be read. "A RING THAT CANNOT BE READ IS AN EMPTY WINDOW,
  which reads as no recap rather than as a recap of nothing: the count would be zero, and zero is under
  every threshold."
- Fail direction: the notification still goes out; an unreadable ring costs the recap only.
- Thresholds: the interval is `(since, until]`. An entry at exactly `since` is excluded; at `since + 1`
  it is included; at exactly `until` it is included; at `until + 1` it is excluded. `ACTIVITY_READ_MAX`
  is 1 MiB, its own number because the ring's depth (`ACTIVITY_KEPT` = 150) is its own. The volume
  threshold `[recap] min_events` defaults to 8 (`src/config.rs:DEFAULT_MIN_EVENTS`), tested as
  `counted.len() >= recap.min_events`: 7 counted events deliver the catch-up card unchanged
  (`tests/dispatch.rs:a_window_under_the_threshold_delivers_the_catch_up_card_unchanged`) and 8 or more
  deliver the recap card
  (`tests/dispatch.rs:a_window_over_the_threshold_delivers_one_recap_card_with_what_needs_you_first`,
  which counts 13).
- Required side effects: none. `activity_in` is a read.
- Forbidden side effects: an entry with no clock is in no window: "Its writer had no readable clock, so
  nothing can place it, and counting it would put an event of unknown age inside a bracket that is
  entirely about age."
- Timeout and cancellation: bounded by `readable_ring`'s size ceiling.
- Idempotency and duplicates: the activity ring is "NEVER CLAIMED AND NEVER CONSUMED, unlike the journal.
  It is a rolling window pruned by depth alone, which is what lets the detached recap child re-read it
  safely and what makes a recap idempotent by WINDOW rather than by deletion"
  (`src/main.rs:record_activity`).
- Privacy: the activity ring holds the operator's own text for every event, at 0600, and "`pns doctor`
  deliberately gains no activity line".
- Process ownership and cleanup: none. The ring prunes by depth in the append.
- Compatibility contract: activity entries are written in the journal's own shape by the same
  `missed_notifications::entry`, at a shorter field cap.

______________________________________________________________________

### 15. A replay delivers at most one card, off the batch it claimed

Given a return moment this run owns

When the replay composes its delivery

Then it builds exactly one synthetic event and dispatches it on this decision's own legs, verbatim.

Which card it is depends on the window (`src/main.rs:replay_missed`):

- Over the threshold with a durable route and the digest on: the recap card from
  `src/missed_notifications.rs:recap_card`, which puts what still needs the operator first, then the true
  counts, then the pointer. The counts are lengths and never claims: "`counted` is the window's own
  length and `missed` is the claimed journal's, so a card that ran out of room still names totals it can
  back." The pointer `. recap in #pns` is added only when a recap child really started.
- Under the threshold with entries waiting: the catch-up card from `src/missed_notifications.rs:summary`,
  which is the true count then as many entries as fit, newest first, because `render::preview` cuts from
  the start.
- Under the threshold with nothing waiting: nothing at all.

The synthetic event is `agent: "pns"`, `state: "missed"`, empty project, branch, channel and pane. The
title renders `pns · missed`, "which is visibly not a live agent card: a replayed card that looked live
would be lying about time."
`tests/dispatch.rs:a_present_event_delivers_one_extra_notification_carrying_the_whole_journal` asserts
every one of those fields.

`src/missed_notifications.rs:NEEDS_YOU` is the one list behind both the phone card's urgent line and the
recap's own section: `["asked", "blocked", "denied", "failed", "plan-ready"]`.

- Success: `tests/dispatch.rs:a_present_event_delivers_one_extra_notification_carrying_the_whole_journal`
  (two banners, newest entry before oldest in the body, the same body on the durable leg);
  `tests/dispatch.rs:the_recap_card_is_exactly_what_the_entries_compose_and_nothing_a_model_said` asserts
  the exact composed body `claude · blocked · p4. 13 events, 2 missed. recap in #pns`.
- Failure sources: a channel that fails or hangs.
- Fail direction: the live notification has already gone out by this point; the replay sits after the
  decision record and before the pulse.
- Thresholds: the body stops at `render::PREVIEW_MAX_CHARS` (260). `summary` never stops before the first
  (newest) entry: "MEASURED: a single missed notification with a 209-character detail took the body one
  character past the cap, so the loop stopped before appending anything and the card read '1 missed
  notification' with no content at all." `recap_card` reserves the counts and the pointer and CUTS the
  newest urgent item to the room left rather than dropping it, because "a card without the one thing
  waiting on the operator is the notification it exists to deliver"; the measurement behind that is a
  253-character title that pushed a card to 289 characters and lost the counts and the pointer to
  `render::preview`.
- Required side effects: the claimed batch is gone before the dispatch starts (behavior 11). The Discord
  half is spawned first, in its own process and its own process group, so the card can say truthfully
  whether there is a recap to point at (`src/main.rs:spawn_recap`,
  `tests/dispatch.rs:the_recap_child_runs_in_a_process_group_of_its_own`).
- Forbidden side effects: nothing is printed. "The event path prints only what a reporting leg said, and
  this rides an event whose stdout a hook reads" (`src/main.rs:replay_missed`, asserted as empty stdout
  AND empty stderr in the delivery test).
- Timeout and cancellation: the legs carry the decision's own deadlines. The detached recap child is
  given `PNS_REMOTE_TIMEOUT=30` (`src/main.rs:RECAP_DEADLINE_SECS`) when the environment asked for no
  deadline at all, because "AN UNBOUNDED DEADLINE IS A TERMINAL'S CHOICE, NEVER A BACKGROUND CHILD'S ...
  a wedged gateway would keep this process alive for good, and every later window would add another." The
  default when the variable is unset is 5 seconds (`src/channels/hermes.rs:remote_deadline`).
- Idempotency and duplicates: at most one card per return moment, of any kind, and it is a single
  dispatch of a single event.
  `tests/dispatch.rs:racing_present_events_recap_one_loud_window_exactly_once_between_them` asserts
  exactly one card and exactly one recap across eight racers. ACCEPTED DUPLICATE, stated in
  `src/main.rs:replay_missed`: "the durable leg is among them, so the summary is posted to a log that
  already holds every entry in it. That is a duplicate in content and a new fact in kind." ACCEPTED LOSS,
  also stated: "A LOSS ON A FAILED DELIVERY IS THE DESIGN, not an oversight ... every journaled event
  already reached the durable log in full, so nothing is lost that a human cannot recover; re-journaling
  against a wedged channel is an unbounded retry that grows the file every event."
- Privacy: the batch reaches "the same channels the live event would have reached" and nowhere else
  (`src/missed_notifications.rs` module header).
- Process ownership and cleanup: the claim is removed before dispatch, so a channel that hangs to its
  deadline and takes the process with it leaves no orphan.
- Compatibility contract: `agent: pns` and `state: missed` are what a channel keys the card off, and
  `pns · missed` is the rendered title. These are operator-visible and are the closest thing in this area
  to a public external contract.

______________________________________________________________________

### 16. A replay is a dispatch, never a second event

Given a composed replay card

When it is delivered

Then it goes straight to `dispatch_legs` and never back through `run_event`.

`src/main.rs:replay_missed` names the loop this closes: "A synthetic event fed back in would take a
SECOND decision (the second reading of one moment `GateInputs` exists to forbid), write a second ring
line for something that is not an event, fire a second pulse, and RE-JOURNAL: under a mute the replay
would journal itself and the next one would replay the replay, forever, growing by one entry each time."

- Success: `tests/dispatch.rs:a_replay_is_never_a_second_event_in_the_ring_or_the_journal` asserts the
  replay really was delivered (two banners), that the decision ring holds exactly one line naming the
  live event `claude/done` and not `pns/missed`, and that no journal exists afterwards.
- Failure sources: not applicable; this is a call-site choice, not an operation.
- Fail direction: not applicable.
- Thresholds: not applicable.
- Required side effects: exactly one decision-ring line per real event.
- Forbidden side effects: no second decision, no second ring line, no second pulse, no re-journal.
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: together with behavior 3, this is what makes the replay terminate.
- Privacy: not applicable beyond behavior 15.
- Process ownership and cleanup: not applicable.
- Compatibility contract: `src/main.rs:replay_missed` states that the legs are the live decision's own,
  verbatim, because "Deciding again would be a second copy of routing's policy, which `routing` itself
  warns is how the two come to drift."

______________________________________________________________________

### 17. Nowhere the operator would see it is not a replay

Given a return event whose legs are all non-decorative (a durable log only)

When the replay path runs

Then it returns before claiming anything, and the journal is left byte identical.

`src/main.rs:replay_missed` calls this "a stronger test than 'nowhere at all'. MEASURED: an event
narrowed with `--remote-only`, and every event on a machine whose config enables only a durable channel,
claimed the queue, posted it into a log that already holds all of it in full, and deleted it, with
nothing the operator would ever see." The empty plan (both narrowing flags typed at once) is refused by
the same line.

Which legs decorate is routing's answer, carried on the leg, "Asking it here by name, or by re-reading
the declarations, would be the second copy of a policy that then drifts."

- Success:
  `tests/dispatch.rs:a_present_event_narrowed_to_the_log_leaves_the_queue_for_a_surface_that_shows_it`
  (with `--remote-only`);
  `tests/dispatch.rs:a_machine_with_only_a_durable_channel_never_consumes_the_queue_it_cannot_show` (with
  no flag typed, config enables `hermes` alone);
  `tests/dispatch.rs:an_event_narrowed_to_no_channel_at_all_leaves_the_journal_where_it_found_it` (both
  flags, asserts `post SKIPPED` really was reached).
  `tests/dispatch.rs:an_away_event_delivers_no_replay_and_leaves_the_journal_byte_identical` is the Away
  half, from `src/missed_notifications.rs:should_replay`: "AWAY IS WHERE MISSES ARE MADE AND NEVER WHERE
  THEY ARE DELIVERED."
- Failure sources: not applicable; this is a guard.
- Fail direction: the live notification is unaffected.
- Thresholds: not applicable.
- Required side effects: none.
- Forbidden side effects: nothing may be claimed, so nothing may be consumed.
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: the queue survives untouched for a later return that has somewhere to show
  it.
- Privacy: not applicable.
- Process ownership and cleanup: no claim is created, so none can be stranded.
- Compatibility contract: not applicable.

______________________________________________________________________

### 18. The catch-up card switch gates the card and never the journal

Given `[recap] replay_card = false`

When a return event runs, and when a miss happens

Then no catch-up card is delivered, the queue is left whole, and misses are still journalled.

`src/main.rs:replay_missed` passes the switch INTO `claim_moment` rather than returning in front of it:
"Claiming the journal renames it out of the way, so a return after that would consume the queue and
deliver nothing, which is the one outcome the four-way `Claimed` enum exists to prevent; handing the
switch in means the journal is never claimed at all when the card is off."

`src/main.rs:record_missed` never learns the switch exists, "so the journal still records every miss and
the doctor still counts them: turning the card back on has something to deliver."

`digest` is its own switch over the Discord half, "so card-only and recap-only are both valid and neither
implies the other". `src/config.rs:Recap` defaults: `replay_card = true`, `digest = true`,
`min_events = 8`.

- Success:
  `tests/dispatch.rs:a_switched_off_replay_card_delivers_no_catch_up_and_leaves_the_journal_whole` (one
  banner, the journal read back as an `Option` so a consumed queue reports as a missing file rather than
  as an operating system error, and empty stderr because "A SETTING IS NOT A COMPLAINT");
  `tests/dispatch.rs:a_switched_off_replay_card_still_journals_the_misses_it_makes` (a delivered event
  journals nothing, a muted one journals exactly its own miss, and the test names its own teeth: "the
  mutation that moves the gate INTO `record_missed`");
  `tests/dispatch.rs:a_switched_off_digest_posts_no_recap_and_leaves_the_catch_up_card_alone`.
- Failure sources: not applicable; a config read.
- Fail direction: the live notification is unaffected.
- Thresholds: not applicable to this switch. `min_events` is covered in behavior 14.
- Required side effects: with the card off and the digest on, the Discord half still fires; the
  `if !recap.replay_card { return; }` sits BELOW the spawn, because "an operator who wants the recap in
  Discord and no card on the phone has asked for exactly that".
- Forbidden side effects: the journal must not be claimed when the card is off, and nothing may be
  printed about the setting.
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: with the card off, a return leaves the queue exactly where it was, so
  turning the card back on delivers it.
- Privacy: unchanged.
- Process ownership and cleanup: no claim is created when the card is off.
- Compatibility contract: the doctor's sentence changes with the switch, because with the card off "a
  doctor that still named 'the next event' would be telling the operator a lie their own setting makes
  permanent" (`src/main.rs:missed_line`). See behavior 7 for both sentence sets.

______________________________________________________________________

### 19. A journalled event leases the lights tick for twelve hours

Given an event `was_missed` returns true for

When the lights tick is registered

Then the lease is `JOURNALLED_LEASE_SECS` (twelve hours) rather than `ORDINARY_LEASE_SECS` (five minutes).

`src/main.rs:JOURNALLED_LEASE_SECS`: "a journalled one ... is an operator who is away or muted. The glow
has to survive the whole absence, and the absence is precisely when no further event arrives to refresh
this."

- Success: `tests/dispatch.rs:an_event_registers_the_tick_and_a_journalled_one_leases_it_for_longer`
  asserts the journal really has one entry and then reads back exactly 300 and exactly 43,200 seconds,
  "EXACT, AND NOT MERELY DIFFERENT".
- Failure sources: no readable clock (no registration at all, "never a job due at epoch zero",
  `src/main.rs:register_lights_tick`); no `[lights]` table.
- Fail direction: the notification still goes out.
  `tests/dispatch.rs:a_registration_that_cannot_be_written_costs_the_event_nothing` is the fail-open
  assertion: "a lamp that did not re-arm must never cost a card, a line of stdout or an exit code."
- Thresholds: 300 seconds ordinary, 43,200 seconds journalled. There is no intermediate value.
- Required side effects: the tick's due second is kept when one is already pending, so an event storm
  cannot push the tick away from itself.
- Forbidden side effects: this reads `was_missed` and must not re-derive the predicate.
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: re-registering replaces the job by name.
- Privacy: the lease carries no event text.
- Process ownership and cleanup: the daemon owns the job.
- Compatibility contract: not applicable.

______________________________________________________________________

## State files

Every file below lives under the state directory, which is `$PNS_STATE_DIR` when set and
`$HOME/.local/state/pns` otherwise (`src/main.rs:state_dir`). `STATE_FILE_MODE` is `0o600` and is
described as "ONE RULE FOR THE DIRECTORY'S CONTENTS rather than a knob for one caller: none of them has a
reason to be world-readable, and the journal holds the operator's own text."

| Path                                                                                      | Mode                                                                                                                                                                                              | Writer                                                                      | Reader                                                                                              | Ownership arbitration                                                                                                     | Stale reclaim                                                                                                                                                                                      | Class                                                                                                                                                                                                                                |
| ----------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `missed-notifications`                                                                    | 0600 at create and at every prune-publish (`src/main.rs:append_ring_line`, `src/main.rs:publish_state_line`; `tests/dispatch.rs:the_journal_is_created_readable_and_writable_by_its_owner_alone`) | `src/main.rs:record_missed` only                                            | `src/main.rs:claim_journal` (parses, delivers) and `src/main.rs:missed_line` (counts, never parses) | `.lock` sibling for append (exclusive create); `rename` to `.claim.<pid>` for consumption (`src/main.rs:claim_by_rename`) | Not applicable: it is never stale, it is claimed away or left in place                                                                                                                             | Internal persistence detail. `src/main.rs:MISSED_NOTIFICATIONS` calls it "Bounded state that prunes itself, not a log stream and not rotate-logs' business"; the module never learns where it lives, and no command prints an entry. |
| `missed-notifications.lock`                                                               | 0600 (`src/main.rs:publish_lock`)                                                                                                                                                                 | `src/main.rs:claim_ring_lock`                                               | its own holder                                                                                      | Exclusive `create_new`; a dead lock is taken by rename, never by remove (`src/main.rs:claim_lock`)                        | `RING_LOCK_STALE_SECS` (5 seconds), then rename-and-republish                                                                                                                                      | Temporary process coordination. It exists only for the width of one append, and `src/main.rs:HeldLock` removes it on drop.                                                                                                           |
| `missed-notifications.new.<pid>`                                                          | 0600, set on the open handle after create (`src/main.rs:publish_state_line`)                                                                                                                      | the pruning run                                                             | nobody                                                                                              | Named for the writing process, so two runs cannot share one                                                               | Removed by its own writer when the rename fails                                                                                                                                                    | Temporary process coordination. It exists only between the write and the rename that publishes it.                                                                                                                                   |
| `missed-notifications.claim.<pid>`                                                        | inherited from the journal's inode (a rename does not chmod, and nothing in `src/main.rs:claim_by_rename` sets a mode)                                                                            | `src/main.rs:claim_by_rename` (by rename)                                   | `src/main.rs:stranded_claims` then `src/main.rs:take_claim`                                         | `rename`, twice: once to take it, once more to hold it                                                                    | Adopted by ANY later return with no liveness test (`src/main.rs:stranded_claims` matches the prefix alone)                                                                                         | Temporary process coordination. Its existence is an in-flight consumption, and `tests/dispatch.rs:the_claim_never_survives_the_run_whether_the_replay_delivered_or_not` asserts a completed run never leaves one.                    |
| `missed-notifications.held.<pid>.<seq>`                                                   | inherited, as above                                                                                                                                                                               | `src/main.rs:take_claim` (by rename)                                        | its owner, then a later `src/main.rs:stranded_claims` once the owner is gone                        | `rename`; the name sits outside the adoption prefix so a live reader cannot be raced                                      | `src/main.rs:abandoned_hold` plus `src/main.rs:owner_is_gone` (only `ESRCH` counts as gone)                                                                                                        | Temporary process coordination. It marks "a batch some run had taken and was reading when it died, in a window one rename wide".                                                                                                     |
| `missed-notifications.held.<pid>` (no sequence)                                           | inherited                                                                                                                                                                                         | nothing in this build writes it                                             | `src/main.rs:owner_is_gone`, which parses "the segment before the first dot"                        | as above                                                                                                                  | as above                                                                                                                                                                                           | Compatibility artifact. `src/main.rs:owner_is_gone` names it explicitly: "a bare held.<pid> from an older build ... parse[s] the same way".                                                                                          |
| `last-present`                                                                            | 0600 (`src/main.rs:publish_state_line`)                                                                                                                                                           | `src/main.rs:advance_marker`, called only from inside a claim               | `src/main.rs:read_epoch` via `src/main.rs:mark_present` and `src/main.rs:claim_moment`              | `rename` to `last-present.claim.<pid>[.<epoch>]` (`src/main.rs:claim_moment`)                                             | Not applicable: the marker itself is never stale; a claim over it can be (next row)                                                                                                                | Internal persistence detail. One decimal epoch, no operator text; `src/main.rs:LAST_PRESENT` states "Absent means no window at all, so a fresh install cannot recap the whole ring."                                                 |
| `last-present.new.<pid>`                                                                  | 0600                                                                                                                                                                                              | `src/main.rs:publish_state_line`                                            | nobody                                                                                              | named for the writing process                                                                                             | removed by its own writer on a failed rename                                                                                                                                                       | Temporary process coordination.                                                                                                                                                                                                      |
| `last-present.claim.<pid>.<epoch>` (or `last-present.claim.<pid>` with no readable clock) | inherited from the marker's inode                                                                                                                                                                 | `src/main.rs:claim_moment` (by rename)                                      | `src/main.rs:stranded_window_claim`                                                                 | `rename`, and a second rename to adopt a free one                                                                         | `src/main.rs:window_claim_is_free`: this run's own id, or `owner_is_gone`, or an age strictly greater than `STALE_WINDOW_CLAIM_SECS` (300 seconds). No epoch in the name means the pid test alone. | Temporary process coordination. At most one exists at a time (`src/main.rs:StrandedWindow`), and it is removed by its taker at the end of the claim.                                                                                 |
| `activity`                                                                                | 0600                                                                                                                                                                                              | `src/main.rs:record_activity`, unconditionally on every first-attempt event | `src/main.rs:activity_in` and the detached recap child                                              | `.lock` sibling for append; never claimed                                                                                 | Not applicable: pruned by depth (`ACTIVITY_KEPT` = 150) and never consumed                                                                                                                         | Internal persistence detail. `src/main.rs:ACTIVITY` states it is "Bounded state that prunes itself, never claimed and never consumed", which is what makes a recap idempotent by window rather than by deletion.                     |
| `activity.lock`, `activity.new.<pid>`                                                     | 0600                                                                                                                                                                                              | as for the journal's siblings                                               | as for the journal's siblings                                                                       | as for the journal's siblings                                                                                             | as for the journal's siblings                                                                                                                                                                      | Temporary process coordination.                                                                                                                                                                                                      |

No file in this area is a public external contract. The operator-facing contracts here are text rather
than files: the six `pns doctor` sentences in behavior 7, and the replayed card's identity (`agent: pns`,
`state: missed`, title `pns · missed`) in behavior 15.

Adjacent files this area touches but does not own: `decisions` (the decision ring, written by
`src/main.rs:record_decision`), `quiet-until` (the operator's mute, which is what most journal entries
come from), and `lights-news` (armed by `src/main.rs:record_news` beside the journal write). They are out
of scope here.

______________________________________________________________________

## Gaps

- NOT ESTABLISHED: no test exercises `STALE_WINDOW_CLAIM_SECS`. `tests/dispatch.rs:plant_window_claim`
  writes `last-present.claim.<owner>` with no epoch segment, so both window-claim tests take the pid-only
  path in `src/main.rs:window_claim_is_free`. The behavior of the age test against a claim whose owner is
  genuinely still alive at 301 seconds is neither stated in the code nor covered.
- NOT ESTABLISHED: `src/main.rs:claim_by_rename` states outright that its own pid guard "IS NOT PINNED BY
  A TEST, and cannot be: no test can plant a claim named for a process id the engine has not been given
  yet."
- NOT ESTABLISHED: the interleaved-claim arm of `src/main.rs:republish_after` (an append whose read-back
  returns `NotFound` because a claim renamed the file away mid-append) is described as "a race no test in
  this tree can stage deterministically; what is pinned here is the decision, and the race itself belongs
  to the out-of-tree probe."
- NOT ESTABLISHED: `src/main.rs:claim_journal` names one accepted race with no test behind it: "an append
  that opened the journal path before the rename writes into the claimed inode, and is replayed or lost
  depending on which side of the read it lands."
- KNOWN GAP, stated in the code rather than missing: the doctor's count reads the journal's own name, so
  a batch waiting under a claim name or a held name is invisible to `pns doctor`
  (`src/main.rs:claim_journal`).
