# pns Part 2 configuration and device acceptance reconciliation

Status: reconciliation, written 2026-09-17 on dresden with the worktree at `306e7afa`. No Rust
changed, no configuration changed, no apply ran. Two read-only probes were run against the deployed
`~/.cargo/bin/pns` (`pns home` and `pns tap --info`), neither of which delivers a notification, and
one read-only copy of `~/.local/state/pns/pns.db` was queried for counts. `pns doctor` was
deliberately NOT run: it sends a real test notification down every channel.

What this reconciles: the A to H, R1 to R4, P1 to P4, DR1 to DR4 and hook-walk proposal recorded in
the memory file `pns-part2-scope.md` as delivered on 2026-08-20, read as a set of claims to check
rather than as facts. The drill half of that record was reconciled on 2026-09-14 and lives at
`~/workspaces/backups/2026-09-14T04-34-55.pns-part2-drill-reconciliation.backup.txt`; this pass
covers the configuration and device acceptance half and re-checks the four things that moved since.

Finding in one line: **most of the proposal shipped or was superseded, three items remain to build,
and four stay with the operator.** Two device facts changed tonight in the ledger's favour, and the
one drill that was unchanged is unchanged still.

## What changed since the 2026-09-14 record

| Claim in the record                         | Tonight                                                |
| ------------------------------------------- | ------------------------------------------------------ |
| `pns home` reads `unknown`, credential gate | reads Home; the gate is closed                         |
| slice 7 quiet window config-gated, deferred | `quiet_hours` live since `ff7dff41`, but superseded    |
| doctor misses the `pns-recap` route         | route retired with task 81; recap uses `pns-events`    |
| doctor misses the bare `posture` route      | route retired tonight by task 99                       |
| drill 28 missing a Codex phone leg          | unchanged, no new Codex event since                    |

The two retirements settle incidental 16 of the 2026-09-14 record. Because posture now posts its own
pages rather than submitting through pns, any acceptance that names a pns ledger row for a posture
event is unsatisfiable by construction, which is what task 50a found; the eight `agent='posture'`
rows in `ledger_events` are historical and cannot be added to.

## Disposition per item

**A. Home module with the UniFi Dream Router probe. ALREADY SHIPPED, device acceptance now half
closed.** The probe is `pns/crates/pns-adapters/src/unifi/`, its configuration is
`dot_config/pns/config-values.toml:89-92` (`router_url`, `device_hostname`, and the `api_key` naming
the vault entry `UniFi :: API Key (dresden-udr)`), and it merged across PR #190, PR #191 and
PR #196. Run tonight, `pns home` answers "on the home network, matched by device_hostname" with the
evidence line naming the same identifier, so the credential or site-id failure the 2026-09-14
record and task 50a both blamed no longer reproduces and the 2026-08-28 Home pass is live again.
NotHome stays OPERATOR-OWNED: take the phone off wifi, run `pns home` and expect "NOT on the home
network", then put wifi back and expect Home again within about fifteen seconds.

**B. Lock override and the lights quiet window. Lock override ALREADY SHIPPED; the quiet window is
SUPERSEDED.** The lock read is `pns/crates/pns-adapters/src/macos/desk.rs`, merged as PR #198, and
its drill closed on live evidence (decisions rows 1999 and 2000). The quiet window merged as PR #199
and its key is live, not commented: `dot_config/pns/config-values.toml:74` and
`dot_config/pns/private_config.toml.tmpl:146` both carry `quiet_hours = "22:00-07:00"`, which makes
the record's "template ships the key commented" stale. It is superseded all the same: the template's
own prose at lines 141 to 145 states that with a `[lights]` table present each place's `dim_window`
decides the night and `quiet_hours` is the mute's schedule alone, and `[lights]` is present
(`config-values.toml:100-139`, four `dim_window` declarations). The replacement differs by being per
place and per behaviour rather than one house-wide gate.

**C. Catch-up on return. ALREADY SHIPPED; the `connectedAt` design input is SUPERSEDED.** The journal
and replay merged as PR #205, PR #206 and the race repair PR #208, and the owned return moment as
PR #212; both drills closed on live evidence. The `connectedAt` field the operator suggested as the
arrival anchor appears in the tree only inside three test fixtures
(`pns/crates/pns-adapters/src/unifi/tests/fixtures.rs:6` and two siblings) and nothing reads it: the
return moment is anchored on the last-present marker and `claim_moment` instead, which needs no
router read and therefore survives a router that is unreachable.

**D. pi and Hermes hook coverage, and Codex drilled as a harness. OPERATOR-OWNED, unchanged.** Drill
28's Codex counts are identical to the 2026-09-14 reading (758 blocked, 289 done, 25 asking), its
legs are 758 hermes and 19 `macos-banner` with no `mobile` leg on any blocked event, and no event in
3005 carries `agent='pi'`. The two operator items are unchanged: answer a Codex permission request on
the phone while away from the desk, and rule on drill 29, whose premise moshi 0.3.16 removed.

**E. Lights vocabulary. ALREADY SHIPPED, then SUPERSEDED by its own rebuild.** The part-2 vocabulary
merged across PR #223, PR #226, PR #231, PR #232 and PR #233, and was then rebuilt on 2026-09-01 by
`5025363c`, whose message names what it retired: the family layer, the place table, `breathe_on`, the
alternating pair, the steady glow, `dim_brightness` and `catch_up`. So the operator's magenta,
breathing and held-verdict decisions are all history; what ships is three-level routing (lamp, room,
zone) with a leased tick driving held states, and no `breathe_on` or `breathe_after_secs` key exists
anywhere in the workspace. The daylight comparison of blocked against loop stays OPERATOR-OWNED.

**F. Moshi host-events endpoint upgrade. SUPERSEDED.** The no-lost-behavior test the operator made a
condition shipped first, as item 24 and PR #215. The switch it was gating was then declined on
measured evidence (no public endpoint, a private unversioned socket, and the CLI composing the action
identifier from terminal introspection), and item 25 shipped instead as the bounded wait, PR #222.
Tonight's source agrees: `pns/crates/pns-adapters/src/moshi_hook.rs:36-52` still spawns the CLI, and
its `answer` arm waits at line 22 with `answer_within(child, submit_deadline())`. The replacement
bounds the wait rather than removing the spawn.

**G, and item 19. Summarizer performance. PARTLY SHIPPED, remainder STILL WANTED.** What shipped is
the stripped private Codex home for the turn condenser,
`pns/crates/pns-adapters/src/codex.rs:43-47`, which records cutting the load from about nine seconds
to about three and also removes the condenser's own Stop hook; its ceiling is
`CONDENSER_DEADLINE` at `codex.rs:81`, thirty seconds. Item 19's brief-writer, launched 2026-08-30,
produced no merge, and no total-runtime figure exists: the 2026-09-17 native probe evaluation
measured the probe stage alone and its adoption is task 144. See the closing list.

**H. Config consolidation. ALREADY SHIPPED, and further than proposed.** Item 23 merged as PR #234,
the surface rename as PR #238, and the whole shipped template is now generated from one committed
values file by `ff7dff41`, with a byte-equality gate inside `just test-rust`. The sweep's own named
candidate, rejecting a mistyped settings key instead of ignoring it, shipped per table:
`pns/crates/pns-adapters/src/config/delivery.rs:12`, `daemon.rs:27`, `focus.rs:25`, `failures.rs:69`
and `lights_tables.rs:37`.

**R1. Alerter desk-banner approve and deny buttons. STILL WANTED.** The gate shipped as PR #220,
twelve approval-contract tests. The buttons did not: `pns/crates/pns-adapters/src/destinations/`
`banner.rs` spawns `terminal-notifier` for text and a click command only, with no action option
anywhere, and nothing in the tree mentions an approve or deny button. See the closing list.

**R2. Pluggable local summarizer. ALREADY SHIPPED for one of its two callers.** The recap summarizer
is fully configurable as argv, `dot_config/pns/private_config.toml.tmpl:308` and `:313`
(`summarizer` and `summarizer_deadline_secs`), merged as PR #213, which satisfies the operator's
configurable-backend condition and is the prepared door for a non-macOS machine. The turn condenser
is not: `pns/crates/pns-adapters/src/codex.rs:69` writes a fixed `model = "gpt-5.5"` with low
reasoning into the private home, and no configuration key reaches it. See the closing list.

**R3. Hue Bridge Pro MotionAware investigation. OPERATOR-OWNED.** The investigation ran and the room
sensor shipped (`[plugins.presence] type = "hue"` at `config-values.toml:83-87`, the bridge side at
`pns/crates/pns-adapters/src/presence/bridge.rs`). The one question it could not settle is recorded
in that file at lines 21 to 25 and in `pns/docs/specs/daemon-jobs.md:50-52`: the machine has zero
MotionAware areas, so whether an area's motion joins its room's roll-up or arrives only as its own
resource is unverifiable. The operator step is to create one MotionAware area in the Hue app; the
answer is then one GET, and the spec names it.

**R4. iPhone Shortcuts as signal writers. ALREADY SHIPPED AND ACCEPTED ON THE DEVICE.** `pns tap`
(`pns/crates/pns/src/invocation.rs:122`) with the install guide in
`pns/crates/pns-adapters/src/tap_install.rs` and `pns/crates/pns/src/tap_report.rs`, writing a marker
file, which is the platform-neutral signal interface the 2026-08-25 design rule asked for rather than
anything iOS-specific. Live tonight: `pns tap --info` reports the marker exists with a last tap
24228 seconds before the reading, so the phone has actually made a tap and the device half is closed.

**P1. Escalation ladder. SUPERSEDED.** The operator ruled rebuild on 2026-09-01: the original ladder
(parked, never shipped) was replaced by the nag, which merged as PR #230 and lives at
`pns/crates/pns-application/src/arm_nag.rs` with `[nag]` armed at `config-values.toml:97`. The
replacement raises at most one nudge for one approval and never stacks, which is what the ladder
could not promise.

**P2. `pns quiet`. ALREADY SHIPPED**, PR #200, with the state held as an absolute expiry.

**P3. `pns doctor`. ALREADY SHIPPED**, PR #201 (test send), PR #203 (decision log) and PR #204
(moshi pairing); both of the additions the operator attached on 2026-08-25 closed on live evidence on
2026-09-14. One standing caution belongs with it: a doctor run delivers a real notification to every
channel, so it is not an agent-safe probe at an hour the operator is asleep.

**P4, and DR3. Native Focus awareness. ALREADY SHIPPED; configuration OPERATOR-OWNED.** Merged as
PR #217 per the operator's per-mode ruling, with the reader at
`pns/crates/pns-adapters/src/macos/focus.rs` and the parser at `config/focus.rs`. The feature is off
by the operator's own design: `[focus]` and `silence` render commented at
`dot_config/pns/private_config.toml.tmpl:350-351` and no `[focus]` table has
ever appeared in `config-values.toml`, which by that ruling is the same statement as naming no mode.
Turning it on is one uncommented line naming a mode, and the drill follows it.

**Hook-walk additions.** StopFailure ALREADY SHIPPED (PR #207, drill closed on six `claude/failed`
events). PermissionDenied ALREADY SHIPPED (PR #209, drill closed on ledger row 161). Elicitation
ALREADY SHIPPED (PR #210). SessionStart and SessionEnd were conditional on the catch-up design
needing an attendance sheet and it did not: the return moment reads the last-present marker, and no
pns hook word names either event. PreCompact was called a dud for pns by the record itself, and the
standalone reminder hook it suggested instead has no trace: `private_dot_claude/modify_settings.json`
contains no `PreCompact` entry. Neither of the last two is part-2 debt.

**DR1. ntfy as a second approval path. SUPERSEDED** by the 2026-08-25 ruling that took it out of
part 2 entirely once moshi's deny was proven working through the full pns stack. It became part 4 in
the 2026-08-28 renumbering, keeping a contingency to jump the queue if moshi's deny regresses. The
replacement is doing nothing until that regression, rather than building a second channel now.

**DR2. Graded urgency and retry until acknowledged. SUPERSEDED** by the post-part-2 delivery
reliability work, which is both more and different: `[delivery]` ships `max_retries`, `event_max_age`
and `retry_step` (`private_config.toml.tmpl:268-279`) over a durable queue, and `7e165ec1`
(task 30) classifies a failure as permanent or temporary so a refused request dead-letters on its
first failure instead of consuming twenty attempts. Grading is `bypass_silence_classes` rather than a
numeric urgency. The jitter that a Pushover-style design would carry was deliberately removed in the
same commit: one local daemon draining one queue has no herd to spread.

**DR4. The fm command-line interface as the summarizer. SUPERSEDED** by R2's argv contract, which
takes any backend and ships an ollama example instead. `fm` is not installed and is not declared in
`.chezmoidata/system_packages_autoinstall.yaml`, so its latency was never measured and no longer
needs to be.

**Post-reboot program item 5, configuration hygiene. ALREADY SHIPPED** by the generator, `ff7dff41`:
defaults are written out visibly, opt-ins stay commented, and absence is documented as meaningful in
`config-values.toml:8-13`.

## Still wanted, in the order a later task should take them

1. **The total-runtime performance pass** (Todoist `6hPxWVHM8pG4qgwp`, which absorbs item 19 and G).
   The one item with a measurement already waiting on it: adopting the native replacements for the
   two `ioreg` reads is task 144, and that decision is the natural first half. What is missing is a
   total-runtime figure for a whole event rather than a probe-stage median, measured on the deployed
   binary ambient and under load, with the condenser path measured separately because its own
   thirty-second ceiling dominates any turn that reaches it. Cost: a measurement pass, no design.
2. **A configuration key for the turn condenser's backend** (`pns-adapters/src/codex.rs:69`). The
   recap summarizer already proves the shape: an argv array plus a deadline, unset meaning the
   shipped behaviour. Doing the same for the condenser finishes R2's configurable-backend condition
   and removes the last hardcoded model name from the crate, which is also what part 4's Linux door
   needs. Cost: one small slice, one config table arm, no new dependency.
3. **The desk-banner approve and deny buttons** (item 27, R1). The gate that pins today's behaviour
   is already on main, so this is buildable, but three constraints are recorded and binding: Claude
   2.1.241 decides a permission request from stdout `hookSpecificOutput.decision` and reads exit
   codes nowhere on that event, so a button pressed after pns has exited must answer through moshi's
   own path; the single-submitter tests count this process's submissions only, so a banner action
   spawning its own `moshi-hook` is invisible to them and the brief must say so; and
   `permission_suggestions` is carried and ignored today, which an allow-with-rule button would want
   pinned. Cost: one slice plus its own no-lost-behavior gate, and it is the one item here that
   would take over notification permissions and app identity from `terminal-notifier`, which task
   144 says to keep unless a feature gap justifies the move. This is that gap, and it is the reason
   to sequence this item after the performance pass has settled what owns the banner.

The Part 2 intent review (Todoist `6hPxWVwHGX9qFWpG`) is not on that list because it is not a build:
the 2026-08-31 grill session covered the lights behaviours only, and a pressure test across all of
Part 2 remains the operator's to schedule.
