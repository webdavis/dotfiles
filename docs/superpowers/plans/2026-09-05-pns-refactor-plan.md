# pns architectural refactor: the migration plan

Written 2026-09-05 against `origin/main` at `cac6ff3f`. It moves the behaviors inventoried in
`docs/superpowers/specs/2026-09-05-pns-behavioral-specification.md` (285 statements, S001 to S285)
out of the legacy package at `dot_local/share/pns` and into the workspace members under
`dot_local/share/pns/crates/`, one reviewable pull request at a time. The method is the
`clean-code` skill's eighteen-step procedure and the `clean-code-rust` skill's spelling of it; the
target shape is the one `dot_agents/skills/clean-code-rust/PNS-EXAMPLE.md` records. This plan does
not restate either; it names, per step, what moves, which tests carry it, which consumer has to keep
working, what every resulting file measures, and which earlier step it has to land after. Corrected
the same day after review, with the two facts that changed under it: the presence policy merged
(`7c58f94b`) and PR 5.1 merged (#366, `10a2116d`).

## 1. Where the ladder starts

Steps 1 to 4 of the procedure are already on `main`:

- Step 1, the baseline as a set of 1,257 test names with results:
  `dot_local/share/pns/docs/test-baseline.tsv` and `.md`.
- Step 2, seventeen area specifications, a glossary and the unpinned-behaviors register:
  `dot_local/share/pns/docs/specs/`.
- Step 3, the classification of every test (permanent, adapter, mechanism, migration):
  `dot_local/share/pns/docs/test-baseline.md`.
- Step 4, the workspace, five member crates with the dependency edges enforced by their manifests:
  `dot_local/share/pns/Cargo.toml`, commit `f870d3de`.

Eleven decision records (`docs/decisions/0001` to `0011`) hold the measured reasoning. Every member
crate's `lib.rs` says "Nothing has moved in yet". The `pns` binary target and every module still live
in the root package; the builder, the LaunchAgent, the hooks, uu and moshi invoke the binary by that
one name, and it may not go missing between two pull requests.

The ladder therefore begins at step 5. Its inputs, all read in full for this plan:

- the crate (`src/`, `tests/`, `Cargo.toml`, the five member manifests);
- the four external readers named in section 2;
- the presence policy branch `pns-hue-presence-policy` (a 2,736-line diff against `main` when this
  was written), merged as `7c58f94b` on 2026-09-05, so every region it touched is on `main` and no
  step waits on it;
- the frozen backlog `~/.claude/pipeline/backlog-consolidated-2026-09-02.md`, absorbed in section 7;
- the delivery-safety rulings of 2026-09-03 (idempotency key per event, no inline retry, write-ahead
  journal, recording decorator at the composition root, at-least-once documented) and the four
  findings of the 2026-09-03 delivery-path review the refactor is the vehicle for (section 6).

PR 5.1 has merged as #366 (`10a2116d`), as its row states, with one deliberate deviation recorded in
the rows for PR 5.1 and PR 5.7. The ladder continues at PR 5.2.

## 2. The consumers that must keep working, and how each is proved

- **The builder**, `.chezmoiscripts/run_onchange_after_58-build-pns-engine.sh.tmpl:87`. Fixed:
  `cargo build --release --locked --quiet --bin pns --manifest-path <crate>/Cargo.toml` produces
  `target/release/pns`. Proof: `test/unit/pns-engine-build-install.sh`; the line itself changes only
  in PR 15.1.
- **The justfile**, `justfile:170-175` (`test-rust`) and `justfile:327-329` (`pns-config-render`).
  Fixed: `test-rust` already passes `--workspace`; the render recipe is a `cargo run --bin
  pns-config-render`. Proof: `just test-rust` on every PR, and
  `just pns-config-render && git diff --exit-code dot_config/pns/private_config.toml.tmpl`.
- **uu**, `dot_local/share/uu/Cargo.toml:36` (`pns = { path = "../pns" }`) and three import sites,
  `src/delivery.rs:11` (`SignedPost`, `delivered`, `outcome_line`, `sign`), `src/delivery.rs:99`
  (`PostOutcome`, under `cfg(test)`) and `src/cli/run.rs:13` (`UreqSignedPost`). Fixed: that one
  signed-POST seam. Proof: `cargo test --locked --manifest-path dot_local/share/uu/Cargo.toml` on
  every PR; the import path moves in PR 14.3 under the decision in section 8.
- **The command-line surface**: `private_dot_claude/modify_settings.json:325-387`, the LaunchAgent,
  `dot_bashrc.tmpl:582-590`, the Codex installer, uu's `[alerts] binary`, and moshi's `pns pi-hook`.
  Fixed: the argv surface in `const USAGE` (S002 to S046). Proof: the argv differential
  (`~/.claude/pipeline/extraction-verify.sh`) with a control mutant, on every PR.
- **The generated template**, `dot_config/pns/private_config.toml.tmpl`. Fixed:
  `BANNER + render(values) + FOOTER`, byte for byte. Proof: the render-and-diff check; the three
  out-of-crate pins move in PR 13.7.

## 3. Rules every pull request in the ladder obeys

1. **Kind of work, declared.** A PR is a pure move or new behavior, never both. A pure move carries
   the block-identity check (removed lines equal added lines after stripping the expected visibility
   token per line) and the whole-file reconstruction check (delete the block and the import lines from
   both sides and `cmp` the remainder), both from the header of `extraction-verify.sh`. New behavior
   carries a red test first and a mutation check by hand (revert the fix, the test goes red, restore).
2. **Gates.** `just test-rust`, `just lint-check`, the uu test line, the builder's build line, the
   render-and-diff check, and the argv differential with its control mutant. `just ship` before the
   pull request opens, because a topic branch with no open pull request runs the suite nowhere.
3. **The name set.** `cargo test --workspace -- --list` diffed against `docs/test-baseline.tsv` by
   leaf name. A test that moves keeps its name. A test that is replaced appears in the baseline's
   mapping table with its successor and its reason.
4. **File size.** Every `.rs` file, tests included, targets 300 lines and never exceeds 500 (operator
   ruling 2026-09-02, no waiver). The crate already uses the pattern that makes this reachable:
   `presence_hue.rs` keeps its tests in `presence_hue/tests.rs` and `presence_hue/selection_tests.rs`
   (`#[cfg(test)] mod tests;`). Every move below puts the unit tests of a module into a sibling
   `<module>/tests.rs`, split by behavior when that file would pass 500. The narrative comments that
   make `main.rs` 12,731 lines move into `docs/decisions/` records at the same time; production keeps
   a one-line invariant and a record link. The sizes in this plan are PROJECTIONS from the measured
   ranges being moved; the PR measures the result with `wc -l` and splits again if a projection was
   wrong.
5. **Live effects.** No step runs `pns doctor`, a bare `pns pulse`, or the built binary against the
   real bridge or lamps (decision 0005). The argv differential excludes both.
6. **Unpinned first.** Before a step moves code behind an UNPINNED statement in the specification, the
   PR before it writes the missing test against the code where it lives. The statements are listed per
   step below.
7. **Order.** The presence policy is on `main` (`7c58f94b`), so no step waits on a branch. A step
   lands after the steps its row names under `Order:`; a row with no `Order:` lands in ladder order.
8. **Two open pns branches at most**, per the operator's 2026-08-30 ruling, and every PR gets the
   independent model review at max reasoning beside the pipeline's own steps.
9. Repository rules: Conventional Commits, no trailers, no em-dashes, `trash` never `rm`, never
   `chezmoi apply`, never a force push.

## 4. What is being moved, measured

Line counts on `main` at `cac6ff3f`. "Production" is the count of lines above `#[cfg(test)] mod tests`.

| File                          | Total  | Production | Tests  | Files at 300 to 500 after |
| ----------------------------- | ------ | ---------- | ------ | ------------------------- |
| `src/main.rs`                 | 13,484 | 9,550      | 3,934  | 36 (production 26, tests 10) |
| `src/config.rs`               | 4,678  | 2,176      | 2,502  | 16 (7 and 9)              |
| `src/lights.rs`               | 3,520  | 1,369      | 2,151  | 10 (5 and 5)              |
| `src/home.rs`                 | 2,484  | 893        | 1,591  | 7 (3 and 4)               |
| `src/recap.rs`                | 2,417  | 1,135      | 1,282  | 8 (4 and 4)               |
| `src/system.rs`               | 2,377  | 923        | 1,454  | 7 (3 and 4)               |
| `src/channels/hue.rs`         | 2,307  | 1,070      | 1,237  | 7 (4 and 3)               |
| `src/config_text.rs`          | 2,124  | 1,140      | 984    | 6 (4 and 2)               |
| `src/engine.rs`               | 1,779  | 433        | 1,346  | 5 (2 and 3)               |
| `src/doctor.rs`               | 1,851  | 757        | 1,094  | 6 (3 and 3)               |
| `src/daemon.rs`               | 1,419  | 420        | 999    | 5 (2 and 3)               |
| `src/missed_notifications.rs` | 1,377  | 500        | 877    | 4 (2 and 2)               |
| `src/decision_log.rs`         | 963    | 366        | 597    | 3                         |
| `src/hooks.rs`                | 890    | 381        | 509    | 3                         |
| `src/registry.rs`             | 874    | 408        | 466    | 3                         |
| `src/surface.rs`              | 854    | 228        | 626    | 3                         |
| `src/channels/moshi.rs`       | 684    | 234        | 450    | 2                         |
| `src/channels/hermes.rs`      | 672    | 286        | 386    | 2                         |
| `src/focus.rs`                | 646    | 156        | 490    | 2                         |
| `src/nag.rs`                  | 545    | 330        | 215    | 2                         |
| `src/setup.rs`                | 525    | 182        | 343    | 2                         |
| `src/routing.rs`              | 502    | 131        | 371    | 2                         |
| every other `src` file        | under 500 | | | unchanged, or 2 where tests push past 300 |
| `tests/dispatch.rs`           | 8,581  |            | 8,581  | 20 by specification area  |
| `tests/hooks.rs`              | 6,217  |            | 6,217  | 14 by specification area  |
| `tests/support/mod.rs`        | 849    |            |        | 3 (sandbox, daemon guard, stubs) |
| `tests/daemon.rs`             | 797    |            |        | 2                         |
| `tests/setup.rs`              | 665    |            |        | 2                         |
| `tests/native.rs`, `tests/config_render.rs` | 395, 286 |    |        | unchanged                 |

The root files under 500 today are `presence_room.rs` 404, `banner.rs` 374, `presence_journal.rs`
357, `presence.rs` 333, `pulse.rs` 320, `presence_file.rs` 317, `presence_lock.rs` 307, `quiet.rs`
261, `args.rs` 256, `presence_instant.rs` 242, `presence_hue.rs` 239 plus 423 in its two test files,
`presence_policy.rs` 222, `channels/mod.rs` 194, `pns-config-render.rs` 185, `http-capture.rs` 138,
`probes.rs` 123 and `lib.rs` 52. PR 5.1 has already landed `crates/pns-domain/src/render.rs` 132
plus `render/tests.rs` 254, `safety.rs` 77 plus 148, `count.rs` 59 plus 72 and `lights.rs` 54 plus
72. `config.rs` carries a test-only `#[cfg(test)]` scanner at line 736 above its production code;
the column counts the lines above `mod tests`, which is what the per-step projections below split.

## 5. The ladder

Each entry: **Moves** (symbols and their measured ranges today), **Tests** (moved by name, or written
first), **Consumer** (what must keep working and how it is proved beyond the standing gates),
**Sizes** (projected files after, with the 300 target and 500 cap noted), and **Order** where a step
has to land after a named earlier one.
Every PR also carries its unit tests into `<module>/tests.rs` per rule 4 and its narrative comments
into a decision record; that is not repeated per entry.

### Step 5: pure policy into `pns-domain`

Every move here is a pure move. The root package gained `pns-domain = { path = "crates/pns-domain" }`
in PR 5.1 (merged) and re-exports each moved module from `src/lib.rs` (`pub use pns_domain::render;`)
so that
`pns::render` and every `tests/*.rs` path keep compiling until PR 17.1 deletes the re-exports. A
member crate never depends on the root package, so the traffic runs one way.

**PR 5.1 counting, text and safety.** MERGED as #366 (`10a2116d`), as planned here, with one deliberate
deviation. Moved `parse_count` and `SHELL_ARITHMETIC_MAX` from `src/lib.rs` to `pns-domain/src/count.rs`;
`src/safety.rs` whole to `pns-domain/src/safety.rs`; `src/render.rs` whole to `pns-domain/src/render.rs`;
and, the deviation, `working_owner`, `WORKING_PENDING` and `WORKING_SWEEP` from `src/lights.rs` to
`pns-domain/src/lights.rs`, because `safety.rs` calls `working_owner` and a member crate never reaches
back into the root package. Tests: the nine count tests, the sixteen `safety` tests, the thirty-one
`render` tests and the two `working_owner` tests, by name, each in a sibling `tests.rs`. Consumer:
`hooks.rs`, `channels/*`, `main.rs` through the re-exports. Sizes, measured: `count.rs` 59 plus
`count/tests.rs` 72; `safety.rs` 77 plus 148; `render.rs` 132 plus 254; `lights.rs` 54 plus 72.
Statements: S004, S021, S050 (the caps), S118, S126, S127, S181 (`working_owner`).

**PR 5.2 surface, quiet and pulse.** Moves `src/surface.rs` (`Surface`, `Visibility`, `SessionView`,
`DeliveryPlan`, `visibility`, `fresh_age`, `is_fresh`, `surface`, `effective_visibility`, `plan`, lines
1-228), `src/quiet.rs` (`parse_duration`, `expiry_from_state`, `is_muted`, `status_line`, `minutes_left`,
1-99), `src/pulse.rs` (colours, `session_was_long`, `exit_behaviour`, `LAMP_BLOCKED`, `state_behaviour`,
1-156) to `pns-domain/src/{surface,quiet,pulse}.rs`. Tests: the 18 surface, 9 quiet and 18 pulse tests by
name; `surface/tests.rs` is split into `matrix.rs` (the arbitration and plan matrices) and `rules.rs`.
Sizes: `surface.rs` 228, `surface/matrix.rs` ~330, `surface/rules.rs` ~300; `quiet.rs` 99 plus tests 162;
`pulse.rs` 156 plus tests 164. Statements: S018, S093 to S100, S104 (the predicate half), S217.

**PR 5.3 routing and the registry's policy half.** Moves `src/routing.rs` whole and, from
`src/registry.rs`, `Routing`, `PluginKind`, `Registration`, `RegistryError`, `Selection`, `Registry`,
`roster`, `ROSTER`, `REQUIRES`, `CORE` (lines 26-343) to `pns-domain/src/routing.rs` and
`pns-domain/src/registry.rs` plus `registry/roster.rs`. `select_plugins` and its two warnings (368-407)
stay in the root package: they take `LoadOutcome`, which is an edge type, and move in PR 6.1. Tests: 14
routing tests and the registry tests that need no `LoadOutcome`, by name. Consumer: `main.rs`
`dispatch_legs`, `doctor_mode`. Sizes: `routing.rs` 131 plus tests 371 (tests split at the
`--local-only`/`--remote-only` seam if over 500 after the presence-gate tests join); `registry.rs` ~300,
`registry/roster.rs` ~110, `registry/tests.rs` ~400. Statements: S016, S017 (the plan half), S119 to
S124.

**PR 5.4 the missed-notification policy.** Moves `KEPT`, `Entry` (the struct only), `summary`,
`event_count`, `NEEDS_YOU`, `needing_you`, `recap_card`, `waiting_line` and the three private helpers
they compose through (`src/missed_notifications.rs:25-49`, `137-199`, `232-417`, `429-499`) to
`pns-domain/src/missed.rs`. Two halves stay in the root package. The JSON codec `entry`, `entries` and
`text` (169-230, 419-427) stays until PR 11.2, because the domain crate takes no `serde_json`. And
`was_missed`, `should_replay` and `is_present` (79-83, 113-115, 133-135) stay until **PR 5.11**, because
they answer over the engine's `Decision` and `Overrides`, which do not reach the domain crate before that
step; moving them here would mean pulling 5.11's whole move forward and leaving that step empty. Tests:
the ten card and doctor-line tests that build an `Entry` directly, by name; the four whose fixture
round-trips a journal through `entries` stay with the codec, as do the predicate tests and the privacy
test that spans both. Consumer: `main.rs` `record_missed`, `replay_missed`, `missed_line`. Sizes,
measured: `missed.rs` 323 plus `missed/tests.rs` 306; the root module splits its remaining tests into
`predicate_tests.rs` 269 and `codec_tests.rs` 315. Statements: S106, S158 (predicate), S159, S161
(predicate), S243, S244; S159 stays UNPINNED here and is tested in PR 11.3, as the specification says.

**PR 5.5 the nag policy.** Moves `Record` (the struct), `nudge`, `is_stale`, `fate`, `Dropped`,
`FIRE_STALE_SECS`, `MAX_SESSION_ID_CHARS`, `marker_name`, `job_id`, `session_of` (`src/nag.rs:23-32`,
`112-119`, `180`, `222-330`) to `pns-domain/src/nag.rs`. The JSON codec `render`/`parse`, `nag_dir`,
`record_path`, `claim_path` (path grammar) stay for PR 11.5. Tests: by name. Sizes: `nag.rs` ~180 plus
tests ~140. Statements: S239 (`fate`), S240.

**PR 5.6 the job policy.** Moves `Job`, `Verdict`, `Reason`, `decide`, `rearm`, `Heartbeat`, the bounds
(`ID_MAX`, `RECORD_MAX`, `ARGS_MAX`, `ARGS_BYTES_MAX`, `EVERY_MAX_SECS`, `MIN_EVERY_SECS`,
`DUE_WINDOW_SECS`, `HEARTBEAT_STALE_SECS`) and `name_is_safe` (`src/daemon.rs:29-45`, `172-340`) to
`pns-domain/src/jobs.rs`. `validate_shape` and `validate_registration` do NOT move, against this row's
first draft: `validate_shape`'s last rule caps the RENDERED record, which makes it a fact about the
serialized form rather than about the job, and `validate_registration` calls it. Both stay beside
`render` with their own tests, and both go to `pns-adapters` with the TAB codec in PR 11.5, which is
where the serialized form lands. A pure `validate_shape(job, rendered_len)` would let the rule move
later; nothing needs it yet. The TAB codec, `spool_entries`, `peek`, `claim`, `hand_back`, `publish_*`,
`marker_exists`, `prepare_spool` stay for PR 11.5 too. Tests: `decide`, `rearm`, heartbeat round trip, by
name; the two validators' tests stay with them. S206's own test is written here, red-first, in
`doctor.rs` rather than the domain, because the behavior it states belongs to the grader `daemon_line`
and a test beside the constant can only restate the constant. Sizes: `jobs.rs` ~230 plus tests ~350.
Statements: S199, S200 (the `rearm` half), S205, S206.

**PR 5.7 the lights policy.** Moves from `src/lights.rs`: `WORKING`, `any_working`, `Streak`,
`next_streak`, `News`, `news_after`, `Unread`, `unread_arming`, `last_interaction`, `Loop`,
`loop_running`, `any_blocked`, `marker_is_live`, `Held`, `House`, `active_held`, `shown`, `pulse_fires`,
`Leg`, `Fade`, `FADE_LEAD_MS`, `Resume`, `breath_cycle`, `breathe_then_flare_cycle`, `step_ms`,
`breath_fades`, `Phase`, `HeldEntry`, `resume_from`, `Action`, `blocked_marker_action`, `Say`, `say`,
`Muted`, `MAX_MUTED_PLACES`, `bare_mute_secs`, `muted_after`, `muted_places`, `muted_report` (lines
19-1369 less the items below; `working_owner` and its two suffixes are already in
`pns-domain/src/lights.rs` since PR 5.1 and stay in that module's root file). Stays for later steps:
`workspace_agent_statuses` (a `serde_json` parse of herdr's answer, PR 14.1),
`render_streak`/`parse_streak`, `render_news`/`parse_news`, `render_held_token`/
`parse_held_token`, `muted_entries`/`render_muted` (state codecs, PR 11.2), `lease_dir`, `lease_marker`,
`blocked_dir`, `blocked_marker`, `sweep_claim` (paths, PR 11.5), `loop_command`, `LOOP_USAGE`,
`QuietCommand`, `quiet_command`, `NO_SCHEDULE` (argv adaptation, PR 8.1). The seven files under
`pns-domain/src/lights/` measure `held.rs` 146 lines, `streak.rs` 67, `unread.rs` 151, `looping.rs` 73,
`breath.rs` 165, `phase.rs` 188 and `mute.rs` 150. The domain root remains `lights.rs` at 63 lines.
`Breath` and `BreatheThenFlare` move early from `src/config.rs` into
`pns-domain/src/lamps/config.rs`, now 57 lines. The consumers `breath_cycle(&Breath)` and
`breathe_then_flare_cycle(&BreatheThenFlare)` require those values in the domain, which cannot depend
back on the legacy configuration parser.

All 54 leaf test names survive, including the two `working_owner` tests already in the domain's
`lights/tests.rs` at 72 lines. The four held, seven breath and five accent tests move into
`lights/held/tests.rs` (124 lines), `lights/breath/tests.rs` (298) and
`lights/breath/tests/accent.rs` (200), with their bodies unchanged apart from paths. Breath and accent
share the literal motion fixtures in `lights/breath/tests/fixtures.rs` (30). The remaining 36 tests
stay in the legacy `src/lights/` modules: `streak_tests.rs` (109), `unread_tests.rs` (280),
`loop_tests.rs` (261), `phase_tests.rs` (343), `mute_tests.rs` (133) and `quiet_command_tests.rs`
(360). These modules mix policy with codecs, marker paths or argument parsing. The legacy
`fixtures.rs` is 76 lines and `src/lights.rs` is 459. Statements: S114 (`blocked_marker_action`),
S115 (`marker_is_live`), S173, S178 (`say`), S223 to S225, S228 (the schedule), S040
(`bare_mute_secs`).

**PR 5.8 the lamp resolution policy.** Moves from `src/channels/hue.rs`: `QuietWindow`, `minute_of_day`,
`quiet_now`, `Fixture`, `Unresolved`, `Missing`, `missing_sentence`, `Lamp`, `Inventory` (types),
`DimWindow`, `Routed`, `Routing`, `LEVELS`, `resolve`, `Showing`, `dim_showing`, `window_refusal`,
`Muting`, `muted_now`, `mutable_names`, `remember` (lines 1-743 less `hue_settings`, `quiet_window` and
`inventory`, which parse TOML and JSON) to `pns-domain/src/lamps/{window,dim,resolve,mute,inventory}.rs`.
Tests: 42 by name, split alongside. Consumer: `main.rs` `fire_pulse_unless_quiet`, `run_pulse_writes`,
`run_tick_writes`, `lights_quiet`. Sizes: five production files of 80 to 260, tests 120 to 400. Its
callers in `main.rs` stay put in this PR. Statements: S107 (`QuietWindow`), S108, S111, S112
(`muted_now`), S222.

**PR 5.9 the recap composition.** Moves `src/recap.rs` whole (it reads no file, no clock and no
environment; its one input type is `missed::Entry` from PR 5.4) to
`pns-domain/src/recap/{sections,night,external,budget,sanitize,prompt}.rs`. Tests: 31 by name, split
alongside; the Unicode 17.0 range table and its checker go with `sanitize`. Sizes: six production files
of 120 to 260; tests of 150 to 420. Statements: S246 (budgets), S247, S250 (`answer`, `safe_line`), S251,
S252.

**PR 5.10 the home-probe policy.** Moves from `src/home.rs`: `DeviceKey`, `DeviceIdentity`,
`HomePresence`, `Client`, `KeyReading`, `KeyOutcome`, `home_reading`, `client_carries`, `first_match`,
`client_label`, `normalized_mac`, `Staleness`, `stale_identifiers`, `episode_id`, `is_new_staleness`,
`stale_warning`, `UNIFI_TYPE` to `pns-domain/src/home/{identity,reading,staleness}.rs`. Stays:
`parse_clients`, `first_site_id`, `Router`, `UniFiRouter`, `read_home`, `ROUTER_*` (PR 14.5);
`router_settings`, `device_identity`, `router_api_key`, `stale_alert_channel`, `enabled_router_table`,
`SetupFailure`, `setup_report`, `report` (config reading and presentation, PRs 13.4 and 15.1). Tests: 52
by name, split. Sizes: three production files of 120 to 260, tests of 200 to 420. Statements: S168
(`episode_id`), S273 (`home_reading`), S274 (`stale_identifiers`).

**PR 5.11 the decision.** Moves from `src/engine.rs`: `DEFAULT_DESK_IDLE_SECS`, `Overrides` (the struct
and `silenced`, `reads_desk`, `reads_phone`), `Decision`, `GateInputs`, `SurfaceReading`, `decide` (lines
29-99, 134-301) to `pns-domain/src/decision.rs`. It ALSO carries the three predicates PR 5.4 had to leave
behind, `was_missed`, `should_replay` and `is_present`, out of `src/missed_notifications.rs` and into
`pns-domain/src/missed.rs` beside the rest of that policy, with their twelve tests: they read only the
`Decision` and `Overrides` this step moves, so this is the first step at which they can go. Stays:
`Overrides::from_env` (reads the environment, PR 8.1), `operator_surface`, `surface_reading`,
`operator_visibility` (they drive probe traits, PR 6.1). Tests: the `decide` tests by name into
`decision/tests.rs`, split by the mute, the override and the readings seams, plus the predicate tests
into `missed/tests.rs`. Sizes: `decision.rs` ~170; three test files of 350 to 450. `decide`'s signature
does not change in `engine.rs`. Statements: S099 (the arbitration), S102, S103, S118, and S106, S158
(predicate), S159, S161 (predicate) arriving from PR 5.4; S159's own test stays assigned to PR 11.3.

**PR 5.12 the presence policy.** Moves `src/presence.rs` (`idle_secs_from_ns`, `PresenceStatus`,
`Unreadable`, `classify`, `unreadable_said`), the new `presence_policy.rs` (`Narrowing`, `narrow`) and
`presence_room.rs` (`Snapshot`, `Full`, `chosen`, `desk_age`) to
`pns-domain/src/presence/{status,narrowing,room}.rs`. `presence_file.rs` (a state-file line codec) and
`presence_instant.rs` (the bridge's timestamp grammar) go to adapters in PR 10.2. Tests: by name,
including the presence policy's own. Sizes: three production files under 200; tests under 350.
Statements: S084 (`idle_secs_from_ns`), S234, S235.

**PR 5.13 the decision record.** Moves from `src/decision_log.rs`: `KEPT`, `Record`, `printable`,
`IDENTITY_MAX`, `tri`, `count`, `verdicts` (lines 5-66, 202-260) to `pns-domain/src/decision_record.rs`.
Stays: `line` (the ring's on-disk shape, PR 11.2), `section`, `render`, `complaint`, `escaped`,
`QUOTED_MAX` (the doctor's presentation, PR 15.1). Tests: the `printable` and `Record` tests by name.
Sizes: ~150 plus tests ~250. Statements: S157 (`printable`).

| Pull request                         | Status   |
| ------------------------------------ | -------- |
| 5.14 shard main.rs into root modules | Complete |

Moves the 13,484-line `src/main.rs` into 51 root-package responsibility modules, with 27 test files and
four shared fixture files. Measured after formatting: `main.rs` is 299 lines; every extracted file is
below 500 lines, with a maximum of 417. The optional hooks split reduces `tests/hooks.rs` from 6,217 to
351 lines; its 29 behavior files are at most 360 lines. Bodies, names and test leaves are preserved. This
changes no ownership: later steps still move these modules into crates. The shard exists so parallel
lanes own distinct files instead of editing the same `main.rs`.

Unpinned statements written first in this step: S015 (last flag wins) before PR 8.1 rather than
here; none of PR 5.1 to 5.13 moves code behind an UNPINNED statement, because the unpinned rows in
sections 1 to 6 of the specification all sit in `main.rs`.

### Step 6: use cases and the ports they own, in `pns-application`

New code, test-first: a use case is the ordering of calls that `run_event` and its siblings perform
today, expressed over traits the use case declares. Each PR moves one `*_mode` body out of
`main.rs` into a use case, leaves a one-line call at the old site, and proves the argv differential
unchanged.

**PR 6.1 the ports and the selection policy.** Moves `src/probes.rs` (the five probe traits, `Wants`,
`ProbeStart`, 123 lines) to `pns-application/src/ports/environment.rs`; moves `operator_surface`,
`surface_reading`, `operator_visibility` (`src/engine.rs:332-433`) to
`pns-application/src/environment_reading.rs`; moves `select_plugins` and its warnings
(`src/registry.rs:368-407`) to `pns-application/src/selection.rs` over a `ConfigOutcome` type it
declares. Declares, test-first, the ports the later use cases need: `Clock`, `NotificationDestination`
(`deliver(&Event, ReportMode) -> Delivery`), `DecisionRing`, `Journal`, `ActivityRing`, `ReturnMoment`,
`LampRecords`, `JobSpool`, `ApprovalForwarder`, `Bridge` (moved from `hue.rs:744-750`), `Router` (from
`home.rs`), `CommandRunner` (from `system.rs`). Tests: the `engine.rs` probe-count tests
(`CountingProbes`) by name; new port tests only where a port carries logic (none should). Sizes:
`ports/*.rs` under 120 each; `environment_reading.rs` ~150 plus tests ~350; `selection.rs` ~80 plus tests
~120. Statements: S085 (the read-only-where-idle-answered rule), S089 to S091, S124.

**PR 6.2 `SubmitNotification`.** Moves `run_event` (`src/main.rs:2917-3223`), `Attempt`, `dispatch_legs`,
`rendered_event`, `overrides_from_env`'s call site, the record tail (`record_decision`, `record_missed`,
`record_activity`, `update_blocked_marker`, `record_news`, `renew_loop_lease`, `mark_present`, the pulse
gate, `clear_held_lamps`, `register_lights_tick`) into `pns-application/src/submit_notification.rs` as
one use case over the PR 6.1 ports, keeping today's ordering exactly (decide, snapshot, dispatch,
decision record, journal, marker, news, lease, activity, replay, edge, pulse, clear, tick). The
filesystem bodies of those records stay in the root package behind port implementations until step 11.
Tests written first: one ordering test per tail item using recording fakes for the ports (the order is
the behavior, S072, S157, S158, S161); the existing `tests/dispatch.rs` rows stay as the acceptance
tests. Consumer: every hook and every producer. Sizes: `submit_notification.rs` ~280 (the 300 target
binds here; the tail becomes a `record_tail.rs` of ~200 if it does not fit),
`submit_notification/tests.rs` ~400. Statements: S006, S017, S021, S072, S080, S106, S117, S218, S230,
S231.

**PR 6.3 `RequestApproval`.** Moves `blocking_event`, `forward_to_moshi`, `answer_within`,
`moshi_decision`, `submit_deadline`, `configured_submit_deadline`, `SUBMISSION_POLL_INTERVAL`, and
`gate_mode`'s body (`src/main.rs:235-247`, `2320-2606`) into `pns-application/src/request_approval.rs`
over an `ApprovalForwarder` port; `spawn_moshi_hook`, `moshi_hook_bin`, `DEFAULT_MOSHI_HOOK_BIN`
(2445-2490) become the adapter in PR 14.6 and stay in the root until then. Tests: the `tests/hooks.rs`
approval section stays as acceptance; a use-case test pins the order (forward, skip-phone only on a real
spawn, arm, notify, wait). Consumer: `pns hook blocked`, `pns gate`, `pns pi-hook`, proved by the argv
differential's `gate` and `hook` rows plus the hooks suite. Sizes: ~240 plus tests ~300. Unpinned first:
S082 (a gate run leaves no marker). Statements: S022, S023, S074 to S083.

**PR 6.4 `ReplayMissedNotifications` and `RecordActivity`.** Moves `replay_missed`, `Moment`,
`claim_moment`, `StrandedWindow`, `stranded_window_claim`, `window_claim_suffix`, `window_claim_is_free`,
`STALE_WINDOW_CLAIM_SECS`, `spawn_recap`'s decision (not its spawn), `Claimed`, `claim_journal`,
`stranded_claims`, `abandoned_hold`, `owner_is_gone`, `claim_by_rename`, `take_claim`, `activity_in`,
`mark_present`, `advance_marker`, `read_epoch` (`src/main.rs:1025-1790`) into
`pns-application/src/replay_missed.rs` over `ReturnMoment`, `Journal` and `ActivityRing` ports; the
rename protocol itself becomes the filesystem adapter in PR 11.5. Tests: the replay rows in
`tests/dispatch.rs` stay as acceptance; use-case tests pin `Moment` arbitration over a fake. Unpinned
first: S163 (the 300 s window-claim age test). Sizes: ~280 plus tests ~350; the claim protocol adapter is
measured in PR 11.5. Statements: S155, S156, S161 to S165, S242 to S245.

**PR 6.5 `RunNag`.** Moves `nag_mode`, `arm_nag`, `clear_nag`, `record_entries`, `claim_record`,
`claim_fire`, `claim_lock`, `publish_lock`, `lock_aged_out`, `release_fire`, `marker_path`,
`write_marker`, `nag_after_secs` (`src/main.rs:4622-5188`) into `pns-application/src/run_nag.rs` and
`arm_nag.rs` over `NagRecords` and `JobSpool` ports. Tests: the nag section of `tests/hooks.rs` as
acceptance. Unpinned first: S061 (the `stop-failure` clear), S183 (`fire.lock` age-out), S241 (the
per-record rename; the code says no test can kill it, so this one is written as a two-process test or
recorded as accepted in the decision record). Sizes: `run_nag.rs` ~230, `arm_nag.rs` ~150, tests ~300.
Statements: S042, S060, S073 (nag half), S182, S236 to S241.

**PR 6.6 `BuildReturnRecap`.** Moves `recap_mode`, `read_sources`, `summarized`, `left_of`,
`recap_bounds`, `wall_clock`, `post_recap`, `deliver_recap`'s decision, `RECAP_ROUTE`, `RECAP_USAGE`,
`NO_WALL_CLOCK` (`src/main.rs:8003-8216`, `8206-8395`) into `pns-application/src/build_return_recap.rs`
over `MergedPullRequestSource`, `ReviewNoteSource`, `Summarizer`, `ActivityRing` and
`NotificationDestination` ports; `merged_pull_requests`, `notes_matching`, `matches_glob`, `within`,
`read_note`, `summarize` and the `GH_*`, `MAX_NOTES`, `NOTE_READ_MAX` constants (7967-8205) become
adapters in PR 14.6. Tests: the recap section of `tests/dispatch.rs` and `tests/native.rs` as acceptance;
a use-case test pins the one-budget rule over a fake summarizer. Unpinned first: S254 (`--:--` end to
end). Sizes: ~260 plus tests ~300. Statements: S034, S246 to S253.

**PR 6.7 `ReconcileLights`.** Moves `lights_tick`, `Breathing`, `run_tick_writes`, `drive_breaths`,
`Standing`, `lights_house`, `last_interaction`, `read_news`, `news_at`, `record_news`, `claim_news`,
`sweep_shell_markers`, `sweep_blocked`, `blocked_lamp`, `advance_streak`, `read_held`, `held_lamps`,
`remember_held`, `say_lights_once`, `sweep_legacy_state`, `sweep_leases`, `sweep_markers`,
`tick_bridge_deadline`, `lights_tick_stale_secs` (`src/main.rs:5579-5682`, `5742-6760`) into
`pns-application/src/reconcile_lights.rs` plus `reconcile_lights/{house,writes,breath}.rs` over `Bridge`,
`LampRecords`, `Clock` and a `Sleeper` port (the tick already takes the clock and the sleeper as
parameters). Tests: the 30 `main.rs` tick tests by name plus the `tests/dispatch.rs` tick rows. Unpinned
first: S113 (the tick ignores `pns quiet` and Focus). Sizes: four production files of 150 to 280; tests
split into four of under 450. Statements: S039, S115, S171 to S180, S226 to S229.

**PR 6.8 `SetLightsQuiet` and `AcquireLoopLease`.** Moves `lights_quiet`, `mutable_names`,
`asks_the_bridge`, `bridge_inventory`, `publish_muted`, `muted_state`, `ad_hoc_quiet`
(`src/main.rs:5700-5961`) and `loop_mode`, `end_lease`, `renew_loop_lease` (5237-5357) into
`pns-application/src/{set_lights_quiet,loop_lease}.rs`. Tests: by name. Sizes: ~200 and ~120 plus tests.
Statements: S040, S041, S175, S177.

**PR 6.9 `RunDaemonTick`, `ScheduleJob`, `CancelJob`.** Moves `daemon_run`, `daemon_pass`, `Bounded`,
`drain_spool`, `act`, `release`, `fire`, `reap`, `kill_group`, `child_bound`, `CHILD_TICKS`,
`SWITCH_TICKS`, `daemon_enabled`, `ensure_presence_poll`, `daemon_tick`, `daemon_schedule`,
`parse_schedule`, `daemon_cancel`, `DEFAULT_LEASE_SLACK_SECS` (`src/main.rs:7024-7731`) into
`pns-application/src/daemon/{run,pass,fire,schedule}.rs` over `JobSpool`, `Clock` and a `JobRunner` port;
`spawn_job` (7106) is the runner adapter in PR 14.6. Tests: the 18 `main.rs` daemon tests by name,
`tests/daemon.rs` as acceptance. Unpinned first: S035, S036 (the daemon usage arms), S038 (`daemon
cancel` end to end), S202 (`kill_group` refusals), S203 (orphaning on exit; written as a test that
observes a child surviving `SIGTERM`, or recorded as accepted). Sizes: four production files of 120 to
260; tests under 450 each. Statements: S035 to S038, S194 to S206.

**PR 6.10 `ReadHomeProbe`.** Moves `home_mode`, `remember_staleness`, `remembered_staleness`
(`src/main.rs:741-773`, `4012-4126`) into `pns-application/src/read_home_probe.rs` over `Router` and a
`StalenessMemory` port. Unpinned first: S029 (`pns home <extra>`). Sizes: ~150 plus tests ~200.
Statements: S028, S029, S168, S271 to S274.

**PR 6.11 `RunDoctor`.** Moves `doctor_mode`, `read_pairing`, `decision_section`, `missed_line`,
`focus_line`, `daemon_line`, `disabled_backend_warnings`, `hue_resolves`, `pulse_outcome`,
`lights_report`, the `DOCTOR_*`, `MOSHI_*_DEADLINE`, `PAIRING_READ_MAX`, `FOCUS_UNREADABLE`,
`MISSED_UNREADABLE`, `NO_HUE_BRIDGE_LINE` constants (`src/main.rs:3646-3699`, `3704-3781`, `4147-4376`,
`7505-7750`) into `pns-application/src/run_doctor.rs` plus `run_doctor/{sections,pairing}.rs`.
`src/doctor.rs` (the pure report shaping) moves to `pns-domain/src/doctor/{census,report,pairing}.rs` in
the same PR because its `Outcome::Presence` arm is what the presence policy added. Tests: the 41
`doctor.rs` tests by name; the doctor rows of `tests/dispatch.rs` as acceptance. Unpinned first: S130 (a
real panic through the three `catch_unwind` sites). Sizes: domain three files of 150 to 240 plus tests
under 400 each; application ~250 plus two of ~150. Statements: S032, S033, S138, S150, S255 to S262.

**PR 6.12 `RunSetup`.** Moves `setup_mode`, `walk`, `armed`, `armed_secret`, `nothing_given`, `ask`,
`ask_hidden`, `read_answer`, `reading_from_the_background`, `read_failure`, `Hushed`, `ask_yes`,
`means_yes`, `router_backend`, `list`, `publish_config`, `pending_name`, `write_then_publish`,
`also_kept`, `keep_aside`, `keep_aside_at`, `CONFIG_FILE_MODE`, `SETUP_PREAMBLE`, `SETUP_USAGE`,
`unresolvable_ancestor` (`src/main.rs:8647-9354`) into `pns-application/src/run_setup.rs` over a
`Terminal` port (ask, ask hidden, is a terminal) and a `ConfigPublisher` port; `Hushed` and the pty work
are the terminal adapter (PR 14.6), the pending-file and hard-link publish is the persistence adapter (PR
11.1). `src/setup.rs` (`Answers`, `compose_config`, `backup_path`) moves to `pns-domain/src/setup.rs`.
Tests: `setup.rs` tests by name, `tests/setup.rs` as acceptance. Unpinned first: S268 (the composed text
parsed before writing, end to end), S270 (`also_kept`). Sizes: `run_setup.rs` ~220, `walk.rs` ~180,
adapters measured at their PRs. Statements: S043, S044, S190, S263 to S270.

**PR 6.13 `SignalLamps` (the event path's pulse and clear).** Moves `fire_pulse_unless_quiet`,
`fire_pulse`, `fire_lights`, `run_pulse_writes`, `routing_complaints`, `clear_held_lamps`,
`register_lights_tick`, `schedule_lights_tick`, `enabled_hue_table` (`src/main.rs:3308-3433`,
`3359-3380`, `3510-3703`) into `pns-application/src/signal_lamps.rs`, called from `SubmitNotification`
(PR 6.2) and `pulse_mode`. Tests: the pulse and lamp rows of `tests/dispatch.rs` as acceptance;
`run_pulse_writes`'s `main.rs` tests by name. Unpinned first: S110 (the fresh clock at the gate: written
as a fake-clock test that advances between decide and gate). Sizes: ~260 plus tests ~300. Statements:
S025, S026, S107, S109, S110, S218 to S221.

### Step 7: the versioned protocols, test-first, in `pns-protocol`

**PR 7.1 the request and result envelopes.** New behavior. `pns-protocol/src/{request,result,bounds}.rs`:
schema id with a major version, a producer-generated request id (the idempotency key per event, ruling 1
of 2026-09-03), tagged `Signal` enum (`Succeeded`, `Failed`, `NeedsAttention`, `ApprovalRequested`,
`Resolved`, `Observation`, `Progress`), `extensions` object, bounds enforced at the boundary (bytes,
field count, text length, collection length, nesting depth). Tests first: round trip, every bound one
step either side, an unknown major version refused, an unknown field inside `extensions` kept. Sizes:
three files under 250 plus tests under 400.

**PR 7.2 the egress envelope.** New behavior. `pns-protocol/src/egress.rs`: what an executable
destination is handed, carrying the request id and the rendered event; the legacy `Event::to_json`
shape (S126) is its version 1 body so today's `<name>.sh` channels keep working byte for byte.
Tests first: the byte-identity with today's JSON over the same event. Sizes: ~150 plus tests ~200.

**PR 7.3 `pns submit --json`.** New behavior. The cli reads one request from stdin, decodes it through
`pns-protocol`, maps it onto `SubmitNotification` (PR 6.2), and prints one result envelope; the request
id rides into the hermes body and an `Idempotency-Key` header (ruling 1), and the moshi body's `data`; a
replay carries the ORIGINAL id (S243 gains that clause). Tests first: an end-to-end submit through the
sandbox, the header on the wire through the capture server, the id surviving a replay. The argv
differential gains a `submit` row. Sizes: `pns-cli/src/submit.rs` ~200 plus tests ~300. Order: after PR
6.2.

### Step 8: the legacy command-line and hook adapters over the use cases

**PR 8.1 the producer argv adapter.** Moves `src/args.rs` whole, `is_producer_argv`, `second_argument`,
`event_mode`, `USAGE`, `PULSE_USAGE`, `LIGHTS_USAGE`, `LOOP_USAGE`, `QUIET_USAGE`, `DAEMON_USAGE`,
`NAG_USAGE`, `Overrides::from_env`, `loop_command`, `quiet_command`, `parse_schedule`, `recap_bounds`,
`pulse_mode`'s and `quiet_mode`'s argument arms into
`pns-cli/src/legacy/{argv,usage,overrides,verbs}.rs`. The both-flags refusal stays here with its tested
wording (decision 0007) and never becomes a domain state. Unpinned first: S007 (a subcommand word
carrying producer flags), S010 (the exact warning sentence), S015 (last flag wins), S027 (`USAGE` versus
`PULSE_USAGE`, fixed rather than pinned: backlog B30), S031 (`pns quiet --help`). Tests: the 9 `args.rs`
tests by name; `tests/dispatch.rs`'s argv rows as acceptance; the argv differential. Sizes: four files of
100 to 240 plus tests under 300. Statements: S001 to S021, S025, S027, S030, S031, S034 to S046.

**PR 8.2 the harness hook adapters.** Moves `src/hooks.rs` (`HookPayload`, `parse_payload`, `flattened`,
`one_line`, `tool_request`, `elicitation_request`, `reported_error`, `TOOL_REQUEST_MAX_CHARS`,
`transcript_reply`, `condenser_verdict`, `condenser_prompt`, `moshi_subcommand`, `is_harness_subcommand`)
to `pns-adapters/src/harness/{payload,transcript,condenser}.rs`, and `hook_mode`, `start_of_turn`,
`turn_marker`, `end_of_turn`, `failed_turn`, `turn_reply`, `transcript_tail`, `consume_turn_marker`,
`rendered_plainly`, `model_switch_detail`, `config_field`, `config_source_label`, `config_change_detail`,
`record_policy_settings_change`, `arm_quota_stale_wait`, `read_payload`, `payload_is_whole`,
`payload_deadline`, `env_deadline`, `reread_*`, `pulse_threshold_secs`, `project_of` and the
`REPLY_MAX_CHARS`, `TRANSCRIPT_TAIL_BYTES`, `*_REREAD_*`, `MAX_PAYLOAD_BYTES` constants
(`src/main.rs:263-680`, `2075-2319`, `2608-2737`) into `pns-cli/src/hooks/{dispatch,turn,observation}.rs`
mapping each of the eleven words onto `SubmitNotification`, `RequestApproval` or `RunNag`'s clear.
`condense`, `condenser_home`, `git_branch` become the adapters in PR 14.6. Unpinned first: S054 (empty
stderr on a healthy hook, and the bare warning spelling), S065 (`plan-ready`), the `asking`/`blocked`
condenser verdict starting a wait (`hook-compatibility.md` behavior 14). Tests: the 32 `hooks.rs` tests
by name; `tests/hooks.rs` as acceptance. Sizes: adapters three files of 120 to 260 plus tests under 400;
cli three files of 150 to 280. Statements: S024, S047 to S073.

**PR 8.3 the shell notifier moves into pns.** New behavior, operator ruling 2026-09-03. Adds `--elapsed
<secs>` to the producer flags (refused beside `--long-running`), and `pns shell begin` and `pns shell end
--exit <code> --elapsed <secs>`, owning the 30 s and 300 s tiers, the interactive TUI skip list and the
`lights-shell/<pid>` marker; `dot_bashrc.tmpl:454-595` becomes two calls;
`test/unit/pns-shell-lights-marker.bats` is deleted in the same change in favour of Rust tests over the
moved logic; `USAGE` gains two lines and the argv differential two rows. Tests first: the eleven bats
behaviors re-expressed as unit tests over the tier and marker policy in `pns-domain/src/shell.rs`, plus
one dispatch acceptance per verb. Sizes: domain ~120 plus tests ~250; cli `shell.rs` ~120. Statements:
S207 to S211 (their bash pins are retired in this PR and named in the baseline mapping).

ALSO A DELIVERABLE OF THIS PR: `pns --version`. There is no version handler today, and there never
has been. `is_producer_argv` rejects the word, so `pns --version` prints `USAGE` and exits 2
(`src/main.rs` dispatcher, `src/invocation.rs::is_producer_argv` at main `52eaeab8`), and Cargo's
`0.1.0` is never emitted anywhere. The second consumer of `--elapsed` is `webdavis/pns.nvim`, the
editor-side producer in its own repository, which carries no thresholds. Its advisory
`:checkhealth pns` comparison needs a version to read; `report()` does not check that version or
withhold `--elapsed`. Nvim overhaul task 26 waits for this PR before wiring the plugin, and sets
its `minimum_version` to this release. Three things this PR states rather than assumes: the word
`--version` (and `-V`) is answered by the dispatcher and exits 0; the output is ONE line of semver on
stdout and nothing else, so a caller can compare it without parsing prose; and the version this PR
ships is the FIRST that carries `--elapsed`, recorded here as the minimum `pns.nvim` pins. `USAGE`
gains a third line for it, and the argv differential a third row.

**PR 8.4 the Codex installer and the Claude hook table, verified.** No code moves. Runs
`test/unit/pns-codex-install-hooks.sh` and reads `private_dot_claude/modify_settings.json:325-387`
against the cli's hook table, recording in the completion report that the eleven words still map.
Statements: S213, S214.

### Step 9: registries in place of central name-based dispatch

**PR 9.1 the destination registry.** New behavior with a pure-move core. The seven name switches of S132
are replaced: `deliver_leg`'s `match leg.name` becomes a lookup in a `Destinations` registry built at the
composition root from the same `ROSTER` order; `dispatch_legs`'s `mobile` gate becomes the mobile
destination's own refusal at construction; `durable_route` is read off the `durable` declaration;
`deliver_recap` asks the registry for the durable destination; `enabled_hue_table` and
`plugin_settings(config, "hermes")` become `settings_for(plugin)`; the doctor pairs by the registry's
names. Tests first: a registry test that a destination added to the roster with no match arm is still
dispatched (the mutant that kept the old switch fails it). Sizes: `pns-application/src/destinations.rs`
~180 plus tests ~250. Order: it edits `dispatch_legs`, which PR 6.2 moves, so it lands after 6.2 or
before it, never beside it. Statements: S128, S129, S131, S132, S137, S148.

**PR 9.2 the recording decorator.** New behavior (ruling 4 of 2026-09-03). A `Recorded<D>` wrapper over
`NotificationDestination` at the composition root writes each leg's outcome to the decision record and
the delivery ledger of PR 11.4, FAIL-QUIET: a sink that cannot write never turns a `Delivered` into a
`Failed`. Tests first: the fail-quiet property with a sink that errors, and that every registered
destination is wrapped (the mutant that constructs one bare fails it). Sizes: ~120 plus tests ~200.
Order: after 9.1.

### Step 10: the stateful indicator split into policy and infrastructure

**PR 10.1 the hue adapter.** Pure move. `inventory`, `hue_settings`, `quiet_window` (the TOML read),
`pulse_body`, `breath_arm_body`, `fade_body`, `clear_body`, `clear_held`, `held_render`, `pulse_render`,
`HuePulse`, `signal_fixtures`, `UNMAPPED_SIGNAL_DURATION_MS`, `DEFAULT_ROOMS`, `UreqBridge`,
`BRIDGE_DEADLINE`, `TYPED_COMMAND_DEADLINE` (`src/channels/hue.rs`, the remainder after PR 5.8) to
`pns-adapters/src/hue/{inventory,bodies,pulse,bridge}.rs`. Unpinned first: S232 (the real transport:
certificate handling, redirect refusal, the timeout; written against a local TLS listener, or recorded as
accepted with the reason). Sizes: four files of 100 to 260 plus tests under 400. Statements: S219, S220
(bodies), S222 (`inventory`), S229, S232.

**PR 10.2 the presence poll adapters.** Pure move. `presence_hue.rs` (the `grouped_motion` read),
`presence_instant.rs` (the bridge's timestamp grammar), `presence_lock.rs` (the `flock`),
`presence_file.rs` (the state-file line codec), the presence policy's `presence_journal.rs` (the
`presence-decisions` ring codec), and `presence_mode`, `presence_launch`, `presence_poll`,
`write_presence_reading`, `Polled` (`src/main.rs:5238-5457`) into
`pns-adapters/src/presence/{bridge,instant,lock,state_file,journal}.rs` and
`pns-application/src/poll_presence.rs`. Tests: by name, including `presence_hue/tests.rs` and
`selection_tests.rs` as they are. Sizes: five adapter files under 250 plus tests; the use case ~150.
Statements: S045, S187, S188, S233.

### Step 11: semantic repositories and their persistence

**PR 11.1 the file protocols library.** Pure move. `state_dir`, `publish_state_line`, `append_ring_line`,
`claim_ring_lock`, `republish_after`, `HeldLock`, `RING_LOCK_*`, `RING_READ_MAX`, `ACTIVITY_READ_MAX`,
`STATE_FILE_MODE`, `readable_state_file` (`src/main.rs:732-803`, `1794-2057`, `src/system.rs:367-382`)
into `pns-adapters/src/persistence/{state_dir,publish,ring,locks}.rs`. Tests: the ring and publish rows
of `tests/dispatch.rs` as acceptance; `main.rs`'s own ring tests by name. Sizes: four files of 80 to 200
plus tests under 350. Statements: S151 to S154, S191.

**PR 11.2 the ring repositories.** Pure move of the codecs onto the PR 6.1 ports: `decision_log::line`
(the decision ring), `missed_notifications::entry` and `entries` (the journal and the activity ring),
`lights::render_streak`/`parse_streak`, `render_news`/`parse_news`,
`render_held_token`/`parse_held_token`, `muted_entries`/`render_muted`, the presence journal codec, and
the `policy-settings-audit` writer, each as `impl <Port> for FileRing` in
`pns-adapters/src/persistence/rings/{decisions,journal,activity,lights,presence,audit}.rs`, keyed by the
file names (`DECISIONS`, `MISSED_NOTIFICATIONS`, `ACTIVITY`, `LAST_PRESENT`, `LIGHTS_*`,
`POLICY_SETTINGS_AUDIT`, `STALENESS_MEMORY`, `QUIET_UNTIL`). These `FileRing` adapters are transitional:
PR 11.3 lands the SQLite store section 8 settles and PR 12.1 moves the records into it; the port shape is
the same either way, which is why the ports land first. Tests: the codec tests by name; the ring rows of
`tests/dispatch.rs` as acceptance. Sizes: six files of 80 to 220 plus tests under 300. Statements: S157,
S158, S160, S161, S167 to S173, S177, S178.

**PR 11.3 the SQLite store.** New infrastructure behind the PR 6.1 ports, settled in section 8. Decision
record 0012 (the two fail directions) lands first in the same PR, then
`pns-adapters/src/persistence/sqlite/{store,migrations,rows}.rs`: one file at
`~/.local/state/pns/pns.db`, `rusqlite` with the bundled feature so the build stays `--locked` and
offline, WAL mode, a bounded busy timeout, versioned migrations, explicit transactions, and one store
type implementing the repository ports the `FileRing` adapters of PR 11.2 implement. Nothing is switched
over here; PR 11.4 is its first writer and PR 12.1 moves the existing records in. Tests first: every port
test of PR 6.1 against a temporary database; a locked store on the delivery path is fail-open (the
delivery proceeds and the miss is written to the daemon log); a locked store under an ownership write is
fail-closed (the caller is told it failed). Sizes: three files of 120 to 260 plus tests under 400.
Statements: none moved; new behavior.

**PR 11.4 the delivery ledger (write-ahead journal and outbox).** New behavior, rulings 2 and 3 of
2026-09-03, and the fix for the highest-cost finding of the 2026-09-03 review (S159); its rows live in
the SQLite store of PR 11.3. `SubmitNotification` writes one ledger row per event BEFORE dispatch
(request id, legs planned, outcome unknown), the recording decorator (PR 9.2) marks each leg's outcome,
and `was_missed` becomes a question about OUTCOMES: a leg the destination confirmed is delivered, a leg
that failed or timed out stays in the ledger as undelivered, and the daemon (PR 6.9) drains undelivered
rows as leased retry jobs off the hot path. At-least-once is documented in decision record 0013, and the
replay carries the original request id (PR 7.3). Tests first: the crash window (kill between dispatch and
record) now yields a retry rather than a loss; a delivered leg is never replayed; a duplicate on retry
carries the same id. The tests that pinned the loss window as intended behavior
(`the_claim_never_survives_the_run_whether_the_replay_delivered_or_not`,
`a_delivered_event_journals_nothing_at_all`) are replaced by name in the baseline mapping with their
successors and the reason. Sizes: `pns-adapters/src/persistence/ledger.rs` ~250 plus tests ~400; the
`SubmitNotification` change ~40 lines. Order: after PR 6.2 and PR 11.3. Statements: S159 and the
successors of S158, S242, S243.

**PR 11.5 the remaining filesystem protocols.** Pure move of the protocols that stay protocols because
another process is the other party: the spool (`spool_entries`, `peek`, `claim`, `hand_back`,
`publish_if_absent`, `publish_job`, `cancel`, `marker_exists`, `prepare_spool`, `publish_heartbeat`, the
TAB codec, `WORKING_PREFIX`, and `validate_shape` and `validate_registration`, which PR 5.6 left behind
because the first caps the RENDERED record and the second calls it, from `src/daemon.rs`), the journal
claim and hold protocol (`claim_by_rename`, `take_claim`, `stranded_claims`, `abandoned_hold`,
`owner_is_gone`, the window claim), the nag records and fire lock (`nag_dir`, `record_path`,
`claim_path`, `render`/`parse`, `claim_record`, `claim_fire`, `claim_lock`, `publish_lock`,
`lock_aged_out`, `release_fire`), the marker directories (`lease_dir`, `lease_marker`, `blocked_dir`,
`blocked_marker`, `sweep_claim`, `sweep_markers`, `sweep_leases`, `sweep_shell_markers`,
`sweep_legacy_state`), the turn marker claim, and the setup publish (`publish_config`, `pending_name`,
`write_then_publish`, `keep_aside_at`) into
`pns-adapters/src/protocols/{spool,claims,nag,markers,turn,config_publish}.rs`. Each keeps the
decision-0001 invariant as a one-line comment linking the record. Unpinned first: S165 (recorded as
accepted; the source says no test can plant it), S183. Tests: by name; the claim rows of
`tests/dispatch.rs`, `tests/hooks.rs` and `tests/daemon.rs` as acceptance. Sizes: six files of 120 to 280
plus tests under 450. Statements: S155, S156, S162 to S166, S174 to S176, S179 to S186, S190.

### Step 12: durable state, migrated or preserved

**PR 12.1 the durable records move into the store.** Migrates every durable record that has no reader or
writer outside the binary (section 8 classifies them): the five rings, `last-present` with its window
claim as a row claim inside one transaction, `quiet-until`, `home-staleness`, `lights-quiet`,
`lights-said`, `lights-quiet-said` and `lights-news`, each `FileRing` or file reader of PR 11.2 replaced
by the PR 11.3 store behind the same port. The first run imports what it can read from the old files and
decision record 0014 says what a failed import costs (at most one recap window; the rings are bounded at
5, 25, 150, 20 and the presence ring's own depth). The files another party writes or reads (the config,
`lights-shell/<pid>`, `phone-attention.marker`), the coordination protocols of PR 11.5 and the daemon
heartbeat stay files. `sweep_legacy_state` stays until decision record 0015 retires it. Tests: the port
tests already run against the store; the import path with a readable, an unreadable and an absent old
file. Sizes: `persistence/import.rs` ~150 plus tests ~250. Statements: S157, S158, S160, S161, S167,
S168, S171, S177, S178, S180, S191.

### Step 13: configuration split into parsing, validation, schema, rendering, setup, publication

**PR 13.1 the schema roster.** Pure move of `TABLE_KEYS`, `TARGET_KEYS`, `TOP_LEVEL`, `SAMPLE_VALUES`,
`admits`, `admits_flat`, `unknown_key` (`src/config.rs:524-664`, `782-833`) to
`pns-adapters/src/config/schema.rs`; the documented-keys scanner (`736`) goes with it under `cfg(test)`.
Tests: the two roster walks by name. Sizes: ~200 plus tests ~150.

**PR 13.2 loading and outcomes.** Pure move of `Config`, `DEFAULT_DAEMON_ENABLED`, `NAG_OFF`,
`ConfigError`, `LoadOutcome`, `config_path`, `parse_config`, `backstop_outlasts_the_nag`, the per-table
parsers `parse_recap`, `parse_focus`, `parse_daemon`, `parse_nag`, `nag_schedule`, `MIN_NAG_AFTER_SECS`,
`MAX_NAG_AFTER_SECS` and `modes` (`src/config.rs:834-1258`), the value readers `text`, `bounded`, `flag`,
`threshold`, `seconds`, `argv`, `repositories`, `strings` and `note_glob` (`1580-1839`), `load_config`,
`armed_mobile` and `submit_deadline` (`1840-1945`), and the types and constants they build, `PluginEntry`
and `Recap` (`32-120`) and `DEFAULT_MIN_EVENTS` through `MAX_SUBMIT_DEADLINE_SECS` (`437-523`), to
`pns-adapters/src/config/{load,values,plugins,recap,nag}.rs`. Unpinned first: S285 (no read deadline;
written as a FIFO-at-the-config-path test that must not park, and if it parks today the bound is added in
its own PR before this one, never inside the move). It is also where the duplicated test fixture from
PR 5.3 is reunited: about 25 lines of config-building setup are spelled twice today, in the domain's
registry tests and in the root's registry and routing tests, and moving the parser alone does not merge
them. This step consolidates the config-free policy cases in the domain and keeps the parsed-config
integration checks with the adapters. Sizes: five files of 100 to 260 plus tests under 450 each (the
config tests split by table). Statements: S078, S116, S189, S236, S276, S280, S284, S285.

**PR 13.3 the lights tables.** Pure move of `Lights`, `Pulse`, `Blocked`, `Unread`, `Looping`, `Target`,
`BEHAVIOUR_WORDS`, the locked defaults, `percent`, `ends_agree`, `accent_agrees`, `behaviour_table`,
`behaviours`, `breath_key`, `parse_lights`, `parse_pulse`, `parse_breath`, `parse_blocked`,
`parse_unread`, `parse_looping`, `parse_targets`
(`src/config.rs:121-436`, `1259-1579`, reading `bounded` from the `values.rs` of 13.2) to
`pns-adapters/src/config/lights/{tables,bounds,targets}.rs`. The plain value types `Pulse` and `Target`
land in `pns-domain/src/lamps/config.rs` because the lamp policy reads them. `Behaviour` already lives
there, and PR 5.7 moved `Breath` and `BreatheThenFlare` there for the breath policy. Their parsers
remain part of this step. Tests: by name, split by table. Sizes: three files of 150 to 250 plus tests
under 450. Statements: S277, S278.

**PR 13.4 the plugin tables that select a backend.** Pure move of `parse_presence`, `presence_count`,
`Presence`, the `desk_room` and `desk_stale_after_secs` bounds and the presence constants
(`src/config.rs:1946-2177`), and `home.rs`'s `router_settings`, `device_identity`, `router_api_key`,
`stale_alert_channel`, `enabled_router_table`, `read_device_key`, `spell`, `SetupFailure`, and
`moshi.rs`'s `mobile_backend`, `moshi_secret`, `hermes.rs`'s `hermes_secret`, into
`pns-adapters/src/config/{presence,router,mobile,hermes}.rs`. Tests: by name. Sizes: four files under 200
plus tests under 300. Statements: S137, S140 (the secret read), S271, S279.

**PR 13.5 rendering.** Pure move of `src/config_text.rs` (`LAYOUT`, `Table`, `Key`, `Sample`, the prose
constants, `quoted`, `quoted_list`, `render`, `render_core`, `render_opt_in`, `render_block`,
`render_lights`, `render_target`, `take_note`, `write_note`, `render_value`, `secret_action`,
`SECRET_FIELDS`) to `pns-adapters/src/config/render/{layout,prose,render,secret}.rs`, and
`strip_chezmoi_actions`, `identity_placeholder` (`src/config.rs:665-735`) from `config.rs` to
`render/stub.rs`. Tests: 32 by name, split. Sizes: four files of 150 to 280 (the `LAYOUT` constant alone
is ~550 lines of prose today and becomes `layout.rs` plus `prose.rs`, each under 300 once the prose moves
to `docs/`-linked one-liners where the operator-facing comment is not itself the contract; the shipped
template's text IS a contract, so `prose.rs` may sit at the cap and is measured in the PR) plus tests
under 400. Statements: S275, S281.

**PR 13.6 setup composition.** Pure move of `pns-domain/src/setup.rs`'s `Answers::values` and
`compose_config` onto the render adapter through a `ConfigRenderer` port declared by `RunSetup`. Sizes:
unchanged. Order: after 13.5. Statements: S267.

**PR 13.7 the out-of-crate pins leave the crate.** Decision 0011. `SHIPPED_TEMPLATE` and `CONFIG_VALUES`
(`include_str!` four directories up) and `tests/config_render.rs`'s `CARGO_MANIFEST_DIR/../../..` reach
become a repository-level test, `test/unit/pns-config-template.test.sh`, that runs `just
pns-config-render` into a scratch path and diffs it against the committed template, asserts the 22 live
headings and the five secret actions, and runs the resolved snapshot check through `pns-config-render
--check` (new flag, test-first). The crate keeps only `RESOLVED_CONFIG_SNAPSHOT`. Tests replaced by name
in the mapping. Sizes: the shell test under 150 lines; `pns-config-render.rs` ~220. Order: after 13.5.
Statements: S282, S283.

### Step 14: system, network, filesystem and process behavior into adapters

**PR 14.1 the macOS and process readers.** Pure move of `src/system.rs` (`CommandRunner`,
`SystemCommandRunner`, `run_bounded`, `wait_until`, `next_poll_interval`, `PROBE_DEADLINE`,
`PROBE_READ_MAX`, the `ioreg`, `pgrep`, `ps` paths and parsers, `phone_reading`, `newest_terminal_atime`,
`parse_focused_tab`, `parse_layout`, `SystemProbes`, `join_desk`, `local_minutes_since_midnight`,
`utc_timestamp`) and `lights::workspace_agent_statuses` to
`pns-adapters/src/{process/bounded,macos/idle,macos/lock,macos/phone,herdr/view,probes}.rs` and
`pns-adapters/src/clock.rs`. Unpinned first: none in section 4 beyond S089's already-pinned rows. Tests:
62 by name, split. Sizes: seven files of 80 to 260 plus tests under 450. Statements: S084 to S092.

**PR 14.2 the executable destination's deadline, and the three unbounded accepts.** New behavior and test
repair, kept out of PR 14.3 so that one stays a pure move. S147: a hanging-stub test (an executable
channel that never exits) is written against `deliver` where it lives (`src/main.rs:4136-4154`), fails
today, and is closed by bounding the child the way `run_bounded` bounds every other probe; the register
of unpinned behaviors records the deadline as the finding it came from. The three hermes and moshi tests
that join a thread parked on an unbounded `accept()` (`hermes.rs:568`, `moshi.rs:632`, `moshi.rs:674`)
are rewritten with a bounded accept so a mutant fails rather than hangs. Sizes: unchanged. Statements:
S147.

**PR 14.3 the three destinations, and uu's seam.** Pure move of `src/channels/banner.rs`, `moshi.rs`,
`hermes.rs` (less the settings reads moved in 13.4) and `channels/mod.rs`'s `Event`, `Delivery`,
`native_first` into `pns-adapters/src/destinations/{banner,moshi,hermes,executable}.rs` plus `deliver`
and `resolve_path` (`src/main.rs:4102-4154`, bounded by PR 14.2) as the executable destination. uu's
`Cargo.toml:36` dependency and its three import sites (`src/delivery.rs:11`, `src/delivery.rs:99`,
`src/cli/run.rs:13`) move to the crate section 8 names in the same PR, and `cargo test --locked
--manifest-path dot_local/share/uu/Cargo.toml` is run and recorded. Unpinned first: S144 (the exact `pns:
posted HTTP 200` line as the weekly helper's contract, asserted through the capture server). Sizes: four
files of 120 to 260 plus tests under 450. Statements: S126, S128, S131, S133 to S147.

**PR 14.4 the Focus store reader.** Pure move of `src/focus.rs` (`active_modes`, `mode_names`,
`silenced`, `same`) and `focus_now`, `FocusReading`, `FOCUS_DB` (`src/main.rs:9480-9549`) to
`pns-adapters/src/macos/focus.rs`, with `silenced` and `same` in `pns-domain/src/focus.rs`. Tests: 16 by
name. Sizes: adapter ~150 plus tests ~350; domain ~60. Statements: S105.

**PR 14.5 the UniFi router.** Pure move of `parse_clients`, `first_site_id`, `Router`, `UniFiRouter`,
`read_home`, `ROUTER_DEADLINE`, `ROUTER_BODY_CAP` to `pns-adapters/src/unifi/{client,parse}.rs`. Tests:
by name. Sizes: two files under 200 plus tests under 400. Statements: S272, S273.

**PR 14.6 the spawned programs.** Pure move of `spawn_moshi_hook`, `moshi_hook_bin`,
`DEFAULT_MOSHI_HOOK_BIN` (the approval forwarder), `condense`, `condenser_home`, `CONDENSER_DEADLINE`
(the condenser), `git_branch`, `GIT_DEADLINE`, `merged_pull_requests` and the `GH_*` constants,
`notes_matching`, `matches_glob`, `within`, `read_note`, `MAX_NOTES`, `NOTE_READ_MAX`, `summarize`,
`spawn_job` and `spawn_recap`'s spawn, and `Hushed` with `ask_hidden`'s terminal work, to
`pns-adapters/src/{moshi_hook,codex,git,gh,notes,summarizer,job_runner,recap_child,terminal}.rs`.
Unpinned first: S192 (the condenser home's modes), S254 (the summarizer's inherited environment: recorded
as accepted or closed with `env_clear` as new behavior, the operator's call inside the PR). Tests: by
name; the spawn rows of `tests/hooks.rs` and `tests/dispatch.rs` as acceptance. Sizes: nine files of 60
to 220 plus tests under 300. Statements: S056 to S059, S076, S192, S201, S245, S248 to S250, S264.

### Step 15: the executables reduced to command adaptation and composition

**PR 15.1 the binary moves to `pns-cli`.** Moves `main` (`src/main.rs:48-161`) and every remaining
`*_mode` presentation (`home.rs`'s `report` and `setup_report`, `decision_log`'s `section`, `render`,
`complaint`, `escaped`, the doctor's printing) into `pns-cli/src/{main,compose,present/*}.rs`; `[[bin]]
name = "pns"` moves to `crates/pns-cli/Cargo.toml`; the root manifest becomes a virtual workspace with
`default-members = ["crates/pns-cli"]` so `cargo build --release --locked --quiet --bin pns
--manifest-path dot_local/share/pns/Cargo.toml` still resolves;
`.chezmoiscripts/run_onchange_after_58-build-pns-engine.sh.tmpl:87` and
`test/unit/pns-engine-build-install.sh` are updated and run in the same PR; `pns-config-render` and
`http-capture` move to `pns-cli/src/bin/`. Tests: the argv differential is the whole proof, with its
control mutant; every integration suite runs against the new binary path. Sizes: `main.rs` under 120,
`compose.rs` ~250 (the one place every adapter is constructed), presentation files under 200. Order:
after every step above. Statements: S001 to S046 (the dispatch order is the contract).

**PR 15.2 the legacy package is emptied.** Deletes what is left of `src/` in the root package, the
`pub use` re-exports of step 5, and the root `[package]`; the workspace root is manifest-only. The
name set is unchanged; the argv differential is unchanged. Sizes: `src/` is gone.

### Step 16: unit and acceptance tests split by behavior

**PRs 16.1 to 16.6.** Pure moves of `tests/dispatch.rs` (8,581 lines) into `tests/dispatch/` as twenty
files named for the specification areas they pin (`producer_argv.rs`, `plan_rows.rs`, `overrides.rs`,
`mutes.rs`, `quiet_window.rs`, `records.rs`, `journal.rs`, `replay.rs`, `return_moment.rs`, `recap.rs`,
`channels.rs`, `hermes_lines.rs`, `pulse.rs`, `lights_tick.rs`, `lights_quiet.rs`, `loop_lease.rs`,
`home.rs`, `doctor.rs`, `doctor_sections.rs`, `setup.rs`), of `tests/hooks.rs` (6,217) into
`tests/hooks/` as fourteen (`payload.rs`, `prompt.rs`, `stop.rs`, `condenser.rs`, `stop_failure.rs`,
`approval.rs`, `gate.rs`, `markers.rs`, `resolved.rs`, `model_switch.rs`, `quota.rs`, `config_change.rs`,
`nag_arm.rs`, `nag_fire.rs`), of `tests/support/mod.rs` (849) into `sandbox.rs`, `daemon_guard.rs`,
`stubs.rs`, and of `tests/daemon.rs` and `tests/setup.rs` into two each. Six PRs, one or two source files
each, so a reviewer can diff moved blocks by name. The name set is the whole proof. Order: last, so no
step above rebases against a moved test file.

### Step 17: obsolete compatibility code and mechanism tests removed

**PR 17.1.** Deletes the step-5 re-exports (done in 15.2 if not before), the `#[ignore]`d soak tests
that duplicate deterministic siblings only where the baseline's classification says so, and the
mechanism tests the baseline classified obsolete, each named in the mapping table with its successor
and reason. Adds the `pns-hermes` (or `pns-adapters`) import path to uu's manifest comment.

### Step 18: every gate and the file-size check

**PR 18.1.** Adds `scripts/treefmt/rust-file-size.sh` to `treefmt.toml` as a check-only formatter
over `dot_local/share/**/*.rs` failing above 500 lines (a lint, not a test, so it sits in
`just lint-check`), runs `just ship`, `cargo doc --workspace` with `RUSTDOCFLAGS="-D warnings"`
(backlog B75, three unresolved intra-doc links on `main` today), and writes the completion report
the `clean-code` skill enumerates (nineteen items), including before-and-after line counts for every
file in section 4 and the name-mapping table.

## 6. The findings of the 2026-09-03 delivery-path review, and the step that closes each

- `was_missed` asks the plan, not the outcomes (S159): PR 11.4, through PR 9.2's decorator and PR
  7.1's request id.
- Hermes, the blocked flash and the approval forward are competing authorities: PR 6.2 orders them
  in one use case, and PR 6.3 names the forward's surface-only read as the one deliberate exception.
- One instant is not one observation (the clock is read before the slow probes, the view after):
  PR 6.1's `environment_reading.rs` takes one snapshot after every probe has joined; the presence
  policy's snapshot-at-decide (`7c58f94b`) is the model.
- Failure direction is global where it must be per destination: PR 5.11 has the `Decision` carry a
  per-reading direction, then PR 9.1 has each destination state its own.
- The crash windows are open (delivery before the record; the claim deleted before the replay):
  PR 11.4 writes ahead, and PR 11.5 keeps the hold until the outcome is recorded.
- Three delivery-path tests can hang instead of fail: PR 14.2.

## 7. The frozen backlog, absorbed

Every open pns item in `~/.claude/pipeline/backlog-consolidated-2026-09-02.md` lands in a step or is
re-filed in the completion report; none is dropped.

- B1 (pin the bridge certificate): a decision the hue adapter PR 10.1 records; the code change is
  new behavior after the ladder unless the operator rules otherwise.
- B4 (Codex prompt parity): PR 8.4 records it; the wiring is a Codex installer change, outside the
  crate, re-filed.
- B5 (`session-<id>.start` litter sweep): PR 11.5's markers protocol gains the sweep as new behavior
  with its test.
- B6, B20, B39 (the elicitation sidecar, `asked` on PreToolUse, the network permission alert): design
  rulings, re-filed; the hook adapter of PR 8.2 is where they would land.
- B11 (`gate_mode` says nothing on a refused word; `hook_mode` reads stdin before validating): PR 8.2
  keeps both behaviors as specified (S023, S024) and records them as deliberate.
- B12 remainder and B30 (`home` and `lights tick` trailing words; the `pulse` usage mismatch): PR 8.1.
- B13 (the scripted fakes drive policy tests): PR 6.1 moves the fakes to the port level.
- B15 (`main.rs` size): the whole ladder.
- B16, B28, B29 (the wizard writes a managed target; a directory at the config path; a failure after
  `keep_aside`): PR 6.12 and PR 11.5, each as new behavior with a test.
- B17 (a dead bridge resolve on the event path for held behaviours): PR 6.13.
- B18 (mute versus held lamps): a design ruling, re-filed; PR 6.7 is where it lands.
- B19, B25 (nag one-second tolerance; `age_of` clamping a future epoch to zero): PR 5.5 and PR 5.11
  as new behavior with tests, if the operator rules; otherwise recorded.
- B23 (the builder hashes `target/`): the builder change rides PR 15.1.
- B24 (hook tests reach the live probes): PR 14.1 gives the tests a `SystemProbes` stub through the
  `CommandRunner` port.
- B26 (`start_of_turn`'s exists-then-write race and mode): PR 11.5's turn protocol.
- B34, B35, B37 (test plumbing): PR 16.x for the sandbox; the shell tests are re-filed.
- B36 (extract decide-plus-marker out of `run_event`): PR 6.2.
- B40 (the runbook's hook count): a docs fix, re-filed.
- B41 (the template guard enumerates tables not keys): PR 13.7's `--check`.
- B51 (the specification pass's live findings: eleven echoed prompts, the unscrubbed argv and
  transcript paths to a channel, the hue key never asserted absent): PR 6.12, PR 8.1, PR 10.1.
- B67 (the blocked-hook-to-moshi CI flake): PR 6.3's use-case test replaces the timing-sensitive
  acceptance row.
- B75 (`cargo doc -D warnings`): PR 18.1.

## 8. What is settled, and the one decision the operator makes before code moves

**Settled: the rings and the ledger move to SQLite.** The charter requires a transactional SQLite
adapter for internal durable multi-record state unless a file's path, name, metadata or existence is
itself an external integration contract, and the review of the charter kept that choice (its D3,
"keep SQLite"), so it is recorded here rather than reopened. The store is one file at
`~/.local/state/pns/pns.db` through `rusqlite` with the bundled feature (the build stays `--locked`
and offline), WAL mode, a bounded busy timeout, versioned migrations and explicit transactions, landed
by PR 11.3. Two fail directions, written as decision record 0012 before the first SQLite code: the
DELIVERY PATH is fail-open, a busy, locked, missing or corrupt store never blocks a delivery and the
miss is recorded where recording is possible; a STATE MUTATION is fail-closed, an ownership or
acknowledgement write that fails is reported as a failure to the caller that asked for it, never
passed off as success. The recording decorator's fail-quiet rule (PR 9.2) is the delivery-path case
of the same rule: a sink that cannot write never turns a `Delivered` into a `Failed`, and the failure
to record is itself written to the daemon log and surfaced by `pns doctor`.

What migrates and what stays is classified by the readers and writers that exist outside the
binary, not by whether a name is read today. Outside the binary: the shell notifier writes
`lights-shell/<pid>` (`dot_bashrc.tmpl:488`), chezmoi writes the config file, an outside program
touches `phone-attention.marker` (S170), launchd owns the daemon log; those stay files. Coordination
that owns by rename or `O_EXCL` because concurrent `unlink` does not arbitrate (the spool's `~claim`
and `~pending` files, the ring locks until their rings move, `nag/fire.lock` and the nag claims, the
leases, the `.new.<pid>` and `.sweep.<pid>` working files, the turn markers, the lamp markers) stays
a filesystem protocol under `pns-adapters/src/protocols/` (PR 11.5), and so does the daemon
heartbeat, because it is what says the daemon is alive when the store is the thing that is broken.
Every other durable record has no reader outside the binary and migrates in PR 12.1: the five
rings, the delivery ledger, `last-present` (its window claim becomes a row claim in one
transaction), `quiet-until`, `home-staleness`, `lights-quiet`, `lights-said`, `lights-quiet-said`
and `lights-news`. The ports of PR 6.1 are the same either way, which is why they land first.

**Decision: a sixth crate for the signed-POST client that uu shares.** uu imports six items from
`pns::channels::hermes` today (`src/delivery.rs:11`, `src/delivery.rs:99`, `src/cli/run.rs:13`).
After PR 14.3 they live in `pns-adapters`, and a path dependency on `pns-adapters` drags the Hue,
UniFi, macOS, process and persistence adapters into uu's build. Recommendation: add
`crates/pns-hermes` (the `SignedPost` trait, `UreqSignedPost`, `sign`, `PostOutcome`, `delivered`,
`outcome_line`, `skipped_line`, `channel_url`, `remote_deadline`, no workspace dependencies), have
`pns-adapters` depend on it, and point uu at it in PR 14.3. The alternative, uu depending on
`pns-adapters` whole, needs no new crate name but couples uu's build to every adapter pns ever grows.

One standing rule is stated rather than decided: the 300/500 ceiling binds `tests/*.rs` as much as
`src/*.rs`, because the ruling says "unit tests INCLUDED, no waiver" and the charter mandates the
acceptance suites' decomposition; that is what makes step 16 six PRs.

## 9. Reading order for a reviewer

The specification's section numbers map onto the ladder: sections 1 and 2 land in step 8, section 3
in 6.3, section 4 in 5.2, 5.11, 6.1 and 14.1, section 5 in 5.3 and 9.1, section 6 in 14.3 and 10.1,
section 7 in 11.x, section 8 in 6.9, section 9 in 8.3, sections 10 to 17 in the use case named for
each. A PR's description names the statements it moves by number, so the reviewer opens the
specification at those numbers and the pins named there are the tests the PR must carry across.
