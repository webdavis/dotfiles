# posture: the plan for the Rust port

Written 2026-09-05 against `origin/main` at `b5412089`. It moves the 368 behaviors inventoried in
`docs/superpowers/specs/2026-09-05-posture-behavioral-specification.md` out of the twenty-one shell
files under `dot_local/libexec/osquery/` and into one Rust workspace, one reviewable pull request at a
time, with every bash script running until its replacement is verified live and its plist repointed.
It follows the shape of `docs/superpowers/plans/2026-09-05-pns-refactor-plan.md` and the standing
rules of the pns charter (the engineering priorities, the Rust-native SOLID rules, the 300-line target
and 500-line cap for every `.rs` file with tests included), which this plan does not restate. It
names, per step, which statements move, which tests carry them or are written first, which deployed
surface changes, what the operator alone must do, and what every resulting file is projected to
measure. Every crate, module, subcommand, path and file name below is proposed, confirm before
creating.

## 1. Where the ladder starts

Nothing of posture exists. There is no crate, no binary, no workspace and no test baseline. The
inputs, all read in full for this plan:

- the specification and its 368 statements, 169 pinned and 199 UNPINNED;
- the twenty-one shell files (7,263 lines, 3,326 of code, 3,622 of comment), the two chezmoi
  runners (`run_after_05`, 386 lines, and `run_after_50`, 58), the seven plists and their seven
  loaders, and the 62 inline jq programs and 7 SQL statement groups a port replaces (about 3,800
  lines of code in all);
- the eleven bats files and one bashunit file, 186 test cases, 3,886 lines, and the five orphan
  fixture libraries under `test/fixtures/` (2,051 lines, 127 functions) that are the only record of
  what the five untested tools once asserted;
- the pns crate's workspace (`dot_local/share/pns/Cargo.toml`, five members, the dependency edges
  declared in the member manifests) and its builder
  (`.chezmoiscripts/run_onchange_after_58-build-pns-engine.sh.tmpl`);
- uu's lane adapter shape (`dot_local/share/uu/src/lanes.rs:39-59`), its spawn seam
  (`src/lanes/spawn.rs:59`) and its two clients of pns (`src/alert.rs`, `src/delivery.rs:11`);
- the locked decisions in the specification's section 4 and the delivery decision in its section 5.

Three pull requests in the pns program gate posture's producer cutovers and nothing else: PR 7.3
(`pns submit --json` with a result envelope), PR 11.4 (the write-ahead delivery ledger the daemon
drains), and the priority-route pull request the specification's section 5.5 sizes. Steps 1 to 4 and
step 7 below do not wait on them; steps 5 and 6 do.

## 2. The target workspace

### 2.1 Location and crates

The crate deploys as source to `~/.local/share/posture` from `dot_local/share/posture/`, the same
arrangement pns and uu use, so a path dependency on `../pns/crates/pns-protocol` resolves in both the
repository checkout and the deployed tree (the reasoning is uu's, `dot_local/share/uu/Cargo.toml:14-19`).
The root manifest is a virtual workspace with `default-members = ["crates/posture-cli"]`, so the
builder's `cargo build --release --locked --quiet --bin posture --manifest-path
dot_local/share/posture/Cargo.toml` resolves from day one:

```
crates/posture-domain        pure policy, no dependencies
crates/posture-application   use cases and the ports they own
crates/posture-adapters      everything concrete, by capability
crates/posture-cli           argv decoding, composition, exit codes; the `posture` binary
```

The dependency direction, declared in the member manifests and nowhere else so the compiler
enforces it: `posture-application` depends on `posture-domain`; `posture-adapters` on both plus
`pns-protocol`; `posture-cli` on all of them. No member depends outward.

There is no `posture-protocol` crate, deliberately. Posture defines no wire contract of its own: it
CONSUMES osquery's result log, chezmoi's manifest runner output, uu's upgrade record and the
controls file rendered from `.chezmoidata/macos_posture_controls.yaml`, and it PRODUCES to pns
through pns's own protocol crate. Each consumed format is a codec inside `posture-adapters`, next to
the reader that needs it. The one file two posture processes share, the digest spool, is an internal
protocol, not an external one.

### 2.2 `posture-domain`

Total functions of their arguments: no filesystem, clock, JSON, environment, process or privilege.
Each module below is a stage or verdict the bash already isolates, which is the
"guide, not a rule" the S9 decomposition design left for this port. Module names are proposed.

- `finding`: `Finding {query, action, columns, enrich_path}`, the 25-name `Detector` enum, pack
  stripping, the baseline discard and its three exemptions, the enrich-path rule, the renameio churn
  rule (S023 to S034, S360).
- `severity`: `Severity`, `route_severity`, `protection_off` (S035 to S040).
- `gate`: `Outcome {Page, Digest, LogOnly}`, the per-detector arms, the promotion rule, the
  fail-safe severity read (S042 to S058, S061 to S063, S066, S067).
- `allowlist`: `Entry`, the label grammar, the `~/` expansion, and
  `Verdict {Suppress, NotAllowlisted, ReusedLabel}` over an already-read file and an already-answered
  vouch (S069 to S077, S301 to S306).
- `known_good`: the four-column `Tuple`, the line grammar, `TrackedSet` (the four patterns and the
  bin arm's inverted fail-safe), `manifest_for`, the tuple match with its empty-column rule (S085 to
  S096, S234).
- `integrity`: the page-or-silent decision over a `DeployedState` reading (kind, hash, mode, uid)
  and a manifest answer, the DELETED rule, the atomic-rename shape (S079 to S084, S097).
- `triage`: the correlation facts from a manifest answer, an on-disk reading and a parsed upgrade
  record; the record grammar and its window, row and count caps (S102 to S115).
- `page`: the header, field, next-step and cap vocabulary, the sanitize chokepoint, the eight-block
  and 1900-character caps, the shell quoting of paths (S124 to S133).
- `digest`: the spool line's six derived fields, the grouper, the four caps, the codepoint cap, the
  torn-line coercion (S117, S120, S286 to S294).
- `canary`: `newest_canary_timestamp` over parsed rows, the range bound, two-sided freshness, the
  three unhealthy cases and their wording (S193 to S204, S211).
- `watchdog`: the five probes' decisions over readings, the crash-loop streak, the fingerprint
  page-once, the seven-word divergence vocabulary, the 99 clamp, the page body (S207 to S226).
- `audit`: `pipeline_audit_scan` as a function of manifest lines and per-path readings: the seven
  kinds, six refusals, three bounds (S227 to S240).
- `controls`: `Control`, the eight `Reader`s and their domains, the eleven validations, whole-file
  refusal (S254 to S256; spec section 3.2).
- `poll`: the probe classifiers (`classify_probe`, `classify_pgrep`, `fdesetup`, `autologin`), the
  baseline trust rules, the per-member gap set, first observation versus transition, `sanitize_span`
  (S246 to S249, S251, S257 to S264, S266, S267).
- `funnel`: the `AllowFunnel` reading over parsed JSON, corrupt versus absent, the transition table,
  the exposure render (S268, S269, S272, S273, S275, S276).
- `drift`: `file_verdict`, `directory_verdict`, `restart_verdict`, the token precedence, the closed
  label vocabulary (S311 to S318, S320).
- `converge_policy`: the named desired-file list, the refusal rules over a listing (unlisted file,
  symlink, unreadable), the restart-evidence rule (parent pid changed and stable across the settle
  window), the bound defaults (S326, S333, S334, S336, S345 to S347).
- `cursor`: `Cursor {inode, offset}` parsing, the reset rules, the snapshot cut at the last newline,
  the occurrence id (S007 to S011, S013, S014, S016).
- `enrich`: the signing classification over a parsed `codesign` report, the interpreter list, the
  bundle suffixes, the fact strings (S134 to S141).

Two rules the charter's SOLID section makes concrete here. `Detector` is a closed enum and the gate
matches on it, so adding a detector is one variant plus one arm and no string comparison anywhere
below the normalizer. And every reading the domain judges arrives as a typed value with its own
"could not read" state (`Reading::Unreadable`), never as an empty string, so the fail-safe directions
of S042, S089, S246 and S314 are enum arms rather than emptiness checks.

### 2.3 `posture-application`

One use case per entry point, each a concrete struct ordering the calls the bash `main` performs
today, over ports the use case declares. Names are proposed.

- `JudgeResults` replaces `results-alerter.sh`'s `main` over `ResultsLog` (size, inode, bounded
  read), `CursorStore`, `SingleInstanceLock`, `Allowlist`, `KnownGood`, `DeployedState`, `Enricher`,
  `UpgradeRecord`, `Spool`, `AlertSink` and `Clock`.
- `BuildDigest` replaces `digest.sh`'s `main` over `Spool` (claim, restore, rotate, sweep) and
  `AlertSink`.
- `Heartbeat` replaces `heartbeat.sh`'s `main` over `SnapshotsLog`, `Clock` and `AlertSink`.
- `Watchdog` replaces `uptime-watchdog.sh` over `SnapshotsLog`, `Clock`, `ProcessTable`,
  `LaunchdState`, `GatewayHealth`, `KnownGood`, `DeployedState`, `StateFile` and `AlertSink`.
- `Poll` replaces `firewall-gatekeeper-monitor.sh` over `Osqueryi`, `ProbeRunner`, `ControlsFile`,
  `StateFile`, `GapMarker` and `AlertSink`.
- `Funnel` replaces `tailscale-monitor.sh` over `TailscaleStatus`, `StateFile`, `GapMarker` and
  `AlertSink`.
- `Converge` replaces `osquery-converge.sh` over `Staging` (listing, private copy), `LiveTree` (kind,
  attributes, content equality), `PrivilegedInstall`, `Osqueryctl`, `ProcessTable`, `Sleeper` and
  `Clock`.
- `CurateAllowlist` replaces `allowlist.sh` over `LaunchdTable`, `SourceAllowlist`, `Publisher`
  (apply one target, refresh the manifests) and `WriteLock`.
- `Enrich` replaces `enrich-finding.sh` over `Codesign`, `PlistReader`, `Quarantine` and `FileKind`.

`AlertSink` is the one port every producer shares. Its contract is the specification's S144 as a
type: `fn submit(&self, alert: &Alert) -> Accepted | NotAccepted(reason)`, where `Accepted` means the
engine has durably recorded the event and owns its delivery from here. Notify-before-persist (SI-4)
is then one line in each use case: advance state only on `Accepted`.

Ports are narrow but cohesive: `Spool` is one trait with claim, restore, rotate and sweep, because
those four operations form one transactional protocol over one file family; splitting them would
let a test double honour one and not the others.

### 2.4 `posture-adapters`, by capability

- `results_log`: `ResultsLog` and `SnapshotsLog`, size and inode by `stat`, a bounded read of a byte
  window, a per-line parse with a torn tail. Replaces `wc -c`, `ls -i`, `tail -c`, `head -c` and the
  jq `fromjson?` lines.
- `state_files`: `CursorStore`, `StateFile`, `GapMarker` and `Spool`, publishing by rename, claiming
  by rename, mode 600 in a 700 parent, whole-file JSON validation. Replaces the `mktemp` plus `mv`
  idioms and the `jq -s` one-object reads.
- `locks`: `SingleInstanceLock` (non-blocking) and `WriteLock` (blocking) over `flock` on the same
  lock paths the bash uses. Replaces `/usr/bin/lockf`.
- `known_good_file`: `KnownGood`, reading a manifest under the root-owned, not-writable-by-others
  trust check and parsing tuples. Replaces the `pipeline-verdict.sh` readers.
- `deployed_state`: `DeployedState`, the `lstat` kind, the sha256 of the current bytes, the
  four-digit mode, the uid and the inode change time, refusing symlinks before hashing. Replaces
  `_pipeline_deployed_state_is_known_good` and `probe_attributes`.
- `allowlist_file`: `Allowlist` and `SourceAllowlist`, the NDJSON codec with comments preserved and
  the one-tuple-per-line rule. Replaces the file reads in `allowlist-verdict.sh` and `allowlist.sh`.
- `controls_file`: `ControlsFile`, the JSON read the domain validates. Replaces `load_controls`.
- `upgrade_record`: `UpgradeRecord`, the regular-file check, one bounded read, the TSV split by
  expansion. Replaces `file-integrity-triage.sh:236-333`.
- `launchd`: `LaunchdState` (`launchctl print`) and `LaunchdTable` (`osqueryi` over the `launchd`
  table for the writer). Replaces the watchdog's probe 2 and `allow_label`'s capture.
- `osqueryi`: `Osqueryi`, one bounded `osqueryi --json` query. Replaces the poller's combined query.
- `probes`: `ProbeRunner` for `fdesetup`, `csrutil`, `sysadminctl`, `defaults`, `pgrep`, `plutil`
  and `readlink`, each by absolute path under one deadline. Replaces `run_bounded` and the readers.
- `tailscale`: `TailscaleStatus`, `tailscale funnel status --json` under a deadline. Replaces the
  funnel monitor's read.
- `codesign`: `Codesign`, `PlistReader`, `Quarantine` and `FileKind` over `codesign -dv`, `plutil`,
  `xattr` and `file`. Replaces `enrich-finding.sh`'s spawns.
- `process`: `ProcessTable`, `pgrep`, the ppid-1 parent, `kill(pid, 0)`. Replaces `pgrep -fq` and
  `daemon_parent_pid`.
- `gateway`: `GatewayHealth`, one unsigned GET of the priority route under a deadline through
  `ureq`. Replaces the watchdog's probe 3.
- `pns_producer`: `AlertSink`, spawning the deployed `pns` binary with `submit --json`, encoding the
  request with `pns-protocol`, decoding the result, mapping the durable bit, with the child bounded.
  Replaces `alert-dispatch.sh` (spec section 5).
- `last_resort_banner`: the one `osascript` spawn posture keeps, raised only when the engine could
  not take the event. Replaces the alarm role of `_osquery_notify_local_durable`.
- `privileged`: `PrivilegedInstall` (`/usr/bin/sudo -n /usr/bin/install -o root -g wheel -m ...` out
  of the private copy) and `Osqueryctl` (resolved and trust-checked; `config-check`, `stop`,
  `start`). Replaces the converge's privileged calls.
- `staging`: `Staging`, the component-by-component symlink walk, the materialized listing, the
  per-run 0700 private copy removed on exit. Replaces `assert_no_symlink_component`, the `find`
  listing and the private stage.
- `publisher`: `Publisher`, `chezmoi source-path`, `chezmoi apply <one target>`, and the manifest
  runner by its source-relative path. Replaces `publish_allowlist`.
- `clock`: `Clock` and `Sleeper`, wall time, `gmtime_r`, the 0.25 s tick. Replaces `date` and
  `sleep`.

A spawned child runs under an explicit deadline with process-group termination, the way uu's
`CommandRunner::run_with_deadline` and pns's `run_bounded` already do, and every spawn goes through
one `CommandRunner` trait with a scripted double, so no adapter test runs a real `sudo`, `osqueryctl`,
`osqueryi`, `codesign`, `tailscale` or `pns`.

### 2.5 `posture-cli`

`main.rs` decodes argv, builds the adapters in one `compose.rs`, hands them to the use case the
subcommand names, and translates the outcome to an exit code; it is projected under 120 lines and
holds no policy. The subcommands, one per entry point, are the specification's section 1 table:
`alert`, `poll`, `funnel`, `watchdog`, `digest`, `heartbeat`, `converge`, `allowlist add|deny|list`,
`enrich <path>`. An unknown word or a missing verb prints usage to stderr and exits 2 (S298, S341).
The exit codes per subcommand are the specification's section 2, unchanged, because each caller
depends on them.

### 2.6 What the binary may spawn

Stated up front, as pns does in its manifest (`dot_local/share/pns/Cargo.toml:1-30`), because the
roster is a security property of a security tool: `osqueryi`, `osqueryctl` (trust-checked), `sudo`
and `install` (by absolute path, converge only), `launchctl`, `pgrep`, `fdesetup`, `csrutil`,
`sysadminctl`, `defaults`, `plutil`, `readlink`, `xattr`, `file`, `codesign`, `tailscale`, `chezmoi`
(the allowlist writer only), the manifest runner (a chezmoi source script, writer only), the deployed
`pns` binary, and `osascript` for the last-resort banner alone. Nothing else. `shasum`, `openssl`,
`sqlite3`, `curl`, `jq`, `alerter`, `gtimeout`, `hostname`, `date` and `lockf` all leave: sha2,
`ureq`, `serde_json`, libc's `flock`, `gethostname` and `gmtime_r`, and a Rust deadline replace them.

### 2.7 What stays a file, and why

Classified by the readers and writers outside one process, as the pns plan's section 8 does. Every
file keeps its current path and shape, so a cutover replays nothing and pages nothing new:

- The cursor, the watchdog state, the poller baseline, the funnel baseline and the two gap markers
  are single-record files published by rename and read whole. They stay files under their current
  names in `~/.local/state/`, because the trust rules on them (mode 600, exactly one object, in-domain
  scalars, same `expect` and `target`) are part of the behavior (S209, S260, S261, S272), and a
  cutover that renamed them would be a migration with a first-observation storm to avoid. A later
  rename to a `posture/` state directory is its own step with an import, not part of this port.
- The digest spool stays a file protocol because two processes share it by rename (S282, S284).
- The two lock files stay, because `flock` on the same path is what serializes a bash run still in
  flight against the first Rust run at cutover.
- The SQLite queue has no successor in posture (spec section 6, D1 to D4).
- The manifests, the controls file, the allowlist and the desired tree are external contracts and
  stay exactly as they are.

No SQLite dependency enters posture. Under the recommended delivery every multi-record store belongs
to pns.

### 2.8 Dependencies

`serde_json` (the result log, the controls file, the allowlist, the funnel status, the spool),
`sha2` (file hashes and the fingerprint), `ureq` with rustls (the one GET), `libc` (`flock`,
`gethostname`, `gmtime_r`, `kill`, `O_NOFOLLOW`), and `pns-protocol` by path. No `toml`: posture has
no configuration file today, every path is derived from `$HOME`, and the port keeps that. No `hmac`:
posture signs nothing.

## 3. The deployment surface

The items here are named because CLAUDE.md's "Moving a script is never just a move" applies literally
to this port, and the specification's section 1 lists the references.

### 3.1 The build step

`.chezmoiscripts/run_onchange_after_03-build-posture.sh.tmpl` mirrors the pns builder
(`run_onchange_after_58-build-pns-engine.sh.tmpl`): it hashes every `*.rs`, the manifests and the
lock so any source change re-fires it, defers with a retry marker when `~/.cargo/bin/cargo` or the
deployed source is absent, builds with `--locked --bin posture`, compares the artifact with the
deployed binary, and installs a changed one with `/usr/bin/install -m 755` into
`~/.local/libexec/posture/posture`. It differs from the pns builder in two ways: there is no daemon to
kickstart, because every posture agent is interval- or watch-driven and spawns a fresh process per
tick, so no pending marker and no `launchctl` call; and it runs at `03`, before the manifest runner
at `05`. The ordering is by target name with the script attributes removed:
`chezmoi --source <checkout> managed --include=scripts` lists
`.chezmoiscripts/03-build-posture.sh` before `.chezmoiscripts/05-osquery-known-good-manifests.sh`
before `.chezmoiscripts/50-setup-osquery.sh`, and chezmoi's reference says scripts run "in ASCII
order of their target names". A companion test, `test/unit/posture-build-install.sh`, mirrors
`test/unit/pns-engine-build-install.sh` (render the script, stub cargo, assert the install path and
the marker behavior).

### 3.2 File-integrity coverage of the new directory, and of the binary

Three copies of the tracked set gain one line each, in the same pull request and before any posture
file deploys: `osquery.conf.tmpl`'s `pipeline_integrity` category under `file_paths` and
`file_paths_hashes` (`osquery-converge/desired/osquery.conf.tmpl:39-84`), the manifest runner's
pipeline `case` arm (`run_after_05:192-206`), and the verdict's `_pipeline_is_tracked`
(`pipeline-verdict.sh:397-406`), each adding `~/.local/libexec/posture/*`. The `managed_bin` watch
already covers `~/.local/libexec/%%`, so the new directory fires under both categories exactly as the
old one does (spec section 3.9).

The binary itself is not chezmoi-managed, so `chezmoi managed` never lists it and the manifest runner
would leave it unvouched; under a tracked directory that is a page on every rebuild. Decision 2 in
section 8 settles this. The recommended shape: the manifest runner gains a third arm that hashes the
BUILD ARTIFACT at `~/.local/share/posture/target/release/posture` (the product of managed source,
never the protected tree) and writes one tuple for the deployed path with mode `0755` and the apply's
uid into the pipeline manifest, skipping the tuple when the artifact is absent so a missing build
leaves the deployed binary unvouched, which pages, which is the safe direction. This is why the build
runs at `03`: the artifact must exist before `05` hashes it. Editing the desired osquery config also
pages a CRIT until the full apply lands, which is the known property of manifesting intent that
CLAUDE.md records for every templated target.

### 3.3 The data files move

`posture-controls.json.tmpl` renders to `~/.local/libexec/posture/controls.json` (from
`~/.local/libexec/osquery/posture-controls.json`; the stutter goes because the directory now names
the tool), and the six desired-state files move to `~/.local/libexec/posture/converge/desired/`.
References that change with them: the poller's `CONTROLS_FILE` default
(`firewall-gatekeeper-monitor.sh:30`) until that script is deleted, the converge's staging path,
`treefmt.toml:118-126`'s four `osquery-config-render` globs, and the template's own header comment.
Decision 3 in section 8 asks whether the data moves at all.

### 3.4 Plists, loaders and the two callers

Each plist's `ProgramArguments` changes from `/opt/homebrew/bin/bash` plus the script path to
`{{ .chezmoi.homeDir }}/.local/libexec/posture/posture` plus the subcommand
(`com.webdavis.osquery-results-alerter.plist.tmpl:7-11`). The loaders change by nothing but their
embedded hash, which is what re-fires them. The labels stay `com.webdavis.osquery-*`: renaming them
would touch the allowlist seed, the manifest's plist arm, the verdict's plist arm, the page renderer's
basename regex (`render-page.sh:75`, `:147`), the watchdog's six-agent list and the plist filenames,
which is a separate migration.

`run_after_50-setup-osquery.sh:46` changes its path to `posture converge`. uu's brew lane runs the
converge as a program with no arguments (`dot_local/share/uu/src/lanes/brew/repairs.rs:75`,
`runner.run(&lane.osquery_converge, &[])`), so its `osquery_converge` key must carry argv rather
than one path; the proposed shape is a TOML array, `["<home>/.local/libexec/posture/posture",
"converge"]`, changed in uu's schema (`src/config/schema.rs:26`), its shipped template
(`dot_config/uu/private_config.toml.tmpl:81`) and its template test (`shipped_template.rs:101`) in
the same pull request as the converge cutover, per the pns charter's rule that a consumer changes in
the pull request that needs it. The operator sees that template change on `chezmoi diff`.

### 3.5 The justfile and CI

`test-rust` (`justfile:167-175`) gains three lines for posture (test, fmt check, clippy with
`-D warnings`), and `just ship` picks them up through `just test`. No CI workflow change: the
toolchain step already ships cargo. `test/validate-tests.sh` is untouched: the Rust tests live in
the crate.

### 3.6 The manifest runner stays bash

`run_after_05` is a chezmoi runner that calls chezmoi and sudo, not one of the twelve entry points,
and it stays a plain script. It changes twice: the one-line arm of section 3.2 and the binary tuple
of decision 2 in step 1, and the removal of the `osquery/*` arm in step 8 once that directory is
empty. The allowlist writer keeps invoking it by its source-relative path
(`allowlist.sh:34`, `MANIFEST_RUNNER_REL`), now from the `Publisher` adapter.

## 4. Rules every pull request in the ladder obeys

1. **Everything is new behavior, and every behavior lands red first.** A port has no pure moves.
   For a pinned statement the Rust test re-expresses the bats or bashunit pin by name, and the pull
   request carries a mapping table from the retired test to its successor (pns keeps that table in
   `dot_local/share/pns/docs/test-baseline.md`; posture keeps
   `dot_local/share/posture/docs/test-baseline.tsv` with the 186 case names recorded in step 1).
   For an UNPINNED statement the test is written first
   against the Rust code, fails for the stated reason, then passes; where the orphan fixture under
   `test/fixtures/` asserted the same behavior, the Rust test cites it. Every fix is mutation-checked
   by hand against an unmutated control, and the table goes in the pull request.
2. **Gates.** `just test-rust`, `just lint-check`, the builder's build line, `just ship` before the
   pull request opens (a topic branch with no open pull request runs the suite nowhere), and
   `cargo test --locked --manifest-path dot_local/share/uu/Cargo.toml` on the two pull requests that
   touch uu.
3. **File size.** Every `.rs` file, tests included, targets 300 lines and never exceeds 500. Unit
   tests live in a sibling `<module>/tests.rs`, split by behavior when they pass the cap. The
   rationale that makes the bash 1.09 lines of comment per line of code moves into
   `dot_local/share/posture/docs/decisions/` records; production keeps a one-line invariant and a
   link.
4. **Nothing reaches the live machine.** No test runs the built binary against the real results
   log, `/var/osquery`, a real `sudo`, `osqueryctl`, `osqueryi`, `launchctl`, `tailscale`, the
   hermes gateway or the deployed `pns`. Every test uses a sandbox `HOME` and the scripted
   `CommandRunner`. The one-second budget per test binds, measured under the parallel scheduler.
5. **One cutover per script, in one pull request.** The Rust subcommand lands, the operator verifies
   it live (section 7 names how), the plist or caller is repointed, and the bash file, its sourced
   helpers that nothing else sources, and its bats or bashunit tests are deleted, all in that pull
   request. Until it merges the bash keeps running. Chezmoi does not delete the retired target, so
   the operator trashes it by hand after the apply, and because the file sits under a tracked
   directory that deletion fires one expected CRIT page per file (S081); the pull request says so.
6. **Operator-only steps are named per pull request** (section 7): `chezmoi apply`, any osqueryd
   restart, the TCC grants a probe binary needs, the hermes config edit, the KeePassXC entry, and the
   trashing of retired files. Agents propose; the operator applies.
7. **Repository rules.** Conventional Commits, no trailers, no em-dashes, `trash` never `rm` for
   operator files, never `chezmoi apply`, never a force push, at most two open posture branches.

## 5. The migration order, and why

The order is by blast radius and test cover, smallest and best first, with the two constraints the
charter names (converge, which runs `sudo`, goes late; producers wait for the pns route):

1. **Deployment prerequisites first**, so every later file lands already covered by the
   file-integrity arm (step 1).
2. **The whole domain next**, because it is where the port's correctness lives and none of it can
   page, restart or write anything. It is also where 199 UNPINNED statements get their first tests,
   against pure functions, which is the cheapest place to write them (step 2).
3. **Adapters and use cases without delivery** (step 3), then the two cutovers that need no
   delivery at all: the enricher, a child process with a two-value exit contract and the smallest
   blast radius in the tree, and the allowlist writer, an operator tool (step 4).
4. **The delivery adapter**, once pns's route pull request has landed (step 5).
5. **Producers, smallest first**: heartbeat (109 lines, 17 tests, a daily silent message), digest
   (238 lines, 23 tests, daily), then the alerter (the WatchPaths agent, best-tested, largest), then
   the three untested producers (watchdog, poller, funnel monitor) each of which becomes tested for
   the first time in step 2, and last the drainer's retirement once the queue is empty (step 6).
6. **Converge last among the tools**, because it is the only one that calls `sudo`, restarts the
   root daemon, and writes under `/var/osquery`; it also has the best cover (48 bats plus 18
   bashunit cases), so its port is a translation of a well-pinned tool rather than a discovery
   (step 7).
7. **Cleanup** (step 8).

## 6. The ladder

Each pull request: **Statements** moved behind Rust; **Tests** (which pins are re-expressed by name,
which UNPINNED statements get their first test); **Surface** (the deployed references that change);
**Cutover** (what the operator verifies and does); **Sizes** (projections; the pull request measures
with `wc -l` and splits again if a projection was wrong). A pull request with no **Cutover** row
deploys source and changes no plist or caller.

### Step 0: the pns program's route pull request

Not a posture pull request. The specification's section 5.5 lists what it delivers. Posture's step 5
lands after it, and steps 1 to 4 do not wait for it.

### Step 1: the workspace and the deployment prerequisites

**PR 1.1 the workspace and the builder.** Creates `dot_local/share/posture/` with the four member
crates (each `lib.rs` a doc comment naming its responsibility), `Cargo.lock`, the `posture` binary
printing usage and exiting 2 on every word (S298, S341), `docs/README.md`, and
`docs/test-baseline.tsv` holding the 186 bats and bashunit case names as the set to map from. Adds
`run_onchange_after_03-build-posture.sh.tmpl`, `test/unit/posture-build-install.sh`, and the three
`test-rust` lines. **Tests**: the build-install test; a cli test that every subcommand word is
refused with usage and exit 2 until it exists. **Surface**: the builder, the justfile. **Sizes**:
`main.rs` under 60; the rest are doc comments. **Order**: first.

**PR 1.2 file-integrity coverage of `~/.local/libexec/posture/`.** The three one-line arms of
section 3.2, plus the binary tuple arm of decision 2 in `run_after_05`. **Tests**: none of the three
copies is in test scope (the 2026-08-05 ruling); the pull request records the three diffs side by
side. **Cutover**: the operator applies; the manifest gains the binary tuple; nothing pages, because
the directory is empty until PR 1.1's binary lands and that binary is vouched for by the same apply.
The desired `osquery.conf` change restarts osqueryd through the converge on that apply.

### Step 2: the domain, red first

Every pull request here is pure Rust in `posture-domain` with no adapter, no I/O and no cutover.
Each carries its unit tests in a sibling `tests.rs`; each UNPINNED statement it covers gets its first
test here.

**PR 2.1 findings.** `finding.rs`: the `Detector` enum (S025, S360), pack stripping (S023, S024),
the action default (S030), the churn rule (S027), the baseline discard and exemptions (S028, S029),
the enrich path (S032, S033), the emitted shape (S034). **Tests**: the twelve
`osquery-normalize-and-digest-store.bats` normalizer cases by name; S022 and S031 gain tests.
**Sizes**: `finding.rs` ~180, `finding/tests.rs` ~260.

**PR 2.2 severity and the gate.** `severity.rs` and `gate.rs`: the matrix (S035 to S040), the
outcome per detector (S046 to S058, S061 to S063, S066), the promotion (S044, S045), the fail-safe
severity read (S042), one outcome per finding (S067). Enrichment and triage are inputs here
(`Option<Signing>`, `Option<Triage>`); the adapters that produce them come in step 3. **Tests**: the
seventeen `osquery-route.bats` cases by name, the seven `osquery-alerter-criteria.bats` cases that
pin routing (C1 to C4d), and the four hostile-column cases, re-expressed over typed columns (the
argv-boundary property of S059 becomes "a column is a value, never a separator", which the type
makes trivially true and the test states). **Sizes**: `severity.rs` ~90 plus tests ~120;
`gate.rs` ~260 plus `gate/tests.rs` split into `page.rs`, `digest.rs`, `log_only.rs` at ~200 each.

**PR 2.3 the allowlist and known-good verdicts.** `allowlist.rs` (S069 to S077, the writer's grammar
S301 to S303, S305, S306), `known_good.rs` (S085 to S096, S234), `integrity.rs` (S079 to S084,
S097). The manifest vouch is a function argument, so S078 and S107 (the `declare -F` reuse) are
compile-time facts (spec D10). **Tests**: `C4a` to `C4d` and the route suite's vouch cases by name;
S072, S074, S081 to S084, S086 to S091, S093 to S097 gain tests, drawn from what
`test/fixtures/osquery-allowlist-lib.bash` and `osquery-manifest-lib.bash` asserted. **Sizes**:
three files of 150 to 240 plus tests under 400 each.

**PR 2.4 the page, the spool line and the digest.** `page.rs` (S124 to S133), `digest.rs` (S117,
S120, S286 to S294). The apostrophe rule (SI-13) is a test over every string constant in `page.rs`
and `digest.rs`, because the Discord body is still built by hand. **Tests**: the eleven
`osquery-render.bats` cases, the eight digest-store cases and the eleven grouping and cap cases of
`osquery-digest-builder.bats` by name. **Sizes**: `page.rs` ~280 plus tests split into `headers.rs`,
`fields.rs`, `caps.rs`; `digest.rs` ~200 plus tests ~350.

**PR 2.5 the canary and the heartbeat's wording.** `canary.rs`: S193 to S204, S211's freshness
rule. **Tests**: the seventeen `osquery-heartbeat.bats` cases by name. **Sizes**: ~150 plus tests
~300.

**PR 2.6 the watchdog's decisions and the audit.** `watchdog.rs` (S207 to S215, S217 to S226 over
readings), `audit.rs` (S227 to S240 over manifest lines and per-path readings). **Tests**: all of
them are first tests; the 41 functions of `test/fixtures/osquery-watchdog-lib.bash` and the 17 of
`osquery-manifest-lib.bash` are the source of the cases. **Sizes**: `watchdog.rs` ~280 plus tests
split by probe into five files; `audit.rs` ~220 plus tests ~380.

**PR 2.7 the controls and the poller's policy.** `controls.rs` (S254 to S256), `poll.rs` (S246 to
S249, S251, S257 to S264, S266, S267). **Tests**: all first tests, drawn from the 26 functions of
`test/fixtures/osquery-poller-lib.bash`. The eleven controls validations each get a one-step-either-
side test. **Sizes**: `controls.rs` ~200 plus tests ~300; `poll.rs` ~290 plus tests split into
`classify.rs`, `baseline.rs`, `gap.rs`, `transitions.rs`.

**PR 2.8 the funnel policy.** `funnel.rs`: S268, S269, S272, S273, S275, S276. **Tests**: first
tests from the 29 functions of `test/fixtures/osquery-tailscale-lib.bash`. **Sizes**: ~140 plus
tests ~250.

**PR 2.9 the converge's decision core.** `drift.rs` (S311 to S318, S320) and `converge_policy.rs`
(S326, S333, S334, S336, S345 to S347). **Tests**: the eighteen
`osquery-converge-drift-verdict.test.sh` cases by name, plus the policy half of the converge suite's
refusal and restart-evidence cases restated over readings; S347 gains its first test. **Sizes**:
`drift.rs` ~160 plus tests ~220; `converge_policy.rs` ~200 plus tests ~300.

**PR 2.10 the cursor, the enricher's classification and the triage.** `cursor.rs` (S007 to S011,
S013, S014, S016), `enrich.rs` (S134 to S141), `triage.rs` (S102 to S106, S110 to S115). **Tests**:
all first tests except the render-side pins of S102 and S130. **Sizes**: three files of 120 to 220
plus tests under 350.

### Step 3: adapters and use cases, without delivery

**PR 3.1 the file adapters and the locks.** `results_log.rs`, `state_files.rs`, `locks.rs`,
`known_good_file.rs`, `deployed_state.rs`, `allowlist_file.rs`, `controls_file.rs`,
`upgrade_record.rs`, `clock.rs`. **Tests**: adapter tests over temporary files: the torn-line read
(S011), the rotated-log inode (S006, S008), the rename claim under two processes (S282), the whole-
file JSON refusal with a trailing `{}` (S209), the trust check on a mode (S091), the symlink refusal
before hashing (S097), the FIFO refusal on the upgrade record (S108), the `flock` contention with a
second process (S001, S002, S299). **Sizes**: nine files of 80 to 220 plus tests under 400 each.

**PR 3.2 the process adapters.** `probes.rs`, `osqueryi.rs`, `launchd.rs`, `tailscale.rs`,
`codesign.rs`, `process.rs`, `gateway.rs`, and the `CommandRunner` with its deadline and process-
group kill. **Tests**: a hanging stub is killed at the deadline and reported as indeterminate
(S244, S271); every reader is exercised through the scripted runner over recorded output; the
gateway GET carries no signature header (S215). **Sizes**: eight files of 80 to 200 plus tests.

**PR 3.3 the use cases that do not deliver.** `Enrich`, `CurateAllowlist`, and the `Converge` use
case's read-only half (probe, verdicts, plan) over the ports of PRs 3.1 and 3.2 plus `staging.rs`,
`publisher.rs` and `privileged.rs` (the latter with a scripted runner only). **Tests**: use-case
tests over recording fakes pin the orderings (both directory verdicts before either action, S326;
the config check before the stop, S343; the vendor plist before anything, S342; the private copy
before any read root performs, S332). **Sizes**: `enrich.rs` ~120, `curate_allowlist.rs` ~200,
`converge.rs` ~280 plus `converge/plan.rs` ~150; tests under 400 each.

### Step 4: the two cutovers that need no delivery

**PR 4.1 `posture enrich`.** The cli subcommand over `Enrich`. **Statements**: S134 to S141.
**Surface**: `route.sh:115`'s `OSQUERY_ENRICH_SCRIPT` default becomes the posture binary with the
`enrich` word; the bash router passes the path as the second argument, so the call site changes by
one string. `executable_enrich-finding.sh` is deleted. **Cutover**: the operator applies, then runs
`~/.local/libexec/posture/posture enrich /Applications/Safari.app` and one unsigned script by hand,
reads the two exit codes (0 and 10) and the fact lines, and trashes the retired script (one expected
CRIT page, rule 5). **Sizes**: cli `enrich.rs` under 60.

**PR 4.2 `posture allowlist`.** The cli subcommand over `CurateAllowlist`. **Statements**: S298 to
S310. **Surface**: `executable_allowlist.sh` and `test/fixtures/osquery-allowlist-lib.bash` are
deleted. **Cutover**: the operator runs `posture allowlist list` against the deployed file and
compares it with `allowlist.sh -l` BEFORE the apply that deletes the script; after the apply, one
`posture allowlist add <own label>` on an already-listed own agent must reproduce the source byte for
byte (S306), run the targeted apply and the manifest runner (sudo prompt), and exit 0. **Sizes**: cli
`allowlist.rs` under 100.

### Step 5: the delivery adapter

**PR 5.1 the pns producer and the last-resort banner.** `pns_producer.rs` over `pns-protocol`'s
request and result envelopes, and `last_resort_banner.rs`. **Statements**: the successors of S142 to
S149 (the request id from the occurrence identity, CRIT-only submission, the durable bit), S180 (the
backslash-first literal) and S183 (the fixed loud sound, raised only on the engine-down path).
**Tests**: a scripted `pns` stub answering `accepted`, `degraded`, a refusal, garbage and nothing;
the durable bit follows the ledger diagnostic, never the delivery outcome; a stub that hangs is
killed at the deadline and reported as not accepted; the banner spawn happens on exactly the
not-accepted-because-the-engine-failed path and never on a clean refusal; the key never appears in
argv (there is none). **Order**: after step 0. **Sizes**: `pns_producer.rs` ~200 plus tests ~350;
`last_resort_banner.rs` ~80 plus tests ~120.

### Step 6: the producer cutovers

Each pull request lands one subcommand over one use case, repoints one plist, and deletes the bash
it replaces together with the tests that pinned it (their names appear in the mapping table with
their Rust successors).

**PR 6.1 `posture heartbeat`.** **Statements**: S193 to S206. **Surface**: the heartbeat plist,
`executable_heartbeat.sh`, `test/integration/osquery-heartbeat.bats`. `canary-freshness.sh` stays
until the watchdog cutover, because the watchdog still sources it. **Cutover**: the operator applies,
runs `posture heartbeat` by hand once (a silent Discord line reading "observed" arrives on the
priority route through pns), confirms the pns ledger recorded it, then trashes the retired script.
**Sizes**: `heartbeat.rs` (application) ~120; cli under 40.

**PR 6.2 `posture digest`.** **Statements**: S280 to S297. **Surface**: the digest plist,
`executable_digest.sh`, `test/integration/osquery-digest-builder.bats`. **Cutover**: apply; the
operator runs `posture digest` by hand against a spool the day has filled (or an empty one, which
must produce nothing and exit 0), confirms the `.last` rotation and the single silent message, and
trashes the script. **Sizes**: `build_digest.rs` ~180 plus tests ~300.

**PR 6.3 `posture alert`.** The largest cutover. **Statements**: S001 to S068 (the entry and its
four stages), S069 to S133 (the verdicts, triage, spool line and render, now called from the use
case), S360, S363 to S366 as routing facts. **Surface**: the results-alerter plist;
`executable_results-alerter.sh` and the whole `results-alerter/` directory (seven files);
`test/e2e/osquery-alerter-criteria.bats`, `osquery-alerter-hostile-columns.bats`,
`osquery-alerter-concurrency.bats`, `test/unit/osquery-route.bats`, `osquery-render.bats`,
`osquery-normalize-and-digest-store.bats`; the comment at
`dot_local/share/uu/src/lanes/brew/upgrade_record.rs:8` that names the triage helper's path. The
concurrency pins (S001, S002, S004) are re-expressed as a two-process test over the real lock path
in a sandbox. **Cutover**: the operator applies with the results log quiet, watches one tick of
`posture alert` under the WatchPaths trigger by touching nothing and reading the agent log, then
plants one known finding (a new user LaunchAgent that is not allowlisted) and confirms the page
arrives through pns and the cursor advanced by exactly one record, then removes the plant and
trashes the eight retired files. **Sizes**: `judge_results.rs` ~280 plus `judge_results/tail.rs`
~120 (the checkpoint and delivery tail), tests split by stage into four files under 400.

**PR 6.4 `posture watchdog`.** **Statements**: S207 to S240. **Surface**: the watchdog plist,
`executable_uptime-watchdog.sh`, `executable_pipeline-audit.sh`, `executable_canary-freshness.sh`,
`test/fixtures/osquery-watchdog-lib.bash`, `osquery-manifest-lib.bash`. Probe 4 (S216) is not
ported (spec D4); a probe that the deployed `pns` binary exists and is executable replaces it, named
as new behavior with its test. **Cutover**: apply; the operator reads one healthy tick's silence in
the agent log, then stops the digest agent by hand (`launchctl bootout`) for one tick, confirms the
"not loaded" page, bootstraps it back, and trashes the retired files. **Sizes**: `watchdog.rs`
(application) ~260 plus tests ~350.

**PR 6.5 `posture poll`.** **Statements**: S241 to S267. **Surface**: the poller plist,
`executable_firewall-gatekeeper-monitor.sh`, `test/fixtures/osquery-poller-lib.bash`; the controls
file path per section 3.3. **Cutover**: apply; the operator confirms the first tick reads the existing
baseline (no first-observation page), then toggles one control the safe way (start and stop the
OverSight process) across two ticks and reads the page and the silent recovery. The probe binaries
`fdesetup`, `csrutil`, `sysadminctl`, `defaults`, `pgrep`, `plutil` and `readlink` need no TCC grant;
`plutil` reading LuLu's rules archive under `/Library/Objective-See` reads a world-readable file
today and the port changes nothing about who reads it. **Sizes**: `poll.rs` (application) ~280 plus
tests split into three.

**PR 6.6 `posture funnel`.** **Statements**: S268 to S279. **Surface**: the funnel plist,
`executable_tailscale-monitor.sh`, `test/fixtures/osquery-tailscale-lib.bash`. **Cutover**: apply;
the operator confirms the existing `inactive` baseline is read as such (no first-observation page),
then runs `tailscale funnel status --json` by hand to confirm the reader's input matches what the
adapter parsed in the agent log. **Sizes**: `funnel.rs` ~140 plus tests ~220.

**PR 6.7 the drainer and the dispatch library retire.** No Rust. **Surface**: the alert-drainer
plist and its loader, `executable_drain-undelivered-alerts.sh`, `executable_alert-dispatch.sh`,
`test/unit/osquery-alert-dispatch.bats`, `test/integration/osquery-drain-continuation.bats`,
`test/helpers/build-dispatch-harness.sh`. **Cutover**, and this one has a precondition: before the
apply, the operator confirms both counters read zero (the two library functions, sourced into a
shell, or `sqlite3 -readonly` over the two tables) so no page is stranded in the retiring queue, and
after the apply trashes the retired files, the queue's three files, `osquery-spool/`,
`osquery-tailscale-funnel` (the two leftovers of spec section 3.10) and
`~/.config/osquery/webhook-secret` (retired with the route in step 0). **Order**: after PR 6.1 to
6.6, because every producer must have stopped writing the queue first.

### Step 7: the converge

**PR 7.1 `posture converge`.** The cli subcommand over the `Converge` use case with the real
`privileged.rs` behind the scripted runner in tests. **Statements**: S311 to S349, S368.
**Tests**: the 48 `osquery-converge.bats` cases by name, re-expressed against a sandbox target
directory with a stub `sudo` and stub `osqueryctl` exactly as the bats harness does today, plus the
restart-evidence cases over a fake `ProcessTable` and fake `Clock` with no sleeps. The test seam gate
(S330, S331) becomes the one environment variable posture reads, kept because a harness that set only
some of the seams would otherwise converge the live machine. **Surface**: `run_after_50:46`; uu's
`osquery_converge` key per section 3.4 (its schema, template, template test, and `repairs.rs:75`);
`executable_osquery-converge.sh`, `osquery-converge/drift-verdict.sh`,
`test/unit/osquery-converge.bats`, `test/unit/osquery-converge-drift-verdict.test.sh`. The desired
tree stays where PR 1.2 or decision 3 put it. **Cutover**: the operator applies; the apply itself
runs `posture converge` through `run_after_50` and, with nothing drifted, prints nothing. The
operator then edits nothing and instead confirms the quiet no-op in the apply output, runs
`sudo /usr/bin/install -m 0666 /var/osquery/osquery.flags /var/osquery/osquery.flags` to plant a
mode drift by hand, re-runs the apply, reads the one repair line and the daemon restart, and confirms
osqueryd's parent pid changed. That restart is the operator's. **Sizes**: cli `converge.rs` under
80; the application and adapters were measured in step 3.

### Step 8: cleanup

**PR 8.1 the old directory leaves the tracked set.** Once `~/.local/libexec/osquery/` holds nothing
(the operator has trashed every retired file), the three `osquery/*` arms of section 3.2 are removed
from the watch paths, the manifest runner and the (now Rust) tracked set, and `run_after_05`'s
docblock names the new directory. **Cutover**: apply, one osqueryd restart through the converge for
the watch-path change.

**PR 8.2 the file-size lint and the completion record.** `scripts/treefmt/rust-file-size.sh` over
`dot_local/share/posture/**/*.rs` at the 500 cap, if the pns program's PR 18.1 has not already added
a shared one; the completion report with the before-and-after line table (spec section 9 against
the crate), the test mapping table complete (186 retired names, each with a successor or a reason),
and the decision records index.

## 7. Operator-only steps, collected

Every one of these is the operator's and never an agent's:

- Every `chezmoi apply` in the ladder, with KeePassXC unlocked (fourteen templates read it).
- The osqueryd restarts: PR 1.2 and PR 8.1 change the desired `osquery.conf` and the converge
  restarts the daemon on that apply; PR 7.1's planted drift restarts it again.
- Step 0's hermes edit: re-key the `priority` route to the pns secret and switch its prompt to pns's
  fields in `private_dot_hermes/encrypted_private_config.yaml.age` (age identity required), then
  retire the runtime key file `~/.config/osquery/webhook-secret` in PR 6.7.
- The live verification of each cutover, as its **Cutover** row states, before the pull request that
  deletes the bash merges.
- Trashing each retired deployed file after its apply (chezmoi deletes nothing), and reading the one
  expected CRIT page per trashed file.
- Confirming the two queue counters read zero before PR 6.7's apply.
- TCC: no posture subcommand needs a grant the bash did not already have. `osqueryd` itself keeps
  Full Disk Access as today; `posture poll` reads the same world-readable files the poller reads.

## 8. Decisions the operator makes before code moves

Two options each, the recommendation first.

1. **How posture delivers.** (a) Posture is a pns producer through `pns submit --json` with the
   priority route re-keyed to the pns secret; pns owns the ledger, the retries, the presence gate,
   the phone and the replay; posture keeps one last-resort banner for the engine-down case
   (spec section 5.2). (b) Posture ports the dispatch library as its own `AlertSink`: a second
   SQLite queue, drainer, dead-letter policy and signer key, with no presence gate, phone, replay or
   recap (spec section 5.3). Recommendation: (a). It is better on every axis the charter names and
   worse only on schedule, which the operator has ruled is not a design input; the cost is that the
   producer cutovers wait for three pns pull requests, and the plan is ordered so nothing else does.
2. **Whether the built binary is vouched for by the file-integrity arm.** (a) The manifest runner
   hashes the build artifact and writes one tuple for the deployed binary, so a swapped binary pages
   like a swapped script does today; the build moves to slot `03` so the artifact exists at `05`.
   (b) The binary lives under a tracked directory as an untracked neighbour, the way the `pns` and
   `uu` binaries do today, and the file-integrity arm is blind to it. Recommendation: (a). Posture is
   the tool that judges the integrity of everything else; leaving its own binary unwatched would make
   the port a regression against the scripts it replaces.
3. **Where posture's files live.** (a) `~/.local/libexec/posture/` for the binary, `controls.json`
   and `converge/desired/`, joining the three tracked-set copies once in PR 1.2 and leaving the
   `osquery/` arms in PR 8.1. (b) Everything stays under `~/.local/libexec/osquery/` beside the
   scripts it replaces, touching no watch path or manifest arm. Recommendation: (a). CLAUDE.md's
   naming rule says a libexec directory names the system, and osquery is the vendor daemon, not the
   tool; (b) saves two one-line edits and leaves the tool misnamed for good.
4. **Whether a security page honours the operator's mute.** (a) It does: under pns a muted or
   Focus-silenced page still lands in Discord at once, is journaled as a miss, and reaches the banner
   and phone on the operator's return (pns S103, S106). (b) pns gains a mute bypass for
   `NeedsAttention` from this producer, which is a pns design change. Recommendation: (a). The
   operator set the mute, Discord still pings, and a bypass would be the first producer allowed to
   overrule the operator in pns.
5. **Whether the port changes product behavior along the way.** (a) It does not: option E of the
   allowlist boundary design (own-agent suppression by manifest membership, retiring the
   empty-`sha256` convention) and any change to the detector set or the page wording are separate
   pull requests after the port. (b) Fold option E into PR 2.3, since the verdict is being rewritten
   anyway. Recommendation: (a). A port whose tests re-express the bats pins by name proves nothing
   if the behavior changed underneath; option E is a one-page follow-up with its own red test.

Two matters are recorded as settled rather than asked. The LaunchAgent labels keep the
`com.webdavis.osquery-*` spelling, because renaming them is a separate migration touching six
references (section 3.4). And the state files keep their `~/.local/state/osquery-*` paths and shapes,
because a cutover that read the same file is a cutover that pages nothing new (section 2.7).

## 9. Reading order for a reviewer

The specification's sections map onto the ladder: section 1 and 2 (entry points and exit codes)
land in PR 1.1 and each cutover of steps 4, 6 and 7; section 3 (data contracts) in PRs 2.3, 2.4,
2.7, 3.1 and 3.3; section 4 (the invariants) is cited by the PR that carries each SI number, and a PR
that touches an SI names it; section 5 (delivery) in step 0 and PR 5.1; section 6 (the drops) in
PRs 6.3, 6.4 and 6.7, whose mapping tables list the dropped statements with their D number; section
8's statement numbers appear in every PR description, so the reviewer opens the specification at
those numbers and the pins named there are the tests the PR must carry across or write.
