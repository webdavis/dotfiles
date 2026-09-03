# The daemon and its jobs

## Scope

Everything `pns daemon` is: the clock (`pns daemon run`, run by the `com.webdavis.pns-daemon`
LaunchAgent), the two typed verbs beside it (`pns daemon schedule`, `pns daemon cancel`), and the whole
life of one `job` from the moment a client writes it into the `spool` to the moment the daemon claims it,
fires it, bounds the child it started, kills that child's process group and reaps it. The record format,
the validation, the `claim` and `lease` protocol, the marker cancellation, the heartbeat, the enable
switch and the shutdown behavior are all in scope, along with the three jobs the crate registers for
itself (the lights `tick`, the nag, and the room sensor's `presence` poll). Out of scope: what a fired job then does (that is the event path,
covered by `routing-and-delivery.md`), the lamp policy the lights tick applies, and the `quiet window`,
`dim window` and `quiet hours` the tick reads. Everything below is derived from the crate at
`dot_local/share/pns` and its tests only. Where the code does not settle a question the line begins
`NOT ESTABLISHED:` and names what was looked for and where.

The whole design rests on one property, stated in the module comment at `src/daemon.rs` head: the
communication between a client and the daemon is a DIRECTORY. A short-lived process registers work by
writing one file; the daemon reads the directory on its tick. There is no connection, no handshake, no
reply and nothing for a hook to wait on, so a daemon that is dead, wedged or mid-restart changes nothing
about the write.

## The jobs

| Job identifier                                                           | What schedules it                                                                                                                                                                                                                                                                                                                                                             | Lease                                                                                                                                                                                                                                                                                                                                                                                                                                     | What it runs                                                                                                                                                                                                 | Bound on the child                                                                                                                                  | Tests that pin it                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `lights` (`src/main.rs:LIGHTS_JOB`)                                      | Three callers, all through `src/main.rs:schedule_lights_tick`: every event decision on a machine with a `[lights]` table (`src/main.rs:register_lights_tick`), `pns loop begin` (`src/main.rs:loop_mode`, `LoopCommand::Begin` arm), and the tick itself when work is still in flight (`src/main.rs`, the `!active.is_empty() \|\| standing.in_flight` tail of `lights_tick`) | `until = due.max(now + lease)`. The lease is 300s for an ordinary event (`src/main.rs:ORDINARY_LEASE_SECS`), 43200s (twelve hours) for a journalled one (`src/main.rs:JOURNALLED_LEASE_SECS`), `lights.looping.lease_timeout_secs` for `pns loop begin` (default 3900, accepted range 60 to 86400, `src/config.rs:DEFAULT_LEASE_TIMEOUT_SECS`, `MIN_LEASE_TIMEOUT_SECS`, `MAX_THRESHOLD_SECS`), and 300s again for the tick's own renewal | `pns lights tick`, argv `["lights", "tick"]`, repeating at `every = lights.refresh_secs` (default 12, accepted range 10 to 30, `src/config.rs:DEFAULT_REFRESH_SECS`, `MIN_REFRESH_SECS`, `MAX_REFRESH_SECS`) | `max(tick * 30, MAX_REFRESH_SECS + tick_bridge_deadline(MAX_REFRESH_SECS) + tick)`, which is 37s at the production tick (`src/main.rs:child_bound`) | `tests/dispatch.rs:an_event_registers_the_tick_and_a_journalled_one_leases_it_for_longer`; `tests/dispatch.rs:a_tick_with_work_in_flight_keeps_itself_scheduled_past_the_loop_threshold`; `tests/dispatch.rs:a_tick_with_nothing_in_flight_lets_its_own_lease_lapse`; `tests/dispatch.rs:a_lease_taken_by_hand_schedules_the_tick_that_reads_it`; `src/main.rs:a_child_outlives_the_longest_interval_plus_the_write_and_the_reap_that_follow_it`; `src/main.rs:a_job_waits_while_its_own_child_lives_and_fires_once_that_child_has_gone` |
| `nag:<session-id>` (`src/nag.rs:job_id`, prefix `src/nag.rs:JOB_PREFIX`) | `src/main.rs:arm_nag`, on a blocked approval from the `claude` agent when `[nag] after_secs` is non-zero                                                                                                                                                                                                                                                                      | `until = due + after_secs`, where `due = now + after_secs`. One whole schedule past the due second, deliberately: `until == due` is a zero-length lease and a busy tick loses the nudge                                                                                                                                                                                                                                                   | `pns nag`, argv `["nag"]` (`src/main.rs:NAG_MODE_WORD`), one-shot (`every: None`), cancelled by `unless_marker = "nag-<session-id>"` (`src/nag.rs:marker_name`, prefix `src/nag.rs:MARKER_PREFIX`)           | `tick * 30` = 30s at the production tick (`src/main.rs:child_bound`, non-lights arm)                                                                | `tests/hooks.rs:the_daemon_really_fires_the_nag_and_really_drops_it_when_the_marker_is_there`; `tests/hooks.rs:arming_writes_a_record_registers_a_job_and_clears_a_stale_marker_first` (which asserts `id=nag:s1`, `marker=nag-s1` and `args=["nag"]` in the record)                                                                                                                                                                                                                                                                     |
| `presence` (`src/main.rs:PRESENCE_JOB`) | One caller, `src/main.rs:ensure_presence_poll`, from the `SWITCH_TICKS` block of `src/main.rs:daemon_run`: the daemon registers its own sensor, because no event asks for a room reading. `presence_settings()` returning `None` (the table absent, switched off, or refused) CANCELS it instead | `until = due.max(now + 300)` (`src/main.rs:PRESENCE_LEASE_SECS`), refreshed by every sweep while the table is on. The pending `due` is kept, so a thirty-second sweep never pushes a five-second poll away from itself | `pns presence poll`, argv `["presence", "poll"]`, repeating at `every = [plugins.presence] poll_secs` (default 5, accepted range 2 to 60, `src/config.rs:DEFAULT_PRESENCE_POLL_SECS`, `MIN_PRESENCE_POLL_SECS`, `MAX_PRESENCE_POLL_SECS`) | `tick * 30` = 30s at the production tick (`src/main.rs:child_bound`, non-lights arm), which is past the two `hue::BRIDGE_DEADLINE` calls one poll can take | `src/main.rs:an_armed_sensor_registers_the_poll_at_its_own_interval`; `src/main.rs:a_sensor_that_is_off_cancels_the_poll_it_had_registered`; `src/main.rs:a_sweep_refreshes_the_lease_without_moving_a_poll_that_is_already_due`; `src/main.rs:a_poll_publishes_the_room_it_read_as_the_line_the_sensor_parses`; `src/main.rs:a_bridge_that_did_not_answer_leaves_the_last_reading_where_it_was` |
| Any id the operator types (`pns daemon schedule --id <id>`)              | `src/main.rs:daemon_schedule` through `src/main.rs:parse_schedule`                                                                                                                                                                                                                                                                                                            | `--until <epoch>` or `--until +<secs>` as typed, else `due + 60` (`src/main.rs:DEFAULT_LEASE_SLACK_SECS`)                                                                                                                                                                                                                                                                                                                                 | Everything after `--`, re-executed as `pns <args>`                                                                                                                                                           | `tick * 30` (`src/main.rs:child_bound`, non-lights arm)                                                                                             | `tests/daemon.rs:a_scheduled_job_runs_once_and_its_effect_is_observable`; `tests/daemon.rs:a_repeating_job_keeps_firing_until_its_lease_runs_out_then_stops`; `tests/daemon.rs:a_registration_succeeds_with_no_daemon_anywhere_and_blocks_on_nothing`; `tests/daemon.rs:a_marker_on_disk_cancels_a_scheduled_job_end_to_end`                                                                                                                                                                                                             |

There is no registry of job ids. The daemon knows only what is in the spool directory, and any of the
four routes above writes the same record shape (`src/daemon.rs:Job`, `src/daemon.rs:render`). The
`lights`, `nag:<session-id>` and `presence` jobs are the only ids the crate itself ever writes.

**The open fact behind the `presence` poll.** The poll reads the bridge's `grouped_motion` roll-up,
which is the only per-room motion resource that exists on the operator's bridge: as of 2026-09-03 it
serves zero `motion_area_configuration` and zero `convenience_area_motion`, so whether a MotionAware
area's motion joins its ROOM's roll-up or arrives only as `convenience_area_motion` owned by a
`motion_area_configuration` cannot be established. It is one GET once an area exists, and the answer
is whether the area's room gains a `grouped_motion` service:

```bash
bridge="$(yq -r .bridge ~/.config/openhue/config.yaml)"
key="$(yq -r .key ~/.config/openhue/config.yaml)"
curl -sk -H "hue-application-key: $key" "https://$bridge/clip/v2/resource/room" \
  | jq -r '.data[] | [.metadata.name, ([.services[] | select(.rtype == "grouped_motion") | .rid] | join(","))] | @tsv'
```

A watched room with an empty second column reports nothing and this poll can never name it, whatever
the app shows. NOT ESTABLISHED, and deliberately not coded around: a second code path for a shape
nobody has seen is a guess.

## The processes and threads

| Process or thread                                                                                                                       | Owner                                                                                                                  | Deadline                                                                                      | Termination                                                                                                                                                                                                           | How its error is observed                                                                                                                            | Cleanup                                                                                                                     | Shutdown                                                                                                                                                                                            |
| --------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| The daemon's loop, which is the process's ONLY thread (`src/main.rs:daemon_run`)                                                        | launchd, through `com.webdavis.pns-daemon.plist.tmpl` with `RunAtLoad` true and `KeepAlive { SuccessfulExit = false }` | None. It runs until launchd stops it or the config switch turns off                           | `return 0` from inside the loop when the switch reads off; otherwise the default SIGTERM disposition                                                                                                                  | Both of its streams go to `~/.local/log/pns-daemon.log` (`StandardOutPath`, `StandardErrorPath` in the plist)                                        | Nothing to clean. It holds no durable state: restarting re-reads the spool directory (`src/main.rs:daemon_run` doc comment) | No SIGTERM handler and none needed. A loop sleeping one second dies inside the tick                                                                                                                 |
| A fired job's child process, one per fired occurrence (`src/main.rs:spawn_job`)                                                         | The daemon, recorded in a `Bounded` (`src/main.rs:Bounded`: `id`, `child`, `expires_at`)                               | `expires_at = Instant::now() + child_bound(tick, &job.id)` at spawn time (`src/main.rs:fire`) | Past the deadline: `kill_group` with SIGKILL to the negated process id, then `child.kill()`, then `child.wait()` (`src/main.rs:reap`)                                                                                 | Its stderr is INHERITED from the daemon, so a complaint it writes lands in the daemon's own log; stdin and stdout are null (`src/main.rs:spawn_job`) | `reap` drops the `Bounded` on exit, on error and on kill, so no zombie is held (`src/main.rs:reap`)                         | ORPHANED, never killed. `spawn_job` puts it in a group of its own so launchd stopping the daemon leaves a delivery mid-flight alive (`src/main.rs:spawn_job`, `src/main.rs:daemon_run` doc comment) |
| The rest of that child's process group, meaning any delivery the job spawned for itself (`process_group(0)` in `src/main.rs:spawn_job`) | The job child, not the daemon                                                                                          | The same `expires_at` as its parent, reached only through the group kill                      | SIGKILL to the whole group, which is the difference between a bound and a bound that holds: killing the direct child alone left the delivery measured still alive 750ms past a 300ms bound (`src/main.rs:kill_group`) | Whatever the job child does with it. The daemon never waits on a grandchild and never learns its exit status                                         | None of its own. The group kill is the only reach the daemon has                                                            | Orphaned with its parent. Nothing signals the group on daemon shutdown                                                                                                                              |
| The daemon child a test starts (`tests/support/mod.rs:DaemonGuard::start`)                                                              | The test binary                                                                                                        | The polling deadline in `tests/support/mod.rs:poll_until` or `DaemonGuard::exited_within`     | `Drop for DaemonGuard` calls `child.kill()` then `child.wait()` on every exit path including a panic                                                                                                                  | Both streams are redirected to one sandbox file, read back by `DaemonGuard::said`                                                                    | `Drop for Sandbox` removes the sandbox tree                                                                                 | Test-only. It never runs against the real state directory: `DaemonGuard::start` asserts the state path is inside the sandbox and is not `$HOME/.local/state/pns`                                    |

The daemon process spawns no threads at all. A grep of `src/` for `thread::spawn` finds six sites
(`src/system.rs:run_bounded`, `src/main.rs:spawn_moshi_hook`, `src/main.rs:read_payload`, and three test
servers in `src/channels/`), and not one is on the `daemon_run` path: `daemon_pass` reaps, publishes one
line and reads a directory, `daemon_enabled` and `presence_settings` read and parse a file, and
`ensure_presence_poll` reads one spool entry and writes or removes one.

**Processes left detached without a documented owner.** Two, and both are deliberate rather than
accidental, so they are named here as accepted costs rather than as defects:

1. Every live job child at the moment the daemon exits. Both exit paths leave them: the SIGTERM path (the
   process dies inside its sleep, and Rust's `Child` has no killing `Drop`) and the switched-off path
   (`return 0` from inside the loop, which drops the `children` vector without killing or waiting on
   anything). The daemon's own doc comment states the SIGTERM half ("A child mid-flight is orphaned
   rather than killed, and an orphaned nudge is at worst one extra card") and `spawn_job` states the
   design intent (the group exists so launchd stopping the daemon orphans a child rather than killing it
   mid-delivery). The switched-off path is the same behavior reached through a different door, and no
   comment names it. NOT ESTABLISHED: no test observes an orphan surviving either exit. `DaemonGuard`
   kills the daemon with SIGKILL and asserts nothing about what its children did afterwards.
1. Every member of an orphaned child's process group. The group is only ever signalled from `reap`, so
   once the daemon is gone the group is unreachable by anything in this crate.

## The tick arithmetic

| Value            | Symbol                        | Milliseconds |
| ---------------- | ----------------------------- | ------------ |
| Default          | `src/main.rs:DEFAULT_TICK_MS` | 1000         |
| Minimum accepted | `src/main.rs:MIN_TICK_MS`     | 10           |
| Maximum accepted | `src/main.rs:MAX_TICK_MS`     | 60000        |

`src/main.rs:daemon_tick` reads `PNS_DAEMON_TICK_MS`, parses it with `pns::parse_count`, and keeps the
value only when `(MIN_TICK_MS..=MAX_TICK_MS).contains(&milliseconds)`. Anything else FALLS BACK to
`DEFAULT_TICK_MS` and is never clamped towards it, because a stray `1` in a launchd environment would
spin the loop a thousand times a second and clamping would honour a value nobody meant to write.

One step either side of each edge:

- `9` is out of range, so the tick is 1000ms. `10` is in range, so the tick is 10ms.
- `60000` is in range, so the tick is 60000ms. `60001` is out of range, so the tick is 1000ms.
- `0` is out of range, so the tick is 1000ms.
- Anything `parse_count` refuses is the same fallback: an empty string, a leading `+`, a leading zero on
  a multi-digit numeral, surrounding whitespace, a non-digit byte, and any value above `i64::MAX`
  (`src/lib.rs:parse_count`, `src/lib.rs:SHELL_ARITHMETIC_MAX`).
- An unset variable is the fallback too.

Three constants scale WITH the tick rather than being stated in seconds, so one knob moves them all:

- `src/main.rs:CHILD_TICKS` is 30, so a job child's bound is `tick * 30` (30s at the production tick,
  300ms at the 10ms floor).
- `src/main.rs:SWITCH_TICKS` is 30, so the config switch is re-read every thirtieth pass (once per 30s at
  the production tick).
- The lights child's floor is the larger of `tick * 30` and one whole lights interval plus its write
  deadline plus one reap tick (`src/main.rs:child_bound`).

One constant does NOT scale with it. `src/daemon.rs:HEARTBEAT_STALE_SECS` is `10 * DEFAULT_TICK_SECS`
where `src/daemon.rs:DEFAULT_TICK_SECS` is a PRIVATE `1`, a second copy of the production tick that
nothing checks against `DEFAULT_TICK_MS`. The consequence is real: a daemon deliberately run above a
10000ms tick beats less often than the doctor's staleness window, so a live daemon reads as
`so it is not running`. NOT ESTABLISHED: no test covers a tick above the staleness window, and no
assertion ties `DEFAULT_TICK_SECS` to `DEFAULT_TICK_MS`.

The tick is read ONCE, before the loop (`src/main.rs:daemon_run`), so changing the environment variable
needs a restart.

## Behaviors

### 1. `pns daemon` serves three verbs and refuses everything else

Given the operator types `pns daemon <word>`

When `<word>` is not `run`, `schedule` or `cancel`

Then the usage text goes to stderr and the process exits 2

- Success: `src/main.rs:daemon_mode` matches the three verbs and every other word falls to the arm that
  prints `DAEMON_USAGE` and returns 2. The verb comes from `src/main.rs:second_argument`, which is
  `args_os().nth(2)` lossily converted, so a bare `pns daemon` presents an empty verb and is refused like
  any other unknown word.
- Failure sources: none of its own. It reads argv and branches.
- Fail direction: LOUD and non-zero, per the house rule that an unknown argument never falls through to
  help with exit 0. A verb this does not serve is a command the operator believes ran
  (`src/main.rs:daemon_mode` doc comment).
- Thresholds: Not applicable. No number is compared.
- Required side effects: exactly one line on stderr, verbatim:
  `pns: usage: pns daemon run | pns daemon schedule --id <id> [--in <secs>] [--every <secs>] [--until +<secs>|<epoch>] [--unless-marker <name>] -- <event args> | pns daemon cancel --id <id>`
- Forbidden side effects: nothing is written, no config is read, no clock is read, and nothing is
  spawned.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: Not applicable. The refusal has no state.
- Privacy: the rejected word is NOT echoed. The usage text is fixed and carries nothing the operator
  typed.
- Process ownership and cleanup: no process is created.
- Compatibility contract: `pns daemon --help` and `pns daemon -h` are unknown verbs and get the same exit
  2, not help. The daemon has no help arm of its own; the binary's `--help` is answered only when argv
  reaches the producer parser (`src/main.rs:USAGE`, `src/main.rs:is_producer_argv`), which a leading
  `daemon` never does.

### 2. `pns daemon run` refuses a trailing word

Given the operator types `pns daemon run <anything>`

When the fourth argv word is present

Then the usage text goes to stderr and the process exits 2 without starting a clock

- Success: `src/main.rs:daemon_run` opens with `if std::env::args_os().nth(3).is_some()`, which is the
  word after `run`.
- Failure sources: none. The check reads argv.
- Fail direction: loud and non-zero, before the config is read, before the spool is touched and before
  the loop starts.
- Thresholds: Not applicable.
- Required side effects: `DAEMON_USAGE` on stderr, exit 2.
- Forbidden side effects: no heartbeat is published, no spool directory is created, and the loop is not
  entered.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: Not applicable.
- Privacy: the trailing word is not echoed.
- Process ownership and cleanup: no process is created.
- Compatibility contract: the plist passes exactly `daemon run` and nothing else
  (`Library/LaunchAgents/com.webdavis.pns-daemon.plist.tmpl`, `ProgramArguments`), so a third word can
  only arrive by hand.

### 3. The clock will not start while the config switch is off, and exits 0 so it stays down

Given `[daemon] enabled = false` in the config

When `pns daemon run` starts

Then it prints one line and exits 0

- Success: `src/main.rs:daemon_run` calls `src/main.rs:daemon_enabled` before anything else and returns 0
  on false, after printing to stdout, verbatim: `pns daemon: disabled in the config; exiting`.
- Failure sources: a config that will not parse; a config that is missing; an unreadable `HOME`.
- Fail direction: ON. `src/main.rs:daemon_enabled` answers true for `LoadOutcome::Missing` and true for a
  parse error, and the parse error also prints to stderr:
  `pns daemon: the config could not be read (<detail>); carrying on enabled`. A file that will not parse
  must not silently stop a service the operator enabled. The default is also on with no `[daemon]` table
  at all (`src/config.rs:DEFAULT_DAEMON_ENABLED`, true, and the reasoning in the `Config::daemon_enabled`
  doc comment: this switch delivers nothing, an idle daemon reads one empty directory a second, and
  default-off would put every clock-riding feature behind two switches).
- Thresholds: Not applicable. The key is a boolean; `[daemon] enabled` with a non-boolean value is a
  config error, and any other key inside `[daemon]` is refused by name (`src/config.rs:parse_daemon`).
- Required side effects: exactly ONE line, once, on the path that exits.
  `KeepAlive { SuccessfulExit = false }` is what keeps a clean exit 0 exited, so the line is written at
  most once per bootstrap rather than once per throttle window.
- Forbidden side effects: no heartbeat, no spool preparation, no job runs.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: launchd will not relaunch a clean exit, so the line does not repeat.
- Privacy: the config path is not printed on this arm. The parse-error arm prints the error's detail,
  which is a config error message and not a config value.
- Process ownership and cleanup: no children exist yet.
- Compatibility contract: exit 0 is load bearing. A non-zero exit under
  `KeepAlive { SuccessfulExit = false }` and `ThrottleInterval 10` is roughly 8640 relaunches a day,
  which is the atuin restart-loop bug reproduced deliberately (the reasoning is in the plist comment and
  in `src/main.rs:daemon_run`).

### 4. A spool that is not a directory refuses the start permanently, and still exits 0

Given something that is not a directory sits at `<state>/daemon`

When `pns daemon run` prepares the spool

Then it prints the refusal on stderr and exits 0 without ticking

- Success: `src/daemon.rs:prepare_spool` takes `symlink_metadata` FIRST, because `create_dir_all` follows
  a symlink and a link where the spool should be would silently put every job somewhere this tool did not
  choose. A non-directory answers `Startup::Refused` with, verbatim,
  `<path> is not a directory; refusing to start`, and `src/main.rs:daemon_run` prints it as
  `pns daemon: <refusal>` and returns 0. Pinned by
  `src/daemon.rs:a_spool_path_that_is_not_a_directory_is_a_permanent_refusal` (with an unmutated control
  proving an ABSENT spool is made rather than refused) and end to end by
  `tests/daemon.rs:a_spool_that_is_not_a_directory_refuses_the_start_and_exits_zero`.
- Failure sources: a symlink, a regular file, a named pipe or a device at the spool path; a state
  directory that will not take a new directory.
- Fail direction: refuse rather than repair, and exit 0 rather than crash-loop. `Startup` is a type
  rather than a bool precisely because EVERY refusal here is permanent: relaunching cannot turn a symlink
  into a directory or make an unwritable state directory writable (`src/daemon.rs:Startup::Refused` doc
  comment). A transient variant would belong beside it and there is none today.
- Thresholds: Not applicable.
- Required side effects: on the second arm, the line is
  `pns daemon: the spool directory could not be made (<error>)`. NOT ESTABLISHED: no test exercises that
  arm; only the not-a-directory arm is pinned.
- Forbidden side effects: nothing is repaired, nothing is unlinked, and the offending path is left
  exactly where it was found.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: `create_dir_all` on an existing directory is a no-op, so a normal start is
  silent.
- Privacy: the refusal prints the spool PATH, which is derived from `PNS_STATE_DIR` or `$HOME`, and no
  record contents.
- Process ownership and cleanup: no children exist yet.
- Compatibility contract: the spool directory is `<state>/daemon` (`src/daemon.rs:spool_dir`), the marker
  directory is `<state>/daemon-markers` (`src/daemon.rs:marker_dir`) and the heartbeat is
  `<state>/daemon-heartbeat` (`src/daemon.rs:heartbeat_path`). The heartbeat sits BESIDE the spool and
  not inside it: a heartbeat file in the spool would be read as a job every tick, refused as unparseable
  and dropped, so the daemon would spend its life deleting its own pulse. The spool directory itself is
  created by `create_dir_all` with no explicit mode, so the umask decides it; only the FILES inside carry
  an enforced `0600` (`src/daemon.rs:STATE_FILE_MODE`).

### 5. The loop sleeps first, then takes one pass

Given a started daemon

When each turn of the loop begins

Then it sleeps for one whole tick before doing anything, then increments its counter, then possibly re-reads the switch, then runs one pass

- Success: `src/main.rs:daemon_run`'s loop body is `sleep(tick)`, `ticks = ticks.wrapping_add(1)`, the
  `SWITCH_TICKS` block (the switch, then `ensure_presence_poll`), then
  `daemon_pass(&spool, &state, now, tick, &mut children, &mut reported)`, where `now` is the ONE
  `now_secs()` read that turn takes.
- Failure sources: none inside the loop. Every failure below it is handled by the pass.
- Fail direction: the loop never returns on an error. Only the switch check returns.
- Thresholds: the tick is the table above. The sleep comes FIRST, so the earliest a job can fire after a
  start is one tick in.
- Required side effects: none per turn beyond the pass's own.
- Forbidden side effects: no line per tick. Behavior 27 is the assertion.
- Timeout and cancellation: the sleep is the only wait in the process, which is what makes SIGTERM arrive
  inside a bounded window.
- Idempotency and duplicates: the counter uses `wrapping_add`, so it never overflows and the
  `is_multiple_of(SWITCH_TICKS)` test keeps working past `u64::MAX`.
- Privacy: Not applicable.
- Process ownership and cleanup: the `children` vector and the `reported` set live for the life of the
  loop and are passed by mutable reference into every pass.
- Compatibility contract: the tick is a constant with a test hatch rather than a config key, following
  `PNS_PAYLOAD_DEADLINE_MS`: the only party who has ever needed a different tick is a test, and a knob
  nobody turns is a knob that only ever holds a wrong value (`src/main.rs:daemon_tick` doc comment).

### 6. The switch is re-read every thirtieth tick and stops a daemon that is already running

Given a running daemon and an operator who edits the config to `[daemon] enabled = false`

When the tick counter next hits a multiple of `SWITCH_TICKS`

Then the daemon prints its line and exits 0

- Success: `src/main.rs:daemon_run`'s `if ticks.is_multiple_of(SWITCH_TICKS) && !daemon_enabled()` arm.
  Pinned end to end by `tests/daemon.rs:turning_the_config_switch_off_stops_a_running_daemon`, which
  proves the daemon was up and beating first, then that it exits within ten seconds, that the code is 0,
  and that the log contains `disabled in the config; exiting`.
- Failure sources: an unreadable or unparseable config, which reads as ENABLED (behavior 3's fail
  direction), so a broken config never stops a running daemon.
- Fail direction: on. The daemon keeps ticking.
- Thresholds: `SWITCH_TICKS` is 30 (`src/main.rs:SWITCH_TICKS`). At the production tick that is one
  config read per 30 seconds and an off switch that takes effect within half a minute. At tick 29 the
  config is not read; at tick 30 it is. Counted in TICKS rather than seconds so one knob moves with the
  clock instead of two disagreeing about it.
- Required side effects: the same verbatim line as behavior 3,
  `pns daemon: disabled in the config; exiting`, and exit 0.
- Forbidden side effects: nothing is unlinked on the way out. The heartbeat file is LEFT BEHIND (nothing
  in the crate removes it; a grep of `src/` finds `publish_heartbeat` and `heartbeat_path` and no
  remover), so the doctor grades it by age afterwards.
- Timeout and cancellation: live children are NOT killed and NOT waited on. See the process table.
- Idempotency and duplicates: reading the switch has no side effect, so a re-read that says "still on"
  costs one file read.
- Privacy: the config's contents never reach the log; only a parse error's detail does, on the
  carrying-on-enabled arm.
- Process ownership and cleanup: this is one of the two orphaning exits named in the process table. The
  `children` vector is dropped without `kill` or `wait`.
- Compatibility contract: read once at startup the switch was INERT, because nothing bounces this launchd
  job on a config change (the loader's trigger is the plist hash), so the operator's off switch did
  nothing until a hand-typed bootout while the daemon kept firing jobs and the doctor reported it off
  (`src/main.rs:daemon_run`, `tests/daemon.rs:turning_the_config_switch_off_stops_a_running_daemon`).

### 7. One pass reaps before it drains, and the order is the behavior

Given a pass beginning

When `daemon_pass` runs

Then it reaps first, then publishes the heartbeat, then drains the spool

- Success: `src/main.rs:daemon_pass` is literally `reap(children)`, the clock check, `publish_heartbeat`,
  `drain_spool`. It is a FUNCTION and not four lines inline for exactly that reason: the order is the
  behavior, so a test has to be able to run it in the order production runs it rather than in one of its
  own. Pinned by `src/main.rs:a_job_waits_while_its_own_child_lives_and_fires_once_that_child_has_gone`,
  which drives `daemon_pass` twice: once with a live child of the same id (the record is untouched, and
  its INODE is asserted unchanged, so it was never even claimed) and once after that child is killed (the
  record changes, so the held occurrence fired).
- Failure sources: a clock that cannot be read (behavior 10).
- Fail direction: reaped the other way round, a child that exited moments ago still reads as running and
  holds its own due occurrence to one more `Wait` than it needed, which on the lights job is a tick of a
  lamp that has stopped breathing.
- Thresholds: Not applicable.
- Required side effects: exactly one heartbeat write per pass that has a clock.
- Forbidden side effects: the drain never runs before the reap.
- Timeout and cancellation: the reap is non-blocking (behavior 23), so the pass cannot be held by a
  child.
- Idempotency and duplicates: a pass is safe to repeat. Everything it does is either a rewrite of one
  line or a claim arbitrated by rename.
- Privacy: Not applicable.
- Process ownership and cleanup: the reap is the ONLY place a child is killed or waited on.
- Compatibility contract: `daemon_pass` takes `now: Option<u64>` rather than reading the clock itself,
  which is what lets a test pin a second.

### 8. The daemon publishes a heartbeat every pass, fail-quiet

Given a pass with a readable clock

When it publishes

Then `<state>/daemon-heartbeat` holds `<pid> <epoch>` and a newline, at mode 0600

- Success: `src/main.rs:daemon_pass` builds `Heartbeat { pid: std::process::id(), at: now }` and calls
  `src/daemon.rs:publish_heartbeat`, which stages a private `~pending.<pid>.daemon-heartbeat` in the
  state directory and renames it into place. The line's shape is `src/daemon.rs:render_heartbeat`
  (`"{pid} {at}"`), read back by `src/daemon.rs:parse_heartbeat`. Pinned by
  `tests/daemon.rs:the_daemon_does_not_write_a_log_line_per_tick` (which waits for the file as its proof
  a tick happened at all) and by
  `src/doctor.rs:a_heartbeat_round_trips_and_anything_else_is_no_heartbeat_at_all`.
- Failure sources: an unwritable state directory; a rename that fails.
- Fail direction: FAIL-QUIET. The result is discarded with `let _ =`. A heartbeat that did not land costs
  one doctor line, and complaining about it every tick is the chatter this daemon must never produce.
- Thresholds: `src/daemon.rs:HEARTBEAT_STALE_SECS` is 10 seconds, ten times the private
  `DEFAULT_TICK_SECS`. A beat exactly `HEARTBEAT_STALE_SECS` old still reads as running; one second older
  reads as not running (`src/doctor.rs:the_daemons_doctor_line_tells_the_truth_in_four_states` asserts
  both sides).
- Required side effects: the publish is by RENAME. A plain write truncates first, and a reader landing
  between the truncate and the bytes sees an empty file, which every reader of these files reads as no
  state at all (`src/daemon.rs:publish`). The pending path sits in the same directory, because a rename
  across filesystems is not one, and carries the process id so two runs publishing at once cannot share
  one. A failed rename unlinks the pending file before returning the error.
- Forbidden side effects: the heartbeat is never written into the spool.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: two daemons against one state directory overwrite each other's beat every
  pass. The last writer's pid is what the doctor reports. NOT ESTABLISHED: nothing prevents two daemons
  from running against one state directory, and no test covers the doctor's reading in that state.
- Privacy: the file holds a process id and an epoch second. No job id, no argv, no detail.
- Process ownership and cleanup: the file is never removed, not on the switched-off exit and not anywhere
  else. It ages out instead.
- Compatibility contract: an AGE rather than a process-id probe is what the doctor grades, because a
  process id can be reused, so `kill(pid, 0)` answers "some process exists" and not "this daemon is
  alive" (`src/daemon.rs:Heartbeat` doc comment). `parse_heartbeat` refuses pid 0 and anything that is
  not two plain counts separated by one space, and NO HEARTBEAT is its own answer rather than a beat at
  epoch zero.

### 9. A pass with no readable clock still reaps, and drains nothing

Given a machine whose wall clock cannot be read

When `daemon_pass` runs

Then the reap has already happened and the pass returns before the heartbeat and the drain

- Success: `src/main.rs:daemon_pass` has `reap(children)` above `let Some(now) = now else { return; }`.
- Failure sources: `SystemTime::now().duration_since(UNIX_EPOCH)` failing (`src/main.rs:now_secs`).
- Fail direction: a bound is still a bound with no wall clock to publish against, and a child left
  running past its own because the clock would not answer is the one failure here that accumulates. So
  the reap is unconditional and the drain is not.
- Thresholds: Not applicable.
- Required side effects: none. No heartbeat is published on this path.
- Forbidden side effects: no job is claimed, fired or dropped, because every one of those decisions needs
  a second.
- Timeout and cancellation: the child bounds are `Instant` deadlines, which are monotonic and do not need
  the wall clock, so the kill path keeps working.
- Idempotency and duplicates: Not applicable.
- Privacy: Not applicable.
- Process ownership and cleanup: unchanged. The reap is the cleanup.
- Compatibility contract: the same "no clock is no registration" rule holds on the writer side
  (`src/main.rs:register_lights_tick`, `src/main.rs:arm_nag`, `src/main.rs:daemon_schedule`), so nothing
  ever writes a job due at epoch zero.

### 10. The spool scan is sorted, skips the module's own working files, and survives an unreadable directory

Given a spool directory holding jobs, a claim in flight and a pending write

When `spool_entries` scans it

Then it returns only the entries whose names do not start with `~`, sorted

- Success: `src/daemon.rs:spool_entries` filters on `WORKING_PREFIX` and sorts. `~` is OUTSIDE the id
  character set (`src/daemon.rs:name_is_safe` admits only ASCII alphanumerics and `.`, `_`, `:`, `-`),
  which is what makes this a rule rather than a convention: a claim (`~claim.<pid>.<seq>.<id>`,
  `src/daemon.rs:claim`) and a pending write (`~pending.<pid>.<id>`, `src/daemon.rs:pending_for`) both
  live in the spool directory and the scan has to tell them from a job without parsing them.
- Failure sources: `read_dir` failing (a missing or unreadable directory).
- Fail direction: an unreadable directory yields an EMPTY list, silently. The `read_dir` result is
  flattened with `.into_iter().flatten().flatten()`, so an error is an empty pass rather than a
  complaint.
- Thresholds: Not applicable. There is no cap on how many entries one pass handles.
- Required side effects: none. The scan is read-only.
- Forbidden side effects: no entry is opened by the scan itself.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: the sort is what makes a tick DETERMINISTIC, so two daemons walk the
  entries in the same order and the rename arbitrates each one.
- Privacy: only names are read.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the entry list is collected BEFORE the loop body runs, so a record re-armed
  during the pass is not revisited in that same pass.

### 11. An irregular spool entry is left alone, never opened, and complained about exactly once

Given a named pipe (or a symlink, or a directory) in the spool

When the drain reaches it

Then it is left exactly where it was found, never opened, and named on stderr once rather than once a tick

- Success: `src/daemon.rs:peek` answers `Peeked::Irregular` off `symlink_metadata(...).is_file()` before
  any open, and `src/main.rs:drain_spool` prints only when `reported.insert(entry.clone())` is true,
  verbatim: `pns daemon: <path> is not a regular file; left alone and never opened`. Pinned by
  `tests/daemon.rs:an_irregular_spool_entry_is_left_alone_and_never_opened`, which uses a real
  `/usr/bin/mkfifo` pipe, proves an ordinary job still fires afterwards (so the clock was not stalled),
  asserts the pipe still exists, and counts the complaint lines at exactly 1.
- Failure sources: none. The check is one metadata call.
- Fail direction: refuse rather than repair. `open` on a named pipe blocks until a writer arrives, so a
  daemon that opened what it found would stop forever on the first tick that saw it, with nothing in the
  log to say why. A symlink is a write somewhere this tool did not choose.
- Thresholds: Not applicable.
- Required side effects: the `reported` set grows by one path.
- Forbidden side effects: the entry is never opened, never claimed, never renamed and never removed.
- Timeout and cancellation: this behavior is what keeps the loop from having an unkillable hang.
- Idempotency and duplicates: `reported` is a `BTreeSet<PathBuf>` that is never pruned, so an entry
  removed and recreated at the same path is never complained about a second time in that daemon's life,
  and the set grows monotonically with the number of distinct irregular paths seen. Both are properties
  of the code as written; NOT ESTABLISHED: no test covers either, and no comment names them.
- Privacy: the full path is printed. It is a state path, not a record's contents.
- Process ownership and cleanup: nothing is spawned.
- Compatibility contract: the same refusal is applied to the heartbeat by the doctor (behavior 32) and to
  the markers directory (behavior 19).

### 12. A record that is not a record is dropped by name, never guessed at

Given a regular spool file whose content is not a valid job record

When the drain claims and re-reads it

Then it is dropped and the log names the record and the rule it broke

- Success: `src/daemon.rs:parse` refuses a missing field, a repeated field, an unknown field and a value
  of the wrong shape, each naming the offender (`src/daemon.rs:fill`, `src/daemon.rs:required`,
  `src/daemon.rs:count`). `src/daemon.rs:validate_shape` is then applied to what parsed, which is the
  whole reason it is a function rather than a check inside the registration: a hand-edited spool file
  must not be able to do what a registration could not. `src/main.rs:act` prints
  `` pns daemon: dropped `<id>`: <refusal> `` and releases the claim. Pinned by
  `tests/daemon.rs:a_hand_edited_spool_record_whose_args_fail_validation_is_dropped` (a record with
  `args=[]` that parses cleanly and fails the shape rules; the log must contain both
  `` dropped `handmade`  `` and `` `args` is empty ``, the spool must empty, and the fire count must stay
  0\) and by `src/daemon.rs:a_record_that_is_not_a_record_is_refused_by_name_rather_than_guessed_at`.
- Failure sources: a truncated write, a hand edit, another program writing into the spool, a record over
  the cap.
- Fail direction: a record half-read is a job whose remaining fields somebody else's edit decided, and
  the daemon RE-EXECUTES THIS BINARY from it. So refuse, and say which field.
- Thresholds: the record cap is `src/daemon.rs:RECORD_MAX`, 8192 bytes. `src/daemon.rs:peek` reads with
  `Read::take(file, RECORD_MAX as u64 + 1)`, one byte PAST the cap, so a file over the cap still arrives
  over it and the parse refuses it rather than reading a truncated record as a whole one. The id and
  marker cap is `src/daemon.rs:ID_MAX`, 64 characters. `every` must be between 1
  (`src/daemon.rs:MIN_EVERY_SECS`) and 86400 (`src/daemon.rs:EVERY_MAX_SECS`): 0 is refused as a spin the
  loop would re-arm into the past on every pass, 86401 is refused as a lease-length repeat nobody meant
  to write. `args` may hold at most 32 words (`src/daemon.rs:ARGS_MAX`) totalling at most 4096 bytes
  (`src/daemon.rs:ARGS_BYTES_MAX`), and may not be empty. `until` may equal `due` (a zero-length lease is
  legal and is the shape the nag registers) but may not be less.
- Required side effects: the claim is released, which unlinks the working file, so the record is gone.
- Forbidden side effects: nothing is run, and no partial job is reconstructed.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: the drop is arbitrated by the same rename every action is, so two daemons
  cannot both drop and both log it.
- Privacy: the REFUSAL text is printed, and it names fields and caps, not values. The exception is the
  id-mismatch refusal (behavior 13), which quotes both ids.
- Process ownership and cleanup: nothing is spawned for a dropped record.
- Compatibility contract: the on-disk form is one line of TAB-separated `key=value` with `args` as a JSON
  array (`src/daemon.rs:render`). Tabs rather than spaces is what lets the argv keep its own spaces, and
  JSON escaping turns a literal tab inside a detail string into `\t`, so no field value can carry the
  separator. An ABSENT optional field is not rendered at all rather than rendered as a sentinel, because
  every candidate sentinel is a legal marker name
  (`src/daemon.rs:the_two_optional_fields_round_trip_as_absent_rather_than_as_a_sentinel`).

### 13. A record whose `id` is not its filename is refused

Given a spool file named `a-job` whose record says `id=other-job`

When the peek reads it

Then it is `Unusable` and the refusal names both ids

- Success: `src/daemon.rs:peek` takes `expect_id` and refuses a mismatch with
  `` its `id` is `<found>`, which is not the `<expected>` it was spooled as ``. Pinned by
  `src/daemon.rs:a_record_whose_id_is_not_its_filename_is_refused`, with an unmutated control proving the
  same record under its own name is a job.
- Failure sources: a hand edit; a client that rendered one id and wrote it under another.
- Fail direction: refuse. The id is what a repeat republishes under and what a cancel removes, so a file
  `A` whose record says `id=B` would let a job re-arm itself ON TOP OF an unrelated one.
- Thresholds: Not applicable.
- Required side effects: on the claim path the same `expect_id` is passed, because a claim is the same
  record under a working name and its id must still be the one it was published as (`src/main.rs:act`
  calls `peek(claim, id)` with the id taken from the original filename).
- Forbidden side effects: nothing is re-published under either id.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: Not applicable.
- Privacy: both ids are printed. A nag id carries a session id, which is already in the filename.
- Process ownership and cleanup: the claim is released.
- Compatibility contract: the id IS the spool filename, so re-registering the same id replaces the job by
  rename rather than stacking a second one (`src/daemon.rs:Job::id` doc comment). Every id-based
  guarantee in this document rests on that.

### 14. The peek is read-only, and `Wait` is the only verdict it may settle

Given a spool entry that turns out to be a job that is not due

When the drain looks at it

Then nothing is renamed, nothing is rewritten and the entry is left exactly where it was found

- Success: `src/main.rs:drain_spool`'s match has one arm for `Peeked::Job(job)` guarded by
  `decide(...) == Verdict::Wait`, whose body is empty, and one catch-all arm that claims first. The
  reasoning is stated three ways: a read-only peek is enough to decide to do NOTHING; every decision that
  ACTS is taken again on a claimed record; and a `Wait` is never claimed, because a wait performs no
  action and renaming a waiting job out and back would be the very write that can lose a client's
  refresh. Pinned by the inode assertion in
  `src/main.rs:a_job_waits_while_its_own_child_lives_and_fires_once_that_child_has_gone`.
- Failure sources: none. The peek opens the file read-only.
- Fail direction: towards doing nothing.
- Thresholds: Not applicable.
- Required side effects: none.
- Forbidden side effects: no rename, no write, no unlink. A registration arriving in the same second
  cannot be overwritten by a put-back of the record this tick had already read.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: two daemons can peek the same waiting record simultaneously and both do
  nothing.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the three invariants the drain protocol states (`src/main.rs:drain_spool` doc
  comment) are (1) a client always wins, because every write the daemon makes into the spool is
  create-if-absent; (2) the daemon acts only on what it owns, so everything but `Wait` claims first and
  re-reads; (3) one occurrence runs once, because the rename is the arbiter and it is taken before the
  content is read.

### 15. A claim is taken by rename, and the rename is the ownership test

Given two daemons (or a daemon and a hand-run `pns daemon` process) reaching one due job in the same second

When each tries to claim it

Then exactly one wins, and the loser reads nothing at all

- Success: `src/daemon.rs:claim` renames `<spool>/<id>` to `<spool>/~claim.<pid>.<seq>.<id>` and answers
  the new path, or `None`. The measurement behind it is recorded in the module comment and in the
  function's own: on macOS 26.2 (Apple File System) eight processes unlinking one path were EVERY ONE OF
  THEM told they had succeeded, while 40 rounds of eight racers renaming gave exactly one winner every
  time. So a plain unlink is not an ownership test and a rename is.
- Failure sources: the entry already gone (another claimant won); a name already occupied by this
  process's own earlier claim.
- Fail direction: `None`, and the drain simply moves on. A failed claim means another run got there,
  which is exactly what the rename is for.
- Thresholds: Not applicable.
- Required side effects: the held name carries a PER-RUN SEQUENCE (`CLAIM_SEQ`, an `AtomicU32` fetched
  and incremented) as well as the process id, so one name per process does not couple every claim in a
  run to the first one and a claim the run could not finish does not occupy the name forever.
- Forbidden side effects: `claim` NEVER renames over a claim already there. It checks
  `symlink_metadata(&claim).is_ok()` first and returns `None`, because a rename overwrites and the name
  is this run's alone: anything sitting at it is a job this process claimed and could not finish, and
  losing it silently is worse than leaving it.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: this is the whole duplicate-suppression mechanism. Two daemons cannot both
  run one occurrence, because the claim is taken BEFORE the record is read for anything the daemon acts
  on. A daemon and a hand-run `pns daemon schedule` do not race for a claim at all: the client only ever
  publishes (behavior 28), and its publish uses an overwriting rename that a claim has already moved the
  name out of the way of. Pinned by
  `src/daemon.rs:a_registration_landing_while_the_old_record_is_claimed_is_not_deleted_by_the_cleanup`,
  which proves the daemon's cleanup removes the WORKING name and never the id, so a registration that
  arrived during the claim survives.
- Privacy: the claim name embeds the job id, and the file's mode is whatever the original record carried
  (0600, since a rename preserves it).
- Process ownership and cleanup: `src/main.rs:release` is the only remover, and it names a claim it could
  not remove (behavior 26).
- Compatibility contract: the residual windows are stated honestly in `src/main.rs:drain_spool`. A
  refresh that lands AFTER the claim is taken cannot stop the already-claimed occurrence from running, so
  the operator can see one card from the record that was in flight plus the refreshed job afterwards.
  Nothing is lost and nothing runs twice; the old occurrence simply ran. That refresh also wins the
  re-arm's link, so the repeat continues on the client's terms.

### 16. `decide` checks the lease, then the marker, then a running child, then the due second

Given a job, a second, whether its marker exists and whether a child THIS job already fired is still running

When `decide` is asked

Then it answers `Wait`, `Fire` or `Drop(reason)` and opens no file and reads no clock

- Success: `src/daemon.rs:decide` is a total function of four values, which is what lets the window be
  swept a second at a time in a unit test. The order is fixed: `now > until` is `Drop(LeaseExpired)`;
  then `marker_exists` is `Drop(MarkerPresent)`; then `running` is `Wait`; then `now < due` is `Wait`;
  otherwise `Fire`. An expired job is dropped as expired even when its marker also arrived, and a job
  whose answer came in is dropped without ever being described as waiting.
- Failure sources: none. It is pure.
- Fail direction: a RUNNING CHILD answers `Wait`, never `Drop`, so the occurrence stays due and fires the
  tick after that child is gone rather than being lost.
- Thresholds: BOTH EDGES CLOSED. `due <= now <= until` fires. Pinned exactly by
  `src/daemon.rs:a_job_fires_only_inside_its_window_and_both_edges_are_closed`: `due - 1` waits, `due`
  fires, `due + 1` fires, `until` fires, `until + 1` is `Drop(LeaseExpired)`. The late-storm rule is its
  own test (`src/daemon.rs:a_job_whose_lease_expired_while_the_machine_slept_is_dropped_never_run_late`):
  a laptop that slept through a job wakes to a lease that expired while it was down, and the job is
  dropped rather than run late, because "the machine was asleep" and "the nudge is now pointless" are the
  same condition.
- Required side effects: none.
- Forbidden side effects: none. It touches nothing.
- Timeout and cancellation: the marker IS the cancellation primitive
  (`src/daemon.rs:a_present_marker_cancels_the_job_before_anything_runs`). The lease is the timeout.
- Idempotency and duplicates: the running-child arm is what stops two children of one job driving one
  house at once, and it is told the truth only because the reap runs first
  (`src/daemon.rs:a_running_child_holds_the_next_occurrence_to_a_wait_rather_than_a_fire`,
  `src/main.rs:a_job_waits_while_its_own_child_lives_and_fires_once_that_child_has_gone`). The seamless
  breath is why it exists: the lights tick's last fade is issued to still be running when the child
  exits, so the schedule alone can no longer promise the previous child is gone before a second one
  starts. `rearm`'s `now + every` still governs how soon the next occurrence is due, so a job held here
  by a slow child does not burst once that child finally exits.
- Privacy: Not applicable.
- Process ownership and cleanup: the `running` input is
  `children.iter().any(|bounded| bounded.id == job.id)`, which is ONE process's memory. A tick the
  operator ran by hand and an orphan left behind by a daemon replacement are both invisible to it. That
  is why the lights tick additionally takes a file lock the operating system arbitrates
  (`src/main.rs:LIGHTS_TICK_LOCK`, and the doc comment there stating "THE DAEMON'S OWN BOOKKEEPING IS NOT
  A LOCK").
- Compatibility contract: `Reason` is two variants and not one string, because they send a reader to two
  different places: a lease that ran out is a machine that was down or a client that stopped refreshing,
  and a marker is the thing the job was waiting to be told. The half-sentences are
  `src/daemon.rs:Reason::said`: `its lease had expired` and `its marker was already there`.

### 17. A claimed job that is not due goes back create-if-absent

Given a job whose peek said act but whose claimed re-read says `Wait`

When the daemon puts it back

Then the record goes back under its id only if a client has not written there in the meantime

- Success: `src/main.rs:act`'s `Verdict::Wait` arm calls `src/daemon.rs:hand_back` and releases the claim
  on both `Ok(true)` and `Ok(false)`.
- Failure sources: a staging or link failure.
- Fail direction: on error the line is `` pns daemon: `<id>` could not be put back (<error>) `` and the
  claim is STILL released, so no working file is left behind. NOT ESTABLISHED: no test exercises this
  error arm.
- Thresholds: Not applicable.
- Required side effects: `hand_back` uses `src/daemon.rs:publish_if_absent`, which stages the bytes at
  the pending name and publishes with `hard_link`, because a rename has no create-if-absent form and
  `link(2)` is the one call that publishes a complete file and refuses an occupied name in the same step.
  The temp is unlinked either way, so a name somebody else won leaves nothing behind.
- Forbidden side effects: the daemon NEVER overwrites a client's record. `src/daemon.rs:publish_job` (the
  overwriting form) is PRIVATE, and `schedule` is the only way in, so the loop cannot overwrite a
  client's registration even by mistake: the call that would do it is not in scope where the loop is
  written.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: this is the client-always-wins invariant, pinned by
  `src/daemon.rs:a_refresh_published_while_a_job_is_claimed_survives_the_daemons_re_arm`, which publishes
  a refresh, asserts the daemon's `hand_back` answers false, asserts the refresh is what stayed, and then
  runs the unmutated control proving a free id does take the daemon's write.
- Privacy: the record's contents are re-written, not logged.
- Process ownership and cleanup: the claim is released on every arm.
- Compatibility contract: `hard_link` rather than `create_new` so the file that lands is the one the temp
  already carries, mode, bytes and all, published in one step the way the rename publishes. There is no
  window in which a reader can see the name with nothing behind it.

### 18. A dropped job says which rule dropped it

Given a claimed job whose verdict is `Drop`

When the daemon drops it

Then it prints one line naming the id and the reason, and the record is gone

- Success: `src/main.rs:act`'s `Verdict::Drop(reason)` arm prints
  `` pns daemon: dropped `<id>` because <reason> `` where `<reason>` is one of `its lease had expired`
  and `its marker was already there`, then releases the claim. Pinned end to end by
  `tests/daemon.rs:a_marker_on_disk_cancels_a_scheduled_job_end_to_end` (which asserts the exact phrase
  `its marker was already there` and then runs an in-test control: the same daemon, the same tick, an
  identical job with no marker, which fires) and by
  `tests/hooks.rs:the_daemon_really_fires_the_nag_and_really_drops_it_when_the_marker_is_there` (which
  asserts `` dropped `nag:s1` because its marker was already there `` AND that no `pns nag` process was
  spawned at all).
- Failure sources: none of its own.
- Fail direction: a DROP is still said out loud, even though a successful firing is not. Refusing a job
  is news.
- Thresholds: the lease edge is behavior 16's.
- Required side effects: the claim is released, so the job is gone from the spool. A repeating job that
  is dropped does NOT re-arm: only `fire` re-arms.
- Forbidden side effects: the marker is NOT removed by the drop. Nothing in the crate sweeps
  `<state>/daemon-markers`; the only remover is `src/main.rs:arm_nag`, which clears the previous
  approval's marker for that session before arming a new job. Markers therefore accumulate, one per
  session id that ever answered.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: the claim arbitrates, so only one daemon logs the drop.
- Privacy: the id is printed. For a nag that is `nag:<session-id>`.
- Process ownership and cleanup: nothing is spawned. The nag test asserts exactly that.
- Compatibility contract: `Reason::said` returns a half-sentence, so the log line reads as one sentence.
  Changing either half changes an operator-visible string that two tests match on.

### 19. A marker cancels a job only through a real markers directory

Given a job carrying `unless_marker`

When the daemon asks whether the marker is there

Then it checks the directory first and refuses a symlink standing where it should be

- Success: `src/daemon.rs:marker_exists` returns false for a job with no marker, false for a marker name
  that fails `name_is_safe`, false when `<state>/daemon-markers` is not a real directory, and otherwise
  answers `symlink_metadata(directory.join(marker)).is_ok()`. `symlink_metadata` is deliberate, so a
  DANGLING symlink still counts as present: the question is whether something wrote the marker, not
  whether it resolves.
- Failure sources: a symlinked markers directory; a name that is not a filename; a missing directory.
- Fail direction: a REFUSED DIRECTORY READS AS NO MARKER, so the job RUNS. A marker that cannot be
  trusted cancels nothing, and the cost is one extra card rather than a cancellation somebody else's
  symlink decided. Pinned by `src/daemon.rs:a_symlinked_markers_directory_cancels_nothing`, with an
  unmutated control proving a real directory with the same marker in it does cancel.
- Thresholds: the name rules are `src/daemon.rs:name_is_safe`, the same as an id: 1 to 64 characters of
  letters, digits, `.`, `_`, `:` or `-`, no leading `.`, no `..`.
- Required side effects: none. The check is read-only.
- Forbidden side effects: the field is a NAME and never a path. It is resolved inside the state directory
  by the library, so it cannot become a general filesystem probe. A validated name cannot escape the
  state directory by itself, but a link AT the directory carries the whole lookup somewhere this tool did
  not choose, which is the general filesystem probe the name rule exists to prevent.
- Timeout and cancellation: this IS the cancellation channel. Writers are `src/main.rs:write_marker`
  (empty file, mode 0600, present is the whole message), called from the resolved and answered paths.
- Idempotency and duplicates: writing a marker twice is the same state. A marker left by a PREVIOUS
  approval in the same session would make a new job drop silently, which is why `arm_nag` clears it
  first, and clears it BEFORE writing the record so a concurrent fire cannot find the new record beside
  the old marker.
- Privacy: marker names embed a session id (`nag-<session-id>`), and the file body is empty.
- Process ownership and cleanup: no sweeper exists. See behavior 18's forbidden side effects.
- Compatibility contract: `name_is_safe` is deliberately its OWN rule rather than either of `safety`'s
  two. `session_id_is_safe` refuses the colon, which a job id needs (`nag:sess-123`); `pane_is_safe`
  admits `..` and a leading dot, which a filename must not have. Sharing either would couple this rule to
  a change made for a different reason.

### 20. A fired job re-arms durably before it spawns, create-if-absent

Given a claimed job whose verdict is `Fire`

When the daemon fires it

Then the repeat is written first, then the claim is released, then the child is spawned

- Success: `src/main.rs:fire` calls `rearm`, then `hand_back` for the next occurrence, then
  `release(claim)`, then `spawn_job`.
- Failure sources: a link failure on the re-arm; a spawn failure.
- Fail direction: written the other way round, a daemon killed between the two loses the repeat with the
  job already run, which is the lamp going dark on a loop that is still alive. A failed spawn is said out
  loud: `` pns daemon: `<id>` could not start (<error>) ``, because an action that suppressed its own
  error has not been performed and the alternative is a job that reports as run and delivered nothing. A
  failed re-arm is said too: `` pns daemon: `<id>` will not repeat (<error>) ``.
- Thresholds: Not applicable.
- Required side effects: when the id was taken in the meantime, the line is
  `` pns daemon: `<id>` was registered again while it ran, so its repeat stands down ``, and the client's
  record stands. NOT ESTABLISHED: no test exercises that line, nor the two error lines above.
- Forbidden side effects: the re-arm is create-if-absent for the same reason the put-back is. A client
  that refreshed this id while the occurrence was claimed published the newer signal, and a rename here
  would overwrite it with the due and lease this daemon computed from the record it had already taken.
- Timeout and cancellation: the child's bound is set at spawn (behavior 25).
- Idempotency and duplicates: the entry list for the pass was collected before the loop body, so a
  re-armed record published during the pass is not revisited in that same pass. On the next pass,
  `decide` answers `Wait` while this job's child is still running.
- Privacy: nothing about the job's argv reaches the log on the success path, because the success path is
  silent (behavior 27).
- Process ownership and cleanup: the claim is released BEFORE the spawn, so a spawn failure leaves no
  working file behind.
- Compatibility contract: `hand_back` is the daemon's ONLY write into the spool. Everything else it does
  there is a rename out (claim) or an unlink (release).

### 21. A repeat re-arms at `now + every` and never extends its own lease

Given a fired job with `every` set

When it re-arms

Then the next due is `now + every`, `until` is carried over unchanged, and a next due past `until` leaves nothing behind

- Success: `src/daemon.rs:rearm` is
  `let due = now.saturating_add(job.every?); (due <= job.until).then(|| Job { due, ..job.clone() })`.
  Pinned by `src/daemon.rs:a_repeating_job_re_arms_at_now_plus_every_and_a_one_shot_does_not_re_arm`,
  which asserts the new due, that `until` is UNCHANGED, that the id and argv survive, that a one-shot
  returns `None`, that a job reached 100 seconds late re-arms from NOW and not from its old due, that a
  next occurrence past the lease returns `None`, and that one landing EXACTLY on the lease still re-arms.
- Failure sources: an `every` large enough to overflow, handled by `saturating_add`.
- Fail direction: `now + every`, never `due + every`. A loop that reaches a job late (a busy tick, a
  woken laptop) and re-armed from the OLD due would compute a next due still in the past, fire again
  immediately, and keep firing until it caught up: a burst of cards for a schedule that meant one.
- Thresholds: `due <= until` re-arms; `due == until` re-arms; `due > until` does not. `every` itself is
  bounded 1 to 86400 at validation (behavior 12).
- Required side effects: none beyond the returned job.
- Forbidden side effects: `until` is NOT renewed. A repeat that renewed its own lease would run until the
  machine stopped, with nobody refreshing it and nothing to notice that the client which asked for it is
  gone. And the lease is what ENDS the repeat: rather than leave a record whose own due sits outside its
  own lease (which the loop would then refuse as malformed on its next pass, a true statement about a
  file this code wrote and a confusing one to find in a log), it leaves nothing.
- Timeout and cancellation: the lease is the timeout. Pinned end to end by
  `tests/daemon.rs:a_repeating_job_keeps_firing_until_its_lease_runs_out_then_stops`, which registers
  `--every 1 --until +3`, waits for at least two firings, waits for the spool to empty of its own accord,
  and then settles the firing count over a window longer than one `every` to prove firing really stopped.
- Idempotency and duplicates: `rearm` is pure and produces one record.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the lease is what makes a lights job stop when the operator stops working, and
  it is why every event REFRESHES the lease rather than the daemon renewing it.

### 22. A fired job is this binary re-executed, detached, in a process group of its own

Given a job about to run

When `spawn_job` starts it

Then it runs `std::env::current_exe()` with the record's argv, stdin and stdout null, stderr inherited, and `process_group(0)`

- Success: `src/main.rs:spawn_job`.
- Failure sources: `current_exe` failing; `spawn` failing (a missing binary, a process limit).
- Fail direction: the `io::Result` is returned to `fire`, which prints
  `` pns daemon: `<id>` could not start (<error>) ``.
- Thresholds: Not applicable.
- Required side effects: STDERR IS INHERITED, and that is the one reader a job has. A job runs unattended
  with no terminal behind it, so a complaint it writes goes wherever this puts that stream. With stderr
  null it went to `/dev/null`, and the lights tick's say-once memory then recorded the complaint as SAID,
  so no later tick repeated it either: a lamp renamed on the bridge was reported exactly once, into
  nothing. The daemon's plist points both of its own streams at `~/.local/log/pns-daemon.log`, so
  inheriting is what puts a child's line in front of the operator. Pinned by
  `tests/daemon.rs:a_job_childs_own_complaint_reaches_the_daemons_log`, which schedules a job whose argv
  is a bare `lights` (a usage error) and waits for `usage: pns lights tick` to appear in the daemon's
  log.
- Forbidden side effects: STDOUT STAYS NULL, because that is where a job's ORDINARY output goes and the
  ordinary case is a tick that ran three times a minute and has nothing to report. Only what could not be
  said anywhere else crosses. Stdin is null, so a job can never block on a terminal that is not there.
  And the program is `current_exe` and NEVER a stored path, so nothing in the spool can name another
  program. That is a blast-radius limit rather than a security boundary (anyone who can write a 0600 file
  in this directory can already run `pns`), and it costs nothing.
- Timeout and cancellation: the bound is attached at this moment (behavior 25). `process_group(0)` puts
  the child in a new group whose leader is the child, which is the ONLY reason `kill_group` can name it
  with a negative process id.
- Idempotency and duplicates: one spawn per fired occurrence. `decide`'s running-child arm is what stops
  a second one for the same id.
- Privacy: the argv reaches the process table and the spool file. That is why the nag deliberately puts
  NO free text in `args`: the operator's own question lives in the nag record and `pns nag` takes no
  argument. `pns daemon schedule` places whatever the operator typed after `--` into the record, so that
  route CAN put free text in the spool.
- Process ownership and cleanup: this is the second row of the process table. The daemon owns the child
  through `Bounded`; it owns the rest of the group only through the group kill.
- Compatibility contract: under test, `current_exe` is the test binary, which is why
  `src/main.rs:a_job_waits_while_its_own_child_lives_and_fires_once_that_child_has_gone` uses the
  harness's own `--list` flag as its argv.

### 23. A child is reaped without blocking, and killed as a group past its bound

Given children the daemon started

When a pass reaps

Then each is polled with `try_wait`, exited and errored children are dropped from the list, and any that outlived its deadline has its whole process GROUP killed

- Success: `src/main.rs:reap` uses `retain_mut`. `Ok(Some(_)) | Err(_)` drops the entry; `Ok(None)` past
  `expires_at` kills the group, then the direct child, then waits, then drops the entry; `Ok(None)`
  inside the deadline keeps it.
- Failure sources: a wedged child; a child whose group cannot be signalled.
- Fail direction: `try_wait` AND NEVER `wait`. A blocking wait on a child that hangs holds the whole
  loop, so one wedged delivery stops every later job: the clock would pass every other test here and stop
  in production. The `wait` in the kill arm runs only on a child that has ALREADY been killed, which
  returns at once and is what stops a zombie.
- Thresholds: `Instant::now() >= bounded.expires_at`, so the deadline edge is inclusive. The deadline
  itself is behavior 25.
- Required side effects: after `kill_group`, `child.kill()` runs too, in case the group could not be
  signalled at all, and then `child.wait()` turns a killed child into a reaped one rather than a zombie
  held for the daemon's lifetime.
- Forbidden side effects: the reap never blocks and never touches the spool.
- Timeout and cancellation: this IS the cancellation. Pinned end to end by
  `tests/daemon.rs:a_hung_child_does_not_stall_the_tick_and_is_killed`, which runs a job whose channel
  stub records its own process id, starts a grandchild (`sleep 30 &`), records the grandchild's process
  id and then waits. The test asserts three separate things: a SECOND job registered while the first
  still hangs still fires (so the tick is not stalled), the direct child dies past the bound, and THE
  GRANDCHILD dies too. It runs at `FAST_TICK_MS` (10ms) so the 30-tick bound costs 300ms rather than
  750ms.
- Idempotency and duplicates: a reaped child is removed from `children`, so `decide` stops seeing it as
  running on the very next drain in the same pass.
- Privacy: nothing is logged by the reap, on any arm.
- Process ownership and cleanup: `children` holds at most one entry per job id, because `decide` refuses
  to fire a second child of an id whose first is still listed. The vector's size is therefore bounded by
  the number of distinct job ids in the spool.
- Compatibility contract: the reap runs BEFORE the drain in every pass (behavior 7), which is what makes
  `decide`'s `running` input true.

### 24. `kill_group` refuses a process id it cannot vouch for

Given a bounded child that must be killed

When `kill_group` is called with its process id

Then it signals the negated id with SIGKILL, and refuses 0, 1 and anything not representable

- Success: `src/main.rs:kill_group` converts to `libc::pid_t` and returns early on failure, returns early
  on `pid <= 1`, and otherwise calls `libc::kill(-pid, libc::SIGKILL)` inside one `unsafe` block whose
  safety argument is stated: `kill` takes two integers by value, reads and writes no memory this process
  owns, and the only outcomes are a signal delivered or an errno nothing here reads.
- Failure sources: a process id above `pid_t`'s range; a process id of 0 or 1.
- Fail direction: refuse rather than trust. `kill(0, ...)` signals THIS process's own group and
  `kill(-1, ...)` signals every process the user owns, so a process id that is neither a real child nor
  representable is refused. NOT ESTABLISHED: no test drives `kill_group` with 0, 1 or an out-of-range
  value; the guard is code and comment only.
- Thresholds: `pid <= 1` returns; `pid == 2` proceeds.
- Required side effects: THE GROUP AND NOT THE CHILD, which is the difference between a bound and a bound
  that holds. The job is a `pns` that spawns a delivery of its own and waits on it, so killing the direct
  child alone leaves that delivery running, MEASURED still alive 750ms past a 300ms bound, and a
  repeating job that hangs then accumulates them.
- Forbidden side effects: no signal other than SIGKILL is ever sent, and no other group is ever named.
- Timeout and cancellation: SIGKILL is not catchable, so there is no grace window and no second stage.
- Idempotency and duplicates: killing an already-dead group returns an errno nothing reads.
- Privacy: Not applicable.
- Process ownership and cleanup: the errno is discarded, so a group that could not be signalled is
  invisible; the follow-up `child.kill()` in `reap` is the mitigation.
- Compatibility contract: `process_group(0)` in `spawn_job` is what makes the negative process id name a
  group at all. Removing it silently reduces every bound to a direct-child kill.

### 25. The child bound is thirty ticks, with the lights tick as the one exception

Given a job being spawned

When its deadline is computed

Then it is `tick * CHILD_TICKS` for every job but `lights`, and the larger of that and one whole lights interval for `lights`

- Success: `src/main.rs:child_bound` returns `tick * CHILD_TICKS` when `id != LIGHTS_JOB`, and otherwise
  `(tick * CHILD_TICKS).max(MAX_REFRESH_SECS + tick_bridge_deadline(MAX_REFRESH_SECS) + tick)`. Pinned
  exactly by
  `src/main.rs:a_child_outlives_the_longest_interval_plus_the_write_and_the_reap_that_follow_it`.
- Failure sources: none. It is arithmetic.
- Fail direction: err long for the lights tick. Killing it early loses the phase its last fade landed on.
- Thresholds, exact: `CHILD_TICKS` is 30. At a 1s tick, a non-lights child gets 30s and the lights child
  gets `max(30s, 30 + 6 + 1) = 37s` (the test asserts 37s exactly). At a 60s tick the lights child gets
  `max(1800s, 37s) = 1800s` (the test asserts 1800s exactly). At a 10ms tick a non-lights child gets
  300ms exactly (the test asserts that for `nag:a-session`). `MAX_REFRESH_SECS` is 30
  (`src/config.rs:MAX_REFRESH_SECS`) and `tick_bridge_deadline(30)` is `max(30 / 5, 1)` = 6s
  (`src/main.rs:tick_bridge_deadline`).
- Required side effects: none. The value is stored in `Bounded::expires_at` as
  `Instant::now() + child_bound(...)` at spawn.
- Forbidden side effects: no other job is widened by the lights arm. An event delivery's channels each
  carry their own deadline, so one still alive at `CHILD_TICKS` is wedged rather than slow, and giving it
  37 seconds would only delay the kill.
- Timeout and cancellation: the arithmetic behind the lights arm is stated: the longest interval the
  config permits, plus the longest a single write may take at that interval, plus one reap tick, because
  a child is only noticed as gone on the pass AFTER it exits. Bounded at `CHILD_TICKS` alone, a
  thirty-second refresh equalled a thirty-second child, and a seamless breath that issued its last fade
  at child time 29999ms had its legal six-second reply killed before the tick could record where the lamp
  landed, leaving the next tick to resume from a phase nothing had written.
- Idempotency and duplicates: pure.
- Privacy: Not applicable.
- Process ownership and cleanup: `expires_at` is an `Instant`, so it is monotonic and unaffected by wall
  clock jumps.
- Compatibility contract: the same arithmetic is restated by `src/main.rs:lights_tick_stale_secs` for the
  lights tick's own lock (`MAX_REFRESH_SECS + tick_bridge_deadline(MAX_REFRESH_SECS) + 1`), because it
  bounds the same process. The two are separate expressions of one number and nothing checks them against
  each other.

### 26. A working file the daemon could not remove is named

Given a claim the daemon is finished with

When the remove fails

Then one line names the file and says it was left behind

- Success: `src/main.rs:release` prints, verbatim:
  `pns daemon: the working file <path> could not be removed (<error>); it is left behind`.
- Failure sources: a read-only directory; a file removed under it.
- Fail direction: say it. A claim that could not be removed is a LEAK, not a nothing: it is invisible to
  the scan (the working prefix is outside the id character set), so it sits there until a hand removes
  it, and `claim` refuses to reuse a name already taken, which can wedge that one id after a process id
  is reused. One line naming the file is the whole remedy, and it costs nothing on the path where the
  remove works. NOT ESTABLISHED: no test exercises this line.
- Thresholds: Not applicable.
- Required side effects: the line is printed on every path that releases, which is every non-`Fire` arm
  of `act` plus `fire` itself.
- Forbidden side effects: nothing tries to force the removal.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: a leaked claim is never retried, because the scan cannot see it.
- Privacy: the path embeds the process id, the claim sequence and the job id.
- Process ownership and cleanup: this is the only manual-cleanup obligation the daemon can create.
- Compatibility contract: the working prefix is `~` (`src/daemon.rs:WORKING_PREFIX`), and no valid id can
  start with it.

### 27. The daemon says nothing per tick, and nothing about a firing that worked

Given a running daemon

When it idles, and when it successfully runs a job

Then it writes nothing at all

- Success: two separate tests. `tests/daemon.rs:the_daemon_does_not_write_a_log_line_per_tick` waits for
  the heartbeat file first (so "said nothing" is not vacuous about a daemon that never got going), then
  sleeps eight more ticks and asserts the combined log is exactly the empty string.
  `tests/daemon.rs:a_daemon_that_ran_a_job_says_nothing_about_having_run_it` schedules a job, waits for
  its delivery, sleeps past the spawn and past the drain that follows it, and asserts the log is still
  exactly empty.
- Failure sources: any new print in the loop or in `fire`'s success arm.
- Fail direction: silence. What a job has to say, the job says itself: its stderr is the daemon's now.
- Thresholds: the numbers behind the rule are stated in the code: a line per tick is 86400 lines a day,
  which rotates a real log out of existence and which `compress-and-truncate-local-logs.sh` picks up with
  no registration at all; and the lights tick repeats every twelve seconds for as long as its lease
  holds, so a line per firing is 300 an hour.
- Required side effects: none.
- Forbidden side effects: no success line, no "ran <id>" line, no per-tick trace.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: the irregular-entry complaint is said ONCE per path rather than once a tick
  for exactly this reason (behavior 11).
- Privacy: silence is also the privacy property. Nothing about which jobs exist reaches the log on the
  happy path.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: everything the daemon DOES say is an exception listed in this document:
  behaviors 3, 4, 6, 11, 12, 13, 17, 18, 20, 26. A firing that WORKED is not among them, which is why the
  nag's end-to-end test uses the delivered card as its probe rather than a log line.

### 28. `pns daemon schedule` registers one job and waits on nothing

Given the operator or a rider registering a job

When `pns daemon schedule --id <id> [--in <secs>] [--every <secs>] [--until +<secs>|<epoch>] [--unless-marker <name>] -- <args>` runs

Then the record is validated and published by rename, with no daemon involved

- Success: `src/main.rs:parse_schedule` builds a `ScheduleRequest`, `src/main.rs:daemon_schedule` reads
  the clock once, computes `due = now + in_secs` and `until` from `Until`, and calls
  `src/daemon.rs:schedule`, which validates and then publishes with the private overwriting
  `publish_job`. Exit 0.
- Failure sources: an unparseable argv; no clock; a record the validation refuses; a spool write that
  fails.
- Fail direction: LOUD and non-zero for a typed command, because `pns`'s own event parser is lenient (it
  sits on a notification path that must not fail) and this one sits in front of an operator who typed a
  command and will believe it did what they wrote. An unparseable argv prints `DAEMON_USAGE` and exits 2.
  No clock prints `pns daemon: this machine has no clock to schedule against` and exits 1. A refusal
  prints `pns daemon: <refusal>` and exits 1 (a spool failure reads
  `pns daemon: the spool write failed: <error>`).
- Thresholds: `--in` defaults to 0. `--until` absent gives `due + 60`
  (`src/main.rs:DEFAULT_LEASE_SLACK_SECS`), because a lease is never ABSENT, only unstated: a job with no
  expiry is the parked job the whole design refuses. A minute is long enough that a busy tick or a slow
  boot still delivers, short enough that a machine asleep through the moment wakes to a job whose point
  has passed. `--until +<secs>` is relative, a bare number is an absolute epoch.
  `src/daemon.rs:validate_registration` adds the one bound that needs a clock:
  `due.abs_diff(now) > DUE_WINDOW_SECS` is refused, where `DUE_WINDOW_SECS` is 30 days, in BOTH
  directions (far in the future parks a job the lease can never expire, far in the past is a clock jump
  or a corrupt field rather than a schedule). Pinned by
  `src/daemon.rs:a_due_outside_a_bounded_window_of_now_is_refused_at_registration`, which asserts both
  `now + DUE_WINDOW_SECS + 1` and `now - DUE_WINDOW_SECS - 1` are refused by name.
- Required side effects: exactly one file at `<spool>/<id>`, mode 0600, published by rename from
  `~pending.<pid>.<id>` in the same directory.
- Forbidden side effects: NO WAIT ON ANYTHING. Pinned by
  `tests/daemon.rs:a_registration_succeeds_with_no_daemon_anywhere_and_blocks_on_nothing`, which asserts
  the registration succeeds with no daemon anywhere, the record lands, and the whole call finishes inside
  a deliberately generous 5-second ceiling (a ceiling, never a tight number: it asserts that nothing
  waits, not how fast a process starts on a loaded machine). A registration that talked to the daemon
  would hold its caller for as long as the daemon was wedged, which is the class the whole design exists
  to stay out of.
- Timeout and cancellation: Not applicable. There is no network and no subprocess.
- Idempotency and duplicates: re-registering an id is a REFRESH rather than a second job, because the id
  is the filename and `publish_job` renames over it: newest-signal-wins is what a rename gives for free.
  A client racing the daemon is behaviors 15 and 17.
- Privacy: everything after `--` is written into the spool record verbatim and will appear in the process
  table when the job runs.
- Process ownership and cleanup: no process is created. `schedule` RETURNS its error rather than printing
  it, because every caller states its own fail direction and the one it exists for (a hook registering a
  nudge) drops it the way a log line is dropped: silently, locally, and without touching the return value
  of the thing that called it.
- Compatibility contract: `parse_schedule` refuses an unknown flag and a flag whose value is missing, and
  requires both `--id` and a non-empty argv after `--`. An argv that passes every FIELD bound and still
  renders past `RECORD_MAX` is refused at registration rather than accepted and dropped later, because
  the render JSON-escapes the argv and one control character becomes six bytes; pinned by
  `src/daemon.rs:an_argv_that_renders_past_the_record_cap_is_refused_by_name`, with an unmutated control
  proving the same 4096 plain bytes are accepted.

### 29. `pns daemon cancel` forgets one job, and is not an error the second time

Given `pns daemon cancel --id <id>`

When the job is there, absent, or the id is not a job id

Then the three cases are exit 0, exit 0 and exit 1

- Success: `src/main.rs:daemon_cancel` destructures argv into exactly `[flag, id]` and requires
  `flag == "--id"`; anything else prints `DAEMON_USAGE` and exits 2. `src/daemon.rs:cancel` validates the
  id with `name_is_safe` and unlinks `<spool>/<id>`.
- Failure sources: an unsafe id; a remove that fails for a reason other than not-found.
- Fail direction: an unsafe id is `` pns daemon: `<id>` is not a job id ``, exit 1. A remove error is
  `pns daemon: the spool entry could not be removed: <error>`, exit 1.
- Thresholds: Not applicable.
- Required side effects: on success, `` pns daemon: cancelled `<id>`  `` on stdout, exit 0.
- Forbidden side effects: an ABSENT job is NOT an error. It prints
  `` pns daemon: no job named `<id>` was scheduled `` and exits 0, because the end state the operator
  asked for is the one they already have, and a non-zero exit would make a drill's cleanup step fail the
  second time it ran.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: idempotent by construction, per the forbidden side effect above.
- Privacy: the id is echoed. A nag id carries a session id.
- Process ownership and cleanup: a cancel unlinks the id, so it does NOT reach a claim the daemon already
  holds. An occurrence already claimed still runs.
- Compatibility contract: `cancel` returns `Result<bool, String>`, so the library caller can tell "there
  was one" from "there was not"; the exit code deliberately does not.

### 30. The lights tick registration keeps its due second and takes its lease from the lane

Given an event, a hand-taken loop lease, or a tick with work still in flight

When any of the three registers the tick

Then the lease is refreshed and the due second already pending is KEPT

- Success: `src/main.rs:schedule_lights_tick` peeks `<spool>/lights`, takes the pending job's `due` when
  it is still in the future, and otherwise uses `now + lights.refresh_secs`. It then writes
  `until = due.max(now + lease_secs)`, `every = Some(lights.refresh_secs)`, no marker, and
  `args = ["lights", "tick"]`.
- Failure sources: no `[lights]` table; no readable clock; a spool that will not take a write.
- Fail direction: FAIL-OPEN and silent. `register_lights_tick` returns early with no lights table and no
  clock ("NO CLOCK IS NO REGISTRATION, never a job due at epoch zero"), and the `schedule` result is
  dropped with `let _`. Pinned by
  `tests/dispatch.rs:a_registration_that_cannot_be_written_costs_the_event_nothing`, which puts a regular
  file where the spool directory goes and asserts the event's stdout, stderr, exit code and fired legs
  are byte-identical to the working baseline.
- Thresholds: the ordinary lease is 300s and the journalled one is 43200s, and
  `tests/dispatch.rs:an_event_registers_the_tick_and_a_journalled_one_leases_it_for_longer` asserts both
  EXACTLY rather than merely differently, by recovering the second the lease was measured from
  (`due - refresh`). `until = due.max(now + lease)` is why: a `refresh_secs` longer than the ordinary
  lease used to EXTEND that lease to the refresh, and the config's own 30-second refresh ceiling is what
  closes that. The `max` exists because a lease that ended before its own job's first run is a record
  `validate_shape` refuses, and a refused registration is a lamp that never re-arms with nothing said
  anywhere.
- Required side effects: ONE JOB FOR THE WHOLE HOUSE, not one per lamp, because the tick derives every
  state from scratch and writes every fixture, so a second job would be a second writer of the same
  bulbs.
- Forbidden side effects: keeping the due second is not decoration. Re-registering replaces the job by
  name, so an event storm that pushed `due` out to `now + refresh` every time would keep moving the tick
  away from itself and a busy machine's lamps would never be re-armed at all. The LEASE is what every
  caller refreshes; the schedule is left where the last tick put it.
- Timeout and cancellation: the lease lapses on an idle machine, pinned by
  `tests/dispatch.rs:a_tick_with_nothing_in_flight_lets_its_own_lease_lapse` (a tick with nothing to
  watch registers nothing at all), and it renews while work is in flight, pinned by
  `tests/dispatch.rs:a_tick_with_work_in_flight_keeps_itself_scheduled_past_the_loop_threshold`. The
  hand-taken lease registers the tick that reads it, pinned by
  `tests/dispatch.rs:a_lease_taken_by_hand_schedules_the_tick_that_reads_it`.
- Privacy: the record carries two fixed words and no lamp name, no room name and no detail.
- Process ownership and cleanup: the tick child additionally takes an operating-system-arbitrated lock
  (`src/main.rs:LIGHTS_TICK_LOCK`, released by `src/main.rs:HeldLock`'s `Drop`), because the daemon's own
  in-memory bookkeeping cannot see a tick the operator ran by hand or an orphan left by a replaced
  daemon.
- Compatibility contract: three callers and ONE registration function, because the tick's lease is what
  decides whether a lamp can EVER light and three spellings of it would be three answers.

### 31. The nag registration is one job per session, cancelled by a marker

Given a blocked approval with `[nag] after_secs` set

When `arm_nag` runs

Then the previous marker is cleared, the record is written, and one job `nag:<session-id>` is registered

- Success: `src/main.rs:arm_nag`. The ORDER is load bearing twice over. Clearing the marker at all is
  required for correctness rather than hygiene: the marker name is constant PER SESSION, so one left by
  the previous approval in this session would make the new job drop silently. Clearing it BEFORE the
  record closes a window a concurrent fire can walk into: published first, the new record can be claimed
  by a fire that then finds the previous approval's marker still on disk and drops it as answered.
- Failure sources: a session id that is not usable; no clock; a marker that will not clear; a record that
  will not write; a registration the validation refuses.
- Fail direction: loud on stderr but never fatal to the hook. A refused registration also DROPS the
  record, so the sentence stays true: a record with no job wakes no fire of its own, but it stays
  ENUMERABLE, and leaving it would be the line saying one thing while the state on disk said another. The
  line is
  `pns: the nag could not be scheduled (<refusal>); this approval will not be nudged, <its record is dropped|and its record could not be dropped either>`.
- Thresholds: `due = now + after_secs` and `until = due + after_secs`, one whole schedule past the due
  second, which resolves to the same instant as the fire-time staleness cap. The two are not redundant:
  the lease drops the JOB, so a machine that slept through the window never spawns at all, while the cap
  judges RECORDS, a different set because a fire enumerates siblings whose own jobs have not fired yet.
  `src/nag.rs:MAX_SESSION_ID_CHARS` is `ID_MAX - JOB_PREFIX.len()`, so the composed id always fits.
- Required side effects: the record at `<state>/nag/<session-id>.pending` is mode 0600, asserted by
  `tests/hooks.rs:arming_writes_a_record_registers_a_job_and_clears_a_stale_marker_first` alongside the
  spool assertions.
- Forbidden side effects: NO FREE TEXT REACHES THE SPOOL. `args` is `["nag"]` and the operator's own
  question lives in the record, because `args` are visible in the spool file and in whatever the daemon
  logs.
- Timeout and cancellation: the marker `nag-<session-id>` is written by the resolved and answered paths
  and is what makes coalescing quiet: every sibling job of a coalesced card drops through the marker
  path.
- Idempotency and duplicates: ONE JOB PER APPROVAL, and the id is the spool filename, so a second
  approval in one session REPLACES the job rather than stacking a second one.
- Privacy: the session id appears in the job id, in the marker name and in the record's filename. The
  detail does not reach any of the three.
- Process ownership and cleanup: the fired `pns nag` process takes its own fire claim, released by
  `src/main.rs:release_fire`, which is a separate mechanism from the daemon's claim.
- Compatibility contract: `pns nag` takes NO session argument, because coalescing means it looks at every
  outstanding record rather than at the one whose timer woke it, so an argument would be a value it had
  to ignore.

### 32. The doctor grades the clock by heartbeat age and never by process id

Given `pns doctor`

When it reports the daemon

Then it prints one line derived from the config switch, the heartbeat's age and a count of the spool, and it never moves the exit code

- Success: `src/main.rs:daemon_line` reads the heartbeat only when `symlink_metadata` says it is a
  regular file, parses it, and hands the four inputs to `src/doctor.rs:daemon_line`. The six lines,
  verbatim:
  - `pns doctor: the daemon is off in the config`
  - `pns doctor: the daemon is off in the config, but pid <pid> is still beating; bootout (or wait) to stop it`
  - `pns doctor: the daemon is enabled and has not run yet`
  - `pns doctor: the daemon is running, pid <pid>, <n> job<s> scheduled`
  - `pns doctor: the daemon is enabled, its last beat was <age>s ago, so it is not running`
  - `pns doctor: the daemon is enabled, its last beat was an unknown time ago, so it is not running`
- Failure sources: no heartbeat; a heartbeat that is not a regular file; a heartbeat that will not parse;
  no clock; a beat stamped after now.
- Fail direction: towards NOT RUNNING. No clock, and a beat stamped in the future, both leave nothing to
  compare, and vouching for a daemon on the strength of a timestamp nothing could grade is the
  identity-is-not-presence mistake with a file standing in for the process
  (`src/doctor.rs:a_heartbeat_whose_age_cannot_be_taken_reads_as_not_running`). A non-regular file is
  never OPENED, for a worse reason than the spool's: `open` on a named pipe blocks until a writer
  arrives, so a doctor that read whatever it found there would hang instead of printing any of its four
  states, with the pairing check and the exit code never reached. Pinned by
  `tests/daemon.rs:a_heartbeat_that_is_not_a_regular_file_is_refused_rather_than_opened`, which spawns
  the doctor against a real named pipe, polls for its exit, kills it and PANICS if it never finished.
- Thresholds: `HEARTBEAT_STALE_SECS` is 10. A beat exactly 10s old reads as running; 11s old reads as not
  running (`src/doctor.rs:the_daemons_doctor_line_tells_the_truth_in_four_states` asserts both). The job
  count is singular at 1 and plural everywhere else, asserted in the same test.
- Required side effects: two reads that cost nothing, the heartbeat file and a count of the spool. It
  does NOT signal the process id, because a process id can be reused.
- Forbidden side effects: the line NEVER moves the exit code. Pinned by
  `tests/daemon.rs:the_doctor_reports_a_dead_daemon_without_moving_its_exit_code`, which is explicitly
  there because `exit_code` cannot see the daemon at all, so the only place the mistake could be made is
  the composition root, and only a run of the real binary reaches that.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: reading is idempotent.
- Privacy: it COUNTS jobs and never names them, following the missed journal's structural privacy rule:
  the count answers "is anything scheduled" and the contents are a reader nobody asked for. The count is
  REGULAR FILES ONLY (`src/daemon.rs:job_count`), so the word "job" in the sentence is earned; pinned by
  `src/daemon.rs:the_job_count_counts_records_and_not_whatever_is_in_the_directory`.
- Process ownership and cleanup: no process is created.
- Compatibility contract: OFF IN THE CONFIG IS NOT THE SAME FACT AS STOPPED. Nothing bounces the launchd
  job when the config changes, so a daemon started while the switch was on keeps running and keeps firing
  after it is turned off, and the operator who just flipped it is standing in exactly that state
  (`src/doctor.rs:a_daemon_switched_off_but_still_beating_is_reported_as_still_beating`). `enabled` comes
  from the ONE config read the doctor already took, never a second one, and its broken-config fallback is
  ON, the same one `daemon_run` takes, so the report and the service cannot disagree.

### 33. Shutdown needs no handler, and a child in flight is orphaned rather than killed

Given launchd stopping the job, or the operator booting it out

When SIGTERM arrives

Then the process dies inside its tick and any live child is left running

- Success: `src/main.rs:daemon_run`'s doc comment states it: launchd stops a job with SIGTERM and the
  default disposition terminates the process; a loop sleeping one second dies inside the tick. There is
  no signal handler anywhere in the crate.
- Failure sources: none.
- Fail direction: a child mid-flight is ORPHANED rather than killed, and an orphaned nudge is at worst
  one extra card. `spawn_job`'s process group exists so that a group kill is possible AND so that launchd
  stopping the daemon does not take a delivery with it.
- Thresholds: the shutdown window is one tick, so at the production tick the process exits within a
  second of the signal.
- Required side effects: none. The daemon holds no durable state, so restarting re-reads the directory,
  which is the whole recovery path, and reboot works the same way because the state directory survives it
  and the lease drops whatever went stale. There is no in-memory schedule to diverge from the disk.
- Forbidden side effects: nothing is flushed, nothing is unlinked, and the heartbeat file is left where
  it is to age out.
- Timeout and cancellation: `ThrottleInterval` is 10 in the plist, stated at its default rather than left
  to it, because it decides how fast a crash-looping daemon burns and a reader of the plist should not
  have to know the manual page to see it.
- Idempotency and duplicates: on restart the spool is re-read from scratch. A job whose lease expired
  while the daemon was down is dropped rather than run late (behavior 16). A repeat that was mid-flight
  has already had its next occurrence written durably BEFORE the spawn (behavior 20), so a restart
  resumes it.
- Privacy: Not applicable.
- Process ownership and cleanup: this is the first row of the "left without an owner" list. NOT
  ESTABLISHED: no test sends SIGTERM to a running daemon and observes what happens to its children;
  `DaemonGuard`'s `Drop` uses SIGKILL and asserts nothing afterwards.
- Compatibility contract: `KeepAlive { SuccessfulExit = false }` with `RunAtLoad` true is what makes the
  clean exits in behaviors 3, 4 and 6 stay exited while a crash still restarts. `RunAtLoad` has to be
  stated explicitly because the implication that comes with a bare `KeepAlive true` does not survive the
  dictionary form.

## Glossary

| Term              | Defining symbol                                                                                          |
| ----------------- | -------------------------------------------------------------------------------------------------------- |
| `job`             | `src/daemon.rs:Job`                                                                                      |
| `spool`           | `src/daemon.rs:spool_dir` (`<state>/daemon`)                                                             |
| `claim`           | `src/daemon.rs:claim`                                                                                    |
| `lease`           | `src/daemon.rs:Job::until`, enforced by `src/daemon.rs:decide` and carried over by `src/daemon.rs:rearm` |
| `tick`            | `src/main.rs:daemon_tick`, `src/main.rs:DEFAULT_TICK_MS`                                                 |
| `pass`            | `src/main.rs:daemon_pass`                                                                                |
| `marker`          | `src/daemon.rs:marker_dir`, `src/daemon.rs:marker_exists`, `src/main.rs:write_marker`                    |
| `heartbeat`       | `src/daemon.rs:Heartbeat`, `src/daemon.rs:publish_heartbeat`, `src/daemon.rs:HEARTBEAT_STALE_SECS`       |
| `verdict`         | `src/daemon.rs:Verdict`, `src/daemon.rs:Reason`                                                          |
| `bound`           | `src/main.rs:Bounded`, `src/main.rs:child_bound`, `src/main.rs:CHILD_TICKS`                              |
| `working file`    | `src/daemon.rs:WORKING_PREFIX`, `src/daemon.rs:pending_for`, `src/main.rs:release`                       |
| `startup refusal` | `src/daemon.rs:Startup`, `src/daemon.rs:prepare_spool`                                                   |
| the enable switch | `src/config.rs:Config::daemon_enabled`, `src/main.rs:daemon_enabled`, `src/main.rs:SWITCH_TICKS`         |
