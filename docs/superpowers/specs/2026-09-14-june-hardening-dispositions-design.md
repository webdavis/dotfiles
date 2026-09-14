# June hardening dispositions: signature chain, interpreter payload, per-run grouping

Status: design, written 2026-09-14 with the operator asleep. Nothing is decided. Every choice made in
the operator's place is listed in "Assumptions made in the operator's place" with its alternative, and
the questions that need an answer are listed at the end. No code was written or changed.

## Why this exists

The work ledger carries this task under "Recover the remaining design from PR #24":

> Give the June hardening requirements explicit dispositions: signature-chain verification,
> interpreter-payload assessment and per-run grouping of repeated findings about the same subject.
> Current Rust code reads signature metadata, leaves interpreter payloads unverified and retains each
> finding in a batch. Porting the existing behavior did not implement these proposed changes.

The three requirements come from `docs/superpowers/plans/2026-06-04-osquery-alerter-hardening.md` on the
`docs/osquery-design` branch (head `2202dcbf`), tasks 4, 5 and 3 respectively. They were written against
the Bash alerter and enricher, which have since been ported to the Rust `posture` workspace. The port
carried the old behavior forward, so all three requirements are still open, and none of them has a
recorded decision.

This document supplies one explicit disposition for each: what the code does today, measured; what the
June plan asked for; whether that request survives contact with the current system and the current
operating system; and what to build instead where it does not.

## What was decided before this, and what superseded it

Four documents bear on these three requirements. They do not agree with each other, and the
disagreements are the reason a disposition is needed rather than an implementation.

`docs/superpowers/plans/2026-06-04-osquery-alerter-hardening.md` (the June hardening plan) is the source
of all three requirements. Its binding rule is stated in its own conventions section: "a change may make
a finding louder (NOTICE to CRIT) but must never make a real threat quieter or drop it." That rule still
binds and every disposition below respects it.

`docs/superpowers/specs/2026-06-03-osquery-alerter-ingestion-model-design.md` (the ingestion model),
written one day earlier, rejects the hardening plan's framing of the grouping requirement. Its verdict:
"the duplicate-alert bug is a symptom; the root cause is a model mismatch," and "patching with dedupe
`file_events_recent` by path treats the symptom." Its decision D2 removes `launch_agents` and
`launch_daemons` from the osquery configuration's watched trees so the duplication disappears at source,
which shrinks the grouping requirement from "all evented findings" to exactly three subjects: secure
shell keys, the sudoers files, and the secure shell daemon configuration. Its decision D4 keeps grouping
for those three only. D2 is gated on one verification: does the `launchd` table enumerate a user's
`~/Library/LaunchAgents` plists, including one present on disk but not loaded.

The later June 10 decision addendum and the version 2 master specification narrow the analysis-agent
scope but say nothing about these three requirements.

Two decisions taken after June change the ground under the ingestion model:

- `es_launchd_writes` is now log-only by design. The ledger records this as "intentional log-only
  `es_launchd_writes` handling", and `posture-domain/src/gate.rs` routes the detector to
  `GateOutcome::LogOnly` unconditionally. One of the two evented detectors the June plan wanted to group
  therefore cannot reach a page or a digest at all.
- The `launch_agents` and `launch_daemons` watched trees were repurposed. They no longer act as a
  redundant persistence detector layered on `persistence_launchd`. They now feed the file-integrity arm:
  `gate.rs` sends `FileCategory::LaunchAgents` and `FileCategory::LaunchDaemons` to the integrity
  verdict, and `posture-domain/src/known_good.rs` treats
  `~/Library/LaunchAgents/com.webdavis.osquery-*.plist` as a tracked, manifested path.

That second change closes D2. Removing those two trees from the configuration would delete the integrity
watch on the seven manifested osquery launch agents and would delete the instant-latency detection of a
newly written launch agent plist, replacing it with the `persistence_launchd` differential query at a 600
second interval. The redundancy-removal route to fixing the duplicate-alert bug is gone, which leaves
per-run grouping as the only remaining lever. That is the single most consequential finding in this
document.

For completeness, the D2 gating question now answers yes, and the answer no longer decides anything.
Measured on this machine, osquery 5.23.1, macOS 26.2 build 25C56:

```
osqueryi "SELECT count(*) FROM launchd WHERE path LIKE '$HOME/Library/LaunchAgents/%';"  ->  27
ls -1 "$HOME"/Library/LaunchAgents/*.plist | wc -l                                       ->  27
```

Two of those 27 labels (`com.federicoterzi.espanso` and `Proton Mail Bridge`) are absent from
`launchctl list`, and both appear in the `launchd` table with their paths. The table is derived from
disk, not from the loaded job set, so it does see an unloaded user plist. This was measured as the
operator; the daemon runs as root, and a root process enumerates a different set of home directories,
so the reading is suggestive rather than conclusive for the daemon's own view.

## What the code does today, measured

Every claim in this section was measured on this machine on 2026-09-14 against the installed
`~/.cargo/bin/posture` binary and the live osquery results log, or read directly out of the source in
`posture/crates/`. Nothing here comes from memory.

### The enricher has two paths, and they disagree about strictness

`posture-application/src/enrich.rs` routes a finding by its path. A `.plist` goes to `launchd()`, which
resolves `Program`, falling back to `ProgramArguments.0`. If the resolved program's basename is in
`posture-domain/src/enrich.rs`'s `is_interpreter` list, the function looks for the first of
`ProgramArguments.1` through `ProgramArguments.5` that starts with `/` and is an existing file, renders
"runs script `<name>` via `<interpreter>`, payload unverified", and returns
`CodeTrust::TrustedOrNotApplicable`, which is exit code 0. If no such argument exists it renders "runs
`<interpreter>` (interpreter), payload unverified" and again returns trusted. Quarantine is checked on
the resolved script and appended to the text as ", downloaded", but it does not change the verdict.

If the program is not an interpreter, the finding goes to `code()`, which runs
`codesign -dv --verbose=2` with standard error merged and hands the text to `classify_signing`. That
function is a pure text parser. It returns untrusted for text containing "not signed" or "adhoc", and
otherwise takes the first `Authority=` line, maps `Software Signing` or anything beginning with `Apple`
to the display string "Apple", strips a `Developer ID Application: ` prefix, and returns trusted.

The trust decision is therefore made by a string that the signer chose. A certificate whose subject
common name is "Apple Mac OS Application Signing" satisfies `authority.starts_with(b"Apple")` and reads
as `signed: Apple`, trusted, whatever its chain. That is red-team finding number 7 as the June plan
recorded it, and it is unchanged in the Rust port.

A live reproduction of the spoof was attempted and did not complete. A self-signed code-signing
certificate with that common name was generated and imported into a temporary keychain, but `codesign -s`
answered "no identity found" because establishing code-signing trust for a certificate requires
`security add-trusted-cert`, which requires an administrator. The temporary keychain was never added to
the keychain search list and was deleted in the same session. The generated key material remains in the
session scratchpad at `.../scratchpad/spoof/` and should be discarded. The claim above therefore rests on
reading `classify_signing`, which is a pure function whose only input is attacker-supplied text, rather
than on a live spoof. Completing the live spoof is an operator step.

### The signing verdict is already untrusted for two fifths of this machine's launch jobs

`posture enrich` was run against all 45 readable plists in `~/Library/LaunchAgents`,
`/Library/LaunchAgents` and `/Library/LaunchDaemons`:

| Outcome                                                  | Count |
| -------------------------------------------------------- | ----- |
| Untrusted, exit 10                                       | 19    |
| Trusted, exit 0                                          | 26    |
| of the trusted, interpreter path with an unassessed payload | 12  |

The 19 untrusted verdicts are every one of them a legitimate job: `com.webdavis.atuin-daemon`,
`happy-daemon`, `osquery-digest`, `osquery-heartbeat`, `pns-daemon`, `scalebar` and `uu`;
`homebrew.mxcl.ollama` and `homebrew.mxcl.postgresql@17`; four Microsoft OneDrive updaters; Docker's
socket helper; two Google Keystone agents; `com.qmd.mcp`; and `com.tailscale.tailscaled`.

Two of those deserve their own note, because they are mislabeled rather than untrusted:

- `/Library/LaunchDaemons/com.tailscale.tailscaled.plist` is mode 0700 root:wheel. The operator cannot
  read it, so `plutil` fails on both keys and the enricher reports "launchd job, no program resolved
  (untrusted)". The alerter runs as a user launch agent, so this is its normal view of that file. By
  contrast the two Keystone plists are genuinely empty dictionaries, so "no program resolved" is honest
  for them.
- `/Library/PrivilegedHelperTools/com.docker.socket` is mode `-rwx--x--x` root-owned, so `/usr/bin/file`
  cannot read it, `is_mach_o` answers false, and the finding falls through to a plain metadata line with
  a trusted verdict when reached by path. Reached through its plist, `codesign` also cannot read it and
  the verdict is "UNSIGNED".

There is no state for "could not be inspected". Failure, absence, an unreadable file and a deadline all
collapse into the same two answers. `CommandRunner::run` maps any nonzero exit to
`InspectionFailure::Failed`, and `code()` does `classify_signing(reading.as_deref().ok())`, so `None`
renders the word "UNSIGNED" whatever the real cause was.

### The untrusted verdict is the only thing that promotes, and it overrides the allowlist

`posture-domain/src/gate.rs` contains the only use of the signing verdict that changes an outcome:

```rust
if signing.is_some_and(|signing| signing.untrusted) && severity == Severity::Notice {
    severity = Severity::Critical;
}
```

For `PersistenceLaunchd`, an allowlisted user launch agent falls to the `critical` branch, which pages
when severity is Critical and is log-only otherwise. Because severity for that detector starts at Notice
and the untrusted signing verdict promotes it, an allowlisted agent with an ad-hoc signed program pages
anyway. The June plan named this in its task 6 note: "an allow-list only quiets NOTICE."

Measured against the live allowlist at `~/.config/osquery/page-launchd-allowlist.txt`, which holds nine
entries: four of the nine (`com.webdavis.osquery-digest`, `com.webdavis.osquery-heartbeat`,
`com.webdavis.pns-daemon`, `com.webdavis.scalebar`) enrich untrusted today because cargo-built binaries
carry an ad-hoc signature. Allowlisting them does not quiet them.

### Every interpreter-fronted job on this machine takes the soft path

Twelve of the 45 plists front an interpreter. Their forms, and what `posture enrich` actually says:

| Program form                       | Enricher output                                | Exit |
| ---------------------------------- | ---------------------------------------------- | ---- |
| `/bin/sh -c '<command string>'`    | `runs sh (interpreter), payload unverified`    | 0    |
| `python -m hermes_cli.main`        | `runs python (interpreter), payload unverified` | 0   |
| `node build/main.js` (relative)    | `runs node (interpreter), payload unverified`  | 0    |
| `bash /abs/path/results-alerter.sh` | `runs script results-alerter.sh via bash, payload unverified` | 0 |

The four rows above are `org.nixos.nix-daemon`, `ai.hermes.gateway`,
`com.webdavis.yt-dlp-pot-provider` and `com.webdavis.osquery-results-alerter`.

Seven of the twelve carry a payload the resolver cannot reach at all: three `sh -c` command strings,
three `python -m` module names, and one relative script path. The resolver requires an argument
beginning with `/` that is an existing file, and none of those seven qualifies. Five resolve, and all
five are manifested pipeline scripts under `~/.local/libexec/osquery/`.

The combined effect is an inverted strictness. A job whose program is a plain binary is judged strictly
enough to fail 19 times on a clean machine. A job whose program is an interpreter is trusted
unconditionally, and the trusted-unconditionally set includes the three most common malware-persistence
forms.

### A batch keeps every row, and repeated subjects crowd the page

`posture-adapters/src/judge_batch.rs` maps over `rows(records)` once to collect a signing verdict per
row, then again to gate each row, then a third time to push each paging row into `page_findings`. There
is no grouping anywhere: `grep -rni "coalesc\|dedup" posture/ --include=*.rs` finds only a
`COALESCE` inside a query string and an unrelated `roots.dedup()`. `posture-domain/src/page.rs`
renders at most `BLOCK_LIMIT = 8` blocks and caps the body at `BODY_LIMIT = 1900` characters, so
repeated blocks about one subject displace distinct subjects behind "… and N more CRITICAL
finding(s)".

The magnitude, measured from the machine's own `~/.local/log/osquery/osqueryd.results.log` (12,823 lines,
830 `file_events_recent` rows). The alerter runs on a 300 second interval and `file_events_recent` runs
on a 10 second interval, so a batch spans about 30 query runs. Bucketing rows by their own `time` column
into fixed 300 second windows, the worst same-path count in one window:

| Path                                                          | Rows in one 300s window | Rows in the whole log |
| ------------------------------------------------------------- | ----------------------- | --------------------- |
| `~/.local/bin/herdr-reviewr`                                  | 11                      | 109                   |
| `~/.local/libexec/pns/.DS_Store`                              | 8                       |                       |
| `~/Library/LaunchAgents/homebrew.mxcl.moshi-hook.plist`       | 5                       | 45                    |
| `~/.local/libexec/pns/pns.real`                               |                         | 50                    |
| `/Library/LaunchDaemons/com.docker.vmnetd.plist`              |                         | 20                    |

An eleven-row burst about one file fills all eight blocks of a page. That is the failure the June plan
described, still live, measured rather than modeled.

The duplication has a second, structural source the June documents did not notice: the watched trees
overlap. In `dot_local/libexec/posture/converge/desired/osquery.conf.tmpl`, `managed_bin` watches
`~/.local/bin/%%` and `~/.local/libexec/%%`, while `pipeline_integrity` watches
`~/.local/libexec/osquery/%%` and `~/.local/libexec/posture/%%`. The second is entirely inside the first,
so osquery emits one row per matching tree and every write to a pipeline file produces at least two rows
with different `category` values. Measured: 26 distinct paths in the live log appear under both
`managed_bin` and `pipeline_integrity`. Both categories route to the same integrity arm in the gate, so
both rows produce an integrity page for the same file.

### One inspection budget covers the whole batch, not one finding

`posture/src/alert.rs` declares `INSPECTION_BUDGET: Duration = Duration::from_secs(10)` with the comment
"One budget covers every spawned inspection for a finding, plist fallback included, matching what
`posture enrich` gives itself", and the body repeats it in capitals. The code does something else.
`SystemRunner::new(INSPECTION_BUDGET)` stores `Budget::Total(Instant::now() + budget)`, an absolute
deadline fixed at construction, and `execute` constructs it once for the whole run. So one 10 second
deadline covers every inspection of every row in the batch. `posture enrich` really does give one finding
10 seconds, so the two paths differ and the comment describes the wrong one.

The consequence compounds with the missing grouping. A plist finding spawns up to eight processes
(`plutil` twice, then either `file` plus `codesign` plus `xattr`, or up to five more `plutil` calls plus
`xattr`), at roughly 30 milliseconds each. Once the shared deadline is spent, every remaining row reads
`InspectionFailure::TimedOut`, which `code()` turns into the word "UNSIGNED" and an untrusted verdict,
which promotes Notice to Critical. A noisy batch can therefore manufacture critical findings out of a
deadline.

This is not one of the three requirements and it is not proposed for change here. It is recorded because
both the chain-verification and the grouping dispositions move load across that deadline, and a decision
made without it would be made on a false picture.

## Three measurements that change what the June plan asked for

### `spctl` cannot gate an executable on macOS 26

The June plan's task 4 proposes the gate
`codesign --verify --strict "$f" && spctl -a -t exec "$f"`. On this machine, macOS 26.2 build 25C56,
`spctl --assess -t exec` rejects every plain Mach-O executable, Apple's own included:

| Target                        | `spctl --assess -t exec`                                              | Exit |
| ----------------------------- | --------------------------------------------------------------------- | ---- |
| `/bin/echo`                   | `rejected (the code is valid but does not seem to be an app)`         | 3    |
| `/usr/bin/codesign`           | `rejected (the code is valid but does not seem to be an app)`         | 3    |
| `/opt/homebrew/bin/jq`        | `rejected`                                                            | 3    |
| `~/.cargo/bin/pns`            | `rejected`                                                            | 3    |
| `/System/Applications/Calculator.app` | accepted                                                      | 0    |
| `/Applications/Ghostty.app`   | accepted                                                              | 0    |

`spctl` assesses app bundles. Every launch job whose program is a plain executable would be marked
untrusted by that gate, which on this machine is most of them. Do not build task 4 as written. The
repository already knows `spctl` is unreliable here in another role: `posture-domain/src/poll/render.rs`
carries the note that "spctl cannot enable Gatekeeper from the CLI on macOS 15+".

### `codesign --verify` alone does not establish chain trust, but a requirement does

`codesign --verify --strict` passes for ad-hoc signatures, so it is not a trust check on its own:

| Target                  | `--verify --strict` | `-R="anchor apple"` | `-R="anchor apple generic"` |
| ----------------------- | ------------------- | ------------------- | --------------------------- |
| `/bin/echo`             | pass                | pass                | pass                        |
| `Calculator.app`        | pass                | pass                | pass                        |
| `Ghostty.app`           | pass                | fail (exit 3)       | pass                        |
| `Ghostty.app` inner Mach-O | pass             |                     | pass                        |
| `/opt/homebrew/bin/jq`  | pass                | fail (exit 3)       | fail (exit 3)               |
| `~/.cargo/bin/pns`      | pass                | fail (exit 3)       | fail (exit 3)               |
| unsigned copy of `/bin/echo` | fail (exit 1)  |                     | fail (exit 1)               |

The requirement form discriminates on the certificate chain's anchor, which is exactly what "verify the
chain, not the name" means, and it needs no second tool. Two facts that shape the design:

- The exit codes separate the cases. Exit 1 is a signature that is absent or invalid ("code object is
  not signed at all", or "a sealed resource is missing or invalid"). Exit 3 is a valid signature that
  does not satisfy the requirement ("test-requirement: code failed to satisfy specified code
  requirement(s)"). Anything else is a failure to inspect. `CommandRunner::run_completed` already
  returns the exit code, so the adapter can tell them apart without a text parse.
- Verifying an inner Mach-O of a healthy bundle passes, so no special bundle-ascent rule is needed.
  `codesign` already recognizes the enclosing bundle: measured on Ghostty's inner executable, which
  passes, and on BlueBubbles' inner executable, which fails with a bundle-level complaint.

Cost, measured by repeated invocation:

| Target                      | Per call                |
| --------------------------- | ----------------------- |
| plain Mach-O (`/bin/echo`)  | about 27 ms             |
| `codesign -dv` (the existing call) | about 30 ms      |
| `Google Chrome.app`         | 135 ms                  |
| `Ghostty.app`               | about 600 ms            |
| `Xcode.app`                 | 30.1 s                  |

Thirty seconds for one target is three times the whole batch's current inspection deadline. A bundle
verification is unbounded in practice and has to run under its own deadline with an explicit outcome for
hitting it.

### Tightening the chain check buys almost nothing, because the verdict is already spent

The 31 resolvable launch job program binaries on this machine were checked both ways: today's
`posture enrich` verdict, and `codesign --verify --strict -R="anchor apple generic"`. Twelve fail the
requirement. Only three of those are trusted today, and only one of the three is a genuine code finding:

- `BlueBubbles.app/Contents/MacOS/BlueBubbles`, today `signed: Zachary Shames (WPV275H8W7)` and
  trusted, fails the requirement. It is a true positive today's code misses: plain
  `--verify --strict` also fails it, with "a sealed resource is missing or invalid".
- `/Library/PrivilegedHelperTools/com.docker.socket`, today a metadata line and trusted, fails. It is
  unreadable to the operator rather than untrusted, so this is an artifact of handing `codesign` a
  file it cannot read.
- `~/.local/share/fnm/.../bin/happy`, today a metadata line and trusted, fails. It is a symbolic link
  to a `#!/usr/bin/env node` script, so it is not code at all.

The other nine already read untrusted today, because ad-hoc and unsigned are already caught by the text
parse. So the chain check's marginal detection on this machine is one real case, and that case is caught
by `--verify --strict` without any requirement at all.

That reframes the whole disposition. The question is not whether to verify the chain. It is what the
signing verdict is for, given that it already answers "untrusted" for 19 of 45 legitimate jobs and is
the only input that can promote a Notice to a Critical page.

## Disposition 1: signature-chain verification

**Current behavior.** `posture` reads signature metadata with `codesign -dv --verbose=2` and decides
trust from the text: untrusted for "not signed" or "adhoc", otherwise trusted with the first
`Authority=` value as the display name. The name is chosen by the signer. There is no chain check, no
requirement check, and no state for an inspection that could not be performed.

**Does it change: yes, but not the way June proposed.**

### Approaches considered

**A. Build task 4 as written (`--verify --strict` plus `spctl -a -t exec`).** Rejected on measurement.
`spctl --assess -t exec` rejects every non-bundle executable on macOS 26.2, `/bin/echo` included. This
would mark nearly every launch job untrusted and, through the promotion rule, page on every persistence
transition. It is a false-positive machine.

**B. One requirement check (`codesign --verify --strict -R="anchor apple generic"`) as the trust
predicate, with the authority string kept for display only.** The measured behavior is correct:
Apple-signed platform code and Apple applications pass, Developer ID applications pass, ad-hoc and
unsigned fail, and a certificate whose common name imitates Apple cannot satisfy an Apple anchor. Cost
is one extra process per code finding, about 27 ms for a plain binary and up to tens of seconds for a
large bundle.

**C. Stop treating the signing verdict as a trust decision at all.** Keep the `codesign -dv` reading as
a display fact, delete the promotion rule, and let the allowlist and the integrity manifest carry the
page decision, which they already do for every file-integrity category. This is the smallest change and
it removes 19 of 45 false untrusted verdicts at a stroke. It also removes the only mechanism that would
promote a genuinely unsigned new persistence item, so it lowers a finding, which the June plan's binding
rule forbids.

**D. B plus a third outcome.** Verify the requirement, and distinguish three answers instead of two:
satisfied, not satisfied, and could not be inspected. The third covers the unreadable file, the missing
tool, the non-code file and the deadline, all of which today read as "UNSIGNED" and untrusted.

### Recommended design: D

Three states, one extra process, no `spctl`.

**Domain.** Replace the two-valued `CodeTrust` with three values. `posture-domain/src/enrich.rs` is 87
lines and can carry it:

```
Trusted        the requirement was satisfied
Untrusted      the signature is absent, invalid, or does not satisfy the requirement
Uninspectable  the check could not be performed (unreadable, not code, tool missing, deadline)
```

`Enrichment::exit_code` keeps two codes for the command-line interface, mapping `Uninspectable` to 10
alongside `Untrusted`, so the documented `posture enrich` contract does not change. The gate consumes the
three-state value directly, because the enricher now runs in process and no longer communicates through
an exit code.

**Adapter.** Add one method to `EnrichmentInspection`:

```
fn satisfies(&mut self, path: &Path, requirement: &str) -> RequirementOutcome
```

implemented in `posture-adapters/src/codesign.rs` as a single
`codesign --verify --strict -R=<requirement>` run through `run_completed`, classified by exit code: 0 is
satisfied, 1 and 3 are not satisfied, everything else including a deadline is uninspectable. The existing
`signing()` call stays, because its text is still the display fact the page shows.

**Configuration.** The requirement string is one value in the posture configuration, defaulting to
`anchor apple generic`, visible and uncommented at its default per the repository's configuration
convention. It is configuration and not a constant because it is the one knob that decides how much of a
machine's own software reads as untrusted, and because posture ships to other people whose machines are
not this one.

**Deadline.** The requirement check gets its own short deadline, proposed at 3 seconds, separate from
the batch's shared 10 seconds. Hitting it yields `Uninspectable`, never `Untrusted`. Without this, the
measured 30 second `Xcode.app` verification would consume the entire batch budget and turn every
subsequent row critical.

**What promotes.** `Untrusted` promotes Notice to Critical, as today. `Uninspectable` does not promote;
it is rendered in the fact text and the finding stays at its detector's tier. This is the assumption most
likely to be contested and it is listed as such below.

### Behaviors to pin, test first

1. Text claiming an Apple authority whose requirement check is not satisfied reads `Untrusted`, and the
   fact still shows the claimed authority, marked as a claim.
1. An Apple platform binary reads `Trusted`.
1. A Developer ID application reads `Trusted` under `anchor apple generic` and `Untrusted` under
   `anchor apple`, driven by the configured requirement and nothing else.
1. An ad-hoc signature reads `Untrusted` without a requirement check being spawned, because the text
   parse already answers.
1. An absent signature reads `Untrusted`.
1. A file the process cannot read reads `Uninspectable`, and the fact says so rather than saying
   "UNSIGNED".
1. A requirement check that hits its deadline reads `Uninspectable`.
1. `Uninspectable` does not promote a Notice finding to Critical.
1. The command-line exit code is 10 for both `Untrusted` and `Uninspectable`, and 0 for `Trusted`.

### Failure modes and security

The requirement string is trusted input from the operator's own configuration file. It reaches
`codesign` as a single argument through the existing process spawn, which takes an argument vector and
no shell, so there is no injection surface. A malformed requirement makes `codesign` exit nonzero with a
parse complaint, which the exit-code classifier would read as "not satisfied" and would mark every
binary untrusted. That is the wrong direction for a configuration error, so the classifier treats a
`codesign` exit code it does not recognize as `Uninspectable`, and the run writes one diagnostic naming
the requirement.

The check is a read-only inspection of a file the operator can already read. It grants no authority and
performs no remediation.

## Disposition 2: interpreter-payload assessment

**Current behavior.** An interpreter-fronted launch job is trusted unconditionally. The resolver finds
the first of `ProgramArguments.1` through `.5` that begins with `/` and is an existing file, renders
"runs script `<name>` via `<interpreter>`, payload unverified", and returns trusted. Quarantine on the
payload is appended as text and changes nothing. If no argument resolves, the text says "runs
`<interpreter>` (interpreter), payload unverified" and the verdict is still trusted. Twelve of this
machine's 45 launch jobs take this path and seven of the twelve carry a payload the resolver cannot
reach.

**Does it change: yes.**

### Approaches considered

**A. Build task 5 as written.** Promote when the payload is quarantined, or writable and not owned by
the caller, or modified within the last day. Rejected on three grounds. The ownership condition
(`-w && ! -O` in the original shell) requires a file writable by the caller but owned by somebody else,
which is precisely not where an attacker holding the operator's privileges writes; a script planted in
the operator's home is owned by the operator and would not promote. The modification-time condition
promotes every legitimately updated script, which on this machine means every managed script after every
`chezmoi apply`. And the rule says nothing at all about the seven unresolvable forms, which are the
majority.

**B. Stop vouching for what was not verified.** Make "payload unverified" its own outcome that does not
claim trust. No new inspection, no heuristic, one verdict change. This removes the soft path outright.
Its cost is that seven live jobs, the three `sh -c` nix daemons and the three `python -m` hermes
gateways and the relative-path node job, would stop reading trusted, so a persistence transition for any
of them would promote to Critical.

**C. Ask the payload the question the rest of the pipeline already asks.** A resolved payload is trusted
when the governing known-good manifest vouches for it as it stands, which is the same predicate
`posture-adapters/src/known_good_read.rs` already implements for the integrity arm, and untrusted
otherwise. Measured: all five resolvable payloads on this machine are manifested pipeline scripts, so on
this machine the rule produces no false positives at all. Its weakness is that a payload outside the
manifested trees, a perfectly ordinary user script, reads untrusted.

**D. Parse the `sh -c` command string for a path.** Rejected. A shell command string needs a shell
parser to read correctly, and a wrong parse is worse than an honest refusal. The unresolvable forms
should be named, not guessed at.

### Recommended design: B as the floor, C where a payload resolves, and an explicit unresolved state

Three payload outcomes, mirroring the three code outcomes so the page speaks one language:

```
Trusted        the payload resolved and the governing manifest vouches for it as it stands
Untrusted      the payload resolved and the manifest does not vouch, or it is quarantined,
               or it is group- or other-writable
Unresolved     the payload could not be located (an inline command string, a module name,
               a relative path, a missing file)
```

`Unresolved` is reported as `Uninspectable` at the enrichment boundary, so the two dispositions share one
three-state vocabulary and one promotion rule.

**Boundaries.** The manifest question belongs to the application layer, which already holds both the
enrichment use case and the manifest reader. No new adapter is needed for ownership and mode:
`posture-adapters/src/metadata.rs` already reads them in process through `symlink_metadata`, and
`quarantined()` already runs on the resolved script. The only genuinely new input is the manifest
verdict, which the alert path already computes as the `vouches` collaborator.

**The interpreter binary itself is not checked.** `/opt/homebrew/bin/bash` fails the chain requirement,
and checking it would flip the five resolvable osquery jobs to untrusted for a reason that says nothing
about the job. The composition of interpreter plus payload is what the allowlist and the manifest vouch
for. This is an assumption and its alternative is listed below.

### Behaviors to pin, test first

1. An interpreter program with an absolute, existing, manifest-vouched payload reads `Trusted` and the
   fact names the payload.
1. The same payload with one byte changed reads `Untrusted`.
1. A resolved payload that is group-writable or other-writable reads `Untrusted` even when the manifest
   vouches for it, because the manifest records a moment and a write bit is a future.
1. A quarantined resolved payload reads `Untrusted`.
1. `sh -c '<command string>'` reads `Unresolved`, and the fact says the payload is an inline command
   rather than saying a script was checked.
1. `python -m <module>` reads `Unresolved`.
1. A relative payload argument reads `Unresolved`, and the fact names the limit: no working-directory
   resolution is performed.
1. A resolved payload owned by the operator and freshly modified still reads `Trusted` when the manifest
   vouches. This test exists to pin the refusal of June's modification-time condition, so that the
   condition cannot be reintroduced without turning a test red.
1. `Unresolved` does not promote a Notice finding to Critical.

### Failure modes and security

The resolver walks at most five `ProgramArguments` indices and each walk is a `plutil` process, so a
plist with many arguments cannot make it walk further. Every read is of a file the operator can already
read. An unreadable payload reads `Unresolved`, not `Trusted`.

The residual hole is explicit and should be recorded as accepted rather than closed: an inline command
string is not assessed, so an attacker who writes `sh -c 'curl ... | sh'` into a launch agent gets an
`Unresolved` payload rather than an `Untrusted` one. Under the recommended promotion rule that is
quieter than untrusted. Whether that is acceptable is an open question below, and it is the strongest
argument for promoting `Uninspectable` after all.

## Disposition 3: per-run grouping of repeated findings about one subject

**Current behavior.** `judge_batch.rs` enriches, gates and renders every row in the batch
independently. Nothing folds rows that describe one subject. Measured: eleven rows about
`~/.local/bin/herdr-reviewr` in one 300 second window, which fills all eight blocks of a page; 26 paths
that emit two rows per write because `managed_bin` contains `pipeline_integrity`; and one shared 10
second inspection deadline that every duplicate row draws down.

**Does it change: yes, and it is now the only remaining lever on the duplicate-alert bug, because the
ingestion model's D2 is closed.**

### Approaches considered

**A. Build task 3 as written.** Keep a seen-set keyed on detector and path for the evented detectors,
skip a path already emitted this run, keeping the highest-signal action ranked CREATED then UPDATED then
DELETED. Two problems. One of its two named detectors, `es_launchd_writes`, is now log-only, so half the
rule is dead code. And the action ranking fights the page's own triage: if a file was created then
deleted inside the window, a block labeled CREATED sits next to a triage line reporting the file as
absent.

**B. Narrow the osquery watched trees so `managed_bin` excludes the two pipeline subtrees.** This is
rung one of the ladder and it removes a real, measured doubling for 26 paths without any code. It does
not touch the event-storm multiplier, which is the larger factor (eleven rows for one path), and osquery
file path patterns have no exclusion form, so the narrowing means enumerating `~/.local/libexec`'s other
children by name and keeping that list in step with the directory.

**C. Fold in the domain, before enrichment.** One pure domain function over the batch's ordered
`(detector, target_path)` pairs returns, for each index, either keep or folded-into.
`judge_batch` applies it before the signing map, so N duplicate rows cost one inspection, one
manifest read and one content digest rather than N of each.

### Recommended design: C, with B recorded as a separate, smaller question

**Where it lives.** A new module in `posture-domain`, not inside `judge_batch.rs`, which is already 278
lines against a 300 line target and a 500 line cap, and whose own documentation says it owns ordering
rather than policy. What counts as one subject is policy.

**The fold key** is the detector plus the non-empty `target_path`, and deliberately not the category. Two
rows for one file under `managed_bin` and `pipeline_integrity` describe one subject, and
`manifest_for` already picks the governing manifest from the path rather than the category, so the two
rows cannot reach different verdicts.

**Which detectors fold.** `FileEventsRecent` only. `EsLaunchdWrites` is omitted with the reason written
down: the gate sends it to log-only, so folding it would be code no batch can reach. Re-enabling that
detector reopens this choice.

**Position and ordering.** The surviving row keeps the first arrival position, because `rows()` preserves
arrival order and the page reads as a timeline.

**The action field** lists the distinct verbs in arrival order, for example `CREATED, UPDATED, DELETED`,
rather than one ranked winner. This is a deliberate departure from the June plan. The page also shows
the triage's current on-disk state, and the honest pairing of "three events, now absent" beats "CREATED,
now absent".

**Severity** of the folded set is the maximum over its members, never the first or the last, so folding
cannot lower a finding.

**Counting.** `render_page`'s count becomes subjects rather than rows, and a folded block names its own
event count. The page title's number is then the number of things that happened, which is what the June
plan's "one file, one alert" was asking for.

**The digest side** folds too: a folded set spools one digest record rather than N. Today N rows for one
path spool N lines into the daily digest.

### The no-false-negative argument

Folding collapses only rows sharing the same detector and the same non-empty target path inside one
batch. Two different paths never fold. A genuinely later change to the same file arrives in a later
batch, because the cursor advanced, and pages again. The kept severity is the maximum of the set. The
kept action names every verb observed rather than discarding any. Nothing is dropped and nothing is made
quieter, which satisfies the June plan's binding rule.

### Behaviors to pin, test first

1. Three `file_events_recent` rows for one `target_path` in one batch produce one page block.
1. That block's action field names all three verbs in arrival order.
1. Two rows for two different paths produce two blocks.
1. Two rows for one path under different categories produce one block.
1. The folded block appears at the first row's position in the page.
1. A folded set whose members would gate differently keeps the loudest outcome.
1. The same path in a later batch pages again.
1. Nine rows about one path plus three about three other paths render four blocks, not eight blocks and
   a truncation marker.
1. A folded set spools one digest record.
1. The signing inspection runs once per folded subject, not once per row. This is the test that pins the
   ordering: folding must happen before the enrichment map or the saving does not exist.
1. A row with an empty `target_path` never folds with another.

### Failure modes

Grouping is pure and cannot fail at run time. Its risk is a fold key that is too coarse. The stated key
is exact string equality on the target path, with no normalization: no symbolic link resolution, no case
folding, no trailing-slash handling. That is deliberate, because a normalization is a second opinion
about identity that can differ from osquery's, and two spellings of one path folding into one block is a
quieter page than the rule promises. Two spellings producing two blocks is merely the behavior of today.

## Out of scope

- Task 6 of the June plan, root-owning the launch allowlist. The allowlist is already at
  `~/.config/osquery/page-launchd-allowlist.txt`, mode 0600, covered by the root-owned pipeline-integrity
  manifest, and `allowlist_verdict` already refuses to honor a list the manifest cannot vouch for. The
  file's own header records this. Nothing in this document changes it.
- Task 7 and the ingestion model's D2. Closed by the July repurposing of the `launch_agents` and
  `launch_daemons` watched trees, as argued above. Reopening it would remove integrity coverage.
- The ingestion model's D3 and D5, process attribution from `es_launchd_writes` and the canonical-record
  normalization. D3 depends on a detector that is log-only by decision. D5 is a refactor of a Bash
  pass-one that no longer exists.
- The shared inspection deadline. Recorded as a measured observation, not proposed for change.
- The overlapping watched trees. Recorded, and proposed as its own smaller question rather than folded
  into the grouping change.
- Changing the promotion rule's existence. Approach C of disposition 1 would delete it. The
  recommendation keeps it, and the question of whether it should exist at all is an open question rather
  than a decision made here.
- Every other open item in the pull request #24 recovery section: the Hermes-owned investigation,
  the approval interface, the FleetDM and beaconing and Wazuh research, kernel-extension
  install-state monitoring, and off-host machine-death detection.
- No code. This document is the deliverable.

## Assumptions made in the operator's place

Each of these is a choice this document made because the operator was asleep. Each names its
alternative.

1. **The requirement is `anchor apple generic`.** Alternative: `anchor apple`, which admits only
   Apple-signed code. Measured consequence of the alternative: Ghostty and every other Developer ID
   application flips to untrusted, so the stricter anchor is a false-positive generator on a machine that
   runs third-party software.
1. **`Uninspectable` does not promote a Notice finding to Critical.** Alternative: it does, which is
   fail-closed and matches how the code accidentally behaves today, since a failed inspection currently
   reads "UNSIGNED" and promotes. Measured consequence of the alternative: `com.tailscale.tailscaled`,
   `com.docker.socket` and every inline `sh -c` payload promote on every transition. Measured consequence
   of the assumption: an attacker's `sh -c` payload is quieter than an ad-hoc signed binary.
1. **The command-line exit code stays two-valued**, with `Uninspectable` mapping to 10. Alternative: a
   third exit code. The assumption keeps the documented `posture enrich` contract, at the cost of a
   caller outside this process being unable to tell an uninspectable finding from an untrusted one.
1. **The interpreter binary itself is not trust-checked**, only its payload. Alternative: check both,
   which would flip the five resolvable osquery jobs to untrusted because `/opt/homebrew/bin/bash` fails
   the chain requirement.
1. **A resolved payload is trusted only when the known-good manifest vouches for it.** Alternative: a
   standalone heuristic on ownership, mode and quarantine with no manifest question, which is closer to
   the June plan and does not treat an ordinary unmanifested user script as untrusted.
1. **June's modification-time condition is refused, and a test pins the refusal.** Alternative: implement
   it. Measured consequence of the alternative: every managed script promotes after every apply.
1. **A folded block lists every observed verb** rather than one ranked winner. Alternative: the June
   plan's CREATED then UPDATED then DELETED ranking, which is one word in the field but can contradict
   the triage line beside it.
1. **The fold key ignores the watched-tree category**, so the two rows a pipeline file emits fold into
   one. Alternative: fold per category, which keeps the doubling and leaves it for the configuration
   change instead.
1. **`es_launchd_writes` is left out of the grouping rule** rather than included for future-proofing.
   Alternative: include it, at the cost of code no batch can reach while the detector is log-only.
1. **The requirement string is configuration, not a constant.** Alternative: a constant, which is one
   fewer knob but decides for every machine that installs posture how much of its own software reads as
   untrusted.
1. **These three dispositions are one change set.** Alternative: three separate pull requests. They
   interact: disposition 1 adds a process per code finding against a deadline that disposition 3
   relieves, and dispositions 1 and 2 share the three-state vocabulary. The recommendation is one
   design and a pull-request ladder of three, sequenced grouping first so the inspection load falls
   before it rises.

## Open questions for the operator

1. **What is the signing verdict for?** It answers untrusted for 19 of 45 legitimate launch jobs on this
   machine, and it is the only input that can promote a Notice to a Critical page. Four of the nine
   allowlisted labels page anyway because of it. Is the verdict a page trigger, or is it a display fact
   with the allowlist and the manifest deciding? Every other question here sits downstream of this one.
1. **Does an uninspectable finding promote?** Fail-closed promotes and adds noise from unreadable
   root-owned plists. Fail-open leaves an inline `sh -c` payload quieter than an ad-hoc signed binary.
1. **Is an unmanifested ordinary user script an untrusted payload?** On this machine every resolvable
   payload is manifested, so the rule is free here. On a machine with user scripts in launch agents it is
   not.
1. **Should the allowlist suppress a promotion?** Today it cannot: the promotion happens before the
   allowlist is consulted and outranks it. Making the allowlist win would close the four measured false
   pages, and would also let an allowlist entry quiet a genuinely unsigned program.
1. **Is `anchor apple generic` the right bar, or should Developer ID be a separate, lesser tier** than
   Apple platform code, rather than equal to it?
1. **Should the overlapping watched trees be narrowed?** It is a separate, smaller change than grouping
   and it removes a measured doubling for 26 paths. It also means enumerating `~/.local/libexec`'s other
   children by name, since osquery file path patterns have no exclusion form.
1. **Is the live spoof drill worth an administrator's time?** Signing with a self-signed certificate
   whose common name imitates Apple needs `security add-trusted-cert`. Without it, the claim that
   today's parser trusts a spoofed name rests on reading a pure function rather than on a demonstration.
1. **One change set or three pull requests?** The recommendation is one design and three pull requests,
   grouping first.

## How to check the claims in this document

Every measurement above is reproducible without an apply, and none of it writes to a monitored path.

```bash
# Disposition 1: the tool behavior
spctl --assess -t exec /bin/echo; echo $?                 # 3, "does not seem to be an app"
codesign --verify --strict /opt/homebrew/bin/jq; echo $?   # 0, so --verify alone is not trust
codesign --verify --strict -R="anchor apple generic" /opt/homebrew/bin/jq; echo $?   # 3
codesign --verify --strict -R="anchor apple generic" /bin/echo; echo $?              # 0
time codesign --verify --strict -R="anchor apple generic" /Applications/Xcode.app    # about 30 s

# Disposition 2: the live census and the interpreter forms
for p in "$HOME"/Library/LaunchAgents/*.plist /Library/LaunchAgents/*.plist \
         /Library/LaunchDaemons/*.plist; do
  [ -f "$p" ] || continue
  out=$(posture enrich "$p" 2>&1); rc=$?
  printf '%-52s rc=%-3s %s\n' "$(basename "$p")" "$rc" "$out"
done

# Disposition 3: the duplicate rows and the overlapping trees
grep -h '"name":"file_events_recent"' "$HOME/.local/log/osquery/osqueryd.results.log" \
  | jq -r '[.columns.target_path,.columns.time]|@tsv' \
  | awk -F'\t' '{k=$1"\x1f"int($2/300); c[k]++; if(c[k]>m[$1]) m[$1]=c[k]}
                END{for(p in m) printf "%d\t%s\n", m[p], p}' | sort -rn | head

grep -h '"name":"file_events_recent"' "$HOME/.local/log/osquery/osqueryd.results.log" \
  | jq -r '[.columns.target_path,.columns.category]|@tsv' | sort -u \
  | awk -F'\t' '{n[$1]++} END{k=0; for(p in n) if(n[p]>1) k++; print k}'   # 26

# The D2 gating question, now moot
osqueryi "SELECT count(*) FROM launchd WHERE path LIKE '$HOME/Library/LaunchAgents/%';"
ls -1 "$HOME"/Library/LaunchAgents/*.plist | wc -l
```

The source claims are in `posture/crates/posture-domain/src/enrich.rs` (the text parser and the
interpreter list), `posture/crates/posture-application/src/enrich.rs` (the two paths and the trusted
interpreter branch), `posture/crates/posture-domain/src/gate.rs` (the promotion rule and the log-only
`es_launchd_writes`), `posture/crates/posture-adapters/src/judge_batch.rs` (the ungrouped batch),
`posture/crates/posture-domain/src/page.rs` (`BLOCK_LIMIT` and `BODY_LIMIT`), and
`posture/crates/posture/src/alert.rs` (the one shared inspection deadline and the comment that describes
a different one).
