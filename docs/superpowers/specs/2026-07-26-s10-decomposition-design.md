# S10 decomposition design

- Date: 2026-07-26
- Status: approved (slice list), pending spec review
- Base: `main` at `d331171`
- Decomposition input: pull request #53, branch `feat/s10-macos-defaults-ssh`, tip `cfaa446`

## Context

S9 (the osquery three-tier alerting feature-set) was originally merged as one 4,300-line pull request
with no pre-merge review. It has now been re-landed on `main` as fifteen reviewed slices plus six
follow-up pull requests, and the operator has ruled that S10 and every project after it gets the same
treatment: decomposed into small, self-supporting, dependency-ordered pull requests, each run through the
per-slice review loop. The existing monolithic branch is a reference to re-land from, never a direct
merge.

S10 is macOS security posture. Its product requirements document is already written and its conclusions
are settled; this design builds on top of them and does not re-derive them. The findings that shape every
slice below, all verified live on macOS 26.2:

- The repository manages zero security-relevant macOS settings today. All seven tracked records in
  `.chezmoidata/macos_defaults.yaml` are cosmetic Aerospace window-management keys.
- On macOS 26 most security controls are no longer settable from the command line. `spctl` no longer
  exposes `--master-enable`, so Gatekeeper cannot be enabled by command. `socketfilterfw` on 26.2 has no
  logging-mode flag. `fdesetup enable` is interactive. `sysadminctl -screenLock` requires `-password`.
- Live posture: FileVault on, Gatekeeper enabled, System Integrity Protection (SIP) deliberately
  disabled, firewall on with stealth on, screen lock immediate, auto-login none, guest disabled, Remote
  Login on, `SoftwareUpdate AutomaticCheckEnabled` unset while all four of its dependents are on, Safari
  `AutoOpenSafeDownloads` unset, sudo passwordless.
- The `macos-defaults-{apply,capture,drift}` helpers are user-domain only. They cannot express a
  `/Library/Preferences` setting, which is where most security settings live.

The removed-flag finding is the one that forces the architecture. A script that shells out to a flag the
operating system no longer honors looks like it is enforcing a control and is in fact doing nothing. That
failure is silent, and it is worse than not having the control at all, because the operator believes the
machine is protected. So every declared control carries an explicit tier, and the runner refuses to treat
a control it cannot actually set as if it had set it.

## Goal

Land S10 as ten small, self-supporting, dependency-ordered pull requests that each survive a single
sitting of review, and end with a macOS security posture that is declared in data, enforced where the
operating system still permits enforcement, verified and alerted on where it does not, and written down
in the runbook where neither is possible.

Each pull request:

1. is self-supporting: merged on top of its predecessors it references nothing unshipped, and its own
   tests pass standalone;
1. is small enough to review in one sitting, with no single new file over roughly 120 lines; and
1. states every intentional divergence from pull request #53 in its own description.

Behavior drift from `cfaa446` is expected and welcome. Pull request #53 is a starting reference, not a
fidelity target. Three of the ten slices (2, 3, 5) have no counterpart in the monolith at all; they are
net-new design from the product requirements document. Five (1, 4, 6, 7, 8) draw on monolith content and
improve it under review. Two (9, 10) take a single cask line each from the monolith and build the
posture work around it that the monolith never had.

## Constraints and standing policy

- Self-supporting is the hard constraint. It is enforced by the dependency graph, not by file count.
- Per-slice review loop, with review weight scaled by blast radius (see each slice). The code-quality
  dimension applies to every slice regardless of security weight: single responsibility, no hardcoded
  paths or values, no duplication, self-documenting names, testability.
- Slice 8 gets a personal operator review before merge. It is the first slice in this program that can
  lock the operator out of the machine.
- Feature-branch workflow: pull requests merge to `main` on GitHub with subject
  `Merge pull request #N from webdavis/<branch> (#N)` using `--merge`. Never rebase onto `main`.
- Never a `Co-Authored-By` trailer and never a generated-with footer on any commit.
- Pull request descriptions go through the humanizer skill and never describe the internal workflow.

### Recurring bug classes every slice inherits

Every implementer brief carries this checklist, and every reviewer checks against it:

1. Fail safe, never fail quiet. A check that cannot run reports failure, never success.
1. Two-sided bounds. Both ends of every range are checked, not just the one that broke last time.
1. Notify before persist. The alert fires first, the state marker is written only on success.
1. Verify the real artifact, not a proxy. A loaded launchd job is not a listening daemon; a written file
   is not an applied setting.
1. Untrusted data crosses boundaries inert. Never interpolate a value read from the system into a shell
   command or a notification body without neutralizing it.
1. Validate and range-bound before arithmetic. A non-numeric or absent value must not reach an
   expression.
1. Tests that can actually fail. Every assertion is mutation-verified. No bare `! grep` in a bats body,
   because `set -e` ignores an inverted status and the assertion is dead. Use an explicit refute helper.
1. No whitespace field splitting where a field can be empty. Read tab-separated records with
   `IFS=$'\t'`, never bare word splitting.
1. Completeness guards over count guards. Assert the exact set, not how many.
1. Enumerate and diff when several lists must describe one set. Do not eyeball two lists for agreement.
1. Atomic, owner-only state. Write to a temporary file and rename; never leave a partial record.
1. Honest comments. A comment must never claim a property the code does not have.

### Style conventions every slice inherits

- `set -euo pipefail`, quoted expansions, `[[ ]]` not `[ ]`, arrays for command construction, no
  abbreviations in names.
- Shell formatting `shfmt -i 2 -ci -s`; markdown wrapped at 105 columns.
- No em-dashes anywhere, in code, comments, commits, or pull request descriptions.
- No apostrophes inside single-quoted `jq` programs.
- Tests are placed by suite design, not by default: `test/unit` for pure and stubbed logic, and
  `test/integration` for anything that renders a real template or drives a real external parser. The
  suite guard rejects strays.
- Explicit staging paths on every `git add`. Never `git add -A`.
- The generated `graphify-out/graph.json` is not carried per slice. It is regenerated once, after slice
  10, in its own chore commit.
- The SSH slices (7 and 8) run under `#!/bin/bash`, which on macOS is bash 3.2. No associative arrays, no
  `compgen -G`. Their tests invoke the script through `/bin/bash` explicitly so a 3.2-only regression
  fails in the test rather than in production.

## The three-tier control model

This is the spine of S10 and it ships in slice 3, before any security control is declared.

### Where a tier is declared

Every record in `.chezmoidata/macos_defaults.yaml` (under `macos.defaults`) and every record in
`.chezmoidata/macos_system_setup.yaml` (under `macos.system_setup`) carries a required `tier` field with
exactly one of three values:

| `tier`    | Meaning                                                                        |
| --------- | ------------------------------------------------------------------------------ |
| `enforce` | The control is settable from the command line. Declare it and apply it.         |
| `verify`  | The control is readable but not settable. Detect drift and route it to alerting. |
| `manual`  | The control needs an interactive step or a device-management profile.            |

A `verify` record additionally carries the read it is checked by and the value it must hold. A `manual`
record additionally carries a required `runbook` field naming the runbook section that tells the operator
how to set it by hand.

The tier is a property of the control on this operating system version, not a preference. A control moves
from `enforce` to `verify` when Apple removes the flag that set it, and that move is a one-line data
change plus a runbook entry, not a script rewrite. That is the whole point of putting the tier in data.

### How the runner enforces the distinction

Both runners branch on the tier at template-render time:

- `tier: enforce` renders the mutating command (a `defaults write`, or the declared `system_setup`
  command).
- `tier: verify` renders no mutating command at all. It contributes a record to the posture check that
  slice 6 consumes. The runner physically cannot write a `verify` control, because the branch that emits
  a write is not taken.
- `tier: manual` renders no command and no check. It contributes a runbook pointer line.

The refusal is the load-bearing part. A record with a missing `tier`, an unrecognized `tier`, or a
`verify`/`manual` tier that also carries a mutating payload aborts the template render with the offending
domain and key named. It does not warn and continue, and it does not skip the record. `chezmoi apply`
stops before running anything.

Aborting the render (rather than emitting a rendered guard that exits nonzero) is deliberate: a rendered
guard fires after chezmoi has already begun applying, whereas a render abort refuses the whole operation.
chezmoi's `fail` template function does this, verified against chezmoi v2.71.0. Slice 3 must confirm the
version that introduced `fail` and, if it is newer than the current `.chezmoiversion` floor of `2.62.3`,
raise the floor in the same slice.

The absent-key handling uses `index . "tier"`, never `.tier`. Go's `text/template` throws
`map has no entry for key "tier"` on the field form when a record omits the key, which turns the intended
loud-and-specific error into an opaque template panic. This is the same gotcha the `host` and `sudo`
guards already document.

### What this prevents

The concrete failure: someone writes `spctl --master-enable` into `macos_system_setup.yaml` to turn
Gatekeeper on. On macOS 26 that flag does not exist. `spctl` exits nonzero, the runner's loop moves on,
the apply reports success, and the operator believes Gatekeeper is being enforced by the dotfiles. Under
the tier model Gatekeeper is declared `tier: verify` because it cannot be set from the command line; the
runner will not emit a write for it, so nobody can add that command without also lying about the tier,
and lying about the tier is caught by the `verify`-with-a-payload render abort.

## Known blocker: duplicate hermes profile source directories

`main` at `d331171` carries both `dot_hermes/profiles/{butters,concerned,elaine,nicodemus}` and a
`private_` variant of each. chezmoi treats the pair as two source entries for one target and rejects the
entire source state.

Verified on the base commit:

- Blocked: `chezmoi cat`, `chezmoi managed`, `chezmoi dump`, `chezmoi diff`, and `chezmoi apply`, even
  with `--dry-run`. All four profiles report `inconsistent state` and the command exits without doing
  anything.
- Not blocked: `chezmoi execute-template --source <tree>`. It does not read the full source state, so it
  renders a template against the real `.chezmoidata` normally.

Per-slice assessment:

| Slice | Wants a real apply? | Blocked? | Why                                                             |
| ----- | ------------------- | -------- | --------------------------------------------------------------- |
| 1     | No                  | No       | Scripts are executed directly from the source tree by the tests. |
| 2     | No                  | No       | Same, plus template renders via `execute-template`.              |
| 3     | No                  | No       | The render-abort behavior is proved by `execute-template`.       |
| 4     | Yes, for live proof | Partly   | Render tests pass; live firewall state cannot be applied.        |
| 5     | Yes, for live proof | Partly   | Render tests pass; the write-lands question needs a real apply.  |
| 6     | Yes, for live proof | Partly   | Poller runs standalone; end-to-end apply wiring cannot be shown. |
| 7     | No                  | No       | Sandbox drop-in tree plus the real `sshd -G` parser.             |
| 8     | No                  | No       | Fully stubbed daemon; the live drill is manual regardless.       |
| 9     | Yes, for live proof | Partly   | Render tests pass; cask install and running-state need an apply. |
| 10    | Yes, for live proof | Partly   | Same, and its drill needs the extension actually enforcing.      |

So the blocker never stops a slice from building or from passing its tests. It stops slices 4, 5, 6, 9,
and 10 from demonstrating the applied end state on the live machine. Two consequences:

1. Slices 4, 5, 6, 9, and 10 must each state, in their pull request description, that the applied end
   state was not demonstrated and which specific assertion is therefore deferred.
1. Resolving the duplicate directories is a prerequisite for the S10 acceptance drill, not for any
   individual slice. It is tracked separately and must be resolved before the drill. Removing the
   duplicates is a source-tree decision with its own blast radius (which of each pair is authoritative,
   and whether the profiles should be private), so it does not get folded into an S10 slice.

## The slice stack

Ten self-supporting pull requests in dependency order.

| #  | Pull request                      | Monolith source   | Blast radius               |
| -- | --------------------------------- | ----------------- | -------------------------- |
| 1  | macOS defaults shared library     | yes (3 commits)   | developer tooling          |
| 2  | System-domain support             | none (net new)    | developer tooling          |
| 3  | Tiered control model              | none (net new)    | apply-time refusal         |
| 4  | Firewall baseline                 | partial           | network reachability       |
| 5  | Security defaults baseline        | none (net new)    | update and download policy |
| 6  | Posture verification and alerting | partial           | alerting fidelity          |
| 7  | SSH config generation             | yes (most of #53) | sshd config on disk        |
| 8  | SSH apply safety                  | yes               | remote access              |
| 9  | OverSight                         | one cask line     | notification noise         |
| 10 | LuLu                              | one cask line     | outbound network, alerting |

______________________________________________________________________

### Slice 1: macOS defaults shared library

**Scope.** Extract the duplicated logic in `macos-defaults-{apply,capture,drift}.sh` into a new sourced
library, `dot_local/bin/macos-defaults-lib.sh`, and have all three consumers adopt it. The library owns
source-directory resolution, the data-file path, the readable-data-file guard, and the record-to-tab-
separated-values reader.

Source-directory resolution is a behavior change, not a pure extraction, and it is in scope. The three
tools currently hardcode `${HOME}/workspaces/Ivy/webdavis/dotfiles`, so `just defaults-capture` run from
a secondary worktree reads and writes the primary checkout instead of the worktree the operator is
standing in. The library resolves, in order: an explicit `MACOS_DEFAULTS_SOURCE_DIR` override; the
current git worktree top level when it carries `.chezmoidata/macos_defaults.yaml`, routed through
`chezmoi --source=<top> source-path` so chezmoi normalizes and validates it; otherwise chezmoi's
configured source directory.

**Deliberately not in scope.** No new tracked settings. No tier field. No system-domain support. No
change to what any tool does with a record once it has read it.

**Self-supporting proof.** The library is sourced by path relative to `${BASH_SOURCE[0]}`, which resolves
identically in the chezmoi source tree (`dot_local/bin/`) and in the applied `~/.local/bin/` layout,
because the library carries no `executable_` or `dot_` rename and so keeps the same basename in both.
Nothing outside these four files changes. The two tests drive the scripts directly out of the source tree
against a sandbox `HOME` and a stubbed `defaults` binary, so they need no applied state, no chezmoi source
state, and no live machine settings.

**Dependencies.** None. This is the base of the stack.

**Acceptance criteria.**

- A capture run from a secondary worktree writes that worktree's YAML and leaves both the hardcoded
  `~/workspaces/...` path and the chezmoi-configured source directory untouched.
- A drift run from a worktree whose YAML is unreadable exits 2 and names the worktree file in its error,
  proving it resolved the worktree and not a primary.
- A `chezmoi source-path` failure propagates a nonzero return with a message naming `source-path`. It is
  not masked by an unconditional `return 0`. The test asserts this from a caller without `set -e`, which
  is the only context where the mask is observable.
- `drift.sh` keeps `shopt -s lastpipe`. Removing it would silently make `just D` always exit 0.
- All three tools still behave identically on the seven existing records.

**Review weight.** Low security, standard code quality. The reviewer checks that the extraction is
genuinely shared behavior rather than three functions that happen to have similar bodies, and that the
resolution order is documented at its definition.

**Files.** Adds `dot_local/bin/macos-defaults-lib.sh`,
`test/unit/macos-defaults-source-path-propagation.sh`, `test/integration/macos-defaults-source-path.sh`.
Changes `dot_local/bin/executable_macos-defaults-{apply,capture,drift}.sh`.

______________________________________________________________________

### Slice 2: system-domain support in the defaults tooling

**Scope.** Teach the data schema and all four consumers (the three tools plus the Tier 1 runner) that a
record has a scope. A record gains an optional `scope` field with values `user` (the default, absent
field, preserving every existing record unchanged) and `system` (a `/Library/Preferences` domain).

- `apply` and the Tier 1 runner write a system-scope record via
  `sudo defaults write /Library/Preferences/<domain> <key> -<type> <value>`.
- `drift` reads a system-scope record from `/Library/Preferences/<domain>` and distinguishes three
  outcomes, not two: the value, genuinely unset, and unreadable. An unreadable read must never collapse
  into `<unset>` and be reported as drift, and must never be silently skipped. It is reported as its own
  indeterminate row.
- `capture` accepts `--scope system` and writes the field.
- The Tier 1 runner emits a single `sudo -v` prelude only when at least one system-scope record exists,
  so the common all-user-scope case still prompts for nothing.

**Deliberately not in scope.** No security setting is declared here. This slice adds capability and
declares nothing. No tier field yet.

**Self-supporting proof.** `scope` is optional with a `user` default, so the seven existing records and
every existing test are unchanged in meaning. The runner's `sudo -v` prelude is conditional on a record
that does not yet exist, so on merge it renders exactly the same script it does today, which the render
test asserts byte for byte. The new behavior is proved by fixture data files under
`chezmoi execute-template`, which the known blocker does not affect, plus direct script runs against a
stubbed `defaults`.

**Dependencies.** Slice 1. The scope-aware read and write live in the shared library, so they must have a
library to live in. Adding them to three copies and then extracting would double the review surface.

**Acceptance criteria.**

- With no system-scope record present, the rendered Tier 1 runner is unchanged from `main` and contains
  no `sudo` invocation.
- With a system-scope record present, the rendered runner contains exactly one `sudo -v`, before any
  write, and the record's write is prefixed with `sudo` and targets `/Library/Preferences/<domain>`.
- `drift` on a system-scope record that is unreadable reports it as indeterminate, with a distinct marker
  from `<unset>`, and does not count it as drift.
- `drift` on a system-scope record that is set and matching reports no drift.
- `capture --scope system` appends a record carrying `scope: system` and rejects the combination of
  `--scope system` with `--host current`, which is not a meaningful pair.
- The `scope` guard uses `index . "scope"`, not `.scope`.

**Review weight.** Low security, standard code quality. The reviewer checks the three-outcome read
carefully: two-sided bounds and fail-safe-not-fail-quiet both land here.

**Files.** Changes `dot_local/bin/macos-defaults-lib.sh`,
`dot_local/bin/executable_macos-defaults-{apply,capture,drift}.sh`,
`.chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl`, `.chezmoidata/macos_defaults.yaml`
(schema comment only). Adds `test/integration/macos-defaults-system-scope.sh`,
`test/unit/macos-defaults-scope-read.sh`.

______________________________________________________________________

### Slice 3: the tiered control model

**Scope.** Add the required `tier` field to both data schemas and make both runners enforce the
distinction, exactly as specified in the three-tier section above. Backfill `tier: enforce` onto the
seven existing Aerospace records and onto the one existing `system_setup` record, so the field is
required with no exceptions from the moment it exists.

Also in scope: fix the Tier 2 runner's `{{ if .sudo }}` guard to `{{ if index . "sudo" }}`. Go's
`text/template` throws on the field form when a record omits `sudo`, and slice 3 is the slice that
introduces new optional per-record fields, so the same class of bug must be closed here rather than
discovered when a later slice adds a record without the key.

**Deliberately not in scope.** No control is declared `verify` or `manual` yet, because no such control
exists in the data yet. The posture check that consumes `verify` records is slice 6. This slice ships the
mechanism and the refusal, and proves them with fixture data.

**Self-supporting proof.** Every existing record is backfilled to `tier: enforce` in the same commit, so
the rendered output of both runners is byte-identical to `main` after the change, which a render test
asserts. The refusal behaviors are proved against fixture `.chezmoidata` trees under
`chezmoi execute-template`, so no unshipped control is referenced and no live state is touched.

**Dependencies.** Slice 2, because the tier branch and the scope branch are both per-record template
logic in the same two runners, and interleaving them across slices would leave one runner half converted.

**Acceptance criteria.**

- A record with no `tier` aborts the render with an error naming the domain and key. The render does not
  merely warn, and does not skip the record.
- A record with an unrecognized `tier` value aborts the render, naming the value.
- A `tier: verify` record renders no mutating command. A test asserts the specific `defaults write` or
  `system_setup` command string is absent from the render, not merely that the render is short.
- A `tier: verify` or `tier: manual` record that carries a mutating payload aborts the render.
- A `tier: manual` record with no `runbook` field aborts the render.
- A `tier: manual` record renders a runbook pointer and no command.
- After backfilling, the rendered Tier 1 and Tier 2 runners are byte-identical to their `main` renders.
- The Tier 2 sudo guard uses `index . "sudo"`; a fixture record with no `sudo` key renders without a
  `sudo` prefix, and a `sudo: true` record keeps its prefix.
- If the render-abort function requires a chezmoi newer than `2.62.3`, `.chezmoiversion` is raised in
  this slice.

**Review weight.** Low security blast radius (the worst outcome is a refused apply), high design weight.
The reviewer checks that the refusal is total: there is no code path where an unrecognized tier results
in anything other than an aborted render, and no path where a `verify` control can emit a write.

**Files.** Changes `.chezmoidata/macos_defaults.yaml`, `.chezmoidata/macos_system_setup.yaml`,
`.chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl`,
`.chezmoiscripts/run_onchange_after_41-macos-system-setup.sh.tmpl`,
`dot_local/bin/macos-defaults-lib.sh` (tier-aware record reader). Adds
`test/integration/macos-control-tier-refusal.sh`, `test/integration/macos-system-setup-sudo-guard.sh`,
`test/integration/macos-control-tier-render-stability.sh`.

______________________________________________________________________

### Slice 4: firewall baseline

**Scope.** Declare the application firewall in `.chezmoidata/macos_system_setup.yaml` as three
`tier: enforce` records, in this order:

1. `socketfilterfw --setglobalstate on`
1. `socketfilterfw --setstealthmode on`
1. an explicit signed-application policy record, so the policy is declared rather than left at whatever
   the machine happens to carry

Order is load-bearing. `--setstealthmode on` writes a preference that is inert while the firewall's
global state is off, so on a fresh or drifted machine a lone stealth record produces a setting with no
protection behind it. The global-state record must render strictly before the stealth record.

Firewall logging is declared `tier: manual` with a runbook pointer, because `socketfilterfw` on 26.2 has
no logging-mode flag. This is the first real use of the `manual` tier and it is the concrete demonstration
that the tier model earns its keep.

**Deliberately not in scope.** No per-application allow or block rules. No LuLu or OverSight work; those
are slices 10 and 9. No firewall log ingestion.

**Self-supporting proof.** The records are ordinary `system_setup` entries in the schema slice 3 already
shipped, consumed by a runner that already knows how to render them. The test renders the real runner
against the real `.chezmoidata` via `execute-template` and asserts the emitted commands and their
relative order, which needs no live machine and is unaffected by the known blocker.

**Dependencies.** Slice 3, for the tier field the records carry and for the `manual` tier the logging
record uses. Slice 2 is a transitive dependency only.

**Acceptance criteria.**

- The rendered Tier 2 runner emits the global-state command and the stealth command, each with a `sudo`
  prefix, with global state strictly before stealth. The test compares line numbers, not presence.
- The rendered runner emits the signed-application policy command with a `sudo` prefix.
- The logging record renders no command and does render its runbook pointer.
- Every firewall command declared is idempotent (a set-to-on, not a toggle), so the runner re-running the
  whole list on a data change is safe.
- The runbook section named by the logging record exists and describes how to enable firewall logging by
  hand on 26.2.
- The pull request description states that the applied firewall state was not demonstrated on the live
  machine, and why (the known blocker).

**Review weight.** Medium security. Blast radius is network reachability: an incorrectly ordered or
malformed record leaves the machine less protected than the operator believes, but cannot lock anyone
out. The reviewer hunts for the inert-setting class specifically: a preference written under an inactive
subsystem.

**Files.** Changes `.chezmoidata/macos_system_setup.yaml`,
`docs/runbooks/macos-fresh-machine-quickstart.md`. Adds
`test/integration/macos-firewall-globalstate-order.sh`.

______________________________________________________________________

### Slice 5: security defaults baseline

**Scope.** Declare the two settable security defaults the product requirements document identified:

- `SoftwareUpdate AutomaticCheckEnabled`, system scope (`/Library/Preferences/com.apple.SoftwareUpdate`),
  `tier: enforce`. It is currently unset while all four of its dependents are on, which is a latent
  inconsistency: turning the parent off would silently disable the children.
- Safari `AutoOpenSafeDownloads`, `tier` determined by the investigation gate below.

**The Safari investigation gate.** Modern Safari preferences live inside an application container that a
plain `defaults write` cannot reach without Full Disk Access. A write that appears to succeed while
landing nowhere is exactly the silent-no-op failure the tier model exists to prevent. So this slice must
first prove, on the live machine, whether a write to `AutoOpenSafeDownloads` is readable back afterward:

- If the write lands and reads back, declare it `tier: enforce`.
- If the write does not land, declare it `tier: verify` if the value is readable, and `tier: manual`
  otherwise, with a runbook entry either way.

The decision and its evidence go in the pull request description. Declaring it `enforce` without the
read-back evidence is not acceptable.

**Deliberately not in scope.** The remaining posture controls (screen lock, auto-login, guest, FileVault,
Gatekeeper, SIP). Those are read-only checks and belong to slice 6.

**Self-supporting proof.** Both records use the `scope` field from slice 2 and the `tier` field from
slice 3, both already shipped. Nothing else changes. The render test asserts the emitted commands against
the real data file via `execute-template`.

**Dependencies.** Slice 2 (system scope, for the `SoftwareUpdate` domain), slice 3 (the tier field).
Slice 2 is a hard prerequisite here, not a convenience: `SoftwareUpdate` cannot be expressed at all in
the user-domain-only tooling.

**Acceptance criteria.**

- The rendered Tier 1 runner writes `SoftwareUpdate AutomaticCheckEnabled` to `/Library/Preferences`
  under `sudo`, with the correct type.
- `just D` reports no drift once the value is applied, and reports drift when the value is changed out
  from under it. The drift assertion is mutation-verified.
- The Safari record's tier matches the read-back evidence recorded in the description.
- Neither record disturbs the seven Aerospace records.
- The pull request description states which assertions were deferred because of the known blocker.

**Review weight.** Medium-low security. Blast radius is update and download policy. The reviewer's
specific hunt is the silent-no-op class: does each declared write actually land, and is there evidence
rather than an assumption.

**Files.** Changes `.chezmoidata/macos_defaults.yaml`,
`docs/runbooks/macos-fresh-machine-quickstart.md` (if a record lands `manual`). Adds
`test/integration/macos-security-defaults-render.sh`.

______________________________________________________________________

### Slice 6: posture verification and alerting

**Scope.** Make `tier: verify` controls a real runtime check whose failures reach the operator durably,
rather than a line of stderr that scrolls past during an apply.

The existing poller `dot_local/libexec/osquery/executable_firewall-gatekeeper-monitor.sh` already reads
firewall, Gatekeeper, and screen-lock state every 60 seconds via `osqueryi` in the graphical user session,
compares against a persisted baseline, and pages CRIT through the S9 dispatcher when a protection turns
off. That is already the right mechanism, owned by the same operator, with the write-ahead undelivered
store and page-once markers already solved. Slice 6 extends it rather than building a second one.

Extensions:

- Introduce `.chezmoidata/macos_posture_controls.yaml`, the declaration home for `verify`-tier controls
  that are neither a `defaults` key nor a `system_setup` command. A posture control that reads "process X
  is running" or "rule Y exists" has no natural place in either existing data file, and hardcoding the
  set inside the poller would put the controls somewhere the tier model cannot see them. Each record
  carries its identifier, its tier, how it is read, and the value it must hold. The poller reads the file
  rather than carrying the list in its body, so adding a control is a data change. Slices 9 and 10 each
  add records here.
- Add auto-login and guest-account state to the posture read.
- Add FileVault and System Integrity Protection state to the posture read, both as report-only controls
  (`verify` tier: neither can be enabled from the command line, and SIP is deliberately disabled on this
  machine, so its declared expected value is whatever the operator declares, not a blanket "enabled").
- Extend the existing monitoring-gap gate to the new fields, so a missing or out-of-domain value for any
  of them is a gap page, not a silent pass.
- The reader for each new field is an open item resolved inside the slice: osquery's own tables where a
  table genuinely returns the value in this session context, and a direct `defaults read` under the
  poller otherwise. It must be proved, not assumed, and the evidence goes in the description.

Deliberate divergence from pull request #53: the monolith's
`.chezmoiscripts/run_after_67-macos-security-posture.sh.tmpl` is not re-landed as a separate apply-time
script. Two mechanisms reading the same set of controls is the several-lists-describing-one-set bug
class, and the apply-time one is the weaker of the two (stderr only, no durability, only fires when an
apply happens). Its genuinely valuable content is absorbed into the poller: the indeterminate-on-nonzero
discipline (a query that exits nonzero is indeterminate regardless of what it printed, because a failed
probe's output is untrustworthy), and remedy text that names System Settings rather than the removed
`spctl --master-enable` flag.

**Deliberately not in scope.** Any attempt to fix a posture. Every control here is report-only by
definition of its tier. No new launchd agent, no new plist, no change to the dispatcher.

**Self-supporting proof.** The poller, the dispatcher, its plist, and its loader all exist on `main`
already. Slice 6 changes one script and its tests. The `verify` tier it consumes shipped in slice 3. The
poller runs standalone from the source tree under the existing bats harness with a stubbed `osqueryi` and
a stubbed dispatcher, so its tests need neither a live machine nor a chezmoi apply.

**Dependencies.** Slice 3 (the `verify` tier that declares which controls are check-only). Slices 4 and 5
are not dependencies: the poller reads live state, not the declared enforce records.

**Acceptance criteria.**

- Each newly monitored control, turned off in a stubbed read, produces exactly one CRIT page naming that
  control, and stays quiet on subsequent ticks while it remains off.
- Each newly monitored control, restored, clears its marker so a later regression pages again.
- A read that exits nonzero is reported indeterminate and never classified as enabled, even when its
  output contains enabled-looking text. This is asserted per control, with a stub that prints
  enabled-looking text and exits nonzero.
- A missing or out-of-domain value for any monitored field trips the monitoring-gap page.
- Nothing in the poller ever invokes a mutating command. The test drives it with fakes that log and fail
  on any non-status invocation, and asserts the log is empty.
- The Gatekeeper remedy text names System Settings and does not name `spctl --master-enable`.
- No value read from the system reaches a notification body or a shell command without neutralization.
- A record in `macos_posture_controls.yaml` with a tier other than `verify` is rejected, since an
  enforce-tier control does not belong in a file the poller only reads from.
- The poller's monitored set and the file's record set are enumerated and diffed by a test, so a control
  declared in data but never read, or read but never declared, fails rather than passing quietly.
- The pull request description records which reader was chosen for each new field and the evidence for
  it, and states which assertions were deferred because of the known blocker.

**Review weight.** Medium security, high correctness. Blast radius is alerting fidelity: a bug here means
a real posture regression goes unpaged, which is a silent failure with a long tail. The reviewer's hunts
are notify-before-persist ordering on every new marker, the indeterminate classification, and untrusted
data crossing into the notification body.

**Files.** Changes `dot_local/libexec/osquery/executable_firewall-gatekeeper-monitor.sh`. Adds
`.chezmoidata/macos_posture_controls.yaml`, `test/unit/macos-posture-indeterminate.sh`,
`test/integration/osquery-poller-posture-fields.bats` (or
extends `test/integration/osquery-poller.bats`, whichever the existing harness makes cleaner). Does not
add `.chezmoiscripts/run_after_67-macos-security-posture.sh.tmpl`.

______________________________________________________________________

### Slice 7: SSH configuration generation

**Scope.** Rewrite `dot_local/bin/executable_ssh-hardening.sh` so it generates, installs, and verifies a
public-key-only sshd drop-in. Everything in this slice is read-only with respect to the running daemon.

- Drop-in content: `PasswordAuthentication no`, `KbdInteractiveAuthentication no`, `UsePAM yes`,
  `PubkeyAuthentication yes`, `PermitRootLogin no`. The first two together close both interactive password
  channels. `UsePAM yes` is required on macOS for account and session management, and is safe precisely
  because neither password path is open, so the pluggable authentication modules layer has no password
  path to authenticate.
- Drop-in name `000-ssh-hardening.conf`. sshd's `Include /etc/ssh/sshd_config.d/*` is lexical and
  first-value-wins, so the previous name `50-no-password-auth.conf` sorted after Apple's
  `100-macos.conf` and was shadowed by it. Install migrates the old name away.
- Explicit mode `0644`, set by an explicit `chmod` rather than left to the ambient umask. The file holds
  no credential and sshd must be able to read it. Pinning it makes the mode deterministic and makes the
  comment honest.
- Modes: `--print-config` and `--print-path` (pure, no privilege, no writes, the test seam), `--verify`
  (read-only, three-way), and the default install.
- `--verify` asserts three independent ways, all read-only and host-key-free: the global pre-Match
  effective configuration via `sshd -G`; a raw scan of every included file for a `Match` block that
  re-enables a protected directive; and a per-connection resolution via `sshd -G -T -C` for a root
  specification and an ordinary-user specification. `sshd -G` alone dumps only the pre-Match
  configuration, so a `Match Address 0.0.0.0/0` block re-enabling password authentication passes it while
  the machine is wide open. That is the most idiomatic sshd bypass and it must be caught.
- `--verify` fails closed when the verifier cannot run. A skip is permitted only through an explicit test
  seam, never in the default path. `SSHD_BIN` defaults to the absolute `/usr/sbin/sshd`, so a stripped
  `PATH` cannot turn a security check into a no-op.
- Install runs `--verify` after writing and refuses to claim success if the effective configuration is
  not fully hardened.

The drop-in write is declared `tier: enforce` in `macos_system_setup.yaml`. The reload is declared
`tier: manual` with a runbook pointer, and lands in slice 8.

**Deliberately not in scope.** `--reload`. Any `launchctl` call. Any readiness probe. Any rollback mode.
Those are slice 8.

**Self-supporting proof.** Writing a drop-in changes nothing for a running sshd, because sshd does not
re-read its configuration until it restarts. So slice 7 can merge and even apply without altering how any
existing connection or any new connection is authenticated. That is the exact reason the SSH work splits
here and not somewhere else: this is the last point at which the change is inert. On a fresh machine
where Remote Login has never been enabled, the drop-in applies the moment sshd first starts, which is
the desired behavior and still involves no reload.

All tests operate on a sandbox `sshd_config.d` tree through the `SSHD_CONFIG_D`, `SSHD_MAIN_CONFIG`, and
`SSH_HARDENING_SUDO` seams, and never touch `/etc/ssh`. A deliberately failing `sudo` stub on `PATH`
guarantees that even a regressed script cannot reach the live tree: its bare `sudo tee /etc/ssh/...`
fails instead.

**Dependencies.** Slice 3, for the `tier` field on the `system_setup` record that wires the script in.

**Acceptance criteria.**

- `--print-config` emits all five accepted lines and no conflicting directive, with no privilege
  escalation and no write.
- `--print-path` names `000-ssh-hardening.conf`, and a `LC_ALL=C` sort places it before
  `100-macos.conf`.
- Against a sandbox tree containing a hostile `100-macos.conf` that reopens every hole, install writes
  the `000-` drop-in, removes a seeded `50-no-password-auth.conf`, and reports the effective configuration
  verified. An independent `sshd -G` on the same tree confirms all five values.
- A regression guard proves the old `50-` name is defeated by the hostile `100-` file, documenting why
  the rename is required rather than asserting it.
- `--verify` fails, loudly and nonzero, on each of: a `Match Address` re-enable, a `Match User *`
  re-enable, a `Match all` re-enable, a `Match User <specific>` re-enable that the connection-spec
  sampling alone would miss, and a global re-enable in a sibling that sorts first. The `Match`-scan
  failures name the offending file.
- `--verify` passes on a clean tree and passes again after the hostile files are removed, so no state
  leaks between cases.
- With `SSHD_BIN` pointed at a nonexistent path and no test seam set, `--verify` exits nonzero and says
  it is failing closed. With the seam set, it skips cleanly and does not print a verified claim.
- Install pins the drop-in to `0644` even under `umask 0077`.
- Every test invokes the script through `/bin/bash`, so a bash 3.2 regression fails in the test.

**Review weight.** High security. Blast radius is the sshd configuration on disk. It cannot lock the
operator out today, but it determines what happens at the next daemon restart, which may be unattended (a
reboot). The reviewer hunts: any path where `--verify` returns success without having parsed anything;
any `Match` form the scan misses; any place the sandbox seams could be bypassed and reach `/etc/ssh`; and
the honesty of every comment about modes and guarantees.

**Files.** Changes `dot_local/bin/executable_ssh-hardening.sh`, `.chezmoidata/macos_system_setup.yaml`,
`docs/runbooks/macos-fresh-machine-quickstart.md`. Adds `test/unit/ssh-hardening-dropin.sh`,
`test/unit/ssh-hardening-verify-failclosed.sh`, `test/integration/ssh-hardening-sshd-validate.sh`,
`test/integration/ssh-hardening-include-precedence.sh`,
`test/integration/ssh-hardening-match-reenable.sh`, `test/integration/ssh-hardening-dropin-mode.sh`.

______________________________________________________________________

### Slice 8: SSH apply safety

**Scope.** Add the disruptive mode: `--reload`, which restarts sshd so a running daemon picks up the
drop-in, plus the rollback mode that is the way back in.

`--reload` must fail closed at every step:

1. Prime privilege escalation visibly and abort if it is unavailable. A `sudo` failure must never be
   mistaken for "sshd is not running".
1. Validate the complete live configuration before the disruptive step: `sshd -t` for syntax, then the
   full three-way `--verify` from slice 7. Never restart onto a configuration that fails to parse or has
   lost the hardening.
1. Probe the service and distinguish confirmed-absent from an errored probe. `launchctl print` exits 0
   when the service is loaded and 113 when it is genuinely absent. Any other nonzero is a probe error and
   is not proof the daemon is down; propagate it rather than proceeding as if stopped.
1. Kickstart, then confirm the launchd job is loaded again. That is a first signal only, because
   `launchctl print` returns 0 for a loaded-but-crashed service.
1. Prove readiness: the listener must complete an SSH banner exchange. A loaded job that never answers
   means the new configuration crashed sshd, which is a possible lockout and must fail loud rather than
   report green.
1. Return nonzero on any failure, with a message that names the recovery path, not merely "investigate".

`--rollback` removes the managed drop-in and re-verifies that the hardening is no longer in the effective
configuration. It is the way back in expressed as code rather than as prose, and it has its own tests, so
it cannot rot the way a comment does.

**Deliberately not in scope.** Any automatic rollback on failure. `--reload` failing must leave the
operator in control, not trigger a second unattended state change on a machine that just proved it cannot
be trusted to restart cleanly. Also out of scope: changing whether Remote Login is enabled.

**Self-supporting proof.** Slice 7 shipped the drop-in, the verifier, and every seam this slice drives.
Slice 8 adds modes to the same script and touches nothing else. Its tests drive fully controlled `sudo`,
`sshd`, `launchctl`, and `ssh-keyscan` stubs through the `SSH_HARDENING_SUDO`, `SSHD_BIN`,
`LAUNCHCTL_BIN`, and `KEYSCAN_BIN` seams, mirrored on `PATH` so even a bare-name call hits a stub. No
test requires a real tool, so none of them can skip, and none of them can touch the live daemon.

**Dependencies.** Slice 7, for the drop-in, the verifier, and the seams. Slice 4 is not a build
dependency but is an operational one: see the loopback finding in risks.

**Acceptance criteria.** Each case asserts the exit status, whether a kickstart was attempted, and the
message content:

- `sudo` failure: nonzero, no kickstart, stderr names sudo, and stderr does not say "not loaded" or "not
  running".
- `sshd -t` failure: nonzero, no kickstart, stderr names syntax.
- Hardening lost in the effective configuration: nonzero, no kickstart, stderr says not fully hardened.
- Service confirmed absent (probe rc 113): exit 0, no kickstart, stdout explains the drop-in applies when
  Remote Login is next enabled.
- Probe error (any nonzero other than 113): nonzero, no kickstart, stderr says it could not determine the
  state and does not say "not loaded".
- Kickstart itself fails: nonzero, kickstart was attempted, stderr names kickstart.
- Job does not reload after kickstart: nonzero, stderr says it did not reload.
- Job loaded but the readiness probe never sees an SSH banner: nonzero, stderr warns about a possible
  lockout and names the recovery path.
- Readiness prover unavailable: nonzero. `--reload` refuses to run when it cannot prove the daemon came
  back, rather than kickstarting blind.
- Happy path: exit 0, kickstart attempted, stdout confirms sshd is accepting connections on the resolved
  port.
- `--rollback` removes the drop-in, and a following `--verify` reports the hardening absent. Running it
  twice is a clean no-op the second time.
- `--reload` never writes the drop-in, and the default install mode never reloads. The separation is
  asserted, not assumed.
- The recovery instruction text is asserted by a test, not merely present in a comment.

**Review weight.** Highest in this program. Slice 8 is the first change in S9 or S10 that can lock the
operator out of the machine. It gets the full adversarial pass plus a personal operator review before
merge, and it does not merge until the live drill below has been run.

**Files.** Changes `dot_local/bin/executable_ssh-hardening.sh`,
`docs/runbooks/macos-fresh-machine-quickstart.md`. Adds
`test/integration/ssh-hardening-reload-failclosed.sh`, `test/integration/ssh-hardening-rollback.sh`.

______________________________________________________________________

### Slice 9: OverSight

**Scope.** Add the `oversight` cask to `.chezmoidata/system_packages_autoinstall.yaml` in alphabetical
order, and declare one `verify`-tier posture control in `macos_posture_controls.yaml` asserting that
OverSight's monitoring process is actually running.

The split of tiers is the point. Installing the cask is `enforce`, because Homebrew can genuinely do it.
Whether the tool is running is `verify`, because a login item that the operator can quit is not something
an apply-time runner should silently restart, and the microphone and camera permission grants it needs
are interactive. Those grants get a `manual` record with a runbook pointer.

OverSight is passive. It observes microphone and camera activation and raises a notification; it filters
no traffic and carries no network extension. So it has no interactions with any other slice to manage,
and it cannot degrade the alerting channel the way slice 10 can. That is why it ships before slice 10 and
gets a fraction of the review.

**Deliberately not in scope.** Any OverSight configuration beyond installation. Any attempt to grant its
permissions programmatically. Any rule or allowlist.

**Self-supporting proof.** The cask line is consumed by the Brewfile generator in
`run_onchange_before_10-system-packages.sh.tmpl`, which has been on `main` since long before S10 and
needs no change to accept another cask. The posture record uses the tier field from slice 3 and the
declaration file and reader loop from slice 6, both already shipped by the time slice 9 builds. Nothing
in the slice references anything unshipped, and its tests render the real generator and drive the real
poller against a stubbed process probe, so they pass with OverSight absent from the machine.

**Dependencies.** Slice 3, for the tier field the records carry. Slice 6, for
`macos_posture_controls.yaml` and the poller loop that reads it. Both are already merged at this point in
the stack, so neither constrains the ordering further.

**Acceptance criteria.**

- The generated Brewfile contains the `oversight` cask, and the casks list is still in alphabetical
  order. The ordering assertion compares the parsed list against its own sorted copy rather than eyeballing
  the diff.
- The running-state record is declared `verify` and is rejected by the tier guard if changed to
  `enforce`.
- With a stubbed probe reporting OverSight not running, the poller pages exactly once and stays quiet
  while it remains down.
- With the probe restored, the marker clears so a later stop pages again.
- A probe that exits nonzero is reported indeterminate, never as running. The slice must pin the actual
  process name or bundle identifier it probes for, with evidence from the installed application, rather
  than assuming one.
- The permission-grant record is `manual` and its runbook section exists.
- The pull request description states which assertions were deferred because of the known blocker.

**Review weight.** Low. Blast radius is notification noise: the worst realistic outcome is a banner the
operator did not want, or a posture page for a tool that is merely quit. The code-quality dimension
applies as it does everywhere, with the reviewer checking that the running-state probe is a real check
rather than a proxy such as the presence of the application bundle on disk.

**Files.** Changes `.chezmoidata/system_packages_autoinstall.yaml`,
`.chezmoidata/macos_posture_controls.yaml`, `docs/runbooks/macos-fresh-machine-quickstart.md`. Adds
`test/integration/oversight-cask-and-posture.sh`.

______________________________________________________________________

### Slice 10: LuLu

**Scope.** Add the `lulu` cask, declare the system-extension approval as a `manual` control, and declare
`verify` controls for the extension running and for the required allow rules existing. LuLu is an
outbound firewall: it prompts on outbound connections and blocks what is not allowed.

**Why it is last, in dependency terms.** Three reasons, none of them a matter of caution:

1. **It needs slice 3.** LuLu's system-extension approval is interactive, so it is a `manual`-tier
   control. Slice 3 is what creates a place to declare that honestly, instead of a script that pretends to
   apply something it cannot.
1. **It needs slice 6.** LuLu filters outbound traffic and the osquery alerting reaches Discord over
   outbound traffic, so slice 10's acceptance has to prove a page still arrives with LuLu enforcing. That
   proof is only meaningful if the alerting path was already proven working without LuLu, otherwise a
   failure cannot be attributed to a layer.
1. **It is the only slice that can degrade the alerting channel.** Running it last means every other
   slice was already proven through an unfiltered network, so any delivery failure observed during slice
   10 has exactly one new suspect.

**The blast radius is bounded by an S9 mitigation that already exists.** A blocked webhook is not silent.
`send_alert` is write-ahead durable: it persists the page before attempting delivery, so a blocked POST
queues rather than vanishing, the scheduled drainer retries it, and it eventually dead-letters. The
uptime watchdog pages on any nonzero dead-letter count, so a persistently blocked channel surfaces as its
own alert. Separately, the durable local notification channel fires a banner regardless, because that
path never touches the network at all. Both facts are properties of code already on `main`, and slice 10
should verify they still hold rather than re-establish them.

#### The gate slice 10 resolves before it designs anything

**Question.** Can LuLu allow rules be pre-seeded declaratively, or are they only creatable by answering
an interactive prompt?

**Resolved: they cannot be pre-seeded through any supported interface.** Verified against LuLu's own
source, without installing it:

- `LuLu/Shared/consts.h` defines `INSTALL_DIRECTORY @"/Library/Objective-See/LuLu"`,
  `RULES_FILE @"rules.plist"`, and `PREFS_FILE @"preferences.plist"`. So rules live at
  `/Library/Objective-See/LuLu/rules.plist`.
- `LuLu/Extension/Rules.m` writes that file with
  `[NSKeyedArchiver archivedDataWithRootObject:persistentRules requiringSecureCoding:YES ...]` followed by
  `writeToFile:atomically:YES`, and reads it back with `NSKeyedUnarchiver unarchivedObjectOfClasses:` over
  a class set that includes LuLu's own private `Rule` class.
- A keyed archive of a private Objective-C class is not a hand-authorable property list. `defaults write`,
  `PlistBuddy`, `plutil`, and `yq` cannot produce a valid one. The header also carries a
  `RULES_FILE_V1 @"rules_v1.plist"` migration constant, so the layout has already changed once and any
  reimplementation would be version-fragile.
- LuLu does expose an `import:userOnly:` method, but it consumes a file LuLu itself exported, and the
  documented interface is the graphical Rules menu. The product ships no command-line entry point.
- A third-party command-line tool writes the archive directly and then requires its own reload, which
  restarts the system extension and opens a brief window with no filtering. That is an unofficial writer
  against an undocumented format, and the standing rule against depending on modified or unofficial
  interfaces to third-party tools applies.

**So the design is the interactive-only branch:** a runbook entry that walks the operator through
creating each required rule by hand, plus `verify`-tier controls asserting those rules exist, paging when
one goes missing. The slice does not automate rule creation and does not pretend to.

**A second gate the slice must also resolve.** LuLu's preferences surface is distinct from its rules
surface. `consts.h` defines `PREF_ALLOW_LOCALHOST`, `PREF_ALLOW_APPLE`, `PREF_ALLOW_INSTALLED`,
`PREF_USE_ALLOW_LIST` with `PREF_ALLOW_LIST`, and `PREF_PASSIVE_MODE`, all stored in `preferences.plist`
in the same directory. If that file is a plain property list rather than another keyed archive, then some
policy is declaratively settable even though individual rules are not. This must be settled by reading
the file's actual format on a machine that has LuLu, not assumed, and it matters directly:
`allowLocalHost` is what determines whether the alerting path's loopback POST is filtered at all.

#### The allow-rule set, and a correction to how the alerting path egresses

The minimum enumerated set, with a correction the slice must not skip past. The osquery alerter does not
POST to Discord. `alert-dispatch.sh` POSTs to
`http://127.0.0.1:8644/webhooks/osquery-priority`, a Hermes gateway running locally, and Hermes performs
the internet egress. The uptime watchdog's route probe hits the same loopback URL. So:

| Talker                       | What it needs                                              |
| ---------------------------- | ---------------------------------------------------------- |
| Hermes gateway               | Outbound. This is the alerting channel's real egress hop.  |
| The alerter's own `curl`     | Loopback only, so a preference (`allowLocalHost`), not a rule. |
| `tailscaled`                 | Outbound. Also recovery path 2 in the slice 8 story.       |
| Homebrew, npm, nix, and `gh` | Outbound, unattended, on their own schedules.               |

Two consequences. First, anyone who writes an allow rule for the alerter's `curl` has solved the wrong
problem; the rule that protects paging is the one on Hermes. Second, `tailscaled` is safety-critical
rather than convenience: blocking it removes one of the two remote recovery channels slice 8 depends on.

**The granularity tension, and which way to lean.** LuLu keys a rule on the executing binary's path and
code-signing identity. Several talkers above are not distinct binaries; they are scripts that reach the
network through a shared client such as `/usr/bin/curl` or git's HTTPS transport. LuLu cannot see which
script invoked the shared client, so a rule allowing `/usr/bin/curl` allows it for every process on the
machine. On a box running many unattended agents that is close to allowing arbitrary outbound. The
opposite lean, a rule per binary, means more prompts.

**Lean narrow, and accept the prompt cost.** Concretely: do not create blanket allow rules for shared
interpreters and shared clients (`/usr/bin/curl`, `/bin/bash`, `node`, `python3`, `/usr/bin/ssh`). Those
are exactly the binaries an unexpected outbound connection would ride, so allowing them wholesale removes
the reason the tool was installed. Prefer rules on binaries that are themselves the thing being allowed:
`tailscaled`, the Hermes gateway, `nix`. The reasoning is an asymmetry, not a preference: the prompt cost
is bounded and one-time, because the unattended talker set is small, known, and enumerated above, so the
prompts land during the drill rather than becoming an ongoing tax; whereas a wholesale `curl` allow is
permanent, unbounded, and silently forfeits the layer's entire detection value.

Where a talker only reaches the network through a shared client, do not widen the shared client. Either
leave it prompting, or give that path its own dedicated client so it can carry its own narrow rule. If
the drill shows the alerting path itself would ride a shared client outbound, those two options are the
honest ones, and quietly widening `curl` to make a page go through is not.

**Deliberately not in scope.** Automating rule creation. Passive mode or block mode. Any LuLu block
rules. Changing how `alert-dispatch.sh` reaches Hermes.

**Self-supporting proof.** The cask line is consumed by the existing Brewfile generator. Every declared
control uses the tier field from slice 3 and the declaration file and reader loop from slice 6. The
runbook section is prose in a file that already exists. Nothing references anything unshipped. The tests
drive the poller against a stubbed rule-file reader and a stubbed extension probe, so they pass on a
machine where LuLu is not installed, which is the state of the machine when the slice is built.

**Dependencies.** Slice 3 and slice 6, for the three reasons given above. Slice 8 is not a build
dependency, but slice 10's rule set must include `tailscaled` precisely because slice 8's recovery story
leans on the tailnet.

**Acceptance criteria.**

- The generated Brewfile contains the `lulu` cask and the casks list is still alphabetically ordered,
  asserted against a sorted copy.
- The system-extension approval is declared `manual` with a runbook section that exists, and the runner
  emits no command for it.
- Every talker in the enumerated set above has either an allow rule declared as a `verify` control or an
  explicit written reason it needs none. The set of talkers and the set of declared controls are
  enumerated and diffed by a test, so adding a talker without a control fails.
- With a stubbed rule reader reporting a required rule missing, the poller pages once, names the missing
  rule, and stays quiet while it remains missing.
- The rule-existence check states its own limitation honestly. The archive is readable enough to
  enumerate the binary paths it references, so the check can prove a rule mentioning a binary exists; the
  slice must determine whether the rule's action is recoverable from the archive, and if it is not, the
  control is documented as existence-only and explicitly does not prove the rule allows rather than
  blocks. A check that cannot see the action must not claim it did.
- The preferences-surface gate is resolved in writing, with the evidence, before any control is declared.
- The pull request description states which assertions were deferred because of the known blocker.

**Live drill, run before slice 10 merges.** With the alerting path already proven working in slice 6, and
performed in this order:

1. Confirm a test CRIT page reaches Discord with LuLu installed but its extension not yet approved.
1. Approve the extension. Create the enumerated allow rules by hand, following the runbook, and confirm
   the runbook's steps actually match what the interface presents.
1. Send a second test CRIT page and confirm it reaches Discord with the extension enforcing.
1. Confirm the tailnet still works, since it is a slice 8 recovery path.
1. Delete one required rule deliberately and confirm the poller pages that it is missing, then restore
   it.
1. Confirm the S9 mitigation end to end: block the egress, confirm the page queues rather than vanishing,
   confirm the local banner still fires, and confirm the watchdog eventually pages on the dead-letter
   count.

The drill's results go in the slice 10 pull request description. Step 6 is the one that turns the
bounded-blast-radius claim from an assertion into evidence.

**Review weight.** High, and second only to slice 8 in this program. Slice 10 cannot lock the operator
out of the machine, which is why it ranks below slice 8, but it is the only slice that can degrade the
channel by which every other failure is reported, and a broken alerting channel is a silent failure that
hides the next one. The reviewer's specific hunts: any allow rule broader than the talker it exists for,
especially on a shared client; any claim about a rule's action the archive cannot actually support; and
any place the runbook and the declared control set could drift apart.

**Files.** Changes `.chezmoidata/system_packages_autoinstall.yaml`,
`.chezmoidata/macos_posture_controls.yaml`, `docs/runbooks/macos-fresh-machine-quickstart.md`. Adds
`test/integration/lulu-cask-and-rules.sh`, `test/unit/lulu-rule-existence-reader.sh`.

## The recovery story for slice 8

A reload restarts the daemon that serves remote access, on a machine whose sudo is passwordless and whose
Remote Login is on. The question that governs this slice is not "does the code work" but "what is the way
back in when it does not".

### The ways back in, in order of independence

1. **Physical console.** dresden is a machine the operator can sit at. This path depends on nothing the
   slice touches. From it: `sudo rm /etc/ssh/sshd_config.d/000-ssh-hardening.conf`, or
   `ssh-hardening.sh --rollback`, then re-enable Remote Login from System Settings. This is the
   guaranteed path and it is why slice 8 is survivable at all.
1. **Screen Sharing over the tailnet.** A remote channel that does not route through sshd. Tailscale is
   already running and already monitored. This is the real remote recovery path, and it must be confirmed
   working before the drill, not during the incident.
1. **A second SSH session held open across the reload.** The cheapest safety net and the one the operator
   controls directly. `--reload` must instruct it explicitly, and the readiness probe must run and pass
   before the operator closes the old session.
1. **`--rollback` from any of the above**, followed by a second `--reload` once the configuration is
   fixed.

The ordering matters because paths 2 through 4 can all fail together for one shared reason (a
network-level change, which slice 4 introduces), while path 1 cannot.

### How the recovery path is pinned by a test rather than asserted in a comment

Four mechanisms, in increasing strength:

1. The failure messages are asserted. A test greps `--reload`'s stderr for the recovery instruction on
   the not-ready path. If someone shortens the message to "investigate", the test fails.
1. Rollback is a supported mode with its own tests, not a documented sequence of manual commands. A
   documented sequence rots the moment the drop-in name changes. A tested mode does not: the same
   `--print-path` value that install writes is the value rollback removes, so a rename cannot desynchronize
   them.
1. `--reload` refuses to run when it cannot prove the daemon came back. A reload that cannot verify its
   own outcome is a reload that leaves the operator guessing, so the absence of the readiness prover is a
   refusal, not a warning.
1. The separation between "write the drop-in" and "restart the daemon" is asserted in both directions.
   Install never reloads and reload never writes, so an apply can never restart sshd as a side effect,
   and the disruptive step is only ever taken by an operator who typed `--reload`.

### What the tests can and cannot establish

The tests drive stubs. `SSHD_BIN`, `LAUNCHCTL_BIN`, and `KEYSCAN_BIN` are all replaced by scripts that
model a daemon. That is the right design (it makes every failure branch reachable and deterministic, and
it makes the suite safe to run on the operator's own machine), and it bounds what the suite proves.

The tests establish the script's control flow against a modeled daemon: that every failure branch exits
nonzero, that validation happens before the kickstart, that a probe error is not confused with a stopped
service, that a loaded job is not confused with a ready one, and that the messages say what they must
say.

The tests cannot establish anything about the real daemon. Specifically they cannot show that:

- the real `/usr/sbin/sshd` on macOS 26.2 accepts the drop-in as written;
- `launchctl kickstart -k system/com.openssh.sshd` preserves the operator's current session;
- the operator's own key actually authenticates under the new configuration, which is the failure that
  matters most and is entirely outside the stub's reach;
- a loopback readiness probe implies a remote client can connect. It does not, and slice 4 makes that gap
  concrete: the firewall and stealth mode do not filter loopback, so `--reload` can report green on a
  machine no remote client can reach.

Those are live-machine facts, so they are established by a live drill, not by the suite.

### The live drill, run before slice 8 merges

Physically at the machine, with Screen Sharing over the tailnet confirmed working first, and a second SSH
session already open:

1. Confirm key authentication works today, from a second host, before changing anything.
1. Run `--verify` and confirm all three views pass.
1. Run `--reload`. Confirm it reports sshd accepting connections.
1. From the second host, open a brand new SSH session using key authentication. Do not close the old
   session until the new one is established.
1. Confirm password authentication is refused from the second host.
1. Run `--rollback`, confirm `--verify` reports the hardening absent, then re-run install and `--reload`
   to return to the hardened state. This exercises the recovery path once, deliberately, while the
   operator is sitting in front of the machine, rather than for the first time during an incident.

The drill's results go in the slice 8 pull request description.

## Per-slice process

For each slice, in order:

1. Build the slice on a branch off the current `main`.
1. Run the slice's tests plus the repository check command.
1. Chain-of-thought verification before any review: the implementing agent explains its reasoning step by
   step, what changed, why, how the change preserves or intentionally alters behavior, and how it
   verified correctness. This goes in the slice report, never in the pull request description.
1. Adversarial review, weighted per the slice's review weight. The code-quality dimension runs on every
   slice regardless, ranked below correctness.
1. Slice 8 additionally gets a personal operator review and its live drill before merge. Slice 10
   additionally gets its live drill before merge.
1. Merge on approval; move to the next slice.

Where review turns up a bug or a worthwhile improvement, fix it in that slice and call it out in the pull
request description.

## Verification

- Each slice: its own tests and the repository check command pass. A slice that intentionally diverges
  from pull request #53 states the divergence and its reason.
- Slices 4, 5, and 6 each state which assertion was deferred because of the duplicate hermes profile
  directories.
- Slice 5 states the Safari read-back evidence and the tier it justified.
- Slice 6 states which reader was chosen for each new posture field and the evidence for it.
- Slice 8 states its live drill's results.
- Slice 9 states the process identifier its running-state probe uses and the evidence for it.
- Slice 10 states the resolution of both gates (rule pre-seeding, and the preferences surface) with its
  evidence, and its live drill's results including the blocked-egress step.
- End state, after the blocker is resolved: a `chezmoi apply` renders both runners without abort, writes
  only `enforce` controls, and the poller pages on a `verify` control turning off. Every `manual` control
  has a runbook section, and the set of `manual` records and the set of runbook sections are enumerated
  and diffed by a test rather than eyeballed.

## Risks and open items

### Work in pull request #53 the original eight-slice plan did not account for

This is the section to read before starting slice 1, not after reaching slice 7. Item 2 has since been
resolved by an operator ruling and is recorded here with its resolution, because the reasoning still
applies to how slices 9 and 10 are scoped.

1. **Slice 1 is not a pure refactor.** The plan describes it as "extract the lib, three consumers adopt
   it", but the monolith's corresponding commits also fix a real bug: the tools hardcoded the primary
   checkout, so `just defaults-capture` from a worktree wrote the wrong tree. Two of the monolith's tests
   exist for that bug. This design resolves the contradiction by putting the fix in slice 1 and saying so,
   because splitting a bug fix from the extraction that enables it would produce two half-slices. The
   reviewer should expect behavior change in slice 1 and not treat it as scope creep.
1. **The LuLu and OverSight casks were unowned; they are now slices 9 and 10.** The monolith adds two
   endpoint-security casks to `.chezmoidata/system_packages_autoinstall.yaml` under an "R8 endpoint
   hardening" commit, and no slice in the original eight covered package installation. The operator has
   ruled that both are required, so they became slices 9 and 10 rather than being dropped or smuggled
   into slice 4. The observation that drove the concern still stands and is why they are two slices at
   opposite ends of the review-weight scale: OverSight is passive and inert, while LuLu adds a system
   extension that prompts and can block traffic, including the traffic that carries the alerts.
1. **The Tier 2 runner's `{{ if .sudo }}` bug is a prerequisite, not a detail.** The monolith fixes it in
   its own commit with its own test. This design pulls it into slice 3, because slice 3 introduces new
   optional per-record fields and would otherwise reintroduce the identical absent-key throw for `tier`.
   Anyone building slice 3 must fix `sudo` and get `tier` right in the same pass.
1. **The legacy drop-in migration is a live deletion.** Slice 7's description says "Include precedence",
   which implies the rename but does not name the `rm` of `50-no-password-auth.conf` from `/etc/ssh` on a
   live machine. It is a small, correct deletion, but it is a privileged removal outside the chezmoi
   target tree, so it deserves explicit review attention rather than arriving as an implementation detail.
1. **The bash 3.2 constraint is a standing convention for slices 7 and 8, not a slice.** The script's
   shebang selects macOS's `/bin/bash` 3.2: no associative arrays, no `compgen` builtin. The monolith
   needed a late fix for exactly this. It is captured in the conventions section above so it is a
   precondition rather than a discovery.
1. **No rollback mode exists anywhere in the monolith.** Its recovery story is comments. This design adds
   `--rollback` in slice 8. That is new work, not a re-land, and it is the largest single addition the
   plan makes to the monolith.
1. **Three slices have no monolith source at all.** Slices 2, 3, and 5 are net-new from the product
   requirements document. Estimating S10 from the monolith's line count will understate it badly, and
   slices 9 and 10 make that worse rather than better: the monolith contributes one cask line to each,
   while the posture controls, the runbook entries, and slice 10's whole rule-existence design are new.
   Conversely, slice 4's signed-application policy record is an addition the monolith does not contain
   (it declares only global state and stealth), and slice 6 deliberately does not re-land the monolith's
   `run_after_67` script.
1. **The monolith wires `ssh-hardening.sh` into `macos_system_setup.yaml` with `sudo: false`, so the
   script self-escalates.** That means a `chezmoi apply` triggered by a change to that data file runs a
   script that calls `sudo` and writes to `/etc/ssh`. From automation with no interactive terminal that
   either prompts and hangs or fails. Slices 7 and 8 must state what an apply from a non-interactive
   context does with this record, and the answer must not be "it prompts".

### Other risks

- **The loopback readiness gap between slices 4 and 8.** Slice 4 turns on the firewall and stealth mode.
  Slice 8's readiness probe targets loopback, which the firewall does not filter. So a green `--reload`
  is compatible with a machine no remote client can reach. The drill's step 4 (a new session from a
  second host) is the only thing that closes this gap, which is why it is mandatory rather than advisory.
- **Passwordless sudo.** The product requirements document records that sudo on this machine requires no
  password. Every `sudo` in slices 2, 4, 5, 7, and 8 therefore executes without a prompt, including from
  automation. That makes the apply path smoother and the blast radius larger. It is out of scope to
  change here, but it is the reason the render-abort refusal in slice 3 matters: there is no password
  prompt standing between a malformed record and a privileged write.
- **The duplicate hermes profile directories block the acceptance drill**, as detailed in the known
  blocker section. They must be resolved before the end-state verification, and their resolution is its
  own change with its own review.
- **Screen lock cannot be enforced.** `sysadminctl -screenLock` requires `-password`, so screen lock is a
  `verify` control. It is currently immediate on this machine, and slice 6 will page if that changes, but
  nothing in S10 can set it back.
- **The alerting path does not egress where a reader would assume.** `alert-dispatch.sh` POSTs to a
  Hermes gateway on `127.0.0.1:8644`, and Hermes performs the Discord egress. Anyone reasoning about
  outbound filtering from the alerter's own `curl` call will protect the wrong hop. This is written into
  slice 10, but it also affects any future work that assumes the alerter talks to the internet directly.
- **The readability of LuLu's rules file is unresolved.** `/Library/Objective-See/LuLu/rules.plist` lives
  under `/Library` and its ownership and mode are unknown without installing the product. If it is
  root-only, slice 10's rule-existence check needs privilege to read it, which puts a `sudo` in a poller
  that currently runs unprivileged as a user agent. Slice 10 must settle this early, because "the check
  needs root" would change its design rather than its implementation.

## Out of scope

- Enabling System Integrity Protection. It is deliberately disabled and requires Recovery.
- Enabling FileVault or Gatekeeper from automation. Neither is settable from the command line on macOS
  26, which is precisely why they are `verify` controls.
- Any mobile-device-management profile. If a control genuinely requires one, it is declared `manual` and
  documented, not implemented.
- Turning Remote Login off, or changing the sudo password policy.
- Resolving the duplicate hermes profile source directories.
- S11, S12, the D1 cutover, and the Rust rewrite.
