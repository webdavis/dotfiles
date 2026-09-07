# lights: implementation plan

Companion to `docs/superpowers/specs/2026-09-06-lights-design.md`. The tool and controller names are
settled. This plan specifies future implementation; it does not authorize a live bridge call or apply.

Read both canonical standards: `~/.agents/skills/clean-code/SKILL.md` and
`~/.agents/skills/clean-code-rust/SKILL.md`. Rust wins every conflicting number and mechanism, including
crate roles, dispatch, visibility, test placement and quality gates.

## The crate

Source at `dot_local/share/lights`, deployed to `~/.local/share/lights`, built at apply time. It is
designed as a standalone package from the first commit: nothing outside its own folder dictates its
shape, and no path inside it reaches outside the folder. That is what lets it move to its own repository
later without a rewrite.

The final workspace has the five roles required by the Rust standard. Add a member only when its first
behavior lands; early slices do not create empty crates or placeholder implementations.

```
dot_local/share/lights/
  Cargo.toml                  virtual workspace root
  Cargo.lock                  committed; all builds use --locked
  crates/
    lights-domain/            rotation, brightness, aliases, Action; no input/output
    lights-application/       use cases and the LightController and Notifier ports
    lights-protocol/          command grammar, output records and exit-code contract
    lights-adapters/          HueLightController, PnsNotifier, settings parsing and loading
    lights-cli/               process arguments, streams and composition; binary named lights
      src/main.rs             under 100 lines preferred, below 150 required
      tests/                  assembled command tests, fixtures owned by this member
```

The member manifests enforce these final dependency edges:

```
lights-domain      -> std only
lights-application -> lights-domain
lights-protocol    -> std only
lights-adapters    -> lights-application, lights-domain
lights-cli         -> lights-protocol, lights-adapters, lights-application, lights-domain
```

`lights-protocol` owns the external command grammar and output contract, with no dependency on policy.
The command-line adapter translates between its request/output values and the application. This does
not introduce a daemon or a new serialized request envelope. The outbound pns producer arguments remain
pns's contract; lights neither shares a crate with pns nor moves either controller into it.

`main.rs` loads settings, constructs concrete implementers, calls the command adapter and returns its
exit code. Domain policy, decoding and rendering belong in their named modules.

### Dependencies

`ureq` with rustls for bridge calls, `serde_json` for resource payloads, and `toml` with parsing only
for the config. Taken independently of pns, which happens to take the same three, because the tools share
no crate.

### The file-size rule

No handwritten `.rs` file exceeds 500 total lines including tests. Targets are 200 implementation and
300 total lines; 250 implementation or 400 total normally requires decomposition by responsibility.
Use the canonical Rust skill's physical-line command after formatting. Public exports are intentional
crate interfaces; unit tests live in private `#[cfg(test)]` modules beside their implementation.
Ordinary external failures return typed errors, never panic. The Rust skill permits a panic only for a
compiled-in invariant that cannot depend on operator input or runtime conditions.

## The domain

Four pure pieces, each independently testable with no bridge, no config file and no clock.

**`Rotation`** holds the ordered scene list and the fallback. `next(current)` and `previous(current)`
return the scene to activate. Construction rejects an empty list or a fallback outside it.
`current` is an `Option<&str>`, and both `None` and a name the rotation
does not hold return the fallback. The backward step is `(index + len - 1) % len`; writing it as
`(index - 1) % len` underflows a `usize`, which is the trap the bash avoids only because bash indexes
arrays from the end on a negative subscript.

**`Brightness`** is a newtype over `u8` with a private field and a smart constructor that clamps to 1
through 100. Parse numeric input as a wider unsigned integer before clamping and narrowing. There is no
way to build a request level outside that range. `ReportedBrightness` separately preserves finite,
fractional readings from 0 through 100; missing readings stay absent. The 1% request floor is not a
claim about the device's physical minimum.

**`Aliases`** maps a short name to a room name and passes an unlisted name through unchanged.

**`Action`** is the typed outcome a use case returns: `PowerSet { room, on }`,
`BrightnessSet { room, level }`, `BrightnessStepped { room, direction }`, `SceneSet { room, scene }`,
`Reported { room, on, brightness, scene }`. The CLI (command-line interface) renders it for the terminal
and the notifier renders it for pns. Neither the use cases nor the domain build a display string.
Tests assert typed outcomes unless the output itself is the contract. `BrightnessSet` renders the
requested percentage explicitly;
`BrightnessStepped` has no result level and renders direction only. A missing static scene renders
`unknown`, while failed scene acquisition returns an error instead of a `Reported` action.

## The application

Use cases use compile-time composition, for example `TogglePower<'a, C: LightController, N: Notifier>`
holding `&'a C` and `&'a N`, and return `Result<Action, LightsError>`. Status needs no notifier
parameter. Concrete types and generics are the default. `dyn Trait` is permitted only for a heterogeneous
collection at the composition root under the Rust skill; no such collection is needed here.

The completed use cases are:

`TogglePower`, `SetPower`, `AdjustBrightness`, `SetScene`, `ReportStatus`.

They are the only code that sequences a read against a write, and they are tested entirely against the
two recording implementers. `LightsError` maps one to one onto the exit codes in the design.

The application crate owns both trait definitions, because the consumer owns the port. The adapters crate
implements them and depends on the application, never the reverse. Its public `RoomRef` and `SceneRef`
constructors/accessors let real adapters and recording fakes mint snapshot-local indices without sharing
Hue identifiers. Use cases pass references back unchanged; invalid indices fail before any write.

## The adapters

**`HueLightController`** implements `LightController`. It owns one `ureq::Agent` for the process,
memoizes the single `GET /clip/v2/resource` behind the trait's read methods, builds the four PUT bodies,
and validates status plus the response envelope before caching or returning success. A transport
failure is `Unreachable`; an unsuccessful status or nonempty `errors` array is `Refused`, even for a
successful status with both data and errors. Missing/mistyped `errors` or `data`, malformed JSON
(JavaScript Object Notation), or incomplete required resources are `Malformed`. Each maps to exit 4,
with no success output or notification. Error text excludes credentials and raw response bodies.
The proposed certificate-verification exception needs source verification as stated in the design;
keep its rationale at the adapter call site, without claiming secure verification is impossible.

**`PnsNotifier`** implements `Notifier`. It renders an `Action` into a detail string and invokes the
installed `gtimeout --foreground --signal=KILL 2s` with the pns executable and producer arguments,
without a shell. Coreutils is already declared; no package or Rust dependency is added for this.
The adapter owns and reaps the monitor synchronously; the monitor owns the direct pns child, kills it
at the deadline and reaps it before returning. Null stdin, stdout and stderr prevent inherited input
or output pipes from extending that lifetime. There is no detached waiter, custom watchdog or retry.

Match the spawn/wait result and every status explicitly as specified by L021. Notification success,
nonzero/signal status, timeout, unavailable executable and monitor failure all preserve the already
accepted `Action`, its output and exit 0. Failures are silent. Missing `gtimeout` never selects an
unbounded fallback. The two-second production bound is fixed; adapter tests supply a shorter private
duration, without adding a public setting or reading an environment override.

The monitor owns only the direct child. It does not contain descendants; pns must own their cleanup
independently of the producer's survival. At `5cb969d0`, `dot_local/share/pns/src/main.rs`'s `deliver`
waits without a deadline, and its daemon's `kill_group` rationale records that killing a producer can
leave delivery alive. PR 10 therefore requires pns-owned evidence of bounded delivery cleanup first.
Keep notification unwired if that prerequisite is unmet; do not expand this plan into pns implementation.

**The settings parser** is a free function rather than a type. `settings::parse` reads a
TOML (Tom's Obvious Minimal Language) string. It refuses an unknown key by name, a `[controller]` table
with no `type` or an unimplemented `type`, and a missing address or key. `settings::load` is the thin
file read above it. There is no trait and no `ConfigLoader`, because settings are read once in
`main.rs` before any use case exists
and are handed down as plain values. Nothing above ever asks where they came from, so there is no seam
for a trait to sit in.

## Everything outside the crate

### The builder

`.chezmoiscripts/run_onchange_after_54-build-lights.sh.tmpl`, modeled on the uu builder at 59 rather than
the pns builder at 58: `lights` has no daemon, so there is no `launchctl kickstart`, no pending marker
and no restart logic. Slot 54 is free, and it sits just ahead of the other build scripts, which hold
slots 55, 57, 58 and 59.

It hashes every `.rs` file, the workspace and all member manifests, and the lock through globs, so a
new module cannot silently miss the trigger. It defers with a retry marker when cargo or the deployed
source is missing, because chezmoi records a `run_onchange` script as done on any zero exit. A skipped
build has to change the rendered script to fire again. It builds with the line below from the deployed
workspace and installs with `/usr/bin/install -m 755`.
Verify replacement semantics from the installed tool before claiming atomic installation.

```
cargo build --release --locked --quiet --bin lights
```

Install path: `~/.local/libexec/lights`, settled. The repository rule puts everything a keybinding,
launchd, a hook or a `just` recipe invokes under `libexec`, and pns already sits there despite being
typed by hand too.

### The config template

`dot_config/lights/private_config.toml.tmpl`, deploying to `~/.config/lights/config.toml`. It is
`private_` because it holds the bridge key. Its two secret lines:

```
address = {{ (keepassxc "OpenHue :: API Key (hue-bridge-pro)").UserName | toToml }}
key = {{ (keepassxc "OpenHue :: API Key (hue-bridge-pro)").Password | toToml }}
```

Reusing that entry is settled. It already exists and already carries both halves, and the pns config
template reads the same two fields from it, so a second entry would mean one bridge credential in two
places to rotate. The template adds a target requiring KeePassXC at apply time. Update the set in
`CLAUDE.md` in the installation slice; verify its count from that tree.

Unlike the pns template, this one is handwritten. pns generates its template from a committed values file
because its config has grown five plugin tables with argued prose in the comments; `lights` has five
short tables and no renderer, and building one would be scaffolding for a second config that does not
exist.

### Ignore entries

PR (pull request) 1 adds `dot_local/share/lights/target/` to `.gitignore` as soon as Cargo can create it.
It also updates `.chezmoiignore`, before any operator apply could deploy build output.

Source-only exclusions: `.local/share/lights/target`, `.local/share/lights/docs` and the committed test
fixtures under each member's `tests/fixtures` directory. Match deployed target names, not chezmoi source
prefixes. Then three entries in the darwin-conditional block, because the whole tool is macOS only:
`.local/libexec/lights`, `.local/share/lights` and `.config/lights`.

### The justfile

`test-rust` gains the lights manifest in PR 1, alongside the fmt and clippy lines pns and uu already get:

```
cargo test --locked --workspace --manifest-path dot_local/share/lights/Cargo.toml
cargo fmt --all --check --manifest-path dot_local/share/lights/Cargo.toml
cargo clippy --locked --workspace --all-targets \
  --manifest-path dot_local/share/lights/Cargo.toml -- -D warnings
```

### The aerospace bindings

All seven keys point at one tool, and the two that call `openhue` directly stop doing so:

```
f4  = 'exec-and-forget ~/.local/libexec/lights scene "CC Halo Daylight"'
f5  = 'exec-and-forget ~/.local/libexec/lights scene previous'
f6  = 'exec-and-forget ~/.local/libexec/lights scene next'
f7  = 'exec-and-forget ~/.local/libexec/lights scene "CC Halo Amber"'
f8  = 'exec-and-forget ~/.local/libexec/lights brightness down'
f9  = 'exec-and-forget ~/.local/libexec/lights toggle'
f10 = 'exec-and-forget ~/.local/libexec/lights brightness up'
```

`dot_aerospace.toml` is excluded from taplo, so the file's existing visual alignment is preserved by
hand.

### The known-good manifest consumer

Read `.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh` when preparing installation and
cutover. Its file set comes from chezmoi-managed intent and includes managed `libexec` files; a binary
created by a builder is not automatically a source-managed file. Review whether the new generated
binary needs a separate inventory entry before claiming coverage. Removing the Bash source changes
that managed set on the operator's next full apply. Never run this manifest writer from a test.

### The deletion, and what the operator has to finish by hand

`dot_local/libexec/executable_control-hue-lights.sh` is deleted from the source tree in the last pull
request. Deleting a chezmoi source entry does not delete the deployed file, and this repository builds no
removal mechanisms, so two things survive the apply and the operator removes them:

- `~/.local/libexec/control-hue-lights.sh`
- `~/Library/Logs/smart-lights.log`

Both are listed in the final pull request body as manual steps rather than left to be discovered.

The `openhue` formula stays declared in `.chezmoidata/system_packages_autoinstall.yaml`, settled. After
the cutover nothing in the tree calls it, but removal here is manual by standing rule, and the cutover is
not the moment to also uninstall the fallback.

## The test plan

Each test must finish in under one second, measured under `cargo test --workspace`. Use temporary
homes, scripted transports and process runners; set `GIT_CONFIG_GLOBAL=/dev/null` and
`GIT_CONFIG_SYSTEM=/dev/null` in harnesses. No test reads operator settings, calls the bridge, spawns
real pns or sends a desktop notification. Fixtures are synthetic examples derived from the retained
schemas, not a requirement to collect live bridge data. Keep them within the member that reads them.

The retained local sources are `/private/tmp/lights-protocol.q4NxtU/`: `Brightness.yaml`,
`DimmingDelta.yaml`, `Dimming.yaml`, `GroupedLightGet.yaml`, `RoomGet.yaml`, `ResourceGet.yaml`,
`resource.yaml` and `ApiResponse.yaml`. They establish the brightness caveats, room service references,
full-resource shape and response arrays. Their upstream revision is not pinned in the retained files.
Scene read/write schemas and grouped-light write endpoint declarations were not retained there; verify
those from authoritative source before implementing their adapter slices. This specification correction
makes no network requests and does not claim those missing sources were checked locally.

Run reusable `LightController` contracts against `RecordingLightController` and `HueLightController`
with a scripted transport. Exercise consumer behavior and failure mapping, not trait declarations or
manifest contents. Named cases are assigned to slices below; command cases call the actual composition
path with isolated transport/process dependencies, so replacing composition with a constant fails.

For each new behavior: run its named test before implementation and show the intended failure, make it
pass, then mutate the behavior and show that the same test fails against a green unmutated control.
Assert the mutated bytes landed, give blocking cases a deadline, and include a hang mutant for timeout
or child-lifecycle cases. Retain the leaf test names, results, source hashes, elapsed times and mutation
rows in that slice's evidence. These are future implementation tests, not tests added by this doc change.

### Argument-surface differential

Freeze the complete Bash argument cases and map each preserved case to its Rust spelling. Capture
stdout, stderr, exit status, requested room/scene/power changes and notification intent with local
substitutes; never run the Bash bootstrap against the host. Compare the new binary to a separately
built previous-main binary for already shipped Rust commands. For the first Rust slice there is no
previous lights binary: use the isolated Bash reference for mapped cases and explicit expectations for
new commands. Keep original expected values for deliberate changes beside the new values; allow only
the documented changes (including L016-L018 and L022-L023), never a global output normalization.

Name the check `argument_surface_matches_reference`. Prove it can fail with
`argument_surface_rejects_changed_reference_exit`: change only the candidate's unknown-command exit
from 1 to 0, assert distinct reference/candidate executable paths and hashes, and require the
differential to report that mismatch. Restore the candidate and rerun the green control. As each
command ships, extend the cases in the same slice. At cutover, all seven key mappings and every frozen
legacy case must be accounted for by parity, a named deliberate change, or an explicit dropped option.

### Operator acceptance

Hardware acceptance is the operator's work after their own apply. Installation and key cutover remain
separate. Required evidence before cutover:

- `seven_commands_preserve_key_intent`: before repointing keys, the operator executes each proposed
  lights command corresponding to F4 through F10 and compares its effect to the existing binding's
  intent. Include both rotation wraps and cycling from each Halo scene to `Read`. F9 remains toggle.
  After the operator applies the cutover, `seven_keys_preserve_actions` checks the actual keys.
- `brightness_floor_and_power`: request the absolute floor, then step down at the floor; compare the
  displayed requested level/direction with observed lamp behavior and verify no unintended power-off.
- `held_steps_match_isolated_steps`: temporarily use a 5-point step and a 50% start; compare five rapid
  and five isolated command invocations in each direction, then restore the default 15-point step.
  These values keep both runs away from clipping. Repeat with actual held keys after cutover.
- `bulk_read_latency`: record the timing method and bulk-read samples against the 150 millisecond bound.
  Above it, measure the design's targeted strategy before adopting it. Its budget is two reads for
  power/brightness and three for scenes/status with the current `room` port. Update the selected
  strategy, tests and evidence in a separate slice before cutover if it changes.

Schema fixtures and request-count assertions cannot satisfy these hardware cases. If any fails, keep
keys on Bash and report the evidence; do not add an unmeasured retry or accumulation mechanism.

## The pull request ladder

Twelve ordered slices replace the broad four-batch plan. Every implementation slice is new behavior
and owes fail-first evidence, the mutation control, and the argument-surface differential above. The
cutover is consumer wiring and owes the complete differential plus operator acceptance. Each slice
leaves existing keys usable and builds with the builder's exact Cargo line, run inside the source
workspace even before a builder script exists. Run normal commit hooks and the following exact-head
gates for each implementation slice:

```
just test-rust
just lint-check
just ship
```

From `dot_local/share/lights`, also run the canonical Rust gates:

```
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --no-fail-fast
RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --no-deps
cargo build --release --locked --quiet --bin lights
```

Record which member test binaries actually ran, their leaf names and the Rust skill's file-size output.
Do not add meta-tests for manifests, key declarations, ignore entries, headings or the ladder itself.
Review those declarations and run the consuming commands. All hardware calls stay with the operator.

### PR 1: command usage and workspace integration

Introduce the virtual workspace, lockfile, protocol command decoder and `lights-cli` binary for help
and usage refusal. Add other members as their first behavior arrives. Help lists only implemented
commands at each intermediate head. In the same commit add the lights `test-rust` lines, package
`.gitignore` target entry and all `.chezmoiignore` source-only/platform exclusions described above.
No binary installation or key change.

Named tests: `help_exits_zero_without_settings`, `unknown_command_exits_one_on_stderr`,
`unknown_flag_exits_one_on_stderr`, `argument_surface_rejects_changed_reference_exit`.
Control: force usage to exit 0; both the usage test and the independent differential must fail.

### PR 2: configuration validation

Add the settings parser/loader in `lights-adapters` as independently callable, typed functions. This
slice tests parsing and file errors directly; help/usage still do not load settings. PR 3 consumes the
functions in room-command composition and adds process-level exit tests. Later slices add their live
settings with their behavior. Failure diagnostics never include secret values.

Named tests: `missing_config_returns_config_error`, `malformed_config_returns_config_error`,
`unknown_config_key_is_named`, `missing_controller_type_is_rejected`,
`unknown_controller_type_is_rejected`, `missing_bridge_address_or_key_is_rejected`,
`config_error_does_not_print_key`. Control: accept a blank key and require the refusal test to fail.

### PR 3: toggle the resolved room

Add domain room names/aliases and the application-owned `LightController`, references and `TogglePower`.
Implement real `HueLightController` room/grouped-light acquisition and power writes over a scripted
transport seam, then compose it for `toggle` and the no-argument default. No fake-only production binary.
Introduce response-envelope validation with this first network-capable adapter, before any success.
Wire PR 2 settings loading into this first room command and test its process-level failures here.

Named tests: `missing_config_exits_five`, `malformed_config_exits_five`,
`missing_bridge_address_or_key_exits_five`, `default_command_toggles_configured_room`,
`aliases_resolve_and_other_names_pass_through`,
`toggle_writes_opposite_aggregated_power`, `unknown_room_exits_two_on_stderr_without_write`,
`room_without_grouped_light_is_malformed`, `invalid_room_reference_does_not_write`,
`power_body_matches_requested_state`, `bulk_room_reads_share_one_snapshot`,
`successful_status_with_errors_is_refused`, `data_with_errors_is_refused`,
`missing_or_mistyped_envelope_arrays_are_malformed`, `malformed_resource_is_not_cached`,
`transport_timeout_exits_four_without_success`, `unsuccessful_status_exits_four`.
Controls: echo current power; accept a nonempty errors array; make the transport hang. Each corresponding
case must fail against its green control. Unknown room and protocol errors leave no recorded writes.

### PR 4: explicit power

Add `SetPower` and `on`/`off` command adaptation. Permit the resolution read that mints `RoomRef`.

Named tests: `explicit_on_ignores_existing_power`, `explicit_off_ignores_existing_power`,
`explicit_power_resolves_once_without_decision_read`. Test both initial power states for both commands;
require the requested write even when it matches the snapshot. Control: toggle or skip an already
matching write and require failure.

### PR 5: absolute brightness

Add request `Brightness`, numeric parsing, the absolute use-case branch, body and requested-level output.

Named tests: `absolute_brightness_clamps_0_1_2_99_100_101`, `brightness_overflow_is_usage_error`,
`invalid_brightness_text_is_usage_error`, `absolute_body_uses_requested_level`,
`absolute_output_labels_requested_percentage`, `absolute_write_has_no_readback_or_power_off`.
Control: clamp to 0 or render an observed level that was never read; the relevant case must fail.

### PR 6: relative brightness

Add configured step size (default 15), direction parsing and one `dimming_delta` write per invocation.

Named tests: `brightness_step_defaults_to_fifteen`, `configured_step_is_used`,
`step_outside_1_through_100_is_rejected`, `relative_body_uses_direction_and_delta`,
`relative_step_ignores_snapshot_level`, `relative_output_has_no_result_percentage`,
`relative_write_has_no_readback_or_power_off`. Control: derive an absolute target from the snapshot or
print a resulting percentage; require failure. These cases do not prove hardware accumulation.

### PR 7: named scenes

Add `SetScene` with explicit names, scene parsing, room filtering, recall and command output.

Named tests: `scene_name_is_scoped_to_room`, `duplicate_scene_names_do_not_cross_rooms`,
`unknown_scene_exits_three_on_stderr_without_write`, `invalid_scene_reference_does_not_write`,
`scene_recall_body_requests_active`, `bulk_room_and_scene_reads_share_snapshot`.
Control: remove the room filter and require the duplicate-name case to fail.

### PR 8: scene rotation

Add `Rotation` and its settings, then `scene next` and `scene previous` over the named-scene path.
Preserve the four-scene order, both wraps, `Read` fallback, and Halo exclusions.

Named tests: `rotation_next_and_previous_from_each_index`, `previous_from_zero_wraps_to_last`,
`next_from_last_wraps_to_first`, `missing_or_unlisted_scene_uses_read_fallback`,
`both_halo_scenes_use_fallback_in_both_directions`, `only_static_scene_drives_rotation`,
`empty_rotation_is_rejected`, `fallback_outside_rotation_is_rejected`.
Control: subtract from zero before modulo or include a Halo scene in the cycle; require failure.

### PR 9: status

Add `ReportStatus` and plain output, preserving optional fractional brightness separately from the
request clamp, an explicit change from Bash truncation. Distinguish successful lookup without a static
scene from lookup failure.

Named tests: `status_has_no_write_or_notification`, `status_without_static_scene_prints_unknown`,
`status_scene_lookup_failure_exits_four_without_report`, `missing_dimming_is_not_zero`,
`status_output_preserves_reported_fractional_brightness`, `invalid_reported_brightness_is_malformed`,
`status_uses_one_bulk_read`.
Control: turn a failed lookup into absence or apply the request floor to a reading; require failure.

### PR 10: optional notification

Add `Notifier` and `PnsNotifier`, `--notify` and default-off config, composed with the existing actions.
First verify the pns-owned delivery cleanup prerequisite described above, including producer death;
record its exact source head and named passing cases. No live pns execution satisfies this prerequisite.
Use a recorded process runner for argv/status policy and a disposable local child for lifecycle tests,
never real pns. Exercise the real `PnsNotifier` through command composition with those substitutions.

Named tests: `notification_follows_validated_write_once`, `write_errors_never_notify`,
`successful_status_with_errors_never_notifies`, `status_never_notifies_even_when_requested`,
`notify_defaults_off`, `pns_arguments_are_local_only`, `missing_pns_does_not_fail_action`,
`missing_timeout_monitor_does_not_spawn_pns`, `notification_status_never_changes_success`,
`pns_child_hang_is_killed_and_reaped_without_failing_action`.
The status case covers zero, 125, 126, 127, 137, another nonzero, signal termination and spawn/wait
errors, asserting unchanged action output and exit 0 with no notification diagnostic or retry.

The hanging-child case owns its synthetic executable and process records inside `lights-adapters`;
unit checks stay in private `#[cfg(test)]` modules. The assembled command check belongs to
`lights-cli/tests/` with its own fixtures. Launch a child that records its process identifier and
ignores the ordinary termination signal (`SIGTERM`), then wait for its ready record instead of sleeping.
Use a short adapter deadline and an independent harness deadline below one second. Require the
monitor and direct child to be absent, with their wait results consumed, before the command returns
the same successful light action. A runner fake alone cannot prove this lifecycle case.

Controls: notify before envelope validation; drop `--local-only`; remove the monitor deadline; return
without waiting for the monitor; propagate status 137 into the light exit. Each corresponding case
must fail against its green control. The hang mutant must fail at the harness deadline, whose cleanup
kills and reaps every test-owned child even on failure. These tests cover lights' ownership and
composition; do not add a test of coreutils internals or claim that they prove pns descendant cleanup.

### PR 11: installation readiness

Add the builder and private config template, with the `CLAUDE.md` credential-target update and any
manifest integration required by the existing inventory. PR 1 already installed the ignore entries and
justfile coverage. Keep `openhue` declared. Run the exact builder Cargo line against the source and
parse a synthetic credential-free config render; never unlock KeePassXC or apply from a test.

Named behavior test: `shipped_defaults_resolve_studio_and_four_scene_rotation` exercises the parsed
settings through room and rotation policy. Control: change the default room or rotate through Halo and
require the policy result to fail. Inspect declarations and manifests directly; do not test chezmoi,
installation tools or declaration agreement. The operator applies and supplies the hardware acceptance
record for the proposed commands. Actual key acceptance follows the operator's cutover apply. If timing
requires a targeted strategy, add a separate PR 11a with
`targeted_power_reads_room_and_grouped_light`, `targeted_scene_adds_scene_listing`, and
`targeted_status_reads_three_resources_without_write`, plus mutants deleting the grouped-light read or
adding a duplicate read. Operator acceptance must pass for the selected strategy before cutover.

### PR 12: key cutover

After operator acceptance, repoint all seven aerospace bindings, retire the Bash source and update its
`CLAUDE.md` description. Keep installation separate from this change. Carry the full
`argument_surface_matches_reference` case map and its failing control, the seven-command acceptance,
the brightness and latency evidence, and the deployed script/log paths the operator removes by hand.
The bindings retain F9 toggle and the two one-shot Halo keys. After their apply, the operator records
`seven_keys_preserve_actions` and repeats the held-key drill. Hardware failure blocks acceptance and
requires a correction before the cutover can be called complete. No removal script is introduced.
