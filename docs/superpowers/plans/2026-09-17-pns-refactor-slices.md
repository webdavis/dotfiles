# pns refactor: the pull-request ladder

Source plan: `/Users/stephen/workspaces/Ivy/webdavis/dotfiles/pns/docs/pns-refactor.md`, 119 numbered
items in its Changes section (the markdown uses `1.` throughout, so the numbers below are the rendered
ones). Item numbers referenced here are that rendering.

Four plan items are settled twice by later items, and the later one wins, so they are not sliced
separately: 79 by 118, 80 by 117, 88 by 116, 87 by 119.

Item 16 (one shared field list) and item 33 (move every caller in the same change) are not slices: they
are the rule every request-envelope slice below follows. Item 11 (spec updates) and item 62 (do not bump
the envelope version) are likewise properties of the slices that touch those files, not slices of their
own. Item 89 (child env names already match the request fields) is already true and needs no change.

One correction to the plan's own file paths: there is no `posture/crates/posture-producer-wire/`.
posture's copy of the wire contract is `posture/crates/posture-adapters/src/wire/` (`request.rs`,
`result.rs`, `wire.rs`) with its fixture at `posture/crates/posture-adapters/fixtures/request-v1.json`.
Every slice below that the plan sends to `posture-producer-wire` goes there instead.

One standing risk that applies to every slice touching config shape: `dot_config/pns/config-values.toml`
is the input and `dot_config/pns/private_config.toml.tmpl` is generated. Regenerate with
`just pns-config-render`, never hand-edit the template; `just test-rust` byte-compares them. And the
template is a chezmoi target, so the operator must apply before the deployed config matches the new
binary. Every config-shape slice therefore ships the parser change and the values change together, and
the operator applies once per slice.

---

SLICE 1: promote one duration parser out of the quiet module
plan-items: 40, and the parsing half of 38
why-this-order: first. Every duration rename in slices 12, 22, 29, 30, 33, 34, 35, 43, 44 parses a
duration string, and each would otherwise grow its own parser.
files: `pns/crates/pns-domain/src/quiet.rs` (source of `parse_duration`), a new
`pns/crates/pns-domain/src/duration.rs` plus its `tests.rs`, `pns/crates/pns-domain/src/lib.rs`
callers-to-update-in-the-same-slice: none outside pns. The only caller today is `quiet.rs`'s own
`pns quiet <duration>` argument.
behaviour-to-pin: a duration string is accepted in `ms`, `s`, `m` and `h`, a bare number is refused, and
the refusal names the field it was parsing rather than saying "quiet duration".
risk: none. `pns quiet` keeps taking exactly what it takes today; the parser moves and gains `ms`.
size: small

SLICE 2: one sending subcommand, `pns send`, with two input forms
plan-items: 47, 16
why-this-order: after nothing, but before every request-field slice (9 to 15), so each of those changes
one parser rather than two.
files: `pns/crates/pns/src/invocation.rs`, `pns/crates/pns/src/legacy.rs`,
`pns/crates/pns/src/legacy/argv.rs` (delete `is_producer_argv`),
`pns/crates/pns/src/event_flow/submit.rs`, `pns/crates/pns-adapters/src/daemon_children.rs`
callers-to-update-in-the-same-slice:
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/lights/crates/lights-adapters/src/notification.rs`
(and `notification/tests.rs`, and
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/lights/crates/lights/tests/argument_surface/expectations.py`),
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/uu/crates/uu-adapters/src/alert.rs`,
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/posture/crates/posture-adapters/src/wire.rs` (the
producer argv it hands a request to),
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/pns/crates/pns-adapters/src/process/shell_event.rs`,
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/pns/crates/pns/src/command_failures.rs` (the
failed-command text it rebuilds, line 198),
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/.chezmoiscripts/run_onchange_after_64-update-skills-first-install.sh.tmpl`
line 48
behaviour-to-pin: `pns send` with flags and `pns send --json` reach one request parser, and a bare
`pns --state done` is refused with exit 2 rather than delivering an empty event.
risk: high, and the highest in the ladder. Three of the four callers are the operator's own automation
(the lights announcer, the weekly uu alert, the skills-bootstrap failure notice) and the fourth is the
shell notifier every interactive command ends in. A missed caller is a notification that silently stops.
Cross-check with `git grep -n 'cargo/bin/pns\|libexec/pns/pns'` before merging.
size: medium

SLICE 3: `--producer` and `PNS_PRODUCER` replace `--agent` and `PNS_AGENT`
plan-items: 8, 9, 46, 11
why-this-order: after slice 2, so the callers are touched once at their new subcommand. Before slice 23,
which keys reminder config off the producer name.
files: `pns/crates/pns/src/legacy/argv.rs`, `pns/crates/pns/src/hook_dispatch.rs` (line 23),
`pns/crates/pns-application/src/arm_nag.rs`, `pns/crates/pns/src/command_failures.rs`,
`pns/crates/pns/tests/hooks/{approval_forwarding,nag_arming,stale_arming}.rs`,
`docs/specs/blocking-approval.md`,
`docs/superpowers/specs/2026-09-05-pns-behavioral-specification.md` (S007, S008, S073, S075, S210,
S213), `docs/superpowers/specs/2026-09-06-lights-design.md`,
`docs/superpowers/specs/2026-09-08-pns-delivery-failure-reporting-design.md`
callers-to-update-in-the-same-slice:
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/lights/crates/lights-adapters/src/notification.rs:49`
(plus `notification/tests.rs:29` and `lights/crates/lights/tests/argument_surface/expectations.py:176`),
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/uu/crates/uu-adapters/src/alert.rs`,
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/pns/crates/pns-adapters/src/process/shell_event.rs:13`,
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/.chezmoiscripts/run_onchange_after_64-update-skills-first-install.sh.tmpl:48`,
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/dot_local/libexec/pns/hooks/codex/executable_install-hooks.sh`
(lines 12, 13, 41, 45),
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/test/unit/pns-codex-hook-migration.test.sh` (the
fixture strings it builds)
behaviour-to-pin: `pns send --producer <name>` names the sender and `--agent` is refused as an unknown
flag; the hook path reads `PNS_PRODUCER`.
risk: medium. The Codex installer writes `PNS_AGENT=codex` into `~/.codex/hooks.json`, so the deployed
hooks keep the old variable until the operator applies. Until then Codex hook events lose their producer
name and fall back to the default. The migration test in `test/unit/` is the guard that the installer
rewrites existing rows.
size: medium

SLICE 4: `pns tap info` and `pns tap install` replace the two flags
plan-items: 48
why-this-order: independent; any time after slice 2.
files: `pns/crates/pns/src/command_tap.rs`,
`pns/crates/pns-adapters/src/config/render/layout/` (the prose carrying the `pns tap --install` hint),
`dot_config/pns/private_config.toml.tmpl` (regenerated, line 369)
callers-to-update-in-the-same-slice: none typed by a machine. The hint in the shipped config is the only
reference, and it is generated.
behaviour-to-pin: `pns tap info` and `pns tap install` do what the flags did and `pns tap --install` is
refused.
risk: none. Operator-typed only.
size: small

SLICE 5: `pns failures open` and `pns lights pulse` replace `pns click` and `pns pulse`
plan-items: 49, 50
why-this-order: independent; before slice 7, which lists every subcommand in help.
files: `pns/crates/pns/src/command_click.rs` (folded into `command_failures.rs`),
`pns/crates/pns/src/command_pulse.rs` (folded under `command_lights.rs`),
`pns/crates/pns/src/invocation.rs`
callers-to-update-in-the-same-slice: the banner's own click command, which pns builds from
`current_exe` at `pns/crates/pns/src/command_click.rs:95` and stores through
`plugins.macos-banner.click_command`. A banner already on screen when the slice lands carries the old
command, so the click is dead for those; new banners are correct.
behaviour-to-pin: a failure banner's stored click command names `failures open <id>`, and that command
opens the failure view.
risk: low. `pns pulse` is the operator's manual lights check and nothing in the repo calls it (the shell
notifier stopped making its own pulse call). Banners already delivered lose their click.
size: small

SLICE 6: remove `pns gate` and fold `pns home` into `pns doctor`
plan-items: 51, 52, 55
why-this-order: independent; before slice 7.
files: `pns/crates/pns/src/invocation.rs` (the `gate` and `home` branches),
`pns/crates/pns/src/moshi_submission.rs` (line 14, the silent-exit bug),
`pns/crates/pns/src/command_home.rs`, `pns/crates/pns/src/command_doctor.rs`
callers-to-update-in-the-same-slice:
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/.chezmoiscripts/run_after_62-bounce-moshi-hook-on-upgrade.sh.tmpl`
already points `helperBinary` at the bare `pns` pathname, so the surviving `pns <harness>-hook` spelling
is what it uses and no edit is needed. Verify that, do not assume it.
behaviour-to-pin: `pns <harness>-hook` still gates, `pns gate pi-hook` is refused with exit 2, and a
harness word the gate does not accept exits non-zero with a message rather than exiting 0 in silence.
risk: low. The one caller uses the surviving spelling. `pns home` is operator-typed.
size: small

SLICE 7: `--help` and `-h` answer on every subcommand, and `pns --help` lists them all
plan-items: 53, 54
why-this-order: after slices 4, 5 and 6, so the listing names the final subcommands.
files: `pns/crates/pns/src/invocation.rs`, `pns/crates/pns/src/legacy.rs` (`USAGE`), one usage string per
`command_*.rs`
callers-to-update-in-the-same-slice: none
behaviour-to-pin: every subcommand answers `--help` with its own usage and exit 0, and the tool's help
names the machine-called subcommands (`send`, `failures`, `daemon retry`, `recap agent`, `recap git`,
`presence poll --daemon`) as well as the typed ones.
risk: none
size: medium

SLICE 8: `pns shell end --exit-code`
plan-items: 44
why-this-order: independent; earlier is better because the caller is the operator's live shell.
files: `pns/crates/pns/src/shell/parse.rs` (line 28)
callers-to-update-in-the-same-slice:
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/dot_bashrc.tmpl` lines 495 and 505
behaviour-to-pin: `pns shell end --exit-code <n>` records the exit code and `--exit` is refused.
risk: medium in timing, not in size. `dot_bashrc.tmpl` is a chezmoi target, so between the merge and the
operator's apply the deployed bashrc passes `--exit` to a binary that no longer accepts it: every
long-running command's completion notice is lost, silently, because the notifier discards output. Tell
the operator to apply in the same sitting.
size: small

SLICE 9: `--route` replaces `--channel`
plan-items: 20
why-this-order: after slice 2.
files: `pns/crates/pns/src/legacy/argv.rs` (lines 29, 171), `pns/crates/pns/src/channel_dispatch.rs`
(line 223, the refusal text), `pns/crates/pns/src/command_failures.rs` (line 203, the rebuilt command
text), `pns/crates/pns-domain/src/failure/tests.rs`,
`pns/crates/pns-adapters/src/persistence/sqlite/ledger/failing.rs` (the doc comment)
callers-to-update-in-the-same-slice: none. `uu` deliberately names no route and its test
`an_alert_names_no_route_channel_or_gateway_of_any_kind` in
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/uu/crates/uu-adapters/src/alert.rs` already forbids
both spellings, so it stays green either way.
behaviour-to-pin: `pns send --route <name>` selects the route and `--channel` is refused.
risk: none in this repo. Stored failure records replay a command text containing `--channel`; the text
is descriptive, not executed.
size: small

SLICE 10: `state` is one closed set on both paths
plan-items: 17, 18
why-this-order: after slice 2. Before slice 16, whose delivery decisions read the state.
files: `pns/crates/pns/src/legacy/argv.rs`, `pns/crates/pns-protocol/src/request.rs` (the `Signal`
wrapper), `pns/crates/pns/src/event_flow/submit/mapping.rs`, `pns/crates/pns/src/invocation.rs`
(`event_mode` always using `Attempt::First`), `pns/crates/pns-protocol/fixtures/request-v1.json`
callers-to-update-in-the-same-slice:
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/.chezmoiscripts/run_onchange_after_64-update-skills-first-install.sh.tmpl:48`
sends `--state first-install-failed`, which is outside the closed set and becomes `failed` with the
detail carrying the rest;
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/posture/crates/posture-adapters/src/wire/request.rs`
(`Signal`, lines 23 and 84) and
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/posture/crates/posture-adapters/src/producer/request.rs`
(its `AlertSignal::NeedsAttention` mapping) and
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/posture/crates/posture-adapters/fixtures/request-v1.json`;
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/uu/crates/uu-adapters/src/alert.rs` and
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/lights/crates/lights-adapters/src/notification.rs`
already send `failed` and `done`, so they only lose the wrapper if they wrote one.
behaviour-to-pin: `observation` sent by flag and `observation` sent as JSON both deliver as a quiet
update, and a state outside the six words is refused with exit 2.
risk: medium. The skills-bootstrap caller sends a word that stops being legal, so leaving it behind
turns an apply-time failure notice into a refusal. `observation` by flag changes from an ordinary
message to a quiet one, which is the point.
size: medium

SLICE 11: flatten `context` and `session`, and add `--request-id`, `--session` and a duration `--elapsed`
plan-items: 19, 21, 23, 24
why-this-order: after slices 1 (the parser) and 2.
files: `pns/crates/pns-protocol/src/request.rs` (`Context`, `Session`, `elapsed_secs`),
`pns/crates/pns/src/legacy/argv.rs`, `pns/crates/pns/src/event_flow/submit/mapping.rs`,
`pns/crates/pns-protocol/fixtures/request-v1.json`
callers-to-update-in-the-same-slice:
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/posture/crates/posture-adapters/src/wire/request.rs`
(`Session` at 57, `Context` at 66, `elapsed_secs` at 90) and
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/posture/crates/posture-adapters/fixtures/request-v1.json`;
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/pns/crates/pns-adapters/src/process/shell_event.rs`
(which passes `--elapsed`) and
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/dot_bashrc.tmpl` lines 495 and 505, which pass
`--elapsed 0` and a bare seconds count and become `--elapsed 0s` and `--elapsed <n>s`
behaviour-to-pin: `--elapsed 90s` and `"elapsed": "90s"` produce one event, a bare `--elapsed 90` is
refused, and `project`, `branch`, `pane` and `session` are read at the JSON top level.
risk: medium, for the same apply-window reason as slice 8: the deployed bashrc passes a bare number
until the operator applies. Pair this slice's apply with slice 8's.
size: medium

SLICE 12: `--scope` replaces `--local-only` and `--remote-only`
plan-items: 22
why-this-order: after slice 2.
files: `pns/crates/pns/src/legacy/argv.rs` (lines 40, 41, 126, 127), the both-flags refusal it removes
callers-to-update-in-the-same-slice:
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/lights/crates/lights-adapters/src/notification.rs`
(passes `--local-only`), plus `lights/crates/lights-adapters/src/notification/tests.rs` and
`lights/crates/lights/tests/argument_surface/expectations.py`
behaviour-to-pin: `--scope local_only` delivers locally only, `--scope` outside the three words is
refused, and there is no pair of flags left to conflict.
risk: low. One caller, in a workspace whose own argument-surface test pins the argv.
size: small

SLICE 13: remove the request fields that change nothing
plan-items: 26, 27, 28, 29
why-this-order: after slice 11 (which already touches `session`). Item 26's justification depends on
slice 16's exit codes, so order 16 before this one, or state the dependency the other way and do 26 in
16. Simplest: do 27, 28 and 29 here and leave `require_delivery` to slice 16, which is where the exit
code that replaces it is decided.
files: `pns/crates/pns-protocol/src/request.rs` (`event`, `occurred_at`, `interaction`, `session.turn`),
`pns/crates/pns/src/event_flow/submit.rs` (line 86), `pns/crates/pns/src/legacy/argv.rs`
(`--long-running` and its refusal beside `--elapsed`),
`pns/crates/pns/src/event_flow/submit/mapping.rs`, `pns/crates/pns-protocol/fixtures/request-v1.json`
callers-to-update-in-the-same-slice:
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/posture/crates/posture-adapters/src/producer/request.rs`
line 41 sets `occurred_at`, and `posture/crates/posture-adapters/src/wire/request.rs` declares `event`
(83), `occurred_at` (87), `interaction` (103) and `Session.turn` (60); the fixture at
`posture/crates/posture-adapters/fixtures/request-v1.json` carries them too. The `github` extension's
own `occurred_at` inside `extensions` stays.
behaviour-to-pin: a request carrying `event`, `occurred_at`, `interaction` or `session.turn` is refused
as unknown fields (after slice 15) or ignored with them named, and an elapsed time at or above the long
session threshold is treated as long-running with no flag passed.
risk: low. posture's producer path is dormant on dresden (`[notify] mode = "hermes"`), so its wire
change ships ahead of use.
size: small

SLICE 14: `delivery_class` replaces `--kind` and the JSON `class`
plan-items: 12, 13, 15
why-this-order: after slice 2. Before slice 15, whose config tables define what each class does.
files: `pns/crates/pns/src/legacy/argv.rs` (lines 31, 135, 139),
`pns/crates/pns-protocol/src/request.rs` (`class`), `pns/crates/pns-domain/src/routes.rs`,
`pns/crates/pns/src/event_flow/execution.rs` (line 17),
`pns/crates/pns/src/event_flow/submit/mapping.rs`, `pns/crates/pns-protocol/fixtures/request-v1.json`
callers-to-update-in-the-same-slice:
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/uu/crates/uu-adapters/src/alert.rs` (passes
`--kind health`, and its module doc explains the choice, so the prose moves with it) plus its tests;
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/posture/crates/posture-adapters/src/producer/request.rs`
line 45 (`class = "security"`) and
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/posture/crates/posture-adapters/src/wire/request.rs`
line 101, and both fixtures;
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/docs/runbooks/local-daemons.md` lines 78 and 87, which
document `--kind health` as how the priority route is reached
behaviour-to-pin: `--delivery-class health` and `"delivery_class": "health"` reach the same route, and
`--kind` is refused.
risk: medium. uu's weekly alert is the live user of `--kind health`; if it and pns disagree across an
apply, a failed weekly lane pages nowhere. uu is built by
`.chezmoiscripts/run_onchange_after_59-build-uu.sh.tmpl` on the same apply, so one apply covers both.
size: medium

SLICE 15: `[delivery_class.<name>]` defines the classes in config
plan-items: 14, 63, 64, and the removal of `[delivery] bypass_silence_classes`
why-this-order: straight after slice 14, which introduced the field this configures.
files: `pns/crates/pns-adapters/src/config/schema.rs` (the roster: drop
`delivery.bypass_silence_classes`, add the new nested table), a new `config/delivery_class.rs`,
`pns/crates/pns-adapters/src/config/delivery.rs`, `pns/crates/pns-adapters/src/config/render/layout/`,
`pns/crates/pns-domain/src/routes.rs` (delete the hardcoded `agent` and `health` words),
`dot_config/pns/config-values.toml`, `dot_config/pns/private_config.toml.tmpl` (regenerated)
callers-to-update-in-the-same-slice: the shipped values file gains `[delivery_class.default]`,
`[delivery_class.health]` and `[delivery_class.security]`, matching what uu and posture send after
slice 14.
behaviour-to-pin: a `delivery_class` naming no configured table is refused with exit 2 and named, and a
message naming no class takes `[delivery_class.default]`'s route and its `bypass_mute`.
risk: high in the apply window. Between merge and apply the deployed config has no
`[delivery_class.*]` table, so uu's `health` page and posture's `security` page are refused rather than
routed. This is the slice to apply immediately, and the one to name in the pull-request body.
size: medium

SLICE 16: `status` reports delivery, and the exit code matches it
plan-items: 65, 66, 31, 26
why-this-order: after slice 13. Before slice 17, so the result type's other renames land on the new
status.
files: `pns/crates/pns/src/event_flow/submit/receipt.rs` (lines 24-36, 54-60, 69-73),
`pns/crates/pns-protocol/src/result.rs` (`Status`, its doc comment at line 24),
`pns/crates/pns/src/event_flow/submit.rs` (line 50), `pns/crates/pns/src/legacy.rs` (line 46, the
`--require-delivery` gate it deletes), `pns/crates/pns/src/invocation.rs` (`EVENT_NOT_DELIVERED`),
`pns/crates/pns/src/event_flow/submit/receipt/tests.rs` (lines 29-55 pin the old rule),
`pns/crates/pns-protocol/fixtures/result-v1.json`
callers-to-update-in-the-same-slice:
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/posture/crates/posture-adapters/src/wire/result.rs`
(`Status` at line 17) and its tests;
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/uu/crates/uu-adapters/src/alert.rs`, which reads the
exit code and whose fail-open contract must survive the change from "1 only with `--require-delivery`"
to "1 on any failed destination"
behaviour-to-pin: a request whose ledger row committed but whose every destination failed answers
`undelivered` with exit 1 and a `ledger_committed` diagnostic, and a request with one destination
delivered and one failed answers `partial` with exit 1.
risk: medium. Every producer that treated exit 1 as fatal now sees it on a partial delivery. uu fails
open by design and the shell notifier discards the code; the hook paths keep their always-exit-0
contract, which this slice must not touch.
size: medium

SLICE 17: result field names and bare-string closed sets
plan-items: 67, 74, 68, 69
why-this-order: after slice 16.
files: `pns/crates/pns-protocol/src/result.rs` (lines 24-46, 60, 67),
`pns/crates/pns-protocol/src/request.rs` (lines 52, 77, the `{"kind": ...}` wrappers left after
slice 10), `pns/crates/pns/src/event_flow/submit/receipt.rs` (lines 64, 65),
`pns/crates/pns-application/src/submission_delivery.rs` (line 11), both fixtures
callers-to-update-in-the-same-slice:
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/posture/crates/posture-adapters/src/wire/result.rs`
(`decision_id` at 64, `interaction` at 66, `DestinationOutcome.destination` at 50,
`InteractionResult` at 29) and `posture/crates/posture-adapters/src/wire/tests.rs`
behaviour-to-pin: a result carries `ledger_sequence`, each destination carries `name`, no `interaction`
field is emitted, and every closed set serializes as a bare string.
risk: low. posture's result reader is the only consumer and its producer path is dormant.
size: medium

SLICE 18: `ignored_fields` becomes its own list, and the golden fixture is corrected
plan-items: 75, 76
why-this-order: after slice 17.
files: `pns/crates/pns/src/event_flow/submit.rs` (lines 89-91),
`pns/crates/pns-protocol/src/result.rs`, `pns/crates/pns-protocol/fixtures/result-v1.json` (line 11),
`pns/crates/pns-protocol/src/result/tests.rs` (line 36)
callers-to-update-in-the-same-slice: none. posture ignores the field.
behaviour-to-pin: a request with an unrecognized field answers with that field's name in
`ignored_fields` and nothing about it in `diagnostics`.
risk: none
size: small

SLICE 19: every destination outcome carries its reason, its route and its retry time
plan-items: 70, 71, 72, 73
why-this-order: after slice 17, which renamed the field it fills.
files: `pns/crates/pns/src/event_flow/submit/receipt.rs` (lines 32-36, 79-89, 96),
`pns/crates/pns-domain/src/routing.rs` (lines 24-34, 145-168),
`pns/crates/pns-application/src/ports/ledger.rs` (lines 43-47),
`pns/crates/pns-protocol/src/result.rs`, `pns/crates/pns/src/event_flow/submit/receipt/tests.rs`
(line 68 pins the rule that the event's own text never comes back), the result fixture
callers-to-update-in-the-same-slice:
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/posture/crates/posture-adapters/src/wire/result.rs`
(`DeliveryOutcome` at 38 gains `unknown`, `DestinationOutcome` gains `route` and `retry_at`)
behaviour-to-pin: a hermes leg that failed comes back with the destination's own sentence in `note`, its
submitted route in `route`, and a `retry_at` when the ledger will retry it; a channel that ran and said
nothing reads `silent` while a ledger that never learned the answer reads `unknown`.
risk: low. The new fields are additive on the wire; `note` starts carrying text that used to be dropped,
so the "never echo the event's own text" test is the one that matters.
size: medium

SLICE 20: omit absent optional fields, and refuse an unencodable destination name
plan-items: 77, 78
why-this-order: after slice 19, so it covers the fields that slice added.
files: `pns/crates/pns-protocol/src/result.rs` (lines 62, 70-76),
`pns/crates/pns-protocol/src/request.rs`, `pns/crates/pns/src/event_flow/submit/receipt.rs`
(lines 93-94, the `expect`), the destination registry where names are registered, both fixtures
callers-to-update-in-the-same-slice:
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/posture/crates/posture-adapters/src/wire/result.rs`
must tolerate an absent field where it read a `null`
behaviour-to-pin: an absent optional field is omitted rather than serialized as `null`, and a
destination name over 64 characters or carrying a control character is refused at registration instead
of panicking mid-delivery.
risk: low
size: small

SLICE 21: symmetric Rust type names, no wire change
plan-items: 117, 118 (settling 80 and 79)
why-this-order: after slice 20, so the renames land once on the final types.
files: `pns/crates/pns-protocol/src/request.rs` (line 108), `pns/crates/pns-protocol/src/result.rs`
(line 67), `pns/crates/pns-protocol/src/lib.rs` (line 50), `pns/crates/pns-domain/src/retry.rs`
(lines 27-34, 100, 143) and every use site inside pns
callers-to-update-in-the-same-slice: none. These are pns-internal type names; the schema strings stay
`pns.request/1` and `pns.result/1`, and posture has its own separate types.
behaviour-to-pin: nothing new. This is a rename, and `just test-rust` plus the unchanged golden fixtures
are the evidence the wire did not move.
risk: none
size: small

SLICE 22: `[remind]` and `[stale]` replace `[nag]`
plan-items: 42, 35, 100, and the `nag.stale_after_secs` third of 99
why-this-order: after slice 1 (the duration parser). Before slice 23, which adds the per-call switch to
the renamed table.
files: `pns/crates/pns/src/command_nag.rs` (becomes `command_remind.rs`),
`pns/crates/pns-application/src/arm_nag.rs` (becomes `arm_remind.rs`) and its `tests/`,
`pns/crates/pns/src/nag_schedule_runtime.rs`, `pns/crates/pns-adapters/src/config/nag.rs` (splits into
`remind.rs` and `stale.rs`), `pns/crates/pns-adapters/src/config/schema.rs` (the `nag` row and the
top-level row), `pns/crates/pns-adapters/src/config/render/layout/`,
`pns/crates/pns/src/invocation.rs`, `dot_config/pns/config-values.toml`,
`dot_config/pns/private_config.toml.tmpl` (regenerated)
callers-to-update-in-the-same-slice: nothing outside pns types `pns nag`; the daemon schedules it
internally. `private_dot_claude/modify_settings.json` mentions `[nag]` in a comment at line 348 and the
comment should move with the name.
behaviour-to-pin: `[remind] delay = "5m"` arms the reminder five minutes out, `[stale] escalate_after`
with `[stale] route` escalates a blocked session, and `[nag]` is refused as an unknown table.
risk: medium in the apply window. The deployed config still says `[nag]` until the operator applies, and
an unknown table is refused at load, which takes the whole config down rather than one feature. Apply in
the same sitting.
size: medium

SLICE 23: the reminder is switched on explicitly, never by the producer's name
plan-items: 1, 2, 3, 4, 5, 34, 36, 37, 10
why-this-order: after slice 3 (the producer word) and slice 22 (the `[remind]` table).
files: `pns/crates/pns-application/src/arm_remind.rs` (delete the `event.agent != "claude"` check at
old line 52 and the `CLAUDE_AGENT` constant at old line 144) and its `tests/`,
`pns/crates/pns/src/moshi_submission.rs` (line 112), `pns/crates/pns/src/legacy/argv.rs` (`--remind`,
`--no-remind`, `--remind=<duration>`), `pns/crates/pns-adapters/src/config/schema.rs` (a new
per-producer table), a new `config/producer.rs`,
`pns/crates/pns-adapters/src/config/render/layout/`, `pns/crates/pns/src/hook_dispatch.rs`
callers-to-update-in-the-same-slice: none yet. Slice 25 is what passes `--remind` from the harnesses;
until then no reminder arms, which is the plan's intended default.
behaviour-to-pin: the resolution order holds, most specific first: `--remind` beats the producer's config
entry, which beats the built-in default of off; an unset producer name arms nothing; and `--remind` with
no configured `delay` and no `=<duration>` is refused with exit 2 naming both fixes.
risk: high behaviourally, and deliberately. The moment this merges the approval reminder stops arming
for Claude Code, because nothing passes `--remind` yet. Slice 25 restores it. Either ship the two close
together or accept a gap where an unanswered approval prompt gets no nudge.
size: medium

SLICE 24: warn when a reminder is armed for a producer that sends no answered signal
plan-items: 7
why-this-order: after slice 23.
files: `pns/crates/pns-application/src/arm_remind.rs`, `pns/crates/pns/src/hook_dispatch.rs`
callers-to-update-in-the-same-slice: none
behaviour-to-pin: arming a reminder for a producer with no answered signal writes one line to stderr and
nothing to stdout, and the `[remind]` staleness cap still stops the reminder.
risk: low, with one sharp edge: Claude Code parses the hook's stdout, so a warning that leaks there
corrupts the hook's answer. That is the whole behaviour to pin.
size: small

SLICE 25: every harness wires its waiting and answered events
plan-items: 56, 57, 58, 59
why-this-order: straight after slice 23, which is what `--remind` means. Before slice 26.
files: `private_dot_claude/modify_settings.json` (the blocked declaration at line 411 gains `--remind`),
`dot_local/libexec/pns/hooks/codex/executable_install-hooks.sh` (wire `PostToolUse` and `Interrupt` to
`pns hook resolved` beside today's `PermissionRequest` and `Stop`),
`private_dot_hermes/modify_private_config.yaml` (a hooks block wiring `pre_approval_request` to
`pns hook blocked --remind` and `post_approval_response` to `pns hook resolved`)
callers-to-update-in-the-same-slice:
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/test/unit/pns-codex-hook-migration.test.sh`, which pins
the installer's generated rows and must grow the two new events
behaviour-to-pin: Claude Code's blocked declaration passes `--remind`, and the Codex installer writes a
`PostToolUse` row pointing at `pns hook resolved` while preserving rows it did not write.
risk: medium. `modify_settings.json` and `modify_private_config.yaml` are modify-templates over files
the apps rewrite themselves, so both must keep their read-back-live-state behaviour. hermes's event
names are worth confirming with `hermes hooks test <event>` before merging rather than trusting the
plan.
size: medium

SLICE 26: `remind` in the JSON
plan-items: 25
why-this-order: after slice 23.
files: `pns/crates/pns-protocol/src/request.rs`, `pns/crates/pns/src/event_flow/submit/mapping.rs`,
`pns/crates/pns-protocol/fixtures/request-v1.json`
callers-to-update-in-the-same-slice: none. No in-tree JSON producer arms a reminder.
behaviour-to-pin: `"remind": true` uses the configured delay, `"remind": "5m"` overrides it for that
request, and `"remind": false` matches `--no-remind`.
risk: none
size: small

SLICE 27: strict bad input on both paths
plan-items: 32
why-this-order: last of the request-envelope work, after slices 9 to 15 and 26, so it refuses against
the final field list. Item 64's refusal already landed in slice 15 and stays.
files: `pns/crates/pns/src/legacy/argv.rs` (the lenient skip and the missing-value warning),
`pns/crates/pns/src/event_flow/submit.rs` (the ignored-field diagnostic path)
callers-to-update-in-the-same-slice: none, if slices 2 through 15 moved every caller. Re-run
`git grep -n 'cargo/bin/pns\|libexec/pns/pns'` across the repo as the check.
behaviour-to-pin: an unknown flag, a flag missing its value, and a value outside a closed set are each
refused with exit 2 and named, on the flag path and the JSON path alike.
risk: medium. This is the slice that turns every missed caller from a quiet degradation into a hard
refusal, which is why it comes after all of them and not before. That is also its value.
size: small

SLICE 28: `pns mute` replaces `pns quiet`
plan-items: 43
why-this-order: independent of the request work. After slice 15, which already introduced `bypass_mute`.
files: `pns/crates/pns/src/command_quiet.rs` (becomes `command_mute.rs`),
`pns/crates/pns/src/command_lights.rs`, `pns/crates/pns-domain/src/quiet.rs` (becomes `mute.rs`),
`pns/crates/pns/src/invocation.rs`, `pns/crates/pns-adapters/src/config/render/layout/` (prose),
`dot_config/pns/private_config.toml.tmpl` (regenerated)
callers-to-update-in-the-same-slice: none. Both are operator-typed.
behaviour-to-pin: `pns mute 2h` mutes for two hours, `pns lights mute "<place>" off` clears a place, and
`pns quiet` is refused.
risk: none. Note `ReportMode::Silent` serializing as the word `async` on the channel contract
(`routing.rs:24-34`) is a third meaning of the word and is deliberately left alone by this slice.
size: medium

SLICE 29: point-in-time flags say epoch
plan-items: 45, 41, and the `daemon schedule` half of 39
why-this-order: after slice 1.
files: `pns/crates/pns-application/src/build_return_recap/window.rs`,
`pns/crates/pns/src/command_daemon.rs` (line 73), `pns/crates/pns/src/command_recap.rs`
callers-to-update-in-the-same-slice:
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/pns/crates/pns-adapters/src/recap_child.rs` line 47,
which spawns the recap child with `--since`/`--until`
behaviour-to-pin: `pns recap --since-epoch <seconds>` selects the window, `pns daemon schedule --until
+30m` takes a duration while `--until-epoch <seconds>` takes a timestamp, and `--in` and `--every` take
durations.
risk: low. The one caller is pns re-executing itself through `current_exe`, so it is always the matching
version.
size: small

SLICE 30: pns owns its environment prefix, and spells its words out
plan-items: 81 (the prefix half), 86
why-this-order: independent; before slice 31.
files: `pns/crates/pns-adapters/src/process/settings.rs` (line 20, `MOSHI_HOOK_BIN`),
`pns/crates/pns-adapters/src/codex.rs` (line 21, `CODEX_BIN`),
`pns/crates/pns-domain/src/decision/overrides.rs` (lines 88-89, `PNS_IDLE_SECS` and
`PNS_DESK_IDLE_SECS`)
callers-to-update-in-the-same-slice:
`/Users/stephen/workspaces/Ivy/webdavis/dotfiles/pns/crates/pns/tests/hooks/approval_forwarding.rs`
line 70 sets `PNS_IDLE_SECS`. Nothing in the deployed tree sets any of the four.
behaviour-to-pin: `PNS_MOSHI_HOOK_BIN`, `PNS_CODEX_BIN`, `PNS_SCREEN_IDLE` and `PNS_DESK_IDLE` are read
and the old names are not. `REPORT_LIB_PLAIN` and `NO_COLOR` keep their names.
risk: none. No deployed file exports these.
size: small

SLICE 31: delete the environment variables that duplicate a config key
plan-items: 82, and the `HUE_PULSE_ROOMS` half of 81
why-this-order: after slice 30. Before slice 45, which removes `plugins.hue.rooms` itself.
files: `pns/crates/pns/src/lamp_pulse.rs` (line 56), `pns/crates/pns-adapters/src/hue/settings.rs`
(line 25), `pns/crates/pns-domain/src/decision/` (the phone marker read),
`pns/crates/pns-adapters/src/config/render/layout/` (the prose at template line 369 naming
`PNS_PHONE_MARKER_FILE` as taking precedence), `dot_config/pns/private_config.toml.tmpl` (regenerated)
callers-to-update-in-the-same-slice: none. `git grep` finds no deployed file exporting
`PNS_PHONE_MARKER_FILE`, `HUE_PULSE_ROOMS`, `PNS_MOSHI_SUBMIT_DEADLINE_MS` or
`PNS_PULSE_THRESHOLD_SECS`.
behaviour-to-pin: the config key is the only source for the phone marker file, the pulse rooms, the
phone acknowledgement deadline and the loop threshold; the environment variable is ignored.
risk: low. If the operator has one of these exported in a shell profile the setting silently reverts to
config, which is the intent.
size: small

SLICE 32: a config key for every durable setting that had only an environment variable
plan-items: 83
why-this-order: after slice 31.
files: `pns/crates/pns-adapters/src/persistence/state_dir.rs` (line 6),
`pns/crates/pns/src/channel_dispatch.rs` (lines 53, 74, 134, 151, 168),
`pns/crates/pns-adapters/src/config/schema.rs`, `pns/crates/pns-adapters/src/config/delivery.rs`
(`remote_deadline` beside `max_attempts`), `pns/crates/pns-adapters/src/config/render/layout/`,
`dot_config/pns/private_config.toml.tmpl` (regenerated)
callers-to-update-in-the-same-slice: none
behaviour-to-pin: each of the six settings reads from config when the file names it and from its
environment variable otherwise, and `PNS_REMOTE_TIMEOUT` is gone in favour of
`[delivery] remote_deadline`.
risk: low
size: medium

SLICE 33: every duration environment variable takes a duration string
plan-items: 84, 85
why-this-order: after slices 1, 31 and 32, so it covers the variables that survive.
files: `pns/crates/pns-adapters/src/destinations/`, `pns/crates/pns-adapters/src/process/settings.rs`,
`pns/crates/pns-application/src/daemon.rs` (line 123),
`pns/crates/pns/src/turn_text.rs` (lines 82-85, float seconds today),
`pns/crates/pns-domain/src/decision/overrides.rs` (line 90, `PNS_PHONE_INPUT_AGE`, no unit today)
callers-to-update-in-the-same-slice: none deployed.
behaviour-to-pin: `PNS_PAYLOAD_DEADLINE=500ms` is honoured, a bare number is refused, and the surviving
names use one word per kind of knob (`deadline` bounds one operation, `interval` repeats, `delay` waits).
risk: low
size: medium

SLICE 34: the two test knobs leave production builds
plan-items: 116 (settling 88)
why-this-order: after slice 33.
files: `pns/crates/pns-adapters/src/persistence/ring.rs` (lines 158-162),
`pns/crates/pns-adapters/src/persistence/sqlite/store.rs` (lines 35, 40),
`pns/crates/pns-adapters/src/config/schema.rs` (a new `[storage]` table),
`pns/crates/pns-adapters/src/config/render/layout/`, `dot_config/pns/private_config.toml.tmpl`
(regenerated)
callers-to-update-in-the-same-slice: none
behaviour-to-pin: `PNS_RING_LOCK_TEST_DELAY_MS` set in a release build changes nothing, and
`[storage] busy_deadline` sets SQLite's busy bound.
risk: low, and it closes one: today a stray variable in a real environment sleeps inside a locked
section.
size: small

SLICE 35: the recap's text shortener is the summarizer everywhere
plan-items: 119 (settling 87)
why-this-order: after slice 31, which is where the condenser variable's removal was noted; this slice is
what actually does it.
files: `pns/crates/pns-adapters/src/codex.rs`, `pns/crates/pns-adapters/src/config/recap.rs`,
`pns/crates/pns-adapters/src/config/schema.rs`, `pns/crates/pns/src/tests/`,
`pns/crates/pns-adapters/src/config/render/layout/`, `dot_config/pns/private_config.toml.tmpl`
(regenerated)
callers-to-update-in-the-same-slice: none
behaviour-to-pin: `[recap] summarizer_deadline` takes a duration and bounds the summarizer, and
`PNS_CONDENSER_DEADLINE_MS` is not read.
risk: none
size: small

SLICE 36: plugin tables are named for their function
plan-items: 90
why-this-order: first of the config-table block, because the later plugin slices sit inside the tables it
renames.
files: `pns/crates/pns-adapters/src/config/schema.rs` (the `plugins.hue`, `plugins.macos-banner` and
`plugins.router` rows plus the top-level row), `pns/crates/pns-adapters/src/config/banner.rs`,
`config/router.rs`, `config/router_tests.rs`, `config/plugins.rs`,
`pns/crates/pns-adapters/src/config/render/layout/`, `pns/crates/pns/src/command_home.rs`,
`dot_config/pns/config-values.toml`, `dot_config/pns/private_config.toml.tmpl` (regenerated)
callers-to-update-in-the-same-slice: none outside pns read these tables.
behaviour-to-pin: `[plugins.lights] type = "hue"`, `[plugins.banner] type = "macos"` and
`[plugins.home_presence] type = "unifi"` arm their plugins, and the old table names are refused by name
with the new spelling in the message.
risk: medium in the apply window, like every config slice: an unknown table is refused at load, so the
whole config is down between merge and apply.
size: medium

SLICE 37: one durable-log table
plan-items: 91
why-this-order: after slice 36.
files: `pns/crates/pns-adapters/src/config/schema.rs`, `config/discord.rs`, `config/hermes.rs`,
`config/plugins.rs` (delete `refuse_two_durable_logs`), `config/render/layout/`,
`dot_config/pns/config-values.toml` (the `[plugins.hermes]`, `[plugins.hermes.keys]`,
`[plugins.discord]` and `[plugins.discord.channels]` blocks collapse to `[plugins.log]`),
`dot_config/pns/private_config.toml.tmpl` (regenerated)
callers-to-update-in-the-same-slice: none. The per-route key names and channel ids are KeePassXC entry
titles in the values file and do not change.
behaviour-to-pin: `[plugins.log] type = "hermes"` posts through hermes and `type = "discord"` through
Discord, and the two can no longer both be declared because there is one table.
risk: high in the apply window. This is the durable log, so between merge and apply nothing reaches
`#pns-events`. It is also the slice the operator's own KeePassXC-backed secrets flow through, so the
render needs the vault unlocked.
size: medium

SLICE 38: the phone plugin absorbs `[phone]`
plan-items: 92, and the `mobile_watch_card` and `submit_deadline_secs` parts of 108 and 109
why-this-order: after slice 36.
files: `pns/crates/pns-adapters/src/config/schema.rs`, `config/mobile.rs` (becomes `phone.rs`),
`config/phone.rs` (the old top-level table), `config/render/layout/`,
`dot_config/pns/config-values.toml`, `dot_config/pns/private_config.toml.tmpl` (regenerated)
callers-to-update-in-the-same-slice: none
behaviour-to-pin: `[plugins.phone] type = "moshi"` carries `marker_file`, `card_while_watching` and
`ack_deadline` (a duration), and `[phone]` and `[plugins.mobile]` are refused.
risk: medium, apply window. Phone delivery is down until the operator applies.
size: medium

SLICE 39: one word for a credential, a route, and a host
plan-items: 94, 95, and the `router_url` and `bridge` parts of 108
why-this-order: after slices 36, 37 and 38, whose tables hold these keys.
files: `pns/crates/pns-adapters/src/config/schema.rs`, `config/hermes.rs`, `config/discord.rs`,
`config/router.rs`, `pns/crates/pns-adapters/src/hue/settings.rs` (line 32),
`pns/crates/pns/src/command_home.rs` (line 52, whose variable is already `alert_route`),
`config/render/layout/`, `dot_config/pns/config-values.toml`,
`dot_config/pns/private_config.toml.tmpl` (regenerated)
callers-to-update-in-the-same-slice: none
behaviour-to-pin: a single credential is `key` and a map of them is `keys`, the stale alert names
`alert_route`, and the bridge and the router each name a host in one shape.
risk: medium, apply window. Note the open question about `[plugins.github] token`, below.
size: medium

operator-ruling-2026-09-17: the credential keys are named for the secret each tool issues, spelled out,
taking the wording from the KeePassXC entry: `plugins.mobile.token` becomes `device_token`,
`plugins.discord.token` becomes `bot_token`, `plugins.github.token` becomes `personal_access_token`,
`plugins.hue.key` becomes `api_key`, and `plugins.router.api_key` is already right. This reverses item
94's `key`/`keys` rule. `plugins.hermes.keys.<route>` stays a map of route keys and `plugins.hue.bridge`
is a host rather than a credential.

fold-in-here (operator approved 2026-09-17): introduce a `Secret` newtype in the same slice, because
renaming a key and changing its type in one breaking change is cheaper than two. `mobile.rs` and
`router.rs` both carry the comment that the key "never enters a type that derives Debug, so it cannot
ride a formatted dump into a log line", and both return `Option<String>`, which derives both `Debug` and
`Display`. The invariant is therefore prose in two files rather than a property of the type, and one
`#[derive(Debug)]` on a struct that happens to hold a key puts a secret one `{:?}` from a log line. Give
`Secret(String)` no `Debug`, no `Display` and no `Serialize`, so a formatting attempt fails to compile,
and add one shared `secret(table, name) -> Option<Secret>` holding the shape check. The key name stays a
parameter and each plugin keeps its own policy for a missing value, which is deliberate and must not be
flattened: `mobile` and `router` return `None` meaning "not set up, never an error", while `github`
returns a named refusal because its poll is opt-in and a missing token there is a mistake. Expect an
explicit `.expose()` at each egress point where the secret becomes a header value; that call site is the
audit surface and is a feature of the change rather than a cost of it.

SLICE 40: presence keys say what they measure
plan-items: 107, the two presence thirds of 99, and the `poll_secs` part of 109
why-this-order: after slice 36.
files: `pns/crates/pns-adapters/src/config/schema.rs`, `config/presence.rs` (lines 93-108),
`config/presence_values.rs`, `config/render/layout/`, `dot_config/pns/config-values.toml`,
`dot_config/pns/private_config.toml.tmpl` (regenerated)
callers-to-update-in-the-same-slice: none
behaviour-to-pin: `excluded_rooms` is subtracted from `rooms` with `desk_room` required to be in one and
not the other, and `poll_interval`, `reading_max_age` and `desk_input_max_age` each take a duration.
risk: medium, apply window. Presence drives the phone-versus-banner decision, so a config refusal here
costs escalation.
size: medium

SLICE 41: lights behaviour and state names
plan-items: 93, 98, the `[lights.unread]` rows of the Names table
why-this-order: first of three lights slices, because `behaviours` is the key the other two write values
into.
files: `pns/crates/pns-adapters/src/config/schema.rs` (`lights.github`, `lights.unread`, `TARGET_KEYS`),
`config/lights_tables.rs`, `config/lights_targets.rs` (lines 34, 37), `config/render/lights.rs`,
the lamp behaviour enum in `pns-domain`, `dot_config/pns/config-values.toml` (every `shows = [...]` and
`dim_behaviours = [...]` list, including the `"github"` entries),
`dot_config/pns/private_config.toml.tmpl` (regenerated)
callers-to-update-in-the-same-slice: none. The lamps are pns's own.
behaviour-to-pin: `[lights.checks]` with `pass_color` and `fail_color` lights a checks result,
`[lights.unseen]` is the finished-run lamp, and a target declares `behaviours = ["checks", ...]`.
risk: medium, apply window. The lamps go to their unconfigured state until the operator applies.
size: medium

SLICE 42: lights timing keys
plan-items: 97, 105, the `lights.loop threshold_secs` and `lights.unseen after_secs` parts of 99 and 109
why-this-order: after slice 41.
files: `pns/crates/pns-adapters/src/config/schema.rs`, `config/lights_tables.rs`,
`config/lights_bounds.rs` (line 44, `MIN_LEASE_TIMEOUT_SECS`), `config/render/lights.rs`,
`dot_config/pns/config-values.toml` (`[lights.loop] threshold_secs = 360` becomes `arm_after = "6m"`),
`dot_config/pns/private_config.toml.tmpl` (regenerated)
behaviour-to-pin: `lease_expiry` is the one word for a held lamp's expiry in both `[lights.loop]` and
`[lights.blocked]`, `arm_after` is when a lamp arms in both `[lights.loop]` and `[lights.unseen]`, and
`[lights] refresh_secs` is two settings, `arm_interval` and `fade_duration`.
callers-to-update-in-the-same-slice: none
risk: medium, apply window. `refresh_secs` splitting needs two values where one existed; see the open
question below.
size: medium

SLICE 43: lights percentages, durations, and one dim window
plan-items: 103, 96, 106, 111, the `duration_ms` and `flare` rows of the Names table
why-this-order: after slice 42. It removes `plugins.hue.rooms`, so it comes after slice 31 removed the
environment variable that duplicated it.
files: `pns/crates/pns-adapters/src/config/schema.rs`, `config/lights_tables.rs` (lines 172, 176),
`config/lights_targets.rs`, `config/render/lights.rs`, `dot_config/pns/config-values.toml` (the
`[plugins.hue] rooms` and `quiet_hours` lines go, `dim_window` becomes a `[lights]` default, and a
`[lights.zone]` example is added), `dot_config/pns/private_config.toml.tmpl` (regenerated)
callers-to-update-in-the-same-slice: none
behaviour-to-pin: `brightness_percent`, `high_percent`, `low_percent` and `flare_percent` take 1 to 100
while `duration` and `flare_duration` take durations, and `[lights] dim_window` is the default a place
may override with `plugins.hue.quiet_hours` gone.
risk: medium, apply window.
size: medium

SLICE 44: delivery keys match what the code does with them
plan-items: 104
why-this-order: after slice 32, which added `remote_deadline` to the same table.
files: `pns/crates/pns-adapters/src/config/schema.rs`, `config/delivery.rs`, `config/retry.rs`,
`pns/crates/pns-domain/src/retry.rs` (lines 100, 143), `config/render/layout/`,
`dot_config/pns/private_config.toml.tmpl` (regenerated)
callers-to-update-in-the-same-slice: none
behaviour-to-pin: `max_retries` is the retry ceiling the comparison uses, `retry_step` is the per-retry
increment the wait multiplies, and `event_max_age` is measured against the original event's age.
risk: low. These are all shipped at defaults today.
size: small

SLICE 45: recap and failures keys say what they control
plan-items: the recap and failures parts of 109
why-this-order: after slice 35, which already touched the recap table.
files: `pns/crates/pns-adapters/src/config/schema.rs`, `config/recap.rs`, `config/recap_sources.rs`
(line 19, whose parser is already called `repositories`), `config/recap_values.rs`, `config/failures.rs`
and `config/failures/`, `config/render/layout/`, `dot_config/pns/private_config.toml.tmpl`
(regenerated)
callers-to-update-in-the-same-slice: none
behaviour-to-pin: `post_window_recap`, `minimum_events`, `repositories` and `review_notes_glob` do what
`digest`, `min_events`, `repos` and `review_notes` did, and `[failures] page_enabled` and `page_port`
configure the HTTP listener.
risk: low
size: small

SLICE 46: no key doubles as its own switch, and the `enabled` defaults are written out
plan-items: 101, 102
why-this-order: after slices 22, 42 and 45, because the keys it un-overloads are already renamed by
then. `[remind] delay` unset meaning off is the rule slice 22 established; this slice generalizes it.
files: `pns/crates/pns-adapters/src/config/focus.rs` (lines 5-9), `config/recap/options.rs` (line 23),
`config/recap_sources.rs`, `config/daemon.rs` (line 4), `config/load.rs` (line 65),
`config/schema.rs`, `config/render/layout/`, `dot_config/pns/config-values.toml`,
`dot_config/pns/private_config.toml.tmpl` (regenerated)
callers-to-update-in-the-same-slice: none
behaviour-to-pin: an unset duration means off, `[focus] modes` is the roster with `[focus] enabled` the
switch, and the shipped config writes every `enabled` out at its own default rather than leaving
`[daemon]` true and `[plugins.*]` false unstated.
risk: medium. This is where a setting the operator relied on being implicitly on could flip off. Read the
rendered diff carefully.
size: medium

SLICE 47: the plural rule, one way round
plan-items: 110, 114
why-this-order: last of the config block, after every table it has to judge exists in its final form.
files: `pns/crates/pns-adapters/src/config/schema.rs` (the top-level row and every table name),
every `config/*.rs` whose table name changes, `config/render/layout/`,
`dot_config/pns/config-values.toml`, `dot_config/pns/private_config.toml.tmpl` (regenerated)
callers-to-update-in-the-same-slice: none
behaviour-to-pin: a table holding a set is plural, a table keyed by one name is singular, a list key is
plural, and the old spelling of each renamed table is refused by name.
risk: medium, apply window, and it is the widest single config diff. Worth reading the rendered template
diff line by line.
size: medium

SLICE 48: the values file stops using presence and an open table as settings
plan-items: 112, 113
why-this-order: after slices 37 (which reshaped the keys table) and 46 (which made unset mean off, so
the empty `[nag]` trick has nothing left to do).
files: `dot_config/pns/config-values.toml` (the `note` at lines 29-32 and the empty `[nag]` at line 90),
`pns/crates/pns-adapters/src/config/render/write.rs` (lines 120-146, the `note` stripper),
`dot_config/pns/private_config.toml.tmpl` (regenerated)
callers-to-update-in-the-same-slice: none
behaviour-to-pin: a gateway route actually named `note` can be given a key, and no table's mere presence
changes what the render writes.
risk: low
size: small

SLICE 49: pi and omp gain reminders, and their dead pns gate is fixed
plan-items: 60, 61, 115
why-this-order: last. It depends on slice 23 for `--remind`, on slice 6 for the surviving gate spelling,
and on a fact about omp's extension API that has to be confirmed before the code is written.
files: a new pns extension for pi and omp (the plan does not say where it lives in this repo; the
existing moshi extensions are at `~/.pi/agent/extensions/moshi-hooks.ts` and the `~/.omp/` twin, which
are NOT tracked here), `.chezmoiscripts/run_after_62-bounce-moshi-hook-on-upgrade.sh.tmpl` (whose
repoint is reported as wired and changes nothing, so its report must stop claiming it)
callers-to-update-in-the-same-slice: `.chezmoiscripts/run_after_62-bounce-moshi-hook-on-upgrade.sh.tmpl`
lines 74, 88, 92 and 103, the warnings that describe a gate that was never called
behaviour-to-pin: pi's `ui_prompt_start` arms a reminder and `ui_prompt_end` clears it; when omp has no
equivalent pair, omp's setup output says reminders are off for omp rather than arming one it cannot
clear.
risk: medium, and it is the one slice whose target files are largely outside this repository. Confirm
`ui_prompt_start`/`ui_prompt_end` against pi-mono `packages/coding-agent/docs/extensions.md` and omp's
own extension docs before writing anything. The plan already allows omp to ship without reminders.
size: medium

---

TOTAL SLICES: 49

## Operator rulings

- 2026-09-17: the per-producer table is SINGULAR, `[producer.<name>]`, not `[producers.<name>]`. This
  settles question 1 below and unblocks slices 23 and 47. The plan's own item 3 wrote it plural, which
  predates the plural rule in item 110; item 110 wins, because the table is one producer keyed by its
  name rather than a set, which is the same shape as `[delivery_class.<name>]` and
  `[lights.lamp."<name>"]`. `pns/docs/pns-refactor.md` has been corrected in place so the plan no longer
  contradicts itself, and question 1 below is answered rather than open.

- 2026-09-17: `max_age` IS a fourth allowed time word, defined as a bound on how stale a value may be,
  and `PNS_PHONE_INPUT_AGE` becomes `PNS_PHONE_INPUT_MAX_AGE` to match. This settles question 2 below and
  unblocks slices 33, 40 and 44. The plan's item 85 listed `age` among six words naming one idea, while
  its own rename table kept `age` in `event_max_age`, `reading_max_age` and `desk_input_max_age`; the
  table wins. `deadline`, `interval` and `delay` all point forward at work, whereas these three judge a
  value already held (the plan itself notes `delivery.max_age_secs` measures the ORIGINAL EVENT's age),
  so folding them into `deadline` would make each read as a timeout on OBTAINING the value, which is a
  different knob and would recreate the ambiguity item 85 exists to remove. `pns/docs/pns-refactor.md`
  has been corrected in place, so questions 1 and 2 below are both answered rather than open; questions
  3 and 4 remain.

- 2026-09-17: slicing question 4 is ANSWERED BY CORRECTION, not by a number. Item 105 claimed
  `lights.refresh_secs` was both the daemon re-arm interval and the breath-fade budget and should split
  into two settings. It is not: `refresh_secs` is read in `lamp_registration.rs:90` as the re-arm
  interval and in `maintain_lamps.rs:75` to build `tick_bridge_deadline`, which takes a fifth of it as
  the budget for ONE bridge call so three calls cannot outlive the interval that spawned them, which the
  function's own comment states as deliberate coupling. A fade is `duration_ms` on each state's table
  (4000 for `done`, `failed` and `github`, 2000 for the `blocked` breath), never `refresh_secs`. So there
  is no second number to choose, slice 42 carries no split, and item 105 is now a plain rename of
  `refresh_secs` to `arm_interval`. `pns/docs/pns-refactor.md` is corrected in place.

- 2026-09-17: slicing question 3 is ANSWERED, and the answer renames four keys rather than the one an
  earlier reading of it suggested. Each credential is named for the kind of secret its own tool issues,
  spelled out in full, with the KeePassXC entry title as the authority:
  `plugins.mobile.token` becomes `device_token`, `plugins.discord.token` becomes `bot_token`,
  `plugins.github.token` becomes `personal_access_token`, `plugins.hue.key` becomes `api_key`, and
  `plugins.router.api_key` is already correct. `plugins.hermes.keys.<route>` stays a map of route keys,
  and `plugins.hue.bridge` is a host address rather than a credential. This reverses item 94, which
  wanted `key` and `keys` everywhere, and it also satisfies the repository's spell-words-out naming
  rule. `[plugins.github]` does not take item 90's `type = "<vendor>"` shape, because that shape is for
  delivery destinations and GitHub is a notification source. If one internal type helps the Rust it
  lives in the code, never in the file a human reads. Slices 36 and 39 are unblocked, and each is a
  breaking config rename that must move its own callers in the same change.

BLOCKED-ON-OPERATOR:

1. Item 3 writes the per-producer reminder setting as `[producers.<name>] remind`, plural. Item 110's
   plural rule says a table keyed by one name is singular, and gives `[delivery_class.<name>]` and
   `[lights.lamp."<name>"]` as the pattern. Which wins: `[producer.<name>]` or `[producers.<name>]`?
   Slices 23 and 47 both need the answer, and slice 23 comes first.

2. Item 85 says `age` is one of the six words that all name the same kind of time value and must be
   reduced to `deadline`, `interval` or `delay`. The Names table's row for `PNS_PHONE_INPUT_AGE` says
   "unchanged names, duration values", keeping `age`. Item 107's `reading_max_age` and
   `desk_input_max_age` and item 104's `event_max_age` also keep it. Is `max_age` a fourth allowed word
   (a bound on how old a reading may be, which is genuinely not a deadline, an interval or a delay), and
   does `PNS_PHONE_INPUT_AGE` become `PNS_PHONE_INPUT_MAX_AGE` to match? Slices 33, 40 and 44 need this.

3. Item 94 lists four names for one credential concept and says use `key` and `keys`, naming
   `plugins.mobile.token`, `plugins.discord.token`, `plugins.hue.key` and `plugins.router.api_key`. It
   does not mention `plugins.github.token` (`schema.rs`, the `plugins.github` row), which is the same
   kind of value. Does it become `[plugins.github] key` too, and does `[plugins.github]` get the
   function-table treatment of item 90 (it is a notification SOURCE, not a destination, so the
   `type = "<vendor>"` shape may not fit)? Slices 36 and 39 need this.

4. Item 105 splits `[lights] refresh_secs` into `arm_interval` and `fade_duration`, two settings where
   one value exists today. The plan gives no value for either. What does each ship at? Slice 42 needs
   two numbers, and guessing one changes how often the lamps re-arm.
