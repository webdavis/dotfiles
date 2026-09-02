# Blocking approval and the moshi gate

This is the one path where pns stands between a harness and its operator, and the one path where a
non-zero exit code from pns means anything at all. A harness about to run something it needs permission
for calls either `pns hook blocked` (the pns hook, which also raises pns's own notification) or the gate,
spelled `pns gate <harness>-hook` or spelled as the bare word `pns <harness>-hook`, and pns decides
whether to hand that request to `moshi-hook` for a round trip to the operator's phone. Everything here
turns on one rule: pns is a presence-gated pipe, never a decider. It forwards the harness's payload byte
for byte, it waits a bounded time for moshi to acknowledge the submission, and it passes moshi's exit
code back untouched. Every path on which it declines to forward exits 0, which is the harness's "no
opinion, prompt as usual", so a pns that cannot reach moshi costs the operator a phone card and never a
refused tool call. The pns hook path adds two things the gate does not have: its own notification about
the block (raised after the forward starts, with the phone leg suppressed when the forward really began),
and the durable state a wait leaves behind (the blocked marker, the nag record, the decision ring line).

Vocabulary note: throughout, "submission" is the `moshi-hook` child process pns spawns, and "the approval
card" is what moshi raises on the phone from the forwarded payload. pns never sees that card and never
mints its identifier.

### 1. Both spellings of the gate reach one function

Given moshi's own generated pi and omp extensions hold a single pathname in their `helperBinary` field,
with no room for a subcommand

When argv[1] is a harness word (`pns pi-hook`), or argv[1] is `gate` and argv[2] is a harness word
(`pns gate pi-hook`)

Then both dispatch into the same `gate_mode` with that harness word, and behave identically from there.

- Success: `pns pi-hook` and `pns gate pi-hook` forward the same payload as the same argv to `moshi-hook`
  and return the same code. Pinned by
  `tests/hooks.rs:the_bare_harness_word_forwards_through_the_gate_and_returns_the_decision` and
  `tests/hooks.rs:the_documented_gate_subcommand_reaches_the_same_gate_as_the_bare_word`, both asserting
  exit 7 from a stub and `pi-hook` recorded as the child's argv.
- Failure sources: argv[2] absent on the `gate` form yields the empty string
  (`src/main.rs:second_argument`), which fails the shape check in behavior 2.
- Fail direction: fail-open toward the harness. A gate that declines exits 0, which is "no opinion": the
  harness draws its own permission prompt and the operator answers at the pane. It never blocks and never
  denies.
- Thresholds: Not applicable, no deadline is involved in dispatch.
- Required side effects: none beyond process exit. The dispatch in `src/main.rs` calls
  `std::process::exit(gate_mode(...))` directly, so no event path runs.
- Forbidden side effects: a gate raises no notification of its own.
  `tests/hooks.rs:the_documented_gate_subcommand_reaches_the_same_gate_as_the_bare_word` asserts
  `!sandbox.fired("hermes")` with the comment "a gate forwards; it never raises an event of its own".
- Timeout and cancellation: inherited from behavior 6.
- Idempotency and duplicates: one invocation is one submission, see behavior 12.
- Privacy: Not applicable at dispatch, the harness word is the only argument read.
- Process ownership and cleanup: see behavior 5.
- Compatibility contract: the bare-word spelling exists solely because moshi's generated extension cannot
  express a subcommand. The operator-facing spelling is documented in `src/main.rs:USAGE` as
  `pns gate <harness>-hook          presence-gated pass-through to moshi-hook` and
  `pns <harness>-hook               the same gate, spelled the way moshi calls it`.

### 2. The shape the gate will vouch for

Given the harness word arrives from a file moshi generates, and `moshi-hook`'s own positional argument is
a path

When `pns::hooks::is_harness_subcommand` is asked about that word

Then only a lowercase ASCII name followed by `-hook` is vouched for, and everything else is refused
before any child is spawned.

- Success: `pi-hook` and `claude-hook` are accepted, in the unit asserts beside
  `src/hooks.rs:is_harness_subcommand` (its own test module, lines 872 to 879).
- Failure sources: the word is split on its FIRST hyphen; the suffix must equal `hook` exactly and the
  name must be non-empty ASCII lowercase. So `hook`, `-hook`, `Pi-hook`, `pi-hook; rm -rf /`,
  `../../etc/passwd` and the empty string are all refused. A word with two hyphens, `a-b-hook`, splits to
  name `a` and suffix `b-hook`, which is not `hook`, so it is refused too (derived from
  `src/hooks.rs:is_harness_subcommand`; NOT ESTABLISHED: no test drives a two-hyphen word, I grepped
  `tests/hooks.rs` and `src/hooks.rs` for one and found none).
- Fail direction: fail-closed toward moshi (nothing is handed to a third-party binary) and fail-open
  toward the harness (the caller still gets an exit code that means "prompt as usual"). The two spellings
  differ in HOW they exit, see the exit-code table in behavior 7. `pns gate <bad word>` returns 0
  silently, pinned by
  `tests/hooks.rs:the_gate_subcommand_refuses_a_word_it_will_not_vouch_for_without_notifying`. A bare bad
  word never reaches `gate_mode` at all and falls through the dispatch chain to the usage refusal, exit
  2, pinned by `tests/hooks.rs:a_shape_the_gate_will_not_vouch_for_is_never_handed_to_moshi`.
- Thresholds: Not applicable, this is a shape predicate with no numeric bound.
- Required side effects: on the bare-word refusal only, `src/main.rs` prints `USAGE` to stderr.
- Forbidden side effects: no child is spawned, no notification is raised, and stdin is never read. Both
  refusal tests assert `!sandbox.path("moshi.argv").exists()`, and the `gate` form's test also asserts
  `!sandbox.fired("hermes")`.
- Timeout and cancellation: Not applicable, the refusal is synchronous.
- Idempotency and duplicates: Not applicable.
- Privacy: the word is never pasted into a shell. `src/main.rs:spawn_moshi_hook` passes it as a single
  argv element through `Command::new(...).arg(subcommand)`, so `pi-hook; rm -rf /` could not have
  executed anything even had it been vouched for; the shape check is defence in depth on top of that.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the check is SHAPE ONLY and deliberately not a roster, because the harness list
  belongs to moshi and grows (`src/hooks.rs:is_harness_subcommand`). It is distinct from
  `src/hooks.rs:moshi_subcommand`, which IS a closed roster (`claude` and `codex`), because there the
  name arrives from pns's own configuration, see behavior 4.

### 3. The presence gate decides whether to forward at all

Given the operator may be at their desk, on their phone, or away

When `src/main.rs:forward_to_moshi` is asked whether to start a round trip

Then it forwards for every surface EXCEPT `Desk`, because at the desk the harness prompt in front of them
already is the question.

- Success: away (`PNS_IDLE_SECS=99999`) forwards, pinned by
  `tests/hooks.rs:a_blocking_event_hands_moshi_the_payload_byte_for_byte_and_returns_its_decision`. A
  desk touched 90 seconds ago against a phone touched 5 seconds ago also forwards, because newest signal
  wins, pinned by
  `tests/hooks.rs:a_phone_used_more_recently_than_the_desk_gets_the_approval_forwarded_to_it`.
- Failure sources: a presence reading nobody can parse. A garbled `PNS_IDLE_SECS` is refused rather than
  defaulted (`src/engine.rs:surface_reading`), which leaves no fresh desk reading, so the surface is not
  `Desk` and the approval IS forwarded, pinned by
  `tests/hooks.rs:a_presence_reading_nobody_can_parse_still_forwards_the_approval`.
- Fail direction: fail TOWARD the phone. An unreadable presence reading is the failure that looks exactly
  like sitting at the desk, and reading it as a desk would lose approvals entirely. Declining does not
  block and does not deny; it exits 0, the harness prompts as usual, and at the desk that prompt is
  already on screen. Pinned by
  `tests/hooks.rs:at_the_desk_the_approval_is_never_forwarded_and_the_harness_prompts_as_usual` and
  `tests/hooks.rs:at_the_desk_the_gate_submits_nothing_and_exits_zero`.
- Thresholds: the freshness window is `src/engine.rs:DEFAULT_DESK_IDLE_SECS`, 120 seconds, overridable
  with `PNS_DESK_IDLE_SECS`. `src/surface.rs:fresh_age` filters on `age < fresh_secs` strictly, so an age
  of 119 seconds is fresh and speaks for its surface while an age of exactly 120 is not fresh at all. A
  tie between desk and phone ages goes to the desk (`src/surface.rs:surface`), so a desk and a phone both
  last touched 5 seconds ago read `Desk` and do not forward.
- Required side effects: on the pns hook path, ONE probe set is built and shared by the forward decision
  and the delivery plan, so both answer from one moment (`src/main.rs:blocking_event`,
  `src/main.rs:forward_to_moshi`). The gate builds its own throwaway probe set and runs no delivery plan
  at all (`src/main.rs:gate_mode`).
- Forbidden side effects: the forward reads the SURFACE and never the card overrides. `PNS_FORCE_PHONE`
  buys a push and not a round trip, and `PNS_SKIP_PHONE` suppresses pns's own card and not the
  submission; both are applied only to the delivery plan's `phone_card` in `src/engine.rs`, line 214, and
  both directions are pinned by
  `tests/hooks.rs:the_forward_reads_the_surface_and_never_the_card_overrides`. Visibility of the origin
  pane is never read either: `src/engine.rs:operator_surface`'s trait bound carries no session-view probe
  at all, so the forward structurally cannot consult one, pinned by
  `tests/hooks.rs:an_approval_is_forwarded_even_with_the_pane_in_plain_sight`. The operator's mute and a
  named macOS Focus cannot reach it for the same structural reason, pinned by
  `tests/hooks.rs:a_mute_never_touches_the_approval_a_blocked_operator_is_waiting_to_answer` and
  `tests/hooks.rs:a_focus_never_touches_the_approval_a_blocked_operator_is_waiting_to_answer`.
- Timeout and cancellation: the probes carry their own bounds, and the screen-lock probe is read only
  where the idle probe returned a reading, precisely because "the blocked path an approval waits on pays
  that deadline serially" (`src/engine.rs:surface_reading`).
- Idempotency and duplicates: one reading per invocation.
- Privacy: the phone marker path is `$HOME/.local/state/pns/phone-attention.marker` unless
  `PNS_PHONE_MARKER_FILE` overrides it (`src/main.rs:system_probes`). Only file times are read, never
  content.
- Process ownership and cleanup: Not applicable, the presence check spawns no child of its own.
- Compatibility contract: none, this is pns's own policy.

### 4. Only a harness pns registered itself for is handed to moshi, on the hook path

Given the hook path learns which harness it is serving from `PNS_AGENT`, which arrives from a
configuration file

When `src/main.rs:blocking_event` decides whether to forward

Then only `claude` and `codex` map to a subcommand, and anything else forwards nothing.

- Success: the default agent is `claude`, submitted as `claude-hook` (`src/main.rs:hook_mode`,
  `tests/hooks.rs:one_prompt_is_submitted_exactly_once_and_a_zero_answer_from_it_is_an_approve`).
  `PNS_AGENT=codex` is submitted as `codex-hook`, pinned by
  `tests/hooks.rs:a_codex_approval_is_submitted_as_codex_hook_and_names_the_tool_that_wants_to_run`.
- Failure sources: any other agent word. `PNS_AGENT=pi` on the hook path forwards nothing and exits 0,
  pinned by `tests/hooks.rs:a_harness_pns_does_not_register_for_is_never_handed_to_moshi`.
- Fail direction: fail-open toward the harness (exit 0, prompt as usual) and fail-closed toward moshi.
  The operator still hears about the block through pns's own notification.
- Thresholds: Not applicable.
- Required side effects: the notification still goes out on the decline.
- Forbidden side effects: the agent name is MATCHED and never pasted into a subcommand handed to a
  third-party binary (`src/hooks.rs:moshi_subcommand`). The presence probe is not run for a payload that
  was never going to be forwarded, because the roster filter is evaluated first in the filter chain
  (`src/main.rs:blocking_event`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: Not applicable.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: this roster is pns's own. The GATE has no roster at all, only the shape check
  of behavior 2, because there the word is moshi's.

### 5. The payload crosses byte for byte, or not at all

Given this process has already consumed the harness's stdin, and a consumed-but-not-forwarded stream
leaves moshi with an empty parse after which it silently does nothing

When the forward happens

Then the exact bytes read are written to the child's stdin, with no reserialization and no filtering on
whether pns could parse them.

- Success: the payload arrives at the child byte for byte including its trailing newline;
  `tests/hooks.rs:a_blocking_event_hands_moshi_the_payload_byte_for_byte_and_returns_its_decision`
  compares the recorded `moshi.stdin` to the literal payload string. A payload pns cannot parse at all is
  still submitted verbatim, because moshi does the parsing and pns forwarding only what it could read
  itself would silently swallow approvals the day a harness changes its shape; pinned by
  `tests/hooks.rs:a_payload_pns_cannot_parse_is_still_submitted_verbatim`, which submits the literal text
  `not json at all`.
- Failure sources: three, and each behaves differently.
  1. A payload that hit the size cap was cut mid-object, so it is no longer JavaScript Object Notation
     and no longer what anybody wrote. It is refused by `src/main.rs:payload_is_whole` on BOTH entry
     points, pinned by
     `tests/hooks.rs:a_payload_too_large_to_be_whole_is_never_forwarded_as_though_it_were` and
     `tests/hooks.rs:the_gate_refuses_an_over_cap_payload_as_firmly_as_the_hook_does`.
  1. A payload nobody finishes writing expires on the read deadline and is refused, pinned by
     `tests/hooks.rs:a_blocked_payload_nobody_finishes_writing_forwards_nothing_and_exits_zero`.
  1. A payload that is not valid UTF-8 fails `src/main.rs:read_payload`'s string read entirely, and the
     hook returns 0 from `hook_mode` having done nothing at all, pinned by
     `tests/hooks.rs:a_payload_that_is_not_utf8_drops_the_approval_and_tells_the_operator_nothing`.
- Fail direction: fail-open toward the harness on all three (exit 0, prompt as usual). What the operator
  hears differs: an over-cap payload still fires the notification, because "the operator still hears that
  something is blocked", while a payload that never arrived fires nothing, because a block that described
  nothing is not news. The non-UTF-8 case is total silence, and that is a KNOWN LIMIT pinned so that
  changing it is a decision rather than a drift.
- Thresholds: `src/main.rs:MAX_PAYLOAD_BYTES` is 1,000,000 bytes, and one step either side is pinned in
  both directions. A payload of exactly 1,000,000 bytes is whole and IS submitted, pinned by
  `tests/hooks.rs:a_payload_at_the_cap_is_whole_and_is_still_submitted`, whose own arithmetic asserts the
  length; a 1.2 megabyte payload is not. The reader takes `MAX_PAYLOAD_BYTES + 1` bytes on purpose, so a
  payload that HIT the cap is distinguishable from one that merely reached it
  (`src/main.rs:read_payload`). The read deadline is 5 seconds by default, overridable in milliseconds
  with `PNS_PAYLOAD_DEADLINE_MS` (`src/main.rs:payload_deadline`). Unlike the submit deadline,
  `payload_deadline` applies NO zero filter, so `PNS_PAYLOAD_DEADLINE_MS=0` is a zero-length read window
  (derived from `src/main.rs:payload_deadline` and `src/main.rs:env_deadline`; NOT ESTABLISHED: no test
  drives a zero payload deadline, I grepped `tests/hooks.rs` and `tests/dispatch.rs` for
  `PNS_PAYLOAD_DEADLINE_MS` and found only the 200 millisecond case).
- Required side effects: none beyond the write.
- Forbidden side effects: no truncated object may reach moshi, and no payload may be rewritten on the way
  through.
- Timeout and cancellation: the stdin read runs on its own thread and the caller takes it with
  `recv_timeout`. The reader thread outlives a refusal, which is accepted because the process is about to
  exit and the thread holds nothing but its own buffer (`src/main.rs:read_payload`).
- Idempotency and duplicates: one read, one write.
- Privacy: the payload travels on the child's stdin, never argv and never the environment. That is the
  same rule the moshi channel keeps for its token, see behavior 13.
- Process ownership and cleanup: see behavior 6.
- Compatibility contract: byte-for-byte is a contract with moshi, whose parser is the only thing that
  reads the payload. The 1,000,000 byte cap is pns's own.

### 6. Spawning the submission never blocks the notification

Given `moshi-hook` may not read its stdin promptly, and a payload larger than a pipe buffer is ordinary

When `src/main.rs:spawn_moshi_hook` starts the child

Then the write happens on a separate thread, so nothing waits on the child before the notification is
raised and before the one bounded wait.

- Success: the child is `Command::new(moshi_hook_bin()).arg(subcommand).stdin(Stdio::piped())`, and the
  payload is written by a spawned thread that drops the pipe on completion, which is what gives the child
  its end-of-file (`src/main.rs:spawn_moshi_hook`).
- Failure sources: `spawn` failing (the binary is not installed) yields `None`, which is the harness's
  "no opinion". `tests/hooks.rs:moshi_not_being_installed_leaves_the_hook_a_silent_exit_zero` pins exit 0
  AND that the phone card is NOT suppressed, because a forward that never spawned suppresses nothing.
- Fail direction: fail-open. Exit 0, and on the hook path the operator gets the full notification
  including the phone leg.
- Thresholds: Not applicable to the spawn itself.
- Required side effects: on the hook path only, a successful spawn sets `PNS_SKIP_PHONE=1` in this
  process, see behavior 8.
- Forbidden side effects: the write must not happen on the calling thread. A child that does not read its
  stdin must not be able to hold the notification, pinned by
  `tests/hooks.rs:a_moshi_that_never_reads_its_stdin_cannot_hold_the_notification` with a 200,000 byte
  payload, past the 64 kibibyte pipe buffer.
- Timeout and cancellation: the writer thread is allowed to outlive a caller that stops waiting. It holds
  a pipe and a copy of the payload, and the process is on its way out (`src/main.rs:spawn_moshi_hook`).
- Idempotency and duplicates: one spawn per invocation, see behavior 12.
- Privacy: the child inherits the caller's whole environment, HOME included, because `moshi-hook`
  resolves its own host identity out of it. Pinned deliberately as a mechanism by
  `tests/hooks.rs:the_submission_inherits_the_callers_environment`.
- Process ownership and cleanup: pns owns the direct child. Only stdin is piped, so the child's stdout
  and stderr are pns's own inherited streams, see behavior 7.
- Compatibility contract: the binary's location is `MOSHI_HOOK_BIN` if set, else
  `src/main.rs:DEFAULT_MOSHI_HOOK_BIN`, which is `/opt/homebrew/bin/moshi-hook`, Homebrew's own prefix
  where the cask puts it. `src/main.rs:moshi_hook_bin` is the single lookup for every caller, and the
  override is how every test points a caller at a stub instead of at the operator's own moshi.

### 7. The bounded wait, and what each exit code means

Given `moshi-hook` writes one line to its daemon's socket and returns as soon as the daemon answers it,
so a wait measured in minutes is never the operator taking their time but a daemon that stopped answering

When `src/main.rs:answer_within` waits on the submission

Then it returns moshi's own exit code if the child finishes inside the deadline, and 0 if it does not,
killing and reaping the child on expiry.

- Success: a child that exits inside the deadline yields its code through `src/main.rs:moshi_decision`,
  which is the child's status code with `unwrap_or(0)` behind it.
- Failure sources: expiry, a child killed by a signal (no exit code at all), and a wait that cannot be
  performed. All three return 0.
- Fail direction: fail-open, and precisely: exit 0 is NO OPINION and never a decision. The harness draws
  the prompt and the operator answers at the pane. Nothing pns does on this path can deny a tool call.
- Thresholds: the deadline is `src/main.rs:submit_deadline`, resolved in three steps.
  1. `PNS_MOSHI_SUBMIT_DEADLINE_MS`, in milliseconds, the test hatch. A LITERAL ZERO here is filtered out
     and falls through to the config exactly as an unset variable would, because a zero is not a bound,
     it is this wait switched off by accident.
  1. `[plugins.mobile] submit_deadline_secs`, read off the ARMED mobile table only, meaning the table is
     present, `enabled` is true, and `type = "moshi"` (`src/config.rs:armed_mobile`,
     `src/config.rs:submit_deadline`).
  1. `src/config.rs:DEFAULT_SUBMIT_DEADLINE_SECS`, which is 5 seconds. One step either side of the
     accepted range: `submit_deadline_secs = 1` is accepted, and `submit_deadline_secs = 0` is refused by
     name with the message "`mobile` key `submit_deadline_secs` is 0, which is the bound switched off by
     accident: a deadline that expires before the daemon can answer costs the phone card on every
     approval". `submit_deadline_secs = 3600` is accepted and `3601` is refused against
     `src/config.rs:MAX_SUBMIT_DEADLINE_SECS`. A refusal is LOUD:
     `src/main.rs:configured_submit_deadline` prints "pns: config error ({detail}); the moshi submission
     keeps its {n}-second bound" to stderr and takes the 5 second default, because an operator who asked
     for something, did not get it and was told nothing is the defect one level down. The poll interval
     is `src/main.rs:SUBMISSION_POLL_INTERVAL`, 10 milliseconds, short enough to add no latency an
     operator could notice on a submission answered in roughly 150 milliseconds, long enough not to spin
     a core.
- Required side effects: on expiry the child is KILLED and then REAPED. Both halves are load bearing.
  Returning alone leaves a survivor holding the inherited stdout write end open, and Claude Code decides
  a `PermissionRequest` by reading the hook's stdout to end-of-file, so the prompt stays hidden for the
  survivor's whole life. The measurements recorded on `src/main.rs:answer_within` against a ten-second
  silent submission are a reader waiting on the process alone at 0.18s, a reader waiting on stdout
  end-of-file with the child left running at 10.03s, and with the kill 0.19s. An unreaped child would be
  a zombie holding its slot until pns exits.
- Forbidden side effects: the answered path must stay untouched, with no pipe, no cap, and stdout still
  inherited. In particular the submission must NOT be routed through `run_bounded`, which pipes the
  child's stdout on its way to attaching a deadline, because this path's whole stdout contract is that
  moshi's stream IS the hook's stream; pinned by
  `tests/hooks.rs:what_moshi_says_on_stdout_reaches_the_harness_unchanged`, which asserts moshi's line
  arrives as its own line exactly once with nothing added to either end.
- Timeout and cancellation: pinned end to end on both entry points against a stub that reads its stdin
  and then execs a ten-second sleep. The hook run injects a 150 millisecond deadline and requires stdout
  end-of-file inside 600 milliseconds, pinned by
  `tests/hooks.rs:a_moshi_that_never_answers_stops_holding_the_operators_prompt`. The gate run injects
  400 milliseconds and requires end-of-file inside 1600 milliseconds, pinned by
  `tests/hooks.rs:the_gate_is_bounded_by_the_same_clock_as_the_hook`. The stub uses `exec` on purpose:
  without it the shell would fork the sleep and leave a GRANDCHILD holding the pipe, which no kill short
  of a process group could release.
- Idempotency and duplicates: an expiry submits nothing further. Both bound tests assert the recorded
  submissions are exactly one `claude-hook`, with the comment "one prompt, one submission, expiry
  included": a retry after an expiry would be a second card and a second answer to one question.
- Privacy: Not applicable, the wait reads only a process status.
- Process ownership and cleanup: THE KILL REACHES THE DIRECT CHILD ONLY. `moshi-hook` is a single binary
  that writes to its daemon's socket itself, so the direct child IS the process holding the pipe. A
  submission that forked would leave a grandchild holding it open, and that day the kill has to widen to
  the process group (`src/main.rs:answer_within`). The stated COST of the kill is that the pending action
  dies with the child, which is a card a daemon wedged enough to earn the expiry had almost certainly not
  delivered anyway.
- Compatibility contract: see the table below. The forwarded code is moshi's, passed through for whatever
  reads it.

#### Exit-code table

| Exit                                                 | Situation                                                                                                               | What the calling harness does with it                                                                                                                                                            | Pinned by                                                                                                                                                                                                                    |
| ---------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| moshi's own code, whatever it is                     | the submission started and finished inside the deadline                                                                 | passed through untouched. Claude Code reads the exit code on `PermissionRequest` NOWHERE and decides off the hook's stdout (`hookSpecificOutput.decision`); the gate's direct callers do read it | `tests/hooks.rs:a_blocking_event_hands_moshi_the_payload_byte_for_byte_and_returns_its_decision` (42 on the hook), `tests/hooks.rs:the_bare_harness_word_forwards_through_the_gate_and_returns_the_decision` (7 on the gate) |
| 0, from moshi                                        | moshi answered 0, which in production is what EVERY reply shape does                                                    | approve and deny are indistinguishable here; the operator's real answer travels moshi's own bridge                                                                                               | `tests/hooks.rs:one_prompt_is_submitted_exactly_once_and_a_zero_answer_from_it_is_an_approve`, `tests/hooks.rs:a_zero_decision_passes_through_as_zero_and_is_not_a_default`                                                  |
| 2, from moshi                                        | moshi answered 2                                                                                                        | passed through unnormalized, even though 2 is the code that means "block" across the hook family                                                                                                 | `tests/hooks.rs:a_two_from_moshi_comes_back_as_two_and_is_never_normalized`                                                                                                                                                  |
| 0, no opinion: at the desk                           | the presence gate declined                                                                                              | the harness prompts as usual                                                                                                                                                                     | `tests/hooks.rs:at_the_desk_the_approval_is_never_forwarded_and_the_harness_prompts_as_usual`, `tests/hooks.rs:at_the_desk_the_gate_submits_nothing_and_exits_zero`                                                          |
| 0, no opinion: moshi not installed                   | the spawn failed                                                                                                        | the harness prompts as usual, and pns's phone card is NOT suppressed                                                                                                                             | `tests/hooks.rs:moshi_not_being_installed_leaves_the_hook_a_silent_exit_zero`                                                                                                                                                |
| 0, no opinion: unregistered harness                  | `PNS_AGENT` is neither `claude` nor `codex`, on the hook path                                                           | the harness prompts as usual                                                                                                                                                                     | `tests/hooks.rs:a_harness_pns_does_not_register_for_is_never_handed_to_moshi`                                                                                                                                                |
| 0, no opinion: over-cap payload                      | the payload was cut mid-object at 1,000,000 bytes                                                                       | the harness prompts as usual                                                                                                                                                                     | `tests/hooks.rs:a_payload_too_large_to_be_whole_is_never_forwarded_as_though_it_were`, `tests/hooks.rs:the_gate_refuses_an_over_cap_payload_as_firmly_as_the_hook_does`                                                      |
| 0, no opinion: payload never arrived                 | the stdin read deadline expired                                                                                         | the harness prompts as usual, and NOTHING is notified                                                                                                                                            | `tests/hooks.rs:a_blocked_payload_nobody_finishes_writing_forwards_nothing_and_exits_zero`                                                                                                                                   |
| 0, no opinion: payload was not UTF-8                 | the string read failed before any arm ran                                                                               | the harness prompts as usual, total silence                                                                                                                                                      | `tests/hooks.rs:a_payload_that_is_not_utf8_drops_the_approval_and_tells_the_operator_nothing`                                                                                                                                |
| 0, no opinion: the submission died without answering | killed by a signal, so no exit code exists                                                                              | the harness prompts as usual                                                                                                                                                                     | `tests/hooks.rs:a_submission_that_dies_without_answering_is_not_a_decision`                                                                                                                                                  |
| 0, no opinion: the deadline expired                  | moshi never answered; the child is killed and reaped                                                                    | the harness prompts as usual                                                                                                                                                                     | `tests/hooks.rs:a_moshi_that_never_answers_stops_holding_the_operators_prompt`, `tests/hooks.rs:the_gate_is_bounded_by_the_same_clock_as_the_hook`                                                                           |
| 0, refusal: `pns gate <bad word>`                    | the shape check refused the word                                                                                        | silently, with no notification                                                                                                                                                                   | `tests/hooks.rs:the_gate_subcommand_refuses_a_word_it_will_not_vouch_for_without_notifying`                                                                                                                                  |
| 2, refusal: a bare `pns <bad word>`                  | argv[1] names no command and carries no producer flag, so it is an operator typo                                        | `USAGE` on stderr; this is not a hook path                                                                                                                                                       | `tests/hooks.rs:a_shape_the_gate_will_not_vouch_for_is_never_handed_to_moshi`                                                                                                                                                |
| 0, every other pns hook event                        | `stop`, `stop-failure`, `asked`, `plan-ready`, `denied`, `resolved`, `prompt`, `model-switch`, `quota`, `config-change` | a notification must never fail the turn it reports on                                                                                                                                            | `tests/hooks.rs:a_non_blocking_event_never_pays_for_the_round_trip`, `tests/hooks.rs:an_ordinary_stop_never_reaches_moshi`, `tests/hooks.rs:a_denial_never_pays_for_the_approval_round_trip_and_still_exits_zero`            |

Which of these are COMPATIBILITY CONTRACTS with a third-party tool we do not control:

- Every forwarded code (the first three rows) is MOSHI'S OWN and is a pass-through contract with
  `moshi-hook`. pns must never normalize, clamp or reinterpret it. The 2 row is the one a well-meaning
  normalizer would reach for and is pinned separately for exactly that reason.
- The meaning of exit 0 as "no opinion, prompt as usual" is a contract with the CALLING HARNESS. Claude
  Code 2.1.241 does not read this event's exit code at all (measured, recorded in the approval-contract
  section header of `tests/hooks.rs`), so for that harness it is a forward-compatibility guarantee rather
  than today's mechanism. Codex's reading of it is UNVERIFIED. NOT ESTABLISHED: nothing in this crate
  measures what Codex does with a `PermissionRequest` hook's exit code; I looked in `tests/hooks.rs`,
  `tests/dispatch.rs` and the Codex hook-installer references in `src/main.rs` and found only the
  statement that it is unverified.
- Exit 2 on a bare typo is pns's OWN convention, not a harness contract. It is only reachable when
  argv[1] names no command, which is never a hook invocation.

### 8. The blocking hook notifies, and suppresses only the leg moshi is about to duplicate

Given moshi is about to raise the actionable card itself, so pns pushing to the phone too would be the
same event twice

When `src/main.rs:blocking_event` runs

Then the forward is STARTED FIRST, the phone leg is suppressed only if that forward really began, and
then the notification is raised.

- Success: with the forward started, the durable leg still fires and the phone leg does not, pinned by
  `tests/hooks.rs:the_notification_still_goes_out_while_moshi_holds_the_card_but_not_to_the_phone`
  asserting `fired("hermes")` and `!fired("mobile")`.
- Failure sources: a forward that never spawned. The suppression used to be applied to the INTENT to
  forward, so an away operator whose `moshi-hook` could not spawn lost the one notification still able to
  reach them (`src/main.rs:blocking_event`). Now `PNS_SKIP_PHONE=1` is set only inside the
  `forwarded.is_some()` branch, and
  `tests/hooks.rs:moshi_not_being_installed_leaves_the_hook_a_silent_exit_zero` asserts `fired("mobile")`
  on that path.
- Fail direction: fail toward telling the operator. When in doubt the card goes out.
- Thresholds: Not applicable.
- Required side effects: the ORDER is the behavior. The forward's spawn is first and nothing may sit in
  front of it; `arm_nag` is second, so the nag clock starts at the true prompt time and a notification
  that dies still leaves a timer armed; `run_event` is third; the bounded wait is last
  (`src/main.rs:blocking_event`). The card's own content is state `blocked`, project taken as the last
  non-empty segment of the payload's `cwd` (`src/main.rs:project_of`), detail from the payload's message
  chain, and pane from `HERDR_PANE_ID` so a tap lands on the pane that is waiting, pinned by
  `tests/hooks.rs:a_blocked_hook_cards_the_operator_as_blocked_and_says_what_was_asked`. A real
  `PermissionRequest` states no `message`, so the detail falls through `src/hooks.rs:parse_payload`'s
  chain to the tool request: the recorded detail is `Bash: command=rm -rf /tmp/x` for Claude Code, pinned
  by `tests/hooks.rs:a_real_claude_approval_cards_the_tool_that_wants_to_run`, and
  `shell: command=bash -lc rm -rf build` for Codex, pinned by
  `tests/hooks.rs:a_codex_approval_is_submitted_as_codex_hook_and_names_the_tool_that_wants_to_run`. An
  unparseable payload names no tool and the detail is the empty string, because inventing one would be
  worse (`tests/hooks.rs:a_payload_pns_cannot_parse_is_still_submitted_verbatim`).
- Forbidden side effects: `PNS_SKIP_PHONE` is set in THIS PROCESS ONLY. The nag fire is a different
  process minutes later that never inherits it, so the nudge reaches the phone the first card was
  suppressed from, deliberately (`src/main.rs`, line 4500). Suppression must not be applied by the
  delivery plan, because the card moshi is raising is something the surface model cannot know about.
- Timeout and cancellation: inherited from behaviors 5 and 7.
- Idempotency and duplicates: one event, one notification.
- Privacy: the card carries the operator's own text. The detail fields that reach a rendered line are
  flattened by `src/hooks.rs:parse_payload`.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: none, this is pns's own notification.

### 9. The blocking hook writes nothing the harness could read as a decision

Given Claude Code parses a `PermissionRequest` hook's STDOUT and decides off
`hookSpecificOutput.decision` alone

When `pns hook blocked` runs to completion

Then pns prints exactly zero bytes to stdout on that path.

- Success: `tests/hooks.rs:the_blocked_hook_writes_nothing_the_harness_would_read_as_a_decision` asserts
  the printed stdout is exactly the empty string, not merely that it does not start with a brace.
- Failure sources: a build that starts printing on this path. `Delivery::line_for` yields a line only
  under `ReportMode::ReportOutcome`, `channel_plan` selects that mode only for `--remote-only`, and no
  hook path sets it, so every leg on this path is silent.
- Fail direction: fail-closed on stdout. Anything here would be a SECOND SUBMITTER by another name,
  deciding a question moshi has already put in front of the operator, and it would be invisible: the card
  still arrives, the submission still happens, and the harness acts on pns's answer instead of the
  operator's.
- Thresholds: Not applicable.
- Required side effects: none. The assertion is deliberately exactly-empty rather than a first-character
  test, because the harness reads through JavaScript `trim()`, which strips U+FEFF, while Rust's
  `trim_start` does not: a first-character test spelled in Rust would pass a byte-order-mark in front of
  a valid `allow` object that Claude Code accepts.
- Forbidden side effects: printing anything at all.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: Not applicable.
- Privacy: Not applicable.
- Process ownership and cleanup: the child's stdout is INHERITED, so moshi's own stdout is the hook's
  stdout. That is the one thing that may legitimately appear there, see behavior 7.
- Compatibility contract: yes, and it is the load-bearing one on this path. Stdout is a live channel that
  Claude Code parses, so pns's silence there is a contract with a harness we do not control.

### 10. The blocked state markers

Given the lamps need to know which sessions are waiting on the operator, and the tick is the only sweeper

When a blocked event runs through `run_event` with `Attempt::First`

Then one marker file per waiting session is published, and a later event from that session removes it.

- Success: `src/main.rs:update_blocked_marker` takes `Action::Start` for a state in
  `src/pulse.rs:LAMP_BLOCKED` (`blocked`, `asked`, `plan-ready`, `denied`, `asking`) and writes the
  decision's own clock reading into a file named for the session under `src/lights.rs:blocked_dir`,
  resolved by `src/lights.rs:blocked_marker`. Every other state is `Action::End` and removes the file
  (`src/lights.rs:blocked_marker_action`).
- Failure sources: a session id that cannot become a filename is refused by
  `src/safety.rs:session_id_is_safe` and no marker is written; no clock reading means no marker, never a
  marker at epoch zero, because the bound that expires an abandoned wait is measured against that number.
- Fail direction: fail toward the lamp going dark rather than staying lit. A closed set of STARTERS and
  everything else ENDS, so an unrecognized state word ends a wait rather than holding blue on a session
  nobody is waiting for (`src/lights.rs:blocked_marker_action`).
- Thresholds: STARTING a wait rides behind the `[lights]` table AND an enabled `[plugins.hue]` table, the
  `lamps_live` condition in `src/main.rs:run_event`, because a machine that never asked for the lamps
  must not start accumulating files nothing would sweep. ENDING one is unconditional, because a wait that
  ended while the lamps were off would otherwise keep its marker and put blocked on a lamp for a session
  nobody was waiting on.
- Required side effects: `src/main.rs:end_blocked_wait` is called by exactly two arms of
  `src/main.rs:hook_mode`: `prompt`, because the operator typing answers any live wait their session
  could be holding, and `resolved`, guarded on `!payload.in_subagent` because a batch carrying an
  `agent_id` key resolved a subagent's tool and not the parent's own wait. Beside the marker, `run_event`
  also writes the decision ring line and the news record that arms the `unread` lamp
  (`src/main.rs:record_news`). The decision ring line for a forwarded approval carries `claude/blocked`,
  `skip_phone=yes` and `mode=default agent=agent_01 tool=Bash`, pinned by
  `tests/hooks.rs:an_approval_that_was_submitted_is_recorded_and_is_never_journaled_as_missed` and
  `tests/hooks.rs:the_decision_log_carries_the_payloads_mode_agent_and_tool`.
- Forbidden side effects: an approval must NOT be journaled. A forwarded approval is not a missed
  notification, and replaying it later would put Allow and Deny in front of an operator for a prompt
  answered hours ago; the same test asserts the journal file does not exist. An approval must also leave
  the TURN MARKER alone, because the harness resumes the tool call and the turn ends later at the Stop
  that follows (`tests/hooks.rs:an_approval_leaves_the_turn_marker_alone`).
- Timeout and cancellation: the marker's own backstop is `[lights.blocked] give_up_after_secs`, which
  configuration refuses to set shorter than `[nag] after_secs`, because that is a configuration that
  gives up on a wait before it ever nudges about it (`src/main.rs:update_blocked_marker`,
  `src/config.rs:parse_config`).
- Idempotency and duplicates: one file per SESSION carries no generation, so an OLDER Stop can remove a
  NEWER wait's marker. That is a stated limit rather than a rule: unlink cannot arbitrate on this
  filesystem (concurrent unlink reports success to every caller on the Apple File System), and telling
  the two apart would need a generation inside the marker and a compare-and-swap publish over it. The
  damage is bounded by the backstop and closed by the session's next event.
- Privacy: state files hold session ids and clock readings, at mode 0600 like every other state file.
- Process ownership and cleanup: FAIL-QUIET. Every filesystem failure here is dropped, because an event
  path whose stdout a harness hook reads must not gain a line about the state directory, and a missing
  marker costs one lamp its colour and never a card.
- Compatibility contract: none, these files are pns's own.
- Related: the GATE writes no markers at all. `src/main.rs:gate_mode` calls neither
  `update_blocked_marker` nor `run_event`. NOT ESTABLISHED: no test asserts the absence of a marker after
  a gate run specifically; I grepped `tests/hooks.rs` for marker assertions in the gate section and found
  only the "no event raised" assertions cited in behavior 1.

### 11. The nag armed with the wait

Given an approval nobody answers should be nudged once, and the clock should start at the true prompt
time

When `src/main.rs:blocking_event` runs, after the spawn and before the notification

Then `src/main.rs:arm_nag` publishes a record for that session, and clears any previous approval's
answered marker first.

- Success: `arm_nag` writes a record holding agent, project, branch, detail, pane and the arming time.
  `tests/hooks.rs:an_unanswered_approval_is_nudged_once_through_the_ordinary_paths` and
  `tests/hooks.rs:three_unanswered_approvals_produce_one_card_that_says_three` drive the fire.
- Failure sources: no clock reading, an unsafe session id, or a schedule that could not be created. The
  last is pinned by
  `tests/hooks.rs:an_approval_whose_nudge_could_not_be_scheduled_leaves_no_record_behind`.
- Fail direction: fail toward not nudging. A record whose arming time nothing could read would be judged
  stale on the first fire anyway, so not writing it is the same answer one step earlier.
- Thresholds: `[nag] after_secs`, with `src/main.rs:NAG_OFF` (zero) meaning the nag is off and nothing is
  armed.
- Required side effects: the answered marker is removed BEFORE the record is published, and the order is
  load bearing twice over. The marker name is constant per session, so one left by the previous approval
  would make the new job drop silently; and published first, the new record could be claimed by a
  concurrent fire that then finds the previous approval's marker and drops it as answered.
- Forbidden side effects: NO NAG ON CODEX, and the gate is positive (an agent that is not
  `src/main.rs:CLAUDE_AGENT` returns immediately) so an empty or unknown `PNS_AGENT` arms nothing either.
  Codex wires exactly Stop and PermissionRequest, so it has a turn-end clear and no batch-level one, and
  agent turns routinely run tens of minutes: a Codex nag would be wrong in the common case rather than at
  an edge (`src/main.rs:arm_nag`).
- Timeout and cancellation: the nudge is a separate process minutes later, see behavior 8's note on
  `PNS_SKIP_PHONE`.
- Idempotency and duplicates: one card whatever the count. Three waiting approvals produce ONE nudge card
  that says three, which is the structural rate limit of at most one nudge per `after_secs`.
- Privacy: the record holds the same operator text the card does.
- Process ownership and cleanup: arming writes nothing the harness could read as a decision, pinned by
  `tests/hooks.rs:arming_writes_nothing_the_harness_could_read_as_a_decision`.
- Compatibility contract: none.

### 12. One prompt, one submission

Given a second submission would be a second card and a second answer to a question the operator was asked
once

When either entry point forwards

Then exactly one `moshi-hook` child is spawned for that prompt, however the wait ends.

- Success: the hook path submits exactly one `claude-hook`, pinned by
  `tests/hooks.rs:one_prompt_is_submitted_exactly_once_and_a_zero_answer_from_it_is_an_approve`, and the
  gate submits exactly one `pi-hook`, pinned by
  `tests/hooks.rs:the_gate_submits_one_prompt_exactly_once`.
- Failure sources: a retry after an expiry, or a design that made moshi "just another channel" so that
  high-volume events swept into the submission path.
- Fail direction: fail toward NOT submitting. Two submissions is the direction that cannot be undone.
- Thresholds: an expiry submits nothing further, asserted inside both bound tests of behavior 7.
- Required side effects: none.
- Forbidden side effects: no non-blocking event may ever spawn a submission. `stop`, `stop-failure`,
  `asked`, `plan-ready` and `denied` are all pinned with the stub as a tripwire, run AWAY so the presence
  gate would not decline for the wrong reason: `tests/hooks.rs:an_ordinary_stop_never_reaches_moshi`,
  `tests/hooks.rs:a_failed_turn_never_reaches_moshi`,
  `tests/hooks.rs:a_non_blocking_event_never_pays_for_the_round_trip` and
  `tests/hooks.rs:a_denial_never_pays_for_the_approval_round_trip_and_still_exits_zero`. A denial in
  particular is terminal news and not a question: the decision has already been taken, and a card
  offering Allow and Deny would be answering a closed question no prompt is listening to.
- Timeout and cancellation: see behavior 7.
- Idempotency and duplicates: this behavior IS the duplicate rule. The counting helper
  `tests/hooks.rs:submissions` appends one line per spawn, so a second submitter is visible rather than
  hidden behind a last-write-wins record.
- Privacy: Not applicable.
- Process ownership and cleanup: see behavior 6.
- Compatibility contract: single-submitter is a rule about the PROMPT and not about one entry point,
  which is why both entry points are counted separately.

### 13. The approval card

Given the operator answers from their phone

When the forward succeeds

Then moshi raises the actionable card itself from the forwarded payload, and pns's own mobile leg is the
one that is suppressed.

- Success: moshi mints the card's action identifier inside itself and answers pns with an exit code. The
  `skip_phone=yes` field in the decision ring line is THE ONLY TRACE of a forward anywhere in pns's
  records, pinned by
  `tests/hooks.rs:an_approval_that_was_submitted_is_recorded_and_is_never_journaled_as_missed`.
- Failure sources: on the paths where pns DOES raise a phone card (no forward started), the mobile leg
  can still fail. `src/channels/moshi.rs:MoshiChannel::deliver` returns "push SKIPPED -- no moshi token
  in the config ([plugins.mobile] token); nothing was sent" for a missing or empty token, and "push
  FAILED (the moshi endpoint refused it or could not be reached)" for a non-2xx or unreachable endpoint.
  `src/channels/moshi.rs:refused_backend_line` wraps a `type` fault as "push SKIPPED -- {reason}; nothing
  was sent".
- Fail direction: a failed push costs a card and nothing else; no event hears the verdict, because
  `ReportOutcome` is produced only under `--remote-only` and this plugin is not durable.
- Thresholds: `src/channels/moshi.rs:POST_DEADLINE` is 10 seconds for one post. Nobody waits on the
  answer and nothing is retried, so it only bounds how long the process lingers. This is a DIFFERENT
  deadline from the submission bound in behavior 7 and must not be confused with it.
- Required side effects: the card body carries `token`, `title` and `message` (the preview, because the
  phone card has a length ceiling the full message ignores), plus an optional `data` object holding
  `type` of `url` and the deep link `moshi://herdr?pane=<pane>` (`src/channels/moshi.rs:webhook_body`,
  `src/channels/moshi.rs:herdr_link`). The deep link is a DECORATION, so no pane means no action and the
  card ships exactly as it does without one.
- Forbidden side effects: the link must be well-formed by construction, because a malformed action does
  not degrade the card, it DELETES it (moshi answers a bad body non-2xx and this channel reads any
  non-2xx as a failed delivery). The guard is `src/safety.rs:pane_is_safe`, asked at `herdr_link` rather
  than assumed of the caller, and its character set (ASCII alphanumeric plus `.`, `_`, `:` and `-`) is
  legal unencoded in a query value, which leaves nothing to escape. One `data` object carries one `type`,
  which is a structural limit of the field and is what makes a url action and an image action mutually
  exclusive.
- Timeout and cancellation: the post follows no redirects (`max_redirects(0)`), because following one
  would send the token to whatever host the endpoint names.
- Idempotency and duplicates: one post, no retry.
- Privacy: THE SECRET'S PATH IS THE POINT. The token is read from the `[plugins.mobile]` table's `token`
  key, placed in the request BODY, and never touches argv, the environment of a child, or an error string
  (module documentation of `src/channels/moshi.rs`). The delivery verdict says whether the push landed
  and never what it carried.
- Process ownership and cleanup: Not applicable, the post is in-process.
- Compatibility contract: yes. `src/channels/moshi.rs:DEFAULT_MOSHI_URL` is
  `https://api.getmoshi.app/api/webhook`, overridable with `PNS_MOSHI_URL`; the body shape is moshi's;
  the deep-link scheme `moshi://herdr?workspace=&tab=&pane=&session=` is moshi's, with tab and pane
  available since moshi 3.13.0; and a tap resumes a card moshi ALREADY HOLDS. It looks for an active card
  matching server session and workspace, else resumes the most recently minimized card for that session,
  and with no card matching at all it shows an error rather than opening a connection.

### 14. What the round trip is NOT

Given a comment in this crate once claimed the exit code was the operator's decision, which sent one
whole slice off designing against a wait that does not exist

When reading any of the above

Then the exit code is moshi's acknowledgement of a SUBMISSION, never the operator's answer.

- Success: measured 2026-08-29 against `moshi-hook 0.3.3`, every reply shape the daemon can send ends the
  wait with exit 0 and empty stdout, so approve and deny are indistinguishable at this seam
  (`src/main.rs:moshi_decision`). The operator's real answer travels the daemon's own terminal interface
  bridge, which finds the pane, screen-reads the numbered menu and SENDS KEYS into it.
- Failure sources: Not applicable, this behavior is a statement of what the mechanism is.
- Fail direction: Not applicable, no decision is taken here.
- Thresholds: `src/main.rs:answer_within` records the measurement that motivated the bound, 90 seconds
  and still climbing against a listener that accepted the connection and never replied. That is a daemon
  that stopped answering, not an operator taking their time.
- Required side effects: Not applicable.
- Forbidden side effects: pns must not invent a decision. Inventing one here would put pns's own word
  into a channel that is moshi's (doc comment on `src/main.rs:hook_mode`).
- Timeout and cancellation: see behavior 7.
- Idempotency and duplicates: see behavior 12.
- Privacy: Not applicable.
- Process ownership and cleanup: see behaviors 6 and 7.
- Compatibility contract: this is the reason every forwarded code is passed through untouched. The
  harnesses that read a gate's exit code are entitled to whatever moshi said, and pns has no standing to
  edit it.
