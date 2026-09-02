# Setup and publication

## Scope

`pns setup` is the first-run walk and the only writer of the config file. This specification covers the
whole of that path: how the subcommand is reached and how its argument is read, the four refusals that
run before a single question is asked (an unset or empty `HOME`, a config already at the name, a config
path that does not resolve, and a stdin that is not a terminal), the walk itself and every question in
it, how a typed line becomes an answer, how a secret is read with the terminal's echo held off and how
that terminal state is given back, what a read from a background process does, `--force`, and the
publication itself (the pending file, the hard link that publishes it, the backup that keeps the previous
config aside, the file modes, and every refusal along the way). What the answers COMPOSE INTO, the layout
of the rendered file, which tables are commented out and which keys the roster serves, belongs to the
sibling specification `docs/specs/configuration.md`; this one states only that `setup` calls
`pns::setup::compose_config`, that composition is pure, and that the composed text is put through the
engine's own parser before anything is written. Everything below is derived from the crate at
`dot_local/share/pns` and its tests only. Where the code does not settle a question the line begins
`NOT ESTABLISHED:` and names what was looked for and where. Secrets are the centre of this document:
every behavior carries a Privacy line, and behavior 27 is the exhaustive account.

## The questions

The walk asks between six and fifteen prompts, depending on which features are armed. Six is every
feature declined (the moshi token plus the five yes-or-no questions); fifteen is every feature armed.
`ask` appends `": "` to the prompt it is given (`src/main.rs:ask`) and `ask_yes` appends `" [y/N]"`
before that (`src/main.rs:ask_yes`), so the column below shows what the operator actually sees.

| #   | Prompt, as the operator sees it                                                                     | What it asks                                                                                     | Secret                                              | Empty answer                                                                                                                                                                                                                                                                                                                                                                               | Invalid answer                                                                                                                                                                                                                                                                                                               | Tests that pin it                                                                                                                                                             |
| --- | --------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ | --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | `The phone card is on. Paste moshi's webhook secret to complete it, or press enter to pair later: ` | The moshi webhook secret the phone card is submitted with (`src/setup.rs:Answers::mobile_token`) | Yes, read through `ask_hidden` (`src/main.rs:walk`) | The phone card stays ON and uncarded until a pairing supplies a token; the `token` key is written commented out rather than as `token = ""` (`src/setup.rs:a_skipped_token_is_commented_out_rather_than_written_empty`). NO "nothing given" line is printed for this one: it is the one credential asked through bare `ask_hidden` rather than through `armed_secret` (`src/main.rs:walk`) | No answer is invalid. Any non-empty line is a token; it is trimmed and stored verbatim (`src/main.rs:answered`)                                                                                                                                                                                                              | `tests/setup.rs:a_secret_typed_into_setup_never_reaches_the_pty_output`; `src/setup.rs:a_skipped_token_is_commented_out_rather_than_written_empty`                            |
| 2   | `Post every event to hermes, for the durable log and the recap? [y/N]: `                            | Whether to arm the hermes plugin                                                                 | No                                                  | No. Enter is no, and the hermes key is never asked (`src/main.rs:means_yes`)                                                                                                                                                                                                                                                                                                               | Anything that is not `y`/`yes` in any case is a no, silently (`src/main.rs:means_yes`)                                                                                                                                                                                                                                       | `src/main.rs:the_only_answer_that_arms_a_feature_is_a_yes_somebody_typed`                                                                                                     |
| 3   | `the signing key that route verifies: `                                                             | The hermes signing key (`Answers::hermes_key`)                                                   | Yes, `armed_secret` (`src/main.rs:walk`)            | Prints `  nothing given, so hermes stays off; the file says how to arm it` and the hermes table is composed commented out (`src/main.rs:nothing_given`, `src/setup.rs:Answers::values`)                                                                                                                                                                                                    | No answer is invalid; it is untrusted text carried verbatim into the file (`src/setup.rs:a_credential_carrying_quotes_and_backslashes_reaches_the_config_as_itself`)                                                                                                                                                         | `src/setup.rs:a_credential_left_blank_declines_its_feature_rather_than_arming_an_empty_one`; `tests/setup.rs:a_secret_typed_into_setup_never_reaches_the_pty_output`          |
| 4   | `Flash hue lights green when work finishes and red when it dies? [y/N]: `                           | Whether to arm the hue light pulse                                                               | No                                                  | No, and none of the three hue prompts is asked                                                                                                                                                                                                                                                                                                                                             | Not a yes is a no (`src/main.rs:means_yes`)                                                                                                                                                                                                                                                                                  | `src/main.rs:the_only_answer_that_arms_a_feature_is_a_yes_somebody_typed`                                                                                                     |
| 5   | `the hue bridge's address on the network: `                                                         | The bridge address (`Answers::hue_bridge`)                                                       | No, plain `armed`, so it IS echoed                  | Prints `  nothing given, so the light pulse stays off; the file says how to arm it`, and prompts 6 and 7 are skipped: each hue answer gates the next (`src/main.rs:walk`)                                                                                                                                                                                                                  | Not judged here at all. Any non-empty line is accepted and reaches the file                                                                                                                                                                                                                                                  | `src/setup.rs:a_credential_left_blank_declines_its_feature_rather_than_arming_an_empty_one`                                                                                   |
| 6   | `an API key the bridge issued: `                                                                    | The hue API (application programming interface) key (`Answers::hue_key`)                         | Yes, `armed_secret`                                 | Same line as prompt 5, and prompt 7 is skipped                                                                                                                                                                                                                                                                                                                                             | Not judged                                                                                                                                                                                                                                                                                                                   | `src/setup.rs:a_credential_left_blank_declines_its_feature_rather_than_arming_an_empty_one`; `tests/setup.rs:a_secret_typed_into_setup_never_reaches_the_pty_output`          |
| 7   | `the rooms to flash, comma separated, spelled as the bridge spells them: `                          | The room names (`Answers::hue_rooms`), split on commas with blanks dropped (`src/main.rs:list`)  | No, echoed                                          | Prints the same "nothing given" line and declines hue: the rooms count as a credential, because with none named the plugin falls back to `src/channels/hue.rs:DEFAULT_ROOMS`, a pair naming nobody else's rooms (`src/setup.rs:hue_is_armed`)                                                                                                                                              | A list of nothing but commas and spaces is the same as empty (`src/main.rs:a_comma_separated_answer_names_only_the_values_somebody_typed`)                                                                                                                                                                                   | `src/main.rs:a_comma_separated_answer_names_only_the_values_somebody_typed`; `src/setup.rs:a_credential_left_blank_declines_its_feature_rather_than_arming_an_empty_one`      |
| 8   | `Read whether your phone is on the home wifi, off the router's client list? [y/N]: `                | Whether to arm the home probe                                                                    | No                                                  | No, and the four router prompts are skipped                                                                                                                                                                                                                                                                                                                                                | Not a yes is a no                                                                                                                                                                                                                                                                                                            | `src/main.rs:the_only_answer_that_arms_a_feature_is_a_yes_somebody_typed`                                                                                                     |
| 9   | `Which router backend? [unifi]: `                                                                   | Which compiled-in backend answers the home probe (`Answers::router_type`)                        | No, echoed                                          | Enter is the ONE prompt in the walk that an empty answer ANSWERS rather than declines: it names `unifi`, the only backend there is (`src/main.rs:router_backend`, `src/home.rs:UNIFI_TYPE`)                                                                                                                                                                                                | A name no backend answers (`asus`, `eero`, `u`, `unifix`, `unifi-controller`) prints `  nothing here reads that router, so the home probe stays off; the file says how to arm it` and skips prompts 10 to 12. A case variant of `unifi` IS accepted and is written back as the code spells it (`src/main.rs:router_backend`) | `src/main.rs:the_only_backend_the_walk_accepts_is_one_the_home_probe_answers`; `src/setup.rs:a_backend_the_home_probe_cannot_answer_declines_the_probe_rather_than_arming_it` |
| 10  | `the router's URL: `                                                                                | The router's address (`Answers::router_url`)                                                     | No, echoed                                          | Prints `  nothing given, so the home probe stays off; the file says how to arm it` and skips prompts 11 and 12                                                                                                                                                                                                                                                                             | Not judged                                                                                                                                                                                                                                                                                                                   | `src/setup.rs:a_credential_left_blank_declines_its_feature_rather_than_arming_an_empty_one`                                                                                   |
| 11  | `an API key the router issued: `                                                                    | The router API key (`Answers::router_api_key`)                                                   | Yes, `armed_secret`                                 | Same line, and prompt 12 is skipped                                                                                                                                                                                                                                                                                                                                                        | Not judged                                                                                                                                                                                                                                                                                                                   | `src/setup.rs:a_credential_left_blank_declines_its_feature_rather_than_arming_an_empty_one`; `tests/setup.rs:a_secret_typed_into_setup_never_reaches_the_pty_output`          |
| 12  | `the phone's hostname on that router: `                                                             | The phone's hostname on that router (`Answers::router_device_hostname`)                          | No, echoed                                          | Same line; the home probe is declined                                                                                                                                                                                                                                                                                                                                                      | Not judged                                                                                                                                                                                                                                                                                                                   | `src/setup.rs:a_credential_left_blank_declines_its_feature_rather_than_arming_an_empty_one`                                                                                   |
| 13  | `Hold notifications back while a macOS Focus is on? [y/N]: `                                        | Whether to arm Focus silencing                                                                   | No                                                  | No, and prompt 14 is skipped                                                                                                                                                                                                                                                                                                                                                               | Not a yes is a no                                                                                                                                                                                                                                                                                                            | `src/main.rs:the_only_answer_that_arms_a_feature_is_a_yes_somebody_typed`                                                                                                     |
| 14  | `which Focus modes mean it, comma separated: `                                                      | The Focus mode names that mean "not now" (`Answers::focus_modes`), split by `list`               | No, echoed                                          | Prints `  nothing given, so focus silencing stays off; the file says how to arm it`; the `[focus]` table is composed commented out (`src/setup.rs:Answers::values`)                                                                                                                                                                                                                        | A list of nothing but commas is the same as empty                                                                                                                                                                                                                                                                            | `src/main.rs:a_comma_separated_answer_names_only_the_values_somebody_typed`; `src/setup.rs:a_walk_that_armed_nothing_still_writes_the_core`                                   |
| 15  | `Card you a second time about an approval left unanswered? [y/N]: `                                 | Whether to arm the second card about an unanswered approval (`Answers::nag`)                     | No                                                  | No; `[nag]` is composed commented out                                                                                                                                                                                                                                                                                                                                                      | Not a yes is a no                                                                                                                                                                                                                                                                                                            | `src/main.rs:the_only_answer_that_arms_a_feature_is_a_yes_somebody_typed`; `src/setup.rs:every_armed_feature_reaches_the_parsed_config_carrying_its_own_answers`              |

Two rules run underneath the whole table. First, EVERY answer is trimmed and a line of nothing but
whitespace is a blank one (`src/main.rs:answered`, pinned by
`src/main.rs:an_answer_of_nothing_but_spaces_is_a_blank_one`). Second, a blank credential DECLINES its
feature rather than arming an empty one, because an empty value parses as absent and would deliver
nothing while reading as configured (`src/setup.rs:Answers`, `src/setup.rs:hue_is_armed`,
`src/setup.rs:router_is_armed`).

## What is written

Everything below lands in the config's own directory, `$HOME/.config/pns` (`src/config.rs:config_path`).
Nothing is written anywhere else: `setup_mode` touches no state file, no journal, no decision ring and no
log.

| Path                                                   | Mode                                                                                                                                                                                                          | Name pattern                                                                                                                                                                                                                                                                      | When it is created                                                                                                                                                 | If it already exists                                                                                                                                                                                                                                                                                                                               |
| ------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `$HOME/.config/pns` (the directory)                    | Whatever `create_dir_all` gives, which is `0o777` masked by the umask. This code sets NO mode on it (`src/main.rs:publish_config`)                                                                            | Fixed                                                                                                                                                                                                                                                                             | At publication, after the last answer, by `std::fs::create_dir_all`                                                                                                | A no-op. A non-directory standing at the name fails the leaf's own `symlink_metadata` with `ENOTDIR` back in `setup_mode` and is refused as `could not be checked` long before this point                                                                                                                                                          |
| The pending file                                       | `0o600`, twice: requested on the open (`.mode(CONFIG_FILE_MODE)`, which the umask still masks) and then FORCED on the open handle with `set_permissions` (`src/main.rs:write_then_publish`)                   | `config.toml.new.<process id>.<the current instant's sub-second nanoseconds>` (`src/main.rs:pending_name`)                                                                                                                                                                        | Immediately after the directory, before the composed text is written                                                                                               | `create_new(true)` refuses: `<pending> could not be written: File exists`. It is never opened for truncation, because a leftover pending file is a second name for a live config and process ids are reused (`src/main.rs:publish_config`, pinned by `src/main.rs:a_pending_file_left_by_an_abandoned_run_is_never_the_file_this_one_writes_into`) |
| `$HOME/.config/pns/config.toml` (the published config) | `0o600`. It is a second name for the pending file's inode, so it carries that file's mode (`src/main.rs:write_then_publish`)                                                                                  | Fixed (`src/config.rs:config_path`)                                                                                                                                                                                                                                               | By `std::fs::hard_link(pending, path)`, after the whole composed text is written to the pending file and after any backup has been taken                           | The link fails with `AlreadyExists` and the run refuses: `<path> appeared while the questions were being answered; nothing was written over it`. There is no blanket rename and no overwrite on either path (`src/main.rs:write_then_publish`, pinned by `src/main.rs:a_config_that_appeared_during_the_walk_is_refused_rather_than_written_over`) |
| The backup                                             | `0o600`, applied AFTER the move and only when what moved is a regular file: the mode of a symlink is the mode of what it points at, and that is a file this run did not replace (`src/main.rs:keep_aside_at`) | `config.toml.<stamp>.backup`, where the stamp is the coordinated universal time (UTC) instant with its colons turned into hyphens and its trailing `Z` stripped, for example `config.toml.2027-01-15T08-00-00.backup` (`src/setup.rs:backup_path`, `src/system.rs:utc_timestamp`) | Under `--force` ONLY, and in two steps: the name is CLAIMED with `create_new` before anything moves onto it, then the existing config is `rename`d onto that claim | The claim fails with `AlreadyExists` and the run refuses: `<backup> is already claimed by another run this same second; nothing was written`. An earlier run's backup is never written over (`src/main.rs:keep_aside_at`, pinned by `src/main.rs:a_same_second_backup_collision_names_the_backup_it_could_not_claim`)                              |

The pending file is removed unconditionally after the publish attempt, whichever way it went, and only
ever the one this run created (`src/main.rs:publish_config`). The backup is never removed once the rename
has succeeded; it is the operator's to keep or delete. A backup CLAIM whose rename failed is removed, so
an empty file named like a backup is never left behind (`src/main.rs:keep_aside_at`, pinned by
`src/main.rs:a_directory_at_the_config_path_is_named_rather_than_the_backup_it_could_not_replace`).

## Behaviors

### 1. The walk is reachable on a machine with no config at all

Given a machine where `$HOME/.config/pns/config.toml` does not exist When the operator runs `pns setup`
Then the walk runs, because the dispatch arm for `setup` sits above everything that loads a config

- Success: `src/main.rs:main` tests `first == "setup"` and calls `setup_mode()` directly, above
  `is_producer_argv` and above `event_mode`. The doc comment states the reason: this is "a MODE that has
  to be reachable with NO CONFIG AT ALL, which is the state it exists to end". Nothing on the event path
  reaches it and it reaches nothing there.
- Failure sources: a first word that is not exactly `setup`. The comparison is byte-exact, so `pns SETUP`
  falls through to `is_producer_argv`, which finds no producer flag, prints the top-level `USAGE` and
  exits 2 (`src/main.rs:main`).
- Fail direction: loud. A word naming no command is refused, never treated as an event
  (`src/main.rs:main`). No partial write is possible: nothing has been opened at this point.
- Thresholds: Not applicable. No numeric threshold governs dispatch.
- Required side effects: none. Dispatch reads argv and calls one function.
- Forbidden side effects: no config load, no probe, no delivery. `setup_mode` is entered before any
  loader runs.
- Timeout and cancellation: Not applicable. No wait exists here.
- Idempotency and duplicates: Not applicable at this layer; see behavior 20.
- Privacy: argv carries no secret. Every secret in this walk is typed at a prompt, never passed as an
  argument or an environment variable (`src/main.rs:walk`).
- Process ownership and cleanup: `setup_mode` returns an exit code that `main` hands to
  `std::process::exit`. The always-exit-0 contract that governs the hook and notification paths does not
  cover this one, and `setup_mode`'s own doc comment says why: it is hand typed and is never a hook.
- Compatibility contract: `pns setup [--force]` is listed in the top-level `USAGE` text
  (`src/main.rs:USAGE`) as "write a first config, one question at a time".

### 2. Exactly one optional word is accepted, and any other is a refusal

Given `pns setup` with zero or more further words When `setup_mode` reads them Then an empty list means
`force = false`, the single word `--force` means `force = true`, and ANY other shape prints the usage
line and exits 2

- Success: `src/main.rs:setup_mode` matches on the slice of `std::env::args_os().skip(2)`, mapped
  lossily. `[]` and `[word] if word == "--force"` are the only two accepted shapes.
- Failure sources: a mistyped flag (`--frce`), an extra word (`--force extra`), a repeated flag
  (`--force --force`), and `--help` or `-h`, none of which this mode answers: help is only handled inside
  the producer parser, which `setup` never reaches (`src/main.rs:main`, `src/main.rs:event_mode`). All of
  them print `pns: usage: pns setup [--force]; --force replaces an existing config, keeping it beside` to
  stderr and exit 2.
- Fail direction: loud and FIRST. The refusal happens before `HOME` is read, before the config is
  located, before the terminal is checked and before a single question is asked. The doc comment states
  the reason: a mistyped `--force` that walked anyway "would ask ten questions and then refuse at the
  end, over a config it was told to replace". No partial write is possible: nothing is opened.
- Thresholds: the accepted argument count is 0 or 1, and 1 only for the exact string `--force`. Two words
  refuse; one wrong word refuses.
- Required side effects: one line on stderr, exit code 2.
- Forbidden side effects: no silent fallthrough to the walk, no config read, and nothing spawned (pinned
  by `tests/dispatch.rs:a_setup_typed_wrong_is_refused_with_what_it_takes_rather_than_walked_anyway`,
  which asserts `sandbox.spawned()` is empty).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: the refusal is pure. Running it again changes nothing.
- Privacy: the usage text names no path and no value. A non-UTF-8 argument is degraded by
  `to_string_lossy` rather than panicking, and a degraded argument is never `--force`, so it refuses.
- Process ownership and cleanup: nothing was opened, so nothing needs cleaning.
- Compatibility contract: `SETUP_USAGE` is the single sentence quoted above. Tests assert only the
  substring `usage: pns setup`
  (`tests/dispatch.rs:a_setup_typed_wrong_is_refused_with_what_it_takes_rather_than_walked_anyway`).

### 3. An unset or empty HOME is refused by name before the config is even located

Given `HOME` is absent from the environment, or is present and empty When `setup_mode` reads it Then it
prints `pns setup: HOME is unset or empty; nothing was written` and exits 2

- Success: `src/main.rs:setup_mode` reads `std::env::var("HOME").ok().filter(|home| !home.is_empty())`.
  Both shapes fail the same guard and produce the same sentence. Pinned by
  `tests/setup.rs:an_empty_home_is_refused_by_name_before_anything_is_written`, which drives BOTH shapes
  (removed and set-to-empty) because a build using `unwrap_or_default()` in place of `.ok()` would catch
  only one.
- Failure sources: a launchd-less or misconfigured shell handing the process no `HOME`.
- Fail direction: loud, and BEFORE the path is composed. The doc comment gives the reason: an empty
  `HOME` would compose a config path relative to the current directory, which is not the operator's own
  machine-wide config no matter where the command was run from. The test asserts positively that nothing
  was written: `!sandbox.root.join(".config").exists()`.
- Thresholds: the guard is emptiness, not length. A `HOME` of one character passes.
- Required side effects: one stderr line, exit 2.
- Forbidden side effects: no directory is created anywhere, least of all a relative `.config/pns` under
  the current directory. The test sets `current_dir` to the sandbox root specifically so that a
  still-unfixed build's relative write would be caught rather than scattered.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure refusal, repeatable.
- Privacy: nothing is printed but the fixed sentence. No environment value is echoed.
- Process ownership and cleanup: nothing opened.
- Compatibility contract: the exact substring `HOME is unset or empty` is asserted, deliberately, so a
  build that reported both shapes as "HOME is empty" would fail
  (`tests/setup.rs:an_empty_home_is_refused_by_name_before_anything_is_written`).

### 4. A config already at the name refuses without --force, and the refusal names the flag

Given `$HOME/.config/pns/config.toml` exists as a name (a regular file, a directory, or a symlink,
dangling or not) When `pns setup` is run without `--force` Then it prints
`pns setup: <path> already exists; pass --force to replace it, which keeps the old file beside it` and
exits 2, leaving the file untouched

- Success: `src/main.rs:setup_mode` calls `path.symlink_metadata()` and takes the `Ok(_) if !force` arm.
  Pinned by
  `tests/dispatch.rs:the_first_run_walk_refuses_a_config_that_is_already_there_and_leaves_it_alone`,
  which reads the config before and after and asserts it is byte-identical, and by
  `tests/setup.rs:a_dangling_symlink_at_the_config_path_is_refused_before_the_first_question`.
- Failure sources: none for the check itself. `symlink_metadata` is used rather than `exists` on purpose:
  `exists` follows a symlink and asks what it resolves to, so a DANGLING link at the config name would
  read as nothing at all, the whole walk would run, and the publish would refuse it with a claim that it
  "appeared while the questions were being answered", which would not be true (`src/main.rs:setup_mode`).
- Fail direction: loud, and BEFORE the terminal check and the first question. The doc comment says the
  ordering is deliberate: the config check is "the more specific answer", so an operator who already has
  one is told that whether or not they are sitting in front of the questions. The dangling-symlink test
  asserts the ordering by checking the refusal does NOT contain `not a terminal`. No partial write is
  possible: nothing is opened.
- Thresholds: Not applicable. Presence is a name existing, not a size or a mode.
- Required side effects: one stderr line, exit 2.
- Forbidden side effects: the existing config is not read, not opened, not moved and not replaced.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure refusal, repeatable.
- Privacy: the refusal names the PATH and nothing of the file's contents. A config full of plugin secrets
  is never read on this arm.
- Process ownership and cleanup: nothing opened.
- Compatibility contract: the tests assert the substrings `already exists` and `--force`.

### 5. A path that does not resolve is refused regardless of --force

Given a name standing anywhere at or above `$HOME/.config/pns/config.toml` that resolves to nothing (for
example `~/.config/pns` is a symlink to a directory that was moved or never created) When `pns setup` is
run, with or without `--force` Then it prints
`pns setup: <path> could not be checked: <ancestor> does not resolve (<cause>); nothing was written` and
exits 2

- Success: the leaf's own `symlink_metadata` fails with `NotFound`, exactly as a genuinely missing config
  does, so `src/main.rs:setup_mode` consults `src/main.rs:unresolvable_ancestor`. That function climbs
  `path.ancestors().skip(1)`: a component that is not there as a name at all means keep climbing (the
  component under it is genuinely missing, which is the ordinary first run), a component whose
  `symlink_metadata` fails for any OTHER reason is returned as the refusal, and the FIRST component that
  exists as a name is then asked whether it LEADS anywhere by calling `metadata`, which follows the link
  that `symlink_metadata` did not. Pinned by
  `tests/setup.rs:a_dangling_link_above_the_config_is_refused_before_the_first_question`, which runs both
  `setup` and `setup --force` and asserts the refusal names the link.
- Failure sources: a dangling link, a symlink loop, or a plain file where a directory belongs, at any
  level above the config.
- Fail direction: loud, fail-closed, and before the first question. The doc comment on
  `unresolvable_ancestor` states the cost of getting it wrong: read as absence, the walk asks every
  question and only fails at publication, "with every answer already typed and every secret already
  handed over". No partial write.
- Thresholds: the climb stops at the FIRST component that exists as a name. Above that everything
  resolves by definition; below it the components really are missing.
- Required side effects: one stderr line naming both the config path and the offending ancestor, exit 2.
- Forbidden side effects: `--force` cannot buy past it. What `--force` agrees to replace is a config, not
  a path that leads nowhere (`src/main.rs:setup_mode`). The test asserts the refusal does not contain
  `not a terminal`, which is how it proves the pre-check fired rather than the tty check.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure refusal, repeatable.
- Privacy: the refusal names paths and an `io::Error` cause. No file content is read.
- Process ownership and cleanup: nothing opened.
- Compatibility contract: the test asserts the refusal contains the link's own display string.

### 6. A config path that cannot be stat'ed for any other reason is refused, also regardless of --force

Given `$HOME/.config/pns` exists but cannot be searched (mode `0o000`, say) When `pns setup` is run, with
or without `--force` Then it prints
`pns setup: <path> could not be checked: <error>; nothing was written` and exits 2

- Success: `src/main.rs:setup_mode`'s final `Err(error)` arm, which catches everything that is not
  `NotFound`. Pinned by `tests/setup.rs:an_unreadable_config_directory_is_refused_by_path_and_cause`,
  which runs both argument shapes and skips itself when running as root, since root reads through any
  mode and the trick cannot produce the error being pinned.
- Failure sources: a permission refusal, `ENOTDIR` from a plain file standing where a directory belongs,
  or any other stat failure.
- Fail direction: loud and fail-closed. The doc comment states it: a directory this walk cannot even stat
  is not one it can safely publish into either. No partial write.
- Thresholds: the discriminator is `error.kind() == NotFound`, nothing else. `NotFound` goes to behavior
  5's climb; every other kind refuses here.
- Required side effects: one stderr line carrying both the config path and the cause, exit 2.
- Forbidden side effects: it REFUSES rather than reports and carries on. The test asserts the refusal
  does not contain `not a terminal`, which would mean the arm printed and fell through, and asserts both
  argument shapes behave the same, because an arm written `if !force` would have passed with only the
  bare shape tested.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure refusal, repeatable.
- Privacy: names the path and the `io::Error`. Nothing is read.
- Process ownership and cleanup: nothing opened. The test restores the directory's mode before any
  assertion can panic past it, so its own sandbox can still be cleaned up.
- Compatibility contract: the test asserts the substrings `could not be checked` and the config path.

### 7. A stdin that is not a terminal refuses the whole walk

Given stdin is a pipe, a file, or `/dev/null` When `pns setup` gets past the argument, `HOME` and config
checks Then it prints
`pns setup: this is a walk through questions and stdin is not a terminal; nothing was written` and exits
2

- Success: `src/main.rs:setup_mode` calls `std::io::stdin().is_terminal()`. Pinned by
  `tests/dispatch.rs:the_first_run_walk_refuses_a_terminal_nobody_is_at_and_writes_nothing`, which
  asserts the exit code, the sentence, that no config was written, and that nothing was spawned.
- Failure sources: any non-terminal stdin. This is also why every test in `tests/setup.rs` that needs the
  walk to actually run opens a real pseudo-terminal (pty) pair with `openpty` rather than driving a pipe.
- Fail direction: loud and fail-closed. `setup_mode`'s doc comment gives the reason: "without a terminal
  there is no walk, and guessing every answer would write a config the operator never agreed to, over one
  they may already have". No partial write.
- Thresholds: Not applicable. `is_terminal` is a yes-or-no on the descriptor.
- Required side effects: one stderr line, exit 2.
- Forbidden side effects: no defaults are guessed and no file is written. The dispatch test asserts both.
- Timeout and cancellation: Not applicable. Nothing is waited on.
- Idempotency and duplicates: pure refusal, repeatable.
- Privacy: nothing read, nothing echoed.
- Process ownership and cleanup: nothing opened.
- Compatibility contract: the test asserts the substring `not a terminal`. Only STDIN is checked; stdout
  and stderr are never asked whether they are terminals.

### 8. The walk prints its preamble, then asks in the order the file is written

Given a terminal and no config When `walk` runs Then it prints `SETUP_PREAMBLE` and then asks the
questions of the table above, in that order, each feature's credentials immediately after the question
that armed it

- Success: `src/main.rs:walk` prints the preamble first:

  ```
  pns setup: a few questions, and a config at the end of them.
  The macOS banner and the phone card are on and are not asked about. Everything
  else is off unless you arm it here, and enter is no. Nothing is written until
  the last answer.
  ```

  The credentials are asked INSIDE the walk, right after the feature they arm, and the doc comment says
  why: "a feature switched on now and credentialed later is exactly the empty-value config this wizard
  exists to avoid".

- Failure sources: any read failing or ending mid-walk (behaviors 16 to 18), which returns `Err` and
  publishes nothing at all rather than composing a file out of half a walk (`src/main.rs:walk`).

- Fail direction: loud and total. `setup_mode` prints `pns setup: <reason>; nothing was written` and
  exits 2 for any walk error. Nothing has been opened at this point, so no partial write is possible.

- Thresholds: six prompts minimum (everything declined), fifteen maximum (everything armed). One step
  either side is unreachable: the six are unconditional and the nine others are each behind a gate.

- Required side effects: the preamble and the prompts on stdout, flushed per prompt (`ask` and
  `ask_hidden` both call `stdout().flush()`, ignoring the result, so a prompt is visible before its
  answer is read).

- Forbidden side effects: nothing is written, created, or moved during the walk. The doc comment on
  `setup_mode` and the preamble both state it: "Nothing is written until the last answer".

- Timeout and cancellation: NO timeout exists on any prompt. `read_line` blocks indefinitely. The
  2-second `PTY_DEADLINE` in `tests/setup.rs` is the TEST harness's own bound so a hang fails by name
  rather than parking a runner, and its own comment says it is "not a timing assertion". Cancellation is
  covered by behavior 15: during a hidden read the interrupt is held, not answered.

- Idempotency and duplicates: the walk itself holds no state. Running it twice asks the same questions.

- Privacy: the four secret prompts (1, 3, 6, 11) go through `ask_hidden`; the rest through `ask`, which
  leaves echo ON. See behavior 27.

- Process ownership and cleanup: the only resource the walk holds is the terminal's echo state, and only
  inside `ask_hidden` (behavior 16).

- Compatibility contract: the pty test waits on the exact prompt tails, so those strings are load
  bearing: `or press enter to pair later: `, `Post every event to hermes`,
  `the signing key that route verifies: `, `Flash hue lights`,
  `the hue bridge's address on the network: `, `an API key the bridge issued: `,
  `the rooms to flash, comma separated`, `home wifi`, `Which router backend?`, `the router's URL: `,
  `an API key the router issued: `, `the phone's hostname on that router: `, `macOS Focus`,
  `which Focus modes mean it, comma separated: `, `approval left unanswered`
  (`tests/setup.rs:a_secret_typed_into_setup_never_reaches_the_pty_output`).

### 9. A typed line becomes an answer by trimming, and whitespace alone is blank

Given the operator types a line and presses Enter When `read_answer` returns it Then `answered` trims it
on both sides and the result is the answer

- Success: `src/main.rs:answered` is `line.trim().to_string()`. Pinned by
  `src/main.rs:an_answer_of_nothing_but_spaces_is_a_blank_one`: `"   \n"`, `"\t\n"` and `"\n"` all become
  `""`, while `"  192.168.1.9  \n"` becomes `"192.168.1.9"` and `"Studio, Kitchen\n"` survives whole.
- Failure sources: none. Trimming cannot fail.
- Fail direction: this is the rule the whole walk rests on, and its fail direction is toward DECLINING. A
  credential that survived as `"  "` would arm its plugin with two spaces: a table that reads as set up
  and delivers nothing, which is the exact state the wizard exists to keep off a fresh machine
  (`src/main.rs:answered`, `src/setup.rs:Answers`). No file is involved yet.
- Thresholds: the trim is Unicode whitespace, per `str::trim`. A single non-whitespace character is an
  answer; any number of spaces and tabs is not.
- Required side effects: none. `answered` is pure.
- Forbidden side effects: it must not eat a real answer. The test pins both directions for that reason.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure function.
- Privacy: the trimmed secret is what reaches the file. Trimming means a token pasted with a trailing
  newline or a stray space still works.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: interior whitespace is preserved (`Studio, Kitchen` stays as typed); only the
  ends are trimmed.

### 10. Only a typed yes arms a feature

Given a yes-or-no question When `means_yes` judges the answer Then `y`, `yes`, `Y`, `YES` and `Yes` are
yes and everything else, including Enter, is no

- Success: `src/main.rs:means_yes` is `matches!(answer.to_lowercase().as_str(), "y" | "yes")`. Pinned by
  `src/main.rs:the_only_answer_that_arms_a_feature_is_a_yes_somebody_typed`, which asserts the five
  accepted spellings and rejects `""`, `n`, `no`, `N`, `sure`, `ok`, `yeah`, `yep`, `y ` and `1`.
- Failure sources: none. The predicate cannot fail.
- Fail direction: toward NO. The doc comment states the reason: every question this answers arms
  something that delivers to a phone or to a lamp and takes a credential to do it, so the answer nobody
  typed on purpose has to be the one that changes nothing. A predicate reading "not a no" would arm the
  whole walk by default and still pass every test about the file.
- Thresholds: exactly two accepted lowercased forms. `y ` is rejected by `means_yes` in the unit test,
  though in the walk itself `answered` would have trimmed it to `y` first, so a trailing space typed at a
  live prompt does arm the feature. `yeah` and `yep` are rejected.
- Required side effects: none. Pure.
- Forbidden side effects: no prompt is re-asked and no complaint is printed for a word nobody meant. A
  non-yes silently means no.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure function.
- Privacy: the yes-or-no answers are echoed; the pty test asserts `[y/N]: y\r\n` appears in the
  transcript, which is how it proves echo came back on for ordinary questions.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the prompt suffix is exactly ` [y/N]`, appended by `ask_yes`.

### 11. A comma-separated answer names only the values somebody typed

Given a list prompt (hue rooms, Focus modes) When `list` splits the answer Then it splits on `,`, trims
each piece, and drops every empty one

- Success: `src/main.rs:list`. Pinned by
  `src/main.rs:a_comma_separated_answer_names_only_the_values_somebody_typed`: `"Studio, Kitchen"` gives
  `["Studio", "Kitchen"]`, `"Studio, , Kitchen,"` gives the same two, `"  Studio  "` gives `["Studio"]`,
  and both `""` and `" , "` give an empty list.
- Failure sources: none.
- Fail direction: toward FEWER values, and ultimately toward declining the feature. A blank between two
  commas would reach the file as `rooms = [""]`, which the bridge matches to no room at all while the
  table reads as configured (`src/main.rs:list`).
- Thresholds: an empty result is what `hue_is_armed` and the `[focus]` composition read as a decline
  (`src/setup.rs:hue_is_armed`, `src/setup.rs:Answers::values`). One surviving value arms; zero declines.
- Required side effects: none. Pure.
- Forbidden side effects: no de-duplication, no sorting, no case folding. The prompt tells the operator
  to spell the rooms as the bridge spells them, and the answer is carried verbatim.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: a repeated room name is kept twice. NOT ESTABLISHED: nothing in
  `src/main.rs:list` or its test de-duplicates, and no test asserts what a duplicate room does
  downstream.
- Privacy: room names and Focus mode names are echoed at the prompt, since both go through plain `armed`.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the separator is a comma and only a comma. Semicolons and spaces are part of
  the value.

### 12. The router backend is the one answer judged against the code rather than the operator's network

Given the backend question When `router_backend` judges the answer Then an empty answer and any case
spelling of `unifi` both name `pns::home::UNIFI_TYPE`, and every other answer is `None`

- Success: `src/main.rs:router_backend` is
  `(answer.is_empty() || answer.eq_ignore_ascii_case(pns::home::UNIFI_TYPE)).then_some(pns::home::UNIFI_TYPE)`,
  and `src/home.rs:UNIFI_TYPE` is `"unifi"`. The accepted answer is written back AS THE CODE SPELLS IT,
  never as it was typed, because the probe compares the whole string and would refuse the operator's
  capitals. Pinned by `src/main.rs:the_only_backend_the_walk_accepts_is_one_the_home_probe_answers`.
- Failure sources: a router brand nobody implements. `asus`, `unifi-controller`, `u`, `unifix` and `eero`
  are all `None` in the test.
- Fail direction: toward DECLINING the home probe, loudly at the prompt. `walk` prints
  `  nothing here reads that router, so the home probe stays off; the file says how to arm it` and skips
  the three router credential prompts. The composer then declines the table a second time
  (`src/setup.rs:router_is_armed`, pinned by
  `src/setup.rs:a_backend_the_home_probe_cannot_answer_declines_the_probe_rather_than_arming_it`). The
  reason the check exists at all: every key of the router table is free text to the parser, so a backend
  name nothing implements would compose a file that LOADS, is reported as written, and then refuses at
  the first probe.
- Thresholds: the accepted set is exactly one backend today, and it is read off `home` rather than
  restated, so the day a second backend lands `home::router_settings` is what has to agree. The setup
  unit test proves the agreement directly by calling `crate::home::router_settings` on the armed walk's
  own parsed table.
- Required side effects: for an unanswerable backend, one line on stdout, and `Answers::router_type`
  stays empty.
- Forbidden side effects: the walk does NOT end. An unanswerable backend declines the home probe and the
  walk continues to the Focus question (`src/main.rs:walk`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure function.
- Privacy: the refusal line does NOT quote the answer the operator typed. It says "that router", not the
  brand name. (By contrast `src/home.rs:router_settings` does quote an unknown type at probe time, but
  that reads a config value, not a wizard answer.)
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the prompt interpolates the backend name, so it reads
  `Which router backend? [unifi]` and moves with `UNIFI_TYPE`.

### 13. A blank credential declines its feature, says so, and gates the questions after it

Given a credential prompt answered with Enter When `nothing_given` sees the empty answer Then it prints
`  nothing given, so <feature> stays off; the file says how to arm it` and the remaining credentials for
that feature are not asked

- Success: `src/main.rs:armed` and `src/main.rs:armed_secret` both wrap their read in
  `src/main.rs:nothing_given`. The gating is explicit in `walk`: `if !answers.hue_bridge.is_empty()`
  before the hue key, `if !answers.hue_key.is_empty()` before the rooms,
  `if !answers.router_url.is_empty()` before the router key, `if !answers.router_api_key.is_empty()`
  before the hostname. The comment states it: once one comes back empty the feature is already declined,
  and the rest would be questions whose answers are thrown away.
- Failure sources: none. Printing a line cannot fail meaningfully.
- Fail direction: toward a DECLINED feature written as a commented block. `src/setup.rs:Answers::values`
  inserts a plugin table only when its credentials are all non-empty, and
  `src/setup.rs:a_credential_left_blank_declines_its_feature_rather_than_arming_an_empty_one` tries each
  required field on its own and asserts three things: the feature is gone, the OTHER features are
  untouched (exactly four plugins remain), and no setting anywhere is written as an empty string.
- Thresholds: the predicate is `is_empty` on the trimmed answer, which is why behavior 9's trim is load
  bearing.
- Required side effects: exactly one two-space-indented line per blank credential. The feature names used
  are `hermes`, `the light pulse`, `the home probe` and `focus silencing` (`src/main.rs:walk`).
- Forbidden side effects: a blank answer must not cost the walk anything the operator DID fill in. The
  test asserts the surviving plugin count for exactly that reason.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure apart from the print.
- Privacy: the line names the FEATURE, never the answer. A blank answer has nothing to leak; a non-blank
  one is never echoed by this function.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: prompt 1 (the moshi token) is the one credential that does NOT go through
  `nothing_given`: it calls `ask_hidden` directly, so declining it prints nothing and leaves the mobile
  plugin ON and uncarded (`src/main.rs:walk`,
  `src/setup.rs:a_skipped_token_is_commented_out_rather_than_written_empty`).

### 14. A secret is read with the terminal's echo already off before its prompt is printed

Given a secret prompt (1, 3, 6 or 11) When `ask_hidden` runs Then the `Hushed` guard is armed FIRST, the
prompt is printed second, and the answer is read third

- Success: `src/main.rs:ask_hidden` binds `let _hushed = Hushed::arm()?;` before the `print!`.
  `src/main.rs:Hushed::arm` reads the current termios with `tcgetattr`, blocks nine signals, then clears
  `ECHO` and sets `ECHONL` and applies it with `TCSAFLUSH`. Pinned by
  `tests/setup.rs:a_secret_typed_into_setup_never_reaches_the_pty_output`, which reads until the first
  prompt is VISIBLE and then asserts `pty.tcgetattr().c_lflag & libc::ECHO == 0`.
- Failure sources: `tcgetattr` failing, `sigemptyset` or `sigaddset` failing, `pthread_sigmask` failing,
  or `tcsetattr` failing. Each returns its own named `Err`:
  `the terminal's settings could not be read (tcgetattr: <e>)`,
  `the signal mask could not be built (sigemptyset: <e>)` and the `sigaddset` twin,
  `signals could not be held for the read (pthread_sigmask: <e>)`, and
  `the terminal's echo could not be turned off (tcsetattr: <e>)`.
- Fail direction: FAILS CLOSED, and this is the important one. A termios or signal call that cannot be
  completed is refused as loudly as a bad answer rather than silently leaving echo on and asking for a
  secret anyway (`src/main.rs:Hushed::arm`). The refusal propagates through `walk` to
  `pns setup: <reason>; nothing was written`, exit 2. No file exists yet, so no partial write.
- Thresholds: `TCSAFLUSH` is the flag on both the arm and the restore. It also DISCARDS whatever was
  already queued, so a secret typed ahead of its own prompt is lost rather than read, and so is an answer
  typed ahead of the question after it. The pty test's driver is written around that: every answer is
  written only after its own prompt is visible.
- Required side effects: `ECHO` cleared and `ECHONL` set on stdin's terminal for the duration of the
  read. `ECHONL` is what makes the typed Enter still echo, so the display advances; the test asserts the
  resulting `": \r\nPost every event"` shape with no echoed secret in between.
- Forbidden side effects: no window in which the prompt is visible while echo is still on. The doc
  comment says arming after the prompt "would leave a window in which the prompt is already visible but
  echo is still on, so an operator who types ahead of it, or this crate's own pty test, could still have
  a secret echoed before `TCSAFLUSH` takes hold".
- Timeout and cancellation: no timeout. Cancellation is HELD, see behavior 15.
- Idempotency and duplicates: each secret prompt arms and drops its own guard, so four guards are armed
  and dropped in a fully armed walk. Nested guards do not occur.
- Privacy: this is the mechanism. One client is outside its reach and the code says so: mosh, the
  transport under a Moshi-connected phone, predicts keystrokes locally and can draw them on that client
  transiently, ahead of the terminal's own echo state. Nothing here controls that
  (`src/main.rs:ask_hidden`).
- Process ownership and cleanup: the guard owns the terminal state and the signal mask; `Drop` gives both
  back (behavior 16).
- Compatibility contract: only STDIN's terminal is touched (`libc::STDIN_FILENO` throughout). On the
  failing `tcsetattr` path, `arm` restores the signal mask with `SIG_SETMASK` before returning the error,
  so a failed arm leaves no mask behind.

### 15. Nine signals are held for the duration of a hidden read and delivered after the terminal is given back

Given a hidden read in progress When a signal arrives Then it is BLOCKED for the read, and delivered only
once `Hushed::drop` has restored the terminal and then the mask

- Success: `src/main.rs:Hushed::arm` blocks `SIGINT`, `SIGQUIT`, `SIGTSTP`, `SIGTERM`, `SIGHUP`,
  `SIGTTIN`, `SIGTTOU`, `SIGALRM` and `SIGPIPE` with `pthread_sigmask(SIG_BLOCK, ...)`, which is the set
  `readpassphrase(3)` holds. `src/main.rs:Hushed::drop` restores the termios FIRST and the mask second.
  Pinned by `tests/setup.rs:a_signal_sent_during_the_hidden_read_is_held_until_the_guard_drops`, which
  runs five separate sandboxes, one per signal that ends the process by default (`SIGINT`, `SIGALRM`,
  `SIGTERM`, `SIGQUIT`, `SIGHUP`), and for each asserts the child is still alive immediately after
  `kill`, that the process eventually dies FROM that signal, and that the terminal's `ECHO` bit is back
  on when it does.
- Failure sources: `pthread_sigmask` returning non-zero at arm time, which refuses (behavior 14). At drop
  time neither call's failure is checked, deliberately: a terminal that hung up during the read makes
  `tcsetattr` fail, and `Drop` must never panic over a terminal already gone
  (`src/main.rs:Hushed::drop`).
- Fail direction: toward HOLDING rather than acting. The trade is stated in `ask_hidden`'s doc comment:
  each signal is still delivered, just not until the guard drops, so Ctrl-C takes effect at the next
  Enter rather than instantly. No file is open during a hidden read, so a signal cannot tear a write.
- Thresholds: exactly nine signals. Two of them are held for reasons that are not observable today and
  the code says so: `SIGPIPE` is inert because the Rust runtime sets it to `SIG_IGN` before `main`, and
  `SIGTTIN` cannot be observed by the test harness because a pending tty-stop signal is discarded once
  the process group is orphaned. `SIGKILL` and `SIGSTOP` are not in the set. NOT ESTABLISHED: what the
  terminal's echo state is after a `SIGKILL` during a hidden read; `Drop` does not run, no test covers
  it, and nothing in the crate re-arms the terminal on a later invocation.
- Required side effects: the mask is restored to what it was, using the saved `original_mask`, not to an
  empty set.
- Forbidden side effects: signals are BLOCKED, never ignored or handled. The doc comment insists on the
  full set rather than a shorter one, because "a quietly shorter set is the model's holes without its
  name".
- Timeout and cancellation: this IS the cancellation model. There is no other. A Ctrl-C during a
  non-hidden prompt is not held and ends the process in the ordinary way, with no terminal state to
  restore because no guard is armed there.
- Idempotency and duplicates: a second signal of the same number arriving while the first is pending is
  coalesced by the kernel's ordinary pending-signal semantics. NOT ESTABLISHED: no test sends two.
- Privacy: `SIGALRM` is in the set specifically so that an alarm armed before the walk began cannot end
  the process mid-prompt and leave the operator's terminal echo-off with no prompt in front of it.
- Process ownership and cleanup: `SIGTTOU` is held because `Drop`'s own `tcsetattr` from a background
  process group can raise it before the restore gets the chance to happen.
- Compatibility contract: `pthread_sigmask` is POSIX and RETURNS its error number rather than setting
  errno, so `arm` builds its message from the returned value with `Error::from_raw_os_error`, never from
  `last_os_error()` (`src/main.rs:Hushed::arm`).

### 16. The terminal's echo is restored on every exit path the guard can reach

Given a hidden read that is over, however it ended When the `Hushed` value goes out of scope Then `Drop`
restores the saved termios and then the saved signal mask

- Success: `src/main.rs:Hushed::drop`. The order is termios first, then the mask, because a signal
  delivered between the two would otherwise run with the operator's terminal still echo-off. Pinned twice
  from outside the process: `tests/setup.rs:a_secret_typed_into_setup_never_reaches_the_pty_output`
  asserts `ECHO` is back on after the wizard has already exited (a pty's master keeps reporting the
  slave's last settings after the slave side closes), and
  `tests/setup.rs:a_signal_sent_during_the_hidden_read_is_held_until_the_guard_drops` asserts the same
  for all five observable signals, which is the path where the process dies rather than returning.
- Failure sources: a terminal that hung up during the read. `tcsetattr` then fails and the failure is
  deliberately unchecked.
- Fail direction: best effort, and it cannot make things worse: the terminal it failed to restore is one
  that is already gone.
- Thresholds: Not applicable.
- Required side effects: the terminal's `c_lflag` returns to exactly the `original` captured at arm time,
  not to a synthesized "echo on" state.
- Forbidden side effects: `Drop` must never panic. Neither call's result is examined.
- Timeout and cancellation: Not applicable. Both calls are non-blocking.
- Idempotency and duplicates: `Hushed` is not `Copy` or `Clone`, so exactly one `Drop` runs per arm.
- Privacy: this is what keeps a secret prompt from leaving the operator's shell echo-off after the walk,
  which would silently hide their next typed command.
- Process ownership and cleanup: the crate carries NO `panic = "abort"`, so `Drop` runs on an unwinding
  panic as well as on a normal return (`src/main.rs:Hushed`). That matters because
  `src/setup.rs:compose_config` can panic (behavior 19), although by then every guard has already
  dropped. NOT ESTABLISHED as an assertion: the read-failure path in
  `tests/setup.rs:a_non_utf8_paste_is_reported_as_a_read_failure_rather_than_the_answers_ending`
  exercises a guard dropping on an `Err` return but does not assert the echo state afterwards; the
  restoration on that path rests on `Drop` semantics and on the two tests above.
- Compatibility contract: the restore also uses `TCSAFLUSH`, so anything typed ahead of the prompt AFTER
  the secret is discarded too.

### 17. A read from a background process is named as job control, not as an input fault

Given `pns setup &`, so the walk's process group does not own the terminal When a hidden read fails with
`EIO` Then the refusal is
`this walk cannot read the terminal from the background; bring it to the foreground with fg`

- Success: `src/main.rs:read_answer` passes the error and `src/main.rs:reading_from_the_background()` to
  `src/main.rs:read_failure`, which requires BOTH halves: the terminal is owned by another process group
  AND the raw errno is `EIO`. Pinned by
  `src/main.rs:a_background_read_names_job_control_rather_than_an_io_fault`.
- Failure sources: the underlying mechanism is termios(4): a background process that BLOCKS `SIGTTIN`,
  which the hidden read does, gets `EIO` from the read "and no signal is sent", where an unblocked one
  would have been stopped and could be resumed with `fg`.
- Fail direction: loud and non-zero, through `walk`'s `Err` to
  `pns setup: <reason>; nothing was written`, exit 2. Nothing is open, so no partial write.
- Thresholds: `reading_from_the_background` is `foreground > 0 && foreground != getpgrp()`. A FAILED
  `tcgetpgrp` answers -1 and is NOT this case, because a terminal that hung up answers -1 as well and
  that read really did fail for its own reason. A zero is no foreground group at all, which is not this
  case either.
- Required side effects: one refusal string. Nothing is retried and nothing is backgrounded or
  foregrounded on the operator's behalf.
- Forbidden side effects: an `EIO` in the FOREGROUND must keep its own reason. The test asserts
  `read_failure(&eio, false)` still says `the answers could not be read`, because a hung-up terminal
  answers `EIO` too.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure function of the error and the flag.
- Privacy: the refusal carries no answer and no path.
- Process ownership and cleanup: the guard drops on the way out, restoring echo before the process exits.
- Compatibility contract: the two substrings the test pins are `bring it to the foreground with fg` and
  `the answers could not be read`.

### 18. A read that fails keeps its own reason, and input ending is a different reason again

Given a byte that is not valid UTF-8 pasted at a prompt, or the input closing When `read_answer` handles
it Then a failed read reports `the answers could not be read: <the io::Error>` and a closed input reports
`the answers ended before the walk did`, and the two are never confused

- Success: `src/main.rs:read_answer` distinguishes `Ok(0)` (input ended) from `Err(error)` (the read
  failed) from `Ok(_)` (an answer). Pinned end to end by
  `tests/setup.rs:a_non_utf8_paste_is_reported_as_a_read_failure_rather_than_the_answers_ending`, which
  writes a bare `0xFF` byte followed by a newline into a real pty and asserts exit code 2, the substring
  `the answers could not be read`, the underlying detail `valid UTF-8`, and positively that the
  transcript does NOT say `the answers ended before the walk did`.
- Failure sources: a non-UTF-8 paste is the realistic one, because this walk asks for pasted answers.
  `ISTRIP` is off on the test's pty, so the byte reaches `read_line` unmangled.
- Fail direction: loud and total, through `walk`'s `Err`. `setup_mode` prints
  `pns setup: <reason>; nothing was written` and exits 2. Nothing is open, so no partial write.
- Thresholds: `Ok(0)` versus `Err`. There is no retry count and no second chance at a prompt: one bad
  read ends the walk.
- Required side effects: one stderr line, exit 2.
- Forbidden side effects: no partial `Answers` is composed. The doc comment on `walk` says an `Err` "is
  the walk ending mid-conversation, named by its own reason, which publishes nothing at all rather than
  composing a file out of half of one".
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure.
- Privacy: the underlying `io::Error` is `stream did not contain valid UTF-8`, which names the fault and
  not the bytes. NOT ESTABLISHED for the general case: `read_failure` interpolates whatever
  `io::Error::Display` produces, and no test enumerates every error whose text could carry input. For the
  one reachable non-UTF-8 case the text is fixed by the standard library and carries no input.
- Process ownership and cleanup: the guard drops before the process exits, restoring echo.
- Compatibility contract: the test pins the detail (`valid UTF-8`) as well as the generic prefix, so a
  build that reported the same generic prefix for every read failure fails.

### 19. Composition is a pure function of the answers, and a wizard's own answers always render

Given a completed `Answers` When `setup_mode` composes the file Then `pns::setup::compose_config` renders
the whole config text from `Answers::values()` alone

- Success: `src/setup.rs:compose_config` calls `crate::config_text::render(&answers.values())` and
  `expect`s it. Everything about WHAT lands in the file is `src/setup.rs` and `src/config_text.rs`; see
  `docs/specs/configuration.md` for the layout, which tables are commented out, the core defaults written
  unprompted, and the roster scan. The module comment states the split: `walk` "does nothing but ask,
  read a line, and hand what came back to `compose_config`".
- Failure sources: `render` can return `Err` for an unknown plugin or an unknown top-level key
  (`src/config_text.rs:render`). `compose_config` turns that into a PANIC with the message
  `a wizard's own answers always render`.
- Fail direction: a panic, deliberately. The doc comment argues it cannot be reached by an operator:
  every value `values()` composes is a plain literal off the roster this wizard's own layout serves, "so
  the only way this expect fires is a bug in `values()` itself, not an operator's input". Because it
  panics BEFORE `publish_config` is called, no file is opened and no partial write is possible. The
  process aborts with a panic message rather than a `pns setup:` refusal.
- Thresholds: Not applicable.
- Required side effects: none. `values()` and `compose_config` are pure.
- Forbidden side effects: the composed text is NEVER printed. `src/main.rs:setup_mode` holds it in
  `composed` and passes it only to `parse_config` and `publish_config`.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: the same `Answers` compose byte-identical text every time.
- Privacy: `src/setup.rs:a_wizard_render_carries_no_chezmoi_action_because_every_answer_is_a_literal`
  asserts the composed text contains no `{{`, so a wizard's own file is real TOML (Tom's Obvious Minimal
  Language) from the first line and never a chezmoi template holding a secret marker. Untrusted text is
  quoted properly:
  `src/setup.rs:a_credential_carrying_quotes_and_backslashes_reaches_the_config_as_itself` pastes `a"b\c`
  as the hermes key and asserts it parses back identically, which is what stops a pasted secret from
  ending its own value at the operator's own quote.
- Process ownership and cleanup: `Answers` holds every secret as a plain heap `String` and nothing
  zeroizes it; the crate has no `zeroize` dependency. `Answers` also derives `Debug`, so a `{:?}` would
  print every secret in the clear. Nothing in the crate does that today (no `answers` value is
  debug-formatted anywhere in `src/main.rs`).
- Compatibility contract: `compose_config` takes `&Answers` and returns `String`. The wizard's own
  answers must stay within the roster `config_text::render` serves; the anti-drift fence for that is
  `src/setup.rs:every_key_it_writes_is_a_key_the_roster_serves_however_the_walk_was_answered`.

### 20. The composed text goes through the engine's own parser before anything is written

Given composed text When `setup_mode` reaches publication Then it calls
`pns::config::parse_config(&composed)` first, and a refusal there writes nothing

- Success: `src/main.rs:setup_mode` prints
  `pns setup: what it composed does not load (<detail>); nothing was written` and returns 2 on any parse
  error. The doc comment states the reason: "A wizard that writes a config pns then refuses is worse than
  no wizard: it leaves a machine falling back to the core with a complaint nobody is standing in front
  of, and it does it while the operator is being told it worked."
- Failure sources: only a bug in the composer, since behavior 19 already guarantees the text renders. The
  unit tests parse both ends of the walk on every run (`src/setup.rs:parsed`, used by every test in that
  module).
- Fail direction: loud and pre-write. `publish_config` is never called, so nothing is created, nothing is
  moved, and no partial write is possible.
- Thresholds: Not applicable.
- Required side effects: one stderr line, exit 2.
- Forbidden side effects: no file is opened before this check passes.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: pure.
- Privacy: `ConfigError::detail()` is documented as "already sanitized for printing"
  (`src/config.rs:ConfigError::detail`). For a `Malformed` error the detail is REBUILT from the parser's
  own message plus a line number, deliberately not echoing the offending source line, because the config
  carries plugin secrets into log lines; pinned by
  `src/config.rs:a_malformed_line_is_reported_without_echoing_its_value`, which parses
  `token = "SUPERSECRET" trailing` and asserts the message does not contain the value. For an `Invalid`
  error from a plugin table, the only judgement made is on KEY NAMES (`src/config.rs:admits_flat`,
  `src/config.rs:unknown_key`), so a plugin's VALUE, which is where every secret lives, is never
  interpolated into a refusal. Some non-plugin `Invalid` arms do quote values (the `recap` key
  `review_notes` pattern, the numeric bounds), but the wizard writes none of those from an answer.
- Process ownership and cleanup: nothing held.
- Compatibility contract: the check uses the same `parse_config` the engine itself loads with, not a
  second validator.

### 21. A first config is published through a pending file and a hard link, and leaves nothing behind

Given no config at the name When `publish_config` runs with `force = false` Then it creates the
directory, creates a pending file with `create_new`, forces its mode to `0o600`, writes the whole
composed text, hard-links it to the config path, removes the pending name, and answers `Ok(None)`

- Success: `src/main.rs:publish_config` and `src/main.rs:write_then_publish`. Pinned by
  `src/main.rs:a_first_config_is_published_for_its_operator_alone_and_leaves_no_pending_file`, which
  asserts the answer is `Ok(None)`, the file's contents, the mode `0o600`, and that the directory holds
  nothing besides `config.toml`. Also pinned end to end by
  `tests/setup.rs:a_secret_typed_into_setup_never_reaches_the_pty_output`, which reads the published file
  and asserts its mode is `0o600`.
- Failure sources: the directory cannot be created (`<parent> could not be created: <error>`), the
  pending file cannot be created (`<pending> could not be written: <error>`), the mode cannot be forced
  (`<pending> could not be secured: <error>`), the write fails
  (`<pending> could not be written: <error>`), or the link fails (behavior 22). Every one of them returns
  `Err`, which `setup_mode` prints as `pns setup: <refusal>` and exits 1, NOT 2: publication refusals are
  the only exit-1 path in this mode (`src/main.rs:setup_mode`).
- Fail direction: the config path is never partially written. The whole composed text is written to the
  pending name first, and the config path only ever comes into existence as a second name for that
  already-complete inode. A failure before the link leaves the config path absent, which is the state the
  run started in. NOT ESTABLISHED: durability. Nothing calls `fsync`, `sync_all` or `sync_data` anywhere
  in `src/main.rs`, so what survives a power loss between the write and the link is not settled by the
  code.
- Thresholds: `CONFIG_FILE_MODE` is `0o600` (`src/main.rs:CONFIG_FILE_MODE`), and its own doc comment
  gives the reason: "The config carries every plugin's secret, so it is the operator's alone." One step
  looser (`0o640`, `0o644`, or whatever the umask alone would give) hands the moshi token and the hue key
  to other processes; the test asserts the exact value rather than a bound.
- Required side effects: the directory, the pending file (transient), the config file. In that order.
- Forbidden side effects: no pending file survives. `publish_config` removes it unconditionally after
  `write_then_publish` returns, whichever way it went, and removes only the one this run created.
- Timeout and cancellation: Not applicable. No wait, no subprocess, no network.
- Idempotency and duplicates: running setup twice without `--force` is refused at behavior 4, long before
  this point. The pending name carries the process id AND the current instant's sub-second nanoseconds,
  so two concurrent runs cannot collide on it (`src/main.rs:pending_name`).
- Privacy: the pending file carries every secret the config does, which is why it is created WITH the
  mode rather than chmodded into it afterwards, and why it never outlives the publish
  (`src/main.rs:a_first_config_is_published_for_its_operator_alone_and_leaves_no_pending_file`). Note the
  mode is applied twice on purpose: `.mode()` on the open is masked by the umask, and the
  `set_permissions` on the open handle in `write_then_publish` is what forces it. The DIRECTORY, by
  contrast, is created with `create_dir_all` and no explicit mode, so it is `0o777` masked by the umask.
- Process ownership and cleanup: `publish_config` owns the cleanup and `write_then_publish` owns the
  publish, which is why the split exists.
- Compatibility contract: `publish_config` returns `Result<Option<PathBuf>, String>`, where `Ok(None)` is
  "nothing was kept aside" and `Ok(Some(backup))` is where the old config went.

### 22. A config that appeared during the walk is refused rather than written over

Given a config that did not exist when the walk started but does when it ends When the publish links the
pending file to the config path Then the link fails with `AlreadyExists` and the run refuses with
`<path> appeared while the questions were being answered; nothing was written over it`

- Success: `src/main.rs:write_then_publish` matches `ErrorKind::AlreadyExists` explicitly. Pinned by
  `src/main.rs:a_config_that_appeared_during_the_walk_is_refused_rather_than_written_over`, which asserts
  the refusal says `appeared`, that the pre-existing config is byte-identical afterwards, and that no
  pending file was left behind.
- Failure sources: another writer creating the config while the questions were being answered. The
  questions take minutes.
- Fail direction: refuses rather than replaces, and the config that was there is untouched. The doc
  comment states the rule: "CREATE-IF-ABSENT, NEVER A BLANKET RENAME, on both paths ... The link failing
  with `AlreadyExists` IS that refusal." Nothing asks whether a config is there first, because the answer
  stops being true the instant it is given.
- Thresholds: Not applicable.
- Required side effects: the pending file is still removed. Exit 1 from `setup_mode`.
- Forbidden side effects: no overwrite, no rename over the arrival, no truncation.
- Timeout and cancellation: no retry and no wait. One attempt.
- Idempotency and duplicates: re-running the whole walk is the only way forward, and it will now hit
  behavior 4's refusal instead.
- Privacy: the arrival is never read, so its contents cannot leak into a message. The refusal names the
  path only.
- Process ownership and cleanup: the pending file is removed by `publish_config`'s unconditional
  `remove_file`.
- Compatibility contract: because the dangling-symlink pre-check in `setup_mode` (behavior 4) already
  refuses a link that leads nowhere, the word "appeared" is exact rather than one of two guesses
  (`src/main.rs:write_then_publish`).

### 23. A leftover pending file is never the file this run writes into

Given a pending name left behind by an abandoned run of a process id that has since been reused When this
run opens its own pending file Then `create_new` refuses to reuse the name, so the leftover is never
truncated

- Success: `src/main.rs:publish_config` opens with `.create_new(true)`. The doc comment gives the exact
  hazard: a pending file is a second name for the LIVE config between the link that publishes it and the
  unlink that removes it, so an abandoned run leaves one behind, and process ids are reused. An open that
  truncated would empty a config this run has not read, "and the backup taken next would hold the
  replacement". Pinned by
  `src/main.rs:a_pending_file_left_by_an_abandoned_run_is_never_the_file_this_one_writes_into`, which
  hard-links a leftover to the live config and then asserts, after a forced publish, that the backup
  holds the OLD text and that the leftover's own contents were left alone.
- Failure sources: `create_new` failing with `AlreadyExists` on a genuine collision, which refuses with
  `<pending> could not be written: File exists`.
- Fail direction: refuses rather than truncates. `pending_name` carries the instant as well as the
  process id specifically so that a leftover from an abandoned run of the same id "would otherwise refuse
  a wizard nobody can unblock" (`src/main.rs:pending_name`).
- Thresholds: the name's discriminators are the process id and `subsec_nanos()` of the current instant,
  which is `0` when the clock cannot be read (`map_or(0, ...)`). NOT ESTABLISHED: no test drives a
  collision on the FULL pending name; the leftover test uses the older, shorter shape
  `config.toml.new.<pid>`, which the current `pending_name` never generates, so what it actually pins is
  that no leftover is truncated rather than that the exact name collides.
- Required side effects: none beyond the refusal.
- Forbidden side effects: removing a pending file this run did not create. `publish_config` removes "only
  ever the file the line above made", and the doc comment calls removing another run's file "the mirror
  of the write it refuses to do".
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: two runs in the same process at the same nanosecond are not reachable.
- Privacy: a leftover pending file holds a complete config, secrets included, at mode `0o600`, in the
  config directory, until somebody removes it. Nothing in this crate sweeps them.
- Process ownership and cleanup: an abandoned run's pending file is the operator's to remove.
- Compatibility contract: the pending name is `config.toml.new.<pid>.<nanos>`, so it sorts beside the
  config and is obviously not one.

### 24. --force moves the old config aside BEFORE the new one is published

Given `--force` and an existing config When the publish runs Then `keep_aside` claims a stamped backup
name, renames the existing config onto it, and only then does the hard link publish the new file

- Success: `src/main.rs:write_then_publish` calls `keep_aside(path)?` before `hard_link`. Pinned by
  `src/main.rs:a_forced_replacement_keeps_the_old_config_before_it_writes_the_new_one`, and the test
  comment explains why reading the BACKUP is the assertion that says "first": taken afterwards it would
  be a copy of the replacement, the old file would be gone, and the line printed to the operator would
  name a path that does not hold what it says it holds.
- Failure sources: the clock cannot be read (behavior 25), the name cannot be claimed (behavior 26), or
  the rename fails.
- Fail direction: a `keep_aside` failure aborts the publish with `?`, so the new config is NOT written
  and the old one is still at its name. The one asymmetric case is a rename that SUCCEEDS followed by a
  link that FAILS: the config path is then EMPTY and the old config is at the backup name. The refusal
  carries the tail `; the config that was there is kept at <backup>` (`src/main.rs:also_kept`), so nobody
  is left hunting for a file the wizard took the name of. NOT ESTABLISHED: no test exercises `also_kept`;
  no test in `src/main.rs` or `tests/` references that sentence, so the combined state (backup written,
  config path empty, refusal naming both) rests on reading the code.
- Thresholds: the move is a `rename`, not a copy. The doc comment states the invariant that buys: "the
  old config is at one of the two names at every instant."
- Required side effects: exactly one backup, named in the operator-facing line
  `pns setup: kept the old config at <backup>` on stdout, printed BEFORE `pns setup: wrote <path>`
  (`src/main.rs:setup_mode`).
- Forbidden side effects: no copy is taken. A copy says only what stood at the name when the copy ran,
  and the publish that follows replaces whatever stands there THEN (`src/main.rs:keep_aside`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: a second forced run in the same second collides on the backup name and
  refuses (behavior 26). A second forced run a second later takes a second backup, and the first is left
  alone.
- Privacy: the backup holds the PREVIOUS config in full, plugin secrets included, and it is chmodded to
  `0o600` after the move for exactly that reason:
  `a_forced_replacement_keeps_the_old_config_before_it_writes_the_new_one` asserts the backup's mode with
  the comment "a backup of a config full of plugin secrets is a config full of plugin secrets". The chmod
  is skipped when what moved is not a regular file, because the mode of a symlink is the mode of what it
  points at, and that is a file this run did not replace and has no business changing
  (`src/main.rs:keep_aside_at`).
- Process ownership and cleanup: nothing ever removes a backup. It accumulates, one per forced run.
- Compatibility contract: what the backup holds is what the publish REPLACED, not what that config NAMED.
  With a symlinked config, the LINK is moved to the backup and the file at the far end is left untouched,
  pinned by `src/main.rs:a_forced_run_keeps_the_config_it_replaced_rather_than_what_that_config_named`
  and `src/main.rs:a_forced_run_keeps_a_config_the_existence_check_reads_as_absent`, which both read the
  backup with `read_link`.

### 25. A clock that cannot be read names no backup at all, and that is a refusal

Given `--force` and a clock that cannot be read, or an epoch second no calendar can express When
`keep_aside` asks for a backup name Then there is no name, and the run refuses rather than replacing the
config

- Success: `src/main.rs:keep_aside` refuses with
  `the clock cannot be read, so the config already there cannot be named and kept; nothing was written`
  when `now_secs()` is `None`. `src/main.rs:keep_aside_at` refuses with
  `<path> cannot be named for keeping, so the config already there cannot be kept; nothing was written`
  when `pns::setup::backup_path` answers `None`. `src/setup.rs:backup_path` answers `None` when
  `crate::system::utc_timestamp` cannot express the second, or when the config path has no file name, or
  when that name is not valid UTF-8. Pinned by
  `src/setup.rs:a_clock_that_cannot_be_read_names_no_backup_at_all`, which passes `u64::MAX`.
- Failure sources: an unreadable clock, an epoch second outside `time_t`, or a `gmtime_r` that returns
  null (`src/system.rs:utc_timestamp`).
- Fail direction: fail-closed and stated as the reason the function exists: "replacing a config whose
  copy cannot be named is the one outcome that loses the file" (`src/setup.rs:backup_path`). Nothing is
  moved and nothing is published; the pending file is still removed.
- Thresholds: `u64::MAX` fails `time_t::try_from` and is the value the test uses. `1_800_000_000` is a
  working value and produces `config.toml.2027-01-15T08-00-00.backup`, pinned exactly by
  `src/setup.rs:the_backup_sits_beside_the_config_stamped_with_the_instant_it_was_moved`.
- Required side effects: none. Nothing was created.
- Forbidden side effects: no unstamped fallback name (`.bak`, say) and no proceeding without a backup.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: the refusal is repeatable while the clock stays unreadable.
- Privacy: the refusal names the config path, never its contents.
- Process ownership and cleanup: `publish_config` still removes the pending file it created.
- Compatibility contract: the stamp is UTC and carries NO colons, because it is a discriminator in a file
  name rather than a clock anybody reads (`src/setup.rs:backup_path`), and the forced-publish test
  asserts the backup path contains no `:`.

### 26. The backup name is claimed before anything moves onto it

Given `--force` twice within the same wall-clock second, or a backup name already taken When
`keep_aside_at` claims the name Then `create_new` fails with `AlreadyExists` and the run refuses with
`<backup> is already claimed by another run this same second; nothing was written`

- Success: `src/main.rs:keep_aside_at` opens the backup name with `create_new(true).write(true)` and
  `.mode(CONFIG_FILE_MODE)` BEFORE the rename. Pinned by
  `src/main.rs:a_same_second_backup_collision_names_the_backup_it_could_not_claim`, which pre-creates the
  collision at a FIXED epoch (which is why `keep_aside_at` takes the moment as an argument at all) and
  asserts the refusal names the pre-claimed path, says `already claimed`, that the config was NOT moved,
  and that the earlier run's backup still holds its own text.
- Failure sources: a same-second collision, and every other claim failure (a missing directory, a
  permission refusal), which carry their own reason: `<backup> could not be claimed: <error>`. Pinned by
  `src/main.rs:a_claim_that_fails_for_another_reason_is_not_blamed_on_a_same_second_run`, which asserts
  the refusal says `could not be claimed` and does NOT say `this same second`.
- Fail direction: refuses rather than renaming over an earlier run's backup, which would replace that
  copy without a word (`src/main.rs:keep_aside_at`).
- Thresholds: the stamp's resolution is ONE SECOND. Two forced runs 1 second apart get distinct names;
  two inside the same second collide. The claim also proves nothing about what the name HOLDS: a run
  killed between the claim and the rename leaves an EMPTY file at that name, so the refusal says only
  that the name is spoken for.
- Required side effects: the claim file, which the rename then replaces.
- Forbidden side effects: a claim whose rename never happened must not survive. Both the nothing-to-move
  path and the rename-failed path call `remove_file(&backup)`, pinned by
  `src/main.rs:a_forced_replacement_with_nothing_to_replace_keeps_nothing_aside` (asserts the directory
  holds nothing but the config) and by
  `src/main.rs:a_directory_at_the_config_path_is_named_rather_than_the_backup_it_could_not_replace`
  (asserts no `.backup` entry is left).
- Timeout and cancellation: no retry with a later stamp. One attempt, one name.
- Idempotency and duplicates: `--force` on a machine with NO config is an ordinary first run and keeps
  nothing aside: the rename fails with `NotFound`, the claim is removed, and the answer is `Ok(None)`
  (`src/main.rs:keep_aside_at`, pinned by
  `src/main.rs:a_forced_replacement_with_nothing_to_replace_keeps_nothing_aside`).
- Privacy: the claim is created at `0o600` so it is never briefly world-readable, even though the rename
  replaces it moments later.
- Process ownership and cleanup: a rename that fails for any reason other than `NotFound` refuses with
  `<path> could not be moved aside to keep it: <error>`, naming `path` rather than `backup`, because the
  backup "was never the problem here: it is a fresh file this call just created". The directory test
  asserts precisely that by checking the refusal does NOT contain `.backup`, which is the only way to
  tell the two apart, since the backup's display string always carries the config path as a prefix.
- Compatibility contract: `keep_aside` reads the clock and delegates to `keep_aside_at`; the split exists
  so a test can name the second rather than racing it.

### 27. What happens to a typed secret, exhaustively

Given the four secret prompts (the moshi token, the hermes key, the hue key, the router API key) When the
walk runs to a successful publish Then each secret reaches exactly three places, and no other

- Success: the three places are the pending file (transient, mode `0o600`), the published config (mode
  `0o600`), and the process's own memory. Pinned by
  `tests/setup.rs:a_secret_typed_into_setup_never_reaches_the_pty_output`, which arms EVERY branch that
  asks for a secret with its own unique value (an earlier version walked only the token and could not
  tell `armed_secret` from `armed` on the other three branches), asserts each value is ABSENT from the
  whole pty transcript, and asserts each value is PRESENT in the published file.
- Failure sources for the guarantee: any of the four prompts being changed from `armed_secret`/
  `ask_hidden` to `armed`/`ask`. The test's four distinct values are what catches that per branch.
- Fail direction: `Hushed::arm` fails closed (behavior 14), so a terminal whose echo cannot be turned off
  refuses the walk rather than asking for a secret in the clear.
- Thresholds: four secret prompts out of fifteen. The other eleven are echoed. That is deliberate but
  worth stating plainly: the hue bridge address, the hue room names, the router URL, the phone's hostname
  on that router, and the Focus mode names are all typed with echo ON, so they land in the terminal's
  scrollback and in any persisted pane history behind it. Only credentials are hidden.
- Required side effects: the config file and, under `--force`, the backup.
- Forbidden side effects, enumerated:
  - The TERMINAL. No secret is echoed (pinned per branch above), and the two-space "nothing given" line
    names the FEATURE, never the answer (`src/main.rs:nothing_given`). The unanswerable-backend line says
    "that router", not the brand typed (`src/main.rs:walk`).
  - A DIAGNOSTIC. `setup_mode` prints exactly two success lines,
    `pns setup: kept the old config at <backup>` and `pns setup: wrote <path>`, both paths only. The
    composed text is never printed (behavior 19).
  - An ERROR MESSAGE. The one refusal that could carry file content is the parse check, and its detail is
    sanitized: `Malformed` is rebuilt from the parser's message and a line number rather than the
    offending line (`src/config.rs:parse_config`, pinned by
    `src/config.rs:a_malformed_line_is_reported_without_echoing_its_value`), and plugin tables are judged
    by key NAME alone (`src/config.rs:admits_flat`), so no plugin value is ever interpolated. Every
    publication refusal names paths and `io::Error`s only.
  - A BACKUP of this run's own secrets. The backup holds the PREVIOUS config, which is a different set of
    secrets, at `0o600` (behavior 24). This run's own secrets never reach a backup.
  - ARGV and the ENVIRONMENT. Every secret is typed at a prompt. `setup_mode` reads only `HOME` and argv,
    and argv is at most `--force`.
  - STATE FILES, the decision ring, the journal, the activity ring and any log. `setup_mode` writes
    nothing but the directory, the pending file, the config and the backup; `tests/dispatch.rs` asserts a
    refusal spawns nothing.
- Timeout and cancellation: a held signal delivered on drop (behavior 15) ends the process with the
  secret still in memory and the terminal already restored.
- Idempotency and duplicates: re-running the walk asks for the secrets again. Nothing caches them.
- Privacy limits, stated rather than papered over:
  - MOSH. The transport under a Moshi-connected phone predicts keystrokes locally and can draw them on
    that client transiently, ahead of the terminal's own echo state. Nothing in `ask_hidden` controls
    that, and the code says so.
  - MEMORY. `Answers` holds each secret as a plain `String`, `Answers::values()` CLONES each one into a
    `toml::Value`, and `compose_config` renders a third copy into the composed `String`. None is
    zeroized; the crate has no `zeroize` dependency. All of them live until the process exits.
  - `Answers` derives `Debug`, which would print every secret verbatim. Nothing formats it that way
    today.
  - The PENDING FILE. Between its creation and its removal there is a second complete copy of the config,
    secrets included, in the config directory. A run killed in that window leaves it there indefinitely
    (behavior 23).
  - NOT ESTABLISHED: whether the terminal driver's own input queue can retain a hidden answer after the
    read. `TCSAFLUSH` discards what is QUEUED at the moment the attributes change, on both the arm and
    the restore (`src/main.rs:Hushed`), but nothing in the crate scrubs the line already consumed.
- Process ownership and cleanup: the terminal's echo state is the only borrowed resource, and it is given
  back on every path `Drop` can reach (behavior 16).
- Compatibility contract: the four secret prompts are the four `armed_secret`/`ask_hidden` call sites in
  `src/main.rs:walk`. Adding a fifth credential means adding a fifth call site AND a fifth unique value
  in `tests/setup.rs:a_secret_typed_into_setup_never_reaches_the_pty_output`, or the new branch is
  unpinned.

### 28. Running setup twice is refused, and --force is the only way to replace a config

Given a machine that already has a config When `pns setup` is run again Then the second run refuses at
behavior 4 unless `--force` is passed, and with `--force` it walks again and replaces the file, keeping
the old one beside it

- Success: `src/main.rs:setup_mode`'s `Ok(_) if !force` arm refuses; the `Ok(_)` arm falls through to the
  walk. Pinned by
  `tests/dispatch.rs:the_first_run_walk_refuses_a_config_that_is_already_there_and_leaves_it_alone` for
  the refusal and by `src/main.rs:a_forced_replacement_keeps_the_old_config_before_it_writes_the_new_one`
  for the replacement.
- Failure sources: everything in behaviors 24 to 26.
- Fail direction: the refusal is the safe direction. `tests/dispatch.rs` states the stake: "Replacing one
  full of plugin secrets on a bare `pns setup` is unrecoverable, and the refusal names the flag that does
  it deliberately."
- Thresholds: `--force` is all-or-nothing. There is no per-feature merge, no "keep my existing token",
  and no reading of the existing config: the walk asks every question again from the shipped posture.
- Required side effects under `--force`: one backup plus one new config, and two stdout lines naming
  both.
- Forbidden side effects: `--force` does NOT buy past the unresolvable-path refusal (behavior 5) or the
  unstat-able-directory refusal (behavior 6). Both tests run both argument shapes for exactly that
  reason.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: NOT idempotent in the file-system sense. Each forced run leaves one more
  `.backup` file in the config directory, forever.
- Privacy: each forced run leaves another full copy of a secret-bearing config on disk at `0o600`. There
  is no pruning.
- Process ownership and cleanup: the backups are the operator's to remove.
- Compatibility contract: the second forced run's backup cannot collide with the first unless they land
  in the same second (behavior 26).

### 29. The operator-facing report names both paths, and only after the work is done

Given a successful publish When `setup_mode` returns Then stdout carries
`pns setup: kept the old config at <backup>` (only when something was kept) followed by
`pns setup: wrote <path>`, and the exit code is 0

- Success: `src/main.rs:setup_mode`'s `Ok(backup)` arm. Both lines are printed AFTER `publish_config`
  answered, so neither can claim work that did not happen.
- Failure sources: none at this point; the work is done.
- Fail direction: on a publication refusal the mode prints `pns setup: <refusal>` to stderr and exits 1.
  Every pre-publication refusal exits 2. So an exit code of 1 means specifically "the walk completed and
  the file could not be published", and 2 means "the run was refused before that".
- Thresholds: three exit codes, 0, 1 and 2, and no others.
- Required side effects: the two lines, in that order.
- Forbidden side effects: no line reports the composed contents and no line reports an answer.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: the report is derived from `publish_config`'s return value, not from what
  the run intended.
- Privacy: paths only.
- Process ownership and cleanup: `setup_mode`'s return value is passed to `std::process::exit` by `main`,
  so no destructor beyond the ones already run is guaranteed after that point. Every guard has already
  dropped by then.
- Compatibility contract: exit 2 for a refusal is explicitly permitted by the always-exit-0 contract's
  own carve-out, because that contract covers the hook and notification paths where a non-zero exit fails
  the turn being reported on, and setup is hand typed and is never a hook (`src/main.rs:setup_mode`).

### 30. The shipped posture is what a walk that armed nothing writes

Given every question declined When the file is composed and published Then the machine gets the macOS
banner and the phone card, both enabled, and nothing else armed

- Success: `src/setup.rs:a_walk_that_armed_nothing_still_writes_the_core` parses the composed text and
  asserts `plugins["macos-banner"].enabled`, `plugins["mobile"].enabled`,
  `mobile.settings["type"] == "moshi"`, that `hermes`, `hue` and `router` are absent, that `lights` is
  `None`, that `focus_silence` is empty and that `nag_after_secs` is 0. The reason every default is
  written OUT rather than left implicit is that a loaded config is authoritative and an absent `enabled`
  reads FALSE, so a wizard that left the core implicit would hand a fresh machine a file that turns the
  banner and the card off (`src/setup.rs:compose_config`).
- Failure sources: none in the wizard. The layout and the defaults are `config_text`'s;
  `docs/specs/configuration.md` owns them.
- Fail direction: toward a WORKING core rather than a silent one. This is the whole reason the wizard
  writes the core defaults it never asked about.
- Thresholds: the values written unprompted are asserted against the CODE's own defaults rather than
  against literals held beside them, so a default moved in `config` and left standing in the wizard would
  fail rather than ship yesterday's number
  (`src/setup.rs:the_values_it_writes_unprompted_are_the_ones_the_code_defaults_to`, which checks
  `Recap::default()`, `daemon_enabled`, `mobile_watch_card` and `DEFAULT_SUBMIT_DEADLINE_SECS`).
- Required side effects: one config file.
- Forbidden side effects: a declined table must be COMMENTED OUT, never written with empty values:
  `silence = []` and `rooms = []` load to the same nothing an absent table does and READ as a feature set
  up (`src/setup.rs:a_walk_that_armed_nothing_still_writes_the_core`, which scans for five headings
  standing uncommented at the head of a line).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: the same declined walk composes the same bytes every time.
- Privacy: a fully declined walk writes a config with no secret in it at all, still at `0o600`.
- Process ownership and cleanup: nothing held.
- Compatibility contract: the wizard never asks about `[lights]`, `[daemon]`, `[recap]` or the banner.
  The lamp-map starter is always present and wholly commented out, whether or not hue is armed
  (`src/setup.rs:the_lamp_map_starter_is_always_offered_and_is_wholly_commented_out`). See
  `docs/specs/configuration.md`.

## Vocabulary

| Term             | Defining symbol                                                                                                                     |
| ---------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| the walk         | `src/main.rs:walk`, the questions asked one at a time in the order the file is written                                              |
| answers          | `src/setup.rs:Answers`, what one walk came back with; every credential is a plain string and empty means declined                   |
| armed            | `src/setup.rs:hue_is_armed`, `src/setup.rs:router_is_armed`, and `src/main.rs:armed`: a feature whose every credential is non-empty |
| declined         | an empty credential, which composes the feature's table commented out (`src/setup.rs:Answers::values`)                              |
| composed         | `src/setup.rs:compose_config`, the whole config text as a pure function of the answers                                              |
| published        | `src/main.rs:publish_config`, the pending file hard-linked to the config path                                                       |
| the pending file | `src/main.rs:pending_name`, `config.toml.new.<pid>.<nanos>`, a second name for the live config between the link and the unlink      |
| kept aside       | `src/main.rs:keep_aside`, the existing config MOVED (never copied) to a stamped backup                                              |
| the backup       | `src/setup.rs:backup_path`, `config.toml.<UTC stamp>.backup`, a sibling of the config                                               |
| hushed           | `src/main.rs:Hushed`, the guard that holds the terminal's echo off and nine signals blocked for one read                            |
| the home probe   | `src/home.rs`, the reading of whether the phone is on the home wifi                                                                 |
| the router       | the backend the home probe reads, named by `src/home.rs:UNIFI_TYPE`                                                                 |
