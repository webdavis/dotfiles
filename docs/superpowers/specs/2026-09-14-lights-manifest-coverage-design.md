# Manifest coverage for the lights binary

Status: design, written 2026-09-14 for ledger task 63. Awaiting the operator's read. Nothing is
built or changed by this document.

Recommendation in one line: **no**. `~/.cargo/bin/lights` does not join either known-good manifest,
and the reason is written into the lights specification and the manifest generator rather than left
as an omission. The stale `~/.local/libexec/lights` install target in the lights plan and
specification is corrected in the same change.

## Why this exists

Ledger task 63 asks for a decision. The lights tool now installs to `~/.cargo/bin/lights`, the same
directory as pns, posture, uu and tailnet-pin, after the 2026-09-09 ruling took the four Rust tools
out of the libexec rule. Two of the binaries in that directory carry rows in the osquery pipeline
known-good manifest and three do not. The lights plan left the question open in as many words:

> Review whether the new generated binary needs a separate inventory entry before claiming coverage.

Task 62 makes the question live. It moves all seven aerospace function keys, F4 through F10, onto the
binary and retires `dot_local/libexec/executable_control-hue-lights.sh`. That Bash script is
manifested today, so the retirement removes real coverage from the lights function unless something
replaces it.

The task also names a second, smaller job: the plan and the specification still say the install target
is `~/.local/libexec/lights`, and they still justify it with a rule the tool no longer follows.

## Constraints this decision sits inside

Each of these is a standing rule, and each one narrows the answer.

1. **The pipeline manifest's single responsibility is the pipeline's own integrity.** The generator's
   own docblock records this as an operator ruling from slice 15, and gives it as the reason the two
   manifests are separate files rather than one list. A row for a tool outside the pipeline widens
   that responsibility, which takes a new ruling.
1. **The managed-bin manifest covers chezmoi-managed scripts that run unattended.** Its stated
   rationale is that LaunchAgents and shell hooks fire those scripts with nobody watching, so a tamper
   there executes on a timer. It is deliberately non-recursive under `~/.local/bin` and deliberately
   manifest-driven rather than directory-driven, because that directory also holds third-party shims
   that rewrite themselves.
1. **This repository tests the behavior of tools it wrote, and nothing else** (operator ruling
   2026-08-05). Declaration-consistency checks are deleted on sight. A guard that pinned which
   binaries appear in the generator's loop is exactly that shape and must not be added.
1. **Optimal over cheap** (operator ruling 2026-09-05). A recommendation may not rest on being less
   work. The argument below is a trust-boundary argument; the cost accounting is secondary and is
   reported separately so the operator can price the alternative.
1. **No removal mechanisms** (operator ruling 2026-08-02). The deployed leftover from the pre-move
   install path is the operator's to trash, not an apply's to clean.
1. **The operator runs applies.** Nothing in this decision can be validated by an agent applying it.

## The existing state, measured

Everything in this section was read off dresden and off the source tree while writing the document.
Nothing here is from memory.

### The two manifests and what they hold

`/var/osquery/pipeline-known-good.sha256` holds 34 rows. Thirty-two come from chezmoi-managed intent:
the page-launchd allowlist, seventeen scripts under `~/.local/libexec/osquery`, seven data files under
`~/.local/libexec/posture`, and seven of this repository's own osquery LaunchAgents. The last two rows
are the compiled binaries:

```
55f1d6d8...ced75ac5 0755 501 /Users/stephen/.cargo/bin/pns
0eac6082...12c1dbcea 0755 501 /Users/stephen/.cargo/bin/posture
```

Those two are appended after the managed rows rather than merged into the sort, so the file is not
globally path-sorted; only the appended tail is sorted among itself. The generator's comment says so.

`/var/osquery/managed-bin-known-good.sha256` holds 16 rows: two files under `~/.local/bin` and
fourteen under `~/.local/libexec`, including `~/.local/libexec/control-hue-lights.sh`, the Bash script
lights replaces.

### Where a compiled binary's hash comes from

Neither compiled row is hashed off the deployed tree. `authorized_record_hash` in
`.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh` reads a build record at
`~/.local/state/<tool>-build-record`, validates its three lines (`sha256 <64 hex>`, `bytes <n>` within
a per-tool ceiling, and a `rustc ...` line), and prints the digest. A missing record yields the
literal token `unbuilt`. No live or target-directory bytes are ever adopted.

`~/.local/state` holds exactly two such records, `pns-build-record` and `posture-build-record`. There
is no lights record and no uu record.

### The three integrity tiers already in the repository

Five binaries install to `~/.cargo/bin` from this checkout. They already sit in three different
tiers, so this decision picks among shapes the repository already has rather than inventing one.

| Binary        | Mechanism today                                                              | Who invokes it                                    |
| ------------- | ---------------------------------------------------------------------------- | ------------------------------------------------- |
| `posture`     | build record, pipeline manifest row, exact-match arm in `pipeline_path`      | the osquery LaunchAgents; it is the pipeline      |
| `pns`         | build record, pipeline manifest row, no `pipeline_path` arm                  | every producer; it is the pipeline's delivery leg |
| `tailnet-pin` | a source-fingerprint marker its caller checks before running it under `sudo` | `run_onchange_after_41`, as root                  |
| `uu`          | nothing                                                                      | `com.webdavis.uu`, weekly, unattended             |
| `lights`      | nothing                                                                      | seven aerospace keys, on an operator keypress     |

The tailnet-pin tier works differently from the other two.
`.chezmoitemplates/tailnet-pin-source-fingerprint.tmpl` concatenates every source byte at render time
and hashes it; the builder writes the same hash to `~/.cache/tailnet-pin-build/installed.fingerprint`;
and `run_onchange_after_41` refuses to run the binary as root when the marker disagrees with the
render. Its stated purpose is that the builder defers with exit 0 when cargo is missing, and without
the check that deferral would aim an older binary at `/etc/hosts` as root.

### What the osquery watch set covers

`dot_local/libexec/posture/converge/desired/osquery.conf.tmpl` declares eight `file_paths` groups:
`~/.ssh`, `~/.config/osquery`, `~/.local/libexec/osquery`, `~/.local/libexec/posture`,
`~/.local/bin`, `~/.local/libexec`, both LaunchAgents directories plus LaunchDaemons, `/etc/sudoers`
and the sshd configuration. None of the four osquery packs declares a `file_paths` group of its
own.

`~/.cargo/bin` is watched by nothing. Two consequences follow, and both bear on the decision.

First, a manifest row for a cargo-bin binary buys periodic coverage only, never event-driven
coverage. No file event can ever arrive for `~/.cargo/bin/lights`, so the only consumer of such a row
is the periodic audit: the Rust watchdog in `posture/crates/posture-adapters/src/watchdog_audit.rs`,
and the Bash `pipeline-audit.sh` until task 46 retires it. Both walk every row of both manifests and
compare the deployed file's digest, mode and owner against the tuple. The watchdog LaunchAgent runs on
a `StartInterval` of 900 seconds, so the detection window is fifteen minutes.

Second, the exact-match arm for posture in `pipeline_path` is currently inert. It exists to make an
event for the posture binary page rather than fall through as an untracked neighbor, and no such event
can be produced. Its comment states the reason for exactness clearly, and that reason stands whatever
this decision is:

> The posture binary moved to `~/.cargo/bin` on 2026-09-09. It is matched EXACTLY, never by prefix:
> that directory is shared with every other cargo-installed program on the machine, and a prefix would
> pull all of them into the pipeline manifest.

pns has no such arm. Under `manifest_for` a cargo-bin path routes to the pipeline manifest anyway
because it is not a bin path, so the omission changes nothing today; it becomes a real disagreement
the moment `~/.cargo/bin` is watched. Git history shows the posture arm was added in `a9b6e00d`, the
commit that moved the four tools, and pns was simply not added with it.

### What lights is, and who calls it

`lights` is a Philips Hue controller. Its seven callers are aerospace keybindings in
`dot_aerospace.toml`, five of which still call the Bash script and two of which still call `openhue`
directly. Task 62 repoints all seven.

It is not on the alerting path and it is not on the delivery path. pns owns its own Hue client in
`pns/crates/pns-adapters/src/hue.rs` and its own `[lights]` configuration tables; it never spawns the
lights binary. Nothing under `Library/LaunchAgents`, nothing in `dot_bashrc.tmpl`, no hook and no
`just` recipe invokes it. Grepping the deployed-script trees for it returns only the retiring Bash
script's own log path and unrelated Neovim highlight names.

### The coverage the port loses

`~/.local/libexec/control-hue-lights.sh` is manifested and watched today. It earned that by being a
chezmoi-managed file inside a covered directory, not by anyone judging that a desk-lamp tool needs
integrity monitoring. When task 62 deletes it, the lights function's coverage goes from
watched-and-manifested to nothing, because the destination directory is watched by nothing.

The same regression already happened, unremarked, when uu moved out of `~/.local/libexec` on
2026-09-09, and uu is the stronger case: it runs weekly under launchd with nobody watching, which is
the managed-bin manifest's own stated criterion.

### The pre-move leftover

`~/.local/libexec/lights` still exists on dresden: 3,091,456 bytes, dated Sep 9, installed by the
builder before the destination moved. It sits inside the watched `~/.local/libexec` tree and is silent
only because `_managed_bin_is_tracked` takes its tracked set from the manifest and this file is not in
it. It is already on the ledger's deployed-leftovers list at `docs/remaining-work.md:287` and wants
trashing. That ledger line is correct as written and must not be swept up as a stale libexec
reference.

### Cost of a row, measured

`shasum -a 256` on the current 3,096,064-byte lights binary: 0.05 seconds, three consecutive runs. On
pns, 0.06 to 0.10 seconds. Against a 900-second tick, one more row is a rounding error, and it stays a
rounding error while both audits run. The audit's default bounds are 500 entries, 8,388,608 bytes per
file and a 60-second budget shared across manifests, so a 35th row crosses nothing and lights sits
under the shared byte ceiling with room to spare.

Cost does not decide this one. The manifests' stated responsibilities do.

## Approaches

### Approach A: no record for lights, and the tier is named

lights joins the "no integrity record" tier and the decision is written down in the two places a
future reader will look: the lights specification's Decisions section and the generator's docblock,
next to the sentence that explains why the compiled rows exist at all.

Why it fits the existing rules without needing a new one. The pipeline manifest's responsibility is
the pipeline's integrity; lights is not pipeline, and pns's row is not a precedent for arbitrary Rust
tools because pns is the leg every security page travels down. The audit says so in its own refusal
text: "do not trust its delivery acknowledgements". The managed-bin manifest's criterion is unattended
execution; lights runs on a keypress, in the foreground, at the operator's own privilege.

What it accepts. A tampered lights binary would run arbitrary code on F4 through F10 with the
operator's privileges and nothing would notice. That is the same exposure every Homebrew formula,
every cargo-installed tool and every npm shim on the machine already carries, and the manifest was
never the boundary for that class.

What it costs. Two documentation edits and one comment. No apply, no manifest change, no rebuild, no
new test.

### Approach B: parity with posture

lights gets a build record, a pipeline manifest row, a per-tool artifact ceiling and an exact-match
arm, so the binary is hashed against its authorized build tuple on every audit tick.

What it buys. Fifteen-minute periodic detection of post-deployment tampering of one binary, and a
critical page when it diverges.

What it costs, concretely. The lights builder grows the whole publication block that posture's
builder carries: `umask 077`, a size refusal against the ceiling before anything is published, the
digest, `rustc --version --verbose` taken from the crate directory so the recorded compiler is the one
the build selected, a 0600 record written through a temporary file, the byte-identical retain path
that keeps the tuple and its publication time, a scoped `--pipeline-only` manifest refresh, record
rollback when that refresh fails, and the atomic install ordered after publication. That is roughly a
hundred lines of security-critical Bash, none of it shareable, because a chezmoiscript cannot source a
library that is not deployed yet.

Then: a third entry in `rust_tools.max_artifact_bytes`; the same number copied by hand into the
generator's `max_artifact_bytes` table, which is a plain script and cannot read the data file; `lights`
inserted at the head of the generator's `for refresh_manifest_binary in pns posture` loop, because
the appended rows must stay sorted among themselves and `lights` sorts before `pns`; an arm in
`pipeline_path`; a new bashunit suite mirroring `test/unit/pns-build-record.test.sh`; and two extended
cases in `test/unit/posture-manifest-refresh.test.sh`, the path-order case and the per-tool ceiling
case. No Rust audit constant is needed: at 3.1 megabytes lights sits under the shared 8 MiB default,
so `bounds_for` stays a pns-only override.

Two failure modes come with it. A deferred lights build publishes no record, the generator writes
`unbuilt`, and `audit_file` turns an `unbuilt` row with a present regular file into a content finding,
so every tick pages until a real build publishes a record. A row must therefore never reach a machine
before the first successful lights build. And the lights builder becomes a privileged writer: it
invokes the generator, which invokes `sudo install`, so a desk-lamp tool's rebuild gains the power to
fail an apply and to write a root-owned file.

Its security ceiling is the one the generator already records. A row buys post-deployment integrity,
not supply-chain integrity of the source, and on this single-user host with passwordless sudo a
process running as the operator can rewrite the manifest at will. For a keypress-only tool the
marginal gain over approach A is small.

### Approach C: widen the boundary properly, and decide lights inside it

Add `~/.cargo/bin/%%` to the watch set and track it manifest-driven, the way `~/.local/bin` already
is, so the binaries that carry rows get event-driven coverage and every other cargo-installed program
in that shared directory stays silent. Close the uu gap in the same change, and give pns the
`pipeline_path` arm it is missing.

Of the three this is the one that improves the machine's posture, and it is what "optimal over
cheap" points at. It is also not a lights change. Its justification comes from uu running weekly
under launchd and from pns carrying every security page; lights would be a passenger. It touches
`osquery.conf`, so it needs an operator-run osqueryd restart, which is task 59's territory, and it
needs the churn question answered for real: uu's own cargo lane upgrades cargo-installed tools
weekly, and a directory watch there reports every one of those rewrites. Manifest-driven tracking is
what makes that affordable, and proving it is affordable is a task of its own.

Recommendation: file it as its own task rather than fold it into a decision about a lamp controller.

## Recommended design

Approach A. The decision, in the sentence to be recorded:

> `~/.cargo/bin/lights` carries no build record and appears in neither known-good manifest. The
> pipeline manifest's responsibility is the osquery pipeline's own integrity, and the managed-bin
> manifest's criterion is unattended execution. lights is neither: it is invoked only by seven
> aerospace keybindings, in the foreground, at the operator's own privilege. The coverage its
> retired Bash predecessor had was incidental to that script being a managed file in a watched
> directory, not a judgment that the tool needs integrity monitoring.

### What is written, and where

Documentation only. No code, no data, no manifest, no test.

1. `docs/superpowers/specs/2026-09-06-lights-design.md`, Decisions item 1 at lines 461 to 463.
   Replace the install target and its justification. Both halves are stale: the path is
   `~/.cargo/bin/lights`, and the reason is no longer the libexec rule but its 2026-09-09 exemption,
   which moved the four Rust tools out because they are products other people install with
   `cargo install` rather than private helpers of this checkout. The sentence "pns already lives
   there even though the operator types `pns doctor` by hand" is now false and is replaced by the
   exemption's own reasoning.
1. The same specification, line 224. The pns invocation example names `~/.local/libexec/pns/pns`.
   It becomes `~/.cargo/bin/pns`.
1. A new Decisions item in the same specification, carrying the decision sentence above.
1. `docs/superpowers/plans/2026-09-06-lights-plan.md`, lines 173 to 175. Same install-path
   correction as the specification's item 1.
1. The same plan, line 205. It says two entries go in "the darwin-conditional block":
   `.local/libexec/lights` and `.config/lights`. Wrong three ways against the shipped
   `.chezmoiignore`: the conditional block is a linux block listing what to drop on Linux, not a
   darwin block; `.local/libexec/lights` does not exist in it, because the binary is built by an
   after-phase script and is never a chezmoi target, so there is nothing to ignore; and the
   source-only exclusion is the bare name `lights` at the target root, at line 70, beside `pns`,
   `uu`, `posture` and `tailnet-pin`. Only `.config/lights` is in the linux block, at line 86.
1. The same plan, lines 224 to 230. All seven aerospace binding examples name
   `~/.local/libexec/lights`. They become `~/.cargo/bin/lights`. These are examples task 62 will
   implement, so leaving them stale hands task 62 a wrong path.
1. The same plan, "The known-good manifest consumer" section at lines 236 to 242. Its open question
   is answered: replace "Review whether the new generated binary needs a separate inventory entry
   before claiming coverage" with the decision and a pointer to this document.
1. `.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh`, the comment above the
   `for refresh_manifest_binary in pns posture` loop. Add one sentence naming why the loop holds
   exactly these two and not every binary the repository builds, so the next reader finds the tier
   rule where the loop is rather than in a plan.

### What is deliberately not changed

- **`docs/remaining-work.md:287`** keeps its reference to `~/.local/libexec/lights`. That path is a
  real leftover on disk, dated Sep 9, and the line is a trash-this note, not a stale target.
- **The specification's lines 5, 111 and the plan's 246 and 250** keep their `libexec` paths. They
  name the Bash script's source path and its deployed leftover, both correct.
- **`pipeline_path` in `posture/crates/posture-domain/src/known_good.rs`** is not touched. Under
  approach A lights needs no arm, and pns's missing arm is a separate question, raised below.
- **No test is added.** A guard asserting which binaries the loop names would be
  declaration-consistency checking, which the 2026-08-05 ruling deletes on sight. The accepted price
  is that a future reader who adds a binary by reflex is caught by review rather than by a gate,
  which is what that ruling says the price is everywhere else in this repository.

### How the decision is verified

There is nothing to run. The verification is that the four claims the decision rests on are true on
this machine, and each was measured while writing this document: no watch covers `~/.cargo/bin`;
`~/.local/state` holds two build records and neither is lights; pns never spawns the lights binary;
and the lights binary's only callers are seven aerospace keybindings. If any of those changes, the
decision is reopened, and the sentence recorded in the specification says which fact it depends on.

## Out of scope

- Task 62's key moves, hardware drills and Bash retirement. This document only settles what happens
  to the manifest and corrects the path the plan hands task 62.
- Closing the uu gap. Named as an open question, not decided here.
- Adding `~/.cargo/bin` to the watch set. Approach C, recommended as its own task.
- Giving pns the `pipeline_path` arm it lacks. Named as an open question.
- Trashing `~/.local/libexec/lights` and `~/Library/Logs/smart-lights.log`. Operator-run, already on
  the deployed-leftovers list.
- The Bash `pipeline-audit.sh` retirement and its `pipeline-verdict.sh` dependency. Task 46.
- Any change to the pns or posture rows, their ceilings or their records.

## Assumptions made in the operator's place

Each of these is a choice made without the operator awake. The alternative is stated so it can be
overturned by answering rather than by rereading.

1. **The slice-15 ruling on the pipeline manifest's single responsibility still binds, and pns's row
   sits inside it as the pipeline's delivery leg.** Alternative: the operator reads pns's row as
   having already widened the manifest to "the binaries this repository builds", in which case lights
   and uu both join it and approach B is the answer for both.
1. **"Runs unattended" is the criterion that earns an integrity record**, taken from the managed-bin
   manifest's own stated rationale. Alternative criterion: "everything this repository compiles",
   which admits lights, uu and tailnet-pin and makes the answer yes three times.
1. **The coverage the Bash script had was incidental.** Alternative: it was deliberate, the port must
   preserve it, and approach B is required to avoid a regression.
1. **The seven aerospace keys stay bare `exec-and-forget` strings.** That is what rules out the
   tailnet-pin caller-side fingerprint pattern, which needs a caller with room for a check.
   Alternative: wrap the seven keys in one gate script, which buys a build-provenance check at the
   cost of a shell fork on every keypress and reintroduces the Bash layer the port removed.
1. **The uu gap is a separate decision.** Alternative: decide all of `~/.cargo/bin` at once, which is
   approach C and a larger change.
1. **No new test.** Alternative: add the membership guard anyway and accept that it contradicts the
   2026-08-05 ruling.
1. **If the operator says yes, the lights ceiling is 6 MiB (6,291,456 bytes)**, following the
   existing convention of about twice the measured size rounded up to a whole mebibyte, from
   3,096,064 bytes measured on dresden. Alternative: reuse posture's 8,388,608 so two tools share one
   number, at the cost of a looser bound on the smaller binary.
1. **The tier rule is recorded in two places, the specification and the generator's comment.**
   Alternative: one place only, which means a reader at the loop does not find it.

## Open questions

1. **Does `~/.cargo/bin/lights` carry a build record and a manifest row?** This document recommends
   no. Accepting or rejecting that is the decision task 63 asks for.
1. **Does `~/.cargo/bin/uu` carry one?** It runs weekly under `com.webdavis.uu` from that path, with
   nobody watching, which satisfies the managed-bin manifest's own stated criterion, and it has no
   record and no row today. It is a stronger case than lights, and it is the gap this investigation
   surfaced.
1. **Is pns's absence from the exact-match list in `pipeline_path` intentional?** The generator
   writes a pns row; `known_good.rs` names only posture. Inert today because nothing watches
   `~/.cargo/bin`, a real disagreement the moment something does.
1. **Should `~/.cargo/bin` join the watch set with a manifest-driven tracked set?** That is what
   would turn a fifteen-minute periodic hash into event-driven coverage for pns and posture. It
   needs an operator-run osqueryd restart and the churn question answered against uu's own weekly
   cargo upgrades.
1. **Is the generator's comment the right home for the tier rule, or should it live only in the
   lights specification?** Two homes means two places to keep true; one means a reader at the loop
   does not find it.
