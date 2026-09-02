# Harness hook compatibility

`pns hook <event>` is the entry point a coding harness (Claude Code or Codex) calls when something
happens in a session. The harness writes one JavaScript Object Notation (JSON) object on standard input
and pns runs exactly one event path for it. Eleven event words are served: `prompt`, `stop`,
`stop-failure`, `blocked`, `asked`, `plan-ready`, `denied`, `resolved`, `model-switch`, `quota` and
`config-change`. This file states, per event, what is read off the payload, what state is mutated, what
state is cleared, what reaches standard output and standard error, and what the exit code means. Three
properties hold across every event and are stated once rather than per row: the payload is bounded in
bytes and in time before any arm sees it, a missing or malformed field is a state and never an error, and
the process exits zero on every path except the forwarded blocking one. **Deferred to
`docs/specs/blocking-approval.md`:** the whole forwarded round trip behind `blocked` and behind
`pns gate <harness>-hook`, that is `blocking_event`, `gate_mode`, `moshi_decision` and `answer_within`,
including the phone suppression, the submit deadline and the pass-through exit code. What this file keeps
of `blocked` is only what it shares with its siblings: the payload contract, the size cap that decides
whether it may be forwarded at all, the turn marker it must not touch, and its standard output contract.

## The eleven events

| Event word      | Reads from the payload                                                                                         | State it mutates                                                                                                                                                                     | State it clears                                                                                                                                                                     | Exit code                                    | Tests that pin it                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| --------------- | -------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `prompt`        | `session_id`                                                                                                   | writes the turn start marker when none exists (`start_of_turn`)                                                                                                                      | this session's wait marker (`end_blocked_wait`)                                                                                                                                     | 0                                            | `the_first_prompt_of_a_turn_writes_a_marker_and_a_later_one_does_not_reset_it`, `a_prompt_from_a_waiting_session_ends_its_wait`, `a_prompt_ends_only_its_own_sessions_wait`, `a_prompt_naming_a_traversal_removes_nothing`, `the_prompt_hook_clears_a_stale_quota_marker`, `a_payload_with_no_session_id_is_a_silent_no_op`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `stop`          | `session_id`, `cwd`, `last_assistant_message`, `transcript_path`                                               | the whole `Attempt::First` tail: decision ring, journal, activity ring, unread news record, loop lease, presence edge, lights tick lease, pulse; wait marker per the condensed state | the turn start marker (claimed by rename), the nag record (an answered marker is written first), and the wait marker whenever the condensed state is not one of the five wait words | 0                                            | `stopping_consumes_the_marker_so_a_second_stop_cannot_re_fire_the_tier`, `a_second_stop_cannot_re_fire_the_tier_because_the_marker_is_claimed_once`, `a_condenser_line_is_used_state_and_all_and_a_blank_summary_falls_back`, `an_ordinary_stop_never_reaches_moshi`, `a_stale_quota_marker_clears_at_the_turns_stop_without_any_prompt_hook`, `an_answered_approval_is_never_nudged_by_either_clearing_signal`                                                                                                                                                                                                                                                                                                                                                                                                           |
| `stop-failure`  | `session_id`, `cwd`, and the message chain (in practice `error`)                                               | the same `Attempt::First` tail as `stop`, with state `failed`                                                                                                                        | the turn start marker, the nag record                                                                                                                                               | 0                                            | `a_turn_that_died_notifies_as_failed_and_says_what_killed_it`, `a_dead_turn_consumes_the_marker_so_the_next_turn_is_not_measured_from_its_start`, `a_dead_turn_spawns_no_condenser_and_reads_no_transcript`, `a_long_turn_that_died_still_earns_its_pulse`, `a_failed_turn_never_reaches_moshi`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `blocked`       | the raw payload bytes, plus `session_id`, `cwd`, the message chain, `permission_mode`, `agent_id`, `tool_name` | arms the nag record, starts this session's wait marker, runs the full `Attempt::First` tail                                                                                          | nothing; the turn start marker is deliberately untouched                                                                                                                            | moshi's own code when forwarded, otherwise 0 | `an_approval_leaves_the_turn_marker_alone`, `the_blocked_hook_writes_nothing_the_harness_would_read_as_a_decision`, `the_decision_log_carries_the_payloads_mode_agent_and_tool`; the forward itself is `docs/specs/blocking-approval.md`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `asked`         | `session_id`, `cwd`, the message chain                                                                         | full `Attempt::First` tail; starts this session's wait marker                                                                                                                        | nothing; the turn start marker is untouched                                                                                                                                         | 0                                            | `an_mcp_server_waiting_on_input_notifies_as_asked_and_names_the_server`, `the_hook_writes_nothing_the_harness_could_read_as_an_answer_and_exits_zero`, `a_non_blocking_event_never_pays_for_the_round_trip`, `a_waiting_agent_leaves_a_marker_and_the_next_event_from_that_session_removes_it`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `plan-ready`    | `session_id`, `cwd`, the message chain                                                                         | full `Attempt::First` tail; starts this session's wait marker                                                                                                                        | nothing                                                                                                                                                                             | 0                                            | NOT ESTABLISHED (see behavior 20)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `denied`        | `session_id`, `cwd`, `tool_name`, `tool_input` (through the message chain)                                     | full `Attempt::First` tail; starts this session's wait marker                                                                                                                        | nothing; the turn start marker is untouched                                                                                                                                         | 0                                            | `a_refused_tool_call_notifies_as_denied_and_says_which_tool_was_refused`, `a_refused_tool_call_leaves_the_turn_marker_alone`, `a_denial_never_pays_for_the_approval_round_trip_and_still_exits_zero`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| `resolved`      | `session_id`, and whether the `agent_id` key is present at all                                                 | writes this session's answered nag marker                                                                                                                                            | the nag record; this session's wait marker, only when the payload carries no `agent_id` key                                                                                         | 0                                            | `a_resolved_batch_with_no_agent_id_ends_its_sessions_wait`, `a_resolved_batch_carrying_an_agent_id_leaves_the_parents_wait_lit`, `a_resolved_batch_with_a_malformed_agent_id_still_leaves_the_parents_wait_lit`, `an_answered_approval_is_never_nudged_by_either_clearing_signal`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `model-switch`  | `source`, `from_model`, `to_model`, `session_id`, `cwd`                                                        | a decision ring line only, and only when `source` is `auto` and the two names differ once rendered plainly                                                                           | nothing                                                                                                                                                                             | 0                                            | `an_observation_still_delivers_and_is_logged`, `an_auto_switch_between_equal_names_delivers_nothing`, `an_auto_switch_missing_a_model_name_delivers_nothing`, `an_auto_switch_strips_a_unicode_format_character_from_the_name`, `a_non_auto_model_switch_source_delivers_nothing_and_writes_nothing`, and the seven `an_observation_*` tests                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| `quota`         | `notification_type`, `message`, `session_id`, `cwd`                                                            | a decision ring line; `quota_auto_resume_stale` additionally starts this session's wait marker                                                                                       | nothing                                                                                                                                                                             | 0                                            | `quota_auto_resume_fired_delivers_one_card_naming_itself`, `quota_auto_resume_stale_delivers_one_card_naming_itself`, `quota_auto_resume_disabled_delivers_one_card_naming_itself`, `a_quota_notification_carrying_no_message_still_names_what_happened`, `an_unrecognised_notification_type_delivers_nothing`, `quota_auto_resume_stale_arms_the_needs_marker_for_its_own_session`, `a_stale_wait_arms_the_needs_marker_before_the_card_is_delivered`, `quota_auto_resume_fired_and_disabled_arm_no_needs_marker`, `every_quota_type_is_logged_as_an_observation_with_no_nag`, and the seven `no_quota_type_*` / `a_quota_observation_*` tests                                                                                                                                                                           |
| `config-change` | `source`, `file_path`, `session_id`, `cwd`                                                                     | a decision ring line; `policy_settings` additionally appends one line to the bounded policy-settings audit trail                                                                     | nothing                                                                                                                                                                             | 0                                            | `each_config_change_source_delivers_one_card_naming_itself_and_its_file`, `a_config_change_with_no_file_names_only_the_source`, `config_change_events_each_deliver_their_own_card_with_no_once_ever_guarantee`, `a_hostile_file_path_is_sanitised_before_it_reaches_the_card`, `an_unrecognised_config_source_delivers_nothing_and_writes_nothing`, `a_policy_settings_change_is_recorded_to_a_bounded_audit_trail`, `a_non_policy_config_change_writes_no_policy_audit_entry`, `the_policy_settings_audit_trail_is_bounded_and_drops_the_oldest_entry`, `an_enormous_file_path_cannot_wipe_the_policy_audit_trail`, `a_newline_in_a_file_path_cannot_forge_a_policy_audit_entry`, `an_arabic_letter_mark_in_a_file_path_reaches_neither_the_card_nor_the_audit_trail`, and the six `a_config_change_*` observation tests |

The five words that START a wait marker are `pulse::LAMP_BLOCKED`: `blocked`, `asked`, `plan-ready`,
`denied`, `asking` (`src/lights.rs:blocked_marker_action`). Every other event state from a session ENDS
that session's wait. `asking` is on the list because a `stop` whose condenser verdict is `asking` becomes
an event with that state word.

______________________________________________________________________

## 1. The hook subcommand is reached by exactly one argv shape

Given a `pns` invocation When the first argument after the program name is the literal word `hook` Then
the second argument is taken as the event word and `hook_mode` runs it; every other leading word is
dispatched elsewhere by `main`, and a word naming no command at all is refused with the usage text on
standard error and exit 2.

- Success: `pns hook stop` reaches `hook_mode("stop")` (`src/main.rs:main`). The usage text names all
  eleven words verbatim: "pns hook <event> a harness hook: prompt, stop, stop-failure, blocked, asked,
  plan-ready, denied, resolved, model-switch, quota, config-change" (`src/main.rs:USAGE`).
- Failure sources: a missing second argument (`second_argument` yields the empty string), a misspelled
  event word.
- Fail direction: open, then quiet. `hook_mode` still reads standard input first, then falls to the
  catch-all arm and prints one line to standard error, notifying nobody
  (`tests/hooks.rs:a_hook_word_this_binary_does_not_serve_says_so_and_notifies_nobody`).
- Thresholds: Not applicable, this is a word match rather than a number.
- Required side effects: none beyond the standard-input read.
- Forbidden side effects: an unserved word must not fall through to the nearest arm. `stop-failed` is one
  letter from `stop-failure` and must not report a dead turn
  (`tests/hooks.rs:a_hook_word_this_binary_does_not_serve_says_so_and_notifies_nobody`).
- Timeout and cancellation: the payload read still runs and is still bounded, so `pns hook` with no event
  word blocks for at most the payload deadline before printing its refusal.
- Idempotency and duplicates: Not applicable, argv dispatch takes no state.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable, no child is spawned on this path.
- Compatibility contract: the eleven words are the whole vocabulary. Adding a harness event means adding
  an arm; until then the harness hears one stderr line and keeps working.

## 2. The payload is bounded in bytes before any arm sees it

Given a harness writing a payload on standard input When `read_payload` runs Then at most
`MAX_PAYLOAD_BYTES + 1` bytes are read, and `payload_is_whole` reports whether what arrived is the bytes
the harness actually sent.

- Success: any payload of 1,000,000 bytes or fewer is read whole and is whole
  (`tests/hooks.rs:a_payload_at_the_cap_is_whole_and_is_still_submitted`, which sends exactly 1,000,000
  bytes and asserts the test's own arithmetic).
- Failure sources: a harness sending more than the cap; a payload that is not valid UTF-8 (Unicode
  Transformation Format, 8-bit).
- Fail direction: closed for forwarding, open for notifying. An over-cap payload is cut mid-object, so it
  is never handed on, but the notification still goes out carrying whatever an unparseable payload yields
  (`src/main.rs:payload_is_whole`,
  `tests/hooks.rs:a_payload_too_large_to_be_whole_is_never_forwarded_as_though_it_were`). Invalid UTF-8
  is the harder direction: `read_payload` reads a `String`, so the read fails, `hook_mode` returns 0
  having done nothing, and the operator hears nothing at all, which is a known limit pinned on purpose
  (`tests/hooks.rs:a_payload_that_is_not_utf8_drops_the_approval_and_tells_the_operator_nothing`).
- Thresholds: `MAX_PAYLOAD_BYTES` is 1,000,000. At exactly 1,000,000 bytes the payload is whole
  (`payload_json.len() <= MAX_PAYLOAD_BYTES as usize`). At 1,000,001 bytes the reader's
  `take(MAX_PAYLOAD_BYTES + 1)` returns 1,000,001 bytes, the comparison fails, and the payload is not
  whole. The one extra byte exists precisely so "reached the cap" and "hit the cap" are distinguishable
  (`src/main.rs:read_payload`).
- Required side effects: none.
- Forbidden side effects: a truncated payload must never be forwarded as though it were the harness's own
  bytes.
- Timeout and cancellation: see behavior 3.
- Idempotency and duplicates: Not applicable, one read per process.
- Privacy: the payload holds tool inputs and, on `Elicitation`, a Model Context Protocol server's own
  prompt, which can name a credential. It is never written to disk by the read itself.
- Process ownership and cleanup: the reader runs on a spawned thread which outlives a refusal. That is
  accepted: the process is about to exit and the thread holds nothing but its own buffer
  (`src/main.rs:read_payload`).
- Compatibility contract: 1 MB is a ceiling on a "small JSON object", not a negotiated limit. A harness
  that sends more gets a notification and no forward.

## 3. The payload is bounded in time before any arm sees it

Given a harness that opened the standard-input pipe and never finished writing When `read_payload` waits
on its reader thread Then the wait expires at `payload_deadline()` and the hook returns 0 having
delivered nothing.

- Success: a payload written and the pipe closed inside the window yields `Some(payload)`.
- Failure sources: a harness that opens the pipe and stalls; a harness killed mid-write.
- Fail direction: closed and silent. No payload is no notification, and still exit 0
  (`src/main.rs:hook_mode`,
  `tests/hooks.rs:a_payload_nobody_finishes_writing_still_exits_on_the_contract`, which also asserts
  nothing is sent on a guess).
- Thresholds: 5 seconds by default (`payload_deadline`). `PNS_PAYLOAD_DEADLINE_MS` overrides it in
  milliseconds through `env_deadline`, and the test drives it at 200 ms. A value that does not parse as
  milliseconds falls back to the 5 second default rather than to no bound.
- Required side effects: none.
- Forbidden side effects: the hook must not hang. Hanging here would park the harness turn before any
  part of the exit contract could run, which is the regression the deadline exists for.
- Timeout and cancellation: `receiver.recv_timeout(payload_deadline())`; on expiry the reader thread is
  abandoned rather than joined.
- Idempotency and duplicates: Not applicable.
- Privacy: Not applicable.
- Process ownership and cleanup: no child process; one abandoned thread, argued at behavior 2.
- Compatibility contract: generous on purpose, because a harness writing a large transcript path is
  normal and a hang is not (`src/main.rs:payload_deadline`).

## 4. Every payload field is optional and a bad payload is an empty one

Given any payload text When `parse_payload` decodes it Then each field is read as a string if present and
as the empty string otherwise, and a document that will not parse yields `HookPayload::default()`.

- Success: a payload naming `session_id`, `cwd`, `transcript_path`, `last_assistant_message` and
  `agent_id` yields all five (`src/hooks.rs:parse_payload`,
  `src/hooks.rs:a_payload_yields_every_field_the_hooks_read`).
- Failure sources: a field the harness does not send for this event; a field of the wrong JSON type; a
  payload that is not JSON at all.
- Fail direction: open and quiet. Every field is optional "because every harness sends a different subset
  and a missing field is a state, never an error: this runs on a path that must exit 0"
  (`src/hooks.rs:HookPayload`). `parse_payload("not json")` and `parse_payload("")` both equal the
  default (`src/hooks.rs:a_payload_that_will_not_parse_is_empty_rather_than_fatal`), and the whole hook
  still exits 0 for `""`, `"not json"` and `{"session_id":null}`
  (`tests/hooks.rs:nothing_that_goes_wrong_building_a_notification_fails_the_harness_turn`).
- Thresholds: Not applicable.
- Required side effects: none, decoding is pure (`src/hooks.rs` module doc: "Everything here is PURE").
- Forbidden side effects: a decode must not guess. `permission_mode`, `agent_type` and `tool_name` are
  absent rather than filled in
  (`src/hooks.rs:permission_mode_agent_type_and_tool_name_are_absent_rather_than_guessed`), and
  `notification_type` is absent off a non-notification event
  (`src/hooks.rs:notification_type_is_absent_rather_than_guessed_off_a_non_notification_event`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure, so repeated decoding of the same text is identical.
- Privacy: Not applicable at this layer; see behaviors 6 through 8.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the seventeen fields on `HookPayload` are a superset across harnesses. A
  harness that adds a key pns does not read costs nothing; one that stops sending a key pns reads
  degrades that key to its absent state.

## 5. A present `agent_id` key marks a subagent, whatever its value

Given a payload When `parse_payload` sets `in_subagent` Then it records whether the `agent_id` KEY was
present at all, never the string's shape.

- Success: `"agent_id":"agent_01"` marks a subagent; a payload with no `agent_id` key does not
  (`src/hooks.rs:a_present_agent_id_of_any_shape_marks_a_subagent_and_absence_does_not`).
- Failure sources: a key present but null, numeric or empty.
- Fail direction: closed. All three shapes still mark a subagent, so `resolved` clears nothing on them.
  The hooks reference promises only ABSENCE on the main thread, so a malformed field is not proof of the
  main thread (`src/hooks.rs:HookPayload::in_subagent`,
  `tests/hooks.rs:a_resolved_batch_with_a_malformed_agent_id_still_leaves_the_parents_wait_lit`). An
  unparseable payload is not a subagent either, since there is no key.
- Thresholds: Not applicable.
- Required side effects: none.
- Forbidden side effects: `resolved` must read `in_subagent` and never the `agent_id` string
  (`src/main.rs:hook_mode`, the `resolved` arm).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: Not applicable.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the residual is stated honestly in the source: a parent session's wait marker
  stays lit until its own Stop when a subagent's batch resolves (`src/main.rs:hook_mode`,
  `tests/hooks.rs:a_resolved_batch_carrying_an_agent_id_leaves_the_parents_wait_lit`).

## 6. One `message` is composed from four payload roads, in a fixed order

Given a payload from any harness When `parse_payload` builds `message` Then the first non-empty of
`elicitation_request`, flattened `message`, flattened `detail`, `reported_error` wins, and `tool_request`
is the fallback when all four say nothing.

- Success: `{"message":"m"}` yields `m`; `{"detail":"d"}` yields `d`; `{"message":"","detail":"d"}`
  yields `d` (`src/hooks.rs:detail_stands_in_for_message_because_the_harnesses_disagree`). A Codex
  `PermissionRequest` carrying only `tool_name` and `tool_input` yields
  `shell: command=bash -lc rm -rf build`
  (`src/hooks.rs:a_codex_permission_request_says_which_tool_wants_what`). A Claude Code `StopFailure`
  carrying only `error` yields `API Error: 500 internal server error`
  (`src/hooks.rs:a_dead_turns_error_becomes_the_message_when_the_payload_states_nothing_else`). An
  elicitation yields `composio: Please authorize Gmail access`
  (`src/hooks.rs:an_elicitation_says_which_server_is_asking_in_front_of_what_it_asked`).
- Failure sources: a payload naming none of the four; a value that is JSON null; a value that flattens to
  nothing.
- Fail direction: quiet. A payload naming no tool and no message yields the empty string rather than a
  guess, and both `{"error":""}` and `{"error":null}` are nothing said
  (`src/hooks.rs:a_payload_naming_no_tool_and_no_message_still_says_nothing_rather_than_guessing`). A
  `message` that is nothing but control bytes says nothing, so the chain moves on
  (`src/hooks.rs:every_payload_string_a_card_is_built_from_is_scrubbed_and_not_the_arguments_alone`).
- Thresholds: `elicitation_request`, `tool_request` and `reported_error` each keep the first
  `TOOL_REQUEST_MAX_CHARS` = 320 characters. At 320 characters nothing is cut; at 321 the tail is
  dropped. The cut keeps the HEAD, because a tool names itself first and an application programming
  interface error states its kind first
  (`src/hooks.rs:an_error_is_kept_to_one_line_and_cut_from_the_head_like_a_tool_request`, which asserts
  the count is exactly 320 rather than merely under a cap). The plain `message` and `detail` reads are
  flattened but NOT capped here; their cap is the reply cap downstream.
- Required side effects: none, this is pure.
- Forbidden side effects: a stated `message` or `detail` must never be second-guessed, so `error` sits
  BEHIND them in the chain rather than in front
  (`src/hooks.rs:a_stated_message_or_detail_still_outranks_an_error`,
  `src/hooks.rs:a_payload_that_states_its_own_message_is_never_second_guessed`). A JSON null must never
  flatten to the word "null" on a card (`src/hooks.rs:elicitation_request`,
  `src/hooks.rs:reported_error`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure.
- Privacy: this composed line is what reaches a banner, a herdr pane and a Discord post. The elicitation
  road exists so the operator can see WHICH connected Model Context Protocol server wants a credential,
  rather than an unattributed "Please provide your API key" (`src/hooks.rs:elicitation_request`).
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `mcp_server_name` appears in only two hook input schemas in the whole 2.1.241
  vocabulary and Codex 0.149.1 sends it on nothing, so `elicitation_request` returns the empty string for
  every payload pns handles today (`src/hooks.rs:elicitation_request`).

## 7. Every payload string a card is built from is flattened to one line and scrubbed of control bytes

Given a payload string destined for a rendered line When `flattened` runs over it (directly, or through
`one_line` for nested values) Then runs of whitespace AND of control characters each become a single
space and the ends are trimmed.

- Success: `a\nb\tc  d` becomes `a b c d`; `a\u{1b}[31mb` becomes `a [31mb`; `a\u{1b}]0;title\u{7}b`
  becomes `a ]0;title b`
  (`src/hooks.rs:every_class_of_control_byte_is_scrubbed_before_a_line_reaches_a_channel`).
- Failure sources: an escape sequence in a provider-written `error` string; a newline in a tool name a
  connected Model Context Protocol server registered under; a control byte in an object KEY.
- Fail direction: closed. The scrub is written as one category test over `char::is_control`, which is
  exactly the Cc set (C0, DEL and C1), so the test walks every codepoint in `0x00..=0x1f`, `0x7f` and
  `0x80..=0x9f` rather than sampling. A `flattened` that let U+0002 through was measured to pass every
  other test in the crate while leaking that byte to a banner (`src/hooks.rs:flattened`, and the same
  test).
- Thresholds: the boundary is by Unicode category, never by codepoint range. `café`, `日本語`, `→ ✓ ×` and
  `naïve résumé ½ ±` pass through untouched, which is what a range test written in bytes would have
  broken (same test).
- Required side effects: none, pure.
- Forbidden side effects: scrubbing one half of a composed string is scrubbing neither. All four
  card-building strings go through it, `tool_name` and `tool_input` together and `message` and `detail`
  too, asserted in one array so a run names every field still riding through
  (`src/hooks.rs:every_payload_string_a_card_is_built_from_is_scrubbed_and_not_the_arguments_alone`). An
  object's key is scrubbed beside its value (`src/hooks.rs:one_line`).
- Timeout and cancellation: recursion in `one_line` is bounded by the parse that produced the value,
  since serde_json refuses a document nested deeper than its own limit (`src/hooks.rs:one_line`).
- Idempotency and duplicates: idempotent, a flattened string flattens to itself.
- Privacy: the motivating feeder is provider-controlled. `error` is whatever the application programming
  interface said, and an escape sequence in it is the one string on this path nobody on this machine
  wrote (`src/hooks.rs:flattened`,
  `src/hooks.rs:a_provider_error_carrying_an_escape_sequence_cannot_dress_up_a_card`).
- Process ownership and cleanup: Not applicable.
- Compatibility contract: paths, session ids and transcript paths are deliberately NOT flattened, because
  they are matched and opened rather than rendered, and flattening one would rewrite a name the
  filesystem gave (`src/hooks.rs:parse_payload`).

## 8. Two fields go through a stricter scrub than the rest

Given a model name on a `model-switch` event or a file path on a `config-change` event When it is
rendered Then `rendered_plainly` runs `flattened` and then strips every character `recap::is_invisible`
answers true for, which is the Unicode format (Cf) set that `flattened` leaves alone.

- Success: a right-to-left override inside a model name is gone from the card
  (`tests/hooks.rs:an_auto_switch_strips_a_unicode_format_character_from_the_name`), and so is one inside
  a file path (`tests/hooks.rs:a_hostile_file_path_is_sanitised_before_it_reaches_the_card`). U+061C
  ARABIC LETTER MARK is gone from both the card and the durable audit line
  (`tests/hooks.rs:an_arabic_letter_mark_in_a_file_path_reaches_neither_the_card_nor_the_audit_trail`).
- Failure sources: a format character in a name or path.
- Fail direction: closed, the character is removed rather than escaped.
- Thresholds: Not applicable, membership in `recap::is_invisible` decides.
- Required side effects: none.
- Forbidden side effects: this must NOT be folded into `flattened` itself. `flattened` is shared by every
  other rendered field, and widening it would let every field silently start allowing format characters
  through in the other direction (`src/main.rs:rendered_plainly`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: idempotent.
- Privacy: the config-change path writes the scrubbed path into a durable state file as well as a card,
  so an invisible character there would round-trip identically on every future read
  (`src/main.rs:rendered_plainly`).
- Process ownership and cleanup: Not applicable.
- Compatibility contract: two callers justify it, and both are named in the source: model name EQUALITY
  (a name that reads the same but compares unequal, or the reverse) and the durable config-change record.

## 9. Every ordinary hook exits zero

Given any of the ten non-forwarding events When `hook_mode` returns Then the process exits 0, whatever
went wrong building the notification.

- Success: every end-to-end test in `tests/hooks.rs` for `prompt`, `stop`, `stop-failure`, `asked`,
  `plan-ready`, `denied`, `resolved`, `model-switch`, `quota` and `config-change` asserts
  `status.code() == Some(0)` or `status.success()`.
- Failure sources: an unparseable payload, an unreadable transcript, a dead condenser, a garbage
  environment knob, an unserved event word.
- Fail direction: always zero. "Every path here is a notification, and a notification that cannot be
  delivered must never fail the turn it reports on, so every path returns 0" (`src/main.rs:hook_mode`).
  Pinned across four bad payloads and one unknown word by
  `tests/hooks.rs:nothing_that_goes_wrong_building_a_notification_fails_the_harness_turn`, and against
  six malformed interval values including `1e300`, which used to panic the `Duration` constructor and
  exit 101, by `tests/hooks.rs:a_malformed_reread_interval_falls_back_instead_of_panicking`.
- Thresholds: Not applicable.
- Required side effects: none.
- Forbidden side effects: a non-zero exit is a decision this hook has no business taking. Claude Code
  reads exit code 2 on an `Elicitation` hook as declining the elicitation outright, so the server would
  report a refusal the operator never made
  (`tests/hooks.rs:the_hook_writes_nothing_the_harness_could_read_as_an_answer_and_exits_zero`).
- Timeout and cancellation: every spawn on this path is bounded, so the exit is reached; see behaviors 3,
  15, 16 and 17.
- Idempotency and duplicates: Not applicable.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the ONE exception is the forwarded blocking path, where the code is moshi's own
  passed through untouched. That is not the operator's decision and is not how Claude Code answers a
  `PermissionRequest` either, which it decides off standard output (`src/main.rs:hook_mode`, and
  `docs/specs/blocking-approval.md`).

## 10. A hook writes nothing to standard output

Given any hook event When it completes Then standard output is empty in practice, and specifically
carries nothing a harness would parse as a decision.

- Success: `a_payload_with_no_session_id_is_a_silent_no_op` asserts standard output equals the empty
  string on `prompt`. `the_blocked_hook_writes_nothing_the_harness_would_read_as_a_decision` asserts
  EXACTLY empty on `blocked`, the one event where the harness reads that channel.
  `the_hook_writes_nothing_the_harness_could_read_as_an_answer_and_exits_zero` asserts the trimmed output
  does not begin with `{` on `asked`.
- Failure sources: a leg printed under `ReportMode::ReportOutcome`; the both-flags refusal line.
- Fail direction: silent. `Delivery::line_for` yields a line only under `ReportMode::ReportOutcome`
  (`src/channels/mod.rs:line_for`), and `channel_plan` selects that mode only for `--remote-only`
  (`src/routing.rs:channel_plan`). No hook arm sets `remote_only`, since every arm builds `EventArgs`
  with `..Default::default()`, so the `pns: {line}` print in `run_event` is unreachable from a hook. The
  `pns: post SKIPPED -- --local-only and --remote-only were both given, which suppresses every channel; nothing was sent`
  line needs both flags and is unreachable for the same reason.
- Thresholds: Not applicable.
- Required side effects: none.
- Forbidden side effects: pns must never print a `hookSpecificOutput` object. A measured mutation
  printing
  `{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"deny","message":"pns declined"}}}`
  at the end of the blocked path passes every other test in the crate and is killed only by that one row
  (`tests/hooks.rs:the_blocked_hook_writes_nothing_the_harness_would_read_as_a_decision`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: Not applicable.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the assertion on `blocked` is exactly-empty rather than a first-character test
  on purpose. The harness reads through JavaScript `trim()`, which strips U+FEFF, while Rust's
  `trim_start` does not, so a first-character test spelled in Rust would pass a byte-order mark in front
  of a valid `allow` object (same test).

## 11. Standard error carries three things and nothing else

Given a hook run When something is worth saying to a log rather than to the harness's parser Then it goes
to standard error.

- Success: an unserved event word prints exactly
  `pns: unknown hook event \`stop-failed\`` (`tests/hooks.rs:a_hook_word_this_binary_does_not_serve_says_so_and_notifies_nobody\`,
  asserting the trimmed stderr equals that string).
- Failure sources: the other two writers are a plugin-selection warning printed by `run_event` when
  `select_plugins` returns one, and `pns: an answered marker could not be written ({error})` from
  `clear_nag`.
- Fail direction: informative and non-fatal. `clear_nag` explicitly notes "ON STDERR AND NEVER ON STDOUT:
  this runs on a harness hook whose output the harness reads" (`src/main.rs:clear_nag`).
- Thresholds: Not applicable.
- Required side effects: none.
- Forbidden side effects: none of these may move to standard output.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: Not applicable.
- Privacy: the plugin warning names a configuration problem, not a secret.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: NOT ESTABLISHED: nothing in `tests/hooks.rs` asserts that standard error is
  EMPTY on a healthy hook run, so a build that started narrating there would not turn a test red.
  Searched `tests/hooks.rs` for `stderr`: the only assertions on its contents are the unknown-event one
  above (line 1708) and two in the `pns nag` section (lines 3511 and 3513), which is a typed mode rather
  than a hook event. Also NOT ESTABLISHED: the plugin-selection warning in `run_event` is printed with a
  bare `eprintln!("{warning}")` and carries no `pns: ` prefix, unlike the argv path's warnings; no test
  in `tests/hooks.rs` pins either spelling.

## 12. `prompt` starts the turn clock once and ends the session's wait

Given a `UserPromptSubmit` payload naming a session When `pns hook prompt` runs Then `start_of_turn`
writes the turn start marker only if none exists, and `end_blocked_wait` removes that session's wait
marker unconditionally.

- Success: the first prompt writes a marker; a second prompt inside the same turn leaves the existing
  value untouched
  (`tests/hooks.rs:the_first_prompt_of_a_turn_writes_a_marker_and_a_later_one_does_not_reset_it`). A
  prompt from a waiting session clears its wait
  (`tests/hooks.rs:a_prompt_from_a_waiting_session_ends_its_wait`), and only its own
  (`tests/hooks.rs:a_prompt_ends_only_its_own_sessions_wait`).
- Failure sources: a session id that cannot be a filename; a payload with no session id; no readable
  clock.
- Fail direction: closed and silent. `turn_marker` refuses an unsafe id, so nothing is written at all
  (`tests/hooks.rs:a_session_id_carrying_a_path_traversal_never_becomes_a_filename`, which asserts the
  state directory is absent or empty). A payload with no session id is a silent no-op with empty standard
  output. NO CLOCK IS NO MARKER, never a marker at epoch zero, because a marker at zero would measure the
  turn from 1970 and make a two-second turn long-running (`src/main.rs:start_of_turn`).
- Thresholds: Not applicable.
- Required side effects: the marker file is `state_dir()/session-<session_id>.start` holding the epoch
  seconds as text (`src/main.rs:turn_marker`), with the parent directory created on demand.
- Forbidden side effects: `prompt` delivers nothing. It calls neither `run_event` nor `clear_nag`, so no
  channel fires, no decision ring line is written and no nag record is touched (`src/main.rs:hook_mode`,
  the `prompt` arm).
- Timeout and cancellation: only the payload read is bounded; the rest is two file operations.
- Idempotency and duplicates: the write is guarded on `!marker.exists()`, so repeated prompts inside one
  turn are idempotent. The wait clear is `remove_file` and is idempotent by nature.
- Privacy: the session id becomes a filename, which is why it is validated; see behavior 26.
- Process ownership and cleanup: no child process.
- Compatibility contract: `prompt` is also the FAST path that clears a stale quota wait. The guarantee
  that does not depend on it is the turn's own Stop; see behavior 22.

## 13. `stop` claims the turn clock first, before anything slow

Given a `Stop` payload When `end_of_turn` runs Then `consume_turn_marker` renames the marker to a
per-process claim path, reads it, removes the claim, and returns the elapsed seconds, all before the
reply, the condenser or any delivery.

- Success: a marker at 9,000 seconds ago yields a long turn and the pulse fires; a marker at 5 seconds
  ago does not (`tests/hooks.rs:a_turn_long_enough_pulses_and_a_short_one_does_not`).
- Failure sources: no marker; a marker whose contents are not a number; a marker another Stop already
  claimed; no readable clock.
- Fail direction: closed, meaning "not long". `consume_turn_marker` returns `None` on any of them, and
  `pulse::session_was_long(None, _)` is false. A hand-edited marker is not a crash and is still consumed
  (`tests/hooks.rs:a_corrupt_marker_declines_rather_than_crashing_and_is_still_consumed`).
- Thresholds: `session_was_long` is `elapsed_secs >= threshold_secs`. The default threshold is
  `pulse::DEFAULT_LONG_SESSION_SECS` = 300 seconds, overridable by `PNS_PULSE_THRESHOLD_SECS`. At exactly
  300 seconds elapsed the turn IS long; at 299 it is not (`src/pulse.rs:session_was_long`, and the crate
  unit test at `src/pulse.rs` asserting `session_was_long(Some(300), Some(300))`).
- Required side effects: the marker is gone afterwards
  (`tests/hooks.rs:stopping_consumes_the_marker_so_a_second_stop_cannot_re_fire_the_tier`), and the claim
  file `session-<id>.claim.<process id>` is removed too.
- Forbidden side effects: two Stops racing one turn must not both report it long. The claim is a rename,
  so exactly one wins and exactly one pulses
  (`tests/hooks.rs:two_stops_racing_one_turn_cannot_both_report_it_long`, asserting the bridge was
  reached exactly once). A second Stop still notifies but cannot re-fire the tier
  (`tests/hooks.rs:a_second_stop_cannot_re_fire_the_tier_because_the_marker_is_claimed_once`).
- Timeout and cancellation: Not applicable, three file operations.
- Idempotency and duplicates: the claim makes it exactly-once per marker. The reason it runs FIRST is
  ordering rather than speed: Stop is asynchronous, so the next prompt can arrive while this one is still
  condensing, and a claim at the end would delete the marker its successor relied on
  (`src/main.rs:consume_turn_marker`,
  `tests/hooks.rs:a_prompt_arriving_while_the_previous_stop_condenses_keeps_its_own_marker`, which uses a
  handshake with the condenser stub rather than a timed sleep).
- Privacy: Not applicable.
- Process ownership and cleanup: the claim path carries this process's own id, so two racers cannot
  collide on the claim name.
- Compatibility contract: the value is validated before it reaches arithmetic, so a truncated write or a
  hand edit is a decision rather than a crash.

## 14. `stop` reports what the turn said, condensed

Given a claimed turn When `end_of_turn` builds the event Then `turn_reply` yields the turn's text, an
empty reply becomes state `done` with no detail, and a non-empty reply goes through `condense`.

- Success: the payload's own `last_assistant_message` becomes the detail with no transcript read
  (`tests/hooks.rs:the_payloads_own_final_text_becomes_the_detail_without_reading_a_transcript`, which
  also asserts `project` is `dotfiles` off `cwd` `/a/dotfiles`). A condenser line overrides both state
  and detail: `asking|it wants a choice` yields state `asking` and detail `it wants a choice`
  (`tests/hooks.rs:a_condenser_line_is_used_state_and_all_and_a_blank_summary_falls_back`).
- Failure sources: a turn that said nothing; a whitespace-only reply.
- Fail direction: still notifies. A reply of `"   "` yields detail `""` and state `done`
  (`tests/hooks.rs:a_turn_with_nothing_readable_still_notifies_with_no_detail`). Emptiness is judged on
  the FLATTENED reply, because a block carrying only whitespace is non-empty raw and empty once flattened
  (`src/main.rs:turn_reply`).
- Thresholds: `REPLY_MAX_CHARS` = 8000 characters, applied by `render::flatten_reply`, which keeps the
  TAIL rather than the head because a turn states its conclusion at the end
  (`src/render.rs:flatten_reply`). At 8000 characters nothing is cut; at 8001 the first character is
  dropped. `flatten_reply` collapses exactly four whitespace characters, space, tab, carriage return and
  newline (`src/render.rs:FLATTEN_WHITESPACE`), which is a narrower set than `hooks::flattened`.
- Required side effects: `branch` is set from `git_branch(&payload.cwd)`, and `pane` from the
  `HERDR_PANE_ID` environment variable, verbatim
  (`tests/hooks.rs:the_herdr_pane_reaches_the_event_verbatim_and_a_hostile_one_is_scrubbed_downstream`).
- Forbidden side effects: `stop` must never reach moshi. A "moshi is just another channel" sweep would
  take the highest-volume event first and put an Allow or Deny card in front of an operator for a turn
  that has already finished (`tests/hooks.rs:an_ordinary_stop_never_reaches_moshi`, read through the
  `submissions` helper rather than a filename so the guard survives a transport switch).
- Timeout and cancellation: see behaviors 15 and 16.
- Idempotency and duplicates: a second Stop delivers a second card; only the tier is exactly-once.
- Privacy: the turn's own text is sent to the condenser subprocess and to every configured channel.
- Process ownership and cleanup: see behavior 16.
- Compatibility contract: a condenser state the prompt never offered is not a verdict.
  `condenser_verdict` accepts only `done`, `asking` and `blocked`
  (`src/hooks.rs:a_state_the_prompt_never_offered_is_not_a_verdict`). Note that `asking` and `blocked`
  are both wait words, so a Stop condensed to either one STARTS a wait marker instead of ending one
  (`src/pulse.rs:LAMP_BLOCKED`, `src/lights.rs:blocked_marker_action`). NOT ESTABLISHED: no test in
  `tests/hooks.rs` drives a condenser verdict of `asking` or `blocked` and then inspects
  `waiting_sessions`. Searched the marker section (lines 2894 to 3115) and the condenser tests (lines 186
  to 234); the marker tests all use a Stop with no reply, which condenses to `done`.

## 15. The transcript is the fallback, re-read inside a bounded window

Given a Stop whose payload carried no assistant text When `turn_reply` falls back Then the transcript
tail is read up to `1 + reread_attempts()` times, sleeping `reread_interval()` between attempts, and the
first non-empty flattened reply wins.

- Success: a transcript that is empty at spawn and gains its reply mid-loop is still reported
  (`tests/hooks.rs:a_turn_whose_transcript_lands_late_is_re_read_until_it_does`, driving 8 attempts at
  0.05 s and writing the reply after 120 ms). A transcript present at spawn is read on the first attempt
  (`tests/hooks.rs:the_transcript_tail_is_the_fallback_when_the_harness_carried_no_text`).
- Failure sources: an empty `transcript_path` (the loop is skipped entirely); a path that is not a
  regular file; an unreadable or unparseable transcript; a transcript that never fills.
- Fail direction: an empty string, reported the same as a turn that said nothing. "An expired window
  proves only that nothing readable arrived in time" (`src/main.rs:turn_reply`).
- Thresholds: `DEFAULT_REREAD_ATTEMPTS` = 4 and `DEFAULT_REREAD_INTERVAL` = 150 ms, so the default is 5
  reads across roughly 600 ms of sleeping. `MAX_REREAD_ATTEMPTS` = 10 and `MAX_REREAD_INTERVAL` = 5
  seconds clamp the two environment knobs (`PNS_REPLY_REREAD_ATTEMPTS`, `PNS_REPLY_REREAD_INTERVAL`), so
  the worst case is 11 reads across 50 seconds. A knob of `11` clamps to 10; a knob of `10` is taken as
  10\. An interval of `6` clamps to 5 seconds; `5` is taken as 5. The caps exist because their PRODUCT is
  how long a Stop can sit re-reading, so "a stray zero in either costs seconds, never hours"
  (`src/main.rs:MAX_REREAD_ATTEMPTS`).
- Required side effects: none, this is a read.
- Forbidden side effects: a bad knob must not panic. `reread_attempts_from` falls back to the default
  rather than to no retries when the value does not parse. `reread_interval_from` uses
  `Duration::try_from_secs_f64`, which IS the validation: a hand-written guard refused NaN, infinity and
  negatives but let a finite oversized value like `1e300` through, which panicked the constructor and
  exited 101 on a path whose whole contract is exiting 0 (`src/main.rs:reread_interval_from`,
  `tests/hooks.rs:a_malformed_reread_interval_falls_back_instead_of_panicking`, covering `NaN`, `inf`,
  `-1`, `not-a-number`, `1e30` and `1e300`). A garbage attempts knob still notifies and still exits 0
  (`tests/hooks.rs:a_garbage_re_read_knob_still_notifies_and_still_exits_zero`).
- Timeout and cancellation: the loop is the bound; there is no separate deadline.
- Idempotency and duplicates: reads only.
- Privacy: the transcript holds the whole conversation; only the last turn's assistant text blocks are
  extracted (`src/hooks.rs:transcript_reply`).
- Process ownership and cleanup: no child process.
- Compatibility contract: `transcript_reply` skips a line that will not parse rather than failing,
  because the tail is cut mid-line by design and the first line is routinely half an object
  (`src/hooks.rs:a_first_line_cut_in_half_by_the_tail_is_skipped_not_fatal`). Tool blocks are not the
  reply (`src/hooks.rs:tool_blocks_are_not_the_reply`), and several text blocks in one turn join with a
  blank line between them
  (`src/hooks.rs:several_text_blocks_in_one_turn_join_the_way_the_harness_renders_them`).

## 16. The transcript is checked before it is opened, and only its tail is read

Given a `transcript_path` When `transcript_tail` runs Then `symlink_metadata` is called on the link
itself, a non-regular file yields the empty string with no open at all, and a regular file is seeked to
its last `TRANSCRIPT_TAIL_BYTES` and read with a matching cap.

- Success: a regular transcript yields its tail, decoded with `from_utf8_lossy`.
- Failure sources: a first-in first-out special file, which blocks on open until a writer appears;
  `/dev/zero`, which never ends; a path that does not exist.
- Fail direction: closed and instant. Neither special file holds the hook open
  (`tests/hooks.rs:a_transcript_that_never_ends_is_not_read_at_all`, which creates a real named pipe with
  `/usr/bin/mkfifo` and also drives `/dev/zero`, asserting exit 0 inside the hang limit).
- Thresholds: `TRANSCRIPT_TAIL_BYTES` = 4,000,000. The seek is to
  `metadata.len().saturating_sub(TRANSCRIPT_TAIL_BYTES)`, so a file at or under 4,000,000 bytes is read
  from byte zero and a larger one loses its head. The read is ALSO capped at the same number with `take`,
  because the file can grow between the metadata call and the read, and a seek that failed would
  otherwise read all of it (`src/main.rs:transcript_tail`). Slurping the whole file was measured on
  2026-08-05 at roughly 33 MB resident and minutes of processor time.
- Required side effects: none.
- Forbidden side effects: no open of a non-regular path. The check happens before the open for exactly
  that reason.
- Timeout and cancellation: there is no deadline here; the type check is what makes one unnecessary.
- Idempotency and duplicates: reads only, called once per re-read attempt.
- Privacy: only the tail is loaded, which still carries far more than one turn.
- Process ownership and cleanup: no child process.
- Compatibility contract: a transcript is a regular file. That is the assumption, stated in the source
  and enforced rather than trusted.

## 17. The condenser is a bounded, re-entrant-guarded subprocess

Given a non-empty reply When `condense` runs Then it spawns Codex against a private stripped home with a
fixed prompt, bounded by `CONDENSER_DEADLINE`, and falls back to `("done", render::preview(reply))` on
anything short of a usable verdict.

- Success: a stub printing `asking|it wants a choice` sets both state and detail
  (`tests/hooks.rs:a_condenser_line_is_used_state_and_all_and_a_blank_summary_falls_back`).
- Failure sources: `PNS_SUMMARIZING` already set; no condenser home; the binary missing; a child that
  closes standard output and sleeps; a child that never reads its standard input; a verdict line with a
  blank summary; a state word the prompt never offered.
- Fail direction: the fallback, always. A summary of spaces is as blank as no summary, so the reply
  itself stands (same test, second half). A state with a blank summary used to count as a hit, which
  shipped a title-only notification over a turn that had text, live on 2026-08-12
  (`src/hooks.rs:condenser_verdict`).
- Thresholds: `CONDENSER_DEADLINE` = 30 seconds, overridable in milliseconds by
  `PNS_CONDENSER_DEADLINE_MS`. Output is capped at `PROBE_READ_MAX` = 1,048,576 bytes. The fallback
  preview is capped at `render::PREVIEW_MAX_CHARS` = 260 characters, cut at the last sentence end that
  fits and otherwise clipped with a trailing `…` (`src/render.rs:preview`, `src/render.rs:clipped`). The
  condenser prompt itself asks for a summary "up to 320 characters" (`src/hooks.rs:condenser_prompt`).
- Required side effects: `condenser_home` creates the home 0700 and, only when absent, writes
  `config.toml` 0600 with `create_new` containing `model = "gpt-5.5"\nmodel_reasoning_effort = "low"\n`;
  it then removes and re-creates `auth.json` as a symbolic link to `$HOME/.codex/auth.json`. The command
  is `codex exec --ephemeral --skip-git-repo-check -C <home> -s read-only -` with `PNS_SUMMARIZING=1` and
  `CODEX_HOME=<home>` in its environment (`src/main.rs:condense`, `src/main.rs:condenser_home`).
  `CODEX_BIN` and `PNS_CODEX_HOME` override the binary and the home.
- Forbidden side effects: no pns-to-Codex-to-pns loop. The stripped home installs no hooks or plugins at
  all, which is the hard guarantee; `PNS_SUMMARIZING` is the cheap one
  (`tests/hooks.rs:the_re_entry_guard_keeps_a_condenser_run_from_condensing_itself`). A dead turn never
  spawns it at all; see behavior 18.
- Timeout and cancellation: `run_bounded` waits on a thread and kills the child when the window closes,
  because there is no wait-with-timeout in the standard library and macOS ships no `timeout(1)`
  (`src/system.rs:run_bounded`). Two shapes are pinned: a child that closes standard output and sleeps
  (`tests/hooks.rs:a_condenser_that_closes_stdout_and_sleeps_is_killed_at_its_deadline`) and a child that
  never drains its standard input, driven with a 200,000 character reply so the pipe buffer fills
  (`tests/hooks.rs:a_condenser_that_never_reads_its_stdin_is_bounded_too`). The write to the child is
  inside the window, which it once was not.
- Idempotency and duplicates: one spawn per Stop with a non-empty reply.
- Privacy: the turn's flattened reply, up to 8000 characters, is written to the child's standard input,
  and the child runs against a home that symbolically links the live Codex credentials, which is why the
  home is created owner-only.
- Process ownership and cleanup: the child is killed on deadline expiry by `run_bounded`.
- Compatibility contract: the condenser's own answer format is one line, `STATE|SUMMARY`, and the LAST
  usable line wins (`src/hooks.rs:the_condensers_last_usable_line_wins`). `asking` is narrowed in the
  prompt text to "has a question or choice for YOU, the human operator, to answer", because the looser
  wording classified a status line reading "waiting on the remaining reviews" as `asking` and carded the
  operator over a turn asking them nothing (`src/hooks.rs:condenser_prompt`).

## 18. `stop-failure` reports the death and reads nothing else

Given a `StopFailure` payload When `failed_turn` runs Then the turn marker is claimed, the nag record is
cleared, and the event is delivered with state `failed` and the payload's own message as the detail, with
no condenser call and no transcript read.

- Success: detail `API Error: 500 internal server error`, state `failed`, project `dotfiles`, pane
  `wX:p9` (`tests/hooks.rs:a_turn_that_died_notifies_as_failed_and_says_what_killed_it`, which also
  asserts the partial `last_assistant_message` the payload carried never stands in for the error).
- Failure sources: a payload naming no error.
- Fail direction: the message chain still resolves, so a `StopFailure` naming a tool would card the tool.
  A payload naming nothing yields an empty detail and state `failed`.
- Thresholds: the error is cut at `TOOL_REQUEST_MAX_CHARS` = 320 from the head; see behavior 6. The
  long-running tier uses the same 300 second threshold as `stop`.
- Required side effects: the marker is consumed. This is the arm that used to leak it: StopFailure fires
  INSTEAD of Stop, so a dead turn left its marker, the next prompt declined to rewrite it, and the turn
  after that was measured from the dead turn's start, promoting later short turns to the long-running
  tier for the rest of the session (`src/main.rs:failed_turn`,
  `tests/hooks.rs:a_dead_turn_consumes_the_marker_so_the_next_turn_is_not_measured_from_its_start`). A
  long dead turn still earns its pulse (`tests/hooks.rs:a_long_turn_that_died_still_earns_its_pulse`).
- Forbidden side effects: no model call on the one path where a model call has just failed, and no
  transcript read, since neither recovers the news. Both are pinned in one test: a Codex stub that
  touches a file is the tripwire for the condenser, and a four-attempt two-second re-read loop is the
  tripwire for the transcript, so a build that reads sits through eight seconds of sleeps and blows the
  hang limit (`tests/hooks.rs:a_dead_turn_spawns_no_condenser_and_reads_no_transcript`). StopFailure must
  never reach moshi (`tests/hooks.rs:a_failed_turn_never_reaches_moshi`).
- Timeout and cancellation: only `git_branch`, bounded at `GIT_DEADLINE` = 5 seconds.
- Idempotency and duplicates: one card per StopFailure; the tier is exactly-once via the claim.
- Privacy: the provider's error string is remote text, scrubbed by behavior 7 before it renders.
- Process ownership and cleanup: at most one bounded `git` child.
- Compatibility contract: the partial `last_assistant_message` is deliberately dropped, because "the
  question at a dead pane is why it stopped rather than what it had said" (`src/main.rs:failed_turn`).

## 19. `resolved` clears and delivers nothing

Given a `PostToolBatch` payload When `pns hook resolved` runs Then `clear_nag` writes this session's
answered marker and removes its nag record, and `end_blocked_wait` removes the wait marker only when the
payload carries no `agent_id` key.

- Success: a batch with no `agent_id` ends the session's wait
  (`tests/hooks.rs:a_resolved_batch_with_no_agent_id_ends_its_sessions_wait`); the record is removed and
  the marker written, and a following nag fire adds nothing
  (`tests/hooks.rs:an_answered_approval_is_never_nudged_by_either_clearing_signal`).
- Failure sources: an `agent_id` key of any shape; a session id that cannot be a filename; a state
  directory that cannot be written.
- Fail direction: closed on the wait, fail-quiet on the record. A subagent's batch leaves the parent's
  wait lit, including for `null`, `7` and `""`
  (`tests/hooks.rs:a_resolved_batch_carrying_an_agent_id_leaves_the_parents_wait_lit`,
  `tests/hooks.rs:a_resolved_batch_with_a_malformed_agent_id_still_leaves_the_parents_wait_lit`). A
  marker that could not be written prints one line to standard error (`src/main.rs:clear_nag`), and the
  record removal is best effort, present or not.
- Thresholds: Not applicable.
- Required side effects: the answered marker is written FIRST and the record removed second, so a crash
  between the two leaves an approval that is never nudged rather than one nudged after being answered
  (`tests/hooks.rs:an_answered_approval_is_never_nudged_by_either_clearing_signal`).
- Forbidden side effects: `resolved` delivers nothing at all. It loads no configuration, builds no event
  and reaches no channel, because it fires on every assistant tool batch this machine runs and a hook
  word that notified would card the operator once per batch forever (`src/main.rs:hook_mode` the
  `resolved` arm, and the same test, which asserts zero deliveries for `resolved` and one for `stop`).
- Timeout and cancellation: only the payload read; the rest is at most two file operations.
- Idempotency and duplicates: idempotent. The marker name is constant per session, so a second batch
  rewrites the same file (`src/main.rs:clear_nag`).
- Privacy: nothing from the payload is rendered or stored beyond the session id as a filename.
- Process ownership and cleanup: no child process.
- Compatibility contract: this arm is declared `async: true` (`PostToolBatch`), so it is UNORDERED
  against the next `PermissionRequest` and against the batch's own `asked`. A late End can unlink a newer
  wait's marker and an early one can leave an answered `asked` lit; the damage is bounded by the backstop
  and by the session's next event (`src/main.rs:hook_mode`). Stop is the free backstop for a batch
  payload over the 1 MB cap, for an operator who escaped the prompt instead of answering it, and for the
  window before the `PostToolBatch` declaration is applied (`src/main.rs:end_of_turn`).

## 20. `asked`, `plan-ready` and `denied` are mid-turn news on one arm

Given a payload for any of the three When `pns hook <word>` runs Then one `Attempt::First` event is built
with that word as its state, the project from `cwd`, the detail from the message chain and the pane from
`HERDR_PANE_ID`, and the turn marker is left alone.

- Success: `asked` on an elicitation yields detail `composio: Please authorize Gmail access`
  (`tests/hooks.rs:an_mcp_server_waiting_on_input_notifies_as_asked_and_names_the_server`), and `denied`
  on a refused tool call yields `Bash: command=rm -rf /tmp/x`
  (`tests/hooks.rs:a_refused_tool_call_notifies_as_denied_and_says_which_tool_was_refused`). Both assert
  the state word EXACTLY, because nothing in the crate validates one and a typo would ship silently.
- Failure sources: a payload stating no message, detail, error or tool.
- Fail direction: an empty detail, still delivered.
- Thresholds: Not applicable beyond the shared 320 character composed-line cap.
- Required side effects: all three are wait words, so each STARTS this session's wait marker through the
  `Attempt::First` tail (`src/pulse.rs:LAMP_BLOCKED`,
  `tests/hooks.rs:a_waiting_agent_leaves_a_marker_and_the_next_event_from_that_session_removes_it`, which
  arms one wait with `blocked` and a second with `asked`). Being `Attempt::First`, each also writes the
  decision ring, the journal, the activity ring, the unread news record, the loop lease, the presence
  edge and the lights tick lease.
- Forbidden side effects: none of the three touches the turn marker, because the turn continues past them
  and consuming it would restart the clock mid-turn, making a long turn report itself short
  (`tests/hooks.rs:a_refused_tool_call_leaves_the_turn_marker_alone`, asserting the marker still reads
  `1755000000`). None of the three forwards to moshi: `asked` and `plan-ready` are answered at the pane
  the harness is already holding open, and a denial is a decision the harness has ALREADY taken, so a
  card offering Allow and Deny would answer a closed question no prompt is listening to
  (`src/main.rs:hook_mode`,
  `tests/hooks.rs:a_denial_never_pays_for_the_approval_round_trip_and_still_exits_zero`,
  `tests/hooks.rs:a_non_blocking_event_never_pays_for_the_round_trip`).
- Timeout and cancellation: no condenser and no `git` call; these three set no `branch`.
- Idempotency and duplicates: one card per received event, no coalescing.
- Privacy: `tool_name` is remote text when a connected Model Context Protocol server names its own tools,
  and on a Codex payload carrying neither `message` nor `detail` that name IS the whole card
  (`src/hooks.rs:HookPayload::tool_name`).
- Process ownership and cleanup: no child process.
- Compatibility contract: NOT ESTABLISHED for `plan-ready` specifically. No test in `tests/hooks.rs`
  drives the word `plan-ready`; searched the file for that string and found it only in the usage text
  expectations and in the shared arm's source comment. Its behavior is inferred from sharing the arm with
  `asked` and `denied` (`src/main.rs:hook_mode`) and from its membership in `pulse::LAMP_BLOCKED`.

## 21. `model-switch` reports only an automatic change between two different names

Given a `PostModelSwitch` payload When `pns hook model-switch` runs Then the event is delivered as an
`Attempt::Observation` only when `source` is exactly `auto` AND `model_switch_detail` finds two
non-empty, unequal names once rendered plainly.

- Success: detail is `automatic session model change: claude-sonnet-4-5 to claude-opus-4-6`, state
  `model-switch`, agent `claude`, one decision ring line containing `claude/model-switch` and `nag=no`
  (`tests/hooks.rs:an_observation_still_delivers_and_is_logged`, whose payload carries a bell and a
  carriage-return-newline inside the model names).
- Failure sources: any `source` other than `auto`; equal names; either name empty.
- Fail direction: silence, and total silence. Equal names deliver nothing
  (`tests/hooks.rs:an_auto_switch_between_equal_names_delivers_nothing`); a missing `to_model` delivers
  nothing (`tests/hooks.rs:an_auto_switch_missing_a_model_name_delivers_nothing`). Every other `source`
  writes no decision line, no activity line and moves no presence edge, proven against `command`,
  `picker`, `sdk`, `resume`, a missing key, an empty string, the number 7, `AUTO` and `manual`, all
  measured against a same-sandbox `auto` control that fired first
  (`tests/hooks.rs:a_non_auto_model_switch_source_delivers_nothing_and_writes_nothing`).
- Thresholds: exact string equality on `auto`. `AUTO` does not match; neither does an absent key, which
  matters because "a gate that let an ABSENT source through would fire on every harness that sends none"
  (same test).
- Required side effects: exactly one decision ring line, logged with `nag=no`.
- Forbidden side effects: an observation must not clear a wait, arm the unread lamp, write an activity
  line, move the presence edge, renew a loop lease, journal a miss, replay the journal or register a
  lights tick; see behavior 24. It is also labelled "automatic session model change" and never
  "fallback", because the payload cannot tell a fallback chain apart from any other automatic change
  (`src/main.rs:hook_mode`).
- Timeout and cancellation: no subprocess.
- Idempotency and duplicates: one card per received event.
- Privacy: both model names are payload text and go through `rendered_plainly`, which is what removes a
  right-to-left override that could reorder the line
  (`tests/hooks.rs:an_auto_switch_strips_a_unicode_format_character_from_the_name`).
- Process ownership and cleanup: no child process.
- Compatibility contract: `source` is ONE field serving two events. It carries a `PostModelSwitch` cause
  and a `ConfigChange` kind, "in `message`'s own style: the two hooks never fire together, both name
  their field `source` in the payload Claude Code sends, and only one caller ever reads it for a given
  invocation" (`src/hooks.rs:HookPayload::source`).

## 22. `quota` recognises exactly three notification types, and one of them starts a wait

Given a `Notification` payload When `pns hook quota` runs Then `quota_label` matches exactly
`quota_auto_resume_fired`, `quota_auto_resume_stale` or `quota_auto_resume_disabled`, the card is
delivered as an `Attempt::Observation`, and `quota_auto_resume_stale` additionally arms this session's
wait marker BEFORE the delivery.

- Success: the three details are `quota auto-resume fired: continuing automatically`,
  `quota auto-resume stale: press enter to continue` and `quota auto-resume disabled: turned off` (three
  tests, `quota_auto_resume_*_delivers_one_card_naming_itself`). With no message the label stands alone
  as `quota auto-resume fired`, with nothing trailing it, for an absent, empty or non-string `message`
  (`tests/hooks.rs:a_quota_notification_carrying_no_message_still_names_what_happened`).
- Failure sources: any other `notification_type`.
- Fail direction: silence. Thirteen near misses and unrelated types deliver nothing, including
  `agent_needs_input` and `agent_completed` (deliberately unwired: the former may duplicate an ordinary
  asked or blocked event and the latter combines success and failure in one type), the empty string,
  `quota_auto_resume_`, `quota_auto_resume_paused`, `quota_auto_resume_firedly`,
  `quota_auto_resume_stale_again` and `pre_quota_auto_resume_fired`, all against a same-sandbox control
  (`tests/hooks.rs:an_unrecognised_notification_type_delivers_nothing`). A widening to
  `starts_with("quota_auto_resume_")` was measured to pass every test except those near misses.
- Thresholds: an exact three-word allowlist, mirroring rather than trusting the matcher declared beside
  it in `modify_settings.json` (`src/main.rs:quota_label`).
- Required side effects: one decision ring line per event, logged with `nag=no`
  (`tests/hooks.rs:every_quota_type_is_logged_as_an_observation_with_no_nag`). For
  `quota_auto_resume_stale` only, `arm_quota_stale_wait` calls the wait marker's Start operation
  directly, gated on the lamps being live, meaning `[lights]` present AND the hue plugin enabled
  (`src/main.rs:arm_quota_stale_wait`,
  `tests/hooks.rs:quota_auto_resume_stale_arms_the_needs_marker_for_its_own_session`).
- Forbidden side effects: `fired` and `disabled` arm no wait marker, because neither reports a session
  waiting on the operator (`tests/hooks.rs:quota_auto_resume_fired_and_disabled_arm_no_needs_marker`). No
  quota type clears a live wait, arms the unread lamp, writes an activity line, journals a miss, replays
  the journal, registers a lights tick, renews a loop lease or moves the presence edge; see behavior 24.
- Timeout and cancellation: `arm_quota_stale_wait` loads the configuration once, which is one open and
  one parse of a local file.
- Idempotency and duplicates: one card per received event.
- Privacy: the harness's own message is rendered onto the card.
- Process ownership and cleanup: no child process.
- Compatibility contract: the ORDER is load-bearing. The declaration is `async: true`, so the hook runs
  beside a session whose screen is already telling the operator to press Enter. Arming after the delivery
  plan would let an Enter inside that window clear nothing and then take a marker published behind it,
  leaving a blue lamp for a session that is working again. Arming first cannot CLOSE that race, which is
  the harness's to close, but it shrinks the window from a plan of network legs to one file write
  (`src/main.rs:arm_quota_stale_wait`, pinned by
  `tests/hooks.rs:a_stale_wait_arms_the_needs_marker_before_the_card_is_delivered`, which reads the wait
  directory from inside the delivering channel stub and asserts it already held `s1`). What CLEARS the
  stale wait is pinned twice, because Claude Code's own continuation prompt may or may not reach the
  `UserPromptSubmit` hook and this repository has no capture that settles it:
  `tests/hooks.rs:the_prompt_hook_clears_a_stale_quota_marker` is the fast path and
  `tests/hooks.rs:a_stale_quota_marker_clears_at_the_turns_stop_without_any_prompt_hook` is the
  guarantee.

## 23. `config-change` recognises exactly five sources, and one of them outlives the decision ring

Given a `ConfigChange` payload When `pns hook config-change` runs Then `config_source_label` matches
exactly `user_settings`, `project_settings`, `local_settings`, `policy_settings` or `skills`, the card is
delivered as an `Attempt::Observation`, and `policy_settings` additionally appends one line to a bounded
audit trail.

- Success: each source yields `<label>: /Users/op/.claude/settings.json` with the five labels
  `user settings changed`, `project settings changed`, `local settings changed`,
  `policy settings changed` and `skills changed`
  (`tests/hooks.rs:each_config_change_source_delivers_one_card_naming_itself_and_its_file`). With no
  `file_path` the label stands alone, with no trailing colon
  (`tests/hooks.rs:a_config_change_with_no_file_names_only_the_source`).
- Failure sources: any other `source` value.
- Fail direction: silence, checked against a missing key, an empty string, the number 7, `User_Settings`,
  `user_settingsx` and `global_settings`, each leaving deliveries, the decision ring, the activity ring
  and the presence edge byte-identical to a same-sandbox control
  (`tests/hooks.rs:an_unrecognised_config_source_delivers_nothing_and_writes_nothing`).
- Thresholds: `CONFIG_PATH_MAX_CHARS` = 1024 characters and `CONFIG_SESSION_MAX_CHARS` = 64 characters,
  both applied through `config_field`, which is `render::clipped` over `rendered_plainly`. At 1024
  characters a path passes whole; at 1025 it is cut to 1023 characters plus a trailing `…`, so the cut is
  MARKED rather than silent (`src/main.rs:config_field`, `src/render.rs:clipped`). 1024 is macOS's own
  `PATH_MAX`; Linux's is 4096, so a genuinely long Linux path is visibly clipped, which is stated as an
  accepted cost (`src/main.rs:CONFIG_PATH_MAX_CHARS`). `POLICY_SETTINGS_AUDIT_KEPT` = 20 entries; the
  twenty-first append drops the oldest
  (`tests/hooks.rs:the_policy_settings_audit_trail_is_bounded_and_drops_the_oldest_entry`, which plants
  twenty and asserts the kept window starts at `planted-1`). `RING_READ_MAX` = 262,144 bytes is the
  read-back ceiling the prune runs on.
- Required side effects: for `policy_settings` only, one line `{now} session={session} file={path}` is
  appended to `state_dir()/policy-settings-audit`, with `path` replaced by the literal `none` when the
  payload named no file (`src/main.rs:record_policy_settings_change`). The ordinary observation card
  still fires on top of it
  (`tests/hooks.rs:a_policy_settings_change_is_recorded_to_a_bounded_audit_trail`).
- Forbidden side effects: the other four sources must not start a second durable file
  (`tests/hooks.rs:a_non_policy_config_change_writes_no_policy_audit_entry`, with a same-sandbox control
  proving the writer was reachable). The observation restrictions of behavior 24 apply, each with its own
  First-attempt control run afterwards on the same sandbox.
- Timeout and cancellation: `append_ring_line` takes a lock beside the ring; failing to claim it returns
  `WouldBlock` and the record is dropped fail-quiet (`src/main.rs:append_ring_line`,
  `src/main.rs:record_policy_settings_change`).
- Idempotency and duplicates: deliberately NOT idempotent. There is no once-per-something guarantee,
  because a corrupt-file recovery, several live sessions or a changed skill can each produce their own
  event, so three received events produce three cards
  (`tests/hooks.rs:config_change_events_each_deliver_their_own_card_with_no_once_ever_guarantee`). Two
  events racing the prune lose neither line
  (`tests/hooks.rs:two_policy_settings_changes_racing_the_prune_lose_neither_line`).
- Privacy: `file_path` is untrusted text landing in a banner, a card AND a durable file. A newline in it
  cannot forge a second audit entry, since the trail is one record per line and the flatten removes it
  (`tests/hooks.rs:a_newline_in_a_file_path_cannot_forge_a_policy_audit_entry`). The detail says only
  WHICH SOURCE and, optionally, WHICH FILE, never what changed: the payload carries no key, no old or new
  value and no actor (`src/main.rs:config_change_detail`).
- Process ownership and cleanup: no child process.
- Compatibility contract: the character cap is what keeps the audit trail readable at all. Both fields
  are harness text bounded only by the 1 MB standard-input ceiling, and one oversized path would make the
  prune's read-back fail, at which point the heal collapses the whole trail to the single line just
  written, losing every policy change before it. Driven directly with a 300,012 character path
  (`tests/hooks.rs:an_enormous_file_path_cannot_wipe_the_policy_audit_trail`, which asserts the earlier
  entry survives, the file stays under 262,144 bytes, and a same-sandbox control still appends
  afterwards).

## 24. An observation changes no workflow state

Given an event routed with `Attempt::Observation`, that is `model-switch`, `quota` or `config-change`
When `run_event` reaches its contiguous tail Then it returns immediately after `record_decision`, so none
of the First-delivery side effects run.

- Success: the card is still delivered and the decision ring still carries one line with `nag=no`
  (`tests/hooks.rs:an_observation_still_delivers_and_is_logged`).
- Failure sources: a misrouting of one of the three arms as `Attempt::First`.
- Fail direction: each restriction is pinned individually with a positive control asserting the delivery
  happened inside the same run, so a negative cannot pass because the arm did nothing at all. The eight
  restrictions and their tests, per family: clear a live wait
  (`an_observation_does_not_clear_a_live_wait`, `no_quota_type_clears_a_live_wait_on_its_own_session`,
  `a_config_change_does_not_clear_a_live_wait_on_its_own_session`); arm the unread news record
  (`an_observation_arms_no_unread_news`, `no_quota_type_arms_unread_news`); write an activity ring line
  (`an_observation_writes_no_activity_line`, `no_quota_type_writes_an_activity_line`,
  `a_config_change_writes_no_activity_line`); move the presence edge
  (`an_observation_moves_no_presence_edge`, `no_quota_type_moves_the_presence_edge`,
  `a_config_change_moves_no_presence_edge`); renew a loop lease (`an_observation_renews_no_loop_lease`,
  `no_quota_type_renews_a_loop_lease`, `a_config_change_renews_no_loop_lease`); journal a missed
  notification (`an_observation_journals_no_missed_notification`,
  `a_quota_observation_journals_no_missed_notification`,
  `a_config_change_observation_journals_no_missed_notification`); replay the journal
  (`an_observation_replays_no_journal_entry`, `a_quota_observation_replays_no_journal_entry`,
  `a_config_change_observation_replays_no_journal_entry`); register a lights tick
  (`an_observation_registers_no_lights_tick`, `a_quota_observation_registers_no_lights_tick`,
  `a_config_change_registers_no_lights_tick`).
- Thresholds: Not applicable.
- Required side effects: exactly one decision ring line, since `record_decision` runs before the guard
  for every attempt (`src/main.rs:run_event`).
- Forbidden side effects: the eight above, plus the pulse, which falls out of the same early return and
  is how "escalation is not a colour" stays enforced without touching the lights at all
  (`src/main.rs:run_event`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: an observation is not an occurrence to replay later, so a suppressed one is
  simply lost, deliberately.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: an observation shaped like a `PermissionRequest` is too late to gate here.
  `blocking_event` forwards and arms the nag before `run_event` ever runs, so a caller on that path must
  refuse the observation at the top of `blocking_event` itself (`src/main.rs:Attempt`).

## 25. The world is read at dispatch, not at the moment the hook started

Given a Stop that spends seconds in the condenser When the delivery plan is built Then the surface
reading is taken inside `run_event`, from one memoized probe set, rather than at process start.

- Success: a phone marker touched as the hook starts and back-dated by the condenser stub to ten seconds
  ago, against a desk reading stated at two seconds, produces a banner and no phone card
  (`tests/hooks.rs:the_world_is_read_at_dispatch_and_not_at_the_moment_the_hook_started`).
- Failure sources: a presence reading nobody can parse; a stuck multiplexer.
- Fail direction: open. Unknown never suppresses, so a herdr that hangs costs a spare notification rather
  than the notification
  (`tests/hooks.rs:a_stuck_multiplexer_leaves_the_view_unreadable_rather_than_blocking`).
- Thresholds: ages are whole seconds and a tie goes to the desk, which is why the test states the desk at
  two seconds rather than one: at one second, a fresh marker whose own age had just rolled over read Desk
  about one run in twenty (measured 2026-09-01, stated in that test).
- Required side effects: one probe set per invocation (`src/main.rs:system_probes`), and one wall clock
  read per event, because "a second wall-clock read here is exactly the boundary that let a phone reading
  and a desk reading about one event disagree" (`src/main.rs:run_event`).
- Forbidden side effects: no second construction of the probe set per consumer.
- Timeout and cancellation: every probe spawn is bounded by `run_bounded`.
- Idempotency and duplicates: Not applicable.
- Privacy: Not applicable.
- Process ownership and cleanup: probe children are killed on their own deadlines.
- Compatibility contract: this claim holds for the hook and blocking paths, where the forward decision
  and the delivery plan share one probe set. `pns gate <harness>-hook` builds its own throwaway probe set
  and runs no delivery plan at all, so the claim does not extend to it (`src/main.rs:forward_to_moshi`).

## 26. A session id that cannot be a filename reaches no file operation

Given a payload whose `session_id` is hostile or empty When any arm would write or remove a marker keyed
by it Then `safety::session_id_is_safe` refuses it first and the arm does nothing.

- Success: an ordinary id such as `s1` passes; the predicate admits ASCII alphanumerics plus `.`, `_` and
  `-` (`src/safety.rs:session_id_is_safe`).
- Failure sources: an empty id; an id containing `..`; an id with a slash or any other character; an id
  that collides with the lights' working-owner names.
- Fail direction: closed on both the write and the remove. A prompt naming `../../etc/passwd` writes
  nothing (`tests/hooks.rs:a_session_id_carrying_a_path_traversal_never_becomes_a_filename`), and a
  prompt naming `../../victim` removes nothing, leaving both the victim file and the real wait marker
  intact (`tests/hooks.rs:a_prompt_naming_a_traversal_removes_nothing`). The END action goes through the
  same predicate as the START, which is the point of that second test.
- Thresholds: Not applicable, membership decides.
- Required side effects: none.
- Forbidden side effects: no identity means no marker. An event that arrives on argv rather than through
  a hook has nothing that could later say the wait ended, so it gets the flash and cannot hold the lamp;
  the same answer comes back through a hook payload whose id cannot be a filename
  (`tests/hooks.rs:an_event_with_no_session_id_behind_it_holds_no_lamp`, which drives both doors).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: Not applicable.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the same predicate backs the turn marker (`session-<id>.start`) and the wait
  marker (`lights-blocked/<id>`), so there is one rule rather than two (`src/lights.rs:blocked_marker`).

## 27. The wait marker is started by five states and ended by every other

Given any event carrying a session id When the `Attempt::First` tail reaches `update_blocked_marker` Then
`blocked_marker_action` maps the event's state word to Start or End, Start is gated on the lamps being
live and End is not.

- Success: `blocked` from `s1` and `asked` from `s2` leave two markers; a `stop` from `s1` removes only
  s1's (`tests/hooks.rs:a_waiting_agent_leaves_a_marker_and_the_next_event_from_that_session_removes_it`,
  which names the survivor rather than counting files).
- Failure sources: a state word nothing recognises; no readable clock; lamps not configured.
- Fail direction: End. "A CLOSED SET OF STARTERS AND EVERYTHING ELSE ENDS", because the fail direction
  that matters is the one that lets a lamp go dark: an unknown word treated as a start would hold blue on
  a session nobody is waiting for (`src/lights.rs:blocked_marker_action`). With no clock, no marker is
  written at all, never one at epoch zero (`src/main.rs:update_blocked_marker`).
- Thresholds: the starter set is exactly five words, `blocked`, `asked`, `plan-ready`, `denied` and
  `asking` (`src/pulse.rs:LAMP_BLOCKED`). It deliberately does NOT include `failed`, which
  `missed_notifications::NEEDS_YOU` does include: a dead turn is red, not blue, and is not a wait anybody
  can end (`src/lights.rs:blocked_marker_action`).
- Required side effects: a Start needs BOTH switches, a `[lights]` table and the hue plugin enabled, so a
  machine with no lamps never accumulates markers nothing will sweep (`src/main.rs:run_event`). An End
  never checks the switches.
- Forbidden side effects: an observation must not reach this at all; see behavior 24. The
  `an_observation_does_not_clear_a_live_wait` family is load-bearing precisely because the End arm is
  ungated, so a misrouted observation would clear the marker whether or not the lamps are configured.
- Timeout and cancellation: Not applicable, one file write or one unlink.
- Idempotency and duplicates: one file per session carries no generation, so an older Stop can remove a
  newer wait's marker. That is a stated limit rather than a rule: concurrent unlink cannot arbitrate on
  this filesystem, so telling the two apart would need a generation inside the marker and a
  compare-and-swap publish. The damage is bounded by the give-up backstop and closed by the session's
  next event (`src/main.rs:update_blocked_marker`).
- Privacy: the marker's contents are one epoch timestamp.
- Process ownership and cleanup: fail-quiet on both arms; a marker that did not land costs one lamp its
  colour and never a card.
- Compatibility contract: `[lights.blocked] give_up_after_secs` shorter than `[nag] after_secs` is
  refused by name at configuration load, so the backstop can never sweep a wait the nag has not yet
  nudged (`src/main.rs:update_blocked_marker`, referring to `config::parse_config`).

## 28. The bare harness word is vouched for by shape, never by roster

Given argv whose first word looks like `<name>-hook` When `main` decides where to dispatch Then
`hooks::is_harness_subcommand` accepts it only when it splits at the first `-` into a non-empty
all-lowercase-ASCII name and the exact suffix `hook`.

- Success: `pi-hook` and `claude-hook` are accepted
  (`src/hooks.rs:the_gate_vouches_for_the_shape_of_a_subcommand_it_did_not_choose`).
- Failure sources: `hook`, `-hook`, `Pi-hook`, `pi-hook; rm -rf /`, `../../etc/passwd` and the empty
  string, all refused (same test).
- Fail direction: closed. A refused word falls through to the usage refusal rather than to the event
  path, which is how the documented spelling used to fire a notification about an empty event
  (`src/main.rs:main`).
- Thresholds: the split is on the FIRST `-` only, and the suffix must equal `hook` exactly, so
  `stop-failure` is not a harness word.
- Required side effects: none at this layer.
- Forbidden side effects: an unvetted word here would be this repository handing a third-party binary a
  filesystem argument nobody chose, because moshi-hook's positional is a PATH
  (`src/hooks.rs:is_harness_subcommand`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: Not applicable, pure.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: shape only, not a roster, because the harness list is moshi's and grows. The
  separate `moshi_subcommand` is the roster, and it admits only `claude` and `codex`
  (`src/hooks.rs:only_the_harnesses_pns_registers_for_are_forwarded_to_moshi`). The forwarding behavior
  behind both is deferred to `docs/specs/blocking-approval.md`.

______________________________________________________________________

## Environment inputs this path reads

| Variable                      | Read by                                    | Effect                                                                              |
| ----------------------------- | ------------------------------------------ | ----------------------------------------------------------------------------------- |
| `PNS_AGENT`                   | `hook_mode`                                | the harness name on the event; defaults to `claude`                                 |
| `HERDR_PANE_ID`               | every delivering arm                       | the pane the card focuses on click, passed verbatim                                 |
| `PNS_STATE_DIR`               | `state_dir`                                | where markers, rings and the audit trail live; defaults to `$HOME/.local/state/pns` |
| `PNS_PAYLOAD_DEADLINE_MS`     | `payload_deadline`                         | the standard-input wait; defaults to 5 s                                            |
| `PNS_REPLY_REREAD_ATTEMPTS`   | `reread_attempts`                          | extra transcript reads; default 4, clamped to 10                                    |
| `PNS_REPLY_REREAD_INTERVAL`   | `reread_interval`                          | seconds between reads; default 0.15, clamped to 5                                   |
| `PNS_CONDENSER_DEADLINE_MS`   | `condense`                                 | the condenser bound; defaults to 30 s                                               |
| `PNS_SUMMARIZING`             | `condense`                                 | the cheap re-entry guard                                                            |
| `CODEX_BIN`, `PNS_CODEX_HOME` | `condense`, `condenser_home`               | the condenser binary and its private home                                           |
| `PNS_PULSE_THRESHOLD_SECS`    | `pulse_threshold_secs`                     | the long-turn threshold; defaults to 300                                            |
| `HOME`                        | `state_dir`, `condenser_home`, `run_event` | the configuration and state roots                                                   |
