# uu tooling lanes: the implementation plan

Written 2026-09-06 against `origin/main` at `319811e2`. It implements
`docs/superpowers/specs/2026-09-05-uu-tooling-lanes-design.md` as amended the same day: nine lane
types, the `pending` record state and its escalation, the failure webhook, the stable toolchain pin,
one new verb, and the retirement of the three bash weekly jobs and the hourly log rotation. The method
is `~/.agents/skills/clean-code/SKILL.md` together with
`~/.agents/skills/clean-code-rust/SKILL.md`; Rust wins every number and mechanism. The ladder has
42 pull requests (PRs), including six prerequisites for architecture left outside the completed
responsibility split in #358 and a separate weekly skills assembly before cutover. It names,
per pull request,
what is added or moved, which tests carry it by name, which consumer has to keep working, what every
resulting file is projected to measure, and which earlier pull request it has to land after.

## 1. Where the ladder starts

The crate at `dot_local/share/uu` has 59 Rust files and none over 500 lines; the largest are
`src/runner.rs` and `src/lanes/brew/upgrade_record.rs` at 381. Five lane kinds exist (`brew`,
`command`, `herdr`, `npm`, `uv`), one config module and one lane module each, and a lane type is added
by extending seven places. This is the starting inventory, not the final extension contract. Step 0
replaces central dispatch and assigns each later path to its owning crate:

1. `LANE_TYPES`, `LaneKind` and `type_name` in `src/config/lanes.rs`, and the dispatch arm in
   `parse_lanes`.
1. The `TABLE_KEYS` row for the type in `src/config/schema.rs`, whose test
   `every_key_the_roster_declares_is_actually_read` fails on a key declared but unread.
1. The `run_lane` arm in `src/lanes.rs`.
1. The three fixture tables that assert `LANE_TYPES.len()` against their own length:
   `every_built_in_lane_type_has_a_minimal_block_the_parser_accepts` (`config/lanes.rs`),
   `every_built_in_lane_type_can_be_selected_and_run_and_keeps_its_own_name` (`lanes.rs`), and
   `every_built_in_lane_type_is_bounded_whether_or_not_its_block_says_so` (`deadline.rs`).
1. The shipped template `dot_config/uu/private_config.toml.tmpl` and the assertion for the new block in
   `the_shipped_template_still_parses_and_selects_what_it_selects` (`config/shipped_template.rs`).
1. `uu doctor` (`src/cli/doctor.rs`): the lane line, and the program-reachability line the command
   lane already prints.
1. The `LaneAdapter` impl beside the config struct, with tests against `ScriptedRunner` in
   `src/lanes/stubs.rs`.

The Neovim config at `dot_config/nvim` has no `lua/uu/` directory yet. Its headless specs live in
`dot_config/nvim/tests/*_spec.lua`, are globbed by `tests/run.lua` with `lua/` on `package.path`, and
run under `nvim --headless --clean -l` through `just test-nvim`, itself a dependency of `just
test-unit`, so a `uu_*_spec.lua` is picked up with no registration and runs at every commit.

The three bash jobs and the rotation script are exactly where the spec says: 3,806, 461 and 561 lines
of bash for `update-skills.sh`, `report-plugin-updates.sh` and `log-entries.sh`, and 337 for
`compress-and-truncate-local-logs.sh`, each with its LaunchAgent and loader. Section 5 lists every
referrer of each, found by grep on this commit.

## 2. The consumers that must keep working, and how each is proved

- **The builder**, `.chezmoiscripts/run_onchange_after_59-build-uu.sh.tmpl`. PR 0.1 adds uu member
  manifests to its hashed inputs; B1 adds the toolchain pin and corrects the cargo working directory.
  Run the resulting cargo line with a fresh retained target directory, never the apply script.
- **The loader**, `run_onchange_after_71-load-uu-launchagent.sh.tmpl`, pinned by
  `test/unit/uu-launchagent-loader.bats`. The plist changes once (PR C5, the PATH) and the loader
  re-fires on the plist hash; the bats file still passes because it stubs `launchctl`.
- **The justfile**: PR 0.1 makes uu's `test-rust` lines select every workspace member for tests,
  formatting and clippy. `test-nvim` picks up the new specs. `update-skills` changes in PR E16.
- **The shipped template test** currently reaches five levels out of the crate. PR 0.4 removes that
  dependency; parser behavior uses package-owned fixtures. Each config change renders with fixture
  data and parses the actual template as review evidence in the outer repository. Do not add a new
  declaration-consistency test or require an external template to compile the crate.
- **The record's readers.** The hermes route `unattended-upgrades` receives the body unchanged in
  shape; `state` gains one value. The osquery file-integrity page reads brew's upgrade record by its
  tab-separated row format, so PR E2's move of `tuple_row` is a pure move proved byte for byte.
- **pns as a path dependency.** The failure webhook (PR A4) reuses the `SignedPost` seam uu already
  imports for the record and adds no import.
- **CI (continuous integration).** `just test` on `macos-latest` runs `test-rust` and `test-nvim`;
  the runner has rustup with
  stable and `brew install neovim`, and the Lua specs must pass under `--clean` with no plugin tree.
- **The three LaunchAgents being retired** keep running until their cutover pull request lands; no
  earlier pull request touches them.

## 3. Rules every pull request in the ladder obeys

1. **Kind of work, declared.** New behavior carries a red test first and a mutation check by hand
   (revert the change, watch the named test go red, restore); a pure move carries the block-identity
   and whole-file reconstruction checks from `~/.claude/pipeline/extraction-verify.sh`. No pull
   request is both.
1. **Gates.** At the final commit, run `just test-rust`, `just lint-check`, `just ship`, and
   `just test-nvim` when Lua changes. From uu's workspace root also run
   `cargo fmt --all -- --check`, `cargo check --locked --workspace --all-targets`,
   `cargo clippy --locked --workspace --all-targets -- -D warnings`,
   `cargo test --locked --workspace --no-fail-fast`, and
   `RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --no-deps`. Run the builder's own
   cargo line and every dependent sibling's test command; verify generated files when affected.
   Record exit codes and failures. A failed baseline gate stays failed. Miri is local evidence only.
1. **Arguments.** Every row compares a separately built previous-main binary with the candidate over
   the frozen CLI (command-line interface), checking stdout, stderr and exit codes in isolated homes
   with fake destinations. Prove the differential fails on an intentional argument-outcome mutant,
   verifying the mutant bytes on disk first. List intended surface changes separately. Comparing a
   binary with itself or presenting an untested differential is not evidence.
1. **File size and speed.** After rustfmt, use exactly the physical-line command in the Rust skill:
   targets 200 implementation and 300 total lines, decomposition normally at 250 implementation or
   400 total, no handwritten file above 500 total including tests, and `main.rs` below 150 at
   completion. A test marker above production code is a defect. Tests remain colocated, with private
   child modules split by behavior when needed; moving tests cannot hide implementation ownership.
   Measure every test below one second under workspace concurrency. Sizes below are projections;
   each row reports actual implementation and total lines, with a named later owner for staged
   violations. PR 0.6 closes existing gaps; later rows own any growth they introduce. Lua targets 300.
1. **The registry touch points.** After PR 0.5 these mean the lane's typed parser, concrete adapter,
   composition registration with diagnostics, behavior tests, and shipped config plus its rendered
   parse evidence. Domain and application gain no type-name switches. Ports remain unregistered and
   their config blocks commented until their complete cutover rows, especially E5 through E15.
1. **Neovim evidence.** The pure half of every `lua/uu/` module is pinned by a `uu_*_spec.lua`; the
   half that drives lazy.nvim, Mason or the treesitter installer is proved by one real headless run of
   the exact command the lane composes against copied config and data. Use one-letter copy names,
   isolated HOME and all four base directories, and `GIT_CONFIG_GLOBAL=/dev/null` and
   `GIT_CONFIG_SYSTEM=/dev/null`. Keep every artifact and put commands and output in the body.
   Never drive a live plugin tree; both production smoke verification and experiments copy Mason.
   A fake plugin manager is not written. Fakes for our sequencing and failure decisions are valid.
1. **No live effects.** No pull request runs `uu run` against the real config, bootstraps or boots
   out a real LaunchAgent, or applies. The operator applies, and section 6 says what they do after.
1. **Order.** A row lands after the rows its `Order:` names; a row with none lands in ladder order.
   Step 0 precedes A; step E, the ports, lands last. Each port stays inactive until its cutover.
1. **Repository rules.** Conventional Commits, one logical change per commit, no trailers; no
   em-dashes anywhere; `trash` never `rm`, including a scratch directory the agent made; never
   `chezmoi apply`; never a force push. Every Rust brief explicitly names
   `~/.agents/skills/clean-code/SKILL.md` and `~/.agents/skills/clean-code-rust/SKILL.md`, says Rust
   wins every number and mechanism, and cites the recurring-bug-classes checklist. Two open uu
   branches at most, and every pull
   request gets the independent model review at max reasoning beside the pipeline's own steps.
1. **Deletion is a move.** Retiring a script means grepping the checkout for its name and its
   directory and changing every referrer in the same pull request (the lists in section 5 are the
   starting point, re-grepped at the time). chezmoi does not delete the old target, so every cutover
   row names what the operator removes from `$HOME` by hand.

## 4. The ladder

Each entry: **Adds** or **Moves**, **Tests** (by name; a test named here is written red first),
**Consumer** beyond the standing gates, **Sizes** (projected, tests included), and **Order** where it
is not ladder order.

### Step 0: finish the remaining Rust architecture

PR #358 already split the implementation by responsibility. Preserve those modules and their test
names. These six rows address the remaining boundaries, not a second file-sharding exercise. Every
brief uses both clean-code skills and Rust precedence as required by section 3.

**PR 0.1 workspace and domain ownership.** Moves pure lane-report values, budget decisions and streak
policy into `crates/uu-domain`; parsing and external effects stay outside. Adds a Cargo workspace with
that real member and the transitional root package, preserving the `uu` binary. Domain has no process,
filesystem, environment, serialization or pns dependency. Moves each policy's colocated tests by name;
records the baseline leaf-name/result set and its successor map. Consumer: builder 59 hashes uu member
manifests as well as sources; `just test-rust` selects the whole workspace for test, fmt and clippy;
the committed lockfile follows. Sizes: policy modules at most 200 implementation/300 total, each new
`lib.rs` below 100. Evidence: all members run, the extracted builder cargo line builds `uu`, and the
argument differential with its failing control passes the unmutated candidate.

**PR 0.2 protocol ownership.** Moves existing record envelope serialization and child exit-code
contracts into `crates/uu-protocol`, with their exact byte, escaping and status tests by name. Keep the
existing protocol shape; no new wire version or unused request model. Protocol owns encoding and depends
on neither domain nor application. Domain owns policy; neither imports concrete delivery. Consumer:
record delivery and command lane import the
protocol contract. Sizes: each module at most 200 implementation/300 total; `lib.rs` below 100.

**PR 0.3 application ownership.** Moves sequencing from `cli/run.rs` into `crates/uu-application`:
lane execution, marker/streak decisions and record/alert orchestration over consumer-owned ports for
state, clock, execution and delivery. Port signatures come from those calls; failures remain typed
outcomes rather than booleans or missing values that erase failure causes. Application depends on
domain, never concrete adapters or free-form configuration tables. Move orchestration tests by name
and retain the failure-direction controls for delivery, staleness and marker advancement. Consumer:
the existing command adapter calls the use case. Sizes: each use case and private tests meet the
200/300 targets; narrow port declarations below 100. No concrete construction remains in the use case.

**PR 0.4 adapters and command composition.** Moves process, filesystem, configuration parsing and
validation, clock and transport into `crates/uu-adapters`; pns is an adapter-only dependency. Moves
argument decoding and concrete construction into `crates/uu-cli`. Adapters consume domain, application
ports and protocol; the command crate composes all four. Update every consumer and remove the root
package once the real binary belongs to `uu-cli`, keeping `[[bin]] name = "uu"` and its installed
path. Replace `config/shipped_template.rs`'s external `include_str!` in this row with package-owned
config parser fixtures, so moving the module cannot break its relative include. Remove roster-equality
assertions that test declarations alone. The outer repository renders and parses its actual template
as verification evidence with fixture values and no secret reads. Move adapter and command tests by
leaf name and run the full argument differential. Re-scan
sibling manifests; currently no sibling depends on uu. Sizes: each module targets 200/300; `main.rs`
below 150. Existing modules retain their responsibilities; only their crate ownership changes.

**PR 0.5 lane registration.** Replaces `LaneKind`, `parse_lanes`, `run_lane` and doctor's parallel
type switches with concrete registrations at composition. Each registration binds a type name to its
typed parser, executable adapter and diagnostics; application receives executable lanes without
knowing their technology. Preserve names, defaults and refusal text, updating the existing minimal
block, selected-lane and deadline tests by name to exercise real registrations. Test a registration
with a different display name through parsing, execution and diagnostics; removing its registration
must fail that behavior. Domain and application cannot import adapters under Cargo dependency rules.
Sizes: registration/composition modules target 200/300; the root stays below 150. No blanket trait per
struct, public module tree or central fallback switch remains.

**PR 0.6 package interface and remaining closure.** Curates each `lib.rs` export, defaults modules to
private and places unit tests beside their implementation. Verify that 0.4 removed every fixture path
outside the package. Map every baseline
test to its successor or a named removal reason, retain every behavioral contract and retire temporary
compatibility exports. Sizes: every file is measured with the prescribed command, 200/300 targets,
250/400 decomposition thresholds, 500 total cap and `main.rs` below 150. Close all staged ownership
and size gaps from 0.1 through 0.5 and run every gate, differential and failing control in section 3.

Later rows retain the familiar source names to identify the existing behavior. Resolve them by owner:

| Existing responsibility | Final owner |
| --- | --- |
| lane-report values, budget and streak policy | `uu-domain` |
| record envelope and child exit-code encoding | `uu-protocol` |
| run/bootstrap, record/alert and streak sequencing | `uu-application` |
| config/schema, state codecs, command runner and lane technology | `uu-adapters` |
| arguments, doctor presentation and concrete lane registration | `uu-cli` |

New lane parsing, spawning, storage and plugin interfaces follow the adapter owner; new pure decisions
follow domain. A later row changes the owning crate and its consumers together. No row restores a
central dispatch switch or an external test fixture dependency removed here.

### Step A: the record model

**PR A1 the `pending` lane verdict and record state.** Adds `pending(line)` to `LaneReport`
(`src/lanes/report.rs`) with a typed verdict (`Completed`, `Pending`, `Deferred`, `Failed`) instead
of independently mutable status flags; updates every verdict consumer. `record_state` uses the order
failed, deferred, pending, completed, the `pending` verdict line and the closing count in
`src/record.rs`; the pending count at the call site in `src/cli/run.rs`, where the marker rule stays
`failures == 0 && deferred == 0 && !record_lost` because a pending lane did succeed. Tests:
`a_run_with_nothing_failed_or_deferred_but_something_pending_is_pending`,
`a_deferral_wins_over_a_pending_lane_in_the_same_run`,
`a_pending_lane_is_named_pending_rather_than_zero_failures`,
`the_closing_line_counts_pending_lanes_beside_failures_and_deferrals`, and at the call site
`a_pending_only_run_posts_a_body_stated_pending_not_completed`. Consumer: the hermes route renders
the new `state` like any other. Sizes: `record.rs` 253 becomes about 150 plus `record/tests.rs` about
220; `report.rs` 68 becomes about 90.

**PR A2 exit code 100 means pending for a child.** Adds `PENDING_EXIT_CODE: i32 = 100` and
`Verdict::Pending(String)` in `src/lanes/spawn.rs`, the mapping in `src/runner.rs` beside the one
for 75, the arm in `src/lanes/command.rs` that calls `report.pending`, and the template comment
beside the existing note on 75. Tests:
`a_child_that_exits_the_pending_code_is_recorded_pending_not_failed`,
`a_pending_lines_reason_is_the_childs_own_text_not_a_fixed_string`,
`a_pending_child_keeps_its_stdout_like_a_deferred_one`, and the runner's mapping test beside its 75
sibling. Consumer: every `command` lane; a child exiting 100 read as failed before, so the template
comment is the contract change. Sizes: `command.rs` 238 becomes about 200 plus `command/tests.rs`
about 230; `spawn.rs` stays under 120.

**PR A3 `escalate_after_runs` and the pending streak.** Adds the key beside `deadline_secs` in the
common lane parser (before the registered type parser, so every type carries it),
`Lane.escalate_after: u32`, and
the `escalate_after_runs` entry in every `TABLE_KEYS` lane row; a new `src/escalation.rs` in the
domain holding `DEFAULT_ESCALATE_AFTER_RUNS = 3` and
`next_pending_streak(previous, pending, threshold)` in the trip-once shape of
`staleness::next_streak`; a file-name parameter on `state/streak.rs` so the same reader and writer
keep `pending` beside `streak`; and `src/cli/run/escalation.rs`, mirroring `cli/run/staleness.rs`,
which reads, advances, alerts once through `send_alert` with how many runs the updates have waited,
and holds the count one short when the alarm was not delivered. `parse_escalation` stays in the
adapter's configuration parser; application owns the alert/streak sequencing under the routing table.
Tests:
`a_lane_pending_for_the_threshold_number_of_runs_trips_exactly_once`,
`a_run_with_nothing_pending_resets_the_pending_streak`,
`a_streak_past_the_threshold_keeps_counting_but_never_trips_again`,
`escalate_after_runs_defaults_to_three_on_every_lane_type`,
`a_lane_may_state_its_own_escalation_threshold`,
`an_escalation_threshold_that_is_not_a_positive_integer_is_refused_by_name`,
`the_escalation_alarm_names_how_many_runs_the_updates_have_waited`,
`an_undelivered_escalation_trip_is_retried_never_lost`. Consumer: `prune_removed_lanes` already
removes the whole lane directory, so the new file needs no pruning of its own. Sizes:
`escalation.rs` about 110 plus tests about 160; `cli/run/escalation.rs` about 100; `config/lanes.rs`
246 becomes about 270; `schema.rs` grows by one entry per lane row.

**PR A4 `failure_webhook`.** Adds `Records.failure_webhook: Option<String>` in `src/config.rs`,
`None` when the key is absent and parsed by `non_empty` like every other string key when present, and
the `records` row in `TABLE_KEYS`. The adapter's delivery implementation takes parsed records settings
and pns's `SignedPost`; when enabled it posts `record_body(<alarm kind>, host, detail)` signed with
`records.key`. Application callers use 0.3's delivery port, passing only the alarm kind (`failed`,
`stale`, `pending`, `record-lost`), host and detail. Neither parsed `Records` nor pns types cross into
application. Adapter tests own the `SignedPost` spy. The template gains a commented
`# failure_webhook = "..."` line
under `[records]`, the opt-in shape. Tests:
`a_records_block_without_a_failure_webhook_leaves_alarms_on_pns_alone`,
`a_blank_failure_webhook_is_refused_by_name_like_every_other_blank_string` (one more row in the two
blank-string table tests in `schema.rs`),
`a_failure_webhook_receives_the_alarm_body_signed_with_the_records_key`,
`every_alarm_kind_reaches_the_failure_webhook` (a spy `SignedPost` sees failed, stale, record-lost
and pending), `a_failure_webhook_that_refuses_is_stated_and_never_fails_the_run`. Consumer: the
existing `send_alert` tests keep their names. Sizes: `delivery.rs` 178 becomes about 150 plus
`delivery/tests.rs` about 260; `config.rs` 301 becomes about 200 plus `config/tests.rs` about 180.

### Step B: the toolchain pin

**PR B1 `channel = "stable"` in the four crate roots.** Adds `rust-toolchain.toml` with
`[toolchain] channel = "stable"` at `dot_local/share/pns/`, `dot_local/share/uu/`,
`dot_local/share/herdr/plugins/herdr-smart-nav/` and
`dot_local/share/herdr/plugins/herdr-last-workspace/`, and a fifth at the repository root, because
`just test-rust` runs cargo from there with
`--manifest-path` and rustup reads the toolchain file from the current directory and its parents,
never from the manifest's. Add exactly `rust-toolchain.toml` to `.chezmoiignore`, making the root
pin source-only while the four nested pins deploy. The pns and uu builders (58 and 59) change their
cargo line to run from the crate directory,
`(cd "$crate_dir" && "$cargo_bin" build --release --locked --quiet --bin <name>)`, since a
chezmoiscript's working directory is the home directory where no toolchain file lives. The two herdr
builders already use `(cd "$plugin_dir" && "$cargo_bin" build --release --locked)` in
`.chezmoitemplates/herdr-plugin-build.sh.tmpl`; retain and prove that behavior. Include each pin in
its builder's hashed inputs. The
`test/unit/pns-engine-build-install.sh`'s cargo stand-in (lines 91 to 96) stops reading
`--manifest-path` and asserts the working directory instead. Corrects
`dot_agents/skills/clean-code-rust/SKILL.md:109` to say the crates pin stable and Miri is
`cargo +nightly miri`. A declaration pull request: no new Rust test. Evidence: `rustup show
active-toolchain` inside each crate prints stable; `just test-rust` green; each builder's line run by
hand in a retained copied crate with a fresh target directory produces the binary. Never run a full
builder that installs or links live state. Rendered source membership is inspected to confirm the
root pin is excluded and the nested pins are included, with no declaration test added.
Nothing in the four crates uses `#![feature]`
(checked 2026-09-06). Order: before D3.

### Step C: Neovim

**PR C1 `lua/uu/report.lua`, the pure half.** Adds `dot_config/nvim/lua/uu/report.lua`: the exit
codes (`OK = 0`, `FAILED = 1`, `PENDING = 100`), `plugin_lines(plugins)` from a list of
`{ name, updates, errors }` to lines plus an exit code, `other_instances(sockets, own_pid)`,
`restart_notice(count)`, `health_counts(lines)`, `keymap_rows(maps_by_mode)` and
`keymap_diff(before, after)` keyed by mode and lhs, and `commit_allowed(porcelain, symbolic_ref)` for
PR C3. Tests, in `dot_config/nvim/tests/uu_report_spec.lua`: "a plugin with updates is listed by name
and the run is pending", "a plugin with errors fails the run even when others are current", "a
current plugin is counted and not named", "no other socket means no notice", "two other sockets read
as two instances and never count our own", "a mapping whose rhs changed is neither added nor
removed", "a health report's ERROR and WARNING lines are counted by severity", "a dirty lock file
refuses the commit and names the reason", "a detached HEAD refuses the commit", "a clean lock on a
branch allows it". Consumer: `stylua` and `luacheck` through `just lint-check`; `.luacheckrc`
already declares `vim`. Sizes: about 160 lines of Lua; the spec about 150.

**PR C2 the `nvim-plugins` lane, report only.** Adds `dot_config/nvim/lua/uu/plugins.lua` (the
`lazy.manage.check` call with `wait = true`, the walk over `lazy.core.config.plugins` reading
`_.updates` and `has_errors`, the lines, the exit); `src/config/lanes/nvim.rs` with `NvimHost { nvim,
config }` parsed once for the four types (`nvim` defaults to `nvim` on PATH like `DEFAULT_UV_BINARY`;
`config` is required and absolute) and `NvimPluginsLane { host }`; `src/lanes/nvim.rs` with
`invoke(host, module, args, runner)` composing
`[nvim, --headless, -u, <config>/init.lua, -l, <config>/lua/uu/<module>.lua, args...]` and mapping
the verdict including 100; `src/lanes/nvim/plugins.rs`, the adapter. The registry touch points, the
template block and its rendered parse evidence, the doctor line. Tests:
`the_plugins_lane_runs_nvim_headless_with_the_configs_init_and_the_uu_module`,
`an_nvim_lane_names_its_own_lane_not_its_type`,
`a_plugins_child_exiting_pending_is_a_pending_lane_carrying_its_lines`,
`a_plugins_child_exiting_non_zero_is_a_counted_failure_carrying_its_stderr_tail`,
`an_nvim_lane_without_a_config_key_is_refused_because_nothing_names_the_init`,
`an_nvim_lane_defaults_its_binary_to_nvim_on_the_running_path`,
`an_nvim_lane_config_that_is_not_absolute_is_refused_by_name`. Evidence: the composed command run by
hand against isolated copied config and data, printing the moved pins and exiting 100 or 0. Sizes:
`config/lanes/nvim.rs` about 140 plus tests about 120; `lanes/nvim.rs` about 110 plus
`lanes/nvim/tests.rs` about 160; `lanes/nvim/plugins.rs` about 60 plus tests about 100;
`plugins.lua` about 80.

**PR C3 `auto_commit`.** Adds `auto_commit: bool` (default false) and `repo: Option<String>`
(absolute, required when `auto_commit` is true) to `NvimPluginsLane` and the two keys to its
`TABLE_KEYS` row; the adapter passes `--auto-commit --repo <path>` in `args` when on;
`plugins.lua` reads `_G.arg`, evaluates `report.commit_allowed` over the output of
`git -C <repo> status --porcelain -- dot_config/nvim/lazy-lock.json` and
`git -C <repo> symbolic-ref -q HEAD`, and selects `lazy.manage.update` with `wait = true` instead of
the check when allowed. It follows the spec's durable write-back recovery contract: save starting branch,
commit and lock before mutation, retain the candidate lock after updating, copy only the owned source
lock and commit that exact path with `SKIP_AI_COMMIT=1 GRAPHIFY_SKIP_HOOK=1` and normal hooks. It
rechecks branch, HEAD and source/index lock bytes against the starting identity immediately before
copying, preserving any intervening edit. It checks committed/deployed/installed lock-managed revisions
before closing recovery. An unchanged candidate completes as a verified no-op without attempting an
empty commit. Any update, copy,
hook or commit failure is failed; recovery remains until operator reconciliation is observed. While
recovery is open, even report-only runs remain failed and no further update starts. Refused preflight
without open recovery runs the check, prints its reason and exits as C2 does. Never reset the index or
overwrite later operator edits. Tests cover our decisions and durable recovery using owned effects:
Tests: `a_plugins_lane_with_auto_commit_on_hands_the_module_the_repo`,
`a_plugins_lane_with_auto_commit_on_and_no_repo_is_refused_by_name`,
`a_plugins_lane_with_auto_commit_off_passes_no_commit_flag`,
`auto_commit_that_is_not_a_boolean_is_refused_naming_what_was_written`,
`a_failed_recovery_record_write_prevents_any_plugin_update`,
`preflight_lock_or_installed_revision_disagreement_falls_back_to_report_only`,
`an_unchanged_candidate_closes_recovery_without_an_empty_commit`,
`update_copy_and_commit_failures_retain_recovery_and_never_report_completion`,
`an_open_recovery_blocks_updates_even_after_auto_commit_is_disabled`,
`reconciliation_requires_clean_committed_deployed_and_installed_pins_to_agree`,
`a_source_edit_or_branch_change_during_update_is_preserved_and_leaves_recovery_open`,
`a_rejected_hook_preserves_unrelated_staged_paths_and_the_candidate_lock`.
The three `commit_allowed` cases are in C1. Evidence: isolated copied config/data and a scratch local
repository, a successful normal-hook commit and a rejecting hook, with retained lock bytes and recovery
state. No live updates or pushes. Sizes: `plugins.lua` about 150, private `writeback.lua` about 180,
their behavior specs about 250; `config/lanes/nvim.rs` grows by about 40.

**PR C4 the restart notice.** Adds `report.running_sockets()`, the one impure line that globs
`<parent of stdpath("run")>/*/nvim.*.0`, and the call in `plugins.lua`'s update path that prints
`restart_notice(other_instances(...))` when the count is above zero. Tests, in `uu_report_spec.lua`:
"the run directory's parent is the per-user socket root" (from a sample `stdpath("run")` string).
Evidence: a real run with one other Neovim open shows the notice line, and one with none shows no
line. Sizes: about 30 lines of Lua.

**PR C5 the `nvim-mason` lane.** Adds `dot_config/nvim/lua/uu/mason.lua` (`MasonUpdate`, then the
`install:success` and `install:failed` handlers on every package before `MasonToolsUpdateSync`,
`MasonToolsUpdateCompleted` only as the finish signal, the servers sentence on every run, the restart
notice, exit 1 on any failed package); `NvimMasonLane { host }` in `config/lanes/nvim.rs` and
`src/lanes/nvim/mason.rs`; the registry touch points, the template block and its rendered parse evidence.
The
uu plist's PATH gains `~/.local/share/fnm/aliases/default/bin` and `~/.cargo/bin`, with the plist
comment saying which Mason packages need them. Tests: Rust
`the_mason_lane_runs_the_mason_module_headless_under_the_lanes_own_name`,
`a_mason_child_exiting_non_zero_is_a_counted_failure`; Lua, in `uu_mason_spec.lua` over a pure
`mason_lines(events)`: "a package that fired install:failed is a failure although the completion list
names it", "a package that fired install:success is listed as updated", "the servers sentence is
present on a clean run". Consumer: `test/unit/uu-launchagent-loader.bats` still passes; the loader
re-fires on the plist hash. Evidence: a real run. Sizes: `mason.lua` about 120; `lanes/nvim/mason.rs`
about 50 plus tests about 80.

**PR C6 the `nvim-parsers` lane.** Adds `dot_config/nvim/lua/uu/parsers.lua`
(`assert(require("nvim-treesitter.install").update(nil, { summary = true }):wait())`, the summary
lines, the restart notice); `NvimParsersLane { host }` and `src/lanes/nvim/parsers.rs`; the registry
touch points, the template block and its rendered parse evidence. Tests: Rust
`the_parsers_lane_runs_the_parsers_module_headless_under_the_lanes_own_name`,
`a_parsers_child_exiting_non_zero_is_a_counted_failure_carrying_the_compiler_tail`; Lua, in
`uu_parsers_spec.lua` over a pure `parser_lines(summary)`: "a false wait result is a failure", "a
true result lists updated and current parsers". Evidence: a real run, with its wall-clock time in the
pull request body, which decides whether the lane ships its own `deadline_secs` below six hours.
Sizes: `parsers.lua` about 80; `lanes/nvim/parsers.rs` about 50 plus tests about 80.

**PR C7 the `nvim-smoke-test` lane: prepare then verify in a fresh process.** Adds
`NvimSmokeTestLane { host, cache }` (`cache` absolute, required) and `src/lanes/nvim/smoke_test.rs`
(copy `<config>` to `<cache>/c/nvim`, data/state/cache roots `d`, `s`, `k`, and a copy of Mason's
tree at `d/nvim/mason`, for production and experiments). It runs the two exact argv forms in the spec:
prepare with `-l .../smoke_test.lua prepare`, then a separate process with
`-c "lua dofile('<copy>/lua/uu/smoke_test.lua')"` and the same `-u` and isolated roots.
`smoke_test.lua` prepares with `lazy.manage.update({ wait = true, show = false })` and rejects task
errors. Its verifier registers a self-quitting `VimEnter` callback, fires `User VeryLazy`, drains
startup work within the deadline and captures errors including Snacks and Noice history, then runs
`silent checkhealth`, never `silent!`; progress echoes must not pollute the startup stderr gate.
It emits a completion record identifying the exact candidate lock; no completion, error history,
unreadable loaded-notifier history, stderr, non-zero exit or timeout fails the lane. Health severity
counts stay separate from startup failure detection. Write health to `<cache>/checkhealth.txt`.
The registry touch points include an opt-in commented block with the disk-cost comment; the operator
enables it in their config. An early `vim.notify` wrapper or `has_errors` alone is never sufficient.
Tests: Rust `the_smoke_test_runs_nvim_through_env_with_the_four_base_directories_redirected`,
`the_smoke_test_copies_config_and_mason_without_linking_live_state`,
`candidate_verification_starts_a_new_process_after_prepare_succeeds`,
`a_failed_prepare_never_launches_the_verifier`,
`a_missing_completion_or_unreadable_loaded_notifier_history_fails_verification`,
`a_smoke_test_child_exiting_non_zero_is_a_counted_failure`,
`a_smoke_test_lane_without_a_cache_key_is_refused_because_nothing_names_the_tree`; Lua, "health
lines are counted by severity" is C1's. Evidence: retained offline healthy and failing copies with
an init error, an owned plugin config error, and errors after Snacks and Noice load. Each fails even
at exit 0 with `has_errors=false`; the notifier controls also have zero stderr. An owned module changed
on disk stays old in the prepare process and loads new in the verifier. Record actual `VimEnter`,
completion, diagnostics and `du -sh <cache>` against the roughly 2.5 gigabyte estimate. Retain a control
showing silent health progress preserves health errors in the buffer with empty stderr. Sizes:
`lanes/nvim/smoke_test.rs` about 170 plus private tests about 200; `smoke_test.lua` about 180 and
private startup-diagnostic code about 120. All new diagnostic decisions have named pure controls.

**PR C8 the keymap dump and its diff.** Adds to the fresh verifier in `smoke_test.lua`: `keymap_rows`
over
`nvim_get_keymap(mode)` for every mode letter, written to `<cache>/keymaps.tsv`, the diff against the
previous file through `report.keymap_diff`, and the added and removed counts in the record. Tests, in
`uu_report_spec.lua`: "the first dump has nothing to diff against and says so"; the diff cases are PR
C1's. Evidence: two real runs, the second showing the diff line. Sizes: about 40 lines of Lua.

### Step D: cargo and rustup

**PR D1 the `cargo` lane, report.** Adds `src/config/lanes/cargo.rs` (`CargoLane { cargo, compile:
false }`, `cargo` absolute), `src/lanes/cargo.rs` (the adapter: the listing, one `cargo search
<crate> --limit 1` per registry crate, the sentence, pending when anything is behind) and
`src/lanes/cargo/listing.rs` (`parse_install_list(stdout)` to `Installed { name, version, source:
Registry | Git { rev }, binaries }`, `parse_search(stdout, crate)`, `behind_sentence(installed,
newest)` producing `fd has a new version: 8.4.0 → 10.5.0. Run the following command to compile it:
cargo install fd-find`); the registry touch points, the template block and its rendered parse evidence.
Tests:
`the_install_list_is_read_as_crates_with_their_versions_sources_and_binaries`,
`a_git_sourced_crate_is_skipped_and_its_rev_is_named`,
`the_search_line_for_the_crate_itself_is_the_newest_version_and_a_neighbour_is_not`,
`a_crate_behind_the_registry_produces_the_operators_sentence_naming_its_one_binary`,
`a_crate_with_several_binaries_is_named_by_its_crate`,
`a_crate_that_is_behind_makes_the_lane_pending_when_compile_is_off`,
`a_crate_that_is_current_is_one_recorded_line`,
`a_search_that_fails_is_a_failure_naming_the_crate_and_the_rest_still_run`. Sizes: `listing.rs` about
150 plus `listing/tests.rs` about 200; `cargo.rs` about 120 plus tests about 160;
`config/lanes/cargo.rs` about 70 plus tests about 60.

**PR D2 `compile = true`.** Adds the arm in `src/lanes/cargo.rs`: `cargo install <crate>` once per
crate that is behind, the exit code per crate, the completed verdict when every build passed. Tests:
`with_compile_on_a_crate_behind_is_installed_by_name_and_recorded_with_both_versions`,
`one_failed_build_does_not_stop_the_next_crate`,
`a_compiled_run_is_completed_rather_than_pending`,
`compile_that_is_not_a_boolean_is_refused_naming_what_was_written`. Sizes: `cargo.rs` grows by about
60; split its tests into `cargo/tests.rs` if it passes 300.

**PR D3 the `rustup` lane.** Adds `src/config/lanes/rustup.rs` (`RustupLane { rustup }`, absolute),
`src/lanes/rustup.rs` (`rustup update`, `parse_update_summary(stdout)` to `Toolchain { name,
outcome: Updated { from, to } | Unchanged(version) }`, one line each); the registry touch points, the
template block and its rendered parse evidence. Tests: `an_updated_toolchain_is_recorded_from_and_to`,
`an_unchanged_toolchain_is_recorded_as_current`, `rustups_own_update_line_is_recorded`,
`a_rustup_that_fails_is_a_failure_carrying_its_stderr_tail`,
`a_summary_line_of_another_shape_is_skipped_rather_than_half_read`. Order: after B1. Sizes:
`rustup.rs` about 120 plus tests about 150; `config/lanes/rustup.rs` about 60 plus tests about 50.

### Step E: the ports, last

**PR E1 `uu bootstrap <lane>`.** Adds the `["bootstrap", lane]` argument and application use case
(load the config, take the run lock, invoke the registered bootstrap capability, print the report's
lines, exit 0 or 1; no record, no marker, no streak). Registrations offer that capability only for
types that implement it; an absent capability is refused with a sentence naming the type. Update the
usage text. Tests: `bootstrap_of_a_lane_whose_type_has_no_bootstrap_step_is_refused_naming_the_type`,
`bootstrap_of_an_undeclared_lane_is_refused_like_a_run_of_one`,
`bootstrap_posts_no_record_and_leaves_the_marker_and_streaks_alone` (in `tests/run.rs` with the
`tests/support` sandbox), `usage_lists_bootstrap_beside_run_doctor_and_schedule`. Sizes:
`cli/bootstrap.rs` about 90; `main.rs` grows by about 8.

**PR E2 the shared change section, a pure move.** Moves `src/lanes/brew/changes.rs` to
`src/lanes/changes.rs` whole, and `change_section`, `change_line`, `code` and `NAME_CAP` from
`src/lanes/brew/sections.rs` to `src/lanes/changes/section.rs`; the brew lane's imports follow; every
test moves by name. `tuple_row`'s bytes are the osquery file-integrity page's contract and the
reconstruction check proves them unchanged. Sizes: unchanged line counts at new paths.

**PR E3 the `claude-plugins` lane.** Adds `src/config/lanes/claude_plugins.rs`
(`ClaudePluginsLane { inventory }`, absolute), `src/lanes/claude_plugins/inventory.rs`
(`read_inventory(text) -> Result<Listing, String>`: one document through `serde_json::from_str`, the
shape checks in the bash job's order, user scope only, the fingerprint rule version then
`gitCommitSha` then `unknown`), `src/lanes/claude_plugins.rs` (the adapter: read, compare with
`~/.local/state/uu/lanes/claude-plugins/snapshot.tsv` through `changes::section` with the bash
job's caveat, write the snapshot through a sibling temp file and rename, the baseline notice on a
first reading). Both run and bootstrap first keep an existing uu snapshot, else import the validated
legacy `~/.local/state/report-plugin-updates/installed-plugins.snapshot` atomically, else seed only
if neither file exists. Import accepts a valid empty snapshot, preserves its bytes and leaves the
legacy file alone. Bootstrap never compares or advances imported history. A failed read, validation
or write fails without reseeding. Registration and the active block wait for E4. Tests:
`an_inventory_holding_two_documents_is_refused_not_read_twice`,
`an_inventory_with_no_plugins_object_or_an_empty_one_is_refused_rather_than_a_quiet_week`,
`a_record_without_a_scope_string_refuses_the_whole_reading_rather_than_reading_as_removed`,
`only_user_scope_records_are_read`, `the_fingerprint_is_version_then_commit_then_unknown`,
`the_first_reading_is_a_baseline_that_compares_nothing`,
`a_second_reading_is_compared_and_the_snapshot_moves`,
`an_unreadable_inventory_fails_the_lane_and_leaves_the_snapshot_alone`,
`bootstrap_seeds_a_baseline_once_and_leaves_an_existing_one_alone`,
`an_imported_legacy_snapshot_reports_changes_since_the_last_delivered_bash_record`,
`an_existing_uu_snapshot_wins_over_a_legacy_snapshot`,
`an_empty_legacy_snapshot_is_imported_and_new_user_plugins_are_reported_added`,
`a_failed_legacy_import_preserves_both_states_and_never_seeds_fresh`,
`repeated_bootstrap_never_consumes_the_imported_comparison`. Sizes: `inventory.rs` about 120
plus `inventory/tests.rs` about 220; `claude_plugins.rs` about 150 plus tests about 170;
`config/lanes/claude_plugins.rs` about 60 plus tests about 50.

**PR E4 the `claude-plugins` cutover.** Registers and enables E3's complete lane, importing the last
delivered snapshot before any fresh seed. Deletes
`dot_local/libexec/unattended-upgrades/claude/executable_report-plugin-updates.sh` and
`Library/LaunchAgents/com.webdavis.report-plugin-updates.plist.tmpl`. Renames
`.chezmoiscripts/run_onchange_after_69-load-report-plugin-updates-launchagent.sh.tmpl` to
`run_onchange_after_69-seed-claude-plugins-baseline.sh.tmpl`: it keeps the ordering paragraph (the
same apply enables marketplace auto-updates), hashes the uu builder's retry marker the way loader 71
does, runs `"$HOME/.local/libexec/uu/uu" bootstrap claude-plugins || printf ...` and never exits
non-zero, and never bootstraps a LaunchAgent. Changes every referrer in section 5. Deletes the
plugin-record cases
from `test/unit/pns-weekly-engine-resolution.sh` (the file itself goes in PR E17). Order: after E3.
Operator steps in section 6.

**PR E5 the `skills` lane: config and the roster gate.** Adds `src/config/lanes/skills.rs` (the
eleven keys of the spec's block, every path absolute) and `src/lanes/skills/roster.rs` (parse
`custom-skill-lock.json` version 2 into typed tables, `tracked_names()` as the union of `npxTracked`
and `clawhubTracked`, the schema check, the zero-union refusal, the hash taken at run start and
`unchanged()` before publish, and the disjointness of `hermesRegistry` and non-empty
`hermesProfiles`). This row adds callable, tested components only. No `run` or `bootstrap` registration
or active block ships until E16; a manually supplied skills type remains unavailable until then.
Document the final block commented, including `escalate_after_runs = 3`; E16 activates it. Tests:
`a_lock_that_is_missing_unparseable_or_schema_broken_refuses_the_run_by_name`,
`a_roster_tracking_zero_skills_is_refused_as_corruption_not_intent`,
`the_tracked_set_is_the_union_of_the_npx_and_clawhub_tables`,
`a_roster_that_changed_mid_run_refuses_the_publish`,
`a_skill_in_both_hermes_registry_and_a_non_empty_hermes_profiles_row_is_refused`. Sizes:
`roster.rs` about 200 plus `roster/tests.rs` about 250; `config/lanes/skills.rs` about 140 plus
tests about 120.

**PR E6 the generation model.** Adds `src/lanes/skills/generation.rs`: the paths
(`.skills-current`, `.skills-generations/<id>/home`, `generation.json` carrying id, createdAt,
`customLockHash`, updater content hash and typed `buildMode` full/additive). Capture a digest of the
running executable once before work, failing before mutation if it cannot be read; a package version
cannot identify the updater. Recovery requires complete metadata, directory/id agreement, current
roster and captured updater hashes, and full mode for weekly reuse. Missing or incompatible metadata
is retained but not reused. `exchange(a, b)` uses
`libc::renameatx_np(AT_FDCWD, a, AT_FDCWD, b, RENAME_SWAP)` in the smallest adapter module with its
safety invariant documented, `is_complete`, the retention of exactly one previous generation, the
garbage sweep, the in-flight marker and `recover()`. Tests:
`an_exchange_swaps_two_directories_atomically_and_both_paths_keep_resolving`,
`a_candidate_without_its_ready_marker_is_incomplete_and_never_published`,
`exactly_one_previous_generation_is_retained_and_older_ones_are_swept`,
`a_recovered_complete_candidate_built_from_the_current_roster_is_reused`,
`an_additive_bootstrap_candidate_is_never_reused_as_a_full_weekly_refresh`,
`different_updater_content_at_the_same_package_version_refuses_candidate_reuse`,
`replacement_after_startup_does_not_change_the_captured_updater_identity`,
`a_generation_whose_metadata_id_disagrees_with_its_directory_is_refused`,
`an_in_flight_exchange_marker_is_finished_on_the_next_run`,
`failed_prior_generation_retention_keeps_the_marker_and_workspace_out_of_the_sweep`,
`recovery_keeps_outgoing_ownership_evidence_until_delisted_pruning_finishes`.
E10 needs the outgoing skill names captured before exchange, including on replay after exchange.
Retain that evidence with the interrupted publication until E10 finishes pruning; sweeping a retained
generation must not erase the only ownership evidence. The bash snapshot `GEN_PREV_OWNED_NAMES` is
process-local, so copying that variable alone would lose ownership on restart.
Sizes: `generation.rs` about 220 plus
`generation/tests.rs` about 280; `generation/exchange.rs` about 60.

**PR E7 the npx lane.** Adds `src/lanes/skills/npx.rs`: one `npx --yes skills@<version> add <repo>
--skill <name> ... --agent claude-code --agent codex -g -y` per repo group against the candidate, run
with `Command::env_clear`, then explicit candidate HOME, base directories, temporary directory and
npm cache, plus PATH captured before HOME changes: the real fnm default-alias bin first, then
`/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin`. No unrelated environment is inherited. Both npx
and clawhub use `#!/usr/bin/env node`, so preserve this interpreter PATH in E8 too. Failed skills are
recorded and the rest proceed; the candidate's
npx lock read as one document and reconciled. Tests:
`the_npx_command_is_run_once_per_repo_group_with_every_skill_of_that_group`,
`the_child_environment_keeps_candidate_roots_and_only_the_explicit_interpreter_path`,
`an_env_node_child_starts_with_the_explicit_path_and_fails_without_it`,
`a_skill_the_cli_failed_is_named_and_the_others_proceed`,
`a_candidate_lock_holding_two_documents_is_refused`. Consumer: `CommandRunner` gains
`run_in(program, args, env)` for a cleared environment, with the `ScriptedRunner` recording it.
The interpreter test starts a harmless owned executable with the installers' real shebang, checks its
isolated HOME/cache, and removes PATH as the failing control. It never invokes an installer or network.
Sizes: `npx.rs` about 180 plus tests about 200.

**PR E8 the clawhub lane.** Adds `src/lanes/skills/clawhub.rs`: an absent skill installed in a
throwaway `--workdir` and its nested `@owner/<name>` output moved flat with `origin.json`; a present
one refreshed in place by bare name; the refusal ladder when the CLI refuses over the repository's
own overlay file (strip, update, re-assert). Tests:
`an_absent_clawhub_skill_is_installed_in_a_throwaway_workdir_and_moved_flat`,
`a_present_clawhub_skill_is_refreshed_in_place_by_bare_name`,
`the_cli_refusing_over_our_own_overlay_is_retried_with_the_overlay_stripped`. Sizes: about 170
plus tests about 180.

**PR E9 Codex overlays and candidate validation.** Adds `src/lanes/skills/overlay.rs` (the
`agents/openai.yaml` policy asserted into every on-demand skill directory, stripped from a core one)
and `src/lanes/skills/validate.rs` (every tracked name present with a `SKILL.md`, the lock one
document, every overlay right; a failure discards the whole candidate). A full candidate's npx lock
keys equal `npxTracked`; additive candidates may retain delisted keys until the next weekly refresh.
Tests:
`an_on_demand_skill_gets_the_codex_overlay_and_a_core_one_does_not`,
`a_candidate_missing_a_tracked_skill_or_its_skill_md_fails_validation_and_is_discarded`,
`a_candidate_whose_overlays_drifted_fails_validation`,
`a_full_candidate_with_a_delisted_npx_lock_key_is_refused_but_additive_preserves_it`.
Mutant: remove `__gen_validate_candidate`'s full-mode exact-key check, leaving single-document
validation intact. Sizes: two files about 120 plus tests about
150 each.

**PR E10 publish and the store links.** Adds `src/lanes/skills/publish.rs`: the exchange, the
store symlinks `~/.agents/skills/<name>` to `../.skills-current/skills/<name>`, the npx lock link,
and full-run pruning based on E6's outgoing ownership evidence. Remove an exact managed symlink when
its name is delisted; quarantine a delisted real directory only when its name belonged to the outgoing
generation. Preserve foreign real directories, foreign/app-owned symlinks and still-tracked names.
For tracked real directories, preserve reconciliation's separate rule: replace one only after recovery
records it and the candidate absorbs its content into the published generation. Report an unseen
competing writer and leave it.
Both fresh and reused weekly candidates reconcile and prune before E11 computes delivery sets.
Interrupted publication replays pruning with the retained outgoing names before releasing their
evidence to the sweep. Additive publication never prunes or replaces an existing store entry. Tests:
`a_published_candidate_becomes_current_and_every_store_link_resolves_into_it`,
`a_delisted_managed_store_link_is_removed_without_following_foreign_symlinks`,
`a_delisted_outgoing_owned_real_directory_leaves_the_store`,
`a_foreign_real_directory_survives_delisted_pruning`,
`a_tracked_real_directory_is_replaced_only_after_its_content_is_absorbed`,
`recovery_after_exchange_prunes_with_outgoing_names_before_sweeping`,
`additive_publication_preserves_existing_store_entries`,
`the_npx_lock_link_points_into_the_current_generation`.
Independent source-derived mutants in `__gen_prune_delisted_store_links`: skip its owned-real-directory
branch; remove only its `__gen_name_was_generation_owned` guard; remove only its exact managed-link
guard. Each must fail its own control while the unmutated control passes. Also drop the outgoing-name
snapshot in `__gen_publish`; replay must retain that identity across restart. In
`__gen_reconcile_store_links`, bypass the reabsorbed-name check and, separately, its additive early
return. Sizes: about 180 implementation plus private tests about 280, split by behavior at thresholds.

**PR E11 the fan-out.** Adds `src/lanes/skills/fanout.rs`: Claude gets every surviving store skill
except a `claudeDelivery` `"none"` row; hermes gets exactly the profiles each `hermesProfiles` row names
(`default` is `~/.hermes/skills`, any other name `~/.hermes/profiles/<name>/skills`), the two
collision names never. Both weekly and additive fan-out create a missing destination with its parents
before creating links. Check the profile parent and its `skills` child independently for symlinks
before creating directories or touching links; refuse either symlink without traversing it. For default,
check `~/.hermes` and `~/.hermes/skills`; for named profiles, check `~/.hermes/profiles/<name>` and its
`skills` child. Visit mapped profiles plus existing destination directories, including demapped
profiles. Full convergence repairs incorrect owned links and removes stale owned links, including in a
demapped profile; additive convergence leaves existing entries alone. Both preserve foreign links and
real destination entries.
Derive eligibility from the post-prune store, not just the tracked roster: a preserved foreign real
skill directory can still reach Claude and, when mapped and non-colliding, Hermes. A removed owned
delisted directory reaches neither, and full convergence removes its stale managed delivery links.
Tests: `every_store_skill_reaches_claude_unless_delivery_is_none`,
`a_hermes_profile_row_plants_exactly_the_listed_profiles_and_an_empty_row_plants_none`,
`a_collision_name_is_never_fanned_out_to_hermes`,
`owned_delisted_content_leaves_both_delivery_sets_while_foreign_content_remains_eligible`,
`missing_hermes_profile_and_skills_directories_are_created_before_link_delivery`,
`a_profile_parent_that_is_a_symlink_is_refused`,
`a_profile_skills_dir_that_is_a_symlink_is_refused`,
`weekly_fanout_repairs_owned_links_and_prunes_them_in_demapped_profiles`,
`foreign_links_and_real_destination_entries_survive_both_fanout_modes`.
In `converge_dir`, replace only `mkdir -p "$dir"` with a missing-directory early return for the
creation mutant. In `__update_skills_hermes_dir_safe`, remove only the parent guard, then only the
child guard in a separate mutant. Each symlink control leaves the other path ordinary, exercises both
default and named profiles, and asserts an outside sentinel and link set remain untouched. Creation
controls assert real directories and resolving links, not just success. For demapped cleanup, remove
only existing destinations from `__update_skills_hermes_profile_universe`; a mapping-only walk must
fail the stale-link control. Sizes: about 180 implementation plus private tests about 280; split by
behavior at the thresholds.

**PR E12 the hermes registry phase.** Adds `src/lanes/skills/hermes.rs`: `hermes -p <profile>
skills update <lockKey>` per `hermesRegistry` entry and profile, `held` respected, `Blocked` in the
output at exit 0 read as a failure. Tests:
`every_registry_entry_is_updated_in_each_of_its_profiles_by_lock_key`,
`a_held_entry_is_skipped_and_said_so`, `blocked_output_at_exit_zero_is_a_failure_not_a_success`.
Sizes: about 100 plus tests about 130.

**PR E13 the fork drift check.** Adds `src/lanes/skills/forks.rs`: a clone into a temp directory
of the lane's own, `git rev-parse HEAD:<skillPath>` (or `HEAD^{tree}` for `.`) against
`lastComparedTreeHash`, the eight states the bash job named, drift as a pending line, the temp
directory removed with `std::fs::remove_dir_all` whatever happened (the lane made it, and a Rust
process has no `trash`). Tests: `a_fork_whose_upstream_tree_hash_moved_is_pending_with_both_hashes`,
`an_unreachable_upstream_is_named_and_compared_never`,
`a_recorded_path_the_upstream_no_longer_has_is_its_own_state`,
`the_temp_clone_is_removed_whatever_the_outcome`. Sizes: about 170 plus tests about 200.

**PR E14 the app-owned pack and the routing assertion.** Adds the two commands to the adapter:
`cua-driver skills update` and the `routing` script, each with its output in the record and a
non-zero exit counted. Tests: `a_failed_pack_refresh_is_a_counted_failure_naming_the_app`,
`a_routing_assertion_that_fails_is_a_counted_failure_carrying_its_output`. Sizes: `skills.rs` grows
by about 40.

**PR E15a complete weekly skills execution.** Composes E5 through E14 in source order: recovery,
flat-store migration and recovery again, before-fingerprints, full candidate build, overlay validation,
publish, app-pack refresh, fan-out, live overlay checks, routing, hermes registry, fork reporting,
after-fingerprints and the capped change section. A failed candidate prevents publication; the later
live phases still run against the unchanged generation and their failures aggregate. Tests:
`a_weekly_skills_run_executes_every_required_phase_and_reports_its_outcome`,
`a_flat_store_is_migrated_and_recovered_before_weekly_candidate_build`,
`a_failed_candidate_leaves_publication_untouched_but_still_runs_live_followup_phases`,
`weekly_before_and_after_fingerprints_produce_the_skills_change_section`.
`weekly_skills_execution_creates_missing_hermes_destinations` exercises the complete path through E11.
Its mutant omits only the weekly `converge_hermes_skills` call; missing links must fail the control.
Use fake effects to remove each phase in turn and prove the whole-run behavior test fails. Add separate
fresh-build and reused-candidate controls:
`fresh_weekly_publication_prunes_owned_delisted_content_before_delivery`,
`reused_weekly_publication_prunes_owned_delisted_content_before_delivery`.
Each starts with owned and foreign real directories plus stale managed delivery links, then asserts
store contents and both harnesses' resulting links. Remove each of `__gen_weekly_attempt`'s two
`__gen_prune_delisted_store_links` calls independently; skipping either must fail its matching control.
Registration waits for E16. Order: after E5 through E14. Sizes: orchestration about 150 implementation,
private tests about 250; migration about 150 implementation, private tests about 200; split at the stated
thresholds.

**PR E15 skills bootstrap.** Adds `src/lanes/skills/bootstrap.rs`: per roster skill
the health reading (`absent`, `link`, `skillmd`, `lock`, `overlay`), a forced reinstall for the first
four, a rebuild for the fifth, additive publish, a fan-out that creates missing links only, live overlay
checks and routing. Bootstrap stays additive and never migrates a flat store or refreshes healthy
skills. A healthy store skips publication only: additive fan-out still creates missing Hermes profile
and skills directories under E11's parent/child safety checks, then fills absent links. The same fan-out
runs after an additive publish. It retains delisted generation names and npx lock keys, never prunes
the store, and leaves existing delivery entries alone; E9's exact lock-key rule applies only to full
candidates. It propagates required phase failures and remains unregistered until E16.
Tests:
`a_healthy_store_bootstraps_as_a_no_op_that_publishes_nothing`,
`an_absent_roster_skill_is_installed_and_published`,
`a_link_that_resolves_to_a_skill_without_its_skill_md_is_reinstalled`,
`bootstrap_never_updates_a_present_and_healthy_skill`,
`bootstrap_never_migrates_a_flat_store`,
`bootstrap_preserves_delisted_generation_lock_store_and_delivery_entries`,
`healthy_bootstrap_creates_missing_hermes_destinations_without_publishing`,
`additive_publish_creates_missing_hermes_destinations_without_replacing_existing_links`,
`bootstrap_returns_failure_when_a_required_post_publish_check_fails`.
Source controls come from `__gen_build_candidate`'s additive copy and `converge_dir`'s
`INSTALL_ONLY` guards; filter that copy to tracked names, remove the wrong-target guard, and remove
the stale-link guard as separate mutants. For missing destinations, omit only the install-only
`converge_hermes_skills` call. Separately change only `return 0` to `exit 0` in
`__gen_install_only_attempt`'s `needs_work == 0` branch, skipping outer fan-out on a healthy store.
Each must fail the healthy-bootstrap control. Repeat E11's independent parent and child symlink
controls through bootstrap, including when publication is skipped.
Order: after E1 and E15a. Sizes: bootstrap about 180 implementation plus private tests about 220.
Re-decompose by responsibility at the thresholds.

**PR E16 the skills cutover.** First requires the operator's pre-apply stop in section 6; registers
and enables the complete weekly and bootstrap paths, with `escalate_after_runs = 3` explicit. Deletes
`dot_local/libexec/unattended-upgrades/agent-skills/executable_update-skills.sh`,
`Library/LaunchAgents/com.webdavis.update-skills.plist.tmpl` and
`.chezmoiscripts/run_onchange_after_63-load-update-skills-launchagent.sh.tmpl`. Rewrites
`run_onchange_after_64-update-skills-first-install.sh.tmpl` to run
`"$HOME/.local/libexec/uu/uu" bootstrap skills` with its existing
`~/.local/state/skills/first-install-pending` retry marker (the deferred branch folds into the failed
one, since uu's lock refusal is exit 1). Failure or lock refusal retains and advances the marker;
only successful bootstrap clears it. The script hashes the uu builder's retry marker instead of
the updater's source. The justfile `update-skills` recipe becomes
`~/.local/libexec/uu/uu run skills` with a comment that `uu bootstrap skills` is the old
`--install-only`. Deletes `test/unit/update-skills-change-report.sh`,
`test/unit/update-skills-json-stream-reads.sh` and `test/unit/update-skills-lock-symlink.sh`, whose
behaviors PRs E5 to E10 carry by name. Changes every referrer in section 5. Verify the owned loader's
retry branches using a failing, contended and successful bootstrap stand-in, retained render evidence
and marker contents. Do not add declaration meta-tests. Order: after E15 and its full phase controls.

**PR E17 `log-entries.sh` retired.** Deletes
`dot_local/libexec/unattended-upgrades/helpers/log-entries.sh` and
`test/unit/pns-weekly-engine-resolution.sh`; points the comment in
`.chezmoiscripts/run_after_68-hermes-log-route-status.sh.tmpl` at uu's `DEFAULT_RECORD_URL`; removes
the two rows for it from `dot_local/share/pns/docs/specs/legacy-producer-flags.md` and its mention in
`unpinned-behaviors.md`; rewrites rule 4 of "Where deployed scripts live" in `CLAUDE.md`, which uses
this file as its example. Order: after E4 and E16.

**PR E18 the `rotate-logs` lane.** Adds `src/config/lanes/rotate_logs.rs` (`logs` a non-empty
list of absolute paths, `rotate_at_bytes` positive, `archives_kept` at least 1, `compressor`
absolute), `src/lanes/rotate_logs.rs` (the adapter) and `src/lanes/rotate_logs/rotation.rs` (the
predicates and the sequence: prune both sides of the window, shift oldest first, compress to
`.partial` and rename to `.1`, truncate in place). Tests, the four bash tests' behaviors ported by
name: `a_file_of_exactly_the_threshold_rotates_and_one_byte_under_does_not`,
`an_oversized_log_is_archived_and_truncated_in_place_keeping_its_inode`,
`a_symlink_is_never_truncated_through`,
`an_unwritable_log_over_the_threshold_is_a_failure_naming_it`,
`an_archive_of_our_own_is_never_re_archived`,
`retention_keeps_exactly_the_window_pruning_index_zero_and_a_lowered_keep_counts_strays`,
`a_compress_that_fails_leaves_the_log_untruncated_and_no_partial_behind`,
`archives_kept_below_one_is_refused_because_it_would_discard_content_outright`,
`a_log_the_list_does_not_name_is_never_touched`. Registration waits for E19; until then the template
block with its thirteen paths stays commented.
Retain its rendered parse evidence. Sizes: `rotation.rs` about 200 plus `rotation/tests.rs` about 280;
`rotate_logs.rs` about 100 plus tests about 120; `config/lanes/rotate_logs.rs` about 110 plus tests
about 100.

**PR E19 the rotation cutover.** Registers and enables E18 after the operator stops the old job.
Deletes
`dot_local/libexec/executable_compress-and-truncate-local-logs.sh`,
`Library/LaunchAgents/com.webdavis.rotate-logs.plist.tmpl`,
`.chezmoiscripts/run_onchange_after_67-load-rotate-logs-launchagent.sh.tmpl`, the
`.local/libexec/compress-and-truncate-local-logs.sh` line in `.chezmoiignore`'s Linux block, and
`test/unit/rotate-logs-{retention,rotation,skips-unmanageable,threshold-predicate}.sh`. Rewrites
the two `CLAUDE.md` sentences that use the script as their flat-leaf and verb-first example (lines
445 and 464) and its LaunchAgent row. The pns crate's three comments saying a ring buffer is "not
rotate-logs' business" stay true of the lane and are not touched. Order: after E18.

## 5. Every referrer, found by grep on `319811e2`

Re-grep at cutover time; this is the list as of this commit.

`update-skills.sh` and its directory (PR E16):

- `.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh:30` (a comment naming it as an
  unattended managed script; the manifest walk itself is by glob and needs no change);
- `.chezmoiscripts/run_onchange_after_63-load-update-skills-launchagent.sh.tmpl` (deleted) and
  `.chezmoiscripts/run_onchange_after_64-update-skills-first-install.sh.tmpl` (rewritten);
- `CLAUDE.md` ("Background jobs and LaunchAgents" table, "Agent skills" paragraph, "Where deployed
  scripts live" rule 4);
- `docs/runbooks/agent-skills-store.md` ("Generation-exchange updates", "Schedule", "Adding a skill");
- `dot_agents/custom-skill-lock.json` (the `comment` field names the script as the installer);
- `dot_local/libexec/executable_brew-shellenv-cache-refresh.sh:39`,
  `dot_local/libexec/osquery/executable_drain-undelivered-alerts.sh:47`,
  `dot_local/libexec/osquery/results-alerter/pipeline-verdict.sh:18`,
  `Library/LaunchAgents/com.webdavis.pns-daemon.plist.tmpl:46` (comments);
- `dot_local/libexec/unattended-upgrades/agent-skills/executable_live-reconcile.sh:22,131,166` (its
  messages name the installer);
- `dot_local/share/pns/docs/specs/legacy-producer-flags.md:120-128` (nine rows);
- `justfile:318-321`;
- `test/fixtures/osquery-watchdog-lib.bash:172-174,346` (the fixture writes its own stub at that
  path; rename the stub to a neutral managed-script name);
- `test/unit/update-skills-*.sh` (three files, deleted), `test/unit/pns-weekly-engine-resolution.sh`
  (PR E17).

`report-plugin-updates.sh` (PR E4):
`.chezmoiscripts/run_onchange_after_69-load-report-plugin-updates-launchagent.sh.tmpl` (rewritten),
`CLAUDE.md` (the table row and rule 4), `docs/runbooks/agent-skills-store.md` ("Plugin update
record"), `docs/runbooks/claude-code-settings.md`,
`dot_local/share/pns/docs/specs/legacy-producer-flags.md:119`,
`Library/LaunchAgents/com.webdavis.report-plugin-updates.plist.tmpl` (deleted),
`test/unit/pns-weekly-engine-resolution.sh`.

`log-entries.sh` (PR E17): `.chezmoiscripts/run_after_68-hermes-log-route-status.sh.tmpl:21`,
`CLAUDE.md` rule 4, `docs/runbooks/agent-skills-store.md`,
`dot_local/share/pns/docs/specs/legacy-producer-flags.md:116-117`,
`dot_local/share/pns/docs/specs/unpinned-behaviors.md`, `test/unit/pns-weekly-engine-resolution.sh`.

`compress-and-truncate-local-logs.sh` (PR E19): `.chezmoiignore` (the Linux block),
`.chezmoiscripts/run_onchange_after_67-load-rotate-logs-launchagent.sh.tmpl` (deleted),
`CLAUDE.md:445,464` and the table row,
`Library/LaunchAgents/com.webdavis.rotate-logs.plist.tmpl` (deleted), `test/unit/rotate-logs-*.sh`
(four files, deleted).

## 6. Operator cutover order

Agents never apply, bootstrap or boot out anything. Before each cutover apply, the operator stops the
retired scheduled writer and waits for any active or manual invocation to finish. For E16 this includes
old updater and `live-reconcile` work; no publisher may overlap the new bootstrap. Source deletion
cannot stop an already loaded LaunchAgent. The completed lane is then deployed and bootstrapped by the
operator's full apply. Manual `live-reconcile` remains separate from running uu jobs.

- PR E4, before apply: `launchctl bootout gui/$(id -u)/com.webdavis.report-plugin-updates`.
  After apply, verify the legacy snapshot was imported (or that neither baseline existed), then trash
  `~/Library/LaunchAgents/com.webdavis.report-plugin-updates.plist`,
  `~/.local/libexec/unattended-upgrades/claude/`, `~/.local/log/plugins/`; trash
  `~/.local/state/report-plugin-updates/` only after successful import is confirmed. Failed import
  retains both states for retry; `uu doctor` alone does not prove the history was carried forward.
- PR E16, before apply: `launchctl bootout gui/$(id -u)/com.webdavis.update-skills`, then confirm no
  old/manual publisher is running. After successful bootstrap, trash
  `~/Library/LaunchAgents/com.webdavis.update-skills.plist`,
  `~/.local/libexec/unattended-upgrades/agent-skills/update-skills.sh`, `~/.local/log/skills/`,
  `~/.local/state/update-skills/`. Preserve `~/.local/state/skills/` and its
  `first-install-pending` marker. A failed or contended bootstrap retains that marker, so the next
  apply retries; only successful bootstrap clears it. Do not discard in-flight generation state.
- PR E17: trash `~/.local/libexec/unattended-upgrades/helpers/`.
- PR E19, before apply: `launchctl bootout gui/$(id -u)/com.webdavis.rotate-logs`.
  After apply, trash
  `~/Library/LaunchAgents/com.webdavis.rotate-logs.plist`,
  `~/.local/libexec/compress-and-truncate-local-logs.sh`, `~/.local/log/rotate-logs.log` and its
  archives, and the retired logs the explicit list no longer covers (`gha-watcher.log`,
  `paseo-daemon.log.1.gz`).
- After every cutover, `uu doctor`, then the first Sunday's record, and the streak directories under
  `~/.local/state/uu/lanes/` for the new names.

## 7. Reading order for a reviewer

The amended spec first, then section 3 of this plan, then step A (the record model every later lane
relies on), then the one lane pull request under review with its registry touch points, then, for a
cutover, section 5's list against a fresh grep.
