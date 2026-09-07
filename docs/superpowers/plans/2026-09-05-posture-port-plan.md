# posture: the plan for the Rust port

Written 2026-09-05 against `origin/main` at `b5412089`, with the SSH (Secure Shell) scope added on
2026-09-06. It moves the 399 behaviors inventoried in
`docs/superpowers/specs/2026-09-05-posture-behavioral-specification.md`
out of the twenty-one shell files under `dot_local/libexec/osquery/` and into one Rust workspace, one
reviewable PR (pull request) at a time. Each bash script runs until its replacement is verified live and
its plist repointed. It follows the shape of `docs/superpowers/plans/2026-09-05-pns-refactor-plan.md`
and the standing rules of the pns charter: the engineering priorities and SOLID (single responsibility,
open/closed, Liskov substitution, interface segregation and dependency inversion) principles in Rust.
The paired Rust standard sets the file-size targets and caps in section 4. This plan names, per step,
which statements move, which tests carry them or are written first, which deployed surface changes,
what the operator alone must do, and what every resulting file is projected to
measure. Every crate, module, subcommand, path and file name below is proposed, confirm before creating.

## 1. Where the ladder starts

At the source baseline, posture had no crate, binary, workspace or test baseline. The inputs read for
this plan:

- the specification's original 368-statement pipeline inventory and 31 SSH additions; after review,
  163 statements are fully pinned, three partially pinned and 233 UNPINNED;
- the twenty-one shell files (7,263 lines, 3,326 of code, 3,622 of comment), the two chezmoi
  runners (`run_after_05`, 386 lines, and `run_after_50`, 58), the seven plists and their seven
  loaders, and the 62 inline jq programs and 7 SQL (Structured Query Language) statement groups a port
  replaces (about 3,800 lines of code in all);
- the eleven bats files and one bashunit file, 186 test cases, 3,886 lines, and the five orphan
  fixture libraries under `test/fixtures/` (2,051 lines, 127 functions) that are the only record of
  what the five untested tools once asserted;
- the pns crate's workspace (`dot_local/share/pns/Cargo.toml`, five members, the dependency edges
  declared in the member manifests) and its builder
  (`.chezmoiscripts/run_onchange_after_58-build-pns-engine.sh.tmpl`);
- uu's lane adapter shape (`dot_local/share/uu/src/lanes.rs:39-59`), its spawn seam
  (`src/lanes/spawn.rs:59`) and its two clients of pns (`src/alert.rs`, `src/delivery.rs:11`);
- the locked decisions in the specification's section 4 and the delivery decision in its section 5;
- `dot_local/bin/executable_ssh-hardening.sh` (2,826 lines) and its one unit test,
  `test/unit/ssh-hardening-dropin.sh`, which joined the port by operator ruling on 2026-09-06 (spec
  section 8.22, S369 to S399).

Four pull requests in the pns program gate step 5 (the delivery adapter) and step 6 (the producer
cutovers), and nothing before them: PR 7.3 (`pns submit --json` with a result envelope), PR 11.4
(the write-ahead delivery ledger the daemon drains), the priority-route pull request the
specification's section 5.5 sizes, and the delivery-class pull request that lets a security page
through the operator's mute (decision 4, step 0.5). They gate those two steps because pns today
acknowledges nothing durably: the channels are dispatched before the record is written and a failed
journal write is dropped (`dot_local/share/pns/src/main.rs:3101-3122`, `:814-858`; spec section
5.2), and step 5 is the first posture code that submits anything. Steps 1 to 4 deliver nothing, so
they run as a second lane beside the pns work, both lanes starting at step 0; only step 0's design
items (the route overlap contract and the three-table queue check) sit ahead of step 1, because code
in the ladder depends on them. Section 5 states the two lanes.

Every Rust brief, implementation and review follows both
`/Users/stephen/.agents/skills/clean-code/SKILL.md` and
`/Users/stephen/.agents/skills/clean-code-rust/SKILL.md`. The Rust binding wins
all numeric and mechanism conflicts.

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
crates/posture-protocol      the existing cross-process digest record codec
crates/posture-adapters      everything concrete, by capability
crates/posture-cli           argv decoding, composition, exit codes; the `posture` binary
```

The five roles are five crates in this workspace, as the paired Rust standard requires. The member
manifests enforce dependency direction: `posture-application` depends on `posture-domain`;
`posture-protocol` owns serialization and depends on `serde_json`, with no application or adapter
dependency; `posture-adapters` depends on domain, application, posture-protocol and the sibling
`pns-protocol`; `posture-cli` composes the domain, application and adapters. Domain and application
import no wire crate, JSON (JavaScript Object Notation) value or concrete adapter. The adapter maps
records to typed domain values.

`posture-protocol` owns the persisted six-field digest record shared by the separate `posture alert` and
`posture digest` processes. This is an existing contract: `results-alerter/digest-store.sh:42-52` writes
it and `executable_digest.sh:108-144` consumes it (spec section 3.5, S117 to S121 and S280 to S296). The
codec keeps the current field names, coercions, torn-line handling and unversioned wire shape. There is
no new queue or version field. Append, claim, restore and filesystem permissions stay in adapters;
derivation, grouping, sanitization and caps stay pure domain policy. PR 1.1 declares the staged protocol
responsibility; PR 2.4 implements and tests the codec against the Bash fixtures.

Pns owns its request/result envelopes, versioning and compatibility tests in the sibling `pns-protocol`.
Only `posture-adapters` consumes that external crate; posture-protocol neither copies nor forwards its
envelopes. Osquery results, chezmoi manifests, uu upgrade records and the controls file are consumed
formats, whose readers retain their adapter codecs. This preserves each owner's contract without an empty
fifth-crate facade.

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
- `known_good`: the four-column `Tuple`, the line grammar, `TrackedSet` (the four patterns and the bin
  arm's inverted fail-safe), `manifest_for`, the tuple match with its empty-column rule (S085 to S096,
  S234), plus PR 1.2's explicit unbuilt variant, which can never vouch for content.
- `integrity`: the page-or-silent decision over a `DeployedState` reading (kind, hash, mode, uid)
  and a manifest answer, the DELETED rule, the atomic-rename shape (S079 to S084, S097).
- `triage`: the correlation facts from a manifest answer, an on-disk reading and a parsed upgrade
  record; the record grammar and its window, row and count caps (S102 to S115).
- `page`: the header, field, next-step and cap vocabulary, the sanitize chokepoint, the eight-block
  and 1900-character caps, the shell quoting of paths (S124 to S133).
- `digest`: the six derived digest facts as typed values, the grouper, sanitization and the four caps
  (S117, S120, S286 to S294). Record encoding, JSON shape coercion and torn-line decoding live in
  posture-protocol; the domain receives parsed values and depends on no codec.
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
- `ssh_policy`: the seven protected directives and their alias fold, `assert_hardened` over a parsed
  `sshd -G` listing, the two sshd tokenizers and the Include pattern analysis as functions over
  strings, the scan's refusal set, the readiness-knob and port validation, the host-key record
  shape, and the tree record grammar with its comparison (S370, S375 to S380, S389, S390, S393,
  S395, S396). Split into `directives.rs`, `tokenizer.rs`, `include.rs` and `tree.rs`.

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
  `LaunchdState`, `GatewayHealth`, `KnownGood`, `DeployedState`, `StateFile`, `PnsLedger`,
  `IndependentAlarm` and `AlertSink`. The application owns these ports. Pns integrity and health
  findings call `IndependentAlarm` directly before any optional `AlertSink` submission; a valid
  acknowledgement from that sink cannot suppress the independent attempt. PR 6.4 also introduces a
  temporary `LegacyQueue` port for the existing queue's pending and dead-letter counts. Its probe
  and growth history remain active until PR 6.7 retires that queue.
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
- `SshInstall`, `SshVerify`, `SshReload`, `SshRollback`, `SshPrintConfig` and `SshPrintPath` replace
  `ssh-hardening.sh`'s six modes over `Sshd` (the bounded `-G`, `-t` and `-G -T -C` calls),
  `SshdTree` (roots, include walk, observation), `PrivilegedFs` (the five privileged file operations
  of the install transaction), `Launchctl` (print, kickstart), `BannerProbe`, `Sleeper` and `Clock`.
  The install's signal handling belongs to the use case, because what it rolls back is the use
  case's own transaction state (S384, S385).

`AlertSink` is the one port every producer shares. Its contract is the specification's S144 as a
type: `fn submit(&self, alert: &Alert) -> Accepted | NotAccepted(reason)`, where `Accepted` means the
engine has COMMITTED a ledger row for this request id before dispatching anything and owns its
delivery from here: a retriable obligation, not a report that the channels were tried. The adapter
answers `Accepted` only from a result envelope whose diagnostics say the row was written; an
`accepted` with no such diagnostic, a `degraded`, a refusal, garbage, silence or a timeout are all
`NotAccepted`. Notify-before-persist (SI-4) is then one line in each use case: advance state only on
`Accepted`.

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
- `allowlist_file`: `Allowlist` and `SourceAllowlist`, newline-delimited JSON with comments preserved and
  the one-tuple-per-line rule. Replaces the file reads in `allowlist-verdict.sh` and `allowlist.sh`.
- `controls_file`: `ControlsFile`, the JSON read the domain validates. Replaces `load_controls`.
- `upgrade_record`: `UpgradeRecord`, the regular-file check, one bounded read, the tab-separated value
  split by expansion. Replaces `file-integrity-triage.sh:236-333`.
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
- `gateway`: `GatewayHealth`, one unsigned GET of the gateway route posture posts to (the pns-keyed
  route once `priority` retires) under a deadline through `ureq`. Replaces the watchdog's probe 3.
- `pns_producer`: `AlertSink`, spawning the deployed `pns` binary with `submit --json`, encoding the
  request with `pns-protocol`, decoding the result, mapping the durable bit, with the child bounded.
  Replaces `alert-dispatch.sh` (spec section 5).
- `last_resort_banner`: `IndependentAlarm`, the one bounded `osascript` spawn posture keeps. It
  reports detectable submission failure (absent, nonzero, timed out, unparseable), and watchdog
  findings about pns integrity and delivery health directly, independently of any pns result. It
  replaces the alarm role of `_osquery_notify_local_durable`; its local-attempt limits are spec
  section 5.2.
- `pns_ledger`: `PnsLedger`, a synchronous `rusqlite` reader opened read-only without create or
  `immutable=1`, so it includes committed rows in the write-ahead log. A bounded busy timeout
  turns contention, corruption, missing files or incompatible schema into an unreadable result.
  Pns owns the schema and migration; this adapter neither writes nor repairs it.
- `legacy_queue`: the temporary `LegacyQueue` reader uses the same read-only connection rules for
  the existing queue's two counters (S176 to S178). PR 6.4 adds it and PR 6.7 removes it; Bash owns
  every write and drain until retirement. The watchdog keeps each store's growth history separate.
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
- `bounded`: the process-group runner every `Sshd` call goes through: `setpgid`, stdin from
  `/dev/null`, SIGTTOU and SIGTTIN ignored in the child, a 0.25 s poll to the deadline, TERM to the
  group, a 2 s grace, KILL to the group, the reap, and a `Timeout` outcome (S381). It is the same
  shape as the `CommandRunner` deadline above and may be the same type; the property that must
  survive is the GROUP kill, which the sshd hang needs and a plain child kill does not deliver.
- `sshd`, `keyscan`, `sshd_tree` and `privileged_fs`: `sshd -G`, `sshd -t` and `sshd -G -T -C <spec>`
  by absolute path; `ssh-keyscan -T <timeout> -p <port> 127.0.0.1`; the drop-in directory listing,
  the include walk over the domain's resolver, `stat` following the link for type, mode, uid and
  gid, and a content digest (sha256 replaces `cksum`; a record never leaves one run); and `sudo -n`
  `tee`, `chmod`, `cp -Rp`, `mv -f` and `rm -f` by absolute path for the install transaction and the
  rollback (S383, S395, S397). Replaces the ssh script's spawns.

A spawned child runs under an explicit deadline with process-group termination, the way uu's
`CommandRunner::run_with_deadline` and pns's `run_bounded` already do, and every spawn goes through
one `CommandRunner` trait with a scripted double, so no adapter test runs a real `sudo`, `osqueryctl`,
`osqueryi`, `codesign`, `tailscale` or `pns`.

### 2.5 `posture-cli`

`main.rs` decodes argv, builds the adapters in one `compose.rs`, hands them to the use case the
subcommand names, and translates the outcome to an exit code; it is projected under 120 lines and
holds no policy. The subcommands, one per entry point, are the specification's section 1 table:
`alert`, `poll`, `funnel`, `watchdog`, `digest`, `heartbeat`, `converge`, `allowlist add|deny|list`,
`enrich <path>`, and `ssh install|verify|reload|rollback|print-config|print-path` (the
specification's section 8.22; the mode words are kept so the runbook reads across, and a bare `ssh`
is usage, spec D15). An unknown word or a missing verb prints usage to stderr and exits 2 (S298,
S341, S369).
The exit codes per subcommand are the specification's section 2, unchanged, because each caller
depends on them.

### 2.6 What the binary may spawn

Stated up front, as pns does in its manifest (`dot_local/share/pns/Cargo.toml:1-30`), because the
roster is a security property of a security tool: `osqueryi`, `osqueryctl` (trust-checked), `sudo`
and `install` (by absolute path, converge only), `launchctl`, `pgrep`, `fdesetup`, `csrutil`,
`sysadminctl`, `defaults`, `plutil`, `readlink`, `xattr`, `file`, `codesign`, `tailscale`, `chezmoi`
(the allowlist writer only), the manifest runner (a chezmoi source script, writer only), the deployed
`pns` binary, `osascript` for the last-resort banner alone, and for `posture ssh` only: `sshd`,
`ssh-keyscan`, and under `sudo` the five file tools of the install transaction (`tee`, `chmod`,
`cp`, `mv`, `rm`, absolute paths). Nothing else. `shasum`, `openssl`, `sqlite3`, `curl`, `jq`,
`alerter`, `gtimeout`, `hostname`, `date` and `lockf` all leave: sha2, `ureq`, `serde_json`, libc's
`flock`, `gethostname` and `gmtime_r`, and a Rust deadline replace them. With the ssh port `stat`,
`cksum`, `awk`, `id`, `mktemp` and `sleep` leave too: libc's `stat` and `getpwuid`, sha2, a
standard-library temporary file and a Rust sleep replace them.

### 2.7 What stays a file, and why

Classified by the readers and writers outside one process, as the pns plan's section 8 does. Existing
state files keep their paths and shapes. The explicit manifest extension is listed below:

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
- The controls file, the allowlist and the desired tree keep their existing data contracts. The
  manifests retain ordinary tuples and gain the explicit unbuilt record described in section 3.2;
  its writer and both consumers change in the same pull request.
- The sshd drop-in `/etc/ssh/sshd_config.d/000-ssh-hardening.conf`, its dot-prefixed staging and
  saved copies, and the legacy `50-no-password-auth.conf` it moves aside are sshd's contract and
  keep their names, content and modes byte for byte (S370, S383).

Every writable multi-record store in the target design belongs to pns. Posture uses SQLite only for
independent read-only health checks: pns's ledger, plus the legacy queue during PRs 6.4 to 6.7. Neither
reader needs its store's producer or drainer to answer.

### 2.8 Dependencies

`serde_json` (the result log, the controls file, the allowlist, the funnel status, the spool),
`sha2` (file hashes and the fingerprint), `ureq` with rustls (the one GET), `libc` (`flock`,
`gethostname`, `gmtime_r`, `kill`, `O_NOFOLLOW`), `rusqlite` (read-only pns and temporary legacy health,
`posture-adapters` only), and `pns-protocol` by path. No `toml`: posture has
no configuration file today, every path is derived from `$HOME`, and the port keeps that. No `hmac`:
posture signs nothing.

## 3. The deployment surface

The items here are named because CLAUDE.md's "Moving a script is never just a move" applies literally
to this port, and the specification's section 1 lists the references.

### 3.1 The build step

`.chezmoiscripts/run_onchange_after_58-build-posture.sh.tmpl` (slot proposed; beside the pns builder
and after `05`) mirrors `run_onchange_after_58-build-pns-engine.sh.tmpl`: it defers with a retry
marker when `~/.cargo/bin/cargo` or the deployed source is absent, builds with
`--locked --bin posture`, compares the artifact with the deployed binary, and installs a changed one
with `/usr/bin/install -m 755` into `~/.local/libexec/posture/posture`. It differs from the pns
builder in four ways.

There is no daemon to kickstart, because every posture agent is interval- or watch-driven and spawns
a fresh process per tick, so no pending marker and no `launchctl` call.

The build does NOT run before the manifest runner. The first draft put it at slot `03` so the
artifact would exist when `05` hashed it, and that ordering is wrong: a release build takes tens of
seconds to minutes, and `05` is placed first (S351) because the WatchPaths alerter judges the change
an apply makes exactly once and waits at most `OSQUERY_PIPELINE_SETTLE_SECONDS` (5 s,
`results-alerter/pipeline-verdict.sh:136`) for a manifest that predates it (S098). A compile in front
of `05` would hold the manifests for every already-deployed script and plist the same apply changed
past that budget, and each of them would page a false CRIT that is never reconsidered. So the
builder runs after `05`, and the tuple for the binary comes from the build record below rather than
from a file that has to exist at `05`.

Its trigger header hashes more than its own tree. Besides every `*.rs`, `build.rs`, the manifests
and the lock under `dot_local/share/posture/`, it globs the sibling path dependency
`dot_local/share/pns/crates/pns-protocol/**` the same way, because a change there changes the bytes
this installs and the pns builder's header would fire only the pns build. The compiler is a build
input too: the header records the modification time of every installed toolchain's `rustc` under
`~/.rustup/toolchains`, never its contents, so a `rustup update` re-fires the build. This avoids
reading rustup's settings at render time. Updating an inactive toolchain may trigger an identical
build, which changes neither installed bytes nor their record. The record also carries `rustc --version
--verbose` so the pull request and the audit trail
can say which compiler produced the vouched bytes.

The builder coordinates publication in this order: build a nonempty artifact no larger than 8 MiB;
publish its mode-600 BUILD RECORD (`~/.local/state/posture-build-record`, sha256, byte size and
`rustc --version --verbose`) by rename while retaining the previous record; invoke the repository's
`run_after_05` by source-relative path with `--pipeline-only`; install the binary atomically. If
both artifact and full record are identical, it skips publication. PR 1.1 includes the minimal
coupled change to that runner: its default invocation still refreshes both manifests, while the
builder's new option selects only the pipeline manifest and its inputs. This moved from PR 1.2
because the old runner could publish the new pipeline tuple and then fail on managed-bin, after
which restoring only the build record would leave an inconsistent prior tuple.

A refusal before pipeline publication restores the previous build record (or removes the first
failed record) and preserves the prior binary and pipeline tuple. The scoped runner has no second
publication that can fail after the pipeline succeeds. A failed install retains the new record and
tuple, leaving the old binary as detectable drift and the run eligible for retry. An interruption
after manifest publication has the same detectable mismatch; there is no cross-file atomicity or
rollback guarantee across interruption. The mismatch interval begins when the manifest publishes
and ends at successful binary installation. It includes process startup and scheduling and has no
fixed time bound; it is not one rename wide. A watchdog tick during that interval can page. The
builder never suppresses that check or hashes live bytes to make it quiet. Section 3.2 defines the
record reader added in PR 1.2; until then the scoped refresh preserves existing pipeline behavior.

The release artifact is measured. The manifest audit refuses to hash a manifested file over
`OSQUERY_PIPELINE_AUDIT_MAX_BYTES` (8 MiB, `executable_pipeline-audit.sh:91`) and reports it
`oversize`, which the watchdog pages as "too large to hash" (`executable_uptime-watchdog.sh:338-346`).
The deployed `pns` binary measures 4,285,136 bytes and `uu` 3,246,960 on 2026-09-06, so a stripped
release build of a crate this size fits, but PR 1.1 records `wc -c` of the artifact and the release
profile (`strip = true`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, proposed) in its
description, and a build that crosses 8 MiB fails the builder rather than publishing a tuple the
audit will page on every tick.

Companion tests, `test/unit/posture-build-install.test.sh` and
`test/unit/posture-manifest-refresh.test.sh`, mirror `test/unit/pns-engine-build-install.sh`
(render the script, stub cargo, assert the install path, the record, the refresh call, the ordering
and the marker behavior).

### 3.2 File-integrity coverage of the new directory, and of the binary

The first draft called this "three one-line edits". It is five edits to the tracked-set copies plus
the callers and globs of section 3.3, all in the same pull request and before any posture file
deploys, each adding `~/.local/libexec/posture/*` beside the `osquery/*` pattern it mirrors:

- membership: the manifest runner's pipeline `case` arm (`run_after_05:192-206`), so the runner
  lists posture's managed files in the pipeline manifest;
- classification: the verdict's `_pipeline_is_tracked` (`pipeline-verdict.sh:397-406`), so a change
  under the directory is judged rather than treated as an untracked neighbour;
- manifest selection: the verdict's `_pipeline_manifest_for` (`pipeline-verdict.sh:278-284`), which
  the first draft missed. It routes `~/.local/libexec/osquery/*` to the pipeline manifest and EVERY
  other `~/.local/libexec/*` path to the managed-bin manifest, and the two manifests never vouch for
  each other (S090). Without this edit the runner would list posture's files in the pipeline
  manifest and the verdict would look them up in the bin manifest, find no tuple, and page every
  file on every change;
- both watch arrays: `osquery.conf.tmpl`'s `pipeline_integrity` category under `file_paths` and
  again under `file_paths_hashes` (`osquery-converge/desired/osquery.conf.tmpl:39-84`), so the change
  event carries a digest and takes the hashed path (S362). The `managed_bin` watch already covers
  `~/.local/libexec/%%`, so the new directory fires under both categories exactly as the old one does
  (spec section 3.9).

The binary itself is not chezmoi-managed, so `chezmoi managed` never lists it and the manifest runner
would leave it unvouched; under a tracked directory that is a page on every rebuild. Decision 2 in
section 8 settles this with four rules the manifest runner follows, replacing the first draft's
"hash `target/release/posture` at `05`", which would have blessed whatever bytes an out-of-band
`cargo build` had left in the target directory while the onchange builder skipped the install.

1. The digest comes from an AUTHORIZED build only. The runner gains a third arm that reads the build
   record the builder wrote (section 3.1) and emits one tuple for `~/.local/libexec/posture/posture`
   with the recorded sha256, mode `0755` and the apply's uid. It never hashes the target directory
   and never hashes the deployed binary, for the reason the other two columns never read the
   protected tree (`run_after_05:62-91`). The record is user-writable state, like the chezmoi source
   it derives from, and SI-14's claim covers it: an attacker who can rewrite the record can rewrite
   the source, and neither is defended at this layer.
2. When no build ran, the trusted tuple is RETAINED, not recomputed. The record changes only through the
   authorized builder publication, so an apply that rebuilt nothing re-emits the same tuple, and a
   `cargo build` run by hand, or a binary copied into place by hand, is drift the audit never adopts:
   it pages until an authorized build republishes the record, and a build that produced byte-identical
   output leaves the tuple as it was.
3. No build record means explicit UNBUILT state, never a hash of empty input. The runner emits
   `unbuilt 0755 <uid> <absolute-binary-path>`. The manifest codec gains a typed `Unbuilt` variant
   beside an ordinary digest tuple; this is an intentional extension of S234's current grammar.
   PR 1.2 updates the Bash audit and tuple lookup together with the writer. The audit enumerates
   the path and reports `missing` if absent or `content` if a regular file exists, including an
   empty file with matching mode and owner. Existing irregular-file refusals remain. The lookup
   never treats `unbuilt` as a digest, including when a supplied event hash is that literal word.
   No bytes can satisfy this record. Malformed build records still refuse the manifest as a whole.
4. The builder orders record, scoped pipeline refresh, then install, as section 3.1 states. Normal
   pre-publication refusal restores the previous record, binary and tuple; after publication an
   install failure or interruption leaves detectable mismatch until a successful retry. Slot order
   alone cannot coordinate this because `05` ran before the build. The mismatch interval is not
   bounded to an install rename, and no audit grace or suppression is introduced.

A compiler or dependency change is authorized through the builder's record and tuple. Successful
installation converges the tuple and bytes; an audit during publication can still page. Editing the
desired osquery config also pages until the full apply lands, the existing source-intent behavior
CLAUDE.md records for templated targets.

### 3.3 The data files move

`posture-controls.json.tmpl` renders to `~/.local/libexec/posture/controls.json` (from
`~/.local/libexec/osquery/posture-controls.json`; the stutter goes because the directory now names
the tool), and the six desired-state files move to `~/.local/libexec/posture/converge/desired/`.
References that change with them: the poller's `CONTROLS_FILE` default
(`firewall-gatekeeper-monitor.sh:30`) until that script is deleted, the converge's staging path
(`osquery-converge.sh`'s desired-tree constant, and later the `Staging` adapter's path),
`treefmt.toml:118-126`'s four `osquery-config-render` globs, and the template's own header comment.
Decision 3 in section 8 asks whether the data moves at all.

The relocation must not weaken the converge's boundary. Today root reads installation bytes only out
of an unprivileged, per-run, mode 0700 private copy of the staging tree, never out of the deployed
tree (SI-7, S327, S332), and every component of the staging path is walked for a symlink before that
copy is made (SI-8, S325, S334). The new path `~/.local/libexec/posture/converge/desired/` is one
directory deeper and gains no privilege: the symlink walk covers the added component, the private
copy is still taken before any read root performs, and the 48 converge cases that pin those two
rules re-express against the new path in PR 7.1. A pull request that moved the tree and read it
directly, or that let `install -d` follow a link at the new depth, would weaken SI-7 or SI-8; the
specification's section 4 makes such a pull request name the weakening, and this plan expects none
to.

### 3.4 Plists, loaders and the two callers

Each plist's `ProgramArguments` changes from `/opt/homebrew/bin/bash` plus the script path to
`{{ .chezmoi.homeDir }}/.local/libexec/posture/posture` plus the subcommand
(`com.webdavis.osquery-results-alerter.plist.tmpl:7-11`). The loaders change by nothing but their
embedded hash, which is what re-fires them. The labels stay `com.webdavis.osquery-*`: renaming them
would touch the allowlist seed, the manifest's plist arm, the verdict's plist arm, the page renderer's
basename regex (`render-page.sh:75`, `:147`), the watchdog's six-agent list and the plist filenames,
which is a separate migration.

`run_after_50-setup-osquery.sh:46` changes its path and argv to `posture converge`, and its source
file is renamed to `run_after_59-setup-osquery.sh` in PR 7.1. Slot 59 follows builder 58, so the
apply installs the version that implements the new subcommand before invoking it. uu's brew lane runs the
converge as a program with no arguments (`dot_local/share/uu/src/lanes/brew/repairs.rs:75`,
`runner.run(&lane.osquery_converge, &[])`), so its `osquery_converge` key must carry argv rather
than one path; the proposed configuration array is `["<home>/.local/libexec/posture/posture",
"converge"]`, changed in uu's schema (`src/config/schema.rs:26`), its shipped template
(`dot_config/uu/private_config.toml.tmpl:81`) and its template test (`shipped_template.rs:101`) in
the same pull request as the converge cutover, per the pns charter's rule that a consumer changes in
the pull request that needs it. The operator sees that template change on `chezmoi diff`.

### 3.5 The justfile and CI (continuous integration)

`test-rust` (`justfile:167-175`) gains three lines for posture (test, fmt check, clippy with
`-D warnings`), and `just ship` picks them up through `just test`. No CI workflow change: the
toolchain step already ships cargo. `test/validate-tests.sh` is untouched: the Rust tests live in
the crate.

### 3.6 The manifest runner stays bash

`run_after_05` is a chezmoi runner that calls chezmoi and sudo, not one of the twelve entry points,
and it stays a plain script. It changes twice: the membership arm of section 3.2 and the build-record
arm of decision 2 in step 1, and the removal of the `osquery/*` arm in step 9 once that directory is
empty. Two callers invoke it by its source-relative path (`allowlist.sh:34`, `MANIFEST_RUNNER_REL`):
the allowlist writer, now from the `Publisher` adapter, and the posture builder after it writes the
build record (section 3.1).

### 3.7 ssh-hardening's surface

There is nothing to repoint, which is the point of keeping it operator-invoked: no plist, no chezmoi
runner, no justfile recipe, no LaunchAgent, and the port adds none. What changes when
`dot_local/bin/executable_ssh-hardening.sh` is deleted: `~/.local/bin` becomes EMPTY, so CLAUDE.md's
sentence "Today that leaves exactly one file in `bin` (`ssh-hardening.sh`)" and its "SSH hardening"
section are rewritten in that pull request to name `posture ssh`; the reload and lockout-recovery
procedure in `docs/runbooks/macos-fresh-machine-quickstart.md` is re-pointed; and the recovery
sentence every reload failure prints (S392) names `posture ssh rollback` in place of
`ssh-hardening.sh --rollback`, a deliberate wording change the pull request records. The deployed
`~/.local/bin/ssh-hardening.sh` is manifested under the managed-bin arm, so trashing it after the
apply is one expected CRIT page (S081), like every other retired file. The ssh knobs
(`SSH_HARDENING_VERIFY_DEADLINE`, the three readiness knobs, `SSH_HARDENING_SUDO` and the eight tool
and path seams of S372) stay environment reads, because they are operator-facing and documented; that
is an exception to the specification's section 3.11 construction-only rule of the same kind as the
converge's seam gate, and the exact list is proposed.

## 4. Rules every pull request in the ladder obeys

1. **Everything is new behavior, and every behavior lands red first.** A port has no pure moves.
   For a pinned statement the Rust test re-expresses the bats or bashunit pin by name, and the pull
   request carries a mapping table from the retired test to its successor (pns keeps that table in
   `dot_local/share/pns/docs/test-baseline.md`; posture keeps
   `dot_local/share/posture/docs/test-baseline.tsv` with the 186 case names recorded in step 1).
   For every UNPINNED statement or uncovered clause of a partial pin, the pull request first records
   a BASH-DERIVED ACCEPTANCE EXAMPLE: the
   exact input handed to the running bash function or script (sourced into a sandbox `HOME`, the way
   the bats harnesses do) and the output, exit status and files it produced, captured before any
   Rust for that statement is written and committed under
   `dot_local/share/posture/docs/acceptance/<statement>.md` (proposed) with the command that produced
   it. The Rust test then asserts that example, not the statement's prose, because the 399 statements
   are an inventory and not proof of parity: a test written from the prose alone checks the porter's
   reading of the bash, and the bash is what the machine has been running. Where the orphan fixture
   under `test/fixtures/` asserted the same behavior, the example cites it. The test fails for the
   stated reason, then passes. Every fix is mutation-checked by hand against an unmutated control,
   and the table goes in the pull request. A pull request may not delete the bash it captured from
   until its examples are committed, which the cutover rule below already implies.
2. **Gates.** `just test-rust`, `just lint-check`, the builder's build line, `just ship` before the
   pull request opens (a topic branch with no open pull request runs the suite nowhere), and
   `cargo test --locked --manifest-path dot_local/share/uu/Cargo.toml` on the two pull requests that
   touch uu.
3. **File size.** The paired Rust standard targets 200 implementation lines and 300 total; 250
   implementation or 400 total normally requires decomposition, and no handwritten `.rs` file
   exceeds 500 total, tests included. `main.rs` must remain below 150. Proposed sizes below that
   cross a decomposition threshold must be split before implementation is complete. Unit
   tests live in a sibling `<module>/tests.rs`, split by behavior when they pass the cap. The
   rationale that makes the bash 1.09 lines of comment per line of code moves into
   `dot_local/share/posture/docs/decisions/` records; production keeps a one-line invariant and a
   link. Count physical lines after `rustfmt` with this exact command, substituting the crate path:

   ```bash
   git ls-files '<crate-path>/*.rs' | while IFS= read -r f; do
     awk -v F="$f" '
       /^[[:space:]]*#\[cfg\(test\)\]/ && !seen { seen = 1 }
       !seen { impl++ }
       { total++ }
       END {
         if (F ~ /(^|\/)tests(\.rs|\/)/) impl = 0
         printf "%5d impl %5d total  %s\n", impl, total, F
       }' "$f"
   done | sort -k3,3rn
   ```

   Implementation lines precede the first `#[cfg(test)]`; `tests.rs` and files under `tests/` have
   zero implementation lines. Placing `#[cfg(test)]` above production code is itself a finding,
   never a way to reduce the count.
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
   restart, the privacy grants a probe binary needs, the hermes config edit, the KeePassXC entry, and the
   trashing of retired files. Agents propose; the operator applies.
7. **Repository rules.** Conventional Commits, no trailers, no em-dashes, `trash` never `rm` for
   operator files, never `chezmoi apply`, never a force push, at most two open posture branches.
8. **Clean code.** Every Rust brief, pull request and review names both
   `/Users/stephen/.agents/skills/clean-code/SKILL.md` and
   `/Users/stephen/.agents/skills/clean-code-rust/SKILL.md`. The Rust binding wins all numbers and
   mechanisms, including dependency boundaries, file sizes, test placement and gates. A brief
   omitting either absolute path is incomplete.

## 5. The migration order, and why

The order is by blast radius and test cover, smallest and best first, with the two constraints the
charter names (converge, which runs `sudo`, goes late; producers wait for the pns route):

1. **Deployment prerequisites first**, so every later file lands already covered by the
   file-integrity arm (step 1).
2. **The whole domain next**, because it is where the port's correctness lives and none of it can
   page, restart or write anything. It is also where unpinned behavior gets its first tests,
   against pure functions, which is the cheapest place to write them (step 2).
3. **Adapters and use cases without delivery** (step 3), then the two cutovers that need no
   delivery at all: the enricher, a child process with a two-value exit contract and the smallest
   blast radius in the tree, and the allowlist writer, an operator tool (step 4).
4. **The delivery adapter**, once the four pns pull requests of step 0 have merged (step 5). Two
   lanes until step 5: steps 1 to 4 deliver nothing and proceed in parallel with the pns work, both
   starting at step 0, and step 5 is where the lanes join. Only step 0's design items (the route
   overlap contract, the three-table queue check) sit ahead of step 1.
5. **Producers, smallest first**: heartbeat (109 lines, 17 tests, a daily silent message), digest
   (238 lines, 23 tests, daily), then the alerter (the WatchPaths agent, best-tested, largest), then
   the three untested producers (watchdog, poller, funnel monitor) each of which becomes tested for
   the first time in step 2, and last the drainer's retirement once the queue is empty (step 6).
6. **Converge last among the tools**, because it is the only one that calls `sudo`, restarts the
   root daemon, and writes under `/var/osquery`; it also has the best cover (48 bats plus 18
   bashunit cases), so its port is a translation of a well-pinned tool rather than a discovery
   (step 7).
7. **ssh-hardening after the converge** (step 8), because it is the other tool that runs `sudo`,
   the only one that can lock the operator out of the machine, and the one whose watchdog nothing
   pins today; it shares nothing with the pipeline but the binary, so it waits for everything else.
8. **Cleanup** (step 9).

## 6. The ladder

Each pull request: **Statements** moved behind Rust; **Tests** (which pins are re-expressed by name,
which UNPINNED statements get their first test); **Surface** (the deployed references that change);
**Cutover** (what the operator verifies and does); **Sizes** (projections; the pull request reports
implementation and total lines with section 4's post-formatting command and splits again if needed).
A pull request with no **Cutover** row deploys source and changes no plist or caller.

### Step 0: the pns lane, and what is settled before PR 1.1 opens

None of this is a posture pull request. Items 0.1, 0.2 and 0.5 are the pns lane: they run beside
steps 1 to 4 and gate step 5. Item 0.3 is a design contract code depends on, so it is settled before
PR 1.1 opens; item 0.4 is settled before step 3.

**0.1 The durable acknowledgement.** pns PR 7.3 (`pns submit --json` and its result envelope) and
PR 11.4 (the ledger row written before dispatch) merge, and the envelope reports the committed row,
so `accepted` means a retriable obligation for that request id (spec section 5.2). Verified by the
pns program's own tests for the crash window between dispatch and record, which PR 11.4 names.

**0.2 The priority-route pull request**, sized by the specification's section 5.5, delivering the
route, the heartbeat and digest preservation (item 2), and the ledger and daemon readable without the
daemon's help (item 4).

**0.3 The mixed-producer route transition.** The first draft re-keyed the `priority` route and
changed its body in one step, while every bash producer still POSTs the old body under the old key
until its cutover in step 6, and every retry already queued in the SQLite store carries the old body
and is signed with the old key by the drainer (S145, S165 to S169). Re-keying the route would turn
each of those into a 401 and, after the permanent-status rule, a dead-letter (S168). So the pns-keyed
route is ADDED beside `priority`, the two run in parallel for the whole of step 6, and `priority`
retires with its key file only in PR 6.7, after the queue is confirmed empty. The empty check covers
all three tables, not the two the counters read (S176): `pending_alerts` and `dead_letter_alerts`
through the two library functions, and `pending_local_notifications` through a `sqlite3 -readonly`
count, because a banner still queued for redelivery (S181, S184) is a page the operator has not seen
and the counters do not report it. Dead letters never drain: the operator must inspect each row,
record its delivery outcome or explicitly acknowledge it as undelivered, and preserve a reviewed
export before authorizing removal of exactly those rows. That removal requires fresh scoped
confirmation; the port adds no automatic purge. Pending remote and local notifications must drain
under the old route. Until the three independent counts are zero, the old route, key, drainer and
queue remain in service with the watchdog's legacy queue probe and drainer monitoring, and PR 6.7
is blocked. PR 6.4 adds pns monitoring alongside those checks; PR 6.7 removes the legacy checks and
the retired drainer's monitored label together with the queue.

**0.4 The independent pns checks are designed**, as spec section 5.2 requires: the tuple for the
deployed `pns` binary under decision 2's authorized-build rule (the pns builder writes the same kind
of record posture's does), the launchd read of `com.webdavis.pns-daemon`, and the read-only ledger
probe and its direct `IndependentAlarm` path (the existing bounded local banner, independent of
`AlertSink` acknowledgements). Designed here, built in PR 6.4, because PR 6.4 is where probe 4 is
re-expressed and the design
must exist before the use case it feeds is written in step 3.

**0.5 The delivery class.** The pns pull request that adds a class to `pns submit --json` and a
config key naming which classes cut through the mute and a configured Focus for the banner and the
phone card, with the hermes leg unchanged (spec section 5.5, item 6; decision 4). The key ships in
pns's config template with its default explicit and uncommented. It changes pns S103 (the mute beats
every producer), so its tests are pns's; posture's side is one field on the request, set for every
security `NeedsAttention` and never for the heartbeat or the digest, and PR 5.1 tests that.

### Step 1: the workspace and the deployment prerequisites

**PR 1.1 the workspace and the builder.** Creates `dot_local/share/posture/` with the five member crates
(each `lib.rs` a doc comment naming its responsibility), `Cargo.lock`, the `posture` binary printing
usage and exiting 2 on every word (S298, S341), `docs/README.md`, and `docs/test-baseline.tsv` holding
the 186 bats and bashunit case names plus the one plain-script ssh-hardening test as the set to map from.
Adds `run_onchange_after_58-build-posture.sh.tmpl` (slot proposed, section 3.1) with its build record and
scoped pipeline refresh, `test/unit/posture-build-install.test.sh`,
`test/unit/posture-manifest-refresh.test.sh`, and the three `test-rust` lines. The minimal coupled
`--pipeline-only` option in `run_after_05` belongs here because it makes the builder's rollback contract
true while preserving all existing default callers (section 3.1). The release profile is set and the
artifact is measured against the audit's 8 MiB bound (section 3.1); the number goes in the pull request.
**Tests**: the build-install test, including the record-refresh- install ordering, refusal to publish
empty or oversized artifacts, prior record/binary/tuple preservation on refresh refusal, install-failure
retry, and default versus pipeline-only runner behavior; a cli test that every subcommand word is refused
with usage and exit 2 until it exists. **Surface**: the builder, the justfile. **Sizes**: `main.rs` under
60; the rest are doc comments. **Order**: first, once step 0.3 is settled; it does not wait for the pns
lane. Until PR 1.2 lands the binary sits under `~/.local/libexec/` as an untracked neighbour, as `pns`
and `uu` do today, so nothing pages.

**PR 1.2 file-integrity coverage of `~/.local/libexec/posture/`.** The five edits of section 3.2
(membership, classification, `_pipeline_manifest_for`, both watch arrays) plus the build-record arm
of decision 2 in `run_after_05`, with its four rules: digest from the record only, tuple retained when
no build ran, explicit unbuilt state for a missing record, and publication coordinated by the
builder. The Bash audit and tuple lookup learn the unbuilt record in this same pull request; later
Rust codecs inherit that contract.
**Tests**: none of the tracked-set copies is in test scope (the 2026-08-05 ruling); the pull request
records the five diffs side by side. The record arm IS in scope, because it is logic this repository
wrote: `test/unit/posture-manifest-record.sh` (proposed) drives the runner over a sandbox record and
asserts the tuple for each of the four rules, including that a record whose digest is not 64 hex
refuses the whole manifest the way S356 refuses an implausible hash. Test an absent binary, a
zero-byte regular file with the expected mode and owner, and a forged `unbuilt` event digest: none
may become known-good. Mutation-check both consumer refusals and the runner publication rules.
**Cutover**: the operator
applies; the manifest gains the binary tuple from the record PR 1.1's builder wrote on the earlier
apply; nothing pages, because the deployed binary is the one the record describes. If the earlier
build was deferred (no cargo yet), the explicit unbuilt record is written and the audit pages `missing`
on
every tick until the build lands, which is stated here as the expected cost on a fresh machine. The
desired `osquery.conf` change restarts osqueryd through the converge on that apply.

### Step 2: the domain, red first

These pull requests implement pure policy in `posture-domain`, with no input/output or cutover. PR 2.4
also implements the existing digest record codec in `posture-protocol`; serialization stays outside the
domain. Each carries sibling unit tests; every unpinned statement or clause gets a Bash-derived
acceptance example before its first test.

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

**PR 2.4 the page, the digest record and the digest.** Domain `page.rs` (S124 to S133) and `digest.rs`
hold derived facts, grouping, sanitization and caps (S117, S120, S286 to S294).
`posture-protocol/src/digest_record.rs` owns the existing six-field record encoding and decoding,
including wrong-shape coercion and malformed/torn-line behavior. Domain and application acquire no JSON
or protocol dependency; adapters map the wire values when the spool reader and writer land. The current
unversioned shape stays compatible with existing Bash writers, readers and on-disk spools. **Tests**: the
eleven `osquery-render.bats` cases, eight digest-store cases and eleven grouping and cap cases of
`osquery-digest-builder.bats` map by name to domain or protocol tests according to the behavior. Capture
Bash-produced record bytes and Bash reader outcomes before Rust; assert both directions of wire
compatibility and mutation-check a dropped field and changed decoding rule. No test merely round-trips
the new codec against itself. The apostrophe rule (SI-13) remains on output strings. **Sizes**: `page.rs`
decomposes its projected 280 implementation lines into page parts before completion, with tests split by
headers, fields and caps; domain `digest.rs` and protocol `digest_record.rs` each target 200
implementation and 300 total lines, with private sibling test files where needed.

**PR 2.5 the canary and the heartbeat's wording.** `canary.rs`: S193 to S204, S211's freshness
rule. **Tests**: the seventeen `osquery-heartbeat.bats` cases by name. **Sizes**: ~150 plus tests
~300.

**PR 2.6 the watchdog's decisions and the audit.** `watchdog.rs` (S207 to S215, S217 to S226 over
readings), `audit.rs` (S227 to S240 over manifest lines and per-path readings). **Tests**: all of
them are first tests; the 41 functions of `test/fixtures/osquery-watchdog-lib.bash` and the 17 of
`osquery-manifest-lib.bash` are the source of the cases. Capture a finding followed by a refusal in
both the same and second manifest: Bash retains the earlier output, and the watchdog classifies the
mixed report as `unknown` before its normal two-tick alarm decision (S227, S230). Preserve that
behavior rather than adding buffering. **Sizes**: `watchdog.rs` ~280 plus tests
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
all first tests except S130's render-side pin. S102 requires a Bash-derived example from the actual
triage producer, including its three string members and quoting; an injected rendering fixture does
not cover it. **Sizes**: three files of 120 to 220 plus tests under 350.

### Step 3: adapters and use cases, without delivery

**PR 3.1 the file adapters and the locks.** `results_log.rs`, `state_files.rs`, `locks.rs`,
`known_good_file.rs`, `deployed_state.rs`, `allowlist_file.rs`, `controls_file.rs`,
`upgrade_record.rs`, `clock.rs`. **Tests**: adapter tests over temporary files: the torn-line read
(S011), the rotated-log inode (S006, S008), the rename claim under two processes (S282), the whole-
file JSON refusal with a trailing `{}` (S209), the trust check on a mode (S091), the symlink refusal
before hashing (S097), the named-pipe refusal on the upgrade record (S108), the `flock` contention with a
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
**Surface**: `route.sh:115`'s `OSQUERY_ENRICH_SCRIPT` default becomes the posture executable
path alone. Its `-x` check continues to test a filename; invocation at `:155` changes to
`"$enrich_script" enrich "$ep"`. Update every injected script double to the same argv contract in
this pull request. **Tests**: a recording executable sees exactly `enrich` and the unsplit path;
exit 10 still promotes NOTICE to CRIT, while exit 0 preserves NOTICE. These tests drive the real
Bash call site with a fake executable, including a path containing spaces. `executable_enrich-finding.sh`
is deleted. **Cutover**: the operator applies, then runs
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
backslash-first literal) and S183 (the fixed loud sound). The same banner implements the
application-owned `IndependentAlarm` port for watchdog pns integrity/health findings in PR 6.4.
**Tests**: a scripted `pns` stub answering `accepted` with the committed-row diagnostic, `accepted`
WITHOUT it, `degraded`, a refusal, garbage and nothing; the durable bit follows the committed-row
diagnostic alone, never the delivery outcome and never the bare word `accepted`; a stub that hangs is
killed at the deadline and reported as not accepted; in the producer path the banner spawn happens on
exactly the
not-accepted-because-the-engine-failed path (absent, nonzero, timeout, unparseable) and never on a
clean refusal or a missing diagnostic; the key never appears in argv (there is none). **Order**:
after the four pns pull requests of step 0 have merged; this is where the pns lane joins the
ladder. **Sizes**: `pns_producer.rs` ~200 plus tests ~350; `last_resort_banner.rs` ~80 plus tests
~120.

### Step 6: the producer cutovers

Each pull request lands one subcommand over one use case, repoints one plist, and deletes the bash
it replaces together with the tests that pinned it (their names appear in the mapping table with
their Rust successors). None opens before the four pns pull requests of step 0 have merged and PR
5.1 has landed on them.

**PR 6.1 `posture heartbeat`.** **Statements**: S193 to S206. **Surface**: the heartbeat plist,
`executable_heartbeat.sh`, `test/integration/osquery-heartbeat.bats`. `canary-freshness.sh` stays
until the watchdog cutover, because the watchdog still sources it. **Cutover**: the operator applies,
runs `posture heartbeat` by hand once (a silent Discord line reading "observed" arrives on the
pns-keyed route, and a silent banner on the desk), confirms the pns ledger recorded it, then trashes
the retired script.
**Sizes**: `heartbeat.rs` (application) ~120; cli under 40.

**PR 6.2 `posture digest`.** **Statements**: S280 to S297. **Surface**: the digest plist,
`executable_digest.sh`, `test/integration/osquery-digest-builder.bats`. **Cutover**: apply; the
operator runs `posture digest` by hand against a spool the day has filled (or an empty one, which
must produce nothing and exit 0), confirms the `.last` rotation and the single silent message, and
trashes the script. **Sizes**: `build_digest.rs` ~180 plus tests ~300.

**PR 6.3 `posture alert`.** The largest cutover. **Statements**: S001 to S068 (the entry and its
four stages), S069 to S133 (the verdicts, triage, spool line and render, now called from the use
case), S360, S363 to S366 as routing facts. **Surface**: the results-alerter plist;
`executable_results-alerter.sh` and six private files in `results-alerter/`; keep
`results-alerter/pipeline-verdict.sh` deployed because Bash `pipeline-audit.sh` still sources it
and otherwise refuses BOTH manifest scans as unavailable. That last helper retires in PR 6.4;
`test/e2e/osquery-alerter-criteria.bats`, `osquery-alerter-hostile-columns.bats`,
`osquery-alerter-concurrency.bats`, `test/unit/osquery-route.bats`, `osquery-render.bats`,
`osquery-normalize-and-digest-store.bats`; the comment at
`dot_local/share/uu/src/lanes/brew/upgrade_record.rs:8` that names the triage helper's path. The
concurrency pins (S001, S002, S004) are re-expressed as a two-process test over the real lock path
in a sandbox. **Cutover**: the operator applies with the results log quiet, watches one tick of
`posture alert` under the WatchPaths trigger by touching nothing and reading the agent log, then
plants one known finding (a new user LaunchAgent that is not allowlisted) and confirms the page
arrives through pns and the cursor advanced by exactly one record, then removes the plant and
trashes the seven retired files, retaining the directory and `pipeline-verdict.sh` until PR 6.4.
**Sizes**: `judge_results.rs` ~280 plus `judge_results/tail.rs`
~120 (the checkpoint and delivery tail), tests split by stage into four files under 400.

**PR 6.4 `posture watchdog`.** **Statements**: S207 to S240. **Surface**: the watchdog plist,
`executable_uptime-watchdog.sh`, `executable_pipeline-audit.sh`, `executable_canary-freshness.sh`,
`results-alerter/pipeline-verdict.sh` (its last Bash consumer retires here),
`test/fixtures/osquery-watchdog-lib.bash`, `osquery-manifest-lib.bash`. Probe 4 (S216) continues to
read the legacy queue through the temporary read-only `LegacyQueue` adapter. It preserves unreadable
pending/dead-letter counts as failures, any existing dead letter as a finding, and pending growth
across two consecutive ticks, including the prior Bash growth history. The six-agent roster still
includes `com.webdavis.osquery-alert-drainer`. These legacy checks remain through PR 6.7 because the
drainer only alarms on rows dead-lettered during its current pass. Pns monitoring is ADDED alongside
them (spec D4 and section 5.2), designed in step 0.4, and none of it asks pns:
the deployed `~/.local/libexec/pns/pns` is judged against its known-good tuple through the same
`KnownGood` and `DeployedState` ports the manifest audit uses; `LaunchdState` reads
`com.webdavis.pns-daemon` and pages it unloaded or not running, exactly as probe 2 reads the six
agents (S212); and the application-owned `PnsLedger` port is implemented by the
`posture-adapters` read-only `rusqlite` reader from section 2.4, with a bounded busy timeout, no
create and no immutable mode (S178's rule), and pages an unreadable ledger, any dead-lettered row, and an
undelivered backlog that grew across two consecutive ticks (S216's own shape, over the new store).
Executable presence is asserted by none of these and is not a probe. **Tests**: retained Bash examples
and Rust tests cover unreadable legacy counters, an existing dead letter with no newly dead-lettered
rows, two consecutive pending increases and missing drainer state while the pns store stays healthy;
keep the stores' growth histories independent. Add first tests for each of the three pns checks over
the scripted runner and a sandbox ledger, including missing, corrupt, locked
and incompatible ledgers returning unreadable instead of zero, and committed rows still in the
write-ahead log contributing to the count. Each integrity or health finding must invoke the
independent banner even when a fake engine returns a valid `Accepted` while delivering nothing.
Mutate the direct alarm call and show that test fails; a failed independent attempt must retain
retry eligibility and cannot be overwritten by the engine result. **Cutover**: apply; the operator reads
one healthy
tick's silence in the agent log, then stops the digest agent by hand (`launchctl bootout`) for one
tick, confirms the "not loaded" page, bootstraps it back, and trashes the retired files. **Sizes**:
`watchdog.rs` (application) ~260 plus tests ~350.

**PR 6.5 `posture poll`.** **Statements**: S241 to S267. **Surface**: the poller plist,
`executable_firewall-gatekeeper-monitor.sh`, `test/fixtures/osquery-poller-lib.bash`; the controls
file path per section 3.3. **Cutover**: apply; the operator confirms the first tick reads the existing
baseline (no first-observation page), then toggles one control the safe way (start and stop the
OverSight process) across two ticks and reads the page and the silent recovery. The probe binaries
`fdesetup`, `csrutil`, `sysadminctl`, `defaults`, `pgrep`, `plutil` and `readlink` need no privacy grant;
`plutil` reading LuLu's rules archive under `/Library/Objective-See` reads a world-readable file
today and the port changes nothing about who reads it. **Sizes**: `poll.rs` (application) ~280 plus
tests split into three.

**PR 6.6 `posture funnel`.** **Statements**: S268 to S279. **Surface**: the funnel plist,
`executable_tailscale-monitor.sh`, `test/fixtures/osquery-tailscale-lib.bash`. **Cutover**: apply;
the operator confirms the existing `inactive` baseline is read as such (no first-observation page),
then runs `tailscale funnel status --json` by hand to confirm the reader's input matches what the
adapter parsed in the agent log. **Sizes**: `funnel.rs` ~140 plus tests ~220.

**PR 6.7 the drainer and the dispatch library retire.** Removes the temporary Rust legacy queue reader,
its probe and growth tracking, and `com.webdavis.osquery-alert-drainer` from the watchdog's
monitored roster in this same retirement change. Preserve the other five labels and all pns checks.
**Tests**: a healthy post-retirement tick with the old queue and drainer absent produces no legacy
health finding; a missing remaining agent or unhealthy pns ledger still produces its finding.
Mutation-check restoring the retired label and disabling a remaining probe. **Surface**: the drainer
plist and its loader, `executable_drain-undelivered-alerts.sh`, `executable_alert-dispatch.sh`,
`test/unit/osquery-alert-dispatch.bats`, `test/integration/osquery-drain-continuation.bats`,
`test/helpers/build-dispatch-harness.sh`. **Cutover**, and this one has a precondition: before the
apply, the operator confirms all THREE tables are empty, not the two the counters read: `pending_alerts`
and `dead_letter_alerts` through the two library functions sourced into a shell, and
`pending_local_notifications` through `sqlite3 -readonly` over that table, because a banner queued for
redelivery (S181) is a page the operator has not seen and no counter reports it (step 0.3).
Dead letters require the step 0.3 operator review, preserved export and fresh confirmation of
exact-row removal; the existing drain never empties that table. Re-read all three counts after
that disposition. An unresolved row blocks retirement, with the route, key, drainer and queue
retained. Only then
does the operator retire the old `priority` route and its key in the hermes config, apply, and trash
the retired files, the queue's three files, `osquery-spool/`, `osquery-tailscale-funnel` (the two
leftovers of spec section 3.10) and `~/.config/osquery/webhook-secret`. **Order**: after PR 6.1 to
6.6, because every producer must have stopped writing the queue first, and the old route must outlive
the last queued retry.

### Step 7: the converge

**PR 7.1 `posture converge`.** The cli subcommand over the `Converge` use case with the real
`privileged.rs` behind the scripted runner in tests. **Statements**: S311 to S349, S368.
**Tests**: the 48 `osquery-converge.bats` cases by name, re-expressed against a sandbox target
directory with a stub `sudo` and stub `osqueryctl` exactly as the bats harness does today, plus the
restart-evidence cases over a fake `ProcessTable` and fake `Clock` with no sleeps. The test seam gate
(S330, S331) becomes the one environment variable posture reads, kept because a harness that set only
some of the seams would otherwise converge the live machine. **Surface**: rename
`run_after_50-setup-osquery.sh` to `run_after_59-setup-osquery.sh` and change
its invocation at `:46`; uu's
`osquery_converge` key per section 3.4 (its schema, template, template test, and `repairs.rs:75`);
`executable_osquery-converge.sh`, `osquery-converge/drift-verdict.sh`,
`test/unit/osquery-converge.bats`, `test/unit/osquery-converge-drift-verdict.test.sh`. The desired
tree stays where PR 1.2 or decision 3 put it. **Cutover**: the operator applies; the apply itself
runs builder 58 before `posture converge` through `run_after_59` and, with nothing drifted,
prints nothing. A sandbox composition test starts with the prior binary refusing `converge`, runs
the rendered builder with fake cargo, then runs the repointed caller against the newly installed
fake binary and verifies the invocation succeeds. This tests the scripts' behavior without a live
apply. After confirming the quiet no-op, the operator records the current osqueryd parent pid and
`/usr/bin/stat -f '%Lp' /var/osquery/osquery.flags` (expected `644`), then runs
`sudo /bin/chmod 0600 /var/osquery/osquery.flags` and verifies the mode reads `600` before proceeding.
The operator re-runs the apply, reads the repair line and daemon restart, verifies the mode is back
to `644`, and confirms osqueryd's parent pid changed and stays stable through the settle window. The
drift and restart are the operator's. **Sizes**: cli `converge.rs` under
80; the application and adapters were measured in step 3.

### Step 8: ssh-hardening

Operator ruling 2026-09-06. Three pull requests, after the converge and before cleanup, with the
watchdog pinned by a test before the bash is deleted, because nothing pins it today (spec S381).

**PR 8.1 the ssh policy domain.** `ssh_policy/directives.rs`, `tokenizer.rs`, `include.rs` and
`tree.rs` in `posture-domain`. **Statements**: S370 (the directive table), S375 and S376 (the
judgement and the alias fold), S377 (both tokenizers), S378 (the Include pattern analysis, including
the `^` refusal), S379's refusal set and S380's root rule as decisions over readings, S389 and S390
(knob and port validation), S393's host-key record shape, S395's record grammar and S396's
comparison. **Tests**: S370 and S371 re-express the two `ssh-hardening-dropin.sh` criteria by name;
every other statement gets its first test, and the tokenizer's cases are the measured forms the bash
comments record (`executable_ssh-hardening.sh:658-683`, `:809-812`, `:831-835`, `:844-850`,
`:874-876`, `:931-944`, `:963-968`, `:979-982`, `:994-997`, `:1036-1045`, `:1058-1061`), each captured
as a bash-derived acceptance example by running `parse_config_line` and `resolve_include_paths` in a
sandbox before the Rust test is written (rule 1). Include S377's malformed trailing argument
example: `PasswordAuthentication yes "unterminated` retains `[yes]` and returns 0; do not
silently change it to a dropped line. **Sizes**: four files of 150 to 250 plus tests
under 400 each.

**PR 8.2 the bounded runner and the ssh adapters.** `bounded.rs`, `sshd.rs`, `keyscan.rs`,
`sshd_tree.rs`, `privileged_fs.rs` and the `Launchctl` print and kickstart calls in `posture-adapters`.
**Statements**: S381, S382 (the child verify becomes an in-process call over the same bounded `Sshd`,
since one binary needs no re-exec to get a fresh `set -e`), S383's five privileged operations, S395's
observation. **Deliberate corrections**: every direct verify call uses the bounded adapter (Bash leaves
two unbounded), and enumeration preserves newline-bearing paths until they can be refused (Bash can omit
them). Capture those Bash counterexamples before writing the new expectations; they are section 6.1
changes, not parity claims. **Tests**, and this is the pin the bash lost: a stub child that ignores TERM
(`trap '' TERM; sleep 600`) and has started a grandchild is stopped at a short deadline, the whole group
is gone afterwards (the grandchild's pid answers ESRCH), the outcome is `Timeout` and the caller sees 124
where the bash exposed it, and the runner returns inside the deadline plus the 2 s grace plus a
tolerance. The process-adapter test uses injected short deadline and grace durations to finish within the
one-second gate; a fake-clock test checks the production two-second grace decision. A healthy child's
status passes through unchanged; the child reads end-of-file on stdin; an observation refuses a path
with a newline or the unit-separator byte, a non-regular file at read time, and a tree past the byte or
visit bound, and follows a symlinked include for its attributes.
**Sizes**: `bounded.rs` ~180 plus tests ~300;
the four adapters 80 to 200 each plus tests.

**PR 8.3 `posture ssh` and the cutover.** The six use cases over the ports of PR 8.2, the cli
`ssh.rs`, and the install's signal handling (S385) as the use case's own concern. **Statements**:
S369 to S399, with D15 and section 6.1's explicit universal-bounds and newline-refusal corrections.
**Surface**: `dot_local/bin/executable_ssh-hardening.sh`,
`test/unit/ssh-hardening-dropin.sh` and `test/fixtures/ssh-hardening-lib.bash` are deleted; CLAUDE.md's
"exactly one file in `bin`" sentence and its "SSH hardening" section, and the quickstart runbook's
reload and lockout-recovery procedure, are rewritten to name `posture ssh` (section 3.7); the
recovery sentence (S392) names `posture ssh rollback`. No plist, no chezmoi runner and no justfile
recipe are added: it stays operator-invoked. **Cutover**: BEFORE the apply that deletes the script
the operator runs `posture ssh verify` and `ssh-hardening.sh --verify` against the live tree and
compares the PASS lines, and diffs `posture ssh print-config` against `--print-config` byte for byte;
then applies and trashes `~/.local/bin/ssh-hardening.sh` (one expected CRIT page, S081). No reload is
part of the cutover: `posture ssh reload` is disruptive and only ever runs because the operator typed
it, and the drop-in on disk is unchanged by the port. **Sizes**: `ssh_install.rs` ~260 plus tests
~350; `ssh_reload.rs` ~280 plus tests ~380; `ssh_verify.rs`, `ssh_rollback.rs` and the two print use
cases under 150 each; cli `ssh.rs` under 80.

### Step 9: cleanup

**PR 9.1 the old directory leaves the tracked set.** Once `~/.local/libexec/osquery/` holds nothing
(the operator has trashed every retired file), the three `osquery/*` arms of section 3.2 are removed
from the watch paths, the manifest runner and the (now Rust) tracked set, and `run_after_05`'s
docblock names the new directory. **Cutover**: apply, one osqueryd restart through the converge for
the watch-path change.

**PR 9.2 the file-size lint and the completion record.** `scripts/treefmt/rust-file-size.sh` over
`dot_local/share/posture/**/*.rs` at the 500 cap, if the pns program's PR 18.1 has not already added
a shared one; the completion report with the before-and-after line table (spec section 9 against
the crate), the test mapping table complete (187 retired names, each with a successor or a reason),
and the decision records index.

## 7. Operator-only steps, collected

Every one of these is the operator's and never an agent's:

- Every `chezmoi apply` in the ladder, with KeePassXC unlocked (fourteen templates read it).
- The osqueryd restarts: PR 1.2 and PR 9.1 change the desired `osquery.conf` and the converge
  restarts the daemon on that apply; PR 7.1's planted drift restarts it again.
- The ssh cutover (PR 8.3): running `posture ssh verify` and `posture ssh print-config` beside the
  bash's `--verify` and `--print-config` before the apply that deletes the script, and trashing
  `~/.local/bin/ssh-hardening.sh` after it. No reload is part of the cutover; `posture ssh reload`
  is disruptive and only ever runs because the operator typed it.
- Step 0's hermes edit: ADD the pns-keyed route for posture beside the existing `priority` route in
  `private_dot_hermes/encrypted_private_config.yaml.age` (age identity required), leaving `priority`
  and its key untouched for the whole of step 6; then, in PR 6.7 and only after the three-table
  check, remove the `priority` route and trash `~/.config/osquery/webhook-secret`.
- Decision 4, once taken: if (b), setting the mute bypass for posture's `NeedsAttention` in the
  operator's own pns config.
- The live verification of each cutover, as its **Cutover** row states, before the pull request that
  deletes the bash merges.
- Trashing each retired deployed file after its apply (chezmoi deletes nothing), and reading the one
  expected CRIT page per trashed file.
- Confirming all three queue tables are empty before PR 6.7's apply (the two counters plus a
  read-only count of `pending_local_notifications`), with reviewed dead-letter export and
  disposition requiring fresh confirmation before exact-row removal. No automatic purge.
- Privacy permissions: no posture subcommand needs a grant the bash did not already have. `osqueryd`
  keeps Full Disk Access as today; `posture poll` reads the same world-readable files the poller reads.

## 8. Decisions the operator makes before code moves

Two options each. Four were reviewed on 2026-09-06 and stand with the conditions recorded under each;
the fourth is reopened and awaits the operator.

1. **How posture delivers.** (a) Posture is a pns producer through `pns submit --json`; pns owns the
   ledger, the retries, the presence gate, the phone and the replay; posture keeps one last-resort
   banner for engine-down and independent pns integrity/health findings (spec section 5.2). (b) Posture
   ports the dispatch library as its
   own `AlertSink`: a second SQLite queue, drainer, dead-letter policy and signer key, with no
   presence gate, phone, replay or recap (spec section 5.3). Decision: (a), with four conditions.
   The `accepted` bit must mean a committed, retriable obligation for that request id; today pns
   dispatches before it records and drops a failed journal write (`main.rs:3101-3122`, `:814-858`),
   so PR 7.3 and PR 11.4 gate step 5 and step 6, the first code that submits, and run as a lane
   beside steps 1 to 4 (step 0.1). Posture keeps
   independent integrity and health checks on pns (the binary's tuple, the daemon's launchd state,
   the ledger read-only), because (a) gives up the failure isolation (b) had and the engine-down
   submit-failure branch covers detectable failure only. Independently detected pns integrity and
   health findings raise the same banner directly, regardless of a forged `accepted` (spec section
   5.2, PR 6.4). The route transition runs both routes in parallel until the queue is drained (step 0.3).
   And every security page carries the delivery class of decision 4, whose pns pull request is the
   fourth prerequisite (step 0.5). "Better on every axis" is withdrawn as the reason: (a) is chosen
   because pns is the one notification engine by ruling, and the lost isolation is paid for by the
   checks.
2. **Whether the built binary is vouched for by the file-integrity arm.** (a) The manifest runner
   writes one tuple for the deployed binary, so a swapped binary pages like a swapped script does
   today. (b) The binary lives under a tracked directory as an untracked neighbour, the way the `pns`
   and `uu` binaries do today, and the file-integrity arm is blind to it. Decision: (a)'s coverage,
   with the mechanism of the first draft replaced. The digest comes from an authorized build's record,
   never from `target/release/posture`, which an out-of-band build can change while the onchange
   builder skips the install; the tuple is retained, not recomputed, when no build ran; an absent
   artifact gets an explicit unbuilt record so the manifest-enumerating audit sees the path without
   vouching for empty bytes; installation and
   publication are coordinated inside the builder (record, refresh, install), because slot order
   cannot do it; the build does not move to `03`, because a compile ahead of `05` would hold every
   other manifest past the verdict's 5 s settle budget and page false CRITs (section 3.1, section
   3.2). The sibling `pns-protocol` crate and the compiler identity are build inputs, and the
   artifact is measured against the audit's 8 MiB bound.
3. **Where posture's files live.** (a) `~/.local/libexec/posture/` for the binary, `controls.json`
   and `converge/desired/`. (b) Everything stays under `~/.local/libexec/osquery/` beside the scripts
   it replaces, touching no watch path or manifest arm. Decision: (a). The binary, the controls and
   the desired state form one tool's directory, which is the CLAUDE.md shape (a directory names a
   system, and `osquery/` was a permitted domain name too, so the naming rule alone did not decide
   it). The cost is not "three one-line edits": it is five edits to the tracked-set copies, with
   `_pipeline_manifest_for` (`pipeline-verdict.sh:278-284`) the one the first draft missed and the
   one whose omission would page every posture file, plus the callers and render globs of section
   3.3, and the relocation must leave the converge's symlink refusal and its private 0700 copy
   exactly as strong at the deeper path (section 3.3).
4. **Whether a security page honours the operator's mute.** SETTLED by the operator on 2026-09-06:
   (b). (a) would have let a muted or Focus-silenced page reach the hermes leg (Discord) and nothing
   else, journaled as a miss and caught up by a card only on the next event after the mute lapsed
   that earned the operator something (`routing.rs:107-114`, `missed_notifications.rs:91-93`; pns
   S106), so Discord was routing intent rather than a ping and the replay was event-driven, not timed
   to the mute's end. The funnel detector reports a service newly exposed to the public internet and
   says "close it now" (`executable_tailscale-monitor.sh:132`), so an hour of Focus would extend an
   unintended exposure by an hour, and a tamper page on a tracked path has the same shape; "every
   existing page tolerates an hour" was asserted, not shown. So (b): a security page cuts through the
   mute and a configured Focus for the banner and the phone. The bypass is a pns feature, not a
   posture one: posture is a producer and cannot deliver around pns's mute, so it marks every
   security `NeedsAttention` with a delivery class (`security`, proposed, under pns's config naming),
   and pns's routing lets events of a class the operator's config names through, with the hermes leg
   unchanged. The switch lives in pns's config, shipped with its default explicit and uncommented, and
   its implementation is the pns pull request of step 0.5, a prerequisite beside the durability ones.
   Posture's own documents say only this: security `NeedsAttention` pages are submitted with that
   class, the heartbeat and the digest are not, and the quiet treatment for observations is
   unchanged. Spec section 5.2 and section 5.5 item 6 carry the same wording.
5. **Whether the port changes product behavior along the way.** (a) The port makes the delivery
   changes listed in spec section 6.1 and no others: D1 to D7 and D9 as stated there, the presence
   gate as a new surface, and whatever decision 4 settles; the heartbeat's and the digest's daily
   Discord lines and every ordinary silent banner are preserved (spec section 6.2); option E of the
   allowlist boundary design (own-agent suppression by manifest membership, retiring the
   empty-`sha256` convention) and any change to the detector set or the page wording are separate
   pull requests after the port, each with its own evidence. (b) Fold option E into PR 2.3, since the
   verdict is being rewritten anyway. Decision: (a), reworded. The first draft said "none", which was
   false: D3 replaces banner retries with presence replay, D4 changes what the queue-health probe
   reads, D9 removes an observable fallback, and each is now named as deliberate. What made "none"
   dangerous rather than only wrong is that the first draft's `Observation` (durable log and nothing
   else) would have removed the heartbeat and digest Discord messages that `send_alert` sends today
   for every `CRIT` regardless of sound (`executable_alert-dispatch.sh:1186-1193`), and that was
   never approved. The 399 statements are an inventory, not proof of parity; rule 1 of section 4 now
   requires a bash-derived acceptance example for every UNPINNED statement before its Rust test.

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
