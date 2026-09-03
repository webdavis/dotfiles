# Worked example: the pns refactor

Everything here is **specific to `pns`**, the notification engine at `dot_local/share/pns`. Read it
for what an answer to the general method looks like in practice. Do not apply any of it to another
tool without deriving the same answer from that tool's own source.

Status: the refactor was scoped 2026-09-03 from the operator's draft plus a Fable review round, four
delivery-safety rulings, and two rounds of `sol` review. The rulings are recorded in
`~/.claude/pipeline/decision-pns-delivery-safety-2026-09-03.md` and the reviews in
`~/.claude/pipeline/reviews/sol-0*-2026-09-03.md`.

## The consumers outside the folder

1. **The chezmoi builder**, `.chezmoiscripts/run_onchange_after_58-build-pns-engine.sh.tmpl`, runs
   `cargo build --release --locked --quiet --bin pns --manifest-path dot_local/share/pns/Cargo.toml`
   and installs `target/release/pns` into `~/.local/libexec/pns/pns`. Its cargo line and paths move
   to the workspace layout in the same pull request as the conversion, together with
   `test/unit/pns-engine-build-install.sh`, which stubs that shape. Fixed: the crate deploys to
   `~/.local/share/pns`, the binary installs at `~/.local/libexec/pns/pns`, and the build runs
   `--locked`.
2. **The justfile recipes** `test-rust` and `pns-config-render` pass
   `--manifest-path dot_local/share/pns/Cargo.toml`. The workspace conversion has since landed:
   `crates/pns-{domain,application,protocol,adapters,cli}` exist as skeletons and `test-rust` already
   passes `--workspace` on its pns lines. `pns-config-render` is a `cargo run` and needs none. Read
   both recipes before assuming either shape.
3. **`dot_local/share/uu`** depends on pns by path and imports
   `pns::channels::hermes::{SignedPost, UreqSignedPost, PostOutcome, delivered, outcome_line, sign}`,
   so one signed-POST seam exists rather than two. Do not keep that path alive behind a facade: put
   the client in the crate where it belongs and update uu's `Cargo.toml` and imports in the same pull
   request. Add `cargo test --locked --manifest-path dot_local/share/uu/Cargo.toml` to the gates.
4. **The command-line surface** is a compatibility contract, and one caller is not ours to change:
   moshi's generated extensions hold one pathname in `helperBinary` and therefore call the bare
   spelling `pns pi-hook` rather than `pns gate pi-hook`. Enumerate the in-repo callers first:

       grep -rn 'libexec/pns/pns' --exclude-dir=.git --exclude-dir=target --exclude-dir=graphify-out . | grep -v dot_local/share/pns/

   They are the Claude Code hook declarations in `private_dot_claude/modify_settings.json`, the daemon
   LaunchAgent's `pns daemon run`, the bash notifier's `pns loop begin|end` in `dot_bashrc.tmpl`, uu's
   alert path, and the Codex hook installer. `const USAGE` in the current `main.rs` is the contract.
5. **The shipped config template** `dot_config/pns/private_config.toml.tmpl` is generated from
   `dot_config/pns/config-values.toml` by `just pns-config-render`. pns reaches out of the crate in
   three places to pin it: `src/config.rs:SHIPPED_TEMPLATE` and `src/config.rs:CONFIG_VALUES` are
   `include_str!` calls four directories up, and `tests/config_render.rs` reaches out at runtime
   through `env!("CARGO_MANIFEST_DIR")` joined with `../../..`. A fourth `include_str!`, the
   resolved-config snapshot, stays inside the crate and is fine. All three move out.
6. **The backlog** in `~/.claude/pipeline/backlog-consolidated-2026-09-02.md` is frozen: every open
   pns item is absorbed as a named design decision or re-filed in the completion report.

## The crate names

    crates/pns-domain
    crates/pns-application
    crates/pns-protocol
    crates/pns-adapters
    crates/pns-cli

The binary target stays `pns`.

## The vocabulary, measured against `src/`

producer, notification, event, signal, attempt, surface, presence, visibility, delivery destination,
route, recap, missed notification, decision ring, nag, job, claim, lease, pulse, unread, quiet window
and dim window, home probe, doctor.

Corrections the code forced on an earlier list:

- **decision ring** and **journal**, never "decision trace".
- **unread**, renamed from glow by operator ruling 2026-08-31. "glow" survives in comment prose and
  in several `tests/dispatch.rs` test names, and `lights-glow` is a FILE that `sweep_legacy_state`
  deletes rather than reads; its replacement record is `lights-held`. Never "held light".
- **quiet window**, **quiet hours** and **dim window**, never "quiet place".
- **the home probe** and **the router**, never "home presence" as a phrase.
- **signal** names nothing in the current code. It is the new normalized protocol concept and is
  introduced only there.

## The expected use cases and roles

`SubmitNotification`, `RequestApproval`, `RecordActivity`, `ReplayMissedNotifications`,
`BuildReturnRecap`, `RunNag`, `ReadHomeProbe`, `ReconcileLights`, `SetLightsQuiet`,
`AcquireLoopLease`, `RunDaemonTick`, `ScheduleJob`, `CancelJob`, `RunDoctor`, `RunSetup`.

Roles, kept separate: `NotificationDestination`, `AttentionIndicator`, `EnvironmentSnapshotReader`,
`DiagnosticCheck`, `ScheduledJob`.

**Hue is not a generic destination.** It is a stateful attention indicator with reconciliation, the
unread state, pulse behavior, leases, phases and quiet policy. **Router and home are an environment
source**, not a destination.

The normalized signal distinguishes successful outcome, failed outcome, attention required, approval
requested, resolved attention, observation, and progress. The transport is `pns submit --json`
reading one JSON request from stdin.

## The legacy surface to preserve

The lenient argv parser, missing-value warnings, recognized flags not consumed as values, help
behavior, typo refusal, notification paths not failing the work they report, hook stdout and stderr
contracts, ordinary hooks exiting zero, blocking approval and gate exit-code translation, the bare
gate spelling, `pns daemon run`, `pns loop begin|end`, the producer flags, and
`pns pulse <exit-code>` (the operator's manual lamp check).

The legacy flags `--local-only` and `--remote-only` are independent booleans today and **passing both
is a tested contract**: nothing is delivered and the refusal says so. That combination is refused at
the legacy adapter with the tested wording. It never becomes a domain state, and the delivery-scope
enum (`Automatic`, `LocalOnly`, `RemoteOnly`) gains no fourth variant for it.

## pns owns the shell notifier (operator, 2026-09-03)

This one is a pns product decision, not a general method. Add `--elapsed <secs>` to the producer
flags: the producer states how long the work took and pns decides the tier, applying the rule it
already owns (nothing under 30 seconds, the presence gate from 30, the lights from 300). Reject
`--elapsed` combined with a caller-supplied tier flag rather than silently preferring one.

`dot_bashrc.tmpl:498-580` (the `__cmd_notify_*` functions) decides those tiers today, keeps the
interactive-TUI skip list, and writes the lights marker under `~/.local/state/pns/lights-shell/<pid>`
that `pns lights tick` reads back. Move the marker, the skip list and the tiers into pns behind a
`pns shell begin` / `pns shell end --exit <code> --elapsed <secs>` pair, leaving the bashrc as two
calls. `test/unit/pns-shell-lights-marker.bats` (11 tests) pins that bash today and is deleted in the
same change in favour of Rust unit tests over the moved logic: pns tests should be in Rust now.

The Neovim overhaul's editor-side producer
(`docs/superpowers/specs/2026-09-01-nvim-overhaul-design-v4.md` section 7.7) is blocked on the flag.

## The state directory, verified against the source

`nag/`, `lights-needs/`, `lights-loop/`, `lights-blocked/`, `lights-shell/`, `daemon/`,
`daemon-markers/`, `daemon-heartbeat`, `lights-tick.lock`, `fire.lock`, `lights-streak`,
`lights-held`, `lights-news`, `lights-said`, `lights-quiet`, `lights-quiet-said`, `last-present` and
its window claims, `session-<id>.start` turn markers, `quiet-until`, `home-staleness`,
`policy-settings-audit`, `phone-attention.marker`, the decision ring, the missed-notification
journal, the activity ring, and the per-ring working-name families (`.lock`, `.new.<pid>`,
`.sweep.<pid>`, `.claim.<pid>`, `.held.<pid>.<seq>`).

Three names are legacy **deletion targets**, not state: `sweep_legacy_state` calls `remove_file` on
`lights-glow` and `lights-working-since` and `remove_dir_all` on the `lights-needs` directory every
tick, and reads none of them. Two more deserve a look during classification: nothing in `src/` writes
`phone-attention.marker` (only the link's own mtime is read), and nothing in `src/` reads
`policy-settings-audit` outside tests.

## The defects the sol reviews of 2026-09-03 found

Each is a real defect measured against the code. The refactor is the vehicle for all of them; none is
fixed by a patch that leaves the structure alone.

**Every line number below was read on 2026-09-03 and several have already drifted.** `main.rs` was
11,937 lines then and is under active refactor, so grep for the named symbol rather than jumping to a
line.

**The delivery answer is not authoritative, and it is the highest-cost defect.** `was_missed`
(`missed_notifications.rs:79`) asks the *plan* whether a banner or card was intended, never whether a
destination accepted anything. Its own doc comment admits two of the three holes. Three more
authorities compete with the plan: Hermes runs independently of presence and mute, a blocked event
can flash Hue when `plan.pulse` is false, and approval forwarding (`forward_to_moshi` in `main.rs`)
decides from surface alone before the plan exists, deliberately ignoring visibility, Focus and mute.

**One instant is not one observation.** The memoized clock makes two call sites share an epoch, which
was the 2026-08 fix, but `now` is read before the slow probes, desk idle comes back as an age while
phone and marker timestamps are aged against the earlier clock, visibility is read after the probes
finish, and Focus is a separate live read. A future marker timestamp becomes age zero through
`saturating_sub` and claims Mobile until wall time catches up.

**Failure direction is global where it must be per destination.** An unreadable lock reads as
unlocked, so a fresh idle reading can hold Desk while the screen is locked, losing the phone. An
unreadable clock discards the phone and marker timestamps while leaving the desk's own idle age
eligible. A tie between two equally fresh inputs resolves to Desk.

**The crash windows are open.** Delivery happens before both the decision record and the missed
journal on `main.rs`'s post path. The replay path deletes its claim before delivering, and the test
`the_claim_never_survives_the_run_whether_the_replay_delivered_or_not` passes, pinning that loss
window as intended behavior. On a daemon restart mid-drain, `~claim` files are excluded from scans
while `fire` deletes its claim before spawning, so either window loses the job.

**Three delivery-path tests cannot fail.** `src/channels/hermes.rs:568` and `src/channels/moshi.rs:632`
and `:674` each join a thread parked on an unbounded `TcpListener::accept()`. A mutant that stops the
client dialing hangs the suite rather than failing it.

## The pns gates

    just test-rust
    just lint-check
    just ship
    cargo test --locked --manifest-path dot_local/share/uu/Cargo.toml
    cargo build --release --locked --quiet --bin pns --manifest-path dot_local/share/pns/Cargo.toml
    just pns-config-render && git diff --exit-code dot_config/pns/private_config.toml.tmpl

`tests/support/mod.rs` enforces the speed guard: over `TEST_BUDGET_MS` (1,000) warns, over
`TEST_CEILING_MS` (5,000) fails unless `allow_slow("reason")` names a structural cause. Keep it.

`doctor` and a bare `pulse` are **live-effect commands**: a verification harness that ran them posted
two real banners and drove the lamps on 2026-09-02. The argv differential at
`~/.claude/pipeline/extraction-verify.sh` excludes both; reuse or extend it.

The template's five secret actions are pinned by tests: each is
`{{ (keepassxc "<entry>").<Field> | toToml }}` with no author quotes, and the test stub refuses any
other action.
