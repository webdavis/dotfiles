# Configuration

## Scope

Everything the `pns` crate does with `~/.config/pns/config.toml`: where the path comes from, what loading
it can answer, how the text is decoded into a `Config`, every key the schema serves with its default and
its bounds, every shape that is refused and the exact words of each refusal, which keys are
secret-bearing and where a secret can and cannot travel, how plugin identity is decided, and the renderer
(`src/config_text.rs` plus the `pns-config-render` binary) that turns the committed values file into the
shipped chezmoi template. The setup wizard and the publication of a first-run file are a sibling
specification's subject (`docs/specs/setup-and-publication.md`); this document names `src/setup.rs` only
where it consumes something owned here, and defers the walk itself. Everything below is derived from the
crate at `dot_local/share/pns` and from the two committed files it reads at `dot_config/pns/`. Where the
code does not settle a question the line begins `NOT ESTABLISHED:` and names what was looked for. The
operator's real config was never read, and no secret value appears anywhere in this document.

Two vocabulary notes that matter for reading the tables below. `quiet hours` is the config key
`[plugins.hue] quiet_hours` and `quiet window` is the parsed value behind it; `dim window` is the
per-target `dim_window` key. `unread` is one of the five behaviour words. `home probe` and `router` name
the `[plugins.router]` sensor. The `config-change` hook event (`src/main.rs:config_change_detail`) is
about the HARNESS's own settings file and has nothing to do with this file; it is out of scope here.

## Is the config versioned?

`NOT ESTABLISHED:` there is no version key and no schema-version concept anywhere in the config surface.
Evidence, all negative and each checked: `src/config.rs:TABLE_KEYS` declares the top level as
`&["daemon", "focus", "lights", "nag", "plugins", "recap"]` and nothing else, so a `version` key at the
top level would be refused by `parse_config`'s `_` arm; `src/config.rs:Config` has six fields (`plugins`,
`recap`, `focus_silence`, `daemon_enabled`, `nag_after_secs`, `lights`) and none of them is a version;
`src/config_text.rs:LAYOUT` declares sixteen tables and no version key; and a case-insensitive grep for
`version` over `src/config.rs`, `src/config_text.rs`, `dot_config/pns/config-values.toml` and
`dot_config/pns/private_config.toml.tmpl` returns nothing at all. There is therefore no migration
mechanism, no compatibility window and no way for a file to declare which schema it was written against.
What stands in for one is the refusal itself: a table that MOVED (`[home]`, whose settings became
`[plugins.router]`) is refused by name with the six live tables listed, which is a hand-executed
migration prompt rather than a versioned one
(`src/config.rs:a_stale_top_level_home_table_is_refused_by_name_rather_than_ignored`,
`src/config.rs:a_table_the_file_does_not_serve_is_refused_listing_the_tables_it_does`).

## The one reach outside the crate

Three places in the pns test build read files that live OUTSIDE `dot_local/share/pns`. All three are
test-only, so the binary an apply builds out of the deployed crate never asks for them
(`src/config.rs:SHIPPED_TEMPLATE` records this as measured both ways: `cargo build --bin pns` exits 0
because `cfg(test)` is stripped before the macro expands, and `cargo test --no-run` fails with "couldn't
read").

The first, and the one the crate's own comment calls out, is at `src/config.rs:SHIPPED_TEMPLATE`:

```rust
const SHIPPED_TEMPLATE: &str =
    include_str!("../../../../dot_config/pns/private_config.toml.tmpl");
```

The path is `../../../../dot_config/pns/private_config.toml.tmpl`, resolved relative to the file holding
the macro (`dot_local/share/pns/src/config.rs`), so the four `..` segments walk `src` to `pns` to `share`
to `dot_local` to the repository root. Its own doc comment states the cost in these words: "THE COST IS
THAT THE TEST BUILD REACHES FOUR LEVELS OUT OF THE CRATE, into the repo checkout around it. `cargo test`
and `cargo clippy --all-targets` therefore only work from inside this repo," and it names what happens
the day pns moves to its own repository: "this test stops compiling and the template it reads has to
arrive by another road (a copy vendored into the crate, or a path handed in by the build). No mechanism
is built for that day here."

The tests that use `SHIPPED_TEMPLATE`, all five in `src/config.rs`:

| Test                                                                          | What it pins                                                              |
| ----------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| `the_committed_template_is_render_over_the_committed_values_file`             | the template is byte for byte `BANNER + render(CONFIG_VALUES) + FOOTER`   |
| `every_table_the_operator_runs_is_still_live_in_the_shipped_template`         | the exact list of 22 uncommented headings                                 |
| `the_shipped_template_names_the_entry_and_field_of_every_secret`              | the five secret lines, each with the table it fell under                  |
| `the_shipped_config_template_still_parses_through_this_schema`                | the template parses and selects hermes, hue, macos-banner, mobile, router |
| `the_shipped_template_states_the_blocked_backstop_at_its_default_uncommented` | `give_up_after_secs = 57600` is present as a live line                    |

The second reach-out sits four lines below the first and is the same shape:

```rust
const CONFIG_VALUES: &str = include_str!("../../../../dot_config/pns/config-values.toml");
```

used by `the_committed_template_is_render_over_the_committed_values_file` and by
`the_resolved_configuration_over_the_committed_values_file_matches_its_snapshot`. Its doc comment says
"Same four-levels-out caveat as `SHIPPED_TEMPLATE` itself."

The third is at runtime rather than compile time, in the integration suite:
`tests/config_render.rs:the_binary_over_the_committed_values_file_writes_the_committed_template_exactly`
builds `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")` and reads both committed files off
disk. So the brief's "one place" is precisely one `include_str!` of the TEMPLATE, and the honest count of
out-of-crate reads is three. A fourth `include_str!` in the same module,
`include_str!("../tests/fixtures/resolved-config.snapshot")` at `src/config.rs:RESOLVED_CONFIG_SNAPSHOT`,
stays inside the crate and is not part of this.

## The five secret actions and where their shape is pinned

Every secret in the shipped template is written in exactly one form, with NO author quotes around it:

```
{{ (keepassxc "<entry>").<Field> | toToml }}
```

The absence of author quotes is load-bearing and is argued at `src/config_text.rs:secret_action`: "A
RENDERED SECRET IS NOT TOML UNTIL `toToml` RUNS. Go's `quote` (`%q`) would emit escapes TOML does not
define (`\a`, `\v`, `\xNN`) for a secret holding a control byte, breaking the whole deployed file from
that line on; `toToml` emits `\uXXXX` for the same bytes and round-trips every one of them. Author quotes
around the action would only duplicate what `toToml` already writes, so this render's job is to write the
bare action, not to quote it."

The shape is pinned in three independent places.

1. **The writer.** `src/config_text.rs:secret_action` builds it with `push_str` rather than `format!`
   (its own comment: "the target text is thick with literal `{`, `}` and `"` characters"), emitting
   `"{{ (keepassxc \""`, the entry, `"\")."`, the field, `" | toToml }}"`. `<Field>` is whitelisted to
   `src/config_text.rs:SECRET_FIELDS`, which is `["Password", "UserName"]`.

1. **The reader.** `src/config.rs:strip_chezmoi_actions` accepts an action in value position ONLY when it
   matches that grammar, and refuses anything else with `` not a `| toToml` secret action: {action} ``.
   Its comment is explicit about why: "Swapping a quoted placeholder in for ANY action would let a
   template line that dropped `| toToml` keep every template test green while chezmoi splices the raw
   vault bytes in unquoted." Pinned by
   `src/config.rs:the_stub_refuses_a_secret_action_that_forgot_totoml`.

1. **The five lines themselves**, table by table, in
   `src/config.rs:the_shipped_template_names_the_entry_and_field_of_every_secret`. The assertion is an
   exact five-element comparison, so a sixth secret appearing or one of these five going away is the same
   red. Quoted verbatim from the test:

```rust
assert_eq!(
    secrets,
    [
        (
            "plugins.mobile".to_string(),
            r#"token = {{ (keepassxc "Moshi :: Webhook Secret").Password | toToml }}"#
        ),
        (
            "plugins.hermes".to_string(),
            r#"key = {{ (keepassxc "Hermes :: Webhook Secret :: #pns").Password | toToml }}"#
        ),
        (
            "plugins.hue".to_string(),
            r#"bridge = {{ (keepassxc "OpenHue :: API Key (hue-bridge-pro)").UserName | toToml }}"#
        ),
        (
            "plugins.hue".to_string(),
            r#"key = {{ (keepassxc "OpenHue :: API Key (hue-bridge-pro)").Password | toToml }}"#
        ),
        (
            "plugins.router".to_string(),
            r#"api_key = {{ (keepassxc "UniFi :: API Key (dresden-udr)").Password | toToml }}"#
        ),
    ]
);
```

The pin carries each line's TABLE as well as its text, and the test's own comment says why: "a bare line
comparison cannot tell hermes's secret sitting under `[plugins.hue]` from hermes's secret sitting under
`[plugins.hermes]`, since the line text alone never says which heading it fell under (sol-1 finding 1)."

The pin's stated ceiling, from the same doc comment: "THE STUB ONLY READS THE GRAMMAR of a secret
action... It reads neither WHICH entry a line names nor WHICH field it takes off that entry, so pointing
hue's `bridge` at `.Password` or a line at another vault entry leaves every other template test green
while the deployed file quietly carries the wrong credential, and both are one character." The exact
five-line assertion above is what closes that gap for the shipped file; the grammar check alone does not.

## The load outcome model

`src/config.rs:LoadOutcome` has two variants and `src/config.rs:ConfigError` has three, so loading
answers with one of five things.

| Answer                                 | When                                                                    | Fail direction on the delivery path                                                        | Fail direction on the pulse path                                       |
| -------------------------------------- | ----------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------- |
| `Ok(LoadOutcome::Loaded(config))`      | the file read and parsed                                                | the config is authoritative                                                                | the config is authoritative                                            |
| `Ok(LoadOutcome::Missing)`             | `read_to_string` returned `NotFound` AND `symlink_metadata` also failed | the CORE selection (`mobile`, `macos-banner`), silently (`src/registry.rs:select_plugins`) | exit 0, silent (`src/main.rs`, `Ok(LoadOutcome::Missing) => return 0`) |
| `Err(ConfigError::Malformed(detail))`  | the text is not TOML                                                    | the CORE plus one warning line                                                             | `pns: config error ({detail}); no pulse`, exit 0                       |
| `Err(ConfigError::Invalid(detail))`    | well-formed TOML that violates the schema                               | the CORE plus one warning line                                                             | the same, exit 0                                                       |
| `Err(ConfigError::Unreadable(detail))` | present but unreadable, a dangling symlink included                     | the CORE plus one warning line                                                             | the same, exit 0                                                       |

`Missing` is deliberately not an error. The type's own comment: "an unconfigured machine is a state to
report, not a fault to diagnose" (`src/config.rs:LoadOutcome`).

## The key table

The `Default` column is what a config that never states the key resolves to. The `Judged by` column names
the function that decides, which for `[plugins.*]` settings other than `enabled` is a reader downstream
of this layer: `src/config.rs:parse_config` judges a plugin table's key NAMES and nothing about its
values (module comment: "the settings inside a plugin's table are free-form here: this layer proves the
shape, the registry interprets the contents").

### Top level

| Key path                                               | Type  | Default | Bound             | Secret | Out of bounds or malformed                                                                                 | Judged by                  | Tests                                                                                                                                                                                                                                                                                                                             |
| ------------------------------------------------------ | ----- | ------- | ----------------- | ------ | ---------------------------------------------------------------------------------------------------------- | -------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `daemon`, `focus`, `lights`, `nag`, `plugins`, `recap` | table | absent  | closed set of six | no     | `Invalid`: `` unknown top-level key `{key}`; the file serves daemon, focus, lights, nag, plugins, recap `` | `parse_config`             | `an_unknown_top_level_key_is_refused_so_a_typo_cannot_disable_a_channel`, `a_table_the_file_does_not_serve_is_refused_listing_the_tables_it_does`, `a_stale_top_level_home_table_is_refused_by_name_rather_than_ignored`, `a_top_level_key_that_merely_looks_like_recap_is_still_refused_by_name`                                 |
| any of the six written as a scalar                     | table | n/a     | must be a table   | no     | `Invalid`: `` `{name}` is not a table ``                                                                   | each table's own parse arm | `a_non_table_recap_value_is_refused_naming_the_key`, `a_non_table_focus_value_is_refused_naming_the_arm_rather_than_the_key`, `a_non_table_plugins_value_is_refused_naming_the_key`, `the_daemon_table_reads_one_switch_defaults_on_and_refuses_the_rest_by_name`, `a_schedule_that_is_not_a_count_of_seconds_is_refused_by_name` |

### `[recap]`

Absent is ALL ON. `src/config.rs:Recap::default` is written out rather than derived, because a derived
bool is false and that "would take every delivery away from every machine whose config was written before
this table existed, and it would do it silently."

| Key path                         | Type              | Default                                         | Bound                                                                                            | Secret | Out of bounds or malformed                                                                                                                                                                                                                                                                                                                                                       | Judged by                                         | Tests                                                                                                                                                                                                                |
| -------------------------------- | ----------------- | ----------------------------------------------- | ------------------------------------------------------------------------------------------------ | ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `recap.replay_card`              | bool              | `true`                                          | none                                                                                             | no     | `Invalid`: `` `recap` key `replay_card` has type `{type}`, not boolean ``                                                                                                                                                                                                                                                                                                        | `src/config.rs:flag`                              | `a_recap_table_is_read_rather_than_refused_and_each_switch_stands_alone`, `a_config_with_no_recap_table_leaves_every_switch_on`                                                                                      |
| `recap.digest`                   | bool              | `true`                                          | none                                                                                             | no     | same wording, key `digest`                                                                                                                                                                                                                                                                                                                                                       | `src/config.rs:flag`                              | `a_non_boolean_recap_switch_is_refused_naming_the_key`                                                                                                                                                               |
| `recap.digest_as_thread`         | bool              | `true`                                          | none                                                                                             | no     | same wording, key `digest_as_thread`                                                                                                                                                                                                                                                                                                                                             | `src/config.rs:flag`                              | `a_recap_table_is_read_rather_than_refused_and_each_switch_stands_alone`                                                                                                                                             |
| `recap.min_events`               | integer (`usize`) | `8` (`DEFAULT_MIN_EVENTS`)                      | floor 1, NO ceiling below `usize`                                                                | no     | not a count: `` `recap` key `min_events` has type `{type}`, not a count ``; zero: `` `recap` key `min_events` is 0, which is not a threshold; 1 is the floor ``                                                                                                                                                                                                                  | `src/config.rs:threshold`                         | `the_recaps_volume_threshold_is_a_count_the_operator_can_state`, `a_volume_threshold_of_zero_is_refused_by_name_rather_than_read_as_every_event`, `a_volume_threshold_that_is_not_a_count_is_refused_naming_the_key` |
| `recap.summarizer`               | array of strings  | `None` (unset, and unset is a working setting)  | non-empty, first word non-empty                                                                  | no     | not a list: `` `recap` key `summarizer` has type `{type}`, not a list of command words ``; a non-string element: `` `recap` key `summarizer` has a `{type}` in it, not a list of command words ``; `[]`: `` `recap` key `summarizer` is empty, so it names no command to run ``; `[""]`: `` `recap` key `summarizer` starts with an empty word, so it names no command to run `` | `src/config.rs:argv` over `src/config.rs:strings` | `the_summarizer_is_an_argument_list_the_operator_states_word_by_word`, `a_summarizer_that_is_not_a_list_of_words_is_refused_naming_the_key`                                                                          |
| `recap.summarizer_deadline_secs` | integer (`u64`)   | `240` (`DEFAULT_SUMMARIZER_DEADLINE_SECS`)      | 0 to 3600 inclusive; zero IS accepted                                                            | no     | not a count: `` `recap` key `summarizer_deadline_secs` has type `{type}`, not a count of seconds ``; over: `` `recap` key `summarizer_deadline_secs` is {count}, past the 3600-second ceiling ``                                                                                                                                                                                 | `src/config.rs:seconds`                           | `the_summarizers_deadline_is_a_count_of_seconds_with_a_generous_default`                                                                                                                                             |
| `recap.repos`                    | array of strings  | `[]` (unset, no `gh` process is started at all) | non-empty, every entry non-empty                                                                 | no     | not a list, or a non-string element: `... not a list of repository names`; `[]` or an empty entry: `` `recap` key `repos` names no repository to read ``                                                                                                                                                                                                                         | `src/config.rs:repositories`                      | `the_two_external_sources_are_named_by_the_operator_or_not_read_at_all`, `a_repos_value_that_is_not_repository_names_is_refused_naming_the_key`                                                                      |
| `recap.review_notes`             | string            | `None` (unset, the directory is never opened)   | absolute or `~/`; no `*` in the directory; at most one `*` in the file name; file name non-empty | no     | four distinct refusals, quoted in behavior 9                                                                                                                                                                                                                                                                                                                                     | `src/config.rs:note_glob`                         | `a_review_notes_glob_that_names_no_readable_file_is_refused_naming_the_key`                                                                                                                                          |

### `[focus]`

| Key path        | Type             | Default                | Bound                                   | Secret | Out of bounds or malformed                                                                                                                                                                                                      | Judged by                                          | Tests                                                                                                                                                                                                                                                                                                                |
| --------------- | ---------------- | ---------------------- | --------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `focus.silence` | array of strings | `[]` (the feature off) | the LIST may be empty, an ENTRY may not | no     | not a list, or a non-string element: `` `focus` key `silence` has type `{type}`, not a list of Focus mode names ``; an empty entry: `` `focus` key `silence` names a mode that is the empty string, which is no Focus at all `` | `src/config.rs:modes` over `src/config.rs:strings` | `a_focus_table_names_the_modes_that_silence_pns`, `a_silence_list_that_is_not_a_list_is_refused_naming_the_key`, `a_mode_name_that_is_not_a_string_is_refused_naming_the_key`, `a_mode_name_that_is_the_empty_string_is_refused_by_name`, `an_empty_silence_list_is_admitted_because_it_is_the_feature_switched_off` |

There is no `enabled` key here, and the absence is deliberate: "naming no mode and switching the feature
off are the same statement, so a second way to say it is a second thing that can disagree with the first"
(`src/config.rs:parse_focus`). The mode NAME itself is not judged: a name matching no mode is an ordinary
thing to write, and `pns doctor` is where the operator learns whether it matched.

### `[daemon]`

| Key path         | Type | Default                           | Bound | Secret | Out of bounds or malformed                                             | Judged by                    | Tests                                                                        |
| ---------------- | ---- | --------------------------------- | ----- | ------ | ---------------------------------------------------------------------- | ---------------------------- | ---------------------------------------------------------------------------- |
| `daemon.enabled` | bool | `true` (`DEFAULT_DAEMON_ENABLED`) | none  | no     | `Invalid`: `` `daemon` key `enabled` has type `{type}`, not boolean `` | `src/config.rs:parse_daemon` | `the_daemon_table_reads_one_switch_defaults_on_and_refuses_the_rest_by_name` |

Default ON, which is the opposite of `[focus]` and of every plugin. The reason given at
`src/config.rs:Config::daemon_enabled`: this switch delivers nothing by itself, "an idle daemon reads one
empty directory a second," and default OFF "would put every feature that rides the clock behind TWO
switches."

### `[nag]`

| Key path         | Type            | Default                          | Bound                                    | Secret | Out of bounds or malformed                                                                                                                                                                       | Judged by                    | Tests                                                                                                                                                |
| ---------------- | --------------- | -------------------------------- | ---------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| `nag.after_secs` | integer (`u64`) | `0` (`NAG_OFF`, the feature off) | 0 is off; otherwise 30 to 3600 inclusive | no     | not a count: `` `nag` key `after_secs` has type `{type}`, not a count of seconds ``; outside: `` `nag` key `after_secs` is {count}, outside the 30 to 3600 second range; 0 is the feature off `` | `src/config.rs:nag_schedule` | `the_nag_table_reads_one_schedule_defaults_off_and_zero_is_off_rather_than_an_error`, `a_schedule_that_is_not_a_count_of_seconds_is_refused_by_name` |

`MAX_NAG_AFTER_SECS` is defined as `MAX_SUMMARIZER_DEADLINE_SECS`, so the two ceilings are one number by
construction rather than two that agree by accident.

### `[lights]`

The table is an `Option` on `Config`, and absence is not its default. "A machine with no `[lights]` table
keeps the room-based pulse it has always had; a machine with an empty one has asked for the lamps and
named no lamp yet. Those are different states and the doctor says different things about them"
(`src/config.rs:Lights`).

| Key path                            | Type            | Default                                                  | Bound                                                                                                                        | Secret | Out of bounds or malformed                                                                                                                       | Judged by                                                                | Tests                                                                                                                                                            |
| ----------------------------------- | --------------- | -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `lights.refresh_secs`               | integer (`u64`) | `12` (`DEFAULT_REFRESH_SECS`)                            | 10 (`MIN_REFRESH_SECS`) to 30 (`MAX_REFRESH_SECS`)                                                                           | no     | `` `lights` key `refresh_secs` is {count}, outside the 10 to 30 range ``; wrong type: `` ... has type `{type}`, not a count between 10 and 30 `` | `src/config.rs:bounded`                                                  | `every_lights_number_is_bounded_on_both_sides_and_refused_by_name_outside_them`, `a_lights_value_of_the_wrong_type_is_refused_by_name_and_by_type`               |
| `lights.done.duration_ms`           | integer         | `4000`                                                   | 200 (`MIN_FADE_MS`) to 5000 (`MAX_FADE_MS`)                                                                                  | no     | `bounded` refusal naming `lights.done`                                                                                                           | `src/config.rs:parse_pulse`                                              | `every_lights_number_is_bounded_on_both_sides_and_refused_by_name_outside_them`                                                                                  |
| `lights.done.brightness`            | integer (`u8`)  | `100`                                                    | 1 (`MIN_BRIGHTNESS`) to 100 (`MAX_BRIGHTNESS`)                                                                               | no     | `bounded` refusal; zero is refused rather than read as off                                                                                       | `src/config.rs:percent`                                                  | same                                                                                                                                                             |
| `lights.failed.duration_ms`         | integer         | `4000`                                                   | 200 to 5000                                                                                                                  | no     | as `lights.done`                                                                                                                                 | `parse_pulse`                                                            | same                                                                                                                                                             |
| `lights.failed.brightness`          | integer         | `100`                                                    | 1 to 100                                                                                                                     | no     | as `lights.done`                                                                                                                                 | `percent`                                                                | same                                                                                                                                                             |
| `lights.blocked.duration_ms`        | integer         | `2000`                                                   | 200 to 5000                                                                                                                  | no     | `bounded` refusal naming `lights.blocked`                                                                                                        | `src/config.rs:breath_key`                                               | `no_lights_table_is_none_and_an_empty_one_is_every_locked_default`                                                                                               |
| `lights.blocked.high`               | integer         | `100`                                                    | 1 to 100, and `low <= high`                                                                                                  | no     | `bounded`, plus `ends_agree`                                                                                                                     | `breath_key`, `src/config.rs:ends_agree`                                 | `a_breath_whose_low_is_above_its_high_is_refused_rather_than_rendered_upside_down`                                                                               |
| `lights.blocked.low`                | integer         | `30`                                                     | 1 to 100, and `low <= high`                                                                                                  | no     | same                                                                                                                                             | same                                                                     | same                                                                                                                                                             |
| `lights.blocked.give_up_after_secs` | integer         | `57600` (`DEFAULT_BLOCKED_GIVE_UP_AFTER_SECS`, 16 hours) | 60 (`MIN_LEASE_TIMEOUT_SECS`) to 604800 (`MAX_GIVE_UP_AFTER_SECS`, a week), AND at least `nag.after_secs` when the nag is on | no     | `bounded` refusal; the cross-table one is quoted in behavior 19                                                                                  | `src/config.rs:parse_blocked`, `src/config.rs:backstop_outlasts_the_nag` | `the_blocked_backstop_reads_the_configured_number_rather_than_a_hardcoded_default`, `a_backstop_that_gives_up_before_the_nag_nudges_is_refused_naming_both_keys` |
| `lights.unread.duration_ms`         | integer         | `4000`                                                   | 200 to 5000                                                                                                                  | no     | `bounded` naming `lights.unread`                                                                                                                 | `breath_key`                                                             | `a_behaviour_table_moves_the_keys_it_states_and_leaves_the_rest_at_their_locked_values`                                                                          |
| `lights.unread.high`                | integer         | `60`                                                     | 1 to 100, `low <= high`                                                                                                      | no     | `bounded`, `ends_agree`                                                                                                                          | `breath_key`, `ends_agree`                                               | `a_breath_whose_low_is_above_its_high_is_refused_rather_than_rendered_upside_down`                                                                               |
| `lights.unread.low`                 | integer         | `10`                                                     | 1 to 100, `low <= high`                                                                                                      | no     | same                                                                                                                                             | same                                                                     | same                                                                                                                                                             |
| `lights.unread.after_secs`          | integer         | `300` (`DEFAULT_UNREAD_AFTER_SECS`)                      | 0 to 86400 (`MAX_THRESHOLD_SECS`); zero means "at once"                                                                      | no     | `bounded` refusal naming `lights.unread`                                                                                                         | `src/config.rs:parse_unread`                                             | `every_lights_number_is_bounded_on_both_sides_and_refused_by_name_outside_them`                                                                                  |
| `lights.loop.duration_ms`           | integer         | `4000`                                                   | 200 to 5000                                                                                                                  | no     | `bounded` naming `lights.loop`                                                                                                                   | `breath_key`                                                             | same                                                                                                                                                             |
| `lights.loop.high`                  | integer         | `60`                                                     | 1 to 100, `low <= high`                                                                                                      | no     | `bounded`, `ends_agree`                                                                                                                          | `breath_key`, `ends_agree`                                               | same                                                                                                                                                             |
| `lights.loop.low`                   | integer         | `10`                                                     | 1 to 100, `low <= high`                                                                                                      | no     | same                                                                                                                                             | same                                                                     | same                                                                                                                                                             |
| `lights.loop.threshold_secs`        | integer         | `300` (`DEFAULT_LOOP_THRESHOLD_SECS`)                    | 1 (`MIN_THRESHOLD_SECS`) to 86400                                                                                            | no     | `bounded` naming `lights.loop`                                                                                                                   | `src/config.rs:parse_looping`                                            | same                                                                                                                                                             |
| `lights.loop.lease_timeout_secs`    | integer         | `3900` (`DEFAULT_LEASE_TIMEOUT_SECS`, 65 minutes)        | 60 to 86400                                                                                                                  | no     | `bounded` naming `lights.loop`                                                                                                                   | `parse_looping`                                                          | same                                                                                                                                                             |
| `lights.dim.duration_ms`            | integer         | `3000`                                                   | 200 to 5000                                                                                                                  | no     | `bounded` naming `lights.dim`                                                                                                                    | `src/config.rs:parse_breath`                                             | `no_lights_table_is_none_and_an_empty_one_is_every_locked_default`                                                                                               |
| `lights.dim.high`                   | integer         | `7`                                                      | 1 to 100, `low <= high`                                                                                                      | no     | `bounded`, `ends_agree`                                                                                                                          | `parse_breath`, `ends_agree`                                             | `a_breath_whose_low_is_above_its_high_is_refused_rather_than_rendered_upside_down`                                                                               |
| `lights.dim.low`                    | integer         | `1`                                                      | 1 to 100, `low <= high`                                                                                                      | no     | same                                                                                                                                             | same                                                                     | same                                                                                                                                                             |

### `[lights.lamp."<name>"]`, `[lights.room."<name>"]`, `[lights.zone."<name>"]`

One roster row (`src/config.rs:TARGET_KEYS`, the string `lights.<level>`) serves all three levels,
because "a lamp, a room and a zone answer the same questions and differ only in how specific they are."
The refusal names the PATH THE OPERATOR WROTE, not the roster row.

| Key path         | Type                     | Default                                                     | Bound                                                                                          | Secret | Out of bounds or malformed                                                                                                                      | Judged by                     | Tests                                                                                                                                                                                                                           |
| ---------------- | ------------------------ | ----------------------------------------------------------- | ---------------------------------------------------------------------------------------------- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `shows`          | array of behaviour words | `None` (said nothing, which is distinct from an empty list) | the closed set `done`, `failed`, `blocked`, `unread`, `loop` (`src/config.rs:BEHAVIOUR_WORDS`) | no     | `` `{path}` key `shows` names `{word}`, which is no behaviour; the lamps say done, failed, blocked, unread, loop ``                             | `src/config.rs:behaviours`    | `a_declaration_at_any_of_the_three_levels_reads_the_same_three_keys`, `a_declaration_that_states_nothing_states_nothing_rather_than_defaulting`, `a_behaviour_word_the_lamps_do_not_speak_is_refused_with_the_closed_set_named` |
| `dim_window`     | string                   | `None`                                                      | not parsed here (the layer reads a file; the window's own grammar is `src/channels/hue.rs`'s)  | no     | `` `{path}` key `dim_window` has type `{type}`, not a string ``                                                                                 | `src/config.rs:text`          | `a_declaration_at_any_of_the_three_levels_reads_the_same_three_keys`                                                                                                                                                            |
| `dim_behaviours` | array of behaviour words | `[]`                                                        | the same closed set, AND `dim_window` must be stated                                           | no     | the behaviour refusal above, or `` `{path}` states `dim_behaviours` with no `dim_window` for them to run in, so nothing would ever read them `` | `src/config.rs:parse_targets` | `dim_behaviours_with_no_window_to_run_them_in_is_refused_rather_than_read_and_dropped`                                                                                                                                          |

A target name is NOT judged against the bridge here: "only the bridge's own listings can say which lamps,
rooms and zones exist" (`src/config.rs:parse_targets`).

### `[plugins.<name>]`

`enabled` is removed from the settings table by this layer rather than read through it, so a plugin never
sees its own selection flag (`src/config.rs:parse_config`, pinned by
`a_plugin_table_with_enabled_true_is_selected_and_keeps_its_settings`). Every other key is checked for
NAME against the roster and passed through untouched; its VALUE is judged by the reader named in the last
column, which lives outside this layer.

| Key path                              | Type             | Default                              | Bound                                                   | Secret                                                               | Out of bounds or malformed                                                                                                                                                 | Judged by                                                               | Tests                                                                                                                                               |
| ------------------------------------- | ---------------- | ------------------------------------ | ------------------------------------------------------- | -------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| `plugins.<any>.enabled`               | bool             | `false` (selection is explicit)      | none                                                    | no                                                                   | `` plugin `{name}` has a non-boolean `enabled` ``                                                                                                                          | `parse_config`                                                          | `an_absent_enabled_flag_reads_disabled_because_selection_is_explicit`, `a_non_boolean_enabled_flag_is_refused_naming_the_plugin`                    |
| `plugins.hermes.key`                  | string           | unset                                | not judged here                                         | YES                                                                  | name-checked only at load                                                                                                                                                  | `src/channels/` at delivery                                             | `every_key_a_shipped_plugin_table_serves_is_still_admitted`                                                                                         |
| `plugins.hue.bridge`                  | string           | unset                                | not judged here                                         | YES (the repo's values file takes it off a vault entry's `UserName`) | name-checked only at load                                                                                                                                                  | `src/channels/hue.rs`                                                   | same                                                                                                                                                |
| `plugins.hue.key`                     | string           | unset                                | not judged here                                         | YES                                                                  | name-checked only at load                                                                                                                                                  | `src/channels/hue.rs`                                                   | same                                                                                                                                                |
| `plugins.hue.rooms`                   | array of strings | unset                                | not judged here                                         | no                                                                   | name-checked only at load                                                                                                                                                  | `src/channels/hue.rs`                                                   | same                                                                                                                                                |
| `plugins.hue.quiet_hours`             | string           | unset                                | not judged here                                         | no                                                                   | name-checked only at load; an unparsable window is handled at read time                                                                                                    | `src/channels/hue.rs:quiet_window`                                      | `tests/dispatch.rs:a_malformed_quiet_hours_refuses_once_and_only_where_a_pulse_was_due`                                                             |
| `plugins.macos-banner.enabled`        | bool             | `false`                              | none                                                    | no                                                                   | as any `enabled`                                                                                                                                                           | `parse_config`                                                          | `every_key_a_shipped_plugin_table_serves_is_still_admitted`                                                                                         |
| `plugins.mobile.type`                 | string           | unset                                | must equal `"moshi"` when the table is armed            | no                                                                   | `no type in [plugins.mobile]; the only type is "moshi"` or `[plugins.mobile] has type "{named}", which no compiled-in backend answers; the only type is "moshi"`           | `src/channels/moshi.rs:mobile_backend` via `src/config.rs:armed_mobile` | `a_mobile_table_naming_no_backend_contributes_no_settings_at_all`, `type_is_the_word_that_selects_a_backend_and_the_old_brand_is_refused`           |
| `plugins.mobile.token`                | string           | unset (not set up, never an error)   | non-empty to count                                      | YES                                                                  | `src/channels/moshi.rs:moshi_secret` answers `None` for every failure shape                                                                                                | `src/channels/moshi.rs:moshi_secret`                                    | `every_key_a_shipped_plugin_table_serves_is_still_admitted`                                                                                         |
| `plugins.mobile.mobile_watch_card`    | bool             | `false`                              | none                                                    | no                                                                   | loud, then off: `pns: config error ([plugins.mobile] mobile_watch_card is {type}, not a boolean); the mobile watching card stays off`                                      | `src/main.rs:watch_card`                                                | `tests/dispatch.rs:a_watch_card_toggle_of_the_wrong_type_is_refused_out_loud`                                                                       |
| `plugins.mobile.submit_deadline_secs` | integer (`u64`)  | `5` (`DEFAULT_SUBMIT_DEADLINE_SECS`) | 1 to 3600 (`MAX_SUBMIT_DEADLINE_SECS`); zero is REFUSED | no                                                                   | three refusals, quoted in behavior 20                                                                                                                                      | `src/config.rs:submit_deadline`                                         | `the_mobile_submission_deadline_is_a_count_of_seconds_defaulted_to_five`, `a_submission_deadline_that_is_not_a_count_of_seconds_is_refused_by_name` |
| `plugins.router.type`                 | string           | unset                                | must equal `"unifi"`                                    | no                                                                   | `home: no type in [plugins.router] (the only type is "unifi")` / `home: [plugins.router] has type "{x}", which no compiled-in backend answers (the only type is "unifi")`  | `src/home.rs:setup_report`                                              | `tests/dispatch.rs:every_way_the_home_probe_is_not_set_up_says_which_one_it_is`                                                                     |
| `plugins.router.router_url`           | string           | unset                                | non-empty string                                        | no                                                                   | `home: the [plugins.router] table is present but router_url is missing, empty, or not a string`                                                                            | `src/home.rs`                                                           | same                                                                                                                                                |
| `plugins.router.device_hostname`      | string           | unset                                | at least one of the three device keys                   | no                                                                   | `home: no device to look for in [plugins.router] (set at least one of device_mac, device_hostname, device_ipv4)`                                                           | `src/home.rs`                                                           | same                                                                                                                                                |
| `plugins.router.device_mac`           | string           | unset                                | six hex pairs under one separator                       | no                                                                   | `home: device_mac = "{x}" in [plugins.router] is not a MAC address (six hex pairs under one separator, e.g. "2e:11:ab:6d:b0:4f")`                                          | `src/home.rs`                                                           | same                                                                                                                                                |
| `plugins.router.device_ipv4`          | string           | unset                                | a dotted quad                                           | no                                                                   | `home: device_ipv4 = "{x}" in [plugins.router] is not an IPv4 address (a dotted quad, e.g. "192.168.1.169")`                                                               | `src/home.rs`                                                           | same                                                                                                                                                |
| `plugins.router.api_key`              | string           | unset                                | non-empty                                               | YES                                                                  | `home: no api_key in the [plugins.router] table (the probe is not set up)`                                                                                                 | `src/home.rs`                                                           | same                                                                                                                                                |
| `plugins.router.stale_alert_channel`  | string           | unset (the default route)            | a usable route name                                     | no                                                                   | loud, then the default route: `pns: config error (stale_alert_channel = "{x}" in [plugins.router] is not a usable route name); the stale alert posts to the default route` | `src/home.rs`                                                           | `tests/dispatch.rs:an_unusable_stale_alert_route_complains_and_still_delivers_the_alert`                                                            |
| `plugins.<unregistered>.<anything>`   | any              | n/a                                  | NOT judged at this layer                                | no                                                                   | the NAME is refused one layer on: `` unknown plugin `{name}` ``                                                                                                            | `src/registry.rs:Registry::enabled`                                     | `an_unregistered_plugin_tables_settings_stay_free_form_because_selection_is_by_name`                                                                |

The five secret-bearing key paths are declared once more, as data, at
`src/bin/pns-config-render.rs:SECRET_BEARING_KEYS`:
`["plugins.mobile.token", "plugins.hermes.key", "plugins.hue.bridge", "plugins.hue.key", "plugins.router.api_key"]`.
That list is what makes "secret" an enforced classification rather than a convention: in the committed
values file each of those five must hold a keepassxc marker table, never a literal.

## The rejection table

Every refusal in the configuration surface, its exact wording, and which way it fails. "Fail closed"
means the thing being configured does not happen; "fail open" means it happens at a default. `{...}`
marks an interpolation.

### Decoding (`src/config.rs`), all returning `ConfigError`

| What is rejected                                                  | Exact wording                                                                                                                                                                                                                                                   | Variant                                                    | Fail direction                                                                         |
| ----------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| text that is not TOML, with a locatable span                      | `{cause} at line {line}`                                                                                                                                                                                                                                        | `Malformed`                                                | closed on the pulse path and on the lights tick; open to the CORE on the delivery path |
| text that is not TOML, no span                                    | `{cause}`                                                                                                                                                                                                                                                       | `Malformed`                                                | same                                                                                   |
| a top-level key outside the six                                   | `` unknown top-level key `{key}`; the file serves daemon, focus, lights, nag, plugins, recap ``                                                                                                                                                                 | `Invalid`                                                  | same                                                                                   |
| `recap`, `focus`, `daemon`, `nag` or `lights` written as a scalar | `` `{name}` is not a table ``                                                                                                                                                                                                                                   | `Invalid`                                                  | same                                                                                   |
| `plugins` written as a scalar                                     | `` `plugins` is not a table ``                                                                                                                                                                                                                                  | `Invalid`                                                  | same                                                                                   |
| a plugin entry that is not a table                                | `` plugin `{name}` is not a table ``                                                                                                                                                                                                                            | `Invalid`                                                  | same                                                                                   |
| a non-boolean `enabled` under a plugin                            | `` plugin `{name}` has a non-boolean `enabled` ``                                                                                                                                                                                                               | `Invalid`                                                  | same                                                                                   |
| a key a table does not serve (every table, top level included)    | `` unknown `{table}` key `{key}`; the table serves {comma-joined roster row} ``                                                                                                                                                                                 | `Invalid`                                                  | same                                                                                   |
| a non-boolean `[recap]` switch                                    | `` `recap` key `{key}` has type `{type}`, not boolean ``                                                                                                                                                                                                        | `Invalid`                                                  | same                                                                                   |
| `min_events` not a count                                          | `` `recap` key `min_events` has type `{type}`, not a count ``                                                                                                                                                                                                   | `Invalid`                                                  | same                                                                                   |
| `min_events` zero                                                 | `` `recap` key `min_events` is 0, which is not a threshold; 1 is the floor ``                                                                                                                                                                                   | `Invalid`                                                  | same                                                                                   |
| `summarizer_deadline_secs` not a count                            | `` `recap` key `summarizer_deadline_secs` has type `{type}`, not a count of seconds ``                                                                                                                                                                          | `Invalid`                                                  | same                                                                                   |
| `summarizer_deadline_secs` over the ceiling                       | `` `recap` key `summarizer_deadline_secs` is {count}, past the 3600-second ceiling ``                                                                                                                                                                           | `Invalid`                                                  | same                                                                                   |
| `summarizer` empty                                                | `` `recap` key `summarizer` is empty, so it names no command to run ``                                                                                                                                                                                          | `Invalid`                                                  | same                                                                                   |
| `summarizer` whose first word is empty                            | `` `recap` key `summarizer` starts with an empty word, so it names no command to run ``                                                                                                                                                                         | `Invalid`                                                  | same                                                                                   |
| `repos` empty, or holding an empty entry                          | `` `recap` key `repos` names no repository to read ``                                                                                                                                                                                                           | `Invalid`                                                  | same                                                                                   |
| a list key that is not a list, or holds a non-string              | `` `{table}` key `{key}` has type `{type}`, not {noun} `` / `` `{table}` key `{key}` has a `{type}` in it, not {noun} `` where `{noun}` is `a list of command words`, `a list of repository names`, `a list of Focus mode names` or `a list of behaviour names` | `Invalid`                                                  | same                                                                                   |
| `review_notes` not a string                                       | `` `recap` key `review_notes` has type `{type}`, not a path with a file name in it ``                                                                                                                                                                           | `Invalid`                                                  | same                                                                                   |
| `review_notes` naming no file                                     | `` `recap` key `review_notes` names no file to read ``                                                                                                                                                                                                          | `Invalid`                                                  | same                                                                                   |
| `review_notes` relative                                           | `` `recap` key `review_notes` is `{pattern}`, which is not an absolute path or a `~/` one ``                                                                                                                                                                    | `Invalid`                                                  | same                                                                                   |
| `review_notes` with a `*` in a directory                          | `` `recap` key `review_notes` is `{pattern}`, and only its file name may hold a `*` ``                                                                                                                                                                          | `Invalid`                                                  | same                                                                                   |
| `review_notes` with two `*` in the file name                      | `` `recap` key `review_notes` is `{pattern}`, and its file name may hold only one `*` ``                                                                                                                                                                        | `Invalid`                                                  | same                                                                                   |
| a `silence` entry that is the empty string                        | `` `focus` key `silence` names a mode that is the empty string, which is no Focus at all ``                                                                                                                                                                     | `Invalid`                                                  | same                                                                                   |
| a non-boolean `[daemon] enabled`                                  | `` `daemon` key `enabled` has type `{type}`, not boolean ``                                                                                                                                                                                                     | `Invalid`                                                  | open: the daemon carries on enabled (`src/main.rs:daemon_enabled`)                     |
| `after_secs` not a count                                          | `` `nag` key `after_secs` has type `{type}`, not a count of seconds ``                                                                                                                                                                                          | `Invalid`                                                  | closed on the pulse path, open to the CORE on the delivery path                        |
| `after_secs` outside its range                                    | `` `nag` key `after_secs` is {count}, outside the 30 to 3600 second range; 0 is the feature off ``                                                                                                                                                              | `Invalid`                                                  | same                                                                                   |
| a `[lights]` scalar of the wrong type                             | `` `{table}` key `{key}` has type `{type}`, not a count between {low} and {high} ``                                                                                                                                                                             | `Invalid`                                                  | closed: the lights tick returns 0 and arms nothing                                     |
| a `[lights]` scalar outside its bounds                            | `` `{table}` key `{key}` is {count}, outside the {low} to {high} range ``                                                                                                                                                                                       | `Invalid`                                                  | same                                                                                   |
| a behaviour table that is not a table                             | `` `{table}` has type `{type}`, not a table of settings ``                                                                                                                                                                                                      | `Invalid`                                                  | same                                                                                   |
| a declaration level that is not a table                           | `` `lights` key `{level}` has type `{type}`, not a table of {level} names ``                                                                                                                                                                                    | `Invalid`                                                  | same                                                                                   |
| one declaration that is not a table                               | `` `lights.{level}.{name}` has type `{type}`, not a table of settings ``                                                                                                                                                                                        | `Invalid`                                                  | same                                                                                   |
| a breath whose ends are reversed                                  | `` `{table}` has low {low} above high {high}, so a fade to `high` would move the lamp down and one to `low` would move it up ``                                                                                                                                 | `Invalid`                                                  | same                                                                                   |
| a behaviour word outside the closed set                           | `` `{path}` key `{key}` names `{word}`, which is no behaviour; the lamps say done, failed, blocked, unread, loop ``                                                                                                                                             | `Invalid`                                                  | same                                                                                   |
| `dim_window` that is not a string                                 | `` `{path}` key `dim_window` has type `{type}`, not a string ``                                                                                                                                                                                                 | `Invalid`                                                  | same                                                                                   |
| `dim_behaviours` with no `dim_window`                             | `` `{path}` states `dim_behaviours` with no `dim_window` for them to run in, so nothing would ever read them ``                                                                                                                                                 | `Invalid`                                                  | same                                                                                   |
| a backstop shorter than the nag                                   | `` `lights.blocked` key `give_up_after_secs` is {give_up}, below `nag` key `after_secs` {after}, so the lamp would be given up on before the nudge it belongs to has ever fired ``                                                                              | `Invalid`                                                  | same                                                                                   |
| a present but unreadable path                                     | `{path}: {io error}`                                                                                                                                                                                                                                            | `Unreadable`                                               | closed on the pulse path, open to the CORE on the delivery path                        |
| `submit_deadline_secs` not a count                                | `` `mobile` key `submit_deadline_secs` has type `{type}`, not a count of seconds ``                                                                                                                                                                             | `Invalid` (returned by `submit_deadline`, not by the load) | open: the caller keeps the 5-second default and says so                                |
| `submit_deadline_secs` zero                                       | `` `mobile` key `submit_deadline_secs` is 0, which is the bound switched off by accident: a deadline that expires before the daemon can answer costs the phone card on every approval ``                                                                        | `Invalid`                                                  | same                                                                                   |
| `submit_deadline_secs` over the ceiling                           | `` `mobile` key `submit_deadline_secs` is {count}, past the 3600-second ceiling ``                                                                                                                                                                              | `Invalid`                                                  | same                                                                                   |

### Rendering (`src/config_text.rs` and `src/config.rs:strip_chezmoi_actions`), all returning `Result<_, String>`

| What is rejected                                             | Exact wording                                                                               | Fail direction                                        |
| ------------------------------------------------------------ | ------------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| a values entry at the top level the layout does not serve    | `` unknown top-level key `{name}` ``                                                        | closed: `render` returns `Err` and nothing is written |
| a plugin name the layout does not serve                      | `` unknown plugin `{name}` ``                                                               | closed                                                |
| a key inside a laid-out table                                | `` unknown `{table}` key `{name}` ``                                                        | closed                                                |
| a key inside `[lights]` itself                               | `` unknown `lights` key `{name}` ``                                                         | closed                                                |
| a key inside a target declaration                            | `` unknown `lights.{level}` key `{name}` ``                                                 | closed                                                |
| a container key that is not a table                          | `` `{key}` has type `{type}`, not a table ``                                                | closed                                                |
| an opt-in table that is not a table                          | `` `{table}` has type `{type}`, not a table ``                                              | closed                                                |
| a target declaration that is not a table                     | `` `lights.{level}.{name}` is not a table ``                                                | closed                                                |
| a value that will not render, wrapped with its position      | `` `{table}` key `{key}`: {inner} `` and `` `lights.{level}.{name}` key `{key}`: {inner} `` | closed                                                |
| a non-string element in an array                             | `` an array element has type `{type}`, not a string ``                                      | closed                                                |
| a value type nothing renders (float, datetime)               | `` type `{type}` does not render ``                                                         | closed                                                |
| a secret table with a third member                           | `` a secret table may only hold `keepassxc` and `field`, not `{key}` ``                     | closed                                                |
| a table value that is not a two-member secret                | `` a table value must be a secret: exactly `keepassxc` and `field` ``                       | closed                                                |
| a secret whose `keepassxc` is not a string                   | `` a secret's `keepassxc` must name the entry as a string ``                                | closed                                                |
| a secret whose `field` is not a string                       | `` a secret's `field` must be a string ``                                                   | closed                                                |
| a secret field outside the two chezmoi methods               | `` a secret's `field` must be one of ["Password", "UserName"], not `{field}` ``             | closed                                                |
| a blank or whitespace-only entry name                        | `` a secret's `keepassxc` entry name cannot be blank ``                                     | closed                                                |
| an entry name carrying `"`, `\`, `}}` or a control character | `` the keepassxc entry name `{entry}` cannot stand inside a chezmoi action ``               | closed                                                |
| a `note` that is not a string                                | `` `note` has type `{type}`, not a string ``                                                | closed                                                |
| a `note` opening a chezmoi action                            | `` `note` cannot open a chezmoi template action ``                                          | closed                                                |
| a `note` holding a control character other than a newline    | `` `note` cannot hold a control character ``                                                | closed                                                |
| an unclosed chezmoi action while stubbing                    | `a chezmoi action is not closed on its own line: {line}`                                    | closed                                                |
| an action that is not the one secret grammar                 | \`\` not a \`                                                                               | toToml\` secret action: {action} \`\`                 |

### The generator binary (`src/bin/pns-config-render.rs`)

| What is rejected                             | Exact wording                                                               | Exit | Fail direction                                       |
| -------------------------------------------- | --------------------------------------------------------------------------- | ---- | ---------------------------------------------------- |
| no arguments, one argument, or three or more | `usage: pns-config-render <values-file> <template-file>` on stderr          | 2    | closed, nothing written                              |
| any run failure                              | `pns-config-render: refused: {message}` on stderr                           | 1    | closed, nothing written                              |
| the values file cannot be read               | message is `reading {values_path}: {error}`                                 | 1    | closed                                               |
| the values file is not TOML                  | `{values_path} is not valid TOML: {error}`                                  | 1    | closed                                               |
| a literal at a secret-bearing path           | `` `{path}` must be a keepassxc secret marker table, not a literal value `` | 1    | closed                                               |
| the render itself refuses                    | `rendering {values_path}: {error}`                                          | 1    | closed                                               |
| the render's own secret action is malformed  | `the render carries a malformed secret action: {error}`                     | 1    | closed                                               |
| the render will not parse back               | `the render does not self-parse: {detail}`                                  | 1    | closed                                               |
| the template file cannot be written          | `writing {template_path}: {error}`                                          | 1    | closed, but see behavior 26 for what "closed" covers |
| success                                      | `wrote {template_path}` on stdout                                           | 0    | n/a                                                  |

## Behaviors

### 1. The config path is a pure function of a home directory

Given a home directory string\

When `config_path` is called with it\

Then the result is that directory joined with `.config/pns/config.toml`, with no environment read and no file touched

- Success: `src/config.rs:config_path` is `Path::new(home).join(".config/pns/config.toml")`. Pinned by
  `src/config.rs:the_config_lives_under_the_homes_dot_config_pns`, which asserts
  `config_path("/Users/operator") == "/Users/operator/.config/pns/config.toml"`.
- Failure sources: none in this function. Every caller reads `HOME` through
  `std::env::var("HOME").unwrap_or_default()`, so an unset `HOME` yields the relative path
  `.config/pns/config.toml` rather than a refusal (`src/main.rs`, every
  `load_config(&config_path(&home))` call site).
- Fail direction: delivery path, an unset `HOME` resolves to a relative path that almost certainly does
  not exist, which reads as `Missing` and selects the CORE silently. Pulse path, the same relative miss
  reads as `Missing` and exits 0 in silence. Neither says anything about `HOME`.
- Thresholds: Not applicable, there is no number here.
- Required side effects: none. The comment says so: "Pure, so the path rule is testable without an
  environment."
- Forbidden side effects: no `XDG_CONFIG_HOME` read, no fallback path, no directory creation.
- Timeout and cancellation: Not applicable, no IO.
- Idempotency and duplicates: total and deterministic.
- Privacy: the path may carry the operator's home directory name, which reaches the `Unreadable` refusal
  text (`{path}: {error}`). No secret is in a path.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the location is fixed. Nothing overrides it, so a second config location is a
  code change rather than a setting.

### 2. Loading answers with three outcomes, and Missing is one of them rather than an error

Given a path\

When `load_config` reads it\

Then the answer is `Loaded`, `Missing`, or one of three named errors

- Success: `src/config.rs:load_config` calls `std::fs::read_to_string` and hands the text to
  `parse_config`. Pinned by `src/config.rs:a_present_file_loads_through_the_parser`.
- Failure sources: `NotFound` where `symlink_metadata` also fails is `Missing`
  (`src/config.rs:a_missing_file_is_its_own_outcome_not_an_error_and_not_empty`, asserting
  `Ok(LoadOutcome::Missing)` for a nonexistent path); anything else from the read is `Unreadable`
  (`src/config.rs:an_unreadable_path_is_an_error_never_a_silent_unconfigured`, which uses a DIRECTORY at
  the config path as the deterministic case).
- Fail direction: delivery path, `Missing` selects the CORE (`mobile`, `macos-banner`) with no warning,
  and an error selects the CORE with the line
  `pns: config error ({detail}); running the core plugins (mobile, macos-banner)`
  (`src/registry.rs:select_plugins`, `src/registry.rs:core_warning`). Pulse path, `Missing` exits 0 in
  silence and an error prints `pns: config error ({detail}); no pulse` and still exits 0 (`src/main.rs`,
  pinned by `tests/dispatch.rs:an_absent_config_stays_silent_in_pulse_mode` and
  `tests/dispatch.rs:a_broken_config_says_so_in_pulse_mode_too_instead_of_dying_quietly`).
- Thresholds: Not applicable.
- Required side effects: one read of one file. Nothing is written and nothing is created.
- Forbidden side effects: no directory is created, no file is repaired, no default file is written.
  Writing a first config is the wizard's job (`docs/specs/setup-and-publication.md`).
- Timeout and cancellation: none. `read_to_string` is unbounded, so a config path that is a blocking
  device or a FIFO would park the call. `NOT ESTABLISHED:` no read deadline exists in `load_config` and
  no test covers a FIFO at the config path (the FIFO tests in `tests/dispatch.rs` are about the journal
  and the replay, not the config).
- Idempotency and duplicates: every mode loads the config afresh; nothing is cached between calls. On one
  event the composition root loads once and threads the value.
- Privacy: `Unreadable` embeds the path and the operating system's error, both of which reach stderr.
  Neither carries a config value.
- Process ownership and cleanup: no subprocess.
- Compatibility contract: `Missing` and `Unreadable` must stay distinct because the doctor reports them
  with different sentences: `no config file, so only the core runs` versus
  `the config could not be read, so only the core runs` (`src/doctor.rs:NO_CONFIG`,
  `src/doctor.rs:UNREADABLE_CONFIG`, pinned by
  `tests/dispatch.rs:the_doctor_tells_a_machine_with_no_config_that_there_is_no_config`).

### 2a. A dangling symlink is Unreadable and never Missing

Given a symlink at the config path whose target does not exist\

When `load_config` runs\

Then the answer is `Err(ConfigError::Unreadable(...))`

- Success: the guard is `error.kind() == NotFound && std::fs::symlink_metadata(path).is_err()`
  (`src/config.rs:load_config`). A dangling link satisfies the first half and fails the second, so it
  falls through to `Unreadable`. Pinned by
  `src/config.rs:a_dangling_config_symlink_is_an_error_never_missing`, which creates the link, loads,
  removes the link and asserts the variant.
- Failure sources: this IS the failure path.
- Fail direction: closed on the pulse path and on the lights tick, open to the CORE on the delivery path.
  The reason it must not read as `Missing` is stated in the code: "chezmoi deploys configs as symlinks: a
  broken link is a PRESENT entry whose target is wrong, and reading it as 'unconfigured' would silently
  disable everything."
- Thresholds: Not applicable.
- Required side effects: one extra `symlink_metadata` call, and only on the `NotFound` arm.
- Forbidden side effects: the link is not followed further, repaired or removed.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: deterministic.
- Privacy: the path reaches the refusal; nothing else does.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: this is the deployment contract with chezmoi. Any future change that treats a
  broken link as unconfigured silently disables every plugin on a machine mid-apply.

### 3. Malformed TOML is a loud error that never echoes the offending value

Given a config file holding a plugin secret on a line that will not parse\

When `parse_config` runs\

Then the refusal is `Malformed`, it names the cause and the line NUMBER, and it does not contain the value

- Success: `src/config.rs:parse_config` discards the parser's own `Display` (which echoes the offending
  source line) and rebuilds the message from `error.message()` plus a line number computed by counting
  newlines before `error.span().start`. The code comment: "The parser's Display echoes the offending
  source line, and this file carries plugin secrets into log lines, so the refusal is rebuilt from the
  cause and the location alone."
- Failure sources: any TOML syntax error. When the error carries no span, the message is the bare cause
  with no line number.
- Fail direction: delivery path, the CORE plus the warning line, so notifications keep working. Pulse
  path, `no pulse`. Both are stated for a real malformed file by
  `tests/dispatch.rs:the_pulse_config_warning_says_what_pulse_mode_actually_did`, which asserts stderr is
  exactly `` pns: config error (key with no value, expected `=` at line 1); no pulse ``.
- Thresholds: Not applicable.
- Required side effects: none.
- Forbidden side effects: the file is not rewritten, repaired or backed up. There is no partial parse: a
  malformed file yields no `Config` at all, which is the module's own first stated failure direction ("a
  MALFORMED file is a loud error and never a silent empty config").
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: deterministic for a given text.
- Privacy: this is THE privacy behavior of the decode layer, and it is pinned.
  `src/config.rs:a_malformed_line_is_reported_without_echoing_its_value` writes
  `[plugins.mobile]\ntoken = "SUPERSECRET" trailing\n` and asserts both that the cause is still named and
  that the message does NOT contain `SUPERSECRET`. Exhaustively, the paths on which a config VALUE can
  reach a refusal string are: `review_notes` (echoes the glob pattern), `bounded` and `nag_schedule` and
  `seconds` and `threshold` and `submit_deadline` (echo an integer), `behaviours` (echoes the offending
  behaviour word), `ends_agree` (echoes two brightness percentages), and `backstop_outlasts_the_nag`
  (echoes two second counts). None of those keys is secret-bearing. Every other refusal echoes a TYPE
  NAME (`setting.type_str()`) or a KEY NAME, never a value. A secret's key NAME can appear (for example
  \`\`unknown `plugins.mobile` key \`tokens\`\`\`), the value cannot.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the rebuilt message is the contract. Reverting to the parser's own `Display`
  would put secrets in `~/.local/log`.

### 4. The file's own top level serves six tables and refuses everything else by name

Given a config whose outermost keys are not all of `daemon`, `focus`, `lights`, `nag`, `plugins`, `recap`\

When `parse_config` runs\

Then the file is refused whole, the offending name is quoted, and the six are listed

- Success: `src/config.rs:parse_config`'s match arm dispatches the six and its `_` arm reads the roster
  row for `src/config.rs:TOP_LEVEL` (the empty string) to build the list. Pinned by
  `src/config.rs:every_table_refuses_an_unknown_key_by_name_and_lists_what_it_serves`, which walks every
  row of `TABLE_KEYS` including the top-level one.
- Failure sources: a misspelled table (`[plugin.hue]` for `[plugins.hue]`, `[recaps]` for `[recap]`), and
  a MOVED table (`[home]`, whose settings became `[plugins.router]`).
- Fail direction: the whole file is refused, which takes every plugin's secret with it. Delivery path,
  the machine falls to the CORE and prints the warning, so the phone and the banner keep working while
  hermes, hue and the home probe stop. Pulse path, no pulse. The home probe's own diagnostic prints the
  refusal verbatim: `tests/dispatch.rs:every_way_the_home_probe_is_not_set_up_says_which_one_it_is`
  asserts the exact line
  `` home: config error (unknown top-level key `home`; the file serves daemon, focus, lights, nag, plugins, recap) ``.
- Thresholds: exactly six names. Adding a seventh is a two-place edit (the match arm and the roster row)
  and the walk test catches a mismatch in either direction.
- Required side effects: none.
- Forbidden side effects: a table this layer does not serve is never passed through as a key nothing
  reads. The reason: an ignored `[home]` "would leave `pns home` reporting 'not configured' beside a file
  that plainly configures it"
  (`src/config.rs:a_stale_top_level_home_table_is_refused_by_name_rather_than_ignored`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: TOML itself refuses a duplicated table, so this layer never sees one.
- Privacy: only the key name is echoed.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: there is no deprecation window. A renamed or moved table refuses every existing
  file that carries the old spelling, immediately, and the operator's migration prompt is the refusal
  itself.

### 5. A plugin is selected by an explicit flag, and the flag never reaches the plugin

Given `[plugins.<name>]` with settings under it\

When `parse_config` runs\

Then `enabled` is REMOVED from the settings and becomes `PluginEntry::enabled`, an absent flag reads `false`, and everything else survives untouched

- Success: `src/config.rs:parse_config` calls `settings.remove("enabled")` and matches `None => false`,
  `Some(Boolean(flag)) => flag`, `Some(_) => Err`. Pinned by
  `src/config.rs:a_plugin_table_with_enabled_true_is_selected_and_keeps_its_settings`, which asserts the
  entry is enabled, that `bridge` survived, and that `enabled` is NOT a key of the settings table; and by
  `src/config.rs:an_absent_enabled_flag_reads_disabled_because_selection_is_explicit`.
- Failure sources: `enabled = "yes"` is refused by name
  (`src/config.rs:a_non_boolean_enabled_flag_is_refused_naming_the_plugin`); a plugin entry that is not a
  table at all is refused by name
  (`src/config.rs:a_plugin_entry_that_is_not_a_table_is_refused_naming_the_plugin`).
- Fail direction: a refusal here refuses the whole file, so both paths behave as in behavior 4. Note the
  asymmetry that matters: a WRONG-TYPE flag is a refusal, while an ABSENT flag is silently `false`. The
  file "SELECTS; it never defines" (`src/config.rs` module comment), so absent means unselected by
  design.
- Thresholds: Not applicable.
- Required side effects: `config.plugins` is a `BTreeMap`, so listings and error messages are
  deterministic in name order (`src/config.rs:Config`, "Ordered, so listings and errors are
  deterministic").
- Forbidden side effects: `enabled` must not reach the plugin's settings, because it "belongs to this
  layer and everything else belongs to the plugin" (`src/config.rs:PluginEntry`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: deterministic.
- Privacy: a plugin's secret-bearing settings live in this table and are carried through the whole
  `Config` by value. They reach a refusal only as key names, never as values (behavior 3).
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `enabled` is the one key every plugin table shares and the one key this layer
  reads, so it is declared in every `plugins.*` roster row even though it is removed before the row is
  consulted (`src/config.rs:parse_config`, "`enabled` is already out of the table and still listed, since
  it is a key the operator writes").

### 6. A shipped plugin's keys are judged, an unregistered plugin's are not

Given `[plugins.hue]` with `room = "x"` (a near miss for `rooms`)\

When `parse_config` runs\

Then it is refused with the table, the key and the whole vocabulary named

- Success: `src/config.rs:parse_config` runs `admits_flat(&format!("plugins.{name}"), key)` over every
  surviving setting. Pinned by
  `src/config.rs:a_mistyped_key_inside_a_plugin_table_is_refused_naming_the_table_and_the_key`, which
  drives five cases across all five shipped tables (`keys` for `key`, `room` for `rooms`, `sound` for
  `enabled`, `tokens` for `token`, `phone` for `device_hostname`) and asserts the table, the key and a
  near neighbour are all in the sentence.
- Failure sources: any key not in that table's roster row. The rows are at `src/config.rs:TABLE_KEYS`.
- Fail direction: the whole file is refused. That is a deliberate widening whose cost is stated in
  `src/config.rs:the_shipped_config_template_still_parses_through_this_schema`: "Judging every plugin
  table's keys can refuse a config that worked yesterday, and the only config that matters is the one
  this repo ships. If it stops loading, the machine falls back to the CORE with a warning nobody is
  standing in front of: the phone and the banner keep working, and the durable paper trail, the lights
  and the home probe all stop."
- Thresholds: the six judged tables are `plugins.hermes`, `plugins.hue`, `plugins.macos-banner`,
  `plugins.mobile`, `plugins.presence`, `plugins.router`. A table for a plugin nothing registered has NO roster row, so
  `keys_of` returns `None` and `admits` passes everything
  (`src/config.rs:an_unregistered_plugin_tables_settings_stay_free_form_because_selection_is_by_name`).
- Required side effects: the positive control is its own test:
  `src/config.rs:every_key_a_shipped_plugin_table_serves_is_still_admitted` parses a config writing every
  key of five of the six tables and asserts five plugins came back (`plugins.presence` is covered by the
  roster walk below rather than here), so a sweep that refused the whole vocabulary
  would not pass silently.
- Forbidden side effects: this layer never invents a schema for a plugin it does not know. "The registry
  refuses the NAME, which is the defect in that case" (`src/config.rs:TABLE_KEYS`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: deterministic.
- Privacy: only key names.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: adding a key to a shipped plugin is a two-place edit (the roster row and the
  reader). The walk `src/config.rs:every_key_the_roster_declares_is_read_by_the_table_that_declares_it`
  parses one valid sample per roster key and then asserts the sample set equals the roster set exactly,
  so a key declared with no sample is red rather than unproven.

### 7. An unregistered plugin NAME is refused one layer on, and the file still counts

Given a parsed config naming `[plugins.nosuch]` or `[plugins.hermess]`\

When the registry is asked what the config enables\

Then the name is refused, the warning is loud, and the selection widens to the WHOLE roster rather than narrowing

- Success: `src/registry.rs:Registry::enabled` walks the config's names first and returns
  `RegistryError::UnknownPlugin` for any that nothing registered, whether or not it is switched on ("an
  unregistered name is a typo whether or not it is switched on"). `src/registry.rs:select_plugins` turns
  that into `(registry.all(), Some(every_plugin_warning(...)))`, with wording
  `` pns: config error (unknown plugin `{name}`); running every built-in plugin ``.
- Failure sources: a misspelled plugin table name.
- Fail direction: OPEN, and deliberately so, on the delivery path. The reasoning at
  `src/registry.rs:select_plugins`: "the file parsed, so every credential in it is in hand... Narrowing
  here would let one mistyped table name cost a fully configured machine its durable paper trail and its
  lights." Pinned end to end by
  `tests/dispatch.rs:one_typod_table_name_costs_a_configured_machine_no_channel`, which asserts the phone
  AND hermes both fired and that stderr carries both \`\`unknown plugin ```` hermess``` and  ````running
  every built-in
  plugin`. On the PULSE path the same config fails CLOSED: `tests/dispatch.rs:an_unknown_plugin_never_resurrects_a_disabled_pulse`proves a deliberate`enabled
  = false\` on hue is not turned back on by an unrelated typo, by binding a listener the pulse must never
  reach.
- Thresholds: the roster is six registrations (`src/registry.rs:ROSTER`): `router` (a sensor),
  `presence` (a sensor), `mobile`, `macos-banner`, `hermes`, `hue`. The CORE is two names
  (`src/registry.rs:CORE`).
- Required side effects: the warning is printed by the composition root, once.
- Forbidden side effects: no third answer. "SELECTING ONLY THE KNOWN NAMES out of a config with one typo
  in it is a third answer, narrower than either of these. It is not built"
  (`src/registry.rs:select_plugins`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: a duplicate name in the compiled-in roster PANICS at
  `src/registry.rs:build_registry`, which is safe because the only reachable refusal is deterministic and
  compiled in.
- Privacy: the plugin name only.
- Process ownership and cleanup: `router` is registered as a `Sensor`, so no event is ever delivered to
  it (`tests/dispatch.rs:the_binarys_own_roster_knows_the_router_sensor`).
- Compatibility contract: the config's names and the registry's names are two lists that must agree, and
  the only enforcement is this refusal at runtime plus
  `src/config.rs:the_shipped_config_template_still_parses_through_this_schema`, which calls
  `registry::roster().enabled(&config)` over the shipped template.

### 8. `[recap]` reads eight keys, each standing alone, and absent is all on

Given `[recap]` stating one key\

When `parse_config` runs\

Then that key moves and the other seven stay at their defaults

- Success: `src/config.rs:parse_recap` starts from `Recap::default()` and moves only what the file
  states. Pinned by
  `src/config.rs:a_recap_table_is_read_rather_than_refused_and_each_switch_stands_alone` (stating
  `digest = false` leaves `replay_card` and `digest_as_thread` true) and by
  `src/config.rs:a_config_with_no_recap_table_leaves_every_switch_on`.
- Failure sources: a misspelled key
  (`src/config.rs:a_misspelled_recap_key_is_refused_by_name_rather_than_left_at_its_default`, using
  `replaycard`), a non-boolean switch, and each of the four judged shapes covered in behavior 9.
- Fail direction: closed on the pulse path, open to the CORE on the delivery path, as for any decode
  refusal. Within the recap itself the direction is stated per key in the template prose: an unset
  `summarizer`, `repos` or `review_notes` is a WORKING setting, not a broken one
  (`src/config_text.rs:LAYOUT`, the `recap` entry).
- Thresholds: `min_events` defaults to 8 and has a floor of 1 with no ceiling; `summarizer_deadline_secs`
  defaults to 240 and admits 0 through 3600 inclusive. One step either side:
  `summarizer_deadline_secs = 3600` parses to 3600 and `3601` is refused
  (`src/config.rs:the_summarizers_deadline_is_a_count_of_seconds_with_a_generous_default`);
  `min_events = 1` parses to 1 and `0` is refused
  (`src/config.rs:a_volume_threshold_of_zero_is_refused_by_name_rather_than_read_as_every_event`).
- Required side effects: `src/config.rs:parse_recap` calls `admits_flat("recap", &key)` before its match
  even though the `_` arm would refuse the same key with the same sentence. The code says why: "a key
  added to an arm and not to the roster stops working at its own feature test instead of quietly working
  while every refusal listing omits it. Do not delete the five as redundant; the plugin tables have no
  `_` arm at all, and there the gate is the only check."
- Forbidden side effects: no switch implies another. "recap-only and card-only are both valid
  configurations and neither implies the other" (`src/config.rs:Recap`).
- Timeout and cancellation: `summarizer_deadline_secs` is a deadline the recap child enforces, not this
  layer. Zero is accepted here because "a deadline of nothing simply cannot be met, so the recap falls to
  the plain lists and SAYS it did" (`src/config.rs:DEFAULT_SUMMARIZER_DEADLINE_SECS`).
- Idempotency and duplicates: deterministic.
- Privacy: `review_notes` echoes its own glob into a refusal. That glob is a path the operator wrote, not
  a credential.
- Process ownership and cleanup: `summarizer` is ARGV and is never handed to a shell, so "nothing is
  interpreted, so there is no quoting rule and no injection surface" (`src/config.rs:Recap`).
- Compatibility contract: the default is written out rather than derived, precisely so that a machine
  whose config predates the table keeps all three deliveries.

### 9. The recap's counts, lists and glob are judged by shape and refused by name

Given a `[recap]` key holding a value of the right type but the wrong SHAPE\

When `parse_config` runs\

Then it is refused by name with what is wrong, rather than clamped or silently dropped

- Success: four judges, each with its own function. `src/config.rs:threshold` refuses a non-count and a
  zero; `src/config.rs:seconds` refuses a non-count and anything over 3600; `src/config.rs:argv` refuses
  an empty list and an empty FIRST word (only the first, because "an empty ARGUMENT is a real thing to
  pass a program"); `src/config.rs:repositories` refuses an empty list and ANY empty entry;
  `src/config.rs:note_glob` refuses a non-string, an empty file name, a relative path, a `*` in the
  directory, and a second `*` in the file name.
- Failure sources: for the glob, all five are pinned as a table in
  `src/config.rs:a_review_notes_glob_that_names_no_readable_file_is_refused_naming_the_key`, with these
  inputs and expected substrings: `3` and `not a path`; `""` and `names no file`;
  `"slices/checklist-*.md"` and `absolute`; `"~/.claude/*/checklist-*.md"` and `file name may hold a`;
  `"~/.claude/checklist-*-*.md"` and `only one`.
- Fail direction: closed on the pulse path, open to the CORE on the delivery path. Within each key the
  argument is the same: "a silently corrected one is a threshold they believe they set"
  (`src/config.rs:threshold`).
- Thresholds: `min_events` floor 1; `summarizer_deadline_secs` ceiling 3600. `9223372036854775807` is
  explicitly in the refused set, because it "is a plain TOML integer: it parses, and
  `Instant::now() + Duration::from_secs` of it PANICS (MEASURED: 'overflow when adding duration to
  instant') inside a process whose stderr is /dev/null and whose exit code nobody reads"
  (`src/config.rs:seconds`).
- Required side effects: none.
- Forbidden side effects: nothing is clamped anywhere in this file. Every out-of-range value is refused.
- Timeout and cancellation: Not applicable at this layer.
- Idempotency and duplicates: deterministic.
- Privacy: the glob refusals echo the pattern verbatim. That is the only value-echoing refusal on the
  recap keys, and the glob is a path.
- Process ownership and cleanup: the glob "is the WHOLE PERMISSION, which is why its shape is judged here
  rather than resolved generously at the read" (`src/config.rs:note_glob`). A relative path would resolve
  against whatever directory the return event fired in, so "the same key would then name a different set
  of files on every run."
- Compatibility contract: exactly one `*`, in the file name only, and one directory named in full. A
  looser matcher is a widening of what pns is allowed to open.

### 10. `[focus] silence` is the feature switch and the policy in one key

Given `[focus] silence = []`\

When `parse_config` runs\

Then the feature is off and nothing is refused

- Success: `src/config.rs:parse_focus` reads the one key through `src/config.rs:modes`. An empty list is
  admitted (`src/config.rs:an_empty_silence_list_is_admitted_because_it_is_the_feature_switched_off`),
  and an empty ENTRY is refused
  (`src/config.rs:a_mode_name_that_is_the_empty_string_is_refused_by_name`).
- Failure sources: `silence = "Sleep"` (a bare string, which "is what a hand writes first"), a non-string
  element, an empty entry, and a misspelled key (`silenced`).
- Fail direction: closed on the pulse path, open to the CORE on the delivery path. Within the feature, no
  mode named is the feature off, and "MEASURED on this operator's own machine, a Focus was asserted for
  95% of one day, so a feature that shipped on would have silenced almost everything pns raised that day"
  (`src/config.rs:a_config_with_no_focus_table_names_no_mode_at_all`).
- Thresholds: one step either side of "empty" is the whole rule: the empty LIST is accepted, the empty
  STRING inside a list is refused. The distinction is argued at `src/config.rs:modes`: "An empty list
  says 'no mode silences pns'... An empty STRING says nothing at all: no Focus mode is named by it."
- Required side effects: none.
- Forbidden side effects: no `enabled` key exists, so there is no second statement that can disagree with
  the first.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: deterministic. Duplicate names are not deduplicated here.
- Privacy: a Focus mode name reaches no refusal except the empty-string one, which quotes nothing.
- Process ownership and cleanup: this layer never reads Apple's Do Not Disturb store; that is
  `src/focus.rs`.
- Compatibility contract: a name matching no mode is legal and stays legal, "because a name that matches
  no mode is an ordinary thing to write (a Focus you keep on another Mac)."

### 11. `[daemon] enabled` defaults on and fails OPEN when the file cannot be read

Given no config at all, or a config the daemon cannot parse\

When the daemon asks whether it is switched on\

Then it runs

- Success: `src/config.rs:parse_daemon` starts at `DEFAULT_DAEMON_ENABLED` (true) and moves only on an
  explicit boolean. Pinned by
  `src/config.rs:the_daemon_table_reads_one_switch_defaults_on_and_refuses_the_rest_by_name`, which
  covers all four states plus the wrong type, the misspelled key and the non-table form.
- Failure sources: `enabled = "yes"` and `enable = true` are each refused by name.
- Fail direction: this is the ONE place in the surface that fails open on an unreadable file.
  `src/main.rs:daemon_enabled` reads `Err(error)` as `true` and prints
  `pns daemon: the config could not be read ({detail}); carrying on enabled`, with the rationale "a file
  that will not parse must not silently stop a service the operator enabled." `Missing` is also `true`.
  The pulse path never asks this question.
- Thresholds: Not applicable, it is a boolean.
- Required side effects: none at parse.
- Forbidden side effects: nothing else is inferred from the switch.
- Timeout and cancellation: the daemon re-reads on a cadence (`src/main.rs:SWITCH_TICKS`, 30 ticks), so a
  flipped switch takes effect within that window rather than at parse time.
- Idempotency and duplicates: deterministic.
- Privacy: nothing echoed but a type name or a key name.
- Process ownership and cleanup: out of scope here; the daemon's own lifecycle is elsewhere.
- Compatibility contract: default ON is load-bearing for every clock-driven feature, and flipping it to
  default OFF would put both rider features behind two switches.

### 12. `[nag] after_secs` is the switch AND the schedule, with zero carved out

Given `[nag] after_secs = 0`\

When `parse_config` runs\

Then the feature is off and it is not an error

- Success: `src/config.rs:nag_schedule` returns `NAG_OFF` for zero before the range check runs, then
  refuses anything outside 30 to 3600. Pinned by
  `src/config.rs:the_nag_table_reads_one_schedule_defaults_off_and_zero_is_off_rather_than_an_error`,
  which asserts no table is 0, `300` is 300, `0` is 0, and both `30` and `3600` are accepted at their own
  edges.
- Failure sources: eight, table-driven in
  `src/config.rs:a_schedule_that_is_not_a_count_of_seconds_is_refused_by_name`: `-1`, `"5m"`, `300.5`,
  `[300]`, `29`, `3601`, the misspelled `after_seconds`, and `nag = 300` at the top level.
- Fail direction: closed on the pulse path, open to the CORE on the delivery path.
- Thresholds: floor `MIN_NAG_AFTER_SECS` 30, ceiling `MAX_NAG_AFTER_SECS` 3600. One step either side is
  pinned in both directions: 30 and 3600 are accepted, 29 and 3601 are refused. Zero is a third state,
  below the floor and accepted.
- Required side effects: none.
- Forbidden side effects: no `enabled` key, on `[focus] silence`'s own precedent.
- Timeout and cancellation: the ceiling "must also sit inside the daemon's own registration window
  (`daemon::DUE_WINDOW_SECS`, thirty days), which it does with room to spare, and it is what keeps
  `2 * after_secs` in the staleness cap far from any arithmetic edge" (`src/config.rs:nag_schedule`).
- Idempotency and duplicates: deterministic.
- Privacy: an integer is echoed.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: DEFAULT OFF, unlike `[daemon]` beside it, "because this table gates something
  that INTERRUPTS" and needs three separate operator steps before it works.

### 13. `[lights]` absent is None, and an empty `[lights]` is every locked default

Given a config with no `[lights]` heading, and separately one with a bare `[lights]`\

When `parse_config` runs\

Then the first yields `None` and the second yields `Lights::default()` in full

- Success: `src/config.rs:parse_config` maps `"lights"` to `Some(Box::new(parse_lights(value)?))`, so the
  presence of the heading is the signal. Pinned by
  `src/config.rs:no_lights_table_is_none_and_an_empty_one_is_every_locked_default`, which asserts every
  one of the locked figures individually: `refresh_secs` 12; `done` and `failed` both
  `Pulse { duration_ms: 4000, brightness: 100 }`; `blocked` breath `2000/100/30` with
  `give_up_after_secs: 57_600`; `unread` breath `4000/60/10` with `after_secs: 300`; `loop` breath
  `4000/60/10` with `threshold_secs: 300` and `lease_timeout_secs: 3900`; `dim` `3000/7/1`; and all three
  declaration maps empty.
- Failure sources: `lights = 3` is refused as a non-table; anything inside is judged by the arms below.
- Fail direction: on the lamp paths, CLOSED and dark. `src/main.rs:lights_tick` returns 0 on anything
  that is not `Loaded`, with the stated reason "a file nobody could parse routed no lamp, and a map this
  could not read must not be replaced with a guess about which lamps carry what." `pns lights quiet`
  likewise treats an unreadable config as naming no place, so every mute is refused by name while the
  report still runs (`src/main.rs:lights_quiet`). On the delivery path a broken file still leaves the
  phone and the banner running.
- Thresholds: the boxing is a measured decision, not a style one: "measured, the table is 72 of this
  struct's bytes and the whole config travels by value inside `LoadOutcome`" (`src/config.rs:Config`).
- Required side effects: a behaviour table that states one key moves that one and leaves the rest at
  their locked values, because the default arrives as a VALUE rather than being rebuilt
  (`src/config.rs:parse_pulse`, pinned by
  `src/config.rs:a_behaviour_table_moves_the_keys_it_states_and_leaves_the_rest_at_their_locked_values`).
- Forbidden side effects: absence must not be collapsed into the default. The two states are different
  and "the doctor says different things about them."
- Timeout and cancellation: Not applicable at this layer.
- Idempotency and duplicates: deterministic; the three declaration maps are `BTreeMap`s so their order is
  fixed.
- Privacy: lamp, room and zone names are the operator's own text and reach refusals verbatim (for example
  `` `lights.room.3F - Studio` key `dim_hours` ``). They are not credentials.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: "EVERY NUMBER HERE WAS SET ON A REAL LAMP under the operator's
  observe-adjust-lock protocol (2026-08-31 and 2026-09-01), so a change to one of them is a change to
  something that was looked at, not a tuning" (`src/config.rs`, the locked-shape constants).

### 14. Every `[lights]` number is bounded on both sides and refused by name outside them

Given a `[lights]` scalar outside its range\

When `parse_config` runs\

Then the refusal names the table, the key, the value and the whole range

- Success: `src/config.rs:bounded` refuses with
  `` `{table}` key `{key}` is {count}, outside the {low} to {high} range `` and refuses a wrong type with
  `` `{table}` key `{key}` has type `{type}`, not a count between {low} and {high} ``.
  `src/config.rs:percent` layers the 1-to-100 range on top and then does an infallible `u8::try_from`.
- Failure sources: fifteen cases are table-driven in
  `src/config.rs:every_lights_number_is_bounded_on_both_sides_and_refused_by_name_outside_them`:
  `refresh_secs` 9 and 31; `duration_ms` 199 and 5001; `brightness` 0 and 101; `low` 0 and `high` 101;
  `give_up_after_secs` 59 and 604801; `threshold_secs` 0 and 86401; `lease_timeout_secs` 59 and 86401;
  `after_secs` 86401.
- Fail direction: closed and dark on the lamp paths, open to the CORE on the delivery path.
- Thresholds: the SAME test asserts the accepted edges, "which is what makes the bound a bound rather
  than an off-by-one": `refresh_secs` 10 and 30; `duration_ms` 200 and 5000 with `brightness` 1 and 100;
  `threshold_secs` 1 with `lease_timeout_secs` 60; `after_secs` 0; `give_up_after_secs` 60 and 604800.
  Each bound is argued at the constant that holds it, for example `MIN_REFRESH_SECS` is the transport
  deadline ("a tick makes bounded bridge calls whose own limit is ten seconds") and `MAX_REFRESH_SECS` is
  what the daemon derives its child bound from.
- Required side effects: none.
- Forbidden side effects: nothing is clamped. Zero brightness is refused rather than read as off, because
  "a dark signal is a lamp that says nothing, and the way to say nothing is to leave the behaviour off
  that lamp's `shows` list" (`src/config.rs:MIN_BRIGHTNESS`).
- Timeout and cancellation: `MAX_FADE_MS` exists so that `breath_fades` stays total: "a fade past this
  ceiling could be asked for a schedule the shortest interval the config allows has no room left to even
  start."
- Idempotency and duplicates: deterministic.
- Privacy: an integer and two bounds are echoed.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `MAX_REFRESH_SECS` is `pub` because the daemon derives its own child bound from
  it, "not the other way round, and that direction is deliberate."

### 15. A breath whose low is above its high is refused rather than rendered upside down

Given `[lights.blocked] high = 20` and `low = 40`\

When `parse_config` runs\

Then it is refused, naming both ends and what it would cost

- Success: `src/config.rs:ends_agree` runs after every breathing table is read and returns
  `` `{table}` has low {low} above high {high}, so a fade to `high` would move the lamp down and one to `low` would move it up ``.
  Pinned across all four breathing tables (`blocked`, `unread`, `loop`, `dim`) by
  `src/config.rs:a_breath_whose_low_is_above_its_high_is_refused_rather_than_rendered_upside_down`.
- Failure sources: any config stating `low > high` on one of those four tables, whether both ends are
  stated or only one is (the check runs over the merged struct, so a stated `low` above the DEFAULT
  `high` is caught too).
- Fail direction: closed and dark on the lamp paths.
- Thresholds: `low == high` is ACCEPTED, and the test asserts it: "equal ends are a lamp that holds
  steady, which is a shape rather than a mistake." One step further, `low = high + 1`, is refused.
- Required side effects: none.
- Forbidden side effects: the ends are not swapped for the operator. The reason is mechanical: "every
  fade the driver issues moves toward one of these two named values, which is why `low` above `high` is
  refused at load" (`src/config.rs:Breath`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: deterministic.
- Privacy: two percentages are echoed.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `Pulse` has no `low` field at all, so the type system carries the same ruling
  for blinks that `ends_agree` carries for breaths.

### 16. Only the knobs that apply to a behaviour exist on it

Given `[lights.done] low = 10`\

When `parse_config` runs\

Then it is refused by name, with the keys that table DOES serve listed

- Success: each behaviour table has its own roster row, so `admits_flat` refuses a knob from a sibling.
  `src/config.rs:TABLE_KEYS` gives `lights.done` and `lights.failed` only
  `["brightness", "duration_ms"]`, `lights.dim` only `["duration_ms", "high", "low"]`, and so on.
- Failure sources: eleven cases in
  `src/config.rs:a_knob_that_does_not_apply_to_a_behaviour_does_not_exist_on_it`, each asserting the
  refusal contains the key AND the phrase `the table serves`: `low` and `high` on `done`, `low` on
  `failed`, `brightness` on `blocked`, `unread`, `loop` and `dim`, `threshold_secs` on `dim` and `done`,
  `after_secs` on `blocked`, `lease_timeout_secs` on `unread`.
- Fail direction: closed and dark on the lamp paths.
- Thresholds: Not applicable, this is a name check.
- Required side effects: none.
- Forbidden side effects: no dead knob is admitted anywhere. This is an operator ruling enforced by the
  roster, not by a comment: "a blink has a duration and one brightness, a breathing state has a duration
  and two ends... There is no dead knob anywhere for a reader to set and watch do nothing"
  (`src/config.rs:Lights`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: deterministic.
- Privacy: key names only.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `[lights.blocked]`, `[lights.unread]` and `[lights.loop]` each carry ONE knob
  beyond the shared breath keys, and those three knobs are the only asymmetry in the cluster.

### 17. One declaration vocabulary serves all three levels, and the refusal names the operator's path

Given `[lights.room."3F - Studio"] dim_hours = "22:00-07:00"`\

When `parse_config` runs\

Then the refusal reads `` `lights.room.3F - Studio` key `dim_hours` `` and lists `dim_behaviours, dim_window, shows`

- Success: `src/config.rs:parse_targets` calls `admits(TARGET_KEYS, &where_it_is, key)`, which is the
  two-name form of `admits`: the roster row is looked up under `lights.<level>` while the refusal is
  printed with the path the operator wrote. Pinned by
  `src/config.rs:an_unknown_declaration_key_is_refused_by_name_with_the_path_the_operator_wrote`.
- Failure sources: any key outside the three; a declaration that is not a table
  (`src/config.rs:a_declaration_that_is_not_a_table_of_settings_is_refused_by_name` covers
  `lamp = { "HCL1" = 3 }`, `room = 3` and `zone = "Upstairs"`).
- Fail direction: closed and dark on the lamp paths.
- Thresholds: the key set is exactly three, asserted as a set rather than by absence in
  `src/config_text.rs:the_target_declaration_key_roster_is_exactly_shows_dim_window_and_dim_behaviours`,
  because "a fourth key added to `render_target`'s own hardcoded list would pass every existing test
  without ever being asserted as belonging."
- Required side effects: all three levels read the same three keys and land in three separate maps, which
  `src/config.rs:a_declaration_at_any_of_the_three_levels_reads_the_same_three_keys` proves by looping
  over `lamp`, `room` and `zone` with an identical body.
- Forbidden side effects: a target name is never validated against a bridge listing at load.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: `BTreeMap` keyed by the operator's own name.
- Privacy: the operator's lamp names appear in refusals.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `shows` is `Option<Vec<Behaviour>>` and the distinction is load-bearing: `None`
  is "said nothing" and inherits, while `Some(vec![])` is an OVERRIDE that takes one lamp out of a routed
  room (`src/config.rs:a_declaration_that_states_nothing_states_nothing_rather_than_defaulting`).

### 18. `dim_behaviours` with no `dim_window` is refused rather than read and dropped

Given a declaration stating `dim_behaviours` and no `dim_window`\

When `parse_config` runs\

Then it is refused, whether the list is empty or not

- Success: `src/config.rs:parse_targets` tracks a `states_behaviours` flag and refuses when
  `target.dim_window.is_none()`. The refusal is exact and is asserted as a whole string in
  `src/config.rs:dim_behaviours_with_no_window_to_run_them_in_is_refused_rather_than_read_and_dropped`:
  `` `lights.room.3F - Studio` states `dim_behaviours` with no `dim_window` for them to run in, so nothing would ever read them ``.
- Failure sources: `dim_behaviours = ["blocked"]` and `dim_behaviours = []`, both with no window. STATED
  rather than non-empty is the rule, "because an empty list with no window is the same dead knob and the
  two must not disagree."
- Fail direction: closed and dark on the lamp paths.
- Thresholds: Not applicable.
- Required side effects: the boundary is asserted in the same test: an empty list BESIDE a window is
  legal and is "the bedroom rule", a room that goes dark for the night with no second mode to spell it.
- Forbidden side effects: the half-written pair is not silently completed. "The operator gets a lamp that
  strobes all night and a file that says it should not."
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: deterministic.
- Privacy: the target path is echoed.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: the two keys resolve as ONE answer at the tick, which is why they must be
  stated together at load.

### 19. The backstop must outlast the nag, and it is the one refusal that reads two tables

Given `[nag] after_secs = 600` and `[lights.blocked] give_up_after_secs = 60`\

When `parse_config` finishes every table\

Then the file is refused, naming both keys and both values

- Success: `src/config.rs:backstop_outlasts_the_nag` runs after the whole document is walked, because
  "each [is] a perfectly good number on their own and contradict each other only together, so the check
  belongs where the whole file is in hand." The refusal:
  `` `lights.blocked` key `give_up_after_secs` is {give_up}, below `nag` key `after_secs` {after}, so the lamp would be given up on before the nudge it belongs to has ever fired ``.
  Pinned by `src/config.rs:a_backstop_that_gives_up_before_the_nag_nudges_is_refused_naming_both_keys`,
  which asserts all six tokens (`lights.blocked`, `give_up_after_secs`, `60`, `nag`, `after_secs`, `600`)
  are in the sentence.
- Failure sources: only the strictly-shorter case.
- Fail direction: closed and dark on the lamp paths, open to the CORE on the delivery path. Because the
  check runs LAST, a file that trips it is refused whole, so a machine loses hermes, hue and the home
  probe over a two-key contradiction.
- Thresholds: EQUAL is accepted. The test asserts `after_secs = 600` with `give_up_after_secs = 600`
  parses, "which is a tight config rather than a contradictory one." One step shorter is refused.
- Required side effects: none.
- Forbidden side effects: nothing is worked around at runtime "by a mechanism that would have to tell a
  live session from a crashed one."
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: deterministic.
- Privacy: two integers.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: two guards inside this function are documented as DEAD CODE TODAY, and the code
  says so out loud rather than leaving a reader to discover it: `NAG_OFF` is zero and
  `give_up_after_secs` has a floor of 60, so the comparison is already false for an off nag; and
  `DEFAULT_BLOCKED_GIVE_UP_AFTER_SECS` (16 hours) sits far above `MAX_NAG_AFTER_SECS` (one hour), so a
  file with no `[lights]` table could not trip the check at its default. They stay "because what makes
  them dead is a coupling between two bounds that have nothing else to do with each other." The test
  still exercises both spellings of an off nag as accepted configs.

### 20. The mobile submission deadline is read off the ARMED mobile table and nowhere else

Given `[plugins.mobile]` switched on, naming `type = "moshi"`, with `submit_deadline_secs = 30`\

When `submit_deadline` is called\

Then the answer is 30 seconds

- Success: `src/config.rs:submit_deadline` goes through `src/config.rs:armed_mobile`, which returns
  `Ok(None)` for an absent or switched-off table and `Err(reason)` for a table naming a backend nothing
  implements. Pinned by
  `src/config.rs:the_mobile_submission_deadline_is_a_count_of_seconds_defaulted_to_five`.
- Failure sources: seven refused values, table-driven in
  `src/config.rs:a_submission_deadline_that_is_not_a_count_of_seconds_is_refused_by_name`: `0`, `-1`,
  `"5s"`, `9.5`, `[5]`, `3601`, `9223372036854775807`.
- Fail direction: OPEN with a loud line. `src/main.rs:configured_submit_deadline` falls back to
  `DEFAULT_SUBMIT_DEADLINE_SECS` and prints
  `pns: config error ({detail}); the moshi submission keeps its {n}-second bound`, on the argument that
  "a silent fallback is the operator asking for something, not getting it, and being told nothing." There
  is no separate pulse-path reading of this key.
- Thresholds: default 5, floor 1 (zero refused), ceiling 3600. Zero is a TRAP here where
  `summarizer_deadline_secs`'s zero is not, and the refusal says so:
  `` `mobile` key `submit_deadline_secs` is 0, which is the bound switched off by accident: a deadline that expires before the daemon can answer costs the phone card on every approval ``.
  Five seconds is "about thirty times the observed round trip" (measured 2026-08-29,
  `src/config.rs:DEFAULT_SUBMIT_DEADLINE_SECS`).
- Required side effects: the table is read ONCE at the composition root (`src/main.rs:read_mobile`), so
  the token, the watch-card toggle and the refusal come out of one verdict.
- Forbidden side effects: a `submit_deadline_secs` written under a table naming another backend must not
  be read as moshi's. Pinned by
  `src/config.rs:a_mobile_table_naming_no_backend_contributes_no_settings_at_all`, which writes
  `type = "pushover"` with `submit_deadline_secs = 1` and asserts the refusal quotes `"pushover"` and
  names `type`; the same test carries a positive control (`type = "moshi"` gives 30) and the switched-off
  case (a disabled table falls back to 5). A key written under ANOTHER plugin's table no longer even
  parses, because the roster judges each table's vocabulary: the same test asserts
  `[plugins.hue]\nsubmit_deadline_secs = 30` is an error.
- Timeout and cancellation: the value IS a deadline. On expiry "the submission is killed and its pending
  card dies with it, and nothing is said either way" (`src/config_text.rs:LAYOUT`, the
  `submit_deadline_secs` prose). There is no off switch, "because an unbounded wait is the defect and
  'off' would be a key whose only function is to restore it."
- Idempotency and duplicates: deterministic.
- Privacy: the refusal quotes the `type` VALUE (`"pushover"`) and the deadline integer. Neither is
  secret. The `token` on the same table never reaches a refusal.
- Process ownership and cleanup: `armed_mobile` returns a borrow of the settings table, so nothing is
  cloned on this path.
- Compatibility contract: `type` is the one word that selects a backend under EVERY table that has one,
  and the retired router-only spelling `brand` is refused by name with `type` listed instead
  (`src/config.rs:type_is_the_word_that_selects_a_backend_and_the_old_brand_is_refused`).

### 21. The roster is the schema's one statement, checked in both directions

Given `src/config.rs:TABLE_KEYS`\

When the module's own walks run\

Then every declared key is parsed by the arm that declares it, and no arm reads a key the roster does not declare

- Success: two walks. `src/config.rs:every_key_the_roster_declares_is_read_by_the_table_that_declares_it`
  parses one valid sample per roster key from `src/config.rs:SAMPLE_VALUES` and then asserts the walked
  pairs equal the declared pairs exactly.
  `src/config.rs:every_table_refuses_an_unknown_key_by_name_and_lists_what_it_serves` writes
  `zzz_not_a_key` under every roster row, top level included, and asserts the refusal names the level and
  the key and lists every key that row serves.
- Failure sources: a key added to a parse arm and not to the roster stops working at its own feature
  test; a key added to the roster with no arm is refused by that arm and caught by the walk. Both are red
  rather than quiet (`src/config.rs:TABLE_KEYS`).
- Fail direction: Not applicable, this is a build-time guard rather than a runtime path.
- Thresholds: `SAMPLE_VALUES` is written out by hand, `enabled` five times included, "because a generator
  over the roster would derive this list from the very thing it is here to check."
- Required side effects: the roster row for the TOP LEVEL is a row like any other, keyed by
  `src/config.rs:TOP_LEVEL` (the empty string), "so that the refusal an operator gets for a misspelled or
  a MOVED table prints from the same source every other refusal prints from."
- Forbidden side effects: no length is declared on the roster, because "a row added to a fixed-size array
  is a two-place edit, and the count says nothing a reader needs."
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: Not applicable.
- Privacy: Not applicable.
- Process ownership and cleanup: Not applicable.
- Compatibility contract:
  `src/config_text.rs:every_layout_table_matches_the_config_roster_exactly_in_both_directions` is the
  THIRD walk, and it holds the RENDERER's layout to the same roster. Its one exception is `lights`, whose
  effective key set is `refresh_secs` plus the leaf of every `lights.<x>` layout table plus the three
  declaration levels; and its two skipped roster rows are `TOP_LEVEL` (no heading to write) and
  `TARGET_KEYS` (written by the hardcoded declaration branch). A fourth document, the doctor's own setup
  report, is held to the router row's spelling by
  `src/config.rs:the_doctors_own_wording_names_only_keys_the_router_table_serves`, which reads the words
  off the report rather than restating them and checks BOTH directions (every underscore-carrying word is
  a key the table serves, and a line that is supposed to send the operator to a key still names one).

### 22. `render` walks the layout, consumes the values, and refuses whatever is left over

Given a values table\

When `src/config_text.rs:render` walks `LAYOUT`\

Then every recognised key and table is removed as it is written, and anything remaining is refused by name

- Success: `render` clones the values, takes `plugins` out, walks `LAYOUT` once in its own order, and
  then checks both containers for leftovers, returning `` unknown plugin `{name}` `` and
  `` unknown top-level key `{name}` ``. `render_block` does the same per table after writing its keys.
  Pinned by `src/config_text.rs:an_unknown_key_is_refused_by_name_wherever_it_appears`, which covers all
  four levels (a top-level key, a plugin name, a key inside a table, a key inside a target declaration),
  and by `src/config_text.rs:an_unknown_table_is_refused_by_name`.
- Failure sources: any name the layout does not declare, and any container written as a scalar.
- Fail direction: CLOSED. `render` returns `Err` and the binary writes nothing (behavior 26).
- Thresholds: `LAYOUT` declares sixteen tables.
  `src/config_text.rs:render_walks_every_layout_table_and_writes_no_heading_outside_it` asserts both
  directions: every layout table appears in the render (live or commented), and every heading the render
  writes is one the layout declares (skipping quoted target headings).
- Required side effects: CORE tables are written LIVE whether or not the values mention them; OPT-IN
  tables are written COMMENTED, heading included, when the values never mention them at all. Pinned by
  `src/config_text.rs:an_opt_in_table_absent_renders_commented_and_present_renders_live`, which asserts
  the exact text `# [plugins.hermes]\n# enabled = true\n` for the absent case and
  `[plugins.hermes]\nenabled = true\n` for the present one, and confirms the parsed result each way. A
  `Sample::Example` key stays commented even inside a live table unless the values supply a value for it
  (`src/config_text.rs:render_block`).
- Forbidden side effects: the `[lights]` cluster is the ONE hardcoded branch, because its seven headings
  share a single presence flag; `render_lights` pulls every cluster and declaration map OUT of `lights`
  before writing anything, or "render_block's own leftover check would otherwise see the whole cluster
  sitting unclaimed under the bare `[lights]` heading."
- Timeout and cancellation: Not applicable, `render` is pure.
- Idempotency and duplicates: deterministic and order-stable. `toml::Table` is `BTreeMap`-backed, and
  `src/config_text.rs:declarations_at_every_level_render_sorted_hostile_names_quoted_with_their_own_notes`
  asserts Alpha sorts before Zeta at each of the three levels. End to end,
  `tests/config_render.rs:running_the_binary_twice_against_the_same_values_file_writes_identical_bytes`
  pins byte identity across two runs.
- Privacy: `render` writes the values it is given. A literal at a secret-bearing key would be written
  verbatim into the template, which is exactly why the binary refuses one first (behavior 26). Within
  `render` itself, the refusals echo entry NAMES
  (`` the keepassxc entry name `{entry}` cannot stand inside a chezmoi action ``) and key names, never a
  vault value, because a values file holds markers rather than values.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `render` has two callers with different needs. `pns-config-render` adds the
  chezmoi banner and the darwin conditional AROUND the render, deliberately not inside it, "because that
  function's other caller is the first-run wizard, which writes a real file straight to disk with no
  chezmoi templating step at all, and a literal `{{- if }}` line in that file would never be resolved"
  (`src/bin/pns-config-render.rs` module comment). The wizard side belongs to
  `docs/specs/setup-and-publication.md`.

### 23. A secret marker renders as the exact chezmoi action, and a literal renders quoted beside it

Given a values entry `token = { keepassxc = "Moshi :: Webhook Secret", field = "Password" }`\

When `render` writes it\

Then the line is `token = {{ (keepassxc "Moshi :: Webhook Secret").Password | toToml }}` with no author quotes

- Success: `src/config_text.rs:render_value` dispatches on the TOML type, so a table value is a secret
  and everything else is a literal; `src/config_text.rs:secret_action` builds the action. Pinned by
  `src/config_text.rs:a_secret_marker_renders_as_the_chezmoi_action_and_a_literal_renders_quoted`, which
  asserts the exact line AND that `type = "moshi"` is written quoted beside it, then round-trips through
  the stub and asserts the parsed value is the substituted string.
- Failure sources: five refusals, each with its own test. A third member:
  `src/config_text.rs:a_secret_tables_unknown_member_is_named_rather_than_only_counted`
  ("`table.len() != 2` ALONE only counts members... naming the offender needs its own check"). A field
  outside the two: `src/config_text.rs:a_secrets_field_is_whitelisted_to_the_two_chezmoi_methods` (using
  `Notes`). A hostile entry name:
  `src/config_text.rs:a_hostile_entry_name_is_refused_rather_than_closing_the_chezmoi_action` over `a"b`,
  `a\b`, `a}}b` and `a\nb`. A blank name:
  `src/config_text.rs:a_blank_or_whitespace_only_entry_name_is_refused_rather_than_written` over `""`,
  `"   "` and `"\t"`, whose stated mutant is "the blank-entry refusal removed, letting `keepassxc ""`...
  reach the shipped template and defer the failure to an apply-time vault lookup nobody is standing in
  front of."
- Fail direction: CLOSED. Nothing is written.
- Thresholds: `SECRET_FIELDS` is exactly `["Password", "UserName"]`, and BOTH are exercised:
  `src/config_text.rs:a_username_secret_marker_renders_the_exact_action_and_round_trips_through_the_stub`
  exists because "a `SECRET_FIELDS` narrowed to `Password` alone would pass every other test in this
  module."
- Required side effects: the action is built with `push_str` rather than `format!`, deliberately,
  "because the target text is thick with literal `{`, `}` and `"` characters, and escaping all of them
  inside a format string is exactly the kind of place a stray brace goes unnoticed."
- Forbidden side effects: no author quotes. This is the whole point, and it is why the stub's placeholder
  must supply a quoted string for the file to parse in a test.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: deterministic.
- Privacy: the ENTRY NAME and the FIELD are written into a committed file (the template) and into a
  committed fixture (`tests/fixtures/resolved-config.snapshot`, where
  `src/config.rs:identity_placeholder` renders each as `"from-the-vault:<entry>:<field>"`). No vault
  VALUE is written anywhere, at any stage, by any of these code paths: `render` never opens KeePassXC and
  neither does the test suite. Verified by reading the committed snapshot: the five stubbed values carry
  only entry names and field names.
- Process ownership and cleanup: Not applicable, no subprocess.
- Compatibility contract: the escape rules that stand around the action.
  `src/config_text.rs:a_secret_holding_a_quote_and_a_backslash_round_trips_through_the_totoml_stub`
  encodes what `toToml` actually emits for the bytes `a"b\c`, namely `"a\"b\\c"`, "per the sol-1 probe
  table," so the stub is handed the TOML text chezmoi would have produced rather than the raw secret.

### 24. A hostile literal, a hostile name and a hostile note cannot escape their line

Given a values string holding a quote, a newline, a `#`, or a `{{`\

When `render` writes it\

Then it crosses as one inert basic string and never as structure

- Success: `src/config_text.rs:quoted` escapes `\`, `"`, every control character and, unusually, `{` and
  `}` as `\uXXXX`. The brace escaping is not TOML's requirement, it is chezmoi's: "this text is what an
  eventual `.tmpl` file regenerates from, and chezmoi's own template engine reads a live `{{ ... }}`
  action anywhere in that file, quotes or no quotes." Pinned by
  `src/config_text.rs:a_hostile_literal_crosses_as_one_inert_string_and_never_as_structure` (the value
  `"\n[evil]\nenabled = true\n# not a comment` parses back as ITSELF and opens no heading) and by
  `src/config_text.rs:a_literal_holding_a_chezmoi_action_opening_crosses_with_its_braces_broken_up`
  (asserting the rendered text contains neither `{{` nor `}}`).
- Failure sources: a `note`, which is written as a RAW comment rather than a quoted string, cannot be
  protected by `quoted` and is therefore refused outright when it opens an action
  (`src/config_text.rs:a_note_holding_a_chezmoi_action_opening_is_refused_by_name`) or holds a forbidden
  control character
  (`src/config_text.rs:a_note_holding_a_forbidden_control_character_is_refused_by_name`, over
  `bad\u{0}byte`, `bad\u{7f}byte` and `lone\rcarriage`).
- Fail direction: for a literal, OPEN and escaped; for a note, CLOSED and refused. The asymmetry is the
  point: a literal can be made inert, a raw comment cannot.
- Thresholds: CRLF is the one control sequence a note may carry, normalized to `\n` rather than refused,
  "since it is an ordinary line ending rather than a hostile control." The same test asserts
  `"line one\r\nline two"` renders as `# line one\n# line two\n` and parses.
- Required side effects: `src/config_text.rs:write_note` gives EVERY `\n`-split line its own `# ` prefix,
  "which is what keeps a newline inside the operator's own text from opening a heading or an uncommented
  key." Pinned by `src/config_text.rs:a_note_holding_a_newline_stays_commented_on_every_line`, which
  plants a full `[plugins.hue]` table inside a note and asserts the parsed config does not contain `hue`.
- Forbidden side effects: `note` is a RESERVED key invisible to the roster. It never reaches the output
  as `note = "..."` and never round-trips into a parsed config
  (`src/config_text.rs:a_note_renders_above_its_heading_as_a_commented_line`).
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: deterministic.
- Privacy: a hostile literal is escaped, so nothing about it leaks structurally. A note is refused with a
  message that does NOT echo the note's text (`` `note` cannot hold a control character ``), so a pasted
  note carrying something sensitive is not printed back.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: a quoted target NAME is escaped the same way, so
  `[lights.room."Alpha \"Room\""]` is the heading a room named with a quote produces
  (`src/config_text.rs:declarations_at_every_level_render_sorted_hostile_names_quoted_with_their_own_notes`).

### 25. `strip_chezmoi_actions` stands in for chezmoi, and only for the one action grammar

Given a chezmoi-templated text\

When `strip_chezmoi_actions` runs\

Then a directive standing on its own line goes with the line, and an action in value position becomes the placeholder the caller supplies

- Success: `src/config.rs:strip_chezmoi_actions` drops every line whose trimmed start is `{{-` (which is
  the darwin conditional's two lines) and replaces every in-value action with
  `placeholder(entry, field)`. It is NOT test-only: `pns-config-render` calls it at runtime "to stand in
  for chezmoi before self-parsing its own render, so it returns a refusal naming the offender rather than
  panicking."
- Failure sources: an unclosed action (`a chezmoi action is not closed on its own line: {line}`) and any
  action that is not the secret grammar (`` not a `| toToml` secret action: {action} ``).
- Fail direction: CLOSED. In the binary this becomes
  `the render carries a malformed secret action: {error}` and nothing is written.
- Thresholds: the grammar check is exact. The prefix must be `{{ (keepassxc "`, the entry must contain no
  `"`, and the remainder must equal `{field} | toToml }}` for one of the two whitelisted fields.
- Required side effects: `src/config.rs:identity_placeholder` carries the action's OWN entry and field
  into the placeholder text, so no two DIFFERENT secrets stub to the same value. That was a review
  finding: "a single fixed placeholder stubs every secret in a multi-secret text to the SAME value, so a
  table-comparison test built on top of it cannot tell a swapped entry from an unswapped one, sol-1
  finding 1 (two tables comparing equal after their secrets traded places)."
- Forbidden side effects: no other action shape is stood in for. "Swapping a quoted placeholder in for
  ANY action would let a template line that dropped `| toToml` keep every template test green while
  chezmoi splices the raw vault bytes in unquoted." Pinned by
  `src/config.rs:the_stub_refuses_a_secret_action_that_forgot_totoml`.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: deterministic. It is not a chezmoi and does not claim to be: "What this
  test reads is which KEYS the file names and under which tables, and no action in it is a key or a
  table; they are one conditional wrapper and five secrets."
- Privacy: the refusals echo the offending ACTION or the whole LINE, which in the template's case names a
  vault ENTRY. No vault value is ever in hand at this point, so none can be echoed.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `identity_placeholder` escapes a backslash defensively, "`entry` is already
  known quote-free by the caller above, but not backslash-free, and this keeps the result a valid TOML
  basic string either way."

### 26. `pns-config-render` reads, refuses, renders, self-parses, then writes, in that order

Given a values file and a template path\

When the binary runs\

Then nothing reaches the template path until every earlier step has succeeded

- Success: `src/bin/pns-config-render.rs:run` is five steps in a fixed order: read,
  `refuse_literal_secrets`, `render`, `strip_chezmoi_actions` plus `parse_config`, then `std::fs::write`.
  The module comment states the invariant: "a values file that renders something the parser itself would
  reject must never reach disk, because the shipped template is the config of record and a rejected file
  falls the machine back to the CORE alone." On success it prints `wrote {template_path}` and exits 0.
- Failure sources and their pins, each with an explicitly named mutant: the self-parse step skipped,
  pinned by
  `tests/config_render.rs:a_values_file_that_renders_something_the_parser_rejects_is_refused_without_writing`
  using `[nag] after_secs = 3601` ("`render` alone never bounds an integer"); the literal-secret check
  removed OR NARROWED, pinned by
  `tests/config_render.rs:a_literal_value_at_any_secret_bearing_key_is_refused_without_writing`, which
  table-drives all five paths because "a single case covering only `plugins.hue.bridge` stays green if
  the other four are removed from that list"; the roster refusal loosened, pinned by
  `tests/config_render.rs:an_unknown_values_entry_is_refused_without_writing`; the banner gutted, pinned
  by
  `tests/config_render.rs:the_written_template_starts_with_the_generated_banner_and_the_darwin_wrapper`,
  which holds a SECOND independent copy of the banner text; nondeterminism in the walk, pinned by
  `tests/config_render.rs:running_the_binary_twice_against_the_same_values_file_writes_identical_bytes`;
  a binary that validates its input and then writes a fixed body regardless, pinned by
  `tests/config_render.rs:the_binary_over_the_committed_values_file_writes_the_committed_template_exactly`;
  and the argv guard, pinned by `tests/config_render.rs:missing_arguments_print_usage_and_exit_2` and
  `tests/config_render.rs:a_third_argument_prints_usage_and_exit_2`.
- Fail direction: CLOSED, and specifically closed on a PRE-EXISTING file. Every refusal test plants
  `tests/config_render.rs:SENTINEL_TEMPLATE` at the output path first and then asserts it survived
  byte-identical, because "asserting `!template_path.exists()` on a path that started out absent proves
  nothing about that: the file was never there to begin with, so a refusal that destroys a pre-existing
  one would still pass."
- Thresholds: exactly two arguments. Zero, one, or three or more print
  `usage: pns-config-render <values-file> <template-file>` and exit 2. The three-argument case also
  asserts nothing was written.
- Required side effects: exactly one file write, at the very end, of `BANNER + rendered + FOOTER`.
- Forbidden side effects: the binary is never installed. The module comment: "Dev-only: turns the
  committed values file into the shipped chezmoi template. Never installed (see the build script under
  `.chezmoiscripts`, which only ever copies `target/release/pns`); run by hand through
  `just pns-config-render`." The recipe at `justfile:pns-config-render` is
  `cargo run --locked --quiet --manifest-path dot_local/share/pns/Cargo.toml --bin pns-config-render -- dot_config/pns/config-values.toml dot_config/pns/private_config.toml.tmpl`.
- Timeout and cancellation: none. `NOT ESTABLISHED:` there is no deadline on the read or the write, and
  none is needed for a hand-run developer tool.
- Idempotency and duplicates: byte-identical across runs, pinned.
- Privacy: `refuse_literal_secrets` names the PATH
  (`` `plugins.hue.bridge` must be a keepassxc secret marker table, not a literal value ``) and never the
  offending value, so a pasted credential that triggers the refusal is not echoed to stderr. That is the
  single most important privacy property of this binary, and it is a consequence of the message's shape
  rather than of an explicit test: the tests assert stderr CONTAINS the key path, not that it excludes
  the value. `NOT ESTABLISHED:` no test asserts that a pasted literal secret is absent from the refusal
  text.
- Process ownership and cleanup: the write is a plain `std::fs::write`, not a publish-by-rename. A write
  that fails part way leaves a truncated template. `NOT ESTABLISHED:` no atomic-write or rollback
  mechanism exists in this binary, and no test covers a partial write.
- Compatibility contract: `BANNER` is duplicated by hand in three places (the binary, the crate test, and
  the integration test) rather than imported, and each copy says why: "if that binary's own copy were
  ever deleted or gutted to an empty string, importing it here would make both sides agree on nothing and
  this test would still pass."

### 27. The shipped template is pinned byte for byte, and three separate pins cover what one cannot

Given the committed `dot_config/pns/config-values.toml`\

When the crate's test suite runs\

Then the committed `dot_config/pns/private_config.toml.tmpl` must equal `BANNER + render(values) + FOOTER` exactly

- Success: `src/config.rs:the_committed_template_is_render_over_the_committed_values_file` parses
  `CONFIG_VALUES`, calls `config_text::render`, wraps it, and asserts equality with `SHIPPED_TEMPLATE`.
  The failure message is the operator's instruction: "the shipped template drifted from `render` over the
  committed values file; regenerate with `just pns-config-render`".
- Failure sources: a hand edit to the template; an unregenerated values-file edit.
- Fail direction: a red test rather than a runtime behavior. There is no runtime consequence until an
  apply deploys the file.
- Thresholds: three pins, layered, because each has a mutant the others cannot see. Byte equality catches
  a hand edit but NOT a table dropped from the values file, because "a table ABSENT from the committed
  values file renders COMMENTED OUT rather than refused, so dropping `[nag]` from that file and running
  `just pns-config-render` writes a template with the nag the operator runs switched OFF, and the
  byte-equality test stays green because both sides moved together. Measured: with `[nag]` dropped the
  whole Rust suite passes." `src/config.rs:LIVE_TABLES` closes that: an enumerated list of the 22
  uncommented headings, kept by hand OUTSIDE the values file, asserted in order by
  `src/config.rs:every_table_the_operator_runs_is_still_live_in_the_shipped_template`. Its stated
  ceiling: "this pins WHICH tables are live, not what every live key holds."
  `src/config.rs:RESOLVED_CONFIG_SNAPSHOT` closes THAT: the `{:#?}` of the parsed `Config` over the
  rendered values, committed at `tests/fixtures/resolved-config.snapshot`, asserted by
  `src/config.rs:the_resolved_configuration_over_the_committed_values_file_matches_its_snapshot`. It
  catches four keys that render COMMENTED when dropped without moving any heading (`plugins.hue.rooms`,
  `plugins.hue.quiet_hours`, `plugins.router.router_url`, `plugins.router.device_hostname`) and a fifth
  that renders LIVE at its schema default (`lights.loop.threshold_secs`, which falls from 360 to 300).
- Required side effects: on a mismatch the snapshot test writes the actual text to a scratch file under
  `std::env::temp_dir()` named for the process id and panics with two literal commands, a `diff` and a
  `cp`, so the update procedure is printed rather than remembered.
- Forbidden side effects: the snapshot is committed SEPARATELY from the values file and the template, on
  purpose, "because a snapshot regenerated the same way those two are would move in lockstep with every
  values-file edit and never disagree with anything."
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: the render is deterministic, so re-running produces the same bytes.
- Privacy: the snapshot carries the five vault entry names and their fields, stubbed through
  `identity_placeholder`, and no vault values. Verified by reading it.
- Process ownership and cleanup: the scratch snapshot file is written and NOT removed, deliberately, so
  the failure message can hand back a `cp` that runs from anywhere.
- Compatibility contract: the honesty ceiling is stated rather than hidden: "it is only as honest as
  whoever updates it: nothing stops a `cp` run without reading the diff first, which is why the diff step
  is spelled out above rather than folded into one command." And what none of the three reaches: "a
  plugin's own runtime reading of its `settings` table (`channels/hue.rs` deciding what a room NAME
  means, for one) is downstream of this layer and out of its reach."

### 28. The shipped template still parses through this schema, and states its defaults visibly

Given `SHIPPED_TEMPLATE` with its chezmoi actions stubbed\

When `parse_config` runs over it\

Then it loads and selects exactly hermes, hue, macos-banner, mobile and router, all of which the registry knows

- Success: `src/config.rs:the_shipped_config_template_still_parses_through_this_schema` asserts the
  plugin key list is exactly `["hermes", "hue", "macos-banner", "mobile", "router"]` and then calls
  `registry::roster().enabled(&config)` to prove every name is registered. Its comment states the stake:
  "THE FENCE UNDER THE SWEEP... If it stops loading, the machine falls back to the CORE with a warning
  nobody is standing in front of."
- Failure sources: any schema tightening that the shipped file happens to violate.
- Fail direction: a red test at build time. At runtime the consequence would be the CORE fallback, which
  keeps the phone and the banner and loses the durable paper trail, the lights and the home probe.
- Thresholds: `src/config.rs:the_shipped_template_states_the_blocked_backstop_at_its_default_uncommented`
  asserts a LINE, `give_up_after_secs = 57600`, rather than a parsed value, and says why: "the key fence
  counts a commented line too, and the parser reads the same number whether the line is there or not, so
  only the line itself pins the ruling." That is the "defaults visible in config" ruling enforced at the
  one place a parsed assertion cannot reach it.
- Required side effects: none.
- Forbidden side effects: none.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: deterministic.
- Privacy: the stub substitutes for the five secrets before parsing, so no vault access happens.
- Process ownership and cleanup: Not applicable.
- Compatibility contract: `src/config.rs:documented_keys_the_roster_serves` is a `#[cfg(test)]` scanner
  that reads COMMENTED lines too, "which is the half a parse cannot reach: most of a documented config is
  documentation, and a key documented there but refused by the code is a line an operator uncomments and
  then cannot load." Its doc comment describes it as serving "BOTH TEXTS HELD TO THIS SCHEMA, the shipped
  template and what `pns setup` composes", and says "the count is returned rather than pinned here
  because only the template has a number worth pinning." That second claim is currently stale: the
  scanner's only two callers are
  `src/config_text.rs:the_routing_prose_is_always_written_and_the_example_only_when_nothing_is_declared`
  and `src/setup.rs:every_key_it_writes_is_a_key_the_roster_serves_however_the_walk_was_answered`, and
  NOTHING runs it over `SHIPPED_TEMPLATE`. `NOT ESTABLISHED:` there is no pin on the shipped template's
  documented-key count, so a key documented in the template under a table that does not serve it would
  not be caught by that scanner today. It would still be caught by behavior 27's byte-equality pin as
  long as the template stays generated from the values file.

### 29. A broken config fails open on the delivery path and closed on every lamp path

Given a config file that will not load\

When each mode runs\

Then the delivery legs continue at the CORE while the pulse, the lights tick and the lamp mutes do nothing

- Success: the split is the design, and both sides are stated in code. Delivery:
  `src/registry.rs:select_plugins` returns `(registry.core(), Some(core_warning(...)))` for every `Err`,
  on the argument that "on an always-exit-0 notification path, a config error that silently turned every
  notification off would be the exact failure the config layer exists to refuse, and the three it leaves
  out could not have delivered anything anyway, since their credentials are in the file nobody could
  read." Pulse: `src/main.rs`'s pulse mode says the opposite in as many words: "FAIL CLOSED, unlike an
  event. The roster fallback that keeps every notification working through a broken config is an
  EVENT-mode rule: applying it here would let an unrelated typo switch a deliberately disabled pulse back
  on."
- Failure sources: any `Malformed`, `Invalid` or `Unreadable`.
- Fail direction: this behavior IS the fail direction, stated per mode.

| Mode                                                      | Missing                                                              | Error                                                 | Wording                                                                            |
| --------------------------------------------------------- | -------------------------------------------------------------------- | ----------------------------------------------------- | ---------------------------------------------------------------------------------- |
| event delivery                                            | CORE, silent                                                         | CORE, loud                                            | `pns: config error ({detail}); running the core plugins (mobile, macos-banner)`    |
| event delivery, unknown plugin name in a file that PARSED | n/a                                                                  | whole roster, loud                                    | `` pns: config error (unknown plugin `{name}`); running every built-in plugin ``   |
| `pns pulse`                                               | exit 0, silent                                                       | exit 0, loud, no pulse                                | `pns: config error ({detail}); no pulse`                                           |
| lights tick                                               | return 0, nothing armed                                              | return 0, nothing armed                               | silent (a line per tick would be a log the rotation job rotates a real log out of) |
| `pns lights quiet`                                        | no place is known, every mute refused by name, the report still runs | same                                                  | the mute's own refusal                                                             |
| daemon enable check                                       | enabled                                                              | enabled, loud                                         | `pns daemon: the config could not be read ({detail}); carrying on enabled`         |
| `submit_deadline`                                         | 5 seconds                                                            | 5 seconds, loud                                       | `pns: config error ({detail}); the moshi submission keeps its {n}-second bound`    |
| `pns home`                                                | a setup line                                                         | a setup line                                          | `home: config error ({detail})`                                                    |
| `pns doctor`                                              | `no config file, so only the core runs` per skipped plugin           | `the config could not be read, so only the core runs` | as shown                                                                           |

- Thresholds: the CORE is exactly two names, `mobile` and `macos-banner` (`src/registry.rs:CORE`), and
  the ruling behind that number is recorded: "Three of the five plugins cannot do anything until a
  credential is stood up for them... so a default that switched them on delivered nothing and reported
  three failures on a machine whose operator had asked for none of it."
- Required side effects: each warning is printed exactly once, by the composition root.
- Forbidden side effects: no mode repairs, rewrites or backs up a broken config.
- Timeout and cancellation: Not applicable.
- Idempotency and duplicates: every mode re-reads, so a repaired file takes effect on the next invocation
  with no cache to invalidate.
- Privacy: `detail` is the sanitized refusal from behavior 3, so a secret cannot ride any of these lines.
- Process ownership and cleanup: the lights tick keeps held-lamp records rather than clearing them when
  it cannot address a bridge, "since putting it out takes a bridge."
- Compatibility contract: the three-way split of `Missing`, `Unreadable` and `Invalid` must survive,
  because three different sentences are built off it and "one wording covering them sends two thirds of
  the operators to the wrong one" (`src/doctor.rs:ConfigState`).

## Gaps

Every `NOT ESTABLISHED:` line above, gathered.

1. There is no config version key and no migration mechanism. Evidence is negative and is set out in the
   section "Is the config versioned?".
1. `load_config` has no read deadline, and no test covers a blocking device or a FIFO at the config path.
1. Nothing pins the shipped template's documented-key count, despite
   `src/config.rs:documented_keys_the_roster_serves` describing itself as serving that text; its only two
   callers are in `src/config_text.rs` and `src/setup.rs`.
1. No test asserts that a pasted literal secret is ABSENT from `pns-config-render`'s refusal text. The
   message's shape (it names the key path, not the value) is what provides the property today.
1. `pns-config-render` writes with a plain `std::fs::write` rather than a publish-by-rename, so a write
   that fails part way leaves a truncated template. No rollback exists and no test covers it.

## Glossary

| Term                                    | Defining symbol                                                  |
| --------------------------------------- | ---------------------------------------------------------------- |
| load outcome                            | `src/config.rs:LoadOutcome`                                      |
| missing config                          | `src/config.rs:LoadOutcome::Missing`                             |
| malformed                               | `src/config.rs:ConfigError::Malformed`                           |
| invalid                                 | `src/config.rs:ConfigError::Invalid`                             |
| unreadable                              | `src/config.rs:ConfigError::Unreadable`                          |
| the config path                         | `src/config.rs:config_path`                                      |
| the roster (schema vocabulary)          | `src/config.rs:TABLE_KEYS`                                       |
| the top-level row                       | `src/config.rs:TOP_LEVEL`                                        |
| the declaration row                     | `src/config.rs:TARGET_KEYS`                                      |
| plugin entry                            | `src/config.rs:PluginEntry`                                      |
| armed mobile table                      | `src/config.rs:armed_mobile`                                     |
| behaviour word                          | `src/config.rs:BEHAVIOUR_WORDS`                                  |
| behaviour                               | `src/config.rs:Behaviour`                                        |
| pulse (a blink)                         | `src/config.rs:Pulse`                                            |
| breath                                  | `src/config.rs:Breath`                                           |
| dim form                                | `src/config.rs:Lights::dim`, default `src/config.rs:DEFAULT_DIM` |
| dim window                              | `src/config.rs:Target::dim_window`                               |
| target (lamp, room or zone declaration) | `src/config.rs:Target`                                           |
| the blocked backstop                    | `src/config.rs:DEFAULT_BLOCKED_GIVE_UP_AFTER_SECS`               |
| the backstop-versus-nag check           | `src/config.rs:backstop_outlasts_the_nag`                        |
| the ends check                          | `src/config.rs:ends_agree`                                       |
| bounded scalar                          | `src/config.rs:bounded`                                          |
| percent                                 | `src/config.rs:percent`                                          |
| the chezmoi stub                        | `src/config.rs:strip_chezmoi_actions`                            |
| identity placeholder                    | `src/config.rs:identity_placeholder`                             |
| the documented-key scanner              | `src/config.rs:documented_keys_the_roster_serves`                |
| the shipped template                    | `src/config.rs:SHIPPED_TEMPLATE`                                 |
| the committed values file               | `src/config.rs:CONFIG_VALUES`                                    |
| the live-table list                     | `src/config.rs:LIVE_TABLES`                                      |
| the resolved-config snapshot            | `src/config.rs:RESOLVED_CONFIG_SNAPSHOT`                         |
| the layout                              | `src/config_text.rs:LAYOUT`                                      |
| core versus opt-in table                | `src/config_text.rs:Table::opt_in`                               |
| default sample versus example sample    | `src/config_text.rs:Sample`                                      |
| secret action                           | `src/config_text.rs:secret_action`                               |
| secret fields                           | `src/config_text.rs:SECRET_FIELDS`                               |
| note (a renderer directive)             | `src/config_text.rs:take_note`, `src/config_text.rs:write_note`  |
| quoted literal                          | `src/config_text.rs:quoted`                                      |
| secret-bearing key                      | `src/bin/pns-config-render.rs:SECRET_BEARING_KEYS`               |
| the generated banner                    | `src/bin/pns-config-render.rs:BANNER`                            |
| the CORE selection                      | `src/registry.rs:CORE`                                           |
| the selection policy                    | `src/registry.rs:select_plugins`                                 |
| config state (as the doctor sees it)    | `src/doctor.rs:ConfigState`                                      |
