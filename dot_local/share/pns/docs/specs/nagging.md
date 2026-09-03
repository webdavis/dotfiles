# Nagging: the second card about an approval nobody answered

This is the one path in pns that speaks about an event minutes after it happened, and the only one whose
delivery is decided in a process the harness never started. When Claude Code blocks on a permission
prompt, `pns hook blocked` arms a nag: it writes one record naming the approval and registers one leased
job with the daemon. If nothing clears that record before the schedule runs out, the daemon re-executes
this same binary as `pns nag`, and that run (the "fire") cards every outstanding approval at once, as one
statement, and never as a second answerable prompt. Everything here falls out of four rules. First, one
approval earns at most one nudge, because the fire consumes the record it counted. Second, several
outstanding approvals are one card and not several, which is the operator's coalescing ruling and the
reason `pns nag` takes no session argument at all. Third, every unreadable, absent, ambiguous or failed
input resolves to silence rather than to a nudge taken on a guess (`src/nag.rs:fate`, "EVERY DROP MEANS
SILENCE"). Fourth, ownership between racing processes is taken by rename or by exclusive creation and
never by removal, because on this filesystem a concurrent unlink reports success to every racer.

Vocabulary, in the code's own words. A **nag** is "one more card when an approval has been sitting
unanswered" (`src/config_text.rs`, the `[nag]` table prose); `[nag] after_secs` is "how long an
unanswered approval waits before it is carded a second time, in seconds"
(`src/config.rs:Config::nag_after_secs`). A **fire** is one run of `pns nag`: "`pns nag`: one card about
every approval nobody has answered, or silence" (`src/main.rs:nag_mode`). A **claim** is ownership taken
by renaming a file out of its own name: "THE RENAME IS THE OWNERSHIP TEST" (`src/nag.rs:claim_path`,
`src/main.rs:claim_record`). A **lease** is the daemon's `until` field, "the LEASE: past this second the
job is dropped, never run" (`src/daemon.rs:Job::until`). The **record** is one approval waiting on the
operator (`src/nag.rs:Record`), the **answered marker** is the empty file whose presence cancels a job,
and the **nudge** is the card's own sentence (`src/nag.rs:nudge`).

## The invariant this design is built on

Cited rather than assumed. On this filesystem a concurrent unlink reports success to EVERY racer, so a
file protocol owns a file by rename or by exclusive creation and never by removal. The crate states it as
a decision record, `docs/decisions/0001-ownership-by-rename-not-by-unlink.md`, whose title is "A file
protocol owns a file by rename, never by removal" and whose status line reads "accepted, and load bearing
for every filesystem protocol in the crate". Its measurement paragraph is the canonical one: "`unlink`
does not arbitrate between racing processes on this machine's filesystem, which is APFS (Apple File
System). It reports success to every caller. This was measured directly: eight racers each removed the
same path and all eight were told they had succeeded." Its own table of implementation sites names three
of this area's four: `src/nag.rs:claim_path` owns "one approval record, taken by a fire before it is read
for anything", and `src/main.rs:claim_record`, `claim_fire` own "a nag record and the fire lock".

The code points at that record from each site rather than restating the measurement:

- `src/nag.rs:claim_path`: "THE RENAME IS THE OWNERSHIP TEST and this is the name it renames to. A plain
  unlink does not arbitrate on this filesystem, which is why the fire takes a record by rename before
  reading it for anything. The measurement behind that is in
  `docs/decisions/0001-ownership-by-rename-not-by-unlink.md`."
- `src/main.rs:nag_mode`: "Both are renames because a plain unlink does not arbitrate on this filesystem;
  the measurement is in `docs/decisions/0001-ownership-by-rename-not-by-unlink.md`."
- `src/main.rs:claim_lock` still carries the measurement inline, and is the one place on the fire's own
  path where the rule is applied to a lock rather than to a record: "THE DEAD LOCK IS TAKEN BY RENAME AND
  NEVER BY REMOVE, which is the one place arbitration is still needed on this path: a remove reports
  success to EVERY racer on APFS (measured, eight racers all told they had succeeded), so two processes
  clearing one dead lock would each then create a fresh one and both would own the window. A rename does
  arbitrate."
- `src/daemon.rs:claim` also still carries it, with the only comparison against the rename: "measured on
  macOS 26.2 (APFS) and recorded in `take_claim`'s own doc comment, eight processes unlinking one path
  were every one of them told they had succeeded, while 40 rounds of eight racers renaming gave exactly
  one winner every time."
- `src/main.rs:update_blocked_marker` states the cost where the rule could NOT be applied, and the
  decision record repeats it under "What the rule does NOT fix, stated so nobody re-derives it": one file
  per session carries no generation, so a blocked event that publishes a new wait while a previous Stop
  is still condensing loses that wait when the Stop reaches its removal. That bounds how promptly a
  wait's lamp reflects reality; it does not reach the nag's own record or marker, which are keyed the
  same way but never removed by a sweeper.

The fire window is the one place where a rename is NOT the arbiter, and the reason is measured too. A
rename claim moves the contended name out of the way, so a later racer finds no lock at that name and
creates one; `src/main.rs:claim_fire` records that "that form delivered TWO cards from four concurrent
fires, reproducibly, under load", and an exclusive create is used instead because it "leaves the lock
sitting at its name for the whole fire". The rename survives inside `claim_lock` for the one step where a
remove would be unsafe: taking over a lock that aged out.

## State files

Every path is relative to the state directory, which is `$PNS_STATE_DIR` or `~/.local/state/pns`
(`src/main.rs:state_dir`). "0600" is `src/main.rs:STATE_FILE_MODE`, the mode "every other state file the
crate publishes" carries (`src/daemon.rs`, same constant).

| Path                                | Mode                                                                                                                                                                                                       | Written by                                                                                                             | Read by                                                                                             | Ownership arbitration between racers                                                                                                                                                                                                                                          | Stale reclaim                                                                                                                                                                                                                                                                                                                                | Class                                                                                                                                                                                                                                                                                                                                                              |
| ----------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `nag/`                              | the process umask (`std::fs::create_dir_all` in `src/main.rs:nag_mode` and `src/main.rs:publish_state_line`; NOT ESTABLISHED: no explicit mode is set on this directory anywhere, and no test asserts one) | `arm_nag` by way of `publish_state_line`, and `nag_mode` before it takes the fire lock                                 | `record_entries`, `claim_fire`                                                                      | Not applicable, `create_dir_all` is idempotent and both writers tolerate an existing directory (`let _ = std::fs::create_dir_all(&directory)`)                                                                                                                                | never removed                                                                                                                                                                                                                                                                                                                                | internal persistence detail: nothing outside this crate names it, and `src/nag.rs:nag_dir` justifies it purely as an enumeration optimisation ("the fire ENUMERATES records, and a flat directory would mean pattern-matching every other state file on every wake")                                                                                               |
| `nag/<session>.pending`             | 0600, asserted in `tests/hooks.rs:arming_writes_a_record_registers_a_job_and_clears_a_stale_marker_first` ("state is owner-only, like every other file this crate publishes")                              | `src/main.rs:arm_nag`, published write-then-rename                                                                     | `src/main.rs:record_entries` then `src/main.rs:claim_record`; removed by `src/main.rs:clear_nag`    | taken by rename to `<name>.claim.<pid>` (`src/main.rs:claim_record`); `clear_nag` removes it best effort and never depends on winning                                                                                                                                         | dropped by the fire when `src/nag.rs:is_stale` says so, or wholesale when the nag is off (`src/main.rs:nag_mode`)                                                                                                                                                                                                                            | internal persistence detail: it is the arm's private note to the fire, its name is derived (`src/nag.rs:record_path`) rather than typed, and nothing outside `pns` reads it                                                                                                                                                                                        |
| `nag/<session>.new.<pid>`           | 0600, set on the open handle and again after it (`src/main.rs:publish_state_line`)                                                                                                                         | `src/main.rs:publish_state_line` during an arm                                                                         | nobody; it is renamed onto the record path                                                          | the name carries this process's own identifier, so no two live processes contend for it                                                                                                                                                                                       | a crash between the open and the rename leaves it; the next run of that pid truncates and reuses it, and it can never be enumerated as a record because it does not end in `.pending`                                                                                                                                                        | temporary process coordination: it exists only to make the record's publication atomic                                                                                                                                                                                                                                                                             |
| `nag/<session>.pending.claim.<pid>` | 0600, inherited from the record it renames                                                                                                                                                                 | `src/main.rs:claim_record`                                                                                             | the fire that took it (`std::fs::read_to_string(&claim)`), removed by that same fire after the card | the rename IS the ownership test; `claim_record` refuses to rename over a name already occupied (`symlink_metadata(&claim).is_ok()` returns None), because anything there is a record this pid claimed and could not finish                                                   | NOT ESTABLISHED: no age rule, no sweeper and no test reclaims a stranded record claim. `src/main.rs:nag_mode` says a crash after the card "leaves claims nothing re-enumerates", and `tests/hooks.rs:an_unanswered_approval_is_nudged_once_through_the_ordinary_paths` asserts the accepted risk "covers only what a CRASH mid-fire strands" | temporary process coordination: a working name, held for the length of one fire                                                                                                                                                                                                                                                                                    |
| `nag/fire.lock`                     | 0600 (`src/main.rs:publish_lock`, `.mode(STATE_FILE_MODE)`)                                                                                                                                                | every fire, by exclusive create                                                                                        | every fire (`src/main.rs:claim_fire`), removed by the winner in `src/main.rs:release_fire`          | `OpenOptions::create_new(true)`, so "of any number of processes racing this exactly one is told it succeeded" (`src/main.rs:publish_lock`); it also never follows a symlink                                                                                                   | aged out at `src/nag.rs:FIRE_STALE_SECS` = 60 seconds and then taken by rename, never by remove (`src/main.rs:claim_lock`). A lock whose own modification time cannot be read counts as LIVE (`src/main.rs:lock_aged_out`)                                                                                                                   | temporary process coordination: it names no approval and holds no fact, it owns a window                                                                                                                                                                                                                                                                           |
| `nag/fire.lock.claim.<pid>`         | 0600, inherited from the lock it renames                                                                                                                                                                   | `src/main.rs:claim_lock` when it takes over a dead lock                                                                | nobody; it is removed immediately after the rename                                                  | the rename is the arbitration, which is why a remove is not used here                                                                                                                                                                                                         | NOT ESTABLISHED: a crash between the rename and the remove strands it, and the same pid is then refused the takeover path for that lock (`symlink_metadata(&claim).is_ok()` returns false). No test covers it                                                                                                                                | temporary process coordination                                                                                                                                                                                                                                                                                                                                     |
| `daemon-markers/nag-<session>`      | 0600, set on the open and again after it because "a marker left by an earlier arm in this session is reused rather than made" (`src/main.rs:write_marker`)                                                 | `src/main.rs:clear_nag` (both clearing signals), and the fire itself for every counted record (`src/main.rs:nag_mode`) | `src/daemon.rs:marker_exists` on every tick, and the fire's own `src/nag.rs:fate`                   | not contended: presence is the whole message, the name is constant per session, and a second write rewrites the same file. `src/main.rs:clear_nag` writes it unconditionally, precisely so it reaches a record another process is holding under a name this one does not know | cleared by the NEXT arm in that session, which unlinks it before publishing the new record (`src/main.rs:arm_nag`). Nothing else sweeps it: `src/main.rs:clear_nag` names the cost, "one marker file per session that ever resolves a tool batch or ends a turn", and accepts it on the no-removal-mechanisms terms                          | public external contract: the daemon's own documented usage takes a marker by name (`src/main.rs:DAEMON_USAGE`, `[--unless-marker <name>]`), and `tests/hooks.rs:the_daemon_really_fires_the_nag_and_really_drops_it_when_the_marker_is_there` types `--unless-marker nag-s1` by hand, so the `nag-<session>` spelling is an interface a person can and does write |
| `daemon/nag:<session>`              | 0600 (`src/daemon.rs:STATE_FILE_MODE`)                                                                                                                                                                     | `src/daemon.rs:schedule`, called from `src/main.rs:arm_nag`                                                            | the daemon loop (`src/main.rs:drain_spool`)                                                         | the id is the filename, so re-registering REPLACES by rename and newest signal wins (`src/daemon.rs:Job::id`, `src/daemon.rs:publish_job`); the daemon claims by rename and its own put-backs are create-if-absent, so a client always wins (`src/daemon.rs:hand_back`)       | the lease: `now > until` drops it (`src/daemon.rs:decide`), and the marker drops it sooner                                                                                                                                                                                                                                                   | public external contract: the whole registration is expressible as `pns daemon schedule --id nag:s1 --in 0 --unless-marker nag-s1 -- nag`, which is exactly what `tests/hooks.rs:the_daemon_really_fires_the_nag_and_really_drops_it_when_the_marker_is_there` runs                                                                                                |

No file in this area is a compatibility artifact. The one compatibility-shaped thing the nag carries is
not a file: it is the state WORD inside the card, `src/main.rs:BLOCKED_STATE` = `"blocked"`, kept because
"a new word would fall out of `missed_notifications::NEEDS_YOU`, and an unanswered approval is exactly
what that section is for" (`src/main.rs:nag_mode`).

______________________________________________________________________

### 1. One key is the switch and the schedule

Given an operator who wants an unanswered approval carded a second time

When `[nag] after_secs` is read out of the configuration file

Then a whole number of seconds between 30 and 3600 arms the feature at that schedule, zero and an absent
table are the same statement (off), and every other value is refused by name.

- Success: `[nag]\nafter_secs = 300\n` yields 300, and 30 and 3600 are admitted at their own edges
  (`src/config.rs:the_nag_table_reads_one_schedule_defaults_off_and_zero_is_off_rather_than_an_error`).
  The operator-facing wizard asks one question for it, verbatim:
  `Card you a second time about an approval left unanswered?` (`src/main.rs`, driven by `tests/setup.rs`
  which waits on the substring `approval left unanswered`), and a `y` inserts a bare `[nag]` table so the
  schedule ships at its rendered default of 300 (`src/setup.rs:Answers::values`, `src/config_text.rs`
  `Sample::Default("300")`).
- Failure sources: a negative number, a duration string such as `"5m"`, a fractional number, a list, 29,
  3601, a misspelled key `after_seconds`, and `nag = 300` written as a scalar rather than a table. Each
  is refused with the offender named
  (`src/config.rs:a_schedule_that_is_not_a_count_of_seconds_is_refused_by_name`). The messages are
  `` `nag` key `after_secs` is {count}, outside the 30 to 3600 second range; 0 is the feature off ``,
  `` `nag` key `after_secs` has type `{type}`, not a count of seconds `` and `` `nag` is not a table ``
  (`src/config.rs:nag_schedule`, `src/config.rs:parse_nag`).
- Fail direction: fail-closed toward the operator's interruption. `src/main.rs:nag_after_secs` reads an
  unreadable or refused config as `NAG_OFF`: "a file nobody can parse asked for nothing, and a feature
  that INTERRUPTS must not be switched on by a parse failure". It names `[recap]` as the deliberate
  contrast, whose fallback is ON.
- Thresholds: the floor is 30 seconds exactly, admitted; 29 is refused. The ceiling is 3600 seconds
  exactly, admitted; 3601 is refused. Zero is carved out of the range and is not an error; one is inside
  the refused band. The floor exists because "a nudge arriving before the operator could plausibly have
  picked up their phone is the stacking this design forbids", and 30 is "low enough that the feature can
  be drilled in half a minute" (`src/config.rs:nag_schedule`).
- Required side effects: none, this is a parse.
- Forbidden side effects: the value is never clamped. "REFUSED RATHER THAN CLAMPED, in `min_events`'s
  style: a silently corrected schedule is a schedule the operator believes they set"
  (`src/config.rs:nag_schedule`).
- Timeout and cancellation: Not applicable, one file open and one Tom's Obvious Minimal Language (TOML)
  parse.
- Idempotency and duplicates: the config is read three times on the blocked path (once by `run_event`,
  once by the wait, once by `arm_nag`), which `src/main.rs:blocking_event` names out loud rather than
  routing around: "each is one open and one TOML parse of a file measured in kilobytes, off local disk,
  with no network and no subprocess in any of them".
- Privacy: Not applicable, no secret is involved in this key.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: one key that is both switch and schedule, following `[focus] silence`, "so
  there is no second `enabled` key that can disagree with the first"
  (`src/config.rs:Config::nag_after_secs`). Default OFF, unlike `[daemon]` beside it, because this one
  interrupts and needs three separate operator steps before it works at all.

### 2. A cross-table refusal: the lamp backstop must outlast the nag

Given `[lights.blocked] give_up_after_secs` darkens an unanswered wait's lamp, and `[nag] after_secs`
cards that same wait

When both tables are present and the nag is on

Then a `give_up_after_secs` shorter than `after_secs` is refused at load, because it is a configuration
that gives up on a wait before it has ever nudged about it.

- Success: 600 and 600 load (equal is accepted), and 300 against 57600 loads (`src/config.rs`, the tests
  beside `backstop_outlasts_the_nag`).
- Failure sources: `[nag] after_secs = 600` with `[lights.blocked] give_up_after_secs = 60` is refused
  with
  `` `lights.blocked` key `give_up_after_secs` is 60, below `nag` key `after_secs` 600, so the lamp would be given up on before the nudge it belongs to has ever fired ``
  (`src/config.rs:backstop_outlasts_the_nag`).
- Fail direction: fail-closed, the whole file is refused at load rather than worked around at runtime.
- Thresholds: strictly less than is the contradiction. Equal is accepted: "reaching the bound exactly as
  the nudge fires is a tight config the operator may well mean". One second below is refused.
- Required side effects: none.
- Forbidden side effects: no runtime mechanism compensates. The check "belongs where the whole file is in
  hand" rather than in either table's own parser.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: Not applicable.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: two of this check's guards are DEAD TODAY and the code says so rather than
  leaving a reader to discover it. `NAG_OFF` is zero, so the off-nag early return can never be the thing
  that makes the comparison false; and the default `give_up_after_secs` (16 hours) sits far above
  `MAX_NAG_AFTER_SECS` (one hour), so a file with no `[lights]` table could not trip it either. Both stay
  "because what makes them dead is a coupling between two bounds that have nothing else to do with each
  other" (`src/config.rs:backstop_outlasts_the_nag`).

### 3. Arming a nag: what a blocked approval leaves behind

Given `pns hook blocked` running for a Claude Code permission prompt, with `[nag] after_secs` set

When `src/main.rs:arm_nag` runs, after the moshi forward has been started and before the notification
itself

Then this session's previous answered marker is cleared, one record is published at
`nag/<session>.pending`, and one leased job is registered at `daemon/nag:<session>`.

- Success: `tests/hooks.rs:arming_writes_a_record_registers_a_job_and_clears_a_stale_marker_first` reads
  the record back as JSON (JavaScript Object Notation) and asserts `detail` is `Bash: cargo test`,
  `agent` is `claude`, and the file mode is `0o600`. It then asserts the spool entry contains
  `id=nag:s1`, `marker=nag-s1` and `args=["nag"]`, that `until - due == 300`, and that the pre-planted
  marker is gone.
- Failure sources: an unreadable clock (`src/main.rs:now_secs` returning None) arms nothing; a session id
  that cannot become a filename arms nothing (behavior 5); a marker that could not be cleared, a record
  that could not be written, and a registration that was refused each produce their own stderr line
  (behaviors 4 and 6).
- Fail direction: toward the operator being helped. The arm is placed "AFTER THE FORWARD IS STARTED AND
  BEFORE THE NOTIFICATION" so "the clock starts at the true prompt time and so a notification that dies
  still leaves a timer armed, which is the direction that helps the operator"
  (`src/main.rs:blocking_event`).
- Thresholds: `due = now + after_secs` and `until = due + after_secs`. The lease is one whole schedule
  past the due second, and
  `tests/hooks.rs:arming_writes_a_record_registers_a_job_and_clears_a_stale_marker_first` states why one
  step below would be wrong: "`until == due` is a zero-length lease: the daemon drops a job one second
  past `until`, so a busy tick or a laptop that woke a moment late loses the nudge entirely". Both edges
  of the lease are closed at fire time (`due <= now <= until` fires, `src/daemon.rs:decide`), so at
  exactly `until` the job still runs and one second later it is dropped.
- Required side effects: exactly three, in this order. The marker unlink, the record publish, the spool
  registration. `armed` is the arm's own clock reading, never the fire's: "`armed` IS THE PROMPT'S OWN
  SECOND, read once by the hook that wrote the record. The fire is a different process minutes later and
  its clock read is the OTHER end of the measurement; taking both from the fire would make every record
  look freshly armed" (`src/nag.rs:Record`).
- Forbidden side effects: nothing on stdout, ever. "Claude Code parses this hook's stdout as
  `let t = e.trim(); if (!t.startsWith(\"{\")) return { plainText: e }`, so one stray line in front of
  moshi's object turns an Allow into no decision at all" (`src/main.rs:arm_nag`). Pinned by
  `tests/hooks.rs:arming_writes_nothing_the_harness_could_read_as_a_decision`, which forces the
  registration to fail and still asserts stdout is exactly empty and the exit code is the moshi stub's
  own 42. No second round trip to moshi either: the same arming test asserts
  `submissions(&sandbox) == vec!["claude-hook"]`, "THE SINGLE-SUBMITTER RULE".
- Timeout and cancellation: none, and that is the bound. "NO NETWORK, NO SUBPROCESS, NO SPAWN AND NO WAIT
  ON ANY OF THEM, which is what makes it safe to sit in front of a notification the operator is waiting
  on". Measured on dresden, 500 runs of the blocked hook each way: "134.7ms +/- 14.1ms armed against
  134.8ms +/- 13.3ms unarmed. The arm is not separable from the hook's own run-to-run variation"
  (`src/main.rs:arm_nag`).
- Idempotency and duplicates: a second approval in the same session REPLACES the first rather than
  stacking. The record path and the job id are both derived from the session id alone
  (`src/nag.rs:record_path`, `src/nag.rs:job_id`: "ONE JOB PER APPROVAL, and the id is the spool
  filename, so a second approval in one session REPLACES the job rather than stacking a second one"). The
  cost is stated in `src/nag.rs:Record`: the session id is the FILENAME and is deliberately not also a
  field inside, so a hand edit cannot set two copies of one fact against each other.
- Privacy: the record holds "the permission prompt's own text, which is the operator's free text and the
  reason this file is JSON" (`src/nag.rs:Record::detail`), at 0600. None of it reaches the spool: "NO
  FREE TEXT REACHES THE SPOOL. `args` are visible in the spool file and in whatever the daemon logs, and
  the detail is the operator's own question, so it lives in the record and `pns nag` takes no argument"
  (`src/main.rs:arm_nag`). The doctor's own view of the spool is a count and never the contents
  (`src/daemon.rs:job_count`).
- Process ownership and cleanup: nothing is spawned. The record is published write-then-rename through
  `src/main.rs:publish_state_line`, whose pending file carries the mode and this process's own pid, and
  which removes the pending file if the rename fails "so nothing half-written is left in the state
  directory".
- Compatibility contract: the record is JSON and never line-oriented `key=value`, "for the journal's own
  reason: the detail is a permission prompt's text and can carry a newline, a tab or a quote, and a
  line-oriented form would let one of those forge a second record" (`src/nag.rs:render`). It is built
  with `serde_json::json!` and never `format!`, which is this repo's build-JSON-with-arguments rule in
  Rust. Reading is by key and never by position, and a missing key reads as empty rather than refusing
  the record (`src/nag.rs:parse`). One consequence worth stating: `branch` is always empty in practice
  today, because `src/main.rs:blocking_event` builds its `EventArgs` with `project` and no `branch`, so
  `arm_nag` copies an empty string into the record. The test fixture writes `"branch": ""` for the same
  reason (`tests/hooks.rs:write_record_at`).

### 4. The previous approval's marker is cleared before the new record is published

Given the answered marker's name is constant per session (`nag-<session>`), so a marker left by an
earlier approval in this session is indistinguishable from one left by this approval

When `arm_nag` runs

Then the marker is unlinked FIRST and only then is the record published.

- Success: `tests/hooks.rs:arming_writes_a_record_registers_a_job_and_clears_a_stale_marker_first` plants
  the marker before the hook runs and asserts it is gone afterwards, "or this one is silently never
  nudged".
- Failure sources: an unlink that fails for any reason other than `NotFound` prints
  `pns: a previous approval's answered marker could not be cleared ({error}); this approval will not be nudged`
  on stderr (`src/main.rs:arm_nag`). `NotFound` is the ordinary case and is silent.
- Fail direction: loud and continuing. The arm carries on and writes the record anyway; the stderr line
  is what makes the outcome discoverable, on the reasoning that "an action that suppressed its own error
  has only been attempted".
- Thresholds: Not applicable, this is an ordering rather than a duration.
- Required side effects: the unlink precedes the record publish. Two distinct defects are closed by that
  order. Clearing at all closes the stale-marker defect: "the marker name is constant PER SESSION, so one
  left by the PREVIOUS approval in this session would make the new job drop silently". Clearing BEFORE
  the record closes a concurrency window: published first, "the new record can be claimed by a fire that
  then finds the PREVIOUS approval's marker still on disk and drops it as answered, which costs this
  approval its nudge. Cleared first, the worst a fire in the window can find is the previous approval's
  own record with no marker, which is an outstanding approval being nudged about correctly"
  (`src/main.rs:arm_nag`).
- Forbidden side effects: no other session's marker is touched, because the name is derived from this
  session id alone (`src/nag.rs:marker_name`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: unlinking an absent marker is a no-op by design.
- Privacy: the marker is empty; "present is the whole message" (`src/main.rs:write_marker`).
- Process ownership and cleanup: Not applicable, one unlink.
- Compatibility contract: `src/nag.rs:marker_name` returns a NAME and never a path, "which is what keeps
  the field from becoming a general filesystem probe"; the daemon resolves it inside its own marker
  directory (`src/main.rs:marker_path`, `src/daemon.rs:marker_dir`).

### 5. A session id that cannot be a filename arms nothing at all

Given a session id arrives inside a harness payload and is interpolated into three names (a record path,
a marker name and a job id)

When any of those names is asked for

Then an id that is empty, traversal-shaped, separator-carrying, control-carrying, leading-dot or longer
than the daemon's own registration cap names nothing, and the arm returns having written nothing.

- Success: `abc-123` yields `/s/nag/abc-123.pending`, `nag-abc-123` and `nag:abc-123`, and an id of
  exactly `MAX_SESSION_ID_CHARS` characters still names a record
  (`src/nag.rs:an_ordinary_session_id_names_a_record_a_marker_a_job_and_a_claim`,
  `src/nag.rs:a_session_id_that_cannot_be_a_filename_names_nothing_at_all`).
- Failure sources: the refused set is pinned as a table: `""`, `".."`, `"../etc/passwd"`, `"a/b"`,
  `".hidden"`, `"a\u{7}b"`, `"a\nb"`, `"a b"`, `"a:b"`, and one character over the length cap. The
  predicate is `src/safety.rs:session_id_is_safe` plus two bounds this layer owns: a leading dot ("a
  hidden file is not a name this writes") and the length.
- Fail direction: "FAIL IN THE SAFE DIRECTION, which here is arming nothing: an id that cannot become a
  name is one no record, marker or job is written for" (`src/nag.rs`, that test's own comment).
- Thresholds: `MAX_SESSION_ID_CHARS = daemon::ID_MAX - "nag:".len()` = 64 minus 4 = 60. At exactly 60 the
  record is named; at 61 all three names are refused. This is "a correctness bound rather than tidiness:
  past it `job_id` is refused at registration, so a longer id would write a record no nudge could ever be
  scheduled for and the file would sit there unread" (`src/nag.rs:MAX_SESSION_ID_CHARS`).
- Required side effects: none. `arm_nag`'s let-else returns before the clock is even read.
- Forbidden side effects: no partial arm. All three names are resolved in one tuple destructure, so a
  record is never written for an id whose job id would be refused (`src/main.rs:arm_nag`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: Not applicable.
- Privacy: an id that would escape the state directory never reaches a path, which is the whole point of
  the predicate (`src/safety.rs:session_id_is_safe`: "the id arrives inside a hook payload and is
  interpolated into a path, so a value carrying a separator or a parent reference would write outside its
  directory").
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `session_id_is_safe` also refuses anything shaped like this crate's own working
  files (`<name>.new.<pid>`, `<name>.sweep.<pid>`, via `src/lights.rs:working_owner`), so a session id
  cannot be misread as a working file by the sweeper that reads that grammar.

### 6. Nothing is armed when nothing should be, and a nudge that cannot be scheduled leaves no record

Given three reasons not to arm: no `[nag]` table, `after_secs = 0`, and an agent that is not Claude Code

When `pns hook blocked` runs under each

Then no record is written and no job is registered; and separately, when the registration itself is
refused, the record that was already written is removed again.

- Success: `tests/hooks.rs:nothing_is_armed_when_nothing_should_be` sweeps all three cases and asserts
  both `!nag_record(...).exists()` and `spool_entries(...).is_empty()`.
  `tests/hooks.rs:an_approval_whose_nudge_could_not_be_scheduled_leaves_no_record_behind` blocks the
  spool with a regular file at `state/daemon`, asserts the stderr line, asserts the record is gone, and
  then runs a fire and asserts the delivery count did not move.
- Failure sources: `src/daemon.rs:schedule` refuses on shape (`validate_shape`) or on a `due` more than
  `DUE_WINDOW_SECS` (thirty days) from now, or the spool write itself fails.
- Fail direction: the stderr sentence and the state on disk must agree.
  `pns: the nag could not be scheduled ({refusal}); this approval will not be nudged, its record is dropped`,
  or, when even that removal fails, `... and its record could not be dropped either`. The reasoning is
  explicit: a record with no job "stays ENUMERABLE: a sibling approval's fire, or the operator running
  `pns nag` by hand, counts it and cards about it. Leaving it would be this line saying one thing while
  the state on disk said another" (`src/main.rs:arm_nag`).
- Thresholds: Not applicable to the three refusals. The registration window is thirty days
  (`src/daemon.rs:DUE_WINDOW_SECS`), which the one-hour ceiling clears "by three orders of magnitude"
  (`src/config.rs`).
- Required side effects: on the registration refusal, one stderr line and one unlink.
- Forbidden side effects: nothing on stdout (behavior 3), and the exit code is untouched: the same
  scenario in `tests/hooks.rs:arming_writes_nothing_the_harness_could_read_as_a_decision` returns moshi's
  own 42.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: Not applicable.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the Codex refusal is a POSITIVE gate, `event.agent != CLAUDE_AGENT` returns,
  "so an empty or unknown `PNS_AGENT` arms nothing either (bug class 16: set-but-empty is not unset)".
  The reason is behavioral, not architectural: "Codex wires exactly Stop and PermissionRequest, so it has
  a turn-end clear and no batch-level one, and agent turns in this repo routinely run tens of minutes: a
  Codex nag would be wrong in the COMMON case rather than at an edge" (`src/main.rs:arm_nag`).

### 7. Clearing a nag: one rule, two writes, three call sites

Given an approval that was dealt with

When any clearing signal arrives for that session

Then `src/main.rs:clear_nag` writes the answered marker FIRST and then removes the record, both best
effort.

- Success: `tests/hooks.rs:an_answered_approval_is_never_nudged_by_either_clearing_signal` drives both
  `pns hook resolved` and `pns hook stop`, asserting for each that the record is gone, the marker exists,
  and a fire run afterwards adds no delivery at all. `resolved` delivers nothing of its own; `stop`
  legitimately delivers its own turn card, which is why the expected count is stated per word rather than
  asserted zero for both.
- Failure sources: a marker write that fails prints
  `pns: an answered marker could not be written ({error})` on stderr and the record is STILL removed,
  "because the record's absence already carries the same fact and the marker is only what saves the
  daemon a no-op spawn" (`src/main.rs:clear_nag`). The record removal is `let _ =`, silent, because the
  ordinary case is a session that never armed.
- Fail direction: "A crash between the two leaves an approval that is never nudged rather than one nudged
  after being answered, which is the safe direction" (`src/main.rs:clear_nag`).
- Thresholds: Not applicable, no duration.
- Required side effects: the marker is written WHETHER OR NOT a record is there, and that is a
  correctness requirement rather than a simplification (behavior 8).
- Forbidden side effects: `pns hook resolved` "loads no config and delivers nothing. A record exists only
  because the feature was on when the approval arrived, so clearing it is right regardless of what the
  config says now, and that keeps this per-batch path to a payload read, a parse and at most two file
  operations" (`src/main.rs:hook_mode`). Output goes to stderr and never stdout, "this runs on a harness
  hook whose output the harness reads".
- Timeout and cancellation: Not applicable, two local file operations.
- Idempotency and duplicates: fully idempotent. The marker name is constant per session so a second clear
  rewrites the same file, and removing an absent record is a no-op. The accumulation cost is named: "one
  marker file per session that ever resolves a tool batch or ends a turn, rather than one per session
  that armed a nag" (`src/main.rs:clear_nag`).
- Privacy: the marker is empty and 0600.
- Process ownership and cleanup: nothing is spawned; nothing sweeps the markers (behavior 7's own cost
  line, and the state table above).
- Compatibility contract: the marker records the BATCH'S RESOLUTION and never the operator's answer, and
  the code forbids a comment that says otherwise: "NO COMMENT HERE MAY SAY THE MARKER RECORDS THE
  OPERATOR'S ANSWER. It records the BATCH'S RESOLUTION, which is the only per-batch fact the harness's
  hook vocabulary carries: an approval answered at ten seconds whose tool then runs past the schedule is
  nudged about anyway" (`src/main.rs:clear_nag`). The shipped configuration template states the same cost
  to the operator: "The signal is the tool batch RESOLVING rather than your answer, so a tool approved at
  once that then runs longer than this is nagged about anyway; if that bites, raise the number"
  (`src/config_text.rs`). Three call sites reach it: `pns hook resolved` (the `PostToolBatch` signal,
  `src/main.rs:hook_mode`), `src/main.rs:end_of_turn` and `src/main.rs:failed_turn`. The Stop pair are
  the free backstop "for a batch payload over the 1MB cap, an operator who escaped the prompt instead of
  answering it, and the window between this merge and the apply that installs the PostToolBatch entry"
  (`src/main.rs:end_of_turn`). NOT ESTABLISHED: no test drives the `stop-failure` call site; the sweep in
  `tests/hooks.rs:an_answered_approval_is_never_nudged_by_either_clearing_signal` covers only `resolved`
  and `stop`.

### 8. A clear that lands inside the fire's claim window still writes the marker

Given the fire owns a record by RENAMING it out of its own name, so for the length of a read, a parse and
a marker test there is no `.pending` file for that session at all

When a clearing signal arrives during exactly that window

Then the marker is still written, and the holder's own marker check is what drops the record.

- Success: `tests/hooks.rs:a_clear_landing_inside_the_fires_claim_window_still_writes_the_marker`
  performs the fire's rename by hand (`state/nag/s1.pending.claim.1`), runs `pns hook resolved`, asserts
  the marker exists, renames the claim back, runs a real fire and asserts zero deliveries and no record
  left behind.
- Failure sources: a marker write that fails (behavior 7); nothing else.
- Fail direction: silence. Every drop the holder can make resolves to no card.
- Thresholds: Not applicable.
- Required side effects: the unconditional marker write. "The marker is the only signal that reaches a
  record somebody else is holding" (`src/main.rs:clear_nag`). A clear gated on the record's presence
  "does nothing in that window and the fire cards an approval that has just been dealt with".
- Forbidden side effects: the clear never touches the claim. It removes only `<session>.pending`, whose
  name the holder has already vacated, and it does not know the holder's pid.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: this is the window where the record's absence is NOT proof of resolution,
  which is exactly why the marker is not conditioned on it.
- Privacy: Not applicable.
- Process ownership and cleanup: the holder removes its own claim (behavior 15).
- Compatibility contract: the pid inside a claim name is not read by anything, which is why the test can
  stand in `1` for a real process: "The pid in the name is not read by anything, so any number stands in
  for the process holding the claim" (that test's own comment).

### 9. The record directory, and the names inside it

Given the state directory is otherwise flat

When records, claims and the fire lock are placed

Then they live in a `nag/` subdirectory, and the record suffix alone decides what a fire enumerates.

- Success: `src/nag.rs:nag_dir` is `state_dir.join("nag")`; `record_path` is `nag/<session>.pending`;
  `src/main.rs:record_entries` filters on `ends_with(RECORD_SUFFIX)` and sorts, "so a fire is
  deterministic". `src/nag.rs:session_of` inverts it and returns None for a name that is not a record.
- Failure sources: a `read_dir` that fails yields an empty list (`.into_iter().flatten().flatten()`), so
  a missing or unreadable directory is "nothing is waiting" rather than an error.
- Fail direction: silence, again.
- Thresholds: Not applicable.
- Required side effects: `nag_mode` calls `create_dir_all` on the directory BEFORE it takes the lock that
  lives in it, because "an operator running the fire by hand before anything has ever armed (drill step
  10\) has no directory to take a lock in, and a fire that could not say `nothing is waiting` would read
  as broken" (`src/main.rs:nag_mode`).
- Forbidden side effects: a claim can never be re-enumerated as a record. "THE SUFFIX IS THE WHOLE TEST,
  which is what keeps a claim out of the fire's enumeration: a claim is `<name>.claim.<pid>` and ends in
  digits, so it can never be read back as a record and taken a second time" (`src/nag.rs:session_of`,
  asserted by `src/nag.rs:an_ordinary_session_id_names_a_record_a_marker_a_job_and_a_claim`, which pins
  `session_of("abc-123.pending.claim.7") == None`). The fire lock is likewise "NOT A RECORD NAME, so it
  can never be enumerated as one" (`src/nag.rs:FIRE_LOCK`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: the claim name is built from the WHOLE file name and never with
  `Path::with_extension`, which replaces everything after the LAST dot. "A harness session id may contain
  dots, so a claim derived from anything short of the full name can collapse two sessions onto one claim:
  one loses its nudge and the other can be delivered twice" (`src/nag.rs:claim_path`). Pinned by
  `src/nag.rs:two_ids_that_differ_only_after_a_dot_claim_two_different_names`, which asserts `a.b` and
  `a.c` claim different names under one pid and that the first is `/s/nag/a.b.pending.claim.7`.
- Privacy: the directory holds only 0600 files; the directory's own mode is NOT ESTABLISHED (see the
  state table).
- Process ownership and cleanup: `publish_state_line`'s pending file is `<stem>.new.<pid>`, which also
  cannot end in `.pending`, so it is invisible to the enumeration.
- Compatibility contract: `src/nag.rs:nag_dir` justifies the subdirectory from the daemon's precedent,
  "The daemon's own `daemon/` and `daemon-markers/` set the precedent."

### 10. `pns nag` takes no argument, and an argument is a refusal

Given coalescing means one fire looks at every outstanding record rather than at the one whose timer woke
it

When `pns nag <anything>` is typed

Then the usage goes to stderr, the exit code is 2, nothing is delivered and nothing is consumed.

- Success: `tests/hooks.rs:pns_nag_refuses_an_argument_rather_than_falling_through_to_a_fire` asserts
  `Some(2)`, that stderr contains `it takes no arguments`, zero deliveries, and that the planted record
  still exists ("and consumes nothing either: the approval is still waiting").
- Failure sources: any `argv[2]` at all, tested by `std::env::args_os().nth(2).is_some()`
  (`src/main.rs:nag_mode`).
- Fail direction: refuse. "ANY EXTRA WORD IS A REFUSAL, per the house rule that an unknown argument never
  falls through to help with exit 0. `pns nag <session>` is a command an operator would believe narrowed
  the fire, and coalescing means nothing here can honour it" (`src/main.rs:nag_mode`).
- Thresholds: Not applicable.
- Required side effects: the exact usage string, verbatim:
  `pns: usage: pns nag (it takes no arguments: one fire cards every outstanding approval at once)`
  (`src/main.rs:NAG_USAGE`).
- Forbidden side effects: the check is the FIRST thing in `nag_mode`, before the state directory is
  resolved, before the config is read and before the fire lock is taken, so a refused command touches no
  file at all.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: Not applicable.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the operator-facing usage line is
  `  pns nag                          card every outstanding approval` (`src/main.rs:USAGE`), and the
  daemon registers the job with `args: vec![NAG_MODE_WORD.to_string()]`, that is, exactly `["nag"]`. The
  two agree by construction: no session identifier is expressible in the spool entry, which is also the
  privacy rule of behavior 3.

### 11. The whole fire window is claimed once, by exclusive creation

Given the daemon can spawn two fires in one tick (two approvals armed inside one wall-clock second come
due together) and waits for neither

When each fire starts

Then exactly one of them takes `nag/fire.lock` by exclusive create and proceeds; every other exits 0
saying nothing on either stream.

- Success: `tests/hooks.rs:fires_racing_over_one_directory_still_produce_exactly_one_card` plants 200
  records (`RACED_RECORDS`), starts 4 concurrent fires (`RACED_FIRES`), asserts every one exits
  successfully, asserts exactly one delivery, and asserts no `.pending` file survives, "so the losers
  standing down costs no approval its nudge: the winner enumerated the directory after it owned it". The
  constants are chosen against a real failure mode: three records is "finished before a second process
  has finished exec, so a three-record race reports green against a build that splits".
- Failure sources: `publish_lock` fails when the lock already exists, when the path is a symlink (an
  exclusive create "NEVER FOLLOWS A LINK"), or when the directory is unwritable.
- Fail direction: stand down silently. "A LOSER SAYS NOTHING AT ALL, on either stream, and exits 0. The
  window belongs to another process whose one card names every approval this one would have, so a line
  here would be noise about work that is being done" (`src/main.rs:nag_mode`).
- Thresholds: the lock is believed for `src/nag.rs:FIRE_STALE_SECS` = 60 seconds (behavior 12).
- Required side effects: the lock is taken BEFORE anything is enumerated. Ownership taken per record
  instead "lets two woken processes each win a DISJOINT, NON-EMPTY subset and each card its own true
  count, which is one card per FIRE rather than one card per fire WINDOW, and that is precisely what the
  coalescing ruling forbids. Measured on the build before this: sixteen concurrent fires over one
  directory produced sixteen cards" (`src/main.rs:claim_fire`).
- Forbidden side effects: a rename is deliberately NOT used to take this lock. "A rename claim moves the
  contended name OUT of the way: the winner renames `fire.lock` to its own claim, so a racer that looked
  for a holder a moment earlier finds no lock at that name, creates one and takes it too. That form
  delivered TWO cards from four concurrent fires, reproducibly, under load" (`src/main.rs:claim_fire`).
- Timeout and cancellation: the fire holds the lock for its whole run and releases it at the end
  (`src/main.rs:release_fire`), on all three exit paths (feature off returns before the lock is taken at
  all; nothing waiting releases then prints; a delivered card releases after removing the claims).
- Idempotency and duplicates: this is the mechanism that answers "can two racing nags card the same
  approval twice". Under a live lock, no: only one process enumerates. See behavior 13 for the second
  level of ownership and behavior 12 for the one residual.
- Privacy: the lock is empty and 0600.
- Process ownership and cleanup: a release that fails prints
  `pns nag: the fire claim {path} could not be given up ({error}); the next fire waits it out`, and the
  consequence is named rather than implied: "the feature is not broken by a claim left behind, it is
  DELAYED, because the age test is what recovers it" (`src/main.rs:release_fire`).
- Compatibility contract: `claim_lock` is "THE SHAPE EVERY LOCK IN THIS BINARY USES", parameterised only
  by name and staleness, "because its two halves are only correct together: an exclusive create
  arbitrates between racers, and the age rule is what stops a holder that died from wedging the path
  forever" (`src/main.rs:claim_lock`).

### 12. A dead fire lock is reclaimed at a minute, and only by rename

Given a fire that crashed mid-run leaves its lock at `nag/fire.lock` with nobody to remove it

When a later fire finds the lock already there

Then it stands down unless the lock's modification time is more than 60 seconds old, in which case it
takes the dead lock by rename, removes it, and publishes a fresh one.

- Success: NOT ESTABLISHED. No test in this suite exercises the aged-out path for `fire.lock`; I grepped
  `tests/hooks.rs`, `tests/dispatch.rs` and `tests/daemon.rs` for `fire.lock` and `FIRE_STALE` and found
  only the comment at `tests/hooks.rs:fires_racing_over_one_directory_still_produce_exactly_one_card`.
  The behavior is stated at `src/main.rs:claim_lock` and `src/main.rs:lock_aged_out` and shared with the
  lights tick, which uses the same function with its own staleness.
- Failure sources: the metadata read can fail, the rename can fail, and the claim name can already be
  occupied. All three answer "not mine" and the caller stands down.
- Fail direction: one window lost, never two holders. "A LOCK WHOSE OWN CLOCK CANNOT BE READ COUNTS AS
  LIVE and stands the caller down. That is the safe direction (one window lost, never two holders), and
  the case behind it is a lock that vanished between the failed create and the question, which the next
  attempt resolves anyway" (`src/main.rs:lock_aged_out`).
- Thresholds: `FIRE_STALE_SECS` = 60, and the comparison is strictly greater than
  (`now.saturating_sub(at.as_secs()) > stale_secs`). At exactly 60 seconds old the lock is still believed
  and the caller stands down; at 61 it is reclaimable. The size is justified rather than picked: "A
  MINUTE IS A WIDE MARGIN OVER THE WORK IT COVERS. The holder claims every record by rename before it
  delivers anything, so a fire that broke in later would find an empty directory in any case; the lock
  only has to cover the enumeration, which is one `read_dir` and a rename per entry. What the wait costs
  when the holder really did crash is one nudge window, which is the safe direction"
  (`src/nag.rs:FIRE_STALE_SECS`).
- Required side effects: rename to `fire.lock.claim.<pid>`, remove that, then `publish_lock` again. The
  final `publish_lock` is still exclusive, so a second reclaimer that got as far as the create is still
  arbitrated.
- Forbidden side effects: never `remove_file` on the dead lock directly. See the invariant section.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: a reclaimer that already has a `fire.lock.claim.<pid>` sitting there (its
  own earlier crash, same pid reused) refuses rather than renaming over it, matching `claim_record`'s
  rule.
- Privacy: Not applicable.
- Process ownership and cleanup: the claim is removed immediately; a crash between the rename and the
  remove strands it (see the state table).
- Compatibility contract: the residual this threshold buys is stated in behavior 11's idempotency line
  and repeated here for completeness. A holder still working at 61 seconds can be broken in on. The
  design's answer is that the holder claims every record by rename before delivering anything, so the
  intruder enumerates an empty directory and cards nothing. NOT ESTABLISHED by test.

### 13. Each record is claimed by rename before it is read for anything

Given a fire that legitimately broke in after a stale window claim aged out, running beside a holder

When the fire walks `record_entries`

Then it renames each record to `<name>.claim.<pid>` first, and reads only what the rename gave it.

- Success: the mechanism is `src/main.rs:claim_record`. It refuses to rename over a claim already at that
  name; it renames; it then re-checks with `symlink_metadata` that what landed is a regular file and
  renames it BACK if it is not, so "AN IRREGULAR FILE GOES BACK WHERE IT WAS AND IS NEVER OPENED,
  following `append_ring_line`'s own refusal at a state path: a FIFO here would park the read forever".
- Failure sources: a rename that fails (another process got there) yields None and the entry is skipped
  entirely, having never been opened.
- Fail direction: skip in silence. "SOMEBODY ELSE OWNS IT, or it is not a regular file: either way this
  process never opened it and never counts it" (`src/main.rs:nag_mode`).
- Thresholds: Not applicable.
- Required side effects: the rename precedes the read, always.
- Forbidden side effects: no record is read in place and removed afterwards, even though that would pass
  the suite. The code says so explicitly: "NO TEST IN THIS SUITE KILLS THIS RENAME: reading each record
  in place and removing it afterwards passes everything, because every fire in the suite bar one is
  single-process, and that one is arbitrated a level up. It is kept on the measurement, not on a test"
  (`src/main.rs:claim_record`). The same admission appears in
  `tests/hooks.rs:a_second_fire_nudges_nothing`: "this test does not kill the rename ... (measured by two
  reviewers independently)".
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: this is the second level of the answer to "can two racing nags card the
  same approval twice". The per-record claim "is what stops ONE approval being counted twice when a
  second process is legitimately running, which is what happens after a crashed fire's window claim ages
  out while its records are still on disk" (`src/main.rs:claim_record`). It is explicitly NOT a duplicate
  of the window lock and neither makes the other redundant: the window lock stops two cards per window,
  this stops one record reaching two cards.
- Privacy: the claim carries the record's bytes and its 0600 mode, unchanged.
- Process ownership and cleanup: every claim the fire took is removed by that fire, on both the counted
  path (after the card) and every dropped path (immediately). A removal that fails prints
  `pns nag: the working file {path} could not be removed ({error}); it is left behind`
  (`src/main.rs:nag_mode`).
  `tests/hooks.rs:an_unanswered_approval_is_nudged_once_through_the_ordinary_paths` asserts the directory
  is completely empty afterwards: "no record claim and no fire claim outlives the fire that took them".
- Compatibility contract: the claim name is `<whole file name>.claim.<pid>` (`src/nag.rs:claim_path`,
  `src/nag.rs:CLAIM_INFIX`), which behavior 9 pins against the `with_extension` collapse.

### 14. What one claimed record is worth: unreadable, answered, stale, or counted

Given a record this process now owns

When `src/nag.rs:fate` is asked about it

Then it is counted only when nothing says otherwise, and every other answer is a drop with a named
reason.

- Success: `src/nag.rs:a_record_is_counted_only_when_nothing_says_otherwise` sweeps five rows: fresh with
  no marker is `Count`; fresh with a marker is `Drop(Answered)`; old with no marker is `Drop(Stale)`; old
  WITH a marker is `Drop(Answered)`; and `None` is `Drop(Unreadable)`.
- Failure sources: the record text failed to parse as a JSON object (`src/nag.rs:parse` returns None for
  `""`, `"not json"`, `"[1,2]"`, `"\"a string\""` and `"{"`), or the answered marker exists, or the armed
  second is outside the window.
- Fail direction: "EVERY DROP MEANS SILENCE, which is the rule the whole design falls out of: an
  unreadable, absent, ambiguous or failed input resolves to no nudge, never to a nudge taken on a guess"
  (`src/nag.rs:fate`).
- Thresholds: see behavior 15 for the staleness numbers. The ORDER of the three tests is itself the
  contract: unreadable first "because nothing else can be asked of a record that did not parse", then the
  marker, then the cap, "so an approval the operator ANSWERED is reported as answered rather than as
  merely old, which is the difference between the feature working and the machine having been asleep"
  (`src/nag.rs:fate`).
- Required side effects: none inside `fate`, which "opens no file and reads no clock, so the whole
  decision is swept in a table". The caller's side effects are: `Count` pushes onto `held`; `Unreadable`
  prints `pns nag: {path} is not a record this can read; it is dropped` on stderr and removes the claim;
  `Answered` and `Stale` remove the claim silently (`src/main.rs:nag_mode`).
- Forbidden side effects: a drop never leaves the file behind under any name. "AN ACTION THAT SUPPRESSED
  ITS OWN ERROR HAS ONLY BEEN ATTEMPTED: a file at a record's path that this could not read is somebody
  else's write, and dropping it in silence is how one would sit there being re-claimed on every fire
  forever" (`src/main.rs:nag_mode`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: the marker drop is what makes coalescing quiet at the daemon level too, see
  behavior 20.
- Privacy: an `Unreadable` drop names the file's PATH on stderr (which carries the session identifier)
  and never its contents.
- Process ownership and cleanup: covered by behavior 13.
- Compatibility contract: three drop reasons rather than one string, "because they send a reader to three
  different places: a file nobody can read is somebody else's write, an answered approval is the feature
  working, and a stale one is a machine that was asleep" (`src/nag.rs:Dropped`). A degraded record is
  tolerated rather than refused: a missing key reads as empty, and a missing `armed` reads as second
  zero, "which the staleness cap then refuses as far too old, so the degraded case resolves to silence
  rather than to a nudge about an unknown moment" (`src/nag.rs:parse`, pinned by
  `src/nag.rs:a_record_missing_a_key_degrades_to_a_thinner_one_and_a_line_that_is_not_json_is_refused`).

### 15. Staleness is bounded on both sides

Given a record whose `armed` second came off disk

When `src/nag.rs:is_stale(armed, now, after_secs)` is asked

Then it is stale if `armed > now`, or if `now > armed + 2 * after_secs`.

- Success: `src/nag.rs:a_record_is_too_old_in_both_directions_and_never_in_only_one` sweeps six rows at
  `after_secs = 300`. `tests/hooks.rs:a_stale_record_is_dropped_rather_than_nudged` drives both sides
  through a real fire: one record armed 7200 seconds ago and one armed 3600 seconds in the FUTURE, both
  dropped, zero deliveries, `pns nag: nothing is waiting` on stdout.
- Failure sources: a clock that moved backwards, a hand-edited epoch, or a machine that slept through the
  window.
- Fail direction: silence. "Past that the prompt is not news, it is history, and the card that wakes a
  laptop to describe last night is the case this exists for" (`src/nag.rs:is_stale`).
- Thresholds: with `after_secs = 300`, the cap is 600 seconds. Armed exactly at the cap (`now - 600`) is
  NOT stale; armed one second past it (`now - 601`) IS stale. In the future, `now + 1` is already stale;
  `now` exactly is not. The future half is "bug class 2: a clock that moved backwards, or a hand-edited
  epoch, would otherwise read as fresh forever and nudge on every fire until somebody deleted the file"
  (`src/nag.rs:is_stale`). The 2x factor is "one `after_secs` is the wait the operator asked for, and a
  second is the slack a busy tick or a woken laptop is allowed".
- Required side effects: none, this is a total function. The arithmetic is saturating "though
  `after_secs` is already range-bound at parse time: the bound is the config layer's and this arithmetic
  is not entitled to assume it, because a record's `armed` comes off disk".
- Forbidden side effects: none.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: Not applicable.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: this cap and the daemon's lease resolve to the same instant and are
  deliberately not merged. "ONE FUNCTION, TWO ENFORCERS, and they are not redundant. The daemon's lease
  drops the JOB, so a machine that slept through the window never spawns at all; this judges RECORDS,
  which is a different set, because a fire wakes on one approval's timer and enumerates siblings whose
  own jobs have not fired yet" (`src/nag.rs:is_stale`, restated at `src/main.rs:arm_nag`). One live
  consequence of the tightness:
  `tests/hooks.rs:three_unanswered_approvals_produce_one_card_that_says_three` deliberately plants its
  second record at 480 seconds rather than 600, because "a record armed at exactly the cap goes STALE if
  the fire's own clock read lands one second later than the fixture's, which under a loaded parallel
  suite it does".

### 16. What a nudge says: one approval names its question, several name none

Given `held` records and the oldest one's wait

When `src/nag.rs:nudge(waiting, oldest_secs, question)` builds the card's detail

Then one waiting approval reads `still waiting <waited>: <question>`, and any other count reads
`<n> approvals waiting, oldest <waited>` with no question at all.

- Success: `src/nag.rs:one_approval_is_nudged_with_its_own_question_and_how_long_it_has_waited` asserts
  `still waiting 5m: Bash: cargo test` and `still waiting 45s: Bash: cargo test`.
  `src/nag.rs:several_approvals_are_one_card_naming_the_count_and_no_question_at_all` asserts
  `3 approvals waiting, oldest 12m` and that no question text reaches it. End to end,
  `tests/hooks.rs:three_unanswered_approvals_produce_one_card_that_says_three` asserts the delivered
  detail is exactly `3 approvals waiting, oldest 8m` and that none of `cargo test`, `config.toml` or
  `main.rs` appears in it.
- Failure sources: an empty question. "AN EMPTY QUESTION ENDS THE SENTENCE rather than trailing a
  separator over nothing, which is what a record written before its detail arrived would otherwise read
  as" (`src/nag.rs:nudge`): `nudge(1, 300, "")` is `still waiting 5m`.
- Fail direction: say less rather than say wrong.
- Thresholds: the duration ladder is `src/nag.rs:waited`: under 60 seconds reads as `{n}s`, 60 up to but
  not including 3600 reads as `{n}m` (integer division), and anything from 3600 up reads as `{n}h`. So 59
  is `59s` and 60 is `1m`; 3599 is `59m` and 3600 is `1h`. Seconds are reachable in practice because the
  configuration floor is 30 (`src/nag.rs`: "the floor is thirty, so a drill really does read in
  seconds").
- Required side effects: none, a pure renderer.
- Forbidden side effects: neither shape is a question. "BOTH ARE STATEMENTS AND NEITHER IS A QUESTION. A
  nudge goes through `run_event` rather than the blocked path, so it structurally cannot carry Allow and
  Deny, and the wording must not suggest it does: moshi's own card is still the one that can be answered"
  (`src/nag.rs:nudge`). The test asserts `!said.contains('?')` for both shapes.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: Not applicable.
- Privacy: the coalesced shape names no question by ruling, not by accident: "a coalesced card that
  quoted one of the questions would imply it was THE one; the card is capped at a couple of hundred
  characters on the phone anyway, so naming one and hiding the rest is the worst of both"
  (`src/nag.rs:nudge`).
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `waited` is deliberately a second renderer rather than a reuse of
  `decision_log`'s ladder, which carries an " ago" suffix, "and sharing it would mean one caller trimming
  a suffix off the other's words". It has exactly two callers, the card and the doctor's schedule line,
  "and a second spelling is how those two come to disagree" (`src/nag.rs:waited`).

### 17. One card, whatever the count: markers first, claims after

Given a fire holding one or more counted records

When it delivers

Then it sorts oldest first, writes an answered marker for EVERY counted record, raises exactly ONE card
built from the oldest, then removes the claims and releases the fire lock.

- Success: `tests/hooks.rs:an_unanswered_approval_is_nudged_once_through_the_ordinary_paths` asserts
  exactly one delivery, `state == "blocked"`, `agent == "claude"`, `project == "dotfiles"`, a detail
  starting `still waiting ` and ending `Bash: cargo test`, `pane == "wW:p21"` ("so a banner click still
  lands on the waiting pane"), the record consumed, the marker present, and the nag directory left
  completely empty.
- Failure sources: a marker write that fails prints
  `pns nag: an answered marker could not be written ({error})` and the fire continues; a claim removal
  that fails prints the working-file line and leaves it behind.
- Fail direction: never a second card. "THE ORDER IS THE SAFE ONE AT EVERY STEP. The markers are written
  BEFORE the card and the claims removed AFTER it: a crash before the card leaves approvals marked and
  silent, a crash after it leaves claims nothing re-enumerates, and neither ordering can produce a SECOND
  card, which is the property that matters" (`src/main.rs:nag_mode`).
- Thresholds: the card's own wait is `now.saturating_sub(oldest.armed)`, rendered by `waited` (behavior
  16). The structural rate limit is one nudge card per `after_secs`, "however many approvals are waiting"
  (`src/main.rs:nag_mode`).
- Required side effects: the sort is by `armed`, "so the card is built from the approval that has waited
  longest: it is the one whose wait the multi-case names, and the one whose pane is likeliest to still be
  the one worth focusing". The markers are what silence the siblings' own daemon jobs: "without it the
  siblings would each wake a process that found nothing and said so" (`src/main.rs:nag_mode`). The count
  in the card is `held.len()`, taken from what was CLAIMED and not from the directory listing:
  `tests/hooks.rs:three_unanswered_approvals_produce_one_card_that_says_three` plants a fourth,
  already-answered record precisely so the two counts differ, "with it, a count taken off the directory
  listing says four and lies to the operator about how many questions are actually waiting".
- Forbidden side effects: no payload identity is invented. `run_event` is handed
  `&HookPayload::default()` because "one card stands for every record in `held`, so naming one of their
  sessions would be inventing an identity the card does not have" (`src/main.rs:nag_mode`).
- Timeout and cancellation: the card goes through `run_event`'s ordinary delivery plan and inherits its
  per-channel deadlines. The daemon additionally bounds the child (behavior 20).
- Idempotency and duplicates: one approval earns exactly one nudge across time, because the fire consumed
  the record. `tests/hooks.rs:a_second_fire_nudges_nothing` runs a fire, asserts one delivery, runs
  another and asserts the count is unchanged and stdout says `nothing is waiting`.
- Privacy: `PNS_SKIP_PHONE` is deliberately NOT in play. "It is set by `blocking_event` in that process
  only, and this is a different process minutes later that never inherits it, so the nudge reaches the
  phone the first card was suppressed from. That is deliberate and must not be `tidied` into the record
  by a later refactor" (`src/main.rs:nag_mode`).
- Process ownership and cleanup: `release_fire` runs on every exit path (behavior 11).
- Compatibility contract: the state word stays `blocked` (see the state-files note above), and the empty
  case prints `pns nag: nothing is waiting` while the delivered case prints
  `pns nag: {n} waiting; one card attempted`. That last word is load bearing: "ATTEMPTED, NEVER SENT.
  `run_event` answers nothing about delivery and this mode cannot know whether a single leg fired: a
  mute, a named Focus or a plan that selected nothing all mean the nudge did not happen. The drill reads
  this line, and an action reported as done when it was suppressed is bug class 19 spoken out loud"
  (`src/main.rs:nag_mode`, asserted by the one-waiting test).

### 18. A nudge is an ordinary delivery but not a new occurrence

Given `run_event` is one path shared by first deliveries, nudges and observations

When it is entered with `Attempt::Nudge`

Then the whole visible delivery machinery runs, and the contiguous tail that records an OCCURRENCE does
not.

- Success: `tests/hooks.rs:a_nudge_is_not_a_new_event` captures the activity ring, the
  missed-notification journal and the last-present marker after a real blocked hook, runs a fire, and
  asserts all three are byte-identical afterwards.
- Failure sources: none of its own; the guard is a single `if attempt != Attempt::First { return; }`
  (`src/main.rs:run_event`).
- Fail direction: a suppressed nudge is LOST rather than queued. "Muted, inside a named Focus, or planned
  to nothing means the nudge does not happen and is not journaled for replay: a `still waiting` card
  replayed hours later, about a question answered long ago, is worse than silence"
  (`src/main.rs:run_event`). The shipped configuration text tells the operator the same thing: "a nag
  held back is LOST rather than queued" (`src/config_text.rs`).
- Thresholds: Not applicable here; the delivery plan's own thresholds are the ordinary ones.
- Required side effects: what a nudge DOES get is everything an operator can see, in the code's own list:
  "the mute, the named Focus modes, the quiet window, the surface and visibility plan, fresh probes taken
  in the nudge's own process" (`src/main.rs:Attempt`). Fresh probes matter here more than anywhere else:
  the fire is a separate process minutes later, so the surface reading, the home probe and the router
  behind it are all re-taken at nudge time rather than inherited from the approval. It is also still
  recorded in the decision ring, with `nag = attempt == Attempt::Nudge` (`src/main.rs:run_event`).
- Forbidden side effects: no journal entry, no activity-ring line, no `mark_present`, no `replay_missed`,
  no pulse, no blocked-marker update, and no `record_news`, which "is what arms the unread lamp"
  (`src/main.rs:run_event`), so an unread lamp is never armed by a nudge. Each is "a defect avoided
  rather than tidiness": the recap counts activity-ring lines toward `min_events` so a nudge that rang
  would inflate the operator's own recap with pns's noise; a nudge is not evidence of presence; and "the
  pulse falling out here is how `escalation is not a colour` stays enforced without touching the lights
  at all" (`src/main.rs:run_event`).
- Timeout and cancellation: inherited from the ordinary delivery plan.
- Idempotency and duplicates: the decision ring is what makes a duplicate diagnosable.
  `tests/hooks.rs:the_decision_log_says_which_line_was_the_nudge` runs a blocked hook then a fire,
  filters the ring for `claude/blocked`, and asserts exactly two lines, the first carrying `nag=no` and
  the second `nag=yes`. Without the field "the ring holds two `claude/blocked` lines differing in nothing
  an operator can see, and `why did I get two cards for one prompt` is the exact question this log exists
  to answer".
- Privacy: the card's detail carries the question for the single-approval shape, so a nudge can put the
  permission prompt's own text on a phone that the first card's phone leg was suppressed from (behavior
  17's privacy line).
- Process ownership and cleanup: the fire process is the daemon's child (behavior 20).
- Compatibility contract: `Attempt` is one argument rather than a second event path, "A nudge is an
  ordinary event in every respect an operator can see ... what it is not is a second OCCURRENCE, and the
  contiguous tail of `run_event` is what records occurrences" (`src/main.rs:Attempt`).

### 19. The feature switched off between arming and firing drops every record

Given an operator who armed a nag and then set `after_secs = 0`, or removed the `[nag]` table, or left a
config that no longer parses

When a fire runs

Then no card is raised, every record is removed, and the fire says what it dropped.

- Success: `tests/hooks.rs:a_fire_with_the_feature_switched_off_drops_every_record_and_cards_nothing`
  asserts zero deliveries, stdout containing `the nag is off; 1 waiting approval(s) dropped`, and the
  record gone.
- Failure sources: a `remove_file` that fails is simply not counted
  (`.filter(|record| std::fs::remove_file(record).is_ok()).count()`), so the printed number is what this
  process actually removed.
- Fail direction: silence plus cleanup. "A CONFIG THAT TURNED THE FEATURE OFF BETWEEN ARMING AND FIRING
  MEANS NO NUDGE, and the records go with it: the operator cancelled the timer, and a card from it would
  be the feature ignoring them" (`src/main.rs:nag_mode`). The records go too because "one left behind is
  a card waiting to be delivered the moment they switch the feature back on, about a prompt from whenever
  it was" (that test's own comment).
- Thresholds: `NAG_OFF` is 0 (`src/main.rs:NAG_OFF`, the composition root's own spelling of the config
  default). Any value from 30 up is on; 1 through 29 cannot reach here because the config refuses them
  (behavior 1).
- Required side effects: the exact line `pns nag: the nag is off; {n} waiting approval(s) dropped` on
  stdout, exit 0.
- Forbidden side effects: no card, and the fire lock is never taken on this path at all. The off branch
  returns before `create_dir_all` and before `claim_fire` (`src/main.rs:nag_mode`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: because this path takes no lock and owns records by REMOVAL rather than by
  rename, two concurrent off-fires can each be told they removed the same record, and each would print a
  count including it. That follows directly from the invariant cited at the top (concurrent unlink
  reports success to every racer) applied to the `remove_file(...).is_ok()` filter. NOT ESTABLISHED: no
  test or comment addresses this case, and nothing downstream reads the printed count, so the observable
  cost is a duplicated number in a line, never a duplicated card.
- Privacy: Not applicable.
- Process ownership and cleanup: this is the only path that removes records without claiming them first.
- Compatibility contract: the same reading covers an unreadable config, because
  `src/main.rs:nag_after_secs` maps every non-`Loaded` outcome to `NAG_OFF` (behavior 1's fail
  direction).

### 20. How the daemon schedules it, and what it does when the marker is there

Given one leased job per approval, spooled at `daemon/nag:<session>` with
`unless_marker = Some("nag-<session>")` and `args = ["nag"]`

When the daemon's tick reaches it

Then it fires `pns nag` as a detached child of this same binary, or drops the job without spawning
anything when the marker is already there.

- Success: `tests/hooks.rs:the_daemon_really_fires_the_nag_and_really_drops_it_when_the_marker_is_there`
  runs both rows against a real `pns daemon run` at a 50 millisecond tick, with a real spool entry
  registered through the public `pns daemon schedule` command line. Unanswered: exactly one delivery
  arrives and the record is consumed. Answered: the daemon says
  `` dropped `nag:s1` because its marker was already there `` and the delivery count stays at zero, "the
  job is dropped WITHOUT spawning a nag process at all". That row "is the only place `unless_marker` is
  PROVEN rather than assumed, and it is what makes coalescing quiet: every sibling job of a coalesced
  card drops through exactly this path".
- Failure sources: the lease expired (`now > until`), the marker exists, the record is unparseable, the
  spool entry is not a regular file, or the spawn itself fails.
- Fail direction: a job is dropped, never run twice. `src/daemon.rs:decide` checks the lease first, then
  the marker, then a running child, then the due second, "an expired job is dropped as expired even when
  its marker also arrived, and a job whose answer came in is dropped without ever being described as
  waiting".
- Thresholds: `due <= now <= until` fires. Both edges are closed: "a job whose lease is exactly its due
  second still runs; one second past `until` never does" (`src/daemon.rs:decide`). For the nag,
  `until - due == after_secs`, so with the shipped 300 the window is 300 seconds wide and the job is
  dropped at second 301 past due. The spawned child's own bound is `tick * CHILD_TICKS`
  (`src/main.rs:child_bound`, which special-cases only the lights job), after which the whole process
  GROUP is killed (`src/main.rs:reap`, `src/main.rs:kill_group`).
- Required side effects: the re-arm is durable before the spawn, though the nag has `every: None` so
  `src/daemon.rs:rearm` returns None and there is nothing to re-arm. The claim on the spool entry is
  released before the spawn (`src/main.rs:fire`).
- Forbidden side effects: the daemon says nothing on a successful firing. "AND A SPAWN THAT WORKED SAYS
  NOTHING, which is the daemon's own no-chatter rule applied to the thing it actually does ... What a job
  has to say, the job says itself: its stderr is the daemon's now" (`src/main.rs:fire`). The end-to-end
  test names the consequence: "THE CARD IS THE PROBE, because a firing that WORKED says nothing."
- Timeout and cancellation: `try_wait` and never `wait`, "A blocking wait on a child that hangs holds the
  whole loop, so one wedged delivery stops every later job" (`src/main.rs:reap`). The kill is sent to the
  process group because `spawn_job` puts each job in a group of its own; killing the direct child alone
  left a delivery "MEASURED still alive 750ms past a 300ms bound" (`src/main.rs:kill_group`).
- Idempotency and duplicates: `src/daemon.rs:claim` is a rename, so "of two daemons exactly one holds the
  record and the loser reads nothing at all". A client re-registering the same id while the record was
  claimed wins, because every daemon-side write is create-if-absent (`src/daemon.rs:hand_back`). Two
  approvals armed inside one second come due in one tick and produce two spawned fires, which is exactly
  the case behavior 11's lock exists for.
- Privacy: the child is re-executed through `std::env::current_exe()` and never a stored path, "the
  record carries arguments, so nothing in the spool can name another program. Anyone who can write a 0600
  file in this directory can already run `pns`, so this is a blast-radius limit rather than a security
  boundary" (`src/main.rs:spawn_job`). The argv is exactly `["nag"]`, so nothing about the approval
  appears in `ps` or in the daemon's log.
- Process ownership and cleanup: stdin and stdout are null and stderr is INHERITED, so a fire's complaint
  reaches the daemon's own log file; the child is in its own process group "so launchd stopping the
  daemon orphans a child in flight rather than killing it mid-delivery" (`src/main.rs:spawn_job`).
- Compatibility contract: `src/daemon.rs:marker_exists` refuses a marker name that is not safe, and reads
  a marker directory that is not a directory as NO marker, so the job runs: "a marker that cannot be
  trusted cancels nothing, and the cost is one extra card rather than a cancellation somebody else's
  symlink decided". The job id prefix is `nag:` with a colon deliberately, because `session_id_is_safe`
  refuses a colon while the daemon's id rule admits one, "so a job id can never be mistaken for a session
  id" (`src/nag.rs:JOB_PREFIX`).

### 21. What the operator can see about it without waiting

Given a feature that needs three separate things before it works (an apply for the hook declaration, a
running daemon, and the config key)

When `pns doctor` runs, or `pns nag` is typed by hand

Then the schedule is stated in the same unit the card uses, and a fire can be forced without waiting out
a timer.

- Success: `src/doctor.rs:nag_line` prints `pns doctor: the nag is off (no `[nag] after_secs`)` when off,
  and `pns doctor: an unanswered approval is carded again after 5m` at 300, or `... after 30s` at 30
  (`src/doctor.rs:the_nag_line_names_the_schedule_or_says_the_feature_is_off`). The line's PLACEMENT is
  itself the contract:
  `tests/dispatch.rs:the_doctor_prints_the_pairing_section_between_its_summary_and_the_decision_section`
  asserts it sits immediately below the daemon's own line, "which is the placement that carries the one
  fact its own sentence leaves out: a nag with a dead daemon never fires, and the line above already says
  whether the daemon is up".
- Failure sources: the nag line "does not move the exit code"; it is reported state, not health (derived
  from `src/main.rs:doctor_mode`, which prints it after the graded outcomes and returns
  `pns::doctor::exit_code(&outcomes, &pairing)`).
- Fail direction: report, never fail.
- Thresholds: the doctor renders through the same `src/nag.rs:waited` the card uses, "so
  `carded again after 30s` and `still waiting 30s` are one operator reading one number twice"
  (`src/doctor.rs:nag_tests`).
- Required side effects: `pns nag` typed by hand is a full fire, which is what makes the drill forceable:
  "RUN BY THE DAEMON AND TYPEABLE BY THE OPERATOR, which is what makes the drill forceable without
  waiting out a timer" (`src/main.rs:nag_mode`). It prints one line in `recap`'s shape.
- Forbidden side effects: `pns doctor` reads no nag record and counts nothing. `src/nag.rs` has no reader
  outside `src/main.rs:nag_mode` and `src/doctor.rs:nag_line` (verified by grepping `nag::` across
  `src/`), so the doctor cannot report how many approvals are outstanding.
- Timeout and cancellation: a hand-typed fire that cannot read a clock prints
  `pns nag: this machine has no clock to measure a wait against` on stderr and exits 0: "NO CLOCK IS NO
  NUDGE. Every input this cannot read resolves to silence, and a wait nothing can measure is one of them"
  (`src/main.rs:nag_mode`).
- Idempotency and duplicates: a hand-typed fire races the daemon's on the same lock and one of them
  stands down (behavior 11).
- Privacy: Not applicable.
- Process ownership and cleanup: a hand-typed fire is an ordinary foreground process with no group
  handling of its own.
- Compatibility contract: the shipped configuration comment is the operator's own statement of every rule
  above, and is quoted here in full because it is the contract's public face: "The nag: one more card
  when an approval has been sitting unanswered. IT IS A STATEMENT AND NEVER A SECOND PROMPT, so the card
  raised when the prompt appeared is still the one carrying Allow and Deny. It needs the daemon running
  and the PostToolBatch hook entry that tells pns an approval was dealt with; without that entry the only
  clearing signal is the end of the turn. It respects every mute the first card respects, a `pns quiet`,
  a Focus, the quiet window, and a nag held back is LOST rather than queued. Several approvals waiting
  are one card rather than several, each approval is nagged at most once, and a card counts every
  approval outstanding at that moment, so a fresh one can be named early and is then done. The signal is
  the tool batch RESOLVING rather than your answer, so a tool approved at once that then runs longer than
  this is nagged about anyway; if that bites, raise the number. THIRTY SECONDS IS THE FLOOR AND AN HOUR
  THE CEILING, anything outside is refused by name; no table at all, and after_secs of zero, are the same
  statement." (`src/config_text.rs`, the `[nag]` table's `prose`).

______________________________________________________________________

## Gaps

Collected for a reader who wants to know what is stated but not pinned.

- `nag/`'s directory mode is never set explicitly and never asserted; it inherits the process umask.
- Nothing reclaims a stranded `<session>.pending.claim.<pid>` or `fire.lock.claim.<pid>`. The accepted
  risk covers only what a crash mid-fire strands.
- No test exercises `fire.lock` aging out at 60 seconds, nor the rename-based takeover of a dead lock.
- No test kills the per-record rename in `claim_record`; the code says so itself and keeps it on the
  measurement rather than on a test.
- No test drives `clear_nag` through the `stop-failure` call site.
- The duplicate-count risk on the nag-is-off path is derived from the unlink invariant, not stated or
  tested anywhere.
