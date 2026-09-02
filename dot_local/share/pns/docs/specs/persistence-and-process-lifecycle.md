# Persistent state and process lifecycle

## Scope

This specification covers everything `pns` remembers between runs and everything it starts that outlives
a function call: the state directory and how it is resolved, the mode every file in it is born with, the
publish-by-rename protocol that keeps a reader from ever seeing a half-written state file, the shared
bounded-ring append (its lock, its prune, its heal and its refusals), the claim protocols that arbitrate
between many short-lived writers over one journal, one turn marker, one return moment and one spool
record, the marker directories the lights tick is the only sweeper of, the locks (`fire.lock`,
`lights-tick.lock`, and one `.lock` per ring), and the bounded spawning and reaping of child processes.
It does NOT cover what any of that state means to a delivery decision: the surface and visibility model,
the routing plan, the recap's rendering, the lights' colour policy, the quiet window's semantics and the
nag's own card are each specified elsewhere (`presence-and-visibility.md`, `routing-and-delivery.md`,
`quiet-behavior.md`, `missed-notifications.md`, `blocking-approval.md`). Where those specifications
describe a file, this one describes the protocol that writes it. Every claim below was read out of `src/`
and `tests/` in this crate; nothing here is recollection, and a gap is written `NOT ESTABLISHED:` rather
than guessed at.

## The writers, named

There is no single writing process. Every family of state below is written by some subset of these, and
the table names which:

| Writer            | What it is                                                                                                                                                              | Lifetime                                                         |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| harness hook      | `pns hook <event>`, spawned by Claude Code or Codex per event (`src/main.rs:hook_mode`)                                                                                 | milliseconds, one event                                          |
| harness gate      | `pns gate <harness>-hook` and the bare `pns <harness>-hook` form (`src/main.rs:gate_mode`)                                                                              | milliseconds, one submission                                     |
| producer          | `pns [<producer flags>]`, the argv form the shell notifier and any external alert path use (`src/main.rs:event_mode`, `USAGE`)                                          | milliseconds, one event                                          |
| interactive shell | `dot_bashrc.tmpl`'s bash-preexec pair, writing `lights-shell/<pid>` directly with no `pns` process at all (`src/lights.rs:any_working`, `src/main.rs:LIGHTS_SHELL_DIR`) | one command                                                      |
| daemon            | `pns daemon run`, the clock under launchd (`src/main.rs:daemon_run`)                                                                                                    | long-lived                                                       |
| daemon child      | the daemon re-executing this binary with a job's argv (`src/main.rs:spawn_job`), for example the lights tick and `pns nag`                                              | bounded, see the process table                                   |
| detached recap    | `pns recap --since --until`, started by an event and never waited on (`src/main.rs:spawn_recap`)                                                                        | unbounded, see finding U1                                        |
| typed command     | `pns quiet`, `pns lights quiet`, \`pns loop begin                                                                                                                       | end`, `pns nag`, `pns doctor`, `pns setup`, `pns daemon schedule |
| external toucher  | whatever touches `phone-attention.marker`; only its mtime is read (`src/system.rs:marker_mtime_secs`)                                                                   | not this crate's                                                 |

`NOT ESTABLISHED:` which program touches `phone-attention.marker`. The crate only reads the link's own
mtime and never writes it (`src/system.rs`, the `PhoneMarkerProbe` impl); no writer of that path exists
anywhere in `src/`.

## The invariant everything rests on: a concurrent unlink does not arbitrate

The authority is the crate's own decision record,
`docs/decisions/0001-ownership-by-rename-not-by-unlink.md`, which states the measurement: "`unlink` does
not arbitrate between racing processes on this machine's filesystem, which is APFS (Apple File System).
It reports success to every caller. This was measured directly: eight racers each removed the same path
and all eight were told they had succeeded." Its rule follows: "Ownership is taken by `rename`, or by
creating a file with the exclusive-creation flag. Never by removing one, and never by reading a file and
then removing it."

The code says the same thing at six sites. Quotes are verbatim as of 2026-09-02; three of them were being
rewritten in place while this specification was written, to move the measurement into the decision record
rather than repeat it, so a quote that no longer matches means the extraction moved on and the record
above is the one to read.

- `src/main.rs:take_claim`: "THE RENAME IS THE OWNERSHIP TEST, and the remove is no longer one. It used
  to be, on the premise that only one of two runs reading a stranded claim could unlink it. MEASURED on
  macOS 26.2 (APFS), that premise is false: eight processes unlinking ONE path were every one of them
  told they had succeeded, and two racing runs that both read one claim both delivered it (reproduced
  twice in 1500 rounds). A rename does arbitrate, measured in the same run: 40 rounds of eight racers,
  one winner every time."
- `src/main.rs:claim_lock`: "THE DEAD LOCK IS TAKEN BY RENAME AND NEVER BY REMOVE, which is the one place
  arbitration is still needed on this path: a remove reports success to EVERY racer on APFS (measured,
  eight racers all told they had succeeded), so two processes clearing one dead lock would each then
  create a fresh one and both would own the window. A rename does arbitrate."
- `src/main.rs:sweep_markers`: "A REMOVAL IS OWNED BY RENAME AND NEVER READ-THEN-UNLINK. Concurrent
  unlink does not arbitrate on this filesystem (see
  `docs/decisions/0001-ownership-by-rename-not-by-unlink.md`), so a sweep that read an expired epoch and
  then unlinked could delete a FRESH marker a racing event had published in between."
- `src/main.rs:update_blocked_marker`: "Unlink cannot arbitrate on this filesystem (see
  `docs/decisions/0001-ownership-by-rename-not-by-unlink.md`), so telling the two apart would need a
  generation IN the marker and a compare-and-swap publish over it."
- `src/daemon.rs:claim`: "THE RENAME IS THE OWNERSHIP TEST, and a plain unlink is not one: measured on
  macOS 26.2 (APFS) and recorded in `take_claim`'s own doc comment, eight processes unlinking one path
  were every one of them told they had succeeded, while 40 rounds of eight racers renaming gave exactly
  one winner every time."
- `src/nag.rs:claim_path`: "THE RENAME IS THE OWNERSHIP TEST and this is the name it renames to. A plain
  unlink does not arbitrate on this filesystem, which is why the fire takes a record by rename before
  reading it for anything."

The consequence, applied throughout: a file protocol owns a file by `rename(2)` onto a name carrying
`std::process::id()`, or by an exclusive `create_new` open, and never by removing the contended path.
`src/main.rs:claim_fire` states the one place the two differ from each other: "AN EXCLUSIVE CREATE IS THE
ARBITRATION, NOT A RENAME, and the difference is measured rather than stylistic. A rename claim moves the
contended name OUT of the way ... That form delivered TWO cards from four concurrent fires, reproducibly,
under load."

## The fail-direction rule, verified

The rule as stated in the brief is that a notification must never fail the work it reports on, so the
delivery path is fail-open while state mutation is fail-closed. Verified against the code, with one
correction to the second half.

The delivery half is confirmed. `src/main.rs:deliver`: "A channel that is missing, is not executable, or
fails is not an error: it is simply not installed, or it declined, and neither may take down the siblings
or the caller." `src/system.rs:SystemCommandRunner`: "EVERY PROBE IS BOUNDED. A wedged herdr, ioreg,
pgrep or ps would otherwise hold a notification open indefinitely, and the readings all have a
fail-direction already: no answer reads as unknown, which never suppresses." Both hook and event paths
return exit 0 (`src/main.rs:hook_mode` ends `0`), and `src/main.rs:setup_mode` names that contract
explicitly: "the always-exit-0 contract permits [a non-zero exit] for the same reason `quiet` does: that
contract covers the hook and notification paths, where a non-zero exit fails the turn being reported on."

The state half needs splitting in two, because "fail-closed" means different things toward the two
parties.

Toward the EVENT, state mutation is fail-QUIET, not fail-closed: a write that could not happen never
fails, delays or decorates the notification. Every record site says so in the same words.
`src/main.rs:record_decision`: "FAIL-QUIET ... a decision that did not record is a diagnostic missing
later, on a path whose stdout is read by a harness hook". `record_missed`, `record_activity`,
`record_policy_settings_change`, `advance_marker`, `record_news`, `advance_streak`, `remember_staleness`
and `update_blocked_marker` each carry the same paragraph, and each drops its error with a comment saying
the failure is dropped here and nowhere else. Pinned by
`tests/dispatch.rs:a_state_directory_that_cannot_be_written_costs_the_event_nothing` and
`tests/dispatch.rs:a_state_directory_that_cannot_be_written_costs_a_missed_event_nothing`.

Toward the STATE ITSELF, mutation is fail-closed: a writer that cannot be sure it would not destroy or
overwrite another writer's newer state declines to write at all. `src/main.rs:claim_ring_lock` states it:
giving up "costs the ONE event that could not get in, in `record_decision`'s own fail-quiet style; it
never risks publishing over a sibling's newer state, which is the loss this lock exists to prevent."
`src/main.rs:lock_aged_out`: "A LOCK WHOSE OWN CLOCK CANNOT BE READ COUNTS AS LIVE and stands the caller
down. That is the safe direction (one window lost, never two holders)." `src/main.rs:append_ring_line`
refuses an irregular file rather than repairing it. `src/daemon.rs:hand_back` publishes create-if-absent
so the daemon can never overwrite a client.

Two deliberate exceptions to the fail-closed reading exist, and both are argued in place rather than
being drift. `src/main.rs:read_quiet_expiry`: "A FILE NOTHING CAN READ OR PARSE COMPLAINS AND READS AS
NOT MUTED, which is the OPPOSITE of the lights window's fail-closed reading and deliberately so: a window
failing closed costs one flash of a lamp, and a mute failing closed costs every notification."
`src/main.rs:lights_quiet`: "FAIL OPEN AT EVERY TURN ... because a lights mute the operator cannot see is
worse than a lamp that flashed." And one deliberate inversion the other way: `src/main.rs:pulse_mode`
"FAIL CLOSED, unlike an event", because a typo elsewhere in the config must not switch a deliberately
disabled pulse back on.

______________________________________________________________________

## Table 1: State inventory

Every path is relative to the state directory, which is `$PNS_STATE_DIR` when that is set and non-empty
and `$HOME/.local/state/pns` otherwise (`src/main.rs:state_dir`, `src/main.rs:resolve_path`). Mode `0600`
means `src/main.rs:STATE_FILE_MODE`, applied at create and re-applied on the open handle by every
publish. Directories are made with `std::fs::create_dir_all` and therefore carry the process umask; no
directory in the state tree is given an explicit mode, and the one exception in the crate is the Codex
condenser home outside this tree at `0700` (`src/main.rs:condenser_home`).

| Path                                    | Kind                    | Mode                                                        | Writers (by process)                                                                                                                        | Readers                                                                                                             | Arbitration                                                                                                                         | Retention / depth                                                                                                                              | Classification                                                                                                                      |
| --------------------------------------- | ----------------------- | ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| `decisions`                             | file                    | 0600                                                        | every event process: harness hook, producer, and the nag fire's own `run_event` (`src/main.rs:record_decision`)                             | `pns doctor` (`src/main.rs`, `DECISIONS_UNREADABLE`)                                                                | `decisions.lock`, exclusive create, held across append, read-back, prune and publish (`src/main.rs:append_ring_line`)               | `decision_log::KEPT` = 5 lines                                                                                                                 | internal persistence detail: no external reader exists in the crate and the only consumer is the crate's own `doctor` section       |
| `decisions.lock`                        | file                    | 0600                                                        | the same event processes (`src/main.rs:claim_ring_lock`)                                                                                    | nobody reads its contents; only its mtime is judged (`src/main.rs:lock_aged_out`)                                   | `create_new`; a dead one is taken by rename to `.claim.<pid>` and removed (`src/main.rs:claim_lock`)                                | released by `HeldLock::drop`; aged out at `RING_LOCK_STALE_SECS` = 5s                                                                          | temporary process coordination: it exists only for the width of one append                                                          |
| `missed-notifications`                  | file                    | 0600                                                        | every event process whose event `was_missed` (`src/main.rs:record_missed`)                                                                  | the return moment's replay, and `pns doctor`'s count                                                                | its own `.lock` for the append; the whole file is CLAIMED by rename for consumption (`src/main.rs:claim_by_rename`)                 | `missed_notifications::KEPT` = 25 entries, and consumed on replay                                                                              | internal persistence detail: written and drained entirely inside this crate                                                         |
| `missed-notifications.claim.<pid>`      | file                    | inherits                                                    | the event process that claimed it (`src/main.rs:claim_by_rename`)                                                                           | the same process, then a later run's adoption scan (`src/main.rs:stranded_claims`)                                  | rename; never renamed over an existing name of the same pid                                                                         | removed by `take_claim`; adopted by a later run when its owner is gone                                                                         | temporary process coordination                                                                                                      |
| `missed-notifications.held.<pid>.<seq>` | file                    | inherits                                                    | the process reading the batch (`src/main.rs:take_claim`)                                                                                    | that process only, until its owner exits                                                                            | rename out of the adoption prefix, so a live owner's batch cannot be taken twice (`src/main.rs:abandoned_hold`)                     | removed after a successful read; re-enters the scan once `owner_is_gone`                                                                       | temporary process coordination                                                                                                      |
| `activity`                              | file                    | 0600                                                        | every event process, delivered or not (`src/main.rs:record_activity`)                                                                       | `pns recap` (`src/main.rs`, the `ACTIVITY_READ_MAX` read)                                                           | its own `.lock`; never claimed and never consumed                                                                                   | `ACTIVITY_KEPT` = 150 entries, `ACTIVITY_MAX_CHARS` = 120 per field                                                                            | internal persistence detail: a rolling window pruned by depth alone                                                                 |
| `policy-settings-audit`                 | file                    | 0600                                                        | any event process handling a `config-change` hook with `source = policy_settings` (`src/main.rs:record_policy_settings_change`)             | `NOT ESTABLISHED:` no production reader exists in `src/`; a durable trace of receipt, read only by `tests/hooks.rs` | its own `.lock`                                                                                                                     | `POLICY_SETTINGS_AUDIT_KEPT` = 20 lines                                                                                                        | internal persistence detail                                                                                                         |
| `last-present`                          | file                    | 0600                                                        | any event process whose decision `is_present` (`src/main.rs:mark_present`, `advance_marker`)                                                | the same, and every return moment (`src/main.rs:claim_moment`)                                                      | claimed by rename to `last-present.claim.<pid>[.<epoch>]` before the read and the publish                                           | one line, one epoch; absent means no window at all                                                                                             | internal persistence detail                                                                                                         |
| `last-present.claim.<pid>[.<epoch>]`    | file                    | inherits                                                    | the process inside a return moment (`src/main.rs:claim_moment`, `window_claim_suffix`)                                                      | `src/main.rs:stranded_window_claim`                                                                                 | rename; freed by this run's own pid, by `owner_is_gone`, or by age past `STALE_WINDOW_CLAIM_SECS` = 300s                            | removed at the end of the moment; adopted otherwise                                                                                            | temporary process coordination                                                                                                      |
| `session-<id>.start`                    | file                    | umask (plain `std::fs::write`, `src/main.rs:start_of_turn`) | the `prompt` harness hook (`src/main.rs:start_of_turn`)                                                                                     | the `stop` and `stop-failure` hooks (`src/main.rs:consume_turn_marker`)                                             | created only when absent; consumed by rename to `.claim.<pid>` (`src/main.rs:consume_turn_marker`)                                  | one line, one epoch; one file per session, never swept                                                                                         | internal persistence detail; its accumulation is named and accepted in `src/main.rs:clear_nag`                                      |
| `quiet-until`                           | file                    | 0600                                                        | `pns quiet <duration>` and `pns quiet off` (`src/main.rs`, `QUIET_UNTIL`)                                                                   | every event process's override read                                                                                 | none: hand-typed, and the read-modify-write race is accepted in `src/main.rs:lights_quiet`'s sibling reasoning                      | one line, one epoch; removed by `off`                                                                                                          | public external contract: the operator types it and every process reads it                                                          |
| `home-staleness`                        | file                    | 0600                                                        | any process that took a HOME reading (`src/main.rs:remember_staleness`)                                                                     | the same (`src/main.rs:remembered_staleness`)                                                                       | none; single logical fact per machine                                                                                               | one line, the episode; removed when a HOME reading shows it resolved                                                                           | internal persistence detail                                                                                                         |
| `phone-attention.marker`                | file (may be a symlink) | not this crate's                                            | `NOT ESTABLISHED:` no writer in `src/`                                                                                                      | every event process's presence probe, mtime only, via `symlink_metadata` (`src/system.rs`)                          | none                                                                                                                                | none; only the mtime carries meaning                                                                                                           | public external contract: an outside program's touch is the whole signal                                                            |
| `daemon/`                               | directory               | umask                                                       | any process registering a job (`src/daemon.rs:schedule`), and the daemon itself putting one back (`hand_back`)                              | the daemon (`src/daemon.rs:spool_entries`)                                                                          | refused rather than repaired if something else stands there (`src/daemon.rs:prepare_spool`)                                         | one file per job id, removed when the job fires and is not re-armed                                                                            | internal persistence detail                                                                                                         |
| `daemon/<job-id>`                       | file                    | 0600                                                        | clients by rename (newest signal wins), the daemon by `hard_link` create-if-absent only (`src/daemon.rs:publish_job`, `hand_back`)          | the daemon                                                                                                          | rename to `~claim.<pid>.<seq>.<id>` before any action (`src/daemon.rs:claim`)                                                       | `RECORD_MAX` 8192 bytes, `ID_MAX` 64, `ARGS_MAX` 32, `ARGS_BYTES_MAX` 4096                                                                     | internal persistence detail                                                                                                         |
| `daemon/~claim.*`, `daemon/~pending.*`  | files                   | 0600                                                        | the claiming or staging process (`src/daemon.rs:claim`, `pending_for`)                                                                      | that process only                                                                                                   | the `~` prefix is outside the id charset, so the scan can never read one as a job (`src/daemon.rs:WORKING_PREFIX`)                  | removed on release; a survivor is named on stderr (`src/main.rs:release`)                                                                      | temporary process coordination                                                                                                      |
| `daemon-markers/`                       | directory               | umask                                                       | `src/main.rs:write_marker`, called by `clear_nag`, the nag fire, and the `resolved` arm                                                     | `src/daemon.rs:marker_exists`                                                                                       | none needed: an empty file's presence is the whole message                                                                          | one file per session that ever resolves a batch; never swept, and that accumulation is named in `src/main.rs:clear_nag`                        | internal persistence detail                                                                                                         |
| `daemon-markers/nag-<session>`          | file                    | 0600                                                        | as above (`src/nag.rs:marker_name`)                                                                                                         | the daemon's `decide`                                                                                               | none                                                                                                                                | permanent until an arm removes it                                                                                                              | internal persistence detail                                                                                                         |
| `daemon-heartbeat`                      | file                    | 0600                                                        | the daemon, once per pass (`src/daemon.rs:publish_heartbeat`)                                                                               | `pns doctor`                                                                                                        | published by rename from `~pending.<pid>.daemon-heartbeat`                                                                          | one line; stale past `HEARTBEAT_STALE_SECS` = 10 * `DEFAULT_TICK_SECS` = 10s                                                                   | internal persistence detail                                                                                                         |
| `nag/`                                  | directory               | umask                                                       | the `blocked` hook arming a nudge (`src/main.rs:arm_nag`)                                                                                   | the nag fire (`src/main.rs:nag_mode`, `record_entries`)                                                             | a subdirectory so the fire can enumerate without pattern-matching every other state file (`src/nag.rs:nag_dir`)                     | one `<session>.pending` per outstanding approval                                                                                               | internal persistence detail                                                                                                         |
| `nag/<session>.pending`                 | file                    | 0600                                                        | `src/main.rs:arm_nag` via `publish_state_line`                                                                                              | the fire, after claiming it                                                                                         | rename to `<name>.claim.<pid>` built from the WHOLE file name, never `with_extension` (`src/nag.rs:claim_path`)                     | removed when counted, dropped, or answered                                                                                                     | internal persistence detail                                                                                                         |
| `nag/fire.lock`                         | file                    | 0600                                                        | the nag fire (`src/main.rs:claim_fire`)                                                                                                     | nobody; only its mtime is judged                                                                                    | exclusive `create_new`, explicitly NOT a rename (`src/main.rs:claim_fire`)                                                          | released by `release_fire`; aged out at `nag::FIRE_STALE_SECS` = 60s                                                                           | temporary process coordination                                                                                                      |
| `lights-blocked/`                       | directory               | umask                                                       | any event process (`src/main.rs:update_blocked_marker`, `end_blocked_wait`)                                                                 | the lights tick (`src/main.rs:sweep_blocked`)                                                                       | per-file: a publish stages `<name>.new.<pid>`; the sweep takes `<name>.sweep.<pid>` by rename                                       | one file per waiting session, one epoch each; swept past the configured `give_up_after_secs`                                                   | internal persistence detail, with a documented operator check (`ls ~/.local/state/pns/lights-blocked`, `src/main.rs:sweep_markers`) |
| `lights-loop/`                          | directory               | umask                                                       | `pns loop begin` and `pns loop end` (typed), plus every event process from that pane renewing (`src/main.rs:renew_loop_lease`, `end_lease`) | the lights tick (`src/main.rs:sweep_leases`)                                                                        | as `lights-blocked/`; a renewal writes in place through the handle it found and creates nothing                                     | one file per pane, one epoch; swept past `lease_timeout_secs`                                                                                  | internal persistence detail, same documented operator check                                                                         |
| `lights-shell/`                         | directory               | umask                                                       | the interactive shell's bash-preexec pair, one file per shell pid, holding one epoch (`src/main.rs:LIGHTS_SHELL_DIR`)                       | the lights tick only (`src/main.rs:sweep_shell_markers`)                                                            | none: each shell is the only writer and the only ordinary remover of its own file                                                   | collected when the named pid is gone; an unreadable epoch is left while its shell lives                                                        | public external contract: the shell writes it and pns never does                                                                    |
| `lights-streak`                         | file                    | 0600                                                        | the lights tick (`src/main.rs:advance_streak`)                                                                                              | the same                                                                                                            | none; one writer                                                                                                                    | one line; removed when the streak ends                                                                                                         | internal persistence detail                                                                                                         |
| `lights-held`                           | file                    | 0600                                                        | the lights tick, and the event path's return (`src/main.rs:remember_held`)                                                                  | both, plus the mute                                                                                                 | `lights-tick.lock` covers the tick; the event path deliberately takes no lock and re-reads instead (`src/main.rs:LIGHTS_TICK_LOCK`) | one line of tokens; removed when nothing is held                                                                                               | internal persistence detail                                                                                                         |
| `lights-news`                           | file                    | 0600                                                        | any event process (`src/main.rs:record_news`)                                                                                               | the lights tick                                                                                                     | claimed by rename to `.claim.<pid>`, `NEWS_CLAIM_ATTEMPTS` = 2 tries, `NEWS_CLAIM_WAIT` = 2ms apart                                 | one line, two epochs; inherently capped                                                                                                        | internal persistence detail                                                                                                         |
| `lights-said`                           | file                    | 0600                                                        | the lights tick (`src/main.rs:say_lights_once`)                                                                                             | the same                                                                                                            | none; one writer                                                                                                                    | one line, the last complaint; removed when it clears                                                                                           | internal persistence detail                                                                                                         |
| `lights-quiet-said`                     | file                    | 0600                                                        | the event path (`src/main.rs:say_lights_once` with `LIGHTS_QUIET_SAID`)                                                                     | the same                                                                                                            | none                                                                                                                                | as above                                                                                                                                       | internal persistence detail                                                                                                         |
| `lights-quiet`                          | file                    | 0600                                                        | `pns lights quiet` (typed) (`src/main.rs:lights_quiet`)                                                                                     | the lights tick and the event path                                                                                  | none; the read-modify-write race is accepted because both racers are a human typing                                                 | one line per muted place                                                                                                                       | public external contract: the operator types it                                                                                     |
| `lights-tick.lock`                      | file                    | 0600                                                        | the lights tick (`src/main.rs:run_tick_writes`)                                                                                             | nobody; only its mtime is judged                                                                                    | exclusive `create_new` via `claim_lock`                                                                                             | released by `HeldLock::drop`; aged out at `lights_tick_stale_secs()` = `MAX_REFRESH_SECS` + `tick_bridge_deadline(MAX_REFRESH_SECS)` + 1 = 37s | temporary process coordination                                                                                                      |
| `<any state file>.new.<pid>`            | file                    | 0600                                                        | every publisher (`src/main.rs:publish_state_line`, `src/daemon.rs:stage`)                                                                   | nobody                                                                                                              | the name carries this process's id, so two runs publishing at once cannot share one                                                 | removed on a failed rename; a survivor is collected by the marker sweep when its run is gone                                                   | temporary process coordination                                                                                                      |
| `lights-glow`                           | file (NOT a directory)  | legacy                                                      | nothing writes it now                                                                                                                       | nothing reads it                                                                                                    | none                                                                                                                                | DELETED unconditionally on every tick (`src/main.rs:sweep_legacy_state`)                                                                       | compatibility artifact                                                                                                              |
| `lights-working-since`                  | file                    | legacy                                                      | nothing writes it now                                                                                                                       | nothing reads it                                                                                                    | none                                                                                                                                | deleted with the above                                                                                                                         | compatibility artifact                                                                                                              |
| `lights-needs/`                         | directory               | legacy                                                      | nothing writes it now                                                                                                                       | nothing reads it                                                                                                    | none                                                                                                                                | `remove_dir_all` on every tick (`src/main.rs:sweep_legacy_state`)                                                                              | compatibility artifact                                                                                                              |

### Entries on the starting list that do NOT exist

- `live/`: does not exist. `live` appears only as a fixture FILE NAME inside a `lights-loop` lease
  directory in a unit test (`src/main.rs`, the test
  `a_sweep_takes_a_marker_before_removing_it_and_leaves_no_working_file_behind`). No state path is named
  `live`.
- `expired/`: does not exist, for the same reason and in the same test.
- `answered/`: does not exist as a path. `answered` is a MARKER NAME used in `src/daemon.rs`'s own
  `spool_tests` (`markers.join("answered")`) to exercise `unless_marker`. The real answered markers are
  `daemon-markers/nag-<session>` (`src/nag.rs:marker_name`, prefix `nag-`).
- `upkeep/`: does not exist. `upkeep` is a JOB ID used in `src/daemon.rs`'s `spool_tests`, so it can only
  ever appear as a spool file name in a test sandbox. No production caller registers it.
- `lights-needs/`, `lights-glow`, `lights-working-since`: present in the code only as deletion targets.
  `lights-glow` and `lights-working-since` are FILES, not directories; only `lights-needs` is a
  directory. `src/main.rs:sweep_legacy_state` removes all three and reads none of them.

______________________________________________________________________

## Table 2: Process table

Every child this crate starts. "Group" means the child is placed in a process group of its own with
`process_group(0)` and, where it is killed, signalled by negative pid. WHICH commands may be spawned at
all is a separate, operator-approved roster recorded in
`docs/decisions/0002-what-the-binary-may-spawn.md`; this table adds what that record does not carry,
which is each spawn's deadline, termination behavior, group handling and cleanup path.

| #   | Command                                                                                                                           | Owner (spawning process)                                | Deadline                                                                                                                                                                                                        | On deadline                                                                                                   | Group killed?                                                                                                      | How its error is observed                                                                                                      | Cleanup path                                                                                         |
| --- | --------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------- |
| P1  | `current_exe()` + the job's own argv (`src/main.rs:spawn_job`)                                                                    | the daemon                                              | `child_bound(tick, id)` = `tick * CHILD_TICKS` (30 ticks), or for the `lights` job `max(tick*30, MAX_REFRESH_SECS + tick_bridge_deadline + tick)` = 37s at the production tick (`src/main.rs:child_bound`)      | `SIGKILL` to the group, then `child.kill()` on the direct child, then `wait()` (`src/main.rs:reap`)           | YES (`src/main.rs:kill_group`, negative pid, refusing pid \<= 1)                                                   | a failed spawn is printed on stderr; a running child's own stderr is INHERITED into the daemon's log (`src/main.rs:spawn_job`) | `reap` on every pass, `try_wait` and never `wait`                                                    |
| P2  | `current_exe() recap --since <n> --until <n>` (`src/main.rs:spawn_recap`)                                                         | any event process                                       | NONE in this process. The child is never waited on. `PNS_REMOTE_TIMEOUT` is set to `RECAP_DEADLINE_SECS` = 30 only when the environment named none, which bounds the child's own network legs and not the child | not applicable                                                                                                | its own group, but nothing ever kills it                                                                           | only whether the spawn succeeded, as a bool that the card reads (`src/main.rs:spawn_recap`)                                    | none. Reparented when the parent exits. **UNBOUNDED (finding U1)**                                   |
| P3  | `<channels_dir>/<leg>.sh` with the event on stdin (`src/main.rs:deliver`)                                                         | any event process                                       | NONE. `let _ = child.wait();` with no clock                                                                                                                                                                     | not applicable                                                                                                | no                                                                                                                 | never: "The exit status of a channel that DID run is still dropped"; only launch failure becomes `Delivery::Unlaunched`        | none. **UNBOUNDED (finding U2)**                                                                     |
| P4  | `moshi_hook_bin() <subcommand>` with the payload on stdin (`src/main.rs:spawn_moshi_hook`)                                        | the harness hook or gate                                | `submit_deadline()`: `PNS_MOSHI_SUBMIT_DEADLINE_MS`, else `[plugins.mobile] submit_deadline_secs`, else `DEFAULT_SUBMIT_DEADLINE_SECS` = 5s (`src/main.rs:submit_deadline`)                                     | `child.kill()` then `child.wait()`, and the call returns 0, which is no opinion (`src/main.rs:answer_within`) | NO, deliberately: "THE KILL REACHES THE DIRECT CHILD ONLY ... that day the kill has to widen to the process group" | the child's exit code becomes this process's exit code (`src/main.rs:moshi_decision`)                                          | `answer_within` reaps on the kill path; a child that finishes is reaped by `moshi_decision`'s `wait` |
| P5  | any probe: `terminal-notifier`, `/usr/sbin/ioreg`, `/usr/bin/pgrep`, `/bin/ps`, `herdr` (`src/system.rs:SystemCommandRunner`)     | any process holding a probe set, and the banner channel | `PROBE_DEADLINE` = 5s, and `PROBE_READ_MAX` = 1 MiB of stdout                                                                                                                                                   | `child.kill()` then `child.wait()`, and the runner answers `None` (`src/system.rs:run_bounded`)               | no                                                                                                                 | `None` reads as unknown, and unknown never suppresses                                                                          | `run_bounded`'s kill-and-wait on every non-answer path                                               |
| P6  | `codex exec --ephemeral --skip-git-repo-check -C <home> -s read-only -` with the prompt on stdin (`src/main.rs:condense`)         | the `stop` hook                                         | `PNS_CONDENSER_DEADLINE_MS`, else `CONDENSER_DEADLINE` = 30s; read cap `PROBE_READ_MAX` = 1 MiB                                                                                                                 | as P5                                                                                                         | no                                                                                                                 | `None` falls back to trimming the reply (`src/main.rs:condense`)                                                               | as P5                                                                                                |
| P7  | `git -C <cwd> branch --show-current` (`src/main.rs:git_branch`)                                                                   | any event process with a cwd                            | `GIT_DEADLINE` = 5s; read cap 1 MiB                                                                                                                                                                             | as P5                                                                                                         | no                                                                                                                 | `None` becomes an empty branch                                                                                                 | as P5                                                                                                |
| P8  | `moshi-hook status --json`, then `moshi-hook status` (`src/main.rs:read_pairing`)                                                 | `pns doctor`                                            | `MOSHI_JSON_DEADLINE` = 5s and `MOSHI_STATUS_DEADLINE` = 8s, run one after the other, so the worst case is 13s; read cap `PAIRING_READ_MAX` = 2 * `doctor::ANSWER_MAX` = 2 MiB                                  | as P5                                                                                                         | no                                                                                                                 | `None` on either leg becomes a "did not answer" reading in the pairing report                                                  | as P5                                                                                                |
| P9  | `gh pr list --repo <r> --state merged --search <window> --json number,title,body --limit 50` (`src/main.rs:merged_pull_requests`) | the detached recap (P2)                                 | `GH_DEADLINE` = 30s; read cap `GH_READ_MAX` = 512 KiB                                                                                                                                                           | as P5                                                                                                         | no                                                                                                                 | `None` aborts the whole fetch and the recap posts without it                                                                   | as P5                                                                                                |
| P10 | the operator's configured summarizer argv (`src/main.rs:summarize`)                                                               | the detached recap (P2)                                 | the caller's remaining episode budget; a zero budget starts NO process at all; read cap `recap::MAX_ANSWER_BYTES + 1` = 16 KiB + 1                                                                              | as P5                                                                                                         | no                                                                                                                 | `None` posts the plain list instead                                                                                            | as P5                                                                                                |

Two threads, not processes, are also started and are named here so they are not mistaken for children:
`src/system.rs:run_bounded` spawns one reader thread per bounded call (it writes stdin, drops the pipe,
and reads the capped stdout), and `src/main.rs:spawn_moshi_hook` spawns one writer thread per submission
so a child that does not read its stdin cannot block the caller.

### Unbounded spawns (findings)

- **U1: the detached recap child (P2) is unbounded and unsupervised.** `src/main.rs:spawn_recap` sets no
  deadline and never waits, and the doc comment states the choice rather than hiding it: "NEVER WAITED
  ON, so this process exits exactly when it would have"; "A CHILD THAT DIES COSTS ONE RECAP AND NOTHING
  ELSE, which is why nothing supervises it". The only bound in play is on the child's own network legs,
  and only when the environment asked for none: "AN UNBOUNDED DEADLINE IS A TERMINAL'S CHOICE, NEVER A
  BACKGROUND CHILD'S." Its process group is its own
  (`tests/dispatch.rs:the_recap_child_runs_in_a_process_group_of_its_own`), so a harness killing the hook
  by group does not take it with it, and nothing else will ever kill it.
- **U2: an executable channel (P3) is spawned and waited on with no deadline at all.**
  `src/main.rs:deliver` writes the event to stdin and then calls `let _ = child.wait();`. A channel
  script that hangs holds the whole event process for as long as it lives. Note the mitigation that
  exists and its limit: when this event process is itself a daemon child (P1) it is killed by group at
  `child_bound`, and `tests/daemon.rs:a_hung_child_does_not_stall_the_tick_and_is_killed` proves the
  grandchild dies with it. An event process started by a harness hook or by the shell has no such parent
  bound.

______________________________________________________________________

## Behaviors

### 1. The state directory is resolved from the environment and made on demand

Given a process that has something to remember

When it resolves where to put it

Then `PNS_STATE_DIR` wins when it is set and non-empty, `$HOME/.local/state/pns` is the default, and an
empty variable means the default rather than the current directory.

`src/main.rs:state_dir` and `src/main.rs:resolve_path`: "EMPTY means the default as much as unset does,
because joining a filename to an empty path resolves into the current directory and quietly delivers
nothing." The directory is never created up front; each writer creates its own parent with
`create_dir_all` immediately before it needs it (`src/main.rs:publish_state_line`,
`src/main.rs:append_ring_line`, `src/main.rs:write_marker`, `src/daemon.rs:stage`).

- Success: `tests/dispatch.rs:an_empty_channels_dir_variable_means_the_default_not_the_current_dir` pins
  the same `resolve_path` rule on its sibling variable.
- Failure sources: an unwritable `$HOME`, a symlink where the directory should be, a missing `$HOME`
  entirely (which resolves to `/.local/state/pns`).
- Fail direction: fail-quiet toward the event, fail-closed toward the state. `create_dir_all`'s error is
  dropped in the ring append and the publish, and the very next syscall fails, which each caller drops or
  reports according to its own site.
- Thresholds: not applicable.
- Required side effects: the parent directory exists before the first write into it.
- Forbidden side effects: nothing creates the directory eagerly at startup, so a process that writes
  nothing leaves no directory behind.
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: `create_dir_all` is idempotent by definition.
- Privacy: the directory itself carries the process umask. `src/main.rs:STATE_FILE_MODE` narrows the
  FILES and says so: "ONE RULE FOR THE DIRECTORY'S CONTENTS rather than a knob for one caller."
- Process ownership and cleanup: not applicable; the directory outlives every process.
- Compatibility contract: the default path is what `src/main.rs:sweep_markers` names in the operator
  check it documents, and what `src/main.rs:system_probes` builds `phone-attention.marker` from.

### 2. Every state file this tool creates is readable and writable by its owner alone

Given any file this crate creates in the state directory

When it is created

Then it is created with mode `0600`, and where the file is published by rename the mode is set on the
open HANDLE as well as at the open.

`src/main.rs:STATE_FILE_MODE` = `0o600`. `src/main.rs:publish_state_line`: "THE PENDING FILE CARRIES THE
MODE, because the rename is what publishes it"; "AND AGAIN AFTER THE OPEN, because `mode` above applies
only when the open CREATES the file ... Set on the open HANDLE rather than on the path, so nothing can be
swapped in underneath between the two." `src/main.rs:write_marker` and `src/daemon.rs:stage` repeat both
steps for the same reason.

- Success: `tests/dispatch.rs:the_journal_is_created_readable_and_writable_by_its_owner_alone`;
  `src/main.rs:tests::a_pending_file_left_behind_wide_open_is_narrowed_before_the_rename_publishes_it`.
- Failure sources: `set_permissions` failing on a handle whose file was removed underneath it.
- Fail direction: fail-closed. `publish_state_line` returns the error rather than publishing a wide-open
  file, and `write_marker` returns it too.
- Thresholds: exactly `0o600`. One bit either side is a different file: `0o400` would break the append,
  `0o644` would publish the operator's own text to every account on the machine.
- Required side effects: none beyond the mode.
- Forbidden side effects: nothing chmods a file it FOUND. `src/main.rs:STATE_FILE_MODE`: "ACCEPTED LIMIT:
  an APPEND applies it at create, so a ring an earlier build already left on disk keeps its umask mode
  until it is next created, and nothing chmods a file it found there, in keeping with the ring's
  refuse-rather-than-repair stance."
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: re-applying the mode is idempotent.
- Privacy: this IS the privacy control. The journal and the activity ring hold the operator's own event
  text, and the decision ring is printed to a terminal by `pns doctor`.
- Process ownership and cleanup: not applicable.
- Compatibility contract: an operator inspecting these files reads them as the owning user; nothing
  outside this crate is promised group or world access.

### 3. One line is published by rename, never by a truncate in place

Given a single-line state file and a reader that may look at any instant

When a writer replaces its contents

Then the bytes are written to a pending path in the SAME directory carrying this process's id, the mode
is narrowed on the handle, and the pending file is renamed over the target; a failed rename removes the
pending file.

`src/main.rs:publish_state_line`: "PUBLISHED BY RENAME ... A plain write truncates first, so a reader
landing between the truncate and the bytes sees an empty file, which every reader of these files reads as
no state at all. The pending path sits in the SAME directory, because a rename across filesystems is not
one." `src/daemon.rs:publish` states the identical rule for the daemon's own side.

- Success: `tests/dispatch.rs:a_publish_whose_rename_fails_leaves_no_pending_file_behind`;
  `src/main.rs:tests::a_pending_file_left_behind_wide_open_is_narrowed_before_the_rename_publishes_it`.
- Failure sources: an unwritable directory, a rename across a filesystem boundary, a full disk.
- Fail direction: the error is RETURNED, not swallowed: "the error is returned rather than swallowed, so
  each caller states its own fail direction: a background warning drops it, and a human waiting on a
  typed command hears about it."
  `tests/dispatch.rs:a_mute_that_could_not_be_written_exits_nonzero_and_leaves_no_state_behind` is the
  loud half.
- Thresholds: not applicable.
- Required side effects: exactly one trailing newline is written by the publish itself, which is why the
  ring's prune joins its kept lines with newlines and adds none.
- Forbidden side effects: no half-written file is left at the target, and no pending file is left in the
  directory after a failed rename.
- Timeout and cancellation: not applicable; the write is local and unbounded in neither time nor size
  because every caller writes one short line.
- Idempotency and duplicates: publishing the same line twice is indistinguishable from publishing it
  once. Two processes publishing at once cannot share a pending name because it carries the pid, but the
  LAST rename wins, which is why `advance_marker` reads and compares inside a claim before it publishes.
- Privacy: the pending file is born `0600` and narrowed again on the handle before any bytes are written.
- Process ownership and cleanup: the pending name is `path.with_extension("new.<pid>")`. Because
  `Path::with_extension` replaces everything after the LAST dot, a target name that itself contains a dot
  (a harness session id may, per `src/safety.rs:session_id_is_safe`) yields a pending stem cut at that
  dot. A run interrupted between the open and the rename leaves that file for the next run of the same
  pid to reuse, which is exactly why the mode is re-applied on the handle. In a marker directory the
  leftover is collected by `sweep_markers` once its run is gone.
- Compatibility contract: readers of these paths must treat "not found" as "no state", never as an error,
  because the target does not exist for the width of the rename (`src/main.rs:republish_after`,
  `src/main.rs:read_epoch`).

### 4. A ring append is one exclusive critical section

Given two or more processes appending to the same ring at the same moment (a Stop hook and the
long-running shell notifier are named as a normal pair)

When each of them appends

Then the append, the read-back, the prune and the publish all happen while exactly one of them holds that
ring's own `.lock`, so no racer can publish a stale, smaller window over a newer one.

`src/main.rs:append_ring_line`: "THE WHOLE OPERATION IS ONE CLAIM ... the prune's read and its publish
were NOT one atomic step, so a racer that read before a sibling's append could still publish its stale,
smaller window AFTER the sibling published a newer one, silently dropping the sibling's line and keeping
the wrong oldest entry." The lock is created before the ring so a missing state directory fails the
lock's own exclusive create rather than being papered over.

- Success: `tests/hooks.rs:two_policy_settings_changes_racing_the_prune_lose_neither_line`, driven
  deterministically by `PNS_RING_LOCK_TEST_DELAY_MS`, which is unset in every real invocation. The test's
  own note: the race "measured across three hundred concurrent real events with no help ... never once
  reproduced", which is why the hatch exists.
- Failure sources: the lock held past every attempt; a state directory that cannot be made.
- Fail direction: fail-closed toward the state and fail-quiet toward the event. The append returns
  `WouldBlock` with "the ring's lock stayed held past every attempt", and every record site drops it.
- Thresholds: `RING_LOCK_ATTEMPTS` = 200 attempts with a 1ms sleep between them, so a live holder is
  waited on for at most 199ms. At attempt 200 the append gives up and the one event's record is lost; at
  attempt 199 it sleeps once more and tries again.
- Required side effects: the lock file exists for the width of the section and is removed after it.
- Forbidden side effects: the section must not be skipped. "unlike a lights tick, which safely stands
  down from a busy window ... standing down here means silently losing whichever event is mid-append."
- Timeout and cancellation: the 199ms ceiling is the only cancellation. There is no signal handling
  inside the section.
- Idempotency and duplicates: an append is not idempotent; a caller that retried would write a second
  line. No caller retries.
- Privacy: the lock file is empty and `0600`.
- Process ownership and cleanup: `HeldLock` (behavior 6) removes it on every exit path.
- Compatibility contract: the lock's name is `<ring>.lock` (`src/main.rs:ring_lock_path`), which places
  it beside the ring in the same directory a `readable_ring` reader may scan.

### 5. A ring lock is waited for, bounded, and aged out

Given a ring lock already on disk

When another process wants it

Then it stands down for a live holder, and takes the lock only when the file's own mtime says it is older
than `RING_LOCK_STALE_SECS`, in which case the dead lock is taken BY RENAME and removed before a fresh
one is created.

`src/main.rs:claim_lock` is the one shape every lock in this binary uses: "an exclusive create arbitrates
between racers, and the age rule is what stops a holder that died from wedging the path forever."

- Success: `src/main.rs:tests::a_second_tick_stands_down_while_a_first_still_holds_the_lamps` pins the
  same `claim_lock` on `lights-tick.lock`.
- Failure sources: an unreadable lock mtime; a rename that loses to another racer clearing the same dead
  lock; a `create_new` that loses.
- Fail direction: fail-closed. `src/main.rs:lock_aged_out`: "A LOCK WHOSE OWN CLOCK CANNOT BE READ COUNTS
  AS LIVE and stands the caller down. That is the safe direction (one window lost, never two holders)."
  And `src/main.rs:claim_ring_lock`: "A CLOCK THAT CANNOT BE READ COUNTS AS ZERO ... a broken clock can
  stand this caller down but never lets it steal a live holder's claim."
- Thresholds: `RING_LOCK_STALE_SECS` = 5 seconds, compared with a STRICT greater-than
  (`now.saturating_sub(at) > stale_secs`). At exactly 5 seconds old the lock is still believed; at 6 it
  is an orphan. The siblings are `nag::FIRE_STALE_SECS` = 60 and `lights_tick_stale_secs()` = 37 at the
  production clock.
- Required side effects: a dead lock is renamed to `<lock>.claim.<pid>` and that claim removed, then a
  fresh lock is created.
- Forbidden side effects: never `remove_file` on the contended lock path. See the invariant above.
- Timeout and cancellation: the caller's own attempt budget; `claim_lock` itself does not sleep.
- Idempotency and duplicates: a second successful claim by the same process would be a second `HeldLock`
  and a double release; no caller does this.
- Privacy: not applicable; the lock is empty.
- Process ownership and cleanup: the dead-lock claim name carries `std::process::id()` and is refused
  when one is already there.
- Compatibility contract: internal.

### 6. A held lock is given back on every exit path

Given a code path that takes a lock and can leave by several routes (the lights tick stands down from
four places, a ring append from several early returns)

When any of those routes is taken

Then the lock file is removed by `Drop`, and a removal that fails is said out loud naming the path.

`src/main.rs:HeldLock`: "A GUARD RATHER THAN A RELEASE AT EVERY EXIT ... `Drop` is the one exit all of
them share. THE MESSAGE NAMES NEITHER CALLER, deliberately."

- Success: `NOT ESTABLISHED:` no test names `HeldLock` directly. The behavior is exercised indirectly by
  every passing ring test, since a leaked lock would stand the next append down for five seconds.
- Failure sources: the lock already removed by an age-out; an unwritable directory.
- Fail direction: fail-open toward the caller (the process continues) and loud toward the operator: "the
  next claimant waits it out".
- Thresholds: the wait a leak costs is the lock's own stale window: 5s for a ring, 37s for the lights
  tick.
- Required side effects: exactly one `remove_file` per acquired lock.
- Forbidden side effects: no second hand-written release beside the guard.
- Timeout and cancellation: `Drop` does not run on `SIGKILL`, which is precisely what the age rule in
  behavior 5 exists for.
- Idempotency and duplicates: `Drop` runs once per value.
- Privacy: the message names a path, never a payload.
- Process ownership and cleanup: this IS the cleanup path.
- Compatibility contract: internal.

### 7. A ring that ends mid line never fuses the next record onto the last

Given a ring whose last byte is not a newline (a truncated write, a hand edit, a backup tool)

When the next record is appended

Then a newline is written IN THE SAME WRITE as the record, so the two never interleave with a racing
append.

`src/main.rs:ends_mid_line` seeks to the end and reads the final byte on its own READ-ONLY handle: "The
end is found by seeking rather than taken from the size the caller already read: another event can append
between the two, and an offset from the stale size would sample a byte out of the middle."
`src/main.rs:append_ring_line`: "The separator rides IN the same write rather than being a write of its
own, so the record still lands in one append and two events racing each other still cannot interleave."

- Success: `tests/dispatch.rs:a_ring_that_ends_mid_line_never_fuses_the_next_record_onto_it`.
- Failure sources: a seek or read that fails; an empty file (which correctly answers `false`).
- Fail direction: fail-closed. The `?` on `ends_mid_line` aborts the append rather than writing a record
  that might fuse.
- Thresholds: exactly one byte is examined, the last. An empty file (`end == 0`) needs no separator.
- Required side effects: at most one extra newline per append.
- Forbidden side effects: no separate write of the separator, and no repair of the file's earlier
  content.
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: a second append after a clean one adds no separator.
- Privacy: not applicable.
- Process ownership and cleanup: runs inside the ring lock, so no sibling can append between the seek and
  the write.
- Compatibility contract: the ring format is one record per line, which every reader relies on
  (`src/missed_notifications.rs:entries`, `src/decision_log.rs`).

### 8. A ring is pruned on its read-back, to the caller's own depth

Given a ring that has just been appended to

When the append reads the file back

Then the last `kept` lines are published over it, and a file already at or under `kept` is left
untouched.

`src/main.rs:append_ring_line` takes `kept` and `read_max` together because they are one decision: "The
prune runs on the READ-BACK, so a ring deep enough to exceed the reader's ceiling can never be pruned
again ... Every caller states both numbers together, and the doc comment on each depth does the
arithmetic."

- Success: `tests/dispatch.rs:the_shared_append_prunes_each_ring_to_its_own_callers_depth`;
  `tests/dispatch.rs:the_ring_keeps_only_the_most_recent_decisions_with_the_oldest_gone`;
  `tests/dispatch.rs:the_journal_keeps_only_the_most_recent_misses_with_the_oldest_gone`;
  `tests/dispatch.rs:a_full_activity_ring_prunes_to_its_own_depth_instead_of_collapsing_to_one_line`.
- Failure sources: a read-back that fails, which is behavior 9.
- Fail direction: fail-quiet toward the event; the record site drops the error.
- Thresholds, with the arithmetic each constant states:
  - `decision_log::KEPT` = 5 against `RING_READ_MAX` = 262,144 bytes. At 5 entries the prune is a no-op;
    at 6 the oldest goes.
  - `missed_notifications::KEPT` = 25. Worst-case entry 7,876 bytes, full journal 196,900 bytes, 75% of
    the ceiling. "Past a depth of 33 a full journal no longer reads back at all"; 33 is the last safe
    depth and 34 is the first that collapses.
  - `ACTIVITY_KEPT` = 150 against `ACTIVITY_READ_MAX` = 1,048,576 bytes. Worst-case entry 5 * 120 * 6 +
    80 = 3,680 bytes, full ring 552,000 bytes, 53% of the ceiling.
  - `POLICY_SETTINGS_AUDIT_KEPT` = 20, worst-case line about 4.4 KB, about 88 KB full, inside 256 KiB.
    Pinned by `tests/hooks.rs:the_policy_settings_audit_trail_is_bounded_and_drops_the_oldest_entry` and
    `tests/hooks.rs:a_policy_settings_change_is_recorded_to_a_bounded_audit_trail`.
- Required side effects: the prune republishes by rename, so it carries the mode with it.
- Forbidden side effects: the prune must not run outside the lock.
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: pruning an already-pruned ring is a no-op (`entries.len() <= kept` returns
  early).
- Privacy: the journal and the activity ring hold the operator's own text; the prune moves it and never
  prints it.
- Process ownership and cleanup: the pending file carries the pid.
- Compatibility contract: raising a depth requires raising its read ceiling in the same change, which
  each constant's doc comment states.

### 9. A ring the append cannot read back heals to the line just written, unless the file vanished

Given an append whose read-back failed

When the failure is anything but `NotFound`

Then the one line just written, which is known good and known this tool's own, is republished alone over
the ring; and when the failure IS `NotFound`, nothing is done at all.

`src/main.rs:republish_after`: "EVERY REASON BUT ONE ... NotFound is the exception and the only one:
these files are removed by nothing but a claim, and a claim is a rename, so an absent path means the line
just written is already inside the claim and on its way to the operator."

- Success: `tests/dispatch.rs:a_ring_holding_bytes_that_are_not_text_heals_to_a_bounded_readable_one`;
  `tests/dispatch.rs:a_ring_too_large_to_read_back_is_replaced_rather_than_slurped`;
  `src/main.rs:tests::a_ring_that_vanished_under_the_append_is_never_republished_over`.
- Failure sources: undecodable bytes, a file past `read_max`, a path that stopped being a regular file, a
  path that was claimed mid-append.
- Fail direction: fail-closed toward duplicate delivery. Republishing over a claimed path "would put a
  second copy of an already-claimed record back at the path, and the operator would be shown it twice."
- Thresholds: `read_max` is INCLUSIVE (`found.len() > read_max` is the refusal), so a file exactly at 256
  KiB reads and one byte over does not.
- Required side effects: the ring becomes readable and bounded again.
- Forbidden side effects: the heal never tries to salvage the unreadable content.
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: the heal loses every earlier record, which is the stated price of a ring
  that could otherwise never be pruned again.
- Privacy: the republished line is this tool's own render, already capped.
- Process ownership and cleanup: the heal runs inside the ring lock.
- Compatibility contract: `src/main.rs:republish_after` exists as its own function so the distinction can
  be stated in a test; the real interleaved-claim race "belongs to the out-of-tree probe".

### 10. Every read of a state file refuses an irregular file and one over the caller's ceiling

Given a path in the state directory that an operator, a backup tool or another program can also reach

When any reader in this crate reads it

Then `symlink_metadata` judges the link itself, anything that is not a regular file is refused unopened,
a file larger than the caller's `read_max` is refused unread, and undecodable bytes are an error rather
than a lossy string.

`src/main.rs:readable_ring`: "EVERY READER OF THESE FILES GOES THROUGH IT ... A FIFO parks the open
forever, for READING as much as for writing, which wedges the hook that appended or the command a human
is waiting on. A file some other hand grew to gigabytes is otherwise learned about by allocating it." The
same refusal appears at the append (`append_ring_line`), at the spool (`src/daemon.rs:peek`,
`Peeked::Irregular`), at the marker directory (`src/daemon.rs:marker_exists`) and at the spool directory
(`src/daemon.rs:prepare_spool`).

- Success: `tests/dispatch.rs:a_fifo_at_the_rings_path_is_never_opened_and_never_parks_the_event`;
  `tests/dispatch.rs:a_fifo_at_the_journals_path_is_refused_untouched_and_never_parks_the_event`;
  `tests/dispatch.rs:a_fifo_at_the_rings_path_never_parks_the_doctor_and_is_named_by_its_kind`;
  `tests/dispatch.rs:a_directory_at_the_journals_path_is_put_back_exactly_where_it_was_found`;
  `tests/daemon.rs:an_irregular_spool_entry_is_left_alone_and_never_opened`;
  `tests/daemon.rs:a_heartbeat_that_is_not_a_regular_file_is_refused_rather_than_opened`;
  `tests/daemon.rs:a_spool_that_is_not_a_directory_refuses_the_start_and_exits_zero`.
- Failure sources: a FIFO, a directory, a symlink, a device node, a file past the ceiling, non-UTF-8
  bytes.
- Fail direction: fail-closed on the read, and REFUSED RATHER THAN REPAIRED on the write: "deleting
  something this tool did not put there, on a path it only ever appends to, is a bigger action than
  skipping one record."
- Thresholds: `RING_READ_MAX` = 262,144, `ACTIVITY_READ_MAX` = 1,048,576, `PROBE_READ_MAX` = 1,048,576,
  `PAIRING_READ_MAX` = 2,097,152, `GH_READ_MAX` = 524,288, `recap::MAX_ANSWER_BYTES` = 16,384,
  `daemon::RECORD_MAX` = 8,192, `MAX_PAYLOAD_BYTES` = 1,000,000, `TRANSCRIPT_TAIL_BYTES` = 4,000,000.
  Each is inclusive on the read side.
- Required side effects: none; a refusal touches nothing.
- Forbidden side effects: the refused path is never opened, never removed and never repaired. A directory
  found at the journal's path is renamed straight back (`src/main.rs:claim_by_rename`: "anything that is
  not a regular file goes straight back to the journal's own path, untouched and unread").
- Timeout and cancellation: the whole point of the type check is that there is no timeout to fall back
  on; a FIFO open would never return.
- Idempotency and duplicates: a refusal is idempotent.
- Privacy: refusing before opening is also what stops the crate reading through a symlink somebody else
  planted.
- Process ownership and cleanup: `src/main.rs:claim_by_rename` names the residue: "A RENAME BACK THAT
  FAILS LEAVES IT AT THE CLAIM PATH, which is a state nothing here can improve on."
- Compatibility contract: `readable_ring` returns `io::Error`s rather than an absence "so a caller that
  has to tell 'there is no file' from 'the file could not be read' still can: the doctor says a different
  sentence for each".

### 11. The decision ring carries no free text

Given a decision about an event whose agent, state, tool name, project, branch, detail and pane id all
came from outside this crate

When the line is composed

Then only the agent, the state, the permission mode, the payload agent id and the tool name carry any
text at all, each through `printable`, and every other value is a number, a boolean, an enum variant name
or a plugin name from the compiled roster.

`src/decision_log.rs:line`: "NO FREE TEXT REACHES IT. The detail, the branch, the project and the pane id
are the operator's own content, and this file is printed to a terminal by `pns doctor`, so recording them
would put that content into a state file and then onto a screen. The pane appears as the two booleans the
decision actually used it for."

- Success:
  `src/decision_log.rs:an_agent_or_state_outside_the_printable_allowlist_is_recorded_as_unprintable` and
  `src/decision_log.rs:a_payload_field_outside_the_printable_allowlist_is_recorded_as_unprintable`. The
  same defence at the audit trail's boundary is pinned by
  `tests/hooks.rs:a_newline_in_a_file_path_cannot_forge_a_policy_audit_entry`,
  `tests/hooks.rs:an_enormous_file_path_cannot_wipe_the_policy_audit_trail` and
  `tests/hooks.rs:an_arabic_letter_mark_in_a_file_path_reaches_neither_the_card_nor_the_audit_trail`.
- Failure sources: a value carrying a newline, an escape sequence, a non-ASCII byte, or more than
  `IDENTITY_MAX` characters.
- Fail direction: fail-closed. A value outside the allowlist becomes the literal `unprintable`, and "The
  line still names the decision it belonged to, which is more than dropping the entry would."
- Thresholds: the allowlist is ASCII alphanumerics plus `.`, `-` and `_`. `IDENTITY_MAX` = 32 characters,
  and "THE WHOLE VALUE IS JUDGED BEFORE ANYTHING IS TRUNCATED, which is also what makes the truncation
  safe: every accepted byte is ASCII, so a cut at `IDENTITY_MAX` can never land inside a multi-byte
  character." At 32 characters the value is kept whole; at 33 the tail is cut. An empty value is the
  literal `none` (`ABSENT`), never a blank field. An unreadable clock is the literal `-` (`NO_CLOCK`),
  never epoch zero, "which would parse cleanly and render as 56 years ago".
- Required side effects: exactly one line per event.
- Forbidden side effects: no actionId is recorded, "because pns never has one". No newline may reach the
  line, since one "FORGES a second entry that the reader cannot tell from a real decision".
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: a nudge is distinguished from a first card by the `nag` BOOLEAN, so "one
  prompt that went unanswered leaves two `claude/blocked` entries differing in nothing an operator can
  see" is fixed without adding text.
- Privacy: this behavior IS the ring's privacy rule. The related caps at the same boundary are
  `CONFIG_PATH_MAX_CHARS` = 1024 and `CONFIG_SESSION_MAX_CHARS` = 64 on the policy-settings audit trail,
  cited by `src/main.rs:config_field` as "the same defence at the same boundary, for the same reason".
- Process ownership and cleanup: not applicable.
- Compatibility contract: the format is `<epoch> <key=value ...>`, and "The only reader is the section
  below, whose whole parse is one `split_once(' ')` over the epoch".

### 12. The journal records only what the operator could not have perceived

Given an event whose plan raised neither a banner nor a phone card, on a pane nobody was watching, and
which was not skipped because another route already carried it

When the record site is reached

Then one JSON object is appended to `missed-notifications` and nothing is printed; a delivered event
writes nothing at all.

`src/main.rs:record_missed` gates on `pns::missed_notifications::was_missed`, and takes the epoch from
the decision's own clock read: "THE EPOCH IS THE DECISION'S OWN CLOCK READ, taken off the readings it
decided from rather than by a second `SystemTime` call here: two readings of one moment can disagree."

- Success: `tests/dispatch.rs:a_delivered_event_journals_nothing_at_all`;
  `tests/dispatch.rs:a_switched_off_replay_card_still_journals_the_misses_it_makes`.
- Failure sources: those of behavior 4 and 8.
- Fail direction: fail-quiet toward the event, for the reason `record_missed` states: "an event path
  whose stdout a harness hook reads must not gain a line about the state directory, and a journal entry
  that did not land costs a replay, never a card."
- Thresholds: five text fields each capped at `render::PREVIEW_MAX_CHARS`; depth 25; see behavior 8's
  arithmetic.
- Required side effects: one line, one JSON object.
- Forbidden side effects: the entry is built with `serde_json::json!` and NEVER with `format!`, "which is
  the Rust spelling of this repo's 'build JSON with `jq -n --arg`' rule: interpolation is exactly how a
  newline in a detail would forge an entry" (`src/missed_notifications.rs:entry`).
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: `record_missed` is called once per event.
- Privacy: this file DOES hold the operator's own free text, unlike the decision ring, and that is why it
  is JSON rather than `key=value` and why it is `0600`. Nothing prints an entry to a terminal;
  `pns doctor` counts it and does not show it
  (`tests/dispatch.rs:the_doctor_leaves_the_journal_exactly_as_it_found_it`).
- Process ownership and cleanup: consumed by claim, behavior 14.
- Compatibility contract: read back by key and never by position, and "A LINE THAT IS NOT A JSON OBJECT
  IS SKIPPED" (`src/missed_notifications.rs:entries`).

### 13. The activity ring records every event, whether or not anybody perceived it

Given any event at all

When the record site is reached

Then one entry in the journal's own shape is appended to `activity`, capped at `ACTIVITY_MAX_CHARS` per
field, and the file is never claimed and never consumed.

`src/main.rs:record_activity`: "NEVER CLAIMED AND NEVER CONSUMED, unlike the journal. It is a rolling
window pruned by depth alone, which is what lets the detached recap child re-read it safely and what
makes a recap idempotent by WINDOW rather than by deletion."

- Success: `tests/dispatch.rs:every_event_is_recorded_in_the_activity_ring_delivered_or_not`.
- Failure sources: as behavior 8.
- Fail direction: fail-quiet; "a missing entry costs one line of one recap."
- Thresholds: `ACTIVITY_KEPT` = 150, `ACTIVITY_MAX_CHARS` = 120. At 150 entries the prune is a no-op; at
  151 the oldest goes. The 120 is deliberately far under the card's 260 because "a recap line is one of a
  hundred".
- Required side effects: one line per event.
- Forbidden side effects: nothing consumes it, so the detached recap child (P2) can re-read it
  concurrently with the parent that spawned it. "TWO INDEPENDENT READS OF ONE RING, STATED"
  (`src/main.rs:spawn_recap`): the two counts may legitimately differ by one.
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: recap idempotence is by WINDOW (the `last-present` edge), not by deletion.
- Privacy: "THE PRIVACY RULE IS THE JOURNAL'S, INHERITED. This file holds the operator's own text for
  every event, at 0600 like every other state file, and nothing prints an entry to a terminal:
  `pns doctor` deliberately gains no activity line."
- Process ownership and cleanup: none; depth is the only bound.
- Compatibility contract: same entry shape as the journal, so one reader serves both.

### 14. The journal is claimed by rename and held outside the adoption scan

Given a journal a returning event wants to replay

When the event claims it

Then the journal is renamed to `missed-notifications.claim.<pid>`, verified AFTER the rename to be a
regular file, renamed again to `missed-notifications.held.<pid>.<seq>` (a name outside the prefix the
adoption scan matches), READ, and only then removed.

`src/main.rs:claim_by_rename`: "VERIFIED AFTER THE RENAME AND NOT BEFORE. A check taken first is a check
of a path something else is still free to change between the look and the move."
`src/main.rs:take_claim`: "THE READ STILL COMES BEFORE THE REMOVE ... Removing first, or removing
whatever the read answered, throws away a batch nobody has seen the moment the read fails: MEASURED as a
journal with one undecodable byte in it coming back empty, with the file already gone."

- Success: `tests/dispatch.rs:the_claim_never_survives_the_run_whether_the_replay_delivered_or_not`;
  `tests/dispatch.rs:a_journal_this_run_could_not_read_is_left_on_disk_rather_than_consumed`;
  `tests/dispatch.rs:a_held_batch_whose_owner_is_still_running_is_left_exactly_where_it_is`.
- Failure sources: a name already occupied by this same pid (a reused pid, in practice), a rename that
  loses, a read that fails, a remove that fails.
- Fail direction: fail-closed toward destruction. The four outcomes are a TYPE (`src/main.rs:Claimed`),
  because "This used to collapse into `Vec::new()`, and that is exactly how a journal whose read failed
  came to be deleted with nothing delivered."
- Thresholds: not applicable; the held name's sequence is a per-run `AtomicU32` starting at 0.
- Required side effects: the held name carries the pid AND a per-run sequence. A single per-process name
  "coupled every stranded claim in a run to the first one, and an UNREADABLE first claim then occupied
  the name ... and so STARVED every good batch behind it forever"
  (`tests/dispatch.rs:an_unreadable_old_claim_cannot_starve_the_good_batch_behind_it`).
- Forbidden side effects: a rename must never land on top of an existing claim or hold, because a rename
  overwrites and "a batch this run has not delivered must never be what it lands on".
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: only one rename can win, so only one process can deliver a batch.
- Privacy: the batch is the operator's own text and is never printed by the claim path.
- Process ownership and cleanup: a run killed inside the hold leaves the held file, which the adoption in
  behavior 15 recovers.
- Compatibility contract: `src/main.rs:owner_is_gone` parses "THE PID IS THE SEGMENT BEFORE THE FIRST DOT
  (held.<pid>.<seq>); a bare held.<pid> from an older build, and the marker's claim.<pid>, both parse the
  same way", which is the one backward-reading promise made here.

### 15. A stranded claim is adopted once, and only once its owner is gone

Given a claim or a hold left behind by a run that died

When a later return moment scans the state directory

Then names matching the journal's own claim prefix, plus held names whose owner has exited, are sorted
oldest first by mtime and adopted by a SECOND rename, so two runs reaching one stranded claim still
cannot both take it.

`src/main.rs:stranded_claims` matches on the exact prefix "because the turn marker claims itself in this
directory too, under its own name, and a wider match would hand a turn's start time to the replayer".
`src/main.rs:owner_is_gone` is the single liveness answer: "`kill(pid, 0)` answers `EPERM` for a process
this user may not signal, which is still a process that exists, so only `ESRCH` counts as gone."

- Success: `tests/dispatch.rs:a_claim_an_earlier_run_never_finished_is_adopted_by_the_next_return`;
  `tests/dispatch.rs:a_held_batch_whose_owner_is_gone_is_adopted_exactly_once`;
  `tests/dispatch.rs:racing_present_events_adopt_one_stranded_claim_exactly_once`;
  `tests/dispatch.rs:a_window_claim_whose_owner_is_gone_is_adopted_rather_than_lost_or_left_behind`.
- Failure sources: a pid the machine has reused, which reads as alive.
- Fail direction: fail-closed toward destruction and duplication. The reused-pid cost is named: "a batch
  that waits for the first return after the process wearing its number exits ... a replay deferred, never
  a replay destroyed and never one delivered twice."
- Thresholds: `owner_is_gone` refuses a non-positive pid outright, "because `kill()` reads non-positive
  values as the GROUP and BROADCAST forms". The WINDOW claim adds an age test that the journal's does
  not: `STALE_WINDOW_CLAIM_SECS` = 300, compared with a strict greater-than, so a claim exactly 300
  seconds old is still live and one at 301 is free. `src/main.rs:window_claim_is_free` argues the bound:
  "four orders of magnitude past what holding one costs".
- Required side effects: the adopted claim's near edge comes back with it, so the adoption is also the
  recovery.
- Forbidden side effects: no adoption may act on a claim whose owner still exists.
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: exactly one adopter per stranded claim, by rename.
- Privacy: not applicable.
- Process ownership and cleanup: this is the collection path for everything the claim protocols leave
  behind.
- Compatibility contract: sorting is by the file's own mtime, "which is the journal's own timestamp: a
  rename does not touch it"; a time that cannot be read sorts oldest, "which costs an ordering and never
  a delivery".

### 16. The return moment is owned once, and the near edge only moves forward

Given several events proving the operator is present at the same moment

When each reaches the return-moment site

Then exactly one of them renames `last-present` to its own window claim and owns the whole moment (both
the recap and the catch-up card), every other is told `Busy` and says nothing, the near edge is
republished immediately by `advance_marker`, and the edge only ever moves FORWARD.

`src/main.rs:claim_moment`: "AND ONE CLAIM OVER BOTH, taken before anything is counted ... a claim per
file MEASURED as two cards at one moment." `src/main.rs:advance_marker`: "READ, COMPARE, PUBLISH.
MEASURED as the reason: a slow event that read epoch 100 and a quick one that read 101 both publish at
the end of their own run, so the slow one used to land last and put the edge back to 100."
`src/main.rs:mark_present` takes the claim even when it wants nothing from it, because "a run that found
the moment held republished the marker here anyway, out from under the holder, and a third run then
renamed that fresh marker and became a SECOND owner alongside the first ... MEASURED at one run in sixty
with eight racers."

- Success: `tests/dispatch.rs:a_present_event_moves_the_last_present_marker_and_an_away_event_does_not`;
  `tests/dispatch.rs:the_marker_advances_so_a_second_present_event_recaps_nothing`;
  `tests/dispatch.rs:racing_present_events_adopt_one_stranded_claim_exactly_once`.
- Failure sources: a rename that loses; an unreadable epoch; no clock at all.
- Fail direction: fail-closed toward a second card. "A LIVE HOLDER IS THE ONLY THING THAT SILENCES AN
  EVENT HERE. No claim at all is a machine that has never published a marker, and that event still owes
  its catch-up card."
- Thresholds: `src/main.rs:read_epoch` refuses anything that is not a plain count: "AN UNPARSEABLE MARKER
  IS NO EDGE AT ALL, never an edge at epoch zero ... reading one as zero would recap the whole ring."
  Pinned by
  `tests/dispatch.rs:a_marker_no_reader_can_parse_opens_no_window_rather_than_one_from_epoch_zero`. The
  window's near edge is INCLUSIVE of its own second
  (`tests/dispatch.rs:events_stamped_at_the_markers_own_second_belong_to_it_and_not_to_the_window_after`).
- Required side effects: the edge is restored before the window is counted and long before anything is
  dispatched, "so the marker's absence is bounded by two renames rather than by a delivery."
- Forbidden side effects: nothing may publish `last-present` while somebody holds it; `advance_marker` is
  "CALLED ONLY FROM INSIDE A CLAIM".
- Timeout and cancellation: the 300-second age rule of behavior 15 is the only way out of a wedged claim.
- Idempotency and duplicates: the ordering (edge moved AFTER the card site) is the idempotence rule:
  "moving the edge before `replay_missed` counted the window would leave every count at one and no recap
  could ever fire."
- Privacy: not applicable; the file holds one epoch.
- Process ownership and cleanup: "a run killed mid-claim leaves ONE file that the next return adopts by
  name."
- Compatibility contract: the claim suffix is `claim.<pid>` or `claim.<pid>.<epoch>`
  (`src/main.rs:window_claim_suffix`), and both forms parse.

### 17. The turn marker is claimed by rename

Given a Stop hook measuring the turn that just finished

When it reads the start marker

Then the marker is renamed to `session-<id>.start.claim.<pid>` FIRST, read from the claim, and the claim
removed; the value is validated before it reaches arithmetic.

`src/main.rs:consume_turn_marker`: "The claim is a rename, which is atomic: two Stops racing the same
turn cannot both read it and both pulse ... Reading first and unlinking after left that window open, and
an unlink that failed left the marker wedged for every later turn." It runs FIRST in the Stop arm, before
the reply and the condenser, because "Stop is asynchronous, so the next prompt can arrive while this one
is still condensing."

- Success: `tests/hooks.rs:stopping_consumes_the_marker_so_a_second_stop_cannot_re_fire_the_tier`;
  `tests/hooks.rs:a_second_stop_cannot_re_fire_the_tier_because_the_marker_is_claimed_once`;
  `tests/hooks.rs:a_prompt_arriving_while_the_previous_stop_condenses_keeps_its_own_marker`;
  `tests/hooks.rs:a_corrupt_marker_declines_rather_than_crashing_and_is_still_consumed`;
  `tests/hooks.rs:the_first_prompt_of_a_turn_writes_a_marker_and_a_later_one_does_not_reset_it`;
  `tests/hooks.rs:a_dead_turn_consumes_the_marker_so_the_next_turn_is_not_measured_from_its_start`.
- Failure sources: a session id that cannot be a filename; no clock; a corrupt or truncated value.
- Fail direction: fail-closed. `src/main.rs:turn_marker` returns `None` for an unsafe id, and
  `src/main.rs:start_of_turn`: "NO CLOCK IS NO MARKER, never a marker at epoch zero ... a marker at zero
  would measure the turn from 1970, so `consume_turn_marker` would call a two-second turn long-running
  and it would earn the watch card and the pulse."
- Thresholds: `src/safety.rs:session_id_is_safe` is the filename gate: non-empty, no `..`, ASCII
  alphanumerics plus `.`, `_`, `-`, and nothing that `working_owner` would read as a working file.
- Required side effects: the marker is written only when absent, so "a second prompt inside one turn must
  not restart the clock".
- Forbidden side effects: the approval path leaves the turn marker alone
  (`tests/hooks.rs:an_approval_leaves_the_turn_marker_alone`,
  `tests/hooks.rs:a_refused_tool_call_leaves_the_turn_marker_alone`).
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: only one rename can win, so only one Stop measures a turn.
- Privacy: the marker holds one epoch and no text.
- Process ownership and cleanup: the claim carries the pid; a run killed between the rename and the
  remove leaves a claim that `stranded_claims` deliberately does NOT match, because its prefix is the
  journal's.
- Compatibility contract: one file per session, never swept, accumulation accepted and named in
  `src/main.rs:clear_nag`.

### 18. A marker directory is swept by the tick, by rename, never by unlink

Given a directory of one-epoch marker files (`lights-blocked/`, `lights-loop/`) and a bound

When the lights tick reads it

Then a marker still inside its bound is read and LEFT EXACTLY WHERE IT IS, and an expired one (or one
whose epoch nobody can read) is taken by rename to `<name>.sweep.<pid>`, its epoch READ AGAIN off the
claim, and removed only if it is still expired; a marker that turned out live is renamed back.

`src/main.rs:sweep_markers` is one function for two directories "because they are one mechanism twice".
Its removal rule is quoted in full in the invariant section above. "THE LIVE PATH TOUCHES NOTHING, which
is what keeps that safety free."

- Success:
  `src/main.rs:tests::a_sweep_takes_a_marker_before_removing_it_and_leaves_no_working_file_behind`
  (asserts the live marker keeps its inode and no claim is left behind);
  `src/main.rs:tests::a_lease_is_renewed_only_while_it_exists_and_swept_once_it_times_out`;
  `tests/hooks.rs:a_waiting_agent_leaves_a_marker_and_the_next_event_from_that_session_removes_it`.
- Failure sources: an unreadable directory (the whole sweep answers with no live epochs); a rename that
  loses; a put-back that fails, in which case the claim is removed.
- Fail direction: fail-closed toward destroying a live marker. "a marker that turned out to be live in
  the meantime is put back rather than destroyed."
- Thresholds: `src/lights.rs:marker_is_live` closes BOTH edges: `now.saturating_sub(at) <= max_age_secs`,
  so exactly at the bound is still live and one second past it is swept. Pinned by the three-way
  assertion in `a_lease_is_renewed_only_while_it_exists_and_swept_once_it_times_out` at `TIMEOUT`,
  `TIMEOUT` and `TIMEOUT + 1`. "A MARKER FROM THE FUTURE IS LIVE TOO, because a clock that stepped
  backwards is not a wait that ended."
- Required side effects: the sweep is the only collector of these directories, "the tick is the only
  process that ever looks in this directory, and a pane that ends without `pns loop end` leaves a file
  nothing else would remove."
- Forbidden side effects: no read-then-unlink; and no sweeping of a working file whose owner is alive
  (behavior 19).
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: sweeping twice is a no-op. "A PUT-BACK CAN OVERWRITE A NEWER PUBLISH, and
  that is the residue rather than a rule: the epoch restored is live and at most one racing publish old."
- Privacy: the file name is a session id or a pane id, and the content is one epoch.
- Process ownership and cleanup: the sweep claim carries the pid (`src/lights.rs:sweep_claim`).
- Compatibility contract: `src/main.rs:sweep_markers` documents the operator's manual check:
  `ls ~/.local/state/pns/lights-blocked ~/.local/state/pns/lights-loop` for any name whose last `.new.`
  or `.sweep.` is followed by digits alone, removed by hand.

### 19. A working file is not a marker, and is collected only when its run is gone

Given a name in a marker directory shaped `<name>.new.<pid>` or `<name>.sweep.<pid>`

When the sweep meets it

Then it is judged by `owner_is_gone` and never by the age rule: removed when its run has exited, and LEFT
ENTIRELY ALONE while its run is alive.

`src/lights.rs:working_owner` decides by the RIGHTMOST of the two suffixes, compared by offset, and
requires a positive process id after it: "A name is a working file only when what follows the LAST such
marker is a positive process id, which is a name only this crate's own writers produce."
`src/main.rs:sweep_markers`: "A publish caught between its open and its rename has no epoch in it yet,
and unlinking it there wins the race against the rename, which then publishes nothing: the wait is lost
with the agent still waiting on the operator."

- Success: `src/main.rs:tests::the_sweep_leaves_a_marker_that_is_mid_publish_alone`;
  `src/main.rs:tests::a_pending_file_whose_run_is_gone_is_collected_and_a_marker_that_spells_it_is_swept`;
  `src/main.rs:tests::a_marker_whose_shell_is_gone_is_swept_and_never_read`;
  `src/main.rs:tests::a_live_shell_whose_marker_holds_no_epoch_yet_is_left_alone`.
- Failure sources: a pane or session id that itself spells the working grammar.
- Fail direction: fail-closed. `src/safety.rs:pane_file_is_safe` and `session_id_is_safe` both refuse a
  NEW id that `working_owner` would read as a working file, so the crate's own callers can never produce
  the shape.
- Thresholds: the pid must be strictly greater than zero (`parse_count(owner)? > 0`).
- Required side effects: none on the live path.
- Forbidden side effects: "Sweeping a working file whose owner is alive is the one thing this must never
  do."
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: idempotent.
- Privacy: not applicable.
- Process ownership and cleanup: this IS the collection rule for leftovers of behaviors 3 and 18.
- Compatibility contract: a legacy marker written under a working-grammar name before the guard existed
  is a stated RESIDUAL, not a case this handles: "while the pid in the name belongs to a LIVE process it
  is never swept at all, and pid 1 is launchd, so that name in particular is permanent until the operator
  removes it. A code fix was weighed and refused."

### 20. The loop lease is renewed through the handle it found and creates nothing

Given a pane holding a loop lease

When an ordinary event from that pane arrives

Then the epoch is written IN PLACE through an open handle that states no `create`, followed by `set_len`,
so a `pns loop end` landing after the open cannot be undone by the renewal.

`src/main.rs:renew_loop_lease`: "IT CREATES NOTHING, and that is a property of the WRITE rather than of a
check in front of one ... a `pns loop end` that lands after the open sends these bytes to an inode nobody
can reach any more, where a look-then-publish would have written the lease back into existence and left
the lamp breathing for a whole timeout over work that had finished." And: "IT WRITES IN PLACE RATHER THAN
TRUNCATING FIRST, so a tick reading the file mid-renewal cannot see an empty one and sweep the lease."

- Success:
  `src/main.rs:tests::a_renewal_writes_through_the_lease_it_found_rather_than_publishing_a_new_one`;
  `tests/dispatch.rs:a_done_event_writes_the_news_record_and_renews_a_lease_its_pane_holds`.
- Failure sources: no pane in the environment; a pane id that cannot be a filename; no clock; the file
  gone.
- Fail direction: fail-quiet; "a lease that did not renew costs the lamp one timeout, and this process
  has no reader for a complaint." `pns loop end` is the opposite: LOUD, "because a human is waiting on
  the answer and the lamp is a liveness signal" (`src/main.rs:end_lease`), and a lease that is not there
  is not a failure.
- Thresholds: both epochs are ten digits "and will be for the next two centuries, so a read caught
  between the two sees a mix of two same-length numbers, which is a second or two out rather than a lease
  nobody can parse. The `set_len` after the write is for the day that stops being true."
- Required side effects: exactly one epoch and one newline.
- Forbidden side effects: no create, no truncate-then-write, no publish-by-rename here.
- Timeout and cancellation: the lease's own `lease_timeout_secs` is what expires it.
- Idempotency and duplicates: renewing twice is a no-op beyond the epoch moving forward.
- Privacy: one epoch, no text.
- Process ownership and cleanup: swept by behavior 18 when the pane stops renewing.
- Compatibility contract: the pane id is the file name, so `HERDR_PANE_ID` is what keys it
  (`src/lights.rs:lease_marker`, `NO_PANE`).

### 21. The spool protocol: a client always wins, and one occurrence runs once

Given a spool directory several clients register into and one daemon drains

When the daemon passes over it

Then a read-only peek decides one thing only (that there is nothing to do); every other verdict claims
the entry by rename FIRST and re-reads the claim; the daemon's only writes are create-if-absent
`hard_link`s, so a refresh that landed during the claim keeps its name.

`src/main.rs:drain_spool` states the three invariants verbatim: "A CLIENT ALWAYS WINS", "THE DAEMON ACTS
ONLY ON WHAT IT OWNS", "ONE OCCURRENCE RUNS ONCE". `src/daemon.rs:hand_back`: "`hard_link` fails with
`AlreadyExists` instead, so the client's record stands and the daemon's stale copy is thrown away"; and
"`hard_link` RATHER THAN `create_new`, so the file that lands is the one the temp already carries: mode,
bytes and all, published in one step."

- Success: `tests/daemon.rs:a_scheduled_job_runs_once_and_its_effect_is_observable`;
  `tests/daemon.rs:a_repeating_job_keeps_firing_until_its_lease_runs_out_then_stops`;
  `tests/daemon.rs:a_marker_on_disk_cancels_a_scheduled_job_end_to_end`;
  `tests/daemon.rs:a_hand_edited_spool_record_whose_args_fail_validation_is_dropped`;
  `tests/daemon.rs:a_registration_succeeds_with_no_daemon_anywhere_and_blocks_on_nothing`. The
  never-claim-a-wait rule is pinned by INODE in `src/main.rs`'s own daemon test module, which asserts the
  record's inode is unchanged after a pass.
- Failure sources: a claim name already occupied by this pid; a rename that loses; an unusable record; a
  record that is not a regular file.
- Fail direction: fail-closed toward running an occurrence twice, and fail-LOUD about a leak. "A CLAIM
  THAT COULD NOT BE REMOVED IS A LEAK, not a nothing: it is invisible to the scan ... One line naming the
  file is the whole remedy" (`src/main.rs:release`).
- Thresholds: `daemon::ID_MAX` = 64, `RECORD_MAX` = 8192 bytes, `ARGS_MAX` = 32, `ARGS_BYTES_MAX` = 4096,
  `EVERY_MAX_SECS` = 86,400, `DUE_WINDOW_SECS` = 30 days.
- Required side effects: THE RE-ARM IS DURABLE BEFORE THE SPAWN. "Written the other way round, a daemon
  killed between the two loses the repeat with the job already run, which is the lamp going dark on a
  loop that is still alive" (`src/main.rs:fire`).
- Forbidden side effects: the loop CANNOT overwrite a client's registration "even by mistake: the call
  that would do it is not in scope where the loop is written" (`src/daemon.rs:publish_job` is private). A
  `Wait` is never claimed. An irregular entry is left alone and never opened, and said ONCE rather than
  once a tick (`src/main.rs:drain_spool`, the `reported` set).
- Timeout and cancellation: a job is cancelled by its `unless_marker` (behavior 26's marker), by its
  lease running out, or by `pns daemon cancel`.
- Idempotency and duplicates: the id IS the filename, so re-registering is a refresh rather than a second
  job. The residual window is stated honestly: "A refresh that lands AFTER the claim is taken cannot stop
  the occurrence already claimed from running ... Nothing is LOST and nothing runs twice; the old
  occurrence simply ran."
- Privacy: "NO FREE TEXT REACHES THE SPOOL. `args` are visible in the spool file and in whatever the
  daemon logs, and the detail is the operator's own question, so it lives in the record and `pns nag`
  takes no argument" (`src/main.rs:arm_nag`).
- Process ownership and cleanup: working names carry the `~` prefix, which is outside the id charset,
  plus the pid and a per-run sequence.
- Compatibility contract: the daemon holds NO durable state of its own beyond the spool and the
  heartbeat: "Restarting re-reads the directory, which is the whole recovery path, and reboot works the
  same way because the state directory survives it."

### 22. A daemon child is bounded and killed as a group

Given a job the daemon started

When it outlives its bound

Then `SIGKILL` is sent to the whole process GROUP by negative pid, then to the direct child, then the
child is waited on so it does not become a zombie; and the loop uses `try_wait` and never `wait`.

`src/main.rs:kill_group`: "THE GROUP AND NOT THE CHILD, which is the difference between a bound and a
bound that holds ... killing the direct child alone leaves that delivery running, MEASURED still alive
750ms past a 300ms bound." `src/main.rs:reap`: "`try_wait` AND NEVER `wait`. A blocking wait on a child
that hangs holds the whole loop, so one wedged delivery stops every later job: the clock would pass every
other test here and stop in production."

- Success: `tests/daemon.rs:a_hung_child_does_not_stall_the_tick_and_is_killed` (asserts the direct child
  AND its grandchild both die, and that a second job still fires meanwhile);
  `src/main.rs:tests::a_child_outlives_the_longest_interval_plus_the_write_and_the_reap_that_follow_it`.
- Failure sources: a group that cannot be signalled at all, which is why the direct child is killed again
  afterwards.
- Fail direction: fail-closed toward accumulation. A child that cannot be killed is still dropped from
  the list, so nothing wedges the loop.
- Thresholds: `CHILD_TICKS` = 30 ticks (30 seconds at the production clock). For the `lights` job the
  bound is `max(tick*30, MAX_REFRESH_SECS + tick_bridge_deadline(MAX_REFRESH_SECS) + tick)` =
  `max(30, 30 + 6 + 1)` = 37 seconds at a one-second tick, asserted exactly by the test above. One step
  either side: at 36 seconds "a legal last write was killed before the tick could record where its breath
  had landed"; at 38 the bound is merely more generous. `kill_group` refuses any pid `<= 1`, "because
  `kill(0, ...)` signals THIS process's own group and `kill(-1, ...)` signals every process the user
  owns".
- Required side effects: `process_group(0)` at spawn, "which is the only reason ... is set in the first
  place"; and `stderr` INHERITED so a child's complaint reaches the daemon's log
  (`tests/daemon.rs:a_job_childs_own_complaint_reaches_the_daemons_log`).
- Forbidden side effects: no log line per tick and no line per firing
  (`tests/daemon.rs:the_daemon_does_not_write_a_log_line_per_tick`,
  `tests/daemon.rs:a_daemon_that_ran_a_job_says_nothing_about_having_run_it`). `stdout` is null, `stdin`
  is null.
- Timeout and cancellation: the bound is recorded as an `Instant` at spawn (`Bounded::expires_at`) and
  checked on every pass.
- Idempotency and duplicates: `decide` refuses to fire a second child of the same id while the first is
  listed, and that in-process list is explicitly NOT a lock, which is why `lights-tick.lock` exists.
- Privacy: the argv is visible in `ps`; the spool carries no free text for that reason.
- Process ownership and cleanup: `reap` is called at the top of every `daemon_pass`.
- Compatibility contract: the job re-executes `current_exe()` "AND NEVER A STORED PATH ... so nothing in
  the spool can name another program" (`src/main.rs:spawn_job`).

### 23. The detached recap child and the executable channel are spawned unbounded

Given an event that earns a recap, or a leg whose destination is an executable channel

When the child is started

Then in the recap's case nothing waits on it and nothing ever kills it, and in the channel's case the
caller waits on it with no deadline at all.

This is finding U1 and finding U2, stated in the process table. Both are DELIBERATE at their sites and
both are still unbounded spawns.

- Success: `tests/dispatch.rs:the_recap_child_runs_in_a_process_group_of_its_own` proves the group, which
  is the detachment half and not a bound.
- Failure sources: for the recap, a `current_exe()` that cannot be resolved, or a spawn that fails; for a
  channel, a missing or non-executable file.
- Fail direction: fail-open both ways. A recap spawn that failed answers `false`, and "A spawn that
  failed must never leave a card pointing at a recap nobody is writing." A channel that will not launch
  answers `Delivery::Unlaunched` and takes down neither its siblings nor the caller.
- Thresholds: the recap child gets `PNS_REMOTE_TIMEOUT` = `RECAP_DEADLINE_SECS` = "30" ONLY when the
  environment named no deadline, because "`PNS_REMOTE_TIMEOUT=0` is curl's `-m 0`, no deadline at all,
  which nobody is behind to interrupt here: a wedged gateway would keep this process alive for good, and
  every later window would add another." There is NO byte or time ceiling on the executable channel.
- Required side effects: the recap child gets `stdin`, `stdout` and `stderr` all null and its own process
  group; the channel gets the event on stdin, newline-terminated, "as the bash's `jq -cn` emitted it".
- Forbidden side effects: the recap child must NOT stay in the parent's group: "A hook the harness times
  out is killed by GROUP, and so is a shell prompt taking `SIGINT`; a child left in the parent's group
  goes with it, after the marker has already moved on, so the window can never fire again."
- Timeout and cancellation: none in either case. When the parent is itself a daemon child, finding U2 is
  bounded transitively by behavior 22; a hook-started or shell-started parent has no such bound.
- Idempotency and duplicates: the recap child re-reads the activity ring itself, so "nothing is
  serialized between them and nothing is lost if the child never starts."
- Privacy: the channel receives the fully rendered event, which is the operator's own text, over a pipe
  rather than argv.
- Process ownership and cleanup: the recap child is reparented when its parent exits; nothing reaps it.
  "A CHILD THAT DIES COSTS ONE RECAP AND NOTHING ELSE, which is why nothing supervises it."
- Compatibility contract: the channel is looked up at `<channels_dir>/<leg-name>.sh` and given one JSON
  line on stdin.

### 24. The moshi submission is bounded, and the expiry kills it

Given a blocking approval forwarded to `moshi-hook`

When the submission does not answer inside the deadline

Then the child is killed and reaped, and the call returns 0, which is no opinion and never a decision.

`src/main.rs:answer_within`: "AND EXPIRY KILLS THE SUBMISSION, WHICH IS WHAT MAKES THE BOUND REAL.
Returning is not enough on its own: the harness decides a `PermissionRequest` by READING THIS HOOK'S
STDOUT TO EOF ... MEASURED against a ten-second silent submission: a reader waiting on the process alone
0.18s, a reader waiting on stdout EOF with the child left running 10.03s, and with the kill 0.19s."

- Success: `tests/hooks.rs:a_submission_that_dies_without_answering_is_not_a_decision`;
  `tests/hooks.rs:the_gate_is_bounded_by_the_same_clock_as_the_hook`;
  `tests/hooks.rs:a_blocking_event_hands_moshi_the_payload_byte_for_byte_and_returns_its_decision`.
- Failure sources: a wedged moshi daemon; a `moshi-hook` that is not installed (which answers `None` and
  is the harness's "no opinion").
- Fail direction: fail-open toward the operator. "The harness draws the prompt and the operator answers
  at the pane." The cost is named: "the pending action dying with the child, which is a card a daemon
  wedged enough to earn this expiry had almost certainly not delivered anyway."
- Thresholds: `PNS_MOSHI_SUBMIT_DEADLINE_MS` (a literal zero here is refused and falls through), then
  `[plugins.mobile] submit_deadline_secs`, then `DEFAULT_SUBMIT_DEADLINE_SECS` = 5 seconds. The poll
  interval is `SUBMISSION_POLL_INTERVAL` = 10ms, "short enough to add no latency an operator could notice
  on a submission answered in roughly 150".
- Required side effects: the payload is written on a SEPARATE THREAD, because "A child that does not read
  its stdin blocks the writer as soon as the pipe buffer fills, and a payload larger than that buffer is
  ordinary."
- Forbidden side effects: NOT `run_bounded`. "That helper pipes the child's stdout on its way to
  attaching a deadline, and this path's whole stdout contract is that moshi's stream IS the hook's
  stream." The answered path is untouched: "no pipe, no cap, stdout still inherited".
- Timeout and cancellation: the kill reaches the DIRECT CHILD ONLY, deliberately, and the code names the
  day that has to change: "A submission that forked could leave a grandchild holding it open, and that
  day the kill has to widen to the process group."
- Idempotency and duplicates: exactly one submission per blocking event.
- Privacy: the payload crosses on stdin and never on argv.
- Process ownership and cleanup: "REAPED, not merely signalled: an unreaped child is a zombie holding its
  slot until pns exits."
- Compatibility contract: the child's exit code is passed through untouched, "because the harnesses that
  read a gate's exit code are entitled to whatever moshi said."

### 25. Every probe and every read-back spawn is bounded in time AND in bytes

Given any command this crate runs to learn something about the machine

When it is run

Then it is spawned with stdin null (or piped when there is text to send), stdout piped and capped at
`max_bytes + 1`, stderr null, the whole wait polled against one deadline, and any of a blown deadline, an
over-cap answer or a non-zero exit reads as NO ANSWER.

`src/system.rs:run_bounded`: "BOUNDED IN BYTES AS WELL AS IN TIME, which it was not ... 'bounded' meant a
child could hand back as much as it managed to write inside the window, and the caller only found out how
much AFTER it was all in memory." And: "AND PAST THE CEILING IS NO ANSWER ... A truncated answer is the
dangerous shape here: a process list cut at the ceiling has lost its last rows and a JSON listing has
stopped mid-object, and both arrive at a caller looking exactly like a complete short answer."

- Success: `src/system.rs`'s own test module drives `/bin/cat` and `/bin/sh` through `run_bounded`;
  `tests/hooks.rs:a_stuck_multiplexer_leaves_the_view_unreadable_rather_than_blocking`.
- Failure sources: a binary that is not installed, a wedged one, one that writes without end, one that
  exits non-zero, one that closes stdout and then sleeps.
- Fail direction: fail-open. "Every caller reads no answer as unknown, and unknown never suppresses."
- Thresholds: `PROBE_DEADLINE` = 5s; the reader is asked for `max_bytes.saturating_add(1)` and the result
  is kept only when `bytes.len() as u64 <= max_bytes`, so exactly at the cap is an answer and one byte
  over is not. Polling starts at `FIRST_POLL_INTERVAL` = 200 microseconds and doubles to `POLL_INTERVAL`
  = 10ms (`src/system.rs:next_poll_interval`), because "This wait begins after the child's stdout has
  already hit EOF, which is a child on its way out." The per-caller deadlines are in the process table.
- Required side effects: the stdin write happens INSIDE the window ("a child that never reads its stdin
  blocks the writer"), and dropping stdin is what gives the child its EOF.
- Forbidden side effects: the wait is polled and never blocking, because "Closed stdout is not an exited
  process: a child can close it and sleep."
- Timeout and cancellation: on any non-answer the child is killed and waited on.
- Idempotency and duplicates: probe readings are memoized per invocation on the probe set
  (`src/system.rs:SystemProbes`, the `get_or_init` cells), so one process takes each reading once.
- Privacy: the bytes travel as bytes and are converted lossily only after the size has been judged,
  "because a lossy conversion grows an invalid byte into three".
- Process ownership and cleanup: `kill` then `wait` on every failure path; the reader thread may outlive
  the call holding a closed pipe.
- Compatibility contract: absolute paths for the system binaries (`IOREG_PATH`, `PGREP_PATH`, `PS_PATH`),
  "because a probe must not resolve a system binary through a PATH it does not control";
  `terminal-notifier` and `herdr` are looked up by name deliberately.

### 26. The nag fire owns the whole window by exclusive create

Given several nag jobs waking at once over one `nag/` directory

When each tries to fire

Then exactly one creates `nag/fire.lock` with an exclusive `create_new` and owns the whole window; every
other returns having done nothing; the winner claims each record by rename, writes every answered marker
BEFORE the card, and removes the claims AFTER it.

`src/main.rs:claim_fire`: "ownership taken per record lets two woken processes each win a DISJOINT,
NON-EMPTY subset and each card its own true count, which is one card per FIRE rather than one card per
fire WINDOW ... Measured on the build before this: sixteen concurrent fires over one directory produced
sixteen cards." And: "A rename claim moves the contended name OUT of the way ... That form delivered TWO
cards from four concurrent fires, reproducibly, under load."

- Success: `tests/hooks.rs:fires_racing_over_one_directory_still_produce_exactly_one_card`;
  `tests/hooks.rs:three_unanswered_approvals_produce_one_card_that_says_three`;
  `tests/hooks.rs:a_second_fire_nudges_nothing`;
  `tests/hooks.rs:an_answered_approval_is_never_nudged_by_either_clearing_signal`;
  `tests/hooks.rs:a_clear_landing_inside_the_fires_claim_window_still_writes_the_marker`.
- Failure sources: a lock somebody live holds; a record that cannot be read; a marker that cannot be
  written.
- Fail direction: fail-closed toward a second card, and LOUD about a claim it could not give back:
  `src/main.rs:release_fire` prints "pns nag: the fire claim {} could not be given up ({error}); the next
  fire waits it out".
- Thresholds: `nag::FIRE_STALE_SECS` = 60 seconds, strict greater-than as in behavior 5. "A minute is a
  wide margin over the work the lock has to cover: the holder claims every record by rename before it
  delivers anything, so a fire that broke in later finds an empty directory in any case."
- Required side effects: the ORDER. "The markers are written BEFORE the card and the claims removed AFTER
  it: a crash before the card leaves approvals marked and silent, a crash after it leaves claims nothing
  re-enumerates, and neither ordering can produce a SECOND card."
- Forbidden side effects: the arm clears this session's previous marker BEFORE it publishes the new
  record, because "Published first, the new record can be claimed by a fire that then finds the PREVIOUS
  approval's marker still on disk and drops it as answered" (`src/main.rs:arm_nag`). The arm performs "NO
  NETWORK, NO SUBPROCESS, NO SPAWN AND NO WAIT", measured at 134.7ms +/- 14.1ms armed against 134.8ms +/-
  13.3ms unarmed over 500 runs each way.
- Timeout and cancellation: the whole feature is off when `[nag] after_secs` is absent or unreadable
  (`NAG_OFF` = 0), and a config that turned it off between arming and firing DROPS the records.
- Idempotency and duplicates: one card per window whatever the count. The claim suffix is built from the
  WHOLE file name and never `Path::with_extension`, "A harness session id may contain dots, so a claim
  derived from anything short of the full name can collapse two sessions onto one claim: one loses its
  nudge and the other can be delivered twice" (`src/nag.rs:claim_path`).
- Privacy: the operator's question lives in the record, never in the spool's argv.
- Process ownership and cleanup: the record claim carries the pid; a record whose claim survives is
  re-enumerated by the next fire.
- Compatibility contract: the record suffix is `.pending` and IS the whole enumeration test, "which is
  what keeps a claim out of the fire's enumeration: a claim is `<name>.claim.<pid>` and ends in digits,
  so it can never be read back as a record and taken a second time" (`src/nag.rs:session_of`).

### 27. Legacy lights state is deleted rather than migrated

Given a machine carrying the state the lamps kept under their old names

When the first lights tick runs after the upgrade

Then `lights-glow` and `lights-working-since` (both FILES) are removed and the `lights-needs` DIRECTORY
is removed whole, with no read of any of them, on every tick and without a marker to say it happened
once.

`src/main.rs:sweep_legacy_state`: "THE DEPLOY TRANSITION, and it is a deletion rather than a migration.
Every one of these files is derived from the machine on the next tick anyway ... THE DARK DIRECTION,
which is what makes the held record safe to drop: the old record named lamps a steady write was holding,
and the binary that wrote them is gone." And: "ONCE, WITHOUT A MARKER TO SAY SO. A removal of a name that
is not there is one failed syscall, so the deletion happens exactly once and every tick after it pays
three of those rather than a fourth state file."

- Success: `src/main.rs:tests::the_first_tick_sweeps_the_state_the_old_names_held`.
- Failure sources: an unwritable state directory. Every error is dropped.
- Fail direction: fail-quiet. A legacy file left behind is read by nothing.
- Thresholds: not applicable.
- Required side effects: three removals per tick, all of which normally fail with `NotFound`.
- Forbidden side effects: none of the three is ever READ. Keeping the old held record "would have the NEW
  tick clear lamps it never wrote by names it never chose."
- Timeout and cancellation: not applicable.
- Idempotency and duplicates: idempotent by construction.
- Privacy: not applicable.
- Process ownership and cleanup: the lights tick is the only caller (`src/main.rs:lights_tick`); a
  machine whose config has no `[lights]` table never runs it, so the legacy files stay there indefinitely
  on such a machine.
- Compatibility contract: the three string literals ARE the contract. They must not be renamed while the
  sweep is deployed, because the string in the source is the only thing naming the file to delete.
