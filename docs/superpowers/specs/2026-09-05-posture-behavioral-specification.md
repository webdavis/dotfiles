# posture behavioral specification

Recorded 2026-09-05 against `origin/main` at `b5412089`, from the twenty-one shell files under
`dot_local/libexec/osquery/`, the two chezmoi runners that serve them, the seven LaunchAgent plists,
and the eleven bats files plus one bashunit file that test them. `posture` is the operator's name for
the Rust program that replaces those scripts: it watches and reports this machine's security posture,
and osquery is the vendor daemon it drives. This document states what the pipeline does today, not
what posture should do differently. Where the code and this document disagree, the code is right and
the document is the defect.

The statements in section 8 (S001 to S368) were inventoried the same day against the working tree at
`aef04428` and re-checked here by sampling ten citations against `b5412089`; the six pipeline files
that changed between the two commits changed in comments only, and every sampled line number still
lands on the symbol it names. The plan at `docs/superpowers/plans/2026-09-05-posture-port-plan.md`
moves code against these statements one pull request at a time.

## How to read it

Sections 1 to 7 are the contracts a port has to honour: what each entry point is called by and what
it owes back (1, 2), the exact shape of every file two parties share (3), the security invariants
stated as rules (4), how posture delivers and why (5), what the port drops on purpose (6), and the
vocabulary the Rust names are drawn from (7). Section 8 is the flat inventory: one numbered
statement of observable behavior per line, grouped by the script that implements it, each with two
citations underneath it.

- `Source:` the file and line range that implements it, with the symbol name beside the number so the
  statement survives a line drift. Paths are repository-relative; the source name prefixes
  (`executable_`, `private_`) are chezmoi's and the deployed path drops them.
- `Pin:` the test whose failure would announce a change, named as the leaf test name with its file
  and line. A second pin is written `also`. A statement no test pins says `UNPINNED` and names what
  was looked for. The plan writes the missing test first, against the code where it lives today,
  before the step that moves the behavior behind it.

Every name proposed for a crate, module, subcommand, path or file in this document and in the plan
is proposed, confirm before creating.

## 1. The twelve entry points, and what replaces each

Every tool lives under `dot_local/libexec/osquery/` in source and `~/.local/libexec/osquery/` on the
machine. Eight entry points are launchd-driven, one is typed by the operator, one is a child the
router forks, one is run by a chezmoi runner and by uu, and the last is the shared library every
producer sources. The `posture` subcommand column is the proposal the plan builds toward, one
subcommand per entry so that a plist changes one argument and nothing else.

The seven LaunchAgents are all labelled `com.webdavis.osquery-<name>`; the middle column gives the
name alone. The allowlist tool's three verbs are `add`, `deny` and `list`.

| Entry today                      | Invoked by                       | `posture` subcommand (proposed) |
| ---------------------------------| -------------------------------- | ------------------------------- |
| `results-alerter.sh`             | `results-alerter`                | `posture alert`                 |
| `firewall-gatekeeper-monitor.sh` | `firewall-gatekeeper-monitor`    | `posture poll`                  |
| `tailscale-monitor.sh`           | `tailscale-monitor`              | `posture funnel`                |
| `uptime-watchdog.sh`             | `uptime-watchdog`                | `posture watchdog`              |
| `drain-undelivered-alerts.sh`    | `alert-drainer`                  | none (section 6, D1)            |
| `digest.sh`                      | `digest`                         | `posture digest`                |
| `heartbeat.sh`                   | `heartbeat`                      | `posture heartbeat`             |
| `osquery-converge.sh`            | `run_after_50`, uu's brew lane   | `posture converge`              |
| `allowlist.sh`                   | the operator (`-a`, `-d`, `-l`)  | `posture allowlist <verb>`      |
| `enrich-finding.sh`              | forked by `route.sh` per finding | `posture enrich <path>`         |
| `alert-dispatch.sh`              | sourced by seven producers       | none (section 5)                |
| `pipeline-audit.sh`              | sourced by the watchdog          | inside `posture watchdog`       |

Source: the deployment map is `Library/LaunchAgents/com.webdavis.osquery-*.plist.tmpl` (seven
files, `ProgramArguments` at lines 7 to 11 of each), `.chezmoiscripts/run_after_50-setup-osquery.sh:46`
(the converge caller), `dot_local/share/uu/src/config/shipped_template.rs:101` (uu's
`osquery_converge` default), `results-alerter/route.sh:115` (the enricher's path), and the eleven
`source "$HOME/.local/libexec/osquery/..."` lines across seven files (`results-alerter.sh:34-46`,
`firewall-gatekeeper-monitor.sh:52`, `tailscale-monitor.sh:38`, `uptime-watchdog.sh:61-76`,
`digest.sh:23`, `heartbeat.sh:27-31`, `drain-undelivered-alerts.sh:30`).

Six more sourced files are stages or verdicts of the alerter (`results-alerter/normalize.sh`,
`route.sh`, `render-page.sh`, `allowlist-verdict.sh`, `pipeline-verdict.sh`, `digest-store.sh`), one
is optional (`results-alerter/file-integrity-triage.sh`, `results-alerter.sh:59-66`), one is the
converge's pure decision core (`osquery-converge/drift-verdict.sh`), and one is the canary reader
shared by the heartbeat and the watchdog (`canary-freshness.sh`). None of them is an entry point; each
becomes a module.

### 1.1 Schedules

| Agent                          | Trigger                                              | RunAtLoad |
| ------------------------------ | ---------------------------------------------------- | --------- |
| `results-alerter`              | `WatchPaths` on the results log, `StartInterval` 300 | false     |
| `firewall-gatekeeper-monitor`  | `StartInterval` 60                                   | true      |
| `tailscale-monitor`            | `StartInterval` 60                                   | true      |
| `uptime-watchdog`              | `StartInterval` 900                                  | true      |
| `alert-drainer`                | `StartInterval` 300                                  | false     |
| `digest`                       | `StartCalendarInterval` 18:00 local                  | false     |
| `heartbeat`                    | `StartCalendarInterval` 09:00 local                  | false     |

Every plist runs `/opt/homebrew/bin/bash <script>`, sets one `PATH`
(`/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin`), and points `StandardOutPath` and
`StandardErrorPath` at one per-agent file under `~/.local/log/osquery/`
(`com.webdavis.osquery-results-alerter.plist.tmpl:7-30`). The two calendar hours come from
`.chezmoidata/osquery.yaml:7-11`. Each plist has a loader,
`.chezmoiscripts/run_onchange_after_60-load-osquery-*-launchagent.sh.tmpl`, that embeds the plist's
content hash, boots the agent out, and bootstraps it back with three retries
(`run_onchange_after_60-load-osquery-results-alerter-launchagent.sh.tmpl:4-20`).

The root daemon is the vendor's `/Library/LaunchDaemons/io.osquery.agent.plist`, published by
`osqueryctl start` from `/var/osquery/io.osquery.agent.plist`; the converge keeps the six files this
repository owns under `/var/osquery` correct and restarts that daemon when one drifted (S319 to S349).

## 2. Exit-code and output contracts, per caller

What each caller reads back, and what it does with it. A port that changes a row here changes the
caller in the same pull request.

The eight launchd producers first. None of them prints anything on stdout; stderr carries its
diagnostics and launchd appends both to the per-agent log.

- `results-alerter.sh` exits 0 always, even on a delivery hard failure, because a nonzero exit would
  false-trip the watchdog's crash-loop probe; only the cursor stays put (S017, S213). Its stderr also
  carries the triage helper's own diagnostics (S065).
- `firewall-gatekeeper-monitor.sh` exits 0 on a healthy or paged tick and 1 when a page could not be
  durably queued or the baseline could not persist (S265, S267).
- `tailscale-monitor.sh` exits 0, and 1 on a persist failure or an unqueued gap page (S270, S278).
- `uptime-watchdog.sh` exits 0 when healthy or when paged and persisted, and 1 when the page was not
  durably queued or the state did not persist (S224, S226).
- `drain-undelivered-alerts.sh` exits 0 always; stderr is its only channel (S190, S192).
- `digest.sh` exits 0 in every case a test reaches (S280, S285).
- `heartbeat.sh` exits 0 always; the send is fire-and-forget (S205).
- `osquery-converge.sh` exits 0 when converged or repaired, nonzero on any refusal or a failed
  start, and 2 on an unknown argument; stdout is one line per repaired path and nothing at all when
  converged (S319, S320, S341, S344).

The operator's and the router's tools:

- `allowlist.sh` exits 0, 2 on usage, and nonzero when the manifest refresh failed; `-l` prints entry
  lines and `-d` prints a note on a no-op (S298, S308 to S310).
- `enrich-finding.sh` exits 0 for trusted or not applicable and 10 for untrusted or undeterminable,
  and prints one short fact line (S134).

The sourced libraries return a status to a function caller rather than to a process:

- `send_alert` returns 0 when the page was delivered or durably stored and nonzero only when the
  write-ahead persist failed; its log lines go to a file, never to stderr (S144, S187).
- `pipeline_audit_scan` returns 0 with `<kind> <path>` lines, or 1 with one refusal token (S227).
- `newest_canary_timestamp` prints the newest validated epoch or nothing (S193, S196).
- `allowlist_verdict` returns 0 to suppress, 1 for not allowlisted, 2 for a reused label (S069).
- `pipeline_verdict` returns 0 to page and 1 to stay silent (S079).

Two callers outside the pipeline depend on one of those rows. `run_after_50-setup-osquery.sh:42-58`
execs the converge and treats a missing tool as a loud stderr line with exit 0 (S368). uu's brew lane
runs the converge after its upgrade pass and fails that step when the tool is not deployed
(`dot_local/libexec/osquery/executable_osquery-converge.sh:14-18` states the two callers;
`dot_local/share/uu/src/config/shipped_template.rs:101` carries the path).

The library exit contracts in the lower half become Rust return types inside one binary, so they stop
being process contracts. They are kept here because the plan's red-first tests pin them as typed
outcomes with the same three, two and two values.

## 3. Data contracts, exact shapes

### 3.1 The two known-good manifests

Produced by `.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh`, consumed by
`results-alerter/pipeline-verdict.sh:128-129` and `executable_pipeline-audit.sh:193-196`. One
whitespace-separated tuple per line with the path LAST, so a path holding spaces is read whole:

```
<sha256> <mode> <uid> <path>
```

`mode` is exactly four octal digits, `uid` is decimal, and the audit matches the whole line against
`^([0-9a-fA-F]{64}) ([0-7]{4}) ([0-9]{1,10}) (/.+)$` (`pipeline-audit.sh:236`, S234).

`/var/osquery/pipeline-known-good.sha256` covers `~/.local/libexec/osquery/**`,
`~/Library/LaunchAgents/com.webdavis.osquery-*.plist`, and the one exact file
`~/.config/osquery/page-launchd-allowlist.txt`. `/var/osquery/managed-bin-known-good.sha256` covers
`~/.local/bin/*` (non-recursive) and `~/.local/libexec/*` (recursive), manifested paths only.

Both install `root:wheel 0644`. A consumer refuses one that is not root-owned or is group or world
writable (`pipeline-verdict.sh:226-237`, S091). The three columns derive from chezmoi INTENT, never
from the protected tree: the set from `chezmoi managed`, the content from `chezmoi cat`, the mode
from `chezmoi dump --format=json` (`.perm`, a decimal integer), and the owner from the uid running
the apply (`run_after_05:60-91`, `:160`, `:259-263`, `:293-297`, `:334`; S352 to S358). The two files
never vouch for each other (`pipeline-verdict.sh:278-284`, S090), and the bin arm fails inverted: an
unusable bin manifest tracks every bin path rather than none (`pipeline-verdict.sh:436-447`, S089).

The path filter that decides membership is a `case` with five arms
(`run_after_05:192-206`): `~/.local/libexec/osquery/*` and our own plists and the exact allowlist path
into the pipeline arm; `~/.local/bin/*` (one level) and `~/.local/libexec/*` into the bin arm. The
tracked-set classifier in the verdict carries the same patterns (`pipeline-verdict.sh:397-406`,
S085), and the osquery watch paths carry them a third time (section 3.9). Nothing pins the three
copies equal; that test was deleted under the 2026-08-05 testing ruling.

### 3.2 `posture-controls.json`

The declaration is `.chezmoidata/macos_posture_controls.yaml`;
`dot_local/libexec/osquery/posture-controls.json.tmpl` renders it to
`~/.local/libexec/osquery/posture-controls.json` with `{{ toJson .macos.posture_controls }}` after
eleven fail-closed validations (`posture-controls.json.tmpl:22-110`). It lives inside the pipeline
home so the file-integrity watch and the pipeline manifest cover it: the file decides what the poller
monitors, so it is part of the monitor's body. The poller re-validates it whole at runtime (S254,
S255).

Record schema, every field required except `remedy`:

| Field         | Rule                                                                              |
| ------------- | --------------------------------------------------------------------------------- |
| `id`          | `^[a-z0-9_]+$`, unique, must not collide with `firewall`, `gatekeeper`, `screenlock` |
| `description` | non-empty; it is what a page prints                                               |
| `tier`        | exactly `verify`; any other value is refused at render and at runtime            |
| `reader`      | one of eight names, each with a two-value domain                                  |
| `expect`      | must sit inside its reader's domain                                               |
| `target`      | required by the two `lulu_rule` readers, forbidden on the other six               |
| `remedy`      | optional, apostrophe-free (section 4, SI-13)                                      |

The eight readers and their domains are declared twice, in the template (`$readerDomains`,
`posture-controls.json.tmpl:35-43`) and in the poller (`reader_domain`,
`firewall-gatekeeper-monitor.sh:143-150`): `fdesetup_status` and `defaults_autologin` answer
`on|off`; `csrutil_status` and `sysadminctl_guest` answer `enabled|disabled`; `pgrep_oversight` and
`pgrep_lulu_extension` answer `running|stopped`; `lulu_rule_present` and
`lulu_rule_resolved_present` answer `present|absent`. Every reader can also answer `indeterminate`,
which is in no declared domain and is what routes a control to the gap gate (S246, S259).

Eight records ship: `filevault` (expect on), `sip` (expect DISABLED, deliberate on this host, section
4 SI-9), `autologin` (off), `guest` (disabled), `oversight` (running), `lulu_extension` (running),
`lulu_rule_tailscaled` (present, target `/usr/local/bin/tailscaled`), `lulu_rule_hermes_gateway`
(present, target the Hermes venv python). A ninth key, `macos.lulu_talkers`, is read by nothing in
the pipeline.

### 3.3 The alert queue as it stands today

`~/.local/state/osquery-undelivered-alerts.sqlite3`, mode 600 in a mode-700 parent, WAL with a
5000 ms busy timeout and a five-attempt retry on `database is locked` (`alert-dispatch.sh:146-176`,
S157). Each table is bootstrapped lazily inside the same transaction as its first insert (S159):

```sql
pending_alerts(sequence_number INTEGER PRIMARY KEY AUTOINCREMENT, request_id TEXT UNIQUE NOT NULL,
               occurrence_ts INTEGER NOT NULL, url TEXT NOT NULL, body_base64 TEXT NOT NULL,
               attempts INTEGER NOT NULL DEFAULT 0, next_attempt_after INTEGER NOT NULL DEFAULT 0,
               created_at INTEGER NOT NULL)
pending_local_notifications(sequence_number ..., notification_id TEXT UNIQUE NOT NULL,
               occurrence_ts INTEGER NOT NULL, title TEXT NOT NULL, message TEXT NOT NULL,
               sound TEXT, attempts ..., next_attempt_after ..., created_at INTEGER NOT NULL)
dead_letter_alerts(<every pending_alerts column>, dead_lettered_at INTEGER NOT NULL,
               last_http_status TEXT NOT NULL, reason TEXT NOT NULL)
```

Drain order is `ORDER BY occurrence_ts, sequence_number` (S164). Thresholds, all env-overridable:
`OSQUERY_DRAIN_MAX_ATTEMPTS` 20, `OSQUERY_DRAIN_MAX_AGE_SECONDS` 604800,
`OSQUERY_DRAIN_RETRY_BASE_SECONDS` 60, `OSQUERY_DRAIN_RETRY_RANDOM_SECONDS` defaulting to the base,
`OSQUERY_LOCAL_NOTIFY_MAX_AGE_SECONDS` 86400 (S166, S170, S185). Under the delivery recommended in
section 5 this database has no successor in posture; its rows move to pns's ledger, and the
retirement steps in the plan drain it to zero first.

### 3.4 The webhook body and the route that renders it

Built once by `_build_webhook_body` (`alert-dispatch.sh:1124-1128`, S150):

```json
{"event_type":"osquery.alert","host":"<hostname -s>","tier":"page|muted","ts":<epoch>,
 "alert":{"title":"<title>","detail":"<detail>"}}
```

POSTed to `http://127.0.0.1:8644/webhooks/priority` with `Content-Type: application/json`,
`X-Webhook-Signature: <lowercase hex HMAC-SHA256 of the body>` and `X-Request-ID: <request_id>`, at a
5 second `--max-time` (`alert-dispatch.sh:1052-1059`, S154). The request id is `osquery-<first 32
hex of sha256(seed)>`, the seed being the caller's occurrence identity when threaded and a per-call
unique string otherwise (`alert-dispatch.sh:1134-1136`, `:1209-1215`, S149). `tier` is `page` when a
sound was requested and `muted` otherwise (S143).

The live route consumes two of those fields. Read from `~/.hermes/config.yaml` on 2026-09-05 with
the secret lines filtered out, the `priority` route is `deliver: discord`, `deliver_only: true`, and
its prompt is `{alert.title}` followed by a blank line and `{alert.detail}`; nothing in the route reads
`tier`, `host`, `ts` or `event_type`. The `pns` route on the same gateway renders
`{agent} · {state} · {project}` over `{detail}`. So the wire `tier` that the heartbeat and the digest
set to `muted` reaches Discord as nothing at all; what "muted" changes today is only the local
banner's sound (S143, S204, S296). Section 5 builds on that fact.

The signing key is the first line of `~/.config/osquery/webhook-secret` with carriage returns
stripped, or `OSQUERY_WEBHOOK_SECRET` (`alert-dispatch.sh:1039-1046`, S156). That file is a runtime
file, mode 600, tracked by nothing in this repository (`alert-dispatch.sh:26-29`; no chezmoi source
names it). The HMAC is built by hand from SHA-256 with `openssl dgst`, and every hash takes its bytes
on stdin so neither key nor body reaches `ps` (`alert-dispatch.sh:75-109`, S155).

### 3.5 The digest spool

NDJSON at `~/.local/state/osquery-digest-spool/digest.ndjson`, directory 700 and file 600. One line
per non-paging finding, six DERIVED fields only, never the whole columns object
(`digest-store.sh:42-52`, S117, S119):

```json
{"timestamp":"<UTC ISO 8601>","detector":"<q>","category":"<cols.category>",
 "identity":"<label|identifier|target_path|path|username|?>","action":"<act>","summary":"<q> <identity>"}
```

The default path is an independent literal in two places, `digest-store.sh:25` and `digest.sh:30`.
The daily run claims the batch by renaming it to `<store>.<epoch>.<pid>.build`, rotates it to
`<store>.last` (mode 600) on success, and appends it back on failure (S281 to S285, S296). Two
processes share this file by a rename protocol, which is why it stays a file in the port (plan,
section 2.4).

### 3.6 The heartbeat canary

`osquery.conf` schedules `heartbeat_canary` as `SELECT unix_time FROM time;` every 600 s with
`snapshot: true`, so it lands in `osqueryd.snapshots.log`, which the alerter never reads
(`osquery-converge/desired/osquery.conf.tmpl:26-31`, S361). `newest_canary_timestamp` selects rows by
parsed `.name`, prefers the envelope `.unixTime` and falls back to `.snapshot[0].unix_time`, takes the
last, and range-binds the value to `^(0|[1-9][0-9]{0,9})$` (`canary-freshness.sh:39-47`, S193, S196).
Freshness is two-sided, `|now - ts| <= OSQUERY_CANARY_MAX_AGE`, default 1800, shared by the heartbeat
and the watchdog (S200, S211).

### 3.7 The page-launchd allowlist

`~/.config/osquery/page-launchd-allowlist.txt`, NDJSON, one `{"label","path","program","sha256"}`
object per line, with comment and blank lines preserved. `path` and `program` store a leading `$HOME`
as `~/`, and the verdict re-expands it (S071, S305). An EMPTY `sha256` is the own-agent convention:
the writer never produces one (`allowlist.sh:249-252`, S304), and an unpinned entry is vouched for by
the manifest instead (`allowlist-verdict.sh:116-126`, S075). The source is
`dot_config/osquery/private_page-launchd-allowlist.txt`; the writer edits that source, applies the
one target, then runs the manifest runner (S307).

### 3.8 The upgrade record

`~/.local/state/homebrew-weekly-upgrade/last-upgrade-changes.tsv`, written by uu's brew lane
(`dot_local/share/uu/src/lanes/brew/upgrade_record.rs:1-14`), read by
`file-integrity-triage.sh:92`. Line 1 is `<epoch>\t<iso-8601-utc>`; every later line is
`<name>\t<added|removed|changed>\t<before>\t<after>` with the absent side empty (S110, S112). The
literal path is duplicated by hand between producer and consumer, deliberately untested
(`file-integrity-triage.sh:71-77`). The record is a lead and is labelled as one: nothing in it can
suppress or downgrade a page (S114).

### 3.9 The desired state for `/var/osquery`

Six files under `osquery-converge/desired/`, two of them templates: `osquery.conf.tmpl`,
`osquery.flags`, and four packs (`agent-attack-surface.conf.tmpl`, `installed-software-drift.conf`,
`intrusion-detection.conf`, `security-policy-regression.conf`). The list is NAMED in
`OSQUERY_CONVERGE_FILES` (`osquery-converge.sh:207-214`), never globbed, and a file in the staging
tree the list does not name is a refusal (S333). Files install `root:wheel 0644`; `/var/osquery` and
`/var/osquery/packs` are `root:wheel 0755` (`drift-verdict.sh:28-31`, S315, S317).

`osquery.conf.tmpl:39-84` names eight `file_paths` categories (`ssh`, `allowlist_file`,
`pipeline_integrity`, `managed_bin`, `launch_agents`, `launch_daemons`, `sudoers`, `sshd_config`)
and four `file_paths_hashes` categories; `managed_bin` and `allowlist_file` deliberately carry no
hash, so their events take the atomic-rename path in the verdict (S083, S362). `pipeline_integrity`
watches `~/.local/libexec/osquery/%%`, and `managed_bin` watches `~/.local/libexec/%%`, so a file
under the pipeline home fires under both categories and both route to `pipeline_verdict`, which routes
the path to exactly one manifest (S063, S090).

### 3.10 The per-tool state files

Each producer keeps a small file under `~/.local/state/`. None of them is multi-record, and each is
read whole and validated before use. Observed on the live machine on 2026-09-05 (names and modes
only):

- `osquery-results-offset` (644), the alerter's cursor: `<inode> <offset>`, both `^[0-9]+$`, else
  the whole log replays (S007). `osquery-results-offset.lock` is the `lockf` file behind fd 9 (S001).
- `osquery-watchdog-state.json` (600), the watchdog's state: exactly one JSON object, else `{}`;
  streaks clamped at 99 (S209, S223).
- `osquery-posture-state.json` (600), the poller's baseline: one object, mode 600, three in-domain
  built-in scalars, and per-control priors trusted only under the same `expect` and `target` (S260,
  S261). `osquery-posture-state.json.gap` holds the space-separated set of gapped members already
  paged for (S257).
- `osquery-tailscale-funnel.json` (600), the funnel monitor's baseline: one object whose `funnel` is
  `active` or `inactive`; a present file holding anything else is CORRUPT, distinct from absent
  (S272, S273). `osquery-tailscale-funnel.json.gap` is its page-once gap marker (S270, S277).
- `osquery-digest-spool/` (700), shared by the alerter and the digest: section 3.5.
- `osquery-undelivered-alerts.sqlite3` (600), the dispatch library's queue: section 3.3;
  `osquery-undelivered-alerts.sqlite3.drain.lock` is the drainer's single-instance lock (S188).
- `osquery-tailscale-funnel` (644) and `osquery-spool/` (700) are leftovers: the first from the
  monitor before it kept JSON, the second the retired file spool the SQLite queue replaced.

The two leftovers are named so the plan can list them for the operator to trash; no script in the
tree reads either (a grep for `osquery-spool` and for the bare `osquery-tailscale-funnel` under
`dot_local/libexec/osquery` finds only the `.json` path).

### 3.11 Environment overrides that double as test seams

Twelve `OSQUERY_*` variables carry a default path and each is also the test seam:
`OSQUERY_RESULTS_LOG`, `OSQUERY_RESULTS_OFFSET`, `OSQUERY_SNAPSHOTS_LOG`,
`OSQUERY_UNDELIVERED_ALERTS_DB`, `OSQUERY_DELIVERY_LOG`, `OSQUERY_WEBHOOK_SECRET_FILE`,
`OSQUERY_DIGEST_STORE`, `OSQUERY_LAUNCHD_ALLOWLIST`, `OSQUERY_PIPELINE_MANIFEST`,
`OSQUERY_MANAGED_BIN_MANIFEST`, `OSQUERY_POSTURE_STATE`, `OSQUERY_WATCHDOG_STATE`. The converge is the
exception: its four overrides are gated behind `OSQUERY_CONVERGE_TEST_SEAM=1`, the gate tests
presence, and with the seam engaged the two production-defaulting seams must be given explicitly
(S330, S331). The port keeps that shape for every path: a Rust `Paths` value built once at the
composition root from `$HOME`, overridable in tests by construction rather than by environment, and
the converge's seam gate re-expressed as the only environment read the binary performs on that path.

## 4. Security invariants

Each is a rule the port keeps, stated once, with the statements that carry it. A pull request that
weakens one names it in its description.

- **SI-1** The deterministic alert fires first and independently. No language model is on the page
  path at any model capability; a future analyst may gate the noisy tier only. Memory
  `osquery-security-project.md`, locked 2026-06-10; nothing in the twenty-one files calls a model.
- **SI-2** Silence is the default and only CRIT leaves the machine. `send_alert` POSTs for `CRIT`
  alone (`alert-dispatch.sh:1193`, S142), so no producer can deliver anything but a priority page.
- **SI-3** Fail-safe toward paging, everywhere. A missing manifest, an unreadable count, an unknown
  agent state, an indeterminate probe, a short severity batch and a broken bin manifest each resolve
  to a page (S042, S089, S208, S211 to S216, S246, S262).
- **SI-4** Notify before persist. Every baseline, streak and marker advances only after the page is
  durably queued (S226, S267, S278, S015).
- **SI-5** Write-ahead delivery. A page is persisted before the first network attempt and deleted
  only on a confirmed 2xx; a failed persist is the only hard failure (S145, S146). Section 5 keeps
  this promise through pns's ledger rather than posture's own table, which holds only once the
  ledger row is committed before dispatch and the result envelope says so (section 5.2).
- **SI-6** The pipeline-broken alarms stay audible for every producer, the two muted ones
  included (S183). Under section 5 this becomes the last-resort banner posture raises when the
  delivery engine itself cannot take the event.
- **SI-7** Root-owned installs out of a private 0700 copy. Every byte root writes is read from a
  per-run private copy of the staging tree, never from the deployed tree, and one `install` call
  carries owner, group and mode (S327, S332). Privileged commands are named by absolute path, and the
  one resolved command is trusted only when its directory is root-owned and not writable by others
  (S328, S329).
- **SI-8** Symlink refusal. A symlink standing where a directory belongs, in the staging path, in
  the desired tree, at the vendor plist, or at a manifested path is refused, never followed
  (S325, S334, S335, S342, S082, S097, S238).
- **SI-9** The allowlist boundary. A user LaunchAgent is default-deny; the allowlist suppresses
  only when the manifest vouches for the plist and for the allowlist file itself; an untrusted
  program behind an allowlisted label still pages; `com.apple.*` labels cannot be allowlisted; the
  writer routes through chezmoi source, a targeted apply and a manifest refresh
  (`docs/superpowers/specs/2026-07-26-osquery-allowlist-boundary-design.md`, options B and D-prime;
  S056 to S058, S075, S076, S303, S307).
- **SI-10** The file-integrity manifest arm. The event digest is never a trust input; every
  suppression is decided against the file's current content, re-read at decision time, against the
  exact four-column tuple, from a manifest that is root-owned and derived from chezmoi intent
  (S084, S091, S093, S097; section 3.1). A `DELETED` verb on a tracked path always pages (S081).
- **SI-11** Attacker-influenced text never becomes structure. Every column crosses into a body
  through one sanitize chokepoint (backticks stripped, `\r\n\t` squashed, capped, wrapped in a code
  span); paths in next-step commands are shell-quoted; AppleScript literals escape the backslash
  first; hostile separators cannot shift field boundaries (S059, S125, S131, S180, S266, S276,
  S291).
- **SI-12** Secrets stay out of readable files. Credential pages render the basename only; the
  digest spool carries derived fields only; the two true secrets are watched by metadata, never by
  hash; the signing key never reaches argv or a log line (S117, S128, S155, S187, S365).
- **SI-13** No apostrophes in a string that reaches a render. The render programs are bash
  single-quoted today (S116, S279, memory `osquery-alerter-render-no-apostrophes`). The port removes
  the mechanism that made this necessary; the rule is kept for the strings that reach a Discord body
  only where the plan says so.
- **SI-14** No option closes a user-level attacker on a passwordless-sudo host, and the code says so
  in four places rather than overclaiming (`pipeline-verdict.sh:103-115`, `:210-220`,
  `run_after_05:105-114`, the allowlist design's "What this does not defend against"). Posture
  inherits the same honest claim and adds no stronger one.
- **SI-15** Single-host scope. The target is this machine alone, provisioned from this repository;
  no fleet framing and no per-host URL work (memory `osquery-security-project.md`).

## 5. Delivery: how posture reaches the operator

### 5.1 What is true today

The pipeline does not call pns in any form. A repository-wide grep for `pns` under
`dot_local/libexec/osquery/` returns one prose comment (`osquery-converge/drift-verdict.sh:12`).
Delivery is `alert-dispatch.sh` end to end: `alerter` or `osascript` for the local banner, a
hand-rolled HMAC plus `curl` for the Discord page, a SQLite write-ahead queue, a drain with backoff
and dead-lettering, and a durable retry of the local banner (S142 to S187). Forty-six statements
describe it; six are pinned, and those six pin the two counters, the SQL quoting and the drain's
skip-and-continue. The write-ahead path, the HMAC, the banner confirmation and the dead-letter
thresholds have no direct test.

pns is the operator's one notification engine (memory `pns-fully-rust-plugin-architecture`, ruling
2026-08-10), and uu already reaches it two ways: alerts as a client of the binary
(`dot_local/share/uu/src/alert.rs:1-15`) and records through the signed-POST seam
(`dot_local/share/uu/src/delivery.rs:11`). The pns refactor plan lands a versioned producer protocol
over `pns submit --json` with a result envelope
(`docs/superpowers/plans/2026-09-05-pns-refactor-plan.md`, PR 7.1 and PR 7.3), a write-ahead
delivery ledger whose undelivered rows the daemon drains as leased
jobs (PR 11.4), an idempotency key per event carried as an `Idempotency-Key` header, and a
`pns-hermes` crate holding the signed-POST client uu depends on (section 8 of that plan). Today the
`pns-protocol` crate is a doc comment ("Nothing has moved in yet",
`dot_local/share/pns/crates/pns-protocol/src/lib.rs:15-17`), so none of that is a transport posture
can target yet.

The two routes on the gateway differ in key and body. pns posts `{agent, state, project, detail}` to
`/webhooks/pns` under `[plugins.hermes] key` (`dot_local/share/pns/src/channels/hermes.rs:18`,
`:48-57`); the pipeline posts section 3.4's body to `/webhooks/priority` under its own runtime key.
Section 3.4 established that the priority route renders `alert.title` and `alert.detail` and reads
nothing else, and that the `pns` route on the same gateway renders `agent · state · project` over
`detail`.

### 5.2 Option (a): posture is a pns producer

Posture spawns the deployed `pns` binary with `pns submit --json`, one request on stdin, one result
envelope on stdout, per page, heartbeat or digest. The request carries producer identity `posture`,
the producer request id (today's occurrence-derived `osquery-<32 hex>`, renamed), a source event name
(`page`, `heartbeat`, `digest`, `gap`, `cursor-reset`), the normalized signal (`NeedsAttention` for a
page, `Observation` for the heartbeat and the digest), the rendered body as bounded detail, and the
route the pns route work assigns to posture (section 5.5, item 5; `priority` itself keeps its old
contract until the bash is gone). Posture reads one bit out of the result: whether pns has durably
recorded the request and now owns its delivery. That bit is what `send_alert`'s return value means
today (S144), so every producer's notify-before-persist rule (SI-4) keeps its exact shape: advance
the cursor, baseline or marker only when the engine answered that it holds the event.

pns does not give that bit today, and the port must not pretend it does. On the event path the
channels are dispatched first and the record written afterwards (`dot_local/share/pns/src/main.rs`,
`dispatch_legs` at 3101 and `record_decision` at 3122), and both the decision ring and the missed
journal drop a write failure on purpose (`main.rs:814-822 record_decision`, `:839-858
record_missed`, each `let _ = append_ring_line`). A successful exit therefore says the channels were
tried, not that anything was committed, and a producer that advanced its cursor on it would lose a
page the moment pns was killed between dispatch and record. `accepted` is sound only when it names a
committed, retriable obligation for that request id: a ledger row written before dispatch (pns PR
11.4) and reported as written in the result envelope (pns PR 7.3). That work is a prerequisite of the
plan's first step, not of its fifth, so posture's `AlertSink` contract is written against a real
acknowledgement rather than a promised one.

A pns-keyed route for posture is added on the gateway beside `priority`, so one signer serves every
pns producer. `priority` keeps its own key and body while any bash producer or queued retry still
POSTs the old contract, and retires with the runtime key file `~/.config/osquery/webhook-secret` only
after the queue is drained (section 5.5, item 5; plan step 0.3 and PR 6.7). The old route's KeePassXC
entry is not in this repository (section 3.4), so retiring it is a hand step.

What posture gains beyond parity:

- One journal. Undelivered security pages sit in the same ledger as every other undelivered
  notification, drained by the same daemon, visible in the same `pns doctor`, and replayed by the
  same missed-notification mechanism when the operator returns (pns S106, S155 to S165).
- One presence gate. A page reaches the phone when the operator is away and the banner when they are
  at the desk, decided by the engine that already knows which (pns section 4). Today a page reaches
  Discord and the local banner only; the phone sees it through Discord's own app if at all.
- One signer, one route table, one HMAC implementation, one retry policy, and no second SQLite
  database, drainer LaunchAgent, lock file or dead-letter table in posture.
- The return recap. A security page that fired while the operator was away appears in pns's recap of
  the window, which nothing in the pipeline offers today.

What it costs, and how each cost is met:

- The engine's delivery path is fail-open by rule (a busy or corrupt store never blocks a delivery;
  pns plan, section 8), while posture's producers are fail-closed at the persist (SI-4, SI-5). The
  two meet at the result envelope: posture treats only an `accepted` result whose diagnostics say
  the ledger row was written as durable. Anything else, including a `degraded` delivery that went
  out while the store was down, leaves the cursor or baseline where it was, so the next tick
  re-submits under the same request id and the ledger deduplicates when it recovers. The gateway
  already treats the request id as idempotent (`alert-dispatch.sh:1064` names the rule), and pns
  PR 7.3 carries the id on the wire.
- The pipeline-broken alarm (SI-6, S183). When the binary is absent, exits nonzero, hangs to its
  deadline, or returns no parseable envelope, the engine itself is the broken component, and no
  engine can report that. So posture keeps one last-resort banner, a bounded `osascript` spawn with
  the fixed loud sound and the backslash-first literal escape (S180), raised only on that path. This
  is the alarm for the engine being down, not a second delivery engine: it retries nothing, stores
  nothing and posts nowhere. It covers DETECTABLE failure and nothing more. A pns binary replaced by
  one that answers `accepted` and delivers nothing is invisible to this path, because the answer is
  the only thing a producer can see, and SI-14's honesty applies: the banner is not a tamper defence
  and the documents must not describe it as one. Catching that case is the integrity check below.
- Independent integrity and health checks for pns. Delegating delivery puts pns on posture's trust
  path, so posture judges pns the way it judges every other component: without asking pns. None of
  the following is answered by the pns binary. The deployed `~/.local/libexec/pns/pns` gets a
  known-good tuple under the same authorized-build rule the plan's decision 2 gives posture's own
  binary, so a swapped engine pages like a swapped script. `posture watchdog` reads launchd's state
  for `com.webdavis.pns-daemon` and the daemon's liveness directly, the way probe 2 reads the six
  agents today (S212), and pages a daemon that is unloaded or wedged. The watchdog opens pns's
  ledger read-only, exactly as S176 to S178 open the SQLite queue today, and pages an unreadable
  ledger, any dead-lettered row, and an undelivered backlog that grew across two passes. Executable
  presence is not detection and replaces none of these; the plan's PR 6.4 is corrected accordingly.
- The operator mute and Focus. OPERATOR DECISION PENDING (the reviewer recommends b); plan decision
  4 carries both options and this document chooses neither. What is true of pns today, read from the
  code: a mute or a configured Focus zeroes the banner, the phone card and the pulse and keeps only
  the durable leg (`crates/pns-domain/src/routing.rs:107-114`, where the durable leg is wanted
  unconditionally; `src/engine.rs:978` a muted decision keeps the durable log and drops every
  decorative leg, `:1011` no pulse, `:1043` the mute beats a forced phone card, `:1074` a named
  Focus does the same). So "Discord at once" is correct as ROUTING INTENT: the hermes leg is
  planned. It is not a delivery guarantee and not a phone ping. The replay does not fire when the
  mute lapses; it waits for the next event that earns the operator a banner or a card
  (`src/missed_notifications.rs:91-93 should_replay`) and delivers a catch-up card then, so "on the
  banner and phone on return" overstates what the operator would see. Option (a) accepts that: a
  security page under a mute reaches Discord, is journaled, and interrupts nobody until the mute
  ends and something else fires; the argument is that a mute is about interruption, not
  concealment, and that no producer has yet been allowed to overrule the operator in pns. Option
  (b) says a security `NeedsAttention` is the one class that may: the funnel detector's page reports
  a service newly exposed to the public internet and tells the operator to "close it now"
  (`executable_tailscale-monitor.sh:132`), so an hour of Focus extends an unintended exposure by an
  hour, and a tamper page on a tracked path (S081 to S084) carries the same shape. Under (b) pns
  gains an operator-configured bypass, set in the operator's own pns config for this producer's
  `NeedsAttention` only, so the operator still holds the switch. Under both options the heartbeat
  and the digest stay quiet and interrupt nothing.
- The heartbeat and the digest. Today both send `CRIT` with an empty sound (`executable_heartbeat.sh:51,
  :80, :101`; `executable_digest.sh:226`), and `send_alert` raises the local notification first,
  silent when the sound is empty, then POSTs any `CRIT` regardless of sound
  (`executable_alert-dispatch.sh:1186-1193`). So each arrives in Discord daily and as a silent
  banner. Section 3.4's finding is that the wire `tier` renders as nothing, not that the message
  does. The port PRESERVES both: the heartbeat and the digest reach the durable route (Discord) and a
  silent local banner, with no phone card, no pulse and no replay. An `Observation` that reached the
  durable log and nothing else would drop the daily Discord line and the silent banner, and that is
  not approved; section 5.5 states the requirement the pns route work must meet.
- The queue counters, the drainer and the watchdog's probe 4 (S173, S176 to S178, S188 to S192,
  S216). The drainer and the counters over the SQLite queue retire with the queue. The plan names
  the pns-side requirements: the daemon dead-letters a row after the same bounded attempts and age,
  raises its own loud alert when it does or when the undelivered backlog grows across two passes,
  and `pns doctor` reports the ledger. That does not replace probe 4: pns reporting on pns is not
  independent, so the watchdog keeps its own read of the ledger and the daemon, as stated above.
- Sequencing. The three pns pull requests (7.3, 11.4 and the priority-route work) are prerequisites
  of the plan's step 1, together with the mixed-producer route transition the plan's step 0 settles.
  Nothing in the ladder starts before the durable acknowledgement exists, because the `AlertSink`
  contract every use case is written against is that acknowledgement.

### 5.3 Option (b): posture keeps a signed webhook client behind a port

Posture ports `alert-dispatch.sh` to Rust as its own `AlertSink` adapter: `rusqlite` for the three
tables, the `pns-hermes` crate for the HTTP and HMAC (so the signer is shared code under a second
key), a `posture drain` subcommand for the drainer LaunchAgent, the durable banner retry through
`terminal-notifier`, and the two counters for the watchdog.

What it buys: no dependency on the pns ladder, exact parity with all forty-six dispatch statements,
and the priority route keeps its own key and body. What it costs: a second write-ahead queue in a
second SQLite file, a second drain daemon and lock, a second dead-letter policy, a second banner
path, a second signer key, no presence gate, no phone, no replay and no recap, and a standing
contradiction with the ruling that pns is the one engine. Every hardening round pns's ledger goes
through (the 2026-09-03 findings on crash windows and outcome authority) would have to be repeated
here or accepted as divergence.

### 5.4 Ranking and recommendation

Ranked by result quality (memory `optimal-over-cheap`, ruling 2026-09-05), option (a) wins on every
axis the charter names: one delivery engine, one presence gate, one journal, one signer. Option (b)
wins only on schedule, and the operator has ruled that schedule is not a design input. The
recommendation is (a), with the priority-route work filed as its own pull request in the pns
program, sequenced before posture's first step (plan step 0), and with the last-resort banner as the
one delivery-shaped thing posture keeps. No bash facade survives either way. The 2026-09-06 review
upheld (a) on the ruling that pns is the one engine, withdrew "better on every axis" as the reason
(option (a) gives up the failure isolation (b) keeps), and attached the conditions section 5.2 now
carries: a durable acknowledgement before step 1, independent checks on pns, and the route overlap.

### 5.5 What the priority-route pull request must deliver

Stated here so the pns program can size it; the design belongs there.

1. `pns submit --json` accepts a route name and a `NeedsAttention` or `Observation` signal from a
   producer, and its result says whether the ledger row was COMMITTED before dispatch (the durable
   bit posture reads). An `accepted` that is answered before the row is on disk, or that is answered
   when the write failed, is a defect in pns, and posture's stub tests treat it as not accepted.
2. A `NeedsAttention` reaches the log and the presence-gated surfaces. The heartbeat and the digest
   reach the hermes route (Discord) and a silent local banner and nothing else: no phone card, no
   pulse, no journal entry for replay. Whether that is an `Observation` with a route that names both
   surfaces or a third signal is the pns program's design; the requirement is that Discord keeps its
   daily heartbeat and digest lines (S204, S296) and the desk keeps the silent banner.
3. The hermes body for a producer-supplied detail must render posture's page whole (the 1900
   character cap of S133 already fits Discord), with the request id on the wire.
4. The daemon dead-letters an undelivered row after bounded attempts and age, and raises its own
   loud alert on a dead-letter or a backlog that grew across two passes; `pns doctor` shows both.
   The ledger stays a file posture can open read-only without the daemon's help, and the daemon
   stays a launchd job posture can inspect, because section 5.2's independent checks read both.
5. A pns-keyed route for posture's producers is added to the hermes config while the existing
   `priority` route keeps its key and body, because the bash producers and every retry still queued
   in the SQLite store POST the old contract until the last one is gone. The old route and the
   runtime key file `~/.config/osquery/webhook-secret` retire only after the plan's PR 6.7 has
   drained the queue (plan, step 0). The edit is in the encrypted source
   `private_dot_hermes/encrypted_private_config.yaml.age`, which the operator edits.
6. Whether a security `NeedsAttention` from this producer may bypass the operator's mute is decided
   by plan decision 4, which is pending. If the operator takes (b), the bypass is a pns config
   option the operator sets, and this pull request carries it.

## 6. What the port drops, with the reason

Nothing below is dropped silently. Each entry names the statements, the reason, and where the
behavior lives afterwards.

- **D1** The drainer script, its LaunchAgent and the single-instance drain lock (S188 to S192).
  pns's daemon leases and retries undelivered rows (pns plan PR 11.4); the plist and its loader are
  deleted.
- **D2** The SQLite write-ahead queue, its three tables, the drain, the backoff and the dead-letter
  move (S145, S157 to S175). pns's ledger is the one store for every producer (section 5.2); the
  durability promise (SI-5) is kept by the result envelope.
- **D3** The durable retry of the local banner, its table, its expiry and the `occurred` subtitle
  (S181, S182, S184 to S186). pns replays a missed notification on the operator's return carrying the
  original time (pns S155 to S165); a late banner with a subtitle is the weaker version of that. This
  is a deliberate delivery change (section 6.1), not parity.
- **D4** The two read-only queue counters over the SQLite store and the watchdog's queue probe as
  written (S176 to S178, S216). The watchdog does not lose the probe; it re-reads it against pns's
  ledger and daemon without asking pns (section 5.2), so the independence of the check is kept and
  only its subject changes. A deliberate change (section 6.1).
- **D5** The hand-rolled HMAC, the webhook POST site, the retry-with-backoff attempt loop and the
  localhost-only check on stored URLs (S152 to S156, S167). pns signs and posts; posture never
  stores a URL, so there is none to check.
- **D6** The `tier` field and the `event_type`, `host` and `ts` fields of the webhook body (S143,
  S150). The live route consumes none of them (section 3.4); the signal replaces `tier`, and pns
  supplies the host.
- **D7** The `osascript` fallback for an ordinary banner and the markdown strip before `alerter`
  (S179). pns's banner destination takes the ordinary banner; posture keeps `osascript` for the
  last-resort alarm alone (SI-6).
- **D8** The `pcount` fallback read through `// 0` and the `9>&-` fd hygiene on every spawn (S004,
  S012). A lock a Rust process holds is not inherited by a child unless posture asks for that, and
  the count is a typed value.
- **D9** The unlocked fallback on a host without `lockf`, on the alerter, the drainer and the writer
  (S003, S189, and that clause of S299). A Rust `flock` has no missing-binary case; a lock that
  cannot be taken fails closed. Observable on a host that lost `lockf`: the bash ran unlocked, the
  port refuses to run, so this is a deliberate change (section 6.1).
- **D10** The `declare -F` presence checks on reused seams and the conditional sourcing of helpers
  (S018, S019, S078, S107, S217, S231, S232). One binary has no partial install, and an absent
  triage helper becomes a compile-time fact.
- **D11** The `$single_quote` SQL escaping and the hex-in-SQL row export (S158, S186). Posture holds
  no SQL; the D2 successor binds parameters.
- **D12** The `heartbeat_canary` exclusion as a rule of its own (S026). It folds into S025's closed
  set, which becomes a Rust enum the canary is not in.
- **D13** The spool default path spelled in two places (that clause of S117). One `Paths` value
  replaces the parity no test pinned.
- **D14** The `bt` variable and the shfmt workaround behind it (S279). A formatter workaround with
  no behavior; the apostrophe rule (SI-13) is kept wherever a body is still built by hand.

Two behaviors are kept although the charter might expect them dropped. The `osqueryctl` trust check
on its containing directory (S329) stays, because `sudo -n` still preserves the caller's `PATH`.
The converge's `rm -rf` of its private stage (S349) stays as a Rust `remove_dir_all` on a path the
process created, because the `trash` rule covers operator files, not a per-run temporary directory
the tool owns.

### 6.1 The delivery changes the port makes on purpose

"No product changes" was the first draft's claim and it is false as written: moving delivery into
pns changes what the operator can observe, and each change below is approved as a deliberate one
rather than smuggled in as parity. Everything not listed here is expected to behave as the bash does,
and a difference found there is a regression.

- D1 and D2: the SQLite queue, its drainer and its LaunchAgent are replaced by pns's ledger and
  daemon. Observable: one fewer agent, one fewer state file, the retry cadence and dead-letter
  policy become pns's.
- D3: the durable local-banner retry (a late banner with an `occurred` subtitle) is replaced by
  pns's presence replay, which fires on the next qualifying event after the operator returns and
  delivers a catch-up card. Observable: a banner missed while away is not re-shown as a banner.
- D4: the watchdog's two SQLite counters are replaced by an independent read of pns's ledger and
  daemon state. Observable: the page wording names pns's ledger; the check stays posture's own.
- D5 and D6: the hand-rolled HMAC, the POST site, the retry loop, and the `tier`, `event_type`,
  `host` and `ts` body fields are replaced by pns's signer and body. Observable: the Discord line is
  rendered by the pns route's prompt rather than the `priority` route's, and the page title and full
  detail are preserved in it (section 5.5, item 3).
- D7: the ordinary banner is raised by pns's banner destination instead of `alerter` or `osascript`
  directly. Observable: the banner's look is pns's.
- D9: a host that lost `lockf` ran the alerter, the drainer and the writer unlocked; the port fails
  closed instead.
- The presence gate: a `NeedsAttention` page reaches the phone when the operator is away, which the
  pipeline never did. Observable: a new surface for the same page.
- The operator mute: whatever decision 4 settles, it is a change, because the bash had no mute.

Option E of the allowlist boundary design (own-agent suppression by manifest membership, retiring
the empty-`sha256` convention) is NOT on this list. It changes suppression semantics and gets its own
pull request with its own evidence after the port; plan decision 5 records that separation.

### 6.2 What the port preserves, and the claim that would have dropped it

The heartbeat's daily Discord line and its silent banner (S204, S205), the digest's daily Discord
message and its silent banner (S296), and every ordinary silent banner an empty sound produces today
(S143) are preserved. The first draft of section 5.2 said an `Observation` reaches the durable log
and nothing else; read against `send_alert` (`executable_alert-dispatch.sh:1186-1193`), which
notifies locally first and then POSTs every `CRIT` whether or not a sound was requested, that
sentence would have removed both daily messages and the silent banners. Section 5.2 and section 5.5
now state the preservation as a requirement on the pns route work.

## 7. Vocabulary

Names come from the code, and where the code and this list disagree the code wins.

- **finding**: one normalized row, `{q, act, cols, ep}` (S034); **detector** or **query**: the bare
  query name after the pack prefix is stripped (S023); **batch**: the findings of one snapshot.
- **severity**: `CRIT`, `NOTICE`, `INFO` from the classifier (S035); **tier** or **outcome**: page,
  digest, log-only, assigned by the gate (S067); **page candidate**: a finding stamped `CRIT`.
- **verdict**: a total function answering suppress, page or silent (`allowlist_verdict`,
  `pipeline_verdict`); **enrichment** and the **signing** fact; **promotion**: NOTICE to CRIT on an
  untrusted signature (S044).
- **manifest** and **tuple**: the four-column known-good line; **tracked set**: the paths the
  verdict judges; **vouch**: a manifest tuple matching the current file; **settle**: the bounded
  wait for a manifest that predates a change (S098).
- **triage**: the three correlation facts attached to a page (S102); **upgrade record**: uu's TSV.
- **spool** and **claim**: the digest's NDJSON and its rename (S282); **group**, **bullet**,
  **roll-up**, **cap** (S286 to S289).
- **canary**, **freshness**, **MISSING**, **STALE**, **IMPLAUSIBLE** (S193, S203).
- **probe** (the watchdog's five, the poller's per-control read), **streak**, **fingerprint**,
  **page-once**, **gap**, **baseline**, **prior**, **first observation**, **degraded** (S213, S221,
  S257, S263).
- **control**, **reader**, **domain**, **expect**, **target**, **indeterminate** (section 3.2).
- **funnel**, **exposure**, **corrupt** versus **absent** (S268, S273).
- **desired**, **staging**, **private stage**, **drift**, **verdict token** (`ok`, `absent`,
  `irregular`, `unreadable`, `content`, `mode`, `owner`, `group`), **repair**, **restart evidence**,
  **settle window** (S311 to S346).
- **label**, **entry**, **pinned** versus **unpinned**, **reused label**, **publish** (allowlist).
- **cursor** (`<inode> <offset>`), **checkpoint**, **torn line**, **occurrence id** (S007 to S015).
- **occurrence**, **request id**, **durably queued**, **last-resort banner** (section 5).

## 8. The statements

One numbered statement per behavior, grouped by the script that implements it. Numbering is the
inventory's and is stable: the plan names statements by these numbers.
### 8.1 The alerter entry: snapshot, lock and checkpoint

S001. A single-instance kernel lock (`/usr/bin/lockf -s -t 0` on a held fd 9) is taken BEFORE the
      cursor is read and held through route, send and checkpoint, so exactly one of two overlapping
      WatchPaths invocations delivers the batch and advances the cursor.
      Source: `executable_results-alerter.sh:82-88 _take_single_instance_lock`, `:107 main`.
      Pin: `T-CONCURRENT-one-notification: two parallel runs deliver a batch exactly once`
           at test/e2e/osquery-alerter-concurrency.bats:48

S002. A contended run is a clean no-op: it exits 0 without reading the cursor, sending, or writing.
      Source: `executable_results-alerter.sh:107 main`.
      Pin: `T-CONCURRENT-one-notification: two parallel runs deliver a batch exactly once`
           at test/e2e/osquery-alerter-concurrency.bats:48

S003. A host with no `lockf` (any non-darwin box) proceeds UNLOCKED by design, while any other
      lock-setup failure fails CLOSED and skips the run.
      Source: `executable_results-alerter.sh:84-87 _take_single_instance_lock`.
      Pin: UNPINNED. Both concurrency tests skip when `/usr/bin/lockf` is absent, so the fallback path
      is never exercised, and no test drives an unwritable lock directory.

S004. Every external command spawned under the lock closes fd 9 (`9>&-`), so a detached grandchild
      cannot keep the kernel lock held after the run exits.
      Source: `executable_results-alerter.sh:115-217 main` (each `9>&-`).
      Pin: `T-CONCURRENT-fd-hygiene: a detached child never wedges the lock (fd not leaked)`
           at test/e2e/osquery-alerter-concurrency.bats:74

S005. An absent results log is exit 0 with no cursor write.
      Source: `executable_results-alerter.sh:110 main`.
      Pin: UNPINNED. Every suite seeds the log before running the entry.

S006. Size and inode are read with portable tools (`wc -c`, `ls -i` plus `awk`), never BSD `stat -f`,
      and the inode is what notices a rotated or recreated log at the same path.
      Source: `executable_results-alerter.sh:113-118 main`.
      Pin: UNPINNED. No test rotates the log.

S007. A missing or malformed cursor (either field not matching `^[0-9]+$`) replays the WHOLE current
      log from byte 0 and fires a CRIT `osquery cursor reset` page with occurrence id
      `cursor-reset:<inode>:<size>`, best effort, before the batch is processed.
      Source: `executable_results-alerter.sh:125-158 main`.
      Pin: UNPINNED. No test deletes or corrupts the cursor.

S008. A changed inode, or a size smaller than the recorded offset, resets the read offset to 0 so
      nothing is skipped or replayed out of the old file.
      Source: `executable_results-alerter.sh:142-145 main`.
      Pin: UNPINNED.

S009. A size equal to the cursor offset exits 0 without touching the cursor.
      Source: `executable_results-alerter.sh:147 main`.
      Pin: UNPINNED.

S010. The read is bounded to the snapshot window (`tail -c +N | head -c span`) with a sentinel byte
      appended and stripped, so rows appended after the size was captured are not consumed early.
      Source: `executable_results-alerter.sh:166-177 main`.
      Pin: UNPINNED.

S011. Only COMPLETE records advance the cursor: the snapshot is cut at its last newline, the byte
      count is `LC_ALL=C wc -c` of that prefix, and a torn trailing line is retained for the next run.
      Source: `executable_results-alerter.sh:186-192 main`.
      Pin: UNPINNED. This is the at-least-once invariant and nothing exercises a torn line.

S012. The batch flows `normalize_findings | route_findings | render_page` in one command
      substitution, and `pcount` is read from the render with a `// 0` fallback and a numeric guard.
      Source: `executable_results-alerter.sh:200-203 main`.
      Pin: `C1: new_admin_user added fires a CRIT page`
           at test/e2e/osquery-alerter-criteria.bats:121
      also `C3c: agent_authfile_changed (config.toml) does NOT page, lands in the digest spool`
           at test/e2e/osquery-alerter-criteria.bats:147

S013. A batch with one page candidate is titled exactly `🔴 **CRITICAL**`; more than one appends
      ` · <count>`.
      Source: `executable_results-alerter.sh:215-216 main`.
      Pin: UNPINNED. The e2e suite asserts `severity=CRIT` and body content, never the multi-finding
      title.

S014. A page's occurrence id is `<inode>:<prev_offset>:<checkpoint_offset>`, so it is unique per
      occurrence yet stable across a retry of the same byte range.
      Source: `executable_results-alerter.sh:217 main`.
      Pin: UNPINNED.

S015. The cursor is checkpointed ONLY after the batch is durably delivered or spooled, through the
      last complete record, by an atomic write plus rename with fd 9 closed in the child.
      Source: `executable_results-alerter.sh:95 _checkpoint`, `:211-226 main`.
      Pin: `T-CONCURRENT-one-notification: two parallel runs deliver a batch exactly once`
           at test/e2e/osquery-alerter-concurrency.bats:48

S016. A batch with no page candidate still checkpoints, because digest and log-only rows were already
      handled in-stage.
      Source: `executable_results-alerter.sh:211-226 main` (`deliver_ok` starts 1).
      Pin: UNPINNED.

S017. The entry exits 0 even on a delivery hard failure, because a nonzero exit would false-trip the
      watchdog's crash-loop check; only the cursor stays put.
      Source: `executable_results-alerter.sh:221-227 main`.
      Pin: UNPINNED. `run_entry` asserts exit 0 but no test forces `SEND_ALERT_RC` nonzero.

S018. The dispatch library and six pipeline helpers are sourced UNGUARDED from literal absolute paths,
      so an absent one aborts under errexit rather than running a half-assembled pipeline.
      Source: `executable_results-alerter.sh:28-46`.
      Pin: UNPINNED.

S019. The file-integrity triage helper is sourced CONDITIONALLY and its absence prints a loud stderr
      warning naming the path, because pages still fire without it.
      Source: `executable_results-alerter.sh:59-66`.
      Pin: UNPINNED.

S020. Only `main` calls `send_alert`, `_checkpoint` or `exit`; every helper defines functions and
      communicates through newline-delimited JSON.
      Source: `executable_results-alerter.sh:11-14` (the rule), all seven helpers.
      Pin: UNPINNED. This is a structural invariant the S9 spec states; no test asserts it.

### 8.2 normalize.sh

S021. Each raw log line is read as a raw string and parsed per line with `try fromjson catch empty`,
      so one malformed line yields nothing for that line and the surrounding rows still normalize.
      Source: `results-alerter/normalize.sh:61 normalize_findings`.
      Pin: `a malformed line drops out without taking the rest of the batch with it`
           at test/unit/osquery-normalize-and-digest-store.bats:66

S022. jq's exit status is PROPAGATED, not swallowed, so a jq killed partway fails the stage and the
      entry's pipefail leaves the cursor put rather than checkpointing past unjudged rows.
      Source: `results-alerter/normalize.sh:31-40, :100`.
      Pin: UNPINNED.

S023. A row's query name is stripped of its `pack_<pack>_` prefix with `sub("^pack_[^_]+_"; "")`, so
      a packed query and a top-level query reach routing under the same bare name.
      Source: `results-alerter/normalize.sh:66`.
      Pin: `a packed row reaches the routing stage under its bare query name, with its columns and
      action intact` at test/unit/osquery-normalize-and-digest-store.bats:46

S024. Only the pack segment is stripped, so a hyphenated pack name leaves the query's own underscores
      intact.
      Source: `results-alerter/normalize.sh:63-66`.
      Pin: `only the pack segment is stripped, so a hyphenated pack name leaves the query's own
      underscores alone` at test/unit/osquery-normalize-and-digest-store.bats:51

S025. A STRICT allowlist of 25 known query names admits a row; anything else is dropped before it can
      become an alert, whether it arrived packed or top-level.
      Source: `results-alerter/normalize.sh:53-60, :67`.
      Pin: `an unrecognized query name never becomes a finding, whether it arrives packed or
      top-level` at test/unit/osquery-normalize-and-digest-store.bats:79

S026. `heartbeat_canary` is excluded from that allowlist defensively, so a stray liveness row can
      never generate noise.
      Source: `results-alerter/normalize.sh:44-52`.
      Pin: `the heartbeat canary is dropped defensively, so a stray liveness row can never generate
      noise` at test/unit/osquery-normalize-and-digest-store.bats:96

S027. A `file_events_recent` row whose `target_path` lies inside a `.renameio-TempDir` scratch
      directory is dropped as atomic-write churn.
      Source: `results-alerter/normalize.sh:72`.
      Pin: `renameio atomic-write churn is dropped while a real file event on the same query survives`
           at test/unit/osquery-normalize-and-digest-store.bats:101

S028. A `counter == 0` row is discarded as the seeded differential baseline, and an ABSENT counter
      defaults to 1 so it survives.
      Source: `results-alerter/normalize.sh:80`.
      Pin: `a counter==0 membership baseline is discarded while counter>0 and counter-absent rows
      survive` at test/unit/osquery-normalize-and-digest-store.bats:112

S029. Three absolute-state queries are exempt from that discard, because their presence IS the unsafe
      state: `filevault_off`, `remote_access_sharing_state`, `agent_exposure_changed`.
      Source: `results-alerter/normalize.sh:81-83`.
      Pin: `the three absolute-state queries keep their counter==0 row, so an already-unsafe state
      pages on first observation` at test/unit/osquery-normalize-and-digest-store.bats:126

S030. A row that omits `action` is normalized to `changed`, so no later stage special-cases a null.
      Source: `results-alerter/normalize.sh:84`.
      Pin: `a row that omits its action is normalized to changed, so no later stage special-cases a
      null` at test/unit/osquery-normalize-and-digest-store.bats:56

S031. A snapshot-action row stays ONE finding, carried through unexploded, because the absolute-state
      queries are differential now and there is no snapshot array to fan out.
      Source: `results-alerter/normalize.sh:20-24, :99`.
      Pin: `a snapshot-action row stays one finding instead of fanning out its snapshot array`
           at test/unit/osquery-normalize-and-digest-store.bats:61

S032. The enrich path `ep` is chosen per query type: `path` for `es_launchd_writes`,
      `persistence_launchd`, `persistence_startup_items_crontab`, `kernel_extensions_new` and
      `suid_bin_unexpected`; `target_path` for `file_events_recent`; `bundle_path` falling back to
      `path` for `system_extensions_new`; empty everywhere else.
      Source: `results-alerter/normalize.sh:91-98`.
      Pin: `the enrich path names the exact file each query type hands the enricher, and is empty
      where signing does not apply` at test/unit/osquery-normalize-and-digest-store.bats:143

S033. Tabs and newlines inside `ep` are squashed to spaces, so the enrich path stays one renderable
      token.
      Source: `results-alerter/normalize.sh:98`.
      Pin: `a tab inside a path is squashed to a space, so the enrich path stays one renderable token`
           at test/unit/osquery-normalize-and-digest-store.bats:164

S034. The normalized shape emitted is exactly `{q, act, cols, ep}` as one `@json` line per finding,
      with `cols` defaulting to an empty object.
      Source: `results-alerter/normalize.sh:99`.
      Pin: `a packed row reaches the routing stage under its bare query name, with its columns and
      action intact` at test/unit/osquery-normalize-and-digest-store.bats:46

### 8.3 route.sh

S035. `route_severity` is a pure classifier that prints one fixed-vocabulary token (`CRIT`, `NOTICE`
      or `INFO`) per finding, in input order.
      Source: `results-alerter/route.sh:17-49 route_severity`.
      Pin: `a protection in its unsafe state, a new admin account and a new setuid-root binary are
      CRIT` at test/unit/osquery-route.bats:333

S036. `protection_off` is CRIT: `firewall_state` added with `global_state == "0"`, `gatekeeper_state`
      added with `assessments_enabled == "0"`, `sip_state` added with `enabled == "0"`, or any
      `filevault_off` added row (that query emits a row only when nothing is encrypted).
      Source: `results-alerter/route.sh:26-30`.
      Pin: `a protection in its unsafe state, a new admin account and a new setuid-root binary are
      CRIT` at test/unit/osquery-route.bats:333

S037. `new_admin_user` and `suid_bin_unexpected` are CRIT at the classifier level.
      Source: `results-alerter/route.sh:37-38`.
      Pin: `a protection in its unsafe state, a new admin account and a new setuid-root binary are
      CRIT` at test/unit/osquery-route.bats:333

S038. A security-policy-regression row that is not `protection_off` falls to NOTICE, never silently
      to INFO.
      Source: `results-alerter/route.sh:34-35, :39`.
      Pin: `a security-policy row that is not the unsafe state falls to NOTICE, never to INFO`
           at test/unit/osquery-route.bats:348

S039. Persistence queries, both extension queries, `file_events_recent` and `es_launchd_writes` are
      NOTICE.
      Source: `results-alerter/route.sh:40-45`.
      Pin: `persistence, extensions, watched files and endpoint-security writes are NOTICE`
           at test/unit/osquery-route.bats:359

S040. Everything else is INFO: the software-drift queries, listeners, logins and the agent queries.
      Source: `results-alerter/route.sh:46`.
      Pin: `software drift, listeners, logins and the agent queries are INFO`
           at test/unit/osquery-route.bats:374

S041. `route_findings` reads the whole batch into an array first, then classifies it in ONE batched
      `route_severity` call under a CHECKED command substitution, never a process substitution whose
      producer status would be discarded.
      Source: `results-alerter/route.sh:83-110 route_findings`.
      Pin: `a healthy severity batch routes normally and warns about nothing`
           at test/unit/osquery-route.bats:496

S042. A finding whose severity slot is empty or off-vocabulary resolves to CRIT so it PAGES, and the
      shortfall is announced on stderr naming the index, the batch size, the classifier exit status
      and the count returned.
      Source: `results-alerter/route.sh:125-141`.
      Pin: `a finding whose severity never arrived pages, the batch completes, and the shortfall is
      announced` at test/unit/osquery-route.bats:479

S043. Enrichment runs BEFORE the per-detector case, only for a finding with a non-empty enrich path
      and a CRIT or NOTICE base tier, and only when the enricher is executable.
      Source: `results-alerter/route.sh:152-157`.
      Pin: `the signing verdict is attached to a paged finding, trusted or not`
           at test/unit/osquery-route.bats:435

S044. An enricher exit of 10 promotes a NOTICE to CRIT; the promotion is one-directional, never
      quieting, and a missing or erroring enricher leaves the finding surfaced without a Signing
      field (fail-open).
      Source: `results-alerter/route.sh:155-157`.
      Pin: `the extension arms honor the untrusted-signing promotion and the log-only arms ignore it`
           at test/unit/osquery-route.bats:412

S045. A non-empty enricher verdict is attached to the finding as `.signing` for the renderer.
      Source: `results-alerter/route.sh:158`.
      Pin: `the signing verdict is attached to a paged finding, trusted or not`
           at test/unit/osquery-route.bats:435

S046. `firewall_state` and `gatekeeper_state` are LOG-ONLY here, overriding the classifier's CRIT,
      because the 60 second poller owns them and routing them twice would double-page.
      Source: `results-alerter/route.sh:159-163`.
      Pin: `the safe-direction rows and the poller-owned protection reach neither channel`
           at test/unit/osquery-route.bats:399

S047. `sip_state` is LOG-ONLY, because SIP is deliberately off on this host so an on-to-off
      transition cannot occur; the poller does not cover it either.
      Source: `results-alerter/route.sh:164-171`.
      Pin: `the safe-direction rows and the poller-owned protection reach neither channel`
           at test/unit/osquery-route.bats:399

S048. `persistence_startup_items_crontab`, `es_launchd_writes` and `agent_binary_changed` are
      LOG-ONLY regardless of the enrichment verdict.
      Source: `results-alerter/route.sh:172-179`.
      Pin: `the extension arms honor the untrusted-signing promotion and the log-only arms ignore it`
           at test/unit/osquery-route.bats:412

S049. `kernel_extensions_new` pages only when enrichment promoted it to CRIT; a signed one stays
      log-only.
      Source: `results-alerter/route.sh:180-183`.
      Pin: `the extension arms honor the untrusted-signing promotion and the log-only arms ignore it`
           at test/unit/osquery-route.bats:412

S050. `system_extensions_new` pages only when promoted to CRIT; a signed one DIGESTS.
      Source: `results-alerter/route.sh:192-200`.
      Pin: `the extension arms honor the untrusted-signing promotion and the log-only arms ignore it`
           at test/unit/osquery-route.bats:412

S051. `agent_authfile_changed` always DIGESTS and never pages, because those three files are
      non-secret configuration.
      Source: `results-alerter/route.sh:184-191`.
      Pin: `C3c: agent_authfile_changed (config.toml) does NOT page, lands in the digest spool`
           at test/e2e/osquery-alerter-criteria.bats:147
      also `the ambiguous tier digests: a credential file, a new listener, a private key, sudoers`
           at test/unit/osquery-route.bats:405

S052. `listening_ports_non_loopback` digests only its `added` direction and never pages.
      Source: `results-alerter/route.sh:201-204`.
      Pin: `the ambiguous tier digests: a credential file, a new listener, a private key, sudoers`
           at test/unit/osquery-route.bats:405

S053. `agent_secretfile_changed` is forced to CRIT: a change to one of the two true secrets pages.
      Source: `results-alerter/route.sh:206`.
      Pin: `C3b: agent_secretfile_changed pages`
           at test/e2e/osquery-alerter-criteria.bats:141

S054. `agent_exposure_changed` and `remote_access_sharing_state` page only their `added` direction; a
      `removed` row is the good-news direction and is log-only.
      Source: `results-alerter/route.sh:207-217`.
      Pin: `C3a: agent_exposure_changed added pages`
           at test/e2e/osquery-alerter-criteria.bats:135
      also `the safe-direction rows and the poller-owned protection reach neither channel`
           at test/unit/osquery-route.bats:399

S055. `suid_bin_unexpected` pages only its `added` direction.
      Source: `results-alerter/route.sh:218-221`.
      Pin: `the safe-direction rows and the poller-owned protection reach neither channel`
           at test/unit/osquery-route.bats:399

S056. `persistence_launchd` considers only `added` rows; a `/System/Library/*` path is log-only, and
      any `*/LaunchDaemons/*` path pages by path alone without consulting the allowlist.
      Source: `results-alerter/route.sh:222-229`.
      Pin: `default-deny: a reused label, an unknown agent and a LaunchDaemon page, an Apple item is
      skipped` at test/unit/osquery-route.bats:428

S057. A user LaunchAgent is DEFAULT-DENY: `allowlist_verdict` 0 suppresses, and both 1 (not
      allowlisted) and 2 (reused label) page.
      Source: `results-alerter/route.sh:230-254`.
      Pin: `C4b: the same allowlisted label with a different program pages (reused label)`
           at test/e2e/osquery-alerter-criteria.bats:159
      also `C4c: an unknown user LaunchAgent pages (default-deny, operator ruling)`
           at test/e2e/osquery-alerter-criteria.bats:165

S058. A suppress verdict cannot quiet a finding enrichment already promoted to CRIT: an untrusted
      program behind a fully allowlisted label still pages.
      Source: `results-alerter/route.sh:246-252`.
      Pin: `an untrusted program behind a fully allowlisted label still pages, and the trusted one is
      suppressed` at test/unit/osquery-route.bats:423
      also `C4d: an allowlisted-but-untrusted program pages (enrichment beats suppression)`
           at test/e2e/osquery-alerter-criteria.bats:171

S059. Every attacker-controlled column the gate routes on is extracted per field by jq and passed as
      a SEPARATE argv word, so an embedded separator, newline or tab cannot shift field boundaries.
      Source: `results-alerter/route.sh:70-76, :226-245`.
      Pin: `HOSTILE-0x1F: a 0x1F-injected path cannot impersonate an allowlisted tuple; the finding
      pages` at test/e2e/osquery-alerter-hostile-columns.bats:93
      also `HOSTILE-newline: a newline in a column does not split the record; the finding pages`
           at test/e2e/osquery-alerter-hostile-columns.bats:105
      also `HOSTILE-tab: a tab in a column stays opaque; the finding pages`
           at test/e2e/osquery-alerter-hostile-columns.bats:128

S060. Those hostile-column pins are non-vacuous: the genuine allowlisted own agent in the same
      fixture IS suppressed.
      Source: `results-alerter/route.sh:230-254`.
      Pin: `HOSTILE-control: the genuine allowlisted own-agent is suppressed, so the injection pins
      are not vacuous` at test/e2e/osquery-alerter-hostile-columns.bats:119

S061. A `file_events_recent` row in the `ssh` category pages for `authorized_keys` and
      `authorized_keys2` by basename and DIGESTS every other file under `~/.ssh`.
      Source: `results-alerter/route.sh:258-271`.
      Pin: `the page tier reaches stdout, including the two remote-auth file events`
           at test/unit/osquery-route.bats:389
      also `the ambiguous tier digests: a credential file, a new listener, a private key, sudoers`
           at test/unit/osquery-route.bats:405

S062. The `sshd_config` category pages; the `sudoers` category digests; any other category is
      log-only.
      Source: `results-alerter/route.sh:272, :350-354`.
      Pin: `the page tier reaches stdout, including the two remote-auth file events`
           at test/unit/osquery-route.bats:389

S063. Five categories consult `pipeline_verdict` instead: `pipeline_integrity`, `managed_bin`,
      `launch_agents`, `launch_daemons` and `allowlist_file`. It answers page or silent, never digest.
      Source: `results-alerter/route.sh:273-302`.
      Pin: `with nothing able to vouch, tracked edits and an unknown agent page while a neighbour
      stays silent` at test/unit/osquery-route.bats:467
      also `C6: a pipeline_integrity file change with no manifest pages (fail-open)`
           at test/e2e/osquery-alerter-criteria.bats:204

S064. Triage facts are gathered ONLY after the verdict has already decided to page, and every step is
      guarded: an absent helper, a failing helper, non-JSON output, or output whose three members are
      not all strings each leaves the finding exactly as the verdict produced it.
      Source: `results-alerter/route.sh:303-348`.
      Pin: `file-integrity triage facts render exactly when the router attached them`
           at test/unit/osquery-render.bats:240

S065. The triage helper's stderr is deliberately NOT redirected, so its diagnostics reach the
      alerter's launchd log.
      Source: `results-alerter/route.sh:330-336`.
      Pin: UNPINNED.

S066. Detectors with no arm (`new_admin_user`, `filevault_off`, `filevault_state`, the software-drift
      queries, `recent_logins`, `persistence_launchd_overrides`) fall through on their base severity.
      Source: `results-alerter/route.sh:357-359`.
      Pin: `C1: new_admin_user added fires a CRIT page`
           at test/e2e/osquery-alerter-criteria.bats:121
      also `C2: differential filevault_off added (not snapshot) fires a CRIT page`
           at test/e2e/osquery-alerter-criteria.bats:128

S067. Every finding reaches exactly one outcome, and every emitted page candidate is stamped
      `.sev = "CRIT"` in one final jq pass in input order.
      Source: `results-alerter/route.sh:361-377`.
      Pin: `every finding reaches exactly one channel and every page is stamped CRIT`
           at test/unit/osquery-route.bats:446

S068. The emit's exit status is RETURNED, not swallowed, so a jq that died while writing the page
      candidates fails the stage and the entry's pipefail leaves the cursor put.
      Source: `results-alerter/route.sh:364-378`.
      Pin: `a failed page emit reports nonzero instead of a clean nothing-to-page`
           at test/unit/osquery-route.bats:520
      also `a healthy emit writes the candidate and a batch with nothing to page emits nothing, both
      exiting 0` at test/unit/osquery-route.bats:529

### 8.4 allowlist-verdict.sh

S069. `allowlist_verdict <label> <path> <program>` returns 0 to suppress, 2 for a reused label, and 1
      for not-allowlisted, which includes an unreadable file, a degraded entry, and an allowlist the
      manifest cannot vouch for.
      Source: `results-alerter/allowlist-verdict.sh:63-138 allowlist_verdict`.
      Pin: `C4a: a persistence agent fully matching an allowlisted own-agent tuple is suppressed`
           at test/e2e/osquery-alerter-criteria.bats:154
      also `C4b: the same allowlisted label with a different program pages (reused label)`
           at test/e2e/osquery-alerter-criteria.bats:159

S070. The FIRST line whose label matches is taken, in one jq pass over the whole file, with
      `fromjson?` dropping comments and blanks rather than aborting.
      Source: `results-alerter/allowlist-verdict.sh:85-87`.
      Pin: `C4a: a persistence agent fully matching an allowlisted own-agent tuple is suppressed`
           at test/e2e/osquery-alerter-criteria.bats:154

S071. Stored `path` and `program` values expand a leading `~/` to `$HOME/` before comparison, so the
      committed seed file stays user-agnostic.
      Source: `results-alerter/allowlist-verdict.sh:23 _allowlist_verdict_expand_home`, `:91-92`.
      Pin: `C4a: a persistence agent fully matching an allowlisted own-agent tuple is suppressed`
           at test/e2e/osquery-alerter-criteria.bats:154

S072. A degraded label-only entry (empty stored path or program) returns 1, never a suppression on
      the bare label.
      Source: `results-alerter/allowlist-verdict.sh:94-96`.
      Pin: UNPINNED. No fixture writes a label-only entry.

S073. A divergence on path or program is 2, a reused label, distinct from a miss.
      Source: `results-alerter/allowlist-verdict.sh:98`.
      Pin: `C4b: the same allowlisted label with a different program pages (reused label)`
           at test/e2e/osquery-alerter-criteria.bats:159

S074. A PINNED entry re-hashes the ON-DISK plist at decision time with `shasum -a 256` and returns 2
      when it no longer matches, which defeats a same-label, same-path, same-program rewrite.
      Source: `results-alerter/allowlist-verdict.sh:116-118`.
      Pin: UNPINNED. Every fixture entry is unpinned.

S075. An UNPINNED entry instead requires the root-owned manifest to vouch for the plist's current
      content, mode and owner, and returns 1 (degraded, not diverged) when it cannot.
      Source: `results-alerter/allowlist-verdict.sh:119-126`.
      Pin: `C4a: a persistence agent fully matching an allowlisted own-agent tuple is suppressed`
           at test/e2e/osquery-alerter-criteria.bats:154 (the harness blesses each unpinned entry's
      plist into the fixture manifest at `:56-89`, so the suppression only happens because this holds)

S076. The LAST gate, and only on the suppress path, is that the manifest must also vouch for the
      ALLOWLIST FILE itself; an unvouched allowlist suppresses nothing.
      Source: `results-alerter/allowlist-verdict.sh:127-137`.
      Pin: `with nothing able to vouch, tracked edits and an unknown agent page while a neighbour
      stays silent` at test/unit/osquery-route.bats:467

S077. That gate is placed last on purpose: a reused label and a miss already page, so gating them
      would spend a hash and a stat to reach an answer that was never in doubt.
      Source: `results-alerter/allowlist-verdict.sh:131-135`.
      Pin: UNPINNED. Ordering is not observable through the return value.

S078. The manifest vouch is REUSED by name (`declare -F _pipeline_tuple_settles`), never
      reimplemented, and a partial install that lacks it fails toward paging.
      Source: `results-alerter/allowlist-verdict.sh:58-61 _allowlist_path_is_manifest_vouched`.
      Pin: UNPINNED.

### 8.5 pipeline-verdict.sh

S079. `pipeline_verdict <target> <event_hash> <verb>` returns 0 to PAGE and 1 to stay SILENT.
      Source: `results-alerter/pipeline-verdict.sh:455-476 pipeline_verdict`.
      Pin: `with nothing able to vouch, tracked edits and an unknown agent page while a neighbour
      stays silent` at test/unit/osquery-route.bats:467

S080. A path that is not pipeline infrastructure is an untracked neighbor and is SILENT.
      Source: `results-alerter/pipeline-verdict.sh:458`.
      Pin: `with nothing able to vouch, tracked edits and an unknown agent page while a neighbour
      stays silent` at test/unit/osquery-route.bats:467

S081. A `DELETED` verb on a tracked path always pages: there are no bytes left to confirm.
      Source: `results-alerter/pipeline-verdict.sh:460`.
      Pin: UNPINNED.

S082. A symlink or a non-regular file standing at a tracked path pages immediately, without the
      rehash delay or the settle wait.
      Source: `results-alerter/pipeline-verdict.sh:461-470`.
      Pin: UNPINNED.

S083. An event carrying an EMPTY hash is the atomic-rename shape, and only that shape pays a
      `OSQUERY_PIPELINE_REHASH_DELAY` (default 0.3 s) pause before hashing.
      Source: `results-alerter/pipeline-verdict.sh:472-473`.
      Pin: UNPINNED.

S084. The event digest is NEVER a trust input: every suppression decision is made against the file's
      CURRENT content, re-read at decision time, which closes the swap-after-the-event race.
      Source: `results-alerter/pipeline-verdict.sh:449-454, :304-337
      _pipeline_deployed_state_is_known_good`.
      Pin: UNPINNED.

S085. The tracked set is four patterns: everything under `~/.local/libexec/osquery/`, our own
      `~/Library/LaunchAgents/com.webdavis.osquery-*.plist`, the ONE exact file
      `~/.config/osquery/page-launchd-allowlist.txt`, and manifest-driven paths under `~/.local/bin`
      or `~/.local/libexec`.
      Source: `results-alerter/pipeline-verdict.sh:397-406 _pipeline_is_tracked`.
      Pin: `with nothing able to vouch, tracked edits and an unknown agent page while a neighbour
      stays silent` at test/unit/osquery-route.bats:467

S086. The plist arm is anchored to `$HOME/Library/LaunchAgents`, never matched by basename, so a
      rogue `com.webdavis.osquery-*.plist` under `/Library` falls through to the persistence detector
      instead of paging forever as watched-but-unmanifestable.
      Source: `results-alerter/pipeline-verdict.sh:378-385`.
      Pin: UNPINNED.

S087. The allowlist is tracked as one EXACT FILE, never by its directory, because
      `~/.config/osquery` also holds the webhook secret, the daemon config, `packs/` and the writer's
      lock file.
      Source: `results-alerter/pipeline-verdict.sh:389-396, :402`.
      Pin: UNPINNED.

S088. A `~/.local/bin` or `~/.local/libexec` path is tracked only when the managed-bin manifest names
      it, so a self-updating third-party shim is an untracked neighbor.
      Source: `results-alerter/pipeline-verdict.sh:436-447 _managed_bin_is_tracked`.
      Pin: UNPINNED.

S089. THE FAIL-SAFE HINGE, inverted for that arm: a missing, unreadable, empty or untrustworthy
      managed-bin manifest tracks EVERYTHING under those directories rather than nothing.
      Source: `results-alerter/pipeline-verdict.sh:422-431, :439-440`.
      Pin: UNPINNED.

S090. A target is judged against EXACTLY ONE manifest, chosen by prefix, so the two lists can never
      vouch for each other's files.
      Source: `results-alerter/pipeline-verdict.sh:278-284 _pipeline_manifest_for`.
      Pin: UNPINNED.

S091. A manifest is trustworthy only when it is root-owned (uid 0) and not group- or world-writable;
      an unreadable mode defaults to 7777 and is refused.
      Source: `results-alerter/pipeline-verdict.sh:226-237 _pipeline_manifest_is_trustworthy`.
      Pin: UNPINNED.

S092. An explicit `OSQUERY_PIPELINE_MANIFEST` or `OSQUERY_MANAGED_BIN_MANIFEST` override skips the
      trust check, which is the test seam; production sets neither.
      Source: `results-alerter/pipeline-verdict.sh:227-228`.
      Pin: `C6: a pipeline_integrity file change with no manifest pages (fail-open)`
           at test/e2e/osquery-alerter-criteria.bats:204 (the suite relies on the seam)

S093. Legitimacy is the EXACT four-column tuple: content hash, mode and owner all bound to that exact
      path. Any one column disagreeing is a page.
      Source: `results-alerter/pipeline-verdict.sh:286-302 _pipeline_manifest_has_tuple`.
      Pin: UNPINNED.

S094. A manifest line is read with the PATH LAST so a path holding spaces is taken whole, and a SHORT
      line leaves the path empty, which can never equal a real target, so it vouches for nothing.
      Source: `results-alerter/pipeline-verdict.sh:296-300`.
      Pin: UNPINNED.

S095. Hashes are compared case-insensitively; mode and uid are compared verbatim against the
      normalized four-octal-digit and decimal forms the readers produce.
      Source: `results-alerter/pipeline-verdict.sh:291, :298-299`.
      Pin: UNPINNED.

S096. An observed column that could not be read is never matched against, so an equally empty
      manifest column cannot vouch for a file nothing was learned about.
      Source: `results-alerter/pipeline-verdict.sh:294`.
      Pin: UNPINNED.

S097. `_pipeline_deployed_state_is_known_good` refuses a symlink and a non-regular file BEFORE
      hashing, and that refusal lives in the shared state reader so every consumer inherits it.
      Source: `results-alerter/pipeline-verdict.sh:328-337`.
      Pin: UNPINNED.

S098. `_pipeline_tuple_settles` waits only when the manifest EXISTS but PREDATES the change: it
      compares the manifest's mtime against the target's inode CHANGE time, because a chmod or chown
      moves ctime alone.
      Source: `results-alerter/pipeline-verdict.sh:351-371`, `:169-171 _pipeline_change_time`.
      Pin: UNPINNED.

S099. The settle budget (`OSQUERY_PIPELINE_SETTLE_SECONDS`, default 5) is spent ONCE PER ALERTER
      INVOCATION, not once per finding, so creating N files cannot stall the pipeline for N times the
      bound.
      Source: `results-alerter/pipeline-verdict.sh:138-146, :364-369`.
      Pin: UNPINNED. Both e2e suites set the budget to 0.

S100. The state is re-read on EVERY retry inside the settle loop, including the file-kind check, so a
      link swapped in mid-window is refused rather than blessed by a check that ran once.
      Source: `results-alerter/pipeline-verdict.sh:322-327, :366-369`.
      Pin: UNPINNED.

S101. The mode reader asks GNU `stat -c '%a'` first and BSD `stat -f '%p'` second (never `%Lp`, which
      prints only the low nine bits), range-checks the raw value, and returns exactly four octal
      digits so a setuid bit cannot read back as an ordinary mode.
      Source: `results-alerter/pipeline-verdict.sh:187-193 _pipeline_file_mode`.
      Pin: `a setuid bit on a live file reads as drift, not as a matching 0644`
           at test/unit/osquery-converge.bats:332 (the converge tool's own reader, the same idiom;
      the verdict's copy is UNPINNED)

### 8.6 file-integrity-triage.sh

S102. `file_integrity_triage <target>` prints one compact JSON object with exactly three string
      members: `recorded`, `ondisk` and `upgrade`, built with `jq -n --arg`, never by interpolation.
      Source: `results-alerter/file-integrity-triage.sh:377-385`.
      Pin: `file-integrity triage facts render exactly when the router attached them`
           at test/unit/osquery-render.bats:240

S103. Every function returns 0 on every input, so a page is never lost to a correlation failure; a jq
      that cannot run yields `{}`.
      Source: `results-alerter/file-integrity-triage.sh:30-36, :383`.
      Pin: UNPINNED.

S104. `recorded` is the first twelve hex characters of the manifest's hash for that exact path, or a
      stated reason: `manifest lookup unavailable`, `manifest unreadable`, or `not in the manifest`.
      Source: `results-alerter/file-integrity-triage.sh:146-174`.
      Pin: UNPINNED. The render test supplies the object rather than producing it.

S105. A hash is validated against `^[0-9a-f]{64}$` before it is sliced, so a stat error string or a
      truncated column renders as nothing rather than a plausible digest.
      Source: `results-alerter/file-integrity-triage.sh:131-136 _file_integrity_short`.
      Pin: UNPINNED.

S106. `ondisk` names a link rather than following it, and distinguishes `a symbolic link`, `absent`,
      `not a regular file` and `unreadable` from a real digest.
      Source: `results-alerter/file-integrity-triage.sh:181-203`.
      Pin: UNPINNED.

S107. The manifest CHOICE is reused from the verdict by name (`_pipeline_manifest_for`), never
      re-derived, and its absence reports a broken lookup rather than resolving every path to the
      wrong list.
      Source: `results-alerter/file-integrity-triage.sh:146-152`.
      Pin: UNPINNED.

S108. The upgrade record is accepted only when it is a READABLE REGULAR FILE, because a readable FIFO
      would block `read` forever while the alerter holds its single-instance lock. Symlinks are
      followed deliberately (`-f` judges the final target).
      Source: `results-alerter/file-integrity-triage.sh:236-250`.
      Pin: UNPINNED.

S109. The record is read in ONE bounded snapshot (`head -c MAX_BYTES+1`, default 262144) and refused
      WHOLE when over the cap, so two opens cannot straddle the producer's atomic rename and pair one
      generation's timestamp with another's rows.
      Source: `results-alerter/file-integrity-triage.sh:251-272`.
      Pin: UNPINNED.

S110. Line 1 must be `<epoch>\t<ISO 8601 UTC>` matching both patterns, or the whole record is refused
      with a stderr diagnostic.
      Source: `results-alerter/file-integrity-triage.sh:273-284`.
      Pin: UNPINNED.

S111. A record older than `OSQUERY_UPGRADE_RECORD_WINDOW_DAYS` (3), or dated in the FUTURE, falls out
      of the window arm and is reported with its timestamp rather than offered as an explanation.
      Source: `results-alerter/file-integrity-triage.sh:286-294`.
      Pin: UNPINNED.

S112. Package rows are decoded BY EXPANSION, never by `IFS=$'\t' read`, because a tab is IFS
      whitespace and a run of tabs would collapse the empty column an add or a remove writes, which
      rendered an added package as a removed one.
      Source: `results-alerter/file-integrity-triage.sh:296-314`.
      Pin: UNPINNED.

S113. A row whose state is not `added`, `removed` or `changed`, or a record over
      `OSQUERY_UPGRADE_RECORD_MAX_ROWS` (500), refuses the WHOLE record rather than skipping a row.
      Source: `results-alerter/file-integrity-triage.sh:316-333`.
      Pin: UNPINNED.

S114. Correlation is a NAME match against the flagged file's basename only, and the rendered sentence
      says in as many words that a name match is not proof.
      Source: `results-alerter/file-integrity-triage.sh:335-345`.
      Pin: UNPINNED.

S115. With no name match the line states what the run DID change, listing at most
      `OSQUERY_UPGRADE_RECORD_NAME_CAP` (5) names plus an "and N more" tail, or says the run recorded
      no package change (never "changed nothing", because a run still in flight reads the same).
      Source: `results-alerter/file-integrity-triage.sh:347-367`.
      Pin: UNPINNED.

S116. No string in this file contains an apostrophe, because every value reaches the page through a
      bash single-quoted jq program.
      Source: `results-alerter/file-integrity-triage.sh:61-63`.
      Pin: UNPINNED. The repository has no lint for this.

### 8.7 digest-store.sh

S117. `digest_append <finding-json>` appends exactly one NDJSON line of six DERIVED fields and never
      copies the whole columns object, so a raw sha256 or a secret column never reaches the spool.
      Source: `results-alerter/digest-store.sh:28-56 digest_append`.
      Pin: `one append records a single line of derived triage fields and nothing else`
           at test/unit/osquery-normalize-and-digest-store.bats:171
      also `a finding's raw hash and secret column never reach the spool, only its path`
           at test/unit/osquery-normalize-and-digest-store.bats:205

S118. Appends accumulate, one line per finding.
      Source: `results-alerter/digest-store.sh:52`.
      Pin: `appends accumulate, one line per finding, so the daily digest sees every one`
           at test/unit/osquery-normalize-and-digest-store.bats:183

S119. The spool directory is chmod 700 BEFORE any file exists and the file is chmod 600 after each
      append, because the line carries full filesystem paths.
      Source: `results-alerter/digest-store.sh:33-36, :56`.
      Pin: `the spool is private: a 700 directory and a 600 file`
           at test/unit/osquery-normalize-and-digest-store.bats:192

S120. A `listening_ports_non_loopback` finding is identified by process name, address and port
      together, not by one column.
      Source: `results-alerter/digest-store.sh:47-49`.
      Pin: `a listening-port finding is identified by name, address and port together`
           at test/unit/osquery-normalize-and-digest-store.bats:198

S121. A failed append never aborts detection: the function always returns success.
      Source: `results-alerter/digest-store.sh:52-56`.
      Pin: `a failed append never aborts the detection path`
           at test/unit/osquery-normalize-and-digest-store.bats:217

S122. A failed append says so on stderr naming ONLY the spool path (never the finding), because jq's
      own stderr would quote the columns the privacy posture keeps out of readable files.
      Source: `results-alerter/digest-store.sh:38-55`.
      Pin: `a failed append says so on stderr, naming the spool it could not write`
           at test/unit/osquery-normalize-and-digest-store.bats:225

S123. A failed append leaves no partial line behind.
      Source: `results-alerter/digest-store.sh:42-52` (one jq write).
      Pin: `a failed append leaves no partial line behind`
           at test/unit/osquery-normalize-and-digest-store.bats:236

### 8.8 render-page.sh

S124. `render_page` slurps the enriched findings, selects the CRIT ones, and prints one JSON object
      `{pcount, pbody}`.
      Source: `results-alerter/render-page.sh:29-178 render_page`.
      Pin: `a CRIT finding renders a plain-English header, its decision fields and a next step`
           at test/unit/osquery-render.bats:172

S125. Every rendered value passes through ONE sanitize chokepoint: backticks stripped, `\r`, `\n` and
      tab squashed to spaces, truncated at 240 characters with an explicit marker, then wrapped in a
      Discord inline-code span.
      Source: `results-alerter/render-page.sh:35-42 code`.
      Pin: `a field value over 240 characters is truncated behind a marker`
           at test/unit/osquery-render.bats:193
      also `an embedded newline in any rendered column stays on one line, so no signing line can be
      forged` at test/unit/osquery-render.bats:267
      also `an embedded carriage return is squashed the same way a newline is`
           at test/unit/osquery-render.bats:282

S126. Each detector has a plain-English header; a protection query renders "<name> turned OFF" at
      CRIT and "<name> changed" otherwise, and an unmapped query renders its name with underscores
      replaced by spaces.
      Source: `results-alerter/render-page.sh:43-84 protname, header`.
      Pin: `a CRIT finding renders a plain-English header, its decision fields and a next step`
           at test/unit/osquery-render.bats:172

S127. A `file_events_recent` header is chosen by basename first (our own osquery plists read
      "Security tooling changed") and then by category.
      Source: `results-alerter/render-page.sh:70-82`.
      Pin: `file-integrity triage facts render exactly when the router attached them`
           at test/unit/osquery-render.bats:240

S128. `agent_authfile_changed` and `agent_secretfile_changed` render the file's BASENAME only, never
      its path and never a sha256, because the body fans out to Discord.
      Source: `results-alerter/render-page.sh:104-107`.
      Pin: `a secret or credential file is rendered by basename, never with its path or its content
      hash` at test/unit/osquery-render.bats:180
      also `C7: a paged agent_secretfile_changed body shows the basename only, never the path or
      sha256` at test/e2e/osquery-alerter-criteria.bats:211

S129. A signing verdict matching `unsigned|untrusted|ad-hoc|unverified|no authority` renders bolded
      with a warning glyph; any other verdict renders plainly, and markdown metacharacters are
      stripped from the authority first.
      Source: `results-alerter/render-page.sh:91-98 fields`.
      Pin: `an embedded newline in any rendered column stays on one line, so no signing line can be
      forged` at test/unit/osquery-render.bats:267

S130. Triage facts render as exactly two extra lines when `.triage` is an object, and nothing at all
      when it is absent.
      Source: `results-alerter/render-page.sh:117-121`.
      Pin: `file-integrity triage facts render exactly when the router attached them`
           at test/unit/osquery-render.bats:240

S131. Every next-step command that quotes a path shell-quotes it with `@sh` before wrapping it in
      code, so a quote-breaking or command-substitution path cannot execute if the operator pastes
      the line.
      Source: `results-alerter/render-page.sh:126-155 nextstep`.
      Pin: `a quote-breaking path never executes in a codesign next-step command`
           at test/unit/osquery-render.bats:291
      also `a quote-breaking path never executes in a cat, sudo cat or shasum next-step command`
           at test/unit/osquery-render.bats:301
      also `a command-substitution path never executes in a rendered next-step command`
           at test/unit/osquery-render.bats:316

S132. The page renders at most EIGHT blocks, followed by a marker naming how many CRIT findings were
      dropped, while `pcount` still counts every one.
      Source: `results-alerter/render-page.sh:158-168`.
      Pin: `the page renders at most eight blocks and counts every CRIT finding it dropped`
           at test/unit/osquery-render.bats:201

S133. A FINAL hard cap truncates the whole body at 1900 characters with its own marker, because eight
      blocks with long fields can still exceed the 2000-character delivery limit.
      Source: `results-alerter/render-page.sh:169-175`.
      Pin: `the page body is hard-capped below the 2000-char delivery limit`
           at test/unit/osquery-render.bats:224

### 8.9 enrich-finding.sh

S134. The exit status is the machine signal: 0 means trusted or not-applicable, 10 means untrusted or
      undeterminable code. The stdout string is a short single-line human fact.
      Source: `executable_enrich-finding.sh:12-22`.
      Pin: UNPINNED. Both e2e suites replace it with a two-branch stub.

S135. An empty path argument exits 0 with no output.
      Source: `executable_enrich-finding.sh:26-27`.
      Pin: UNPINNED.

S136. `codesign -dv --verbose=2` failing, or reporting "not signed", is `UNSIGNED` and exit 10; an
      "adhoc" match is `ad-hoc signature (untrusted)` and exit 10; a present-but-empty authority is
      `signed, no authority (untrusted)` and exit 10.
      Source: `executable_enrich-finding.sh:48-67 assess_code`.
      Pin: UNPINNED.

S137. A named authority exits 0: Apple and `Software Signing` render as `signed: Apple`, a Developer
      ID renders with its team name, and any other authority renders verbatim without promotion.
      Source: `executable_enrich-finding.sh:68-73`.
      Pin: UNPINNED.

S138. A `.plist` resolves its payload from `Program`, then `ProgramArguments.0`; an unresolvable
      program is `launchd job, no program resolved (untrusted)` and exit 10.
      Source: `executable_enrich-finding.sh:86-93`.
      Pin: UNPINNED.

S139. A launchd job whose program is one of fourteen interpreter basenames is NOT escalated: it
      reports the script it runs (first existing absolute-path argument in positions 1 through 5) and
      exits 0, because the operator's own script agents are that exact shape.
      Source: `executable_enrich-finding.sh:78-114 is_interpreter`.
      Pin: UNPINNED.

S140. A quarantine xattr appends `, downloaded` to the fact string, best effort.
      Source: `executable_enrich-finding.sh:34-39 quarantine_note`.
      Pin: UNPINNED.

S141. A bundle suffix (`.app`, `.kext`, `.systemextension`, `.dext`, `.appex`) is assessed directly;
      anything else is assessed only when `file` reports Mach-O, and a non-code file yields stat
      context (owner, mode, modified) with exit 0.
      Source: `executable_enrich-finding.sh:119-138`.
      Pin: UNPINNED.

### 8.10 alert-dispatch.sh, the shared delivery library

S142. `send_alert <severity> <title> <detail> [sound] [occurrence_id]` ALWAYS fires the local
      notification first, then returns early for any severity other than `CRIT`.
      Source: `executable_alert-dispatch.sh:1186-1193 send_alert`.
      Pin: UNPINNED. Every suite replaces `send_alert` with a spy.

S143. An empty `sound` argument means a locally silent notification AND `tier=muted` in the webhook
      body; a non-empty sound means `tier=page`.
      Source: `executable_alert-dispatch.sh:1207`, `:693-697 _notify_locally`.
      Pin: `B2: the healthy message is silent (empty sound), a proof-of-life never pings`
           at test/integration/osquery-heartbeat.bats:152 (the caller's side; the library's threading
      of `tier` is UNPINNED)

S144. The return contract is 0 when the page was DELIVERED or durably STORED, and nonzero ONLY on a
      hard failure where the write-ahead persist itself failed.
      Source: `executable_alert-dispatch.sh:1180-1185, :1221-1231, :1246, :1256, :1262`.
      Pin: UNPINNED.

S145. Delivery is WRITE-AHEAD: the page is persisted as a `pending_alerts` row BEFORE the first
      network attempt and deleted only after a confirmed 2xx.
      Source: `executable_alert-dispatch.sh:1217-1231, :1250-1257`.
      Pin: UNPINNED.

S146. A failed persist logs `STORE-FAILED`, fires a DURABLE loud local banner seeded by the request
      id, and returns nonzero so the caller does not advance its cursor.
      Source: `executable_alert-dispatch.sh:1221-1231`.
      Pin: UNPINNED.

S147. A missing webhook secret logs `STORED-NOSECRET`, fires a durable loud banner, and returns 0:
      the page is safely stored and the drain sends it once the secret returns.
      Source: `executable_alert-dispatch.sh:1236-1247`.
      Pin: UNPINNED.

S148. A delivery that failed after retries logs `STORED` with the HTTP status and still returns 0,
      leaving the row for the drain.
      Source: `executable_alert-dispatch.sh:1258-1262`.
      Pin: UNPINNED.

S149. The request id is `osquery-<32 hex>` derived from the caller's occurrence identity when one is
      threaded, and from a per-call unique seed (timestamp, pid, a monotonic per-process sequence,
      `$RANDOM`, the body) otherwise.
      Source: `executable_alert-dispatch.sh:1134-1136 _derive_request_id`, `:1206-1215`.
      Pin: UNPINNED.

S150. The webhook body is exactly `{event_type, host, tier, ts, alert:{title, detail}}`, built with
      `jq -cn --arg`, with the host from `hostname -s` and `ts` the occurrence epoch.
      Source: `executable_alert-dispatch.sh:1124-1128 _build_webhook_body`.
      Pin: UNPINNED.

S151. A clock read that is not numeric falls back to occurrence timestamp 0 rather than blocking the
      page.
      Source: `executable_alert-dispatch.sh:1198-1200`.
      Pin: UNPINNED.

S152. `_attempt_alert_delivery` retries a transient outcome (429, any 5xx, or curl's 000) up to three
      times with growing backoff, stops early on any other non-2xx, prints the final status, and
      returns 0 only for a 2xx.
      Source: `executable_alert-dispatch.sh:1143-1169`.
      Pin: UNPINNED.

S153. A failed or empty signature stops the attempt BEFORE any POST and prints `signing-failed`,
      because an unchecked assignment inside an `if` would POST an empty signature and a 2xx would
      then delete the write-ahead record.
      Source: `executable_alert-dispatch.sh:1145-1153`.
      Pin: UNPINNED.

S154. There is exactly ONE POST site, carrying `-X POST`, `Content-Type: application/json`,
      `X-Webhook-Signature`, `X-Request-ID` and `--max-time 5`.
      Source: `executable_alert-dispatch.sh:1052-1059 _post_alert_to_webhook`.
      Pin: UNPINNED.

S155. The HMAC is built by hand from SHA-256 using `openssl dgst` (not the OpenSSL-3-only `mac`), and
      the key is a function argument that never reaches a child argv while the bytes to hash arrive
      only on stdin, so neither key nor body can appear in `ps`.
      Source: `executable_alert-dispatch.sh:70-109 _sha256_hex_of_stdin, _hmac_sha256_hex`.
      Pin: UNPINNED. The comment says a test pins it byte-identical to `openssl dgst -hmac`; that
      test no longer exists.

S156. The signing key is the environment override, else the FIRST LINE of
      `~/.config/osquery/webhook-secret` with carriage returns stripped, so a CRLF file cannot corrupt
      it. An absent key prints nothing and the caller decides.
      Source: `executable_alert-dispatch.sh:1039-1046 _read_webhook_secret`.
      Pin: UNPINNED.

S157. Every SQL statement runs through one executor that prepends `.bail on`, `busy_timeout=5000` and
      `journal_mode=WAL`, buffers query rows and prints them only after a fully successful run, and
      retries a `database is locked` failure up to five times with a 0.1 s pause.
      Source: `executable_alert-dispatch.sh:146-176 _osquery_alerts_db_exec`.
      Pin: `the counters report how many pages are queued and how many the drain gave up on`
           at test/unit/osquery-alert-dispatch.bats:97

S158. Every text value interpolated into SQL has its single quotes doubled through a `$single_quote`
      helper variable, because the inline spellings go wrong under bash 3.2.
      Source: `executable_alert-dispatch.sh:220-228, :275-284, :353-355, :381-385, :517-519, :927-929`.
      Pin: `an apostrophe in the page URL is stored intact instead of being rejected by corrupted SQL`
           at test/unit/osquery-alert-dispatch.bats:120
      also `the drain SELECT carries an apostrophe URL through to the delivery attempt`
           at test/unit/osquery-alert-dispatch.bats:125
      also `an apostrophe in a dead-letter reason completes the move out of the pending queue`
           at test/unit/osquery-alert-dispatch.bats:132
      also `an apostrophe request id survives retry bookkeeping and its delete-by-id`
           at test/unit/osquery-alert-dispatch.bats:139

S159. Each table's lazy `CREATE TABLE IF NOT EXISTS` and the insert that needed it commit in ONE
      `BEGIN IMMEDIATE ... COMMIT` batch, so no kill window exists where the schema exists but the
      row vanished; both batches are idempotent so the locked-database retry may replay them.
      Source: `executable_alert-dispatch.sh:229-252 _osquery_store_alert_row`,
      `:285-309 _osquery_store_local_notification_row`.
      Pin: UNPINNED.

S160. The store directory is chmod 700 and the database chmod 600 after every write.
      Source: `executable_alert-dispatch.sh:215-217, :250, :270-272, :307`.
      Pin: UNPINNED.

S161. A store refuses a URL that is empty or carries whitespace or a control character, because the
      drain's row export is tab-separated and an embedded separator would garble the row into an
      undeliverable shape.
      Source: `executable_alert-dispatch.sh:661-669 _store_undelivered_alert`.
      Pin: UNPINNED.

S162. The body is base64-encoded under `set -o pipefail` in a subshell and an empty result is
      rejected, so an encoder failure fails the store rather than persisting an empty body.
      Source: `executable_alert-dispatch.sh:670-679`.
      Pin: UNPINNED.

S163. Re-storing the same occurrence is idempotent (`ON CONFLICT(request_id) DO NOTHING`, and
      `INSERT OR IGNORE` for the local queue), while two distinct occurrences never collide.
      Source: `executable_alert-dispatch.sh:240-244, :297-300`.
      Pin: UNPINNED.

S164. Due rows are selected by `next_attempt_after <= now` and ordered `occurrence_ts,
      sequence_number`, so equal timestamps or a backward clock step still drain in insert order.
      Source: `executable_alert-dispatch.sh:535-542 _osquery_pending_alert_rows`.
      Pin: `T-DRAIN-mixed-batch-full-drain: a mixed batch drains completely in one pass, each row
      handled by class, none starved` at test/integration/osquery-drain-continuation.bats:71

S165. `_deliver_pending_alert_row` gives up BEFORE any send on a row past a threshold, moving it to
      `dead_letter_alerts` and returning nonzero so the pass can count it.
      Source: `executable_alert-dispatch.sh:561-570`.
      Pin: `T-DRAIN-mixed-batch-full-drain: a mixed batch drains completely in one pass, each row
      handled by class, none starved` at test/integration/osquery-drain-continuation.bats:71

S166. A row is over threshold when `attempts >= OSQUERY_DRAIN_MAX_ATTEMPTS` (20) or its age exceeds
      `OSQUERY_DRAIN_MAX_AGE_SECONDS` (604800); attempts is checked first so a maxed-out row names
      attempts, and a zero or future `created_at` is never aged out blind.
      Source: `executable_alert-dispatch.sh:425-450 _osquery_row_over_threshold_reason`.
      Pin: UNPINNED. The drain suite drives the permanent-status path, not the thresholds.

S167. A stored row whose URL is not under `http://127.0.0.1:8644/` is SKIPPED, never sent off-box.
      Source: `executable_alert-dispatch.sh:571-574`, `:1064` (the localhost-only rule).
      Pin: UNPINNED.

S168. A permanent HTTP status (401, 403, 404, 413) moves the row to `dead_letter_alerts` in ONE
      transaction whose insert and delete commit together, so a crash leaves the record in exactly one
      table; the delete fires only once the dead-letter copy exists.
      Source: `executable_alert-dispatch.sh:585-601`, `:377-414 _osquery_dead_letter_alert_row`.
      Pin: `T-DRAIN-continue-past-permanent: a permanent poison row in the middle does not starve the
      rows behind it` at test/integration/osquery-drain-continuation.bats:26

S169. Any other non-2xx is transient: attempts goes up by one and `next_attempt_after` moves to
      `now + base * (attempts + 1) + random_offset`, computed inside the single UPDATE so the
      read-modify-write is atomic under concurrent drains.
      Source: `executable_alert-dispatch.sh:335-366 _osquery_record_retry_failure`.
      Pin: `T-DRAIN-mixed-batch-full-drain: a mixed batch drains completely in one pass, each row
      handled by class, none starved` at test/integration/osquery-drain-continuation.bats:71

S170. The randomized offset is drawn once per call from `$RANDOM % (max + 1)`, defaults to a full
      base-width spread, only ever DELAYS a retry, and is disabled by setting it to 0.
      Source: `executable_alert-dispatch.sh:316-334, :349-352`.
      Pin: UNPINNED.

S171. A malformed row (an empty required field, or a body that does not base64-decode) is skipped
      with a `MALFORMED-ROW` log line naming the row and the reason, and the row is RETAINED.
      Source: `executable_alert-dispatch.sh:557-560, :575-578`.
      Pin: `T-DRAIN-continue-past-malformed: an undecodable poison row in the middle is skipped and
      the rows behind it still deliver` at test/integration/osquery-drain-continuation.bats:49

S172. The drain loop never returns nonzero and never aborts under errexit: each row's delivery runs
      inside an `if`, and a failing row does not block the rows behind it.
      Source: `executable_alert-dispatch.sh:621-647 _drain_pending_alert_rows`.
      Pin: `T-DRAIN-errexit-first-row-failure: under set -e a failing FIRST record does not abort the
      drain; the queue finishes and exit is 0` at test/integration/osquery-drain-continuation.bats:114

S173. The drain PRINTS the number of rows this pass dead-lettered, as the only thing on stdout, and
      `retry_undelivered_alerts` fires exactly ONE durable loud local CRIT banner when that count is
      positive, never one per record. A zero count stays silent.
      Source: `executable_alert-dispatch.sh:613-647`, `:1079-1095`.
      Pin: `T-DRAIN-continue-past-permanent: a permanent poison row in the middle does not starve the
      rows behind it` at test/integration/osquery-drain-continuation.bats:26

S174. `retry_undelivered_alerts` sweeps the LOCAL notification queue FIRST, before the secret gate, so
      a fresh local row the alert drain persists mid-pass waits for the next tick rather than being
      attempted twice in one pass.
      Source: `executable_alert-dispatch.sh:1067-1078`.
      Pin: UNPINNED.

S175. An absent database is a quiet no-op, and each drain gates on its table existing so a database
      created by one queue does not spray "no such table" on every tick.
      Source: `executable_alert-dispatch.sh:185-191 _osquery_table_exists`, `:625-628`, `:1023`,
      `:1068`.
      Pin: `a counter reads zero while its table is still un-bootstrapped, not an error`
           at test/unit/osquery-alert-dispatch.bats:91

S176. `osquery_pending_alert_count` and `osquery_dead_letter_count` are public read-only counters
      that print a bare integer and never modify stored data.
      Source: `executable_alert-dispatch.sh:1104-1115`, `:475-504 _osquery_alert_row_count`.
      Pin: `the counters report how many pages are queued and how many the drain gave up on`
           at test/unit/osquery-alert-dispatch.bats:97

S177. A counter reads 0 for the two legitimately empty cases (an absent database, an un-bootstrapped
      table) and prints NOTHING and returns nonzero for any other failure once the file exists, so a
      broken store is never a false all-clear.
      Source: `executable_alert-dispatch.sh:475-504`.
      Pin: `both counters read zero before anything has ever been stored`
           at test/unit/osquery-alert-dispatch.bats:80
      also `an unreadable store fails the probe instead of reporting a false zero`
           at test/unit/osquery-alert-dispatch.bats:108

S178. The counter opens the database `-readonly` and never creates it, and deliberately does NOT use
      `immutable=1`, because that would skip the WAL and undercount committed rows a checkpoint has
      not folded back.
      Source: `executable_alert-dispatch.sh:465-474, :487`.
      Pin: `a count probe never creates the database it reads`
           at test/unit/osquery-alert-dispatch.bats:85

S179. The ordinary local notification strips Discord markdown (`**` and backticks) before handing
      plain text to `alerter`, backgrounded, or to `osascript` with its failure ignored, so a broken
      notifier never stalls or fails dispatch.
      Source: `executable_alert-dispatch.sh:688-709 _notify_locally`.
      Pin: UNPINNED.

S180. Any text reaching AppleScript is escaped BACKSLASH FIRST and then the quote, because escaping
      only the quote leaves `\"` intact and turns the rest of an attacker-influenced finding into
      AppleScript source.
      Source: `executable_alert-dispatch.sh:49-63 _osquery_applescript_literal`.
      Pin: UNPINNED.

S181. The durable loud banner persists a `pending_local_notifications` row FIRST, then attempts the
      banner, and only a CONFIRMED success deletes the row; the return status is advisory log wording
      only and the function always returns 0.
      Source: `executable_alert-dispatch.sh:819-876 _osquery_notify_local_durable`.
      Pin: UNPINNED.

S182. Confirmation is per channel: `osascript` is synchronous so its exit 0 confirms and deletes
      inline, while `alerter` blocks for the banner's whole lifetime so a backgrounded WATCHER owns
      the confirm and deletes only on exit 0. A caller-facing 0.6 s grace window decides log wording
      only, never persistence.
      Source: `executable_alert-dispatch.sh:764-817 _osquery_show_banner_confirm_delete`.
      Pin: UNPINNED.

S183. The durable banner is DELIBERATELY LOUD for every caller: the sound is fixed to `Funk` here
      rather than taken from the caller, so a muted producer (the heartbeat, the digest) still gets an
      audible alarm when the delivery pipeline itself is broken.
      Source: `executable_alert-dispatch.sh:838-851, :865, :870`.
      Pin: UNPINNED. The comment says two dispatch-suite pins hold the split honest; they no longer
      exist.

S184. A retried local notification renders `occurred <UTC ISO 8601>` as a banner SUBTITLE so a banner
      shown hours late does not read as breaking news; an unrenderable time yields no subtitle, never
      a made-up one.
      Source: `executable_alert-dispatch.sh:988-1014 _redeliver_pending_local_notification`,
      `:912-917 _osquery_epoch_to_iso8601`.
      Pin: UNPINNED.

S185. Local notifications older than `OSQUERY_LOCAL_NOTIFY_MAX_AGE_SECONDS` (86400) are DELETED
      unshown in an expiry pass that runs INDEPENDENTLY of the due filter, with a loud
      `LOCAL-NOTIFY-EXPIRED` log line; a row with a zero occurrence time is never expired blind. There
      is deliberately no dead-letter table and no attempts cap for this queue.
      Source: `executable_alert-dispatch.sh:934-972 _osquery_expire_over_age_local_notifications`.
      Pin: UNPINNED.

S186. The local queue's text columns are hex-encoded IN SQL and decoded by a `sed` plus `printf %b`
      helper, so an embedded tab or newline cannot garble the tab-separated row format.
      Source: `executable_alert-dispatch.sh:878-889 _osquery_hex_to_text`, `:899-907`.
      Pin: UNPINNED.

S187. Only metadata is ever written to the delivery log, never a body or the secret, and a logging
      failure never breaks delivery.
      Source: `executable_alert-dispatch.sh:44-48, :65-68 _osquery_log`.
      Pin: UNPINNED.

### 8.11 drain-undelivered-alerts.sh

S188. The drainer takes a BLOCKING-free single-instance lock derived from the store path
      (`<db>.drain.lock`) and exits 0 immediately when another drain holds it, because that drain
      already sweeps every stored row.
      Source: `executable_drain-undelivered-alerts.sh:59-76 take_single_instance_lock`, `:104-107`.
      Pin: UNPINNED.

S189. A host with no `lockf` proceeds unlocked by design; any other lock-setup failure fails CLOSED.
      Source: `executable_drain-undelivered-alerts.sh:60-75`.
      Pin: UNPINNED.

S190. The stderr silence is scoped to the `exec` with a brace group, because a bare
      `exec 9>>f 2>/dev/null` would redirect the WHOLE script's stderr for good, and stderr is this
      script's only channel since it always exits 0.
      Source: `executable_drain-undelivered-alerts.sh:64-74`.
      Pin: UNPINNED.

S191. The sweep runs inside a SUBSHELL that closes fd 9 with `exec`, not through a scoped
      `9>&-` redirection, because bash implements a scoped close by duplicating to a high fd that a
      forked subshell then inherits (measured: the banner watcher held the lock on fd 10).
      Source: `executable_drain-undelivered-alerts.sh:79-111 main`.
      Pin: UNPINNED.

S192. The exit status is ALWAYS 0: a failure inside a best-effort background sweep must never surface
      as a launchd job error.
      Source: `executable_drain-undelivered-alerts.sh:17-21, :112`.
      Pin: UNPINNED.

### 8.12 canary-freshness.sh and heartbeat.sh

S193. `newest_canary_timestamp` selects `heartbeat_canary` rows by PARSED `.name` from
      `osqueryd.snapshots.log`, prefers the envelope `.unixTime` and falls back to
      `.snapshot[0].unix_time`, and prints the LAST (newest) value or nothing.
      Source: `executable_canary-freshness.sh:39-47`.
      Pin: `B8: freshness is judged from the NEWEST canary row when several exist`
           at test/integration/osquery-heartbeat.bats:264
      also `seam: newest_canary_timestamp returns the newest validated integer, else empty`
           at test/integration/osquery-heartbeat.bats:363

S194. Matching on the PARSED name is whitespace-tolerant, so the read does not couple to osquery's
      compact serialization.
      Source: `executable_canary-freshness.sh:42-44`.
      Pin: `format-tolerance: a spaced-JSON canary reads the same as compact (JSON-semantic reader)`
           at test/integration/osquery-heartbeat.bats:278

S195. `fromjson?` drops a torn or non-JSON line instead of aborting.
      Source: `executable_canary-freshness.sh:42`.
      Pin: `B7: a malformed canary timestamp is rejected, unhealthy, and cannot inject`
           at test/integration/osquery-heartbeat.bats:217

S196. The value is range-bound to `^(0|[1-9][0-9]{0,9})$`, which rejects a leading zero (bash would
      read it as octal and error) and caps at ten digits so an over-range epoch cannot overflow and
      wrap both freshness bounds to fresh. A rejected value returns empty, which every consumer treats
      as MISSING.
      Source: `executable_canary-freshness.sh:33-45`.
      Pin: `B7a: an over-range canary epoch is rejected (fail-safe MISSING), never a 64-bit-overflow
      false fresh` at test/integration/osquery-heartbeat.bats:237
      also `B7b: a leading-zero canary epoch is rejected (fail-safe MISSING), never an octal-parse
      fall-through` at test/integration/osquery-heartbeat.bats:252

S197. An unreadable snapshots log returns empty without an error.
      Source: `executable_canary-freshness.sh:41`.
      Pin: `B5: no canary at all reports unhealthy as MISSING, never a blind checkmark`
           at test/integration/osquery-heartbeat.bats:189

S198. The heartbeat verifies the ROOT DAEMON through its own scheduled canary, never a standalone
      `osqueryi` one-shot, because a one-shot answers while osqueryd is stopped or wedged.
      Source: `executable_heartbeat.sh:14-20`.
      Pin: `B6: the healthy message is honest about what it verified (R2-8)`
           at test/integration/osquery-heartbeat.bats:205

S199. A trustworthy clock is required FIRST: a failed or non-numeric `date` reports unhealthy with a
      time-unknown message, never `now=0`, which would make every historical canary look fresh.
      Source: `executable_heartbeat.sh:43-53`.
      Pin: `clock-failure: a non-numeric clock reports unhealthy, never false-healthy via now=0`
           at test/integration/osquery-heartbeat.bats:344

S200. HEALTHY requires a canary within `OSQUERY_CANARY_MAX_AGE` (default 1800, validated numeric) in
      EITHER direction, so a far-future timestamp cannot false-healthy the way a one-sided check
      would.
      Source: `executable_heartbeat.sh:33-38, :57-63`.
      Pin: `B1: a fresh canary sends exactly one CRIT message that reads healthy`
           at test/integration/osquery-heartbeat.bats:140

S201. A within-window future skew clamps the rendered age to 0 so the message reads "just now", never
      a negative age.
      Source: `executable_heartbeat.sh:64-67`.
      Pin: `clock-skew: a future-dated canary reads healthy with a non-negative rendered age`
           at test/integration/osquery-heartbeat.bats:295

S202. The healthy message states a recent OBSERVATION, not a present-tense claim, and says the
      watchdog owns real-time liveness.
      Source: `executable_heartbeat.sh:68-69`.
      Pin: `healthy-honesty: the healthy body is a recent observation, not a present-tense overclaim`
           at test/integration/osquery-heartbeat.bats:311

S203. Unhealthy has three honest sub-cases, each rendered with a POSITIVE number: MISSING (no canary),
      IMPLAUSIBLE (a future timestamp past the bound, rendered as the skew), and STALE (a real elapsed
      age). Only validated arithmetic is rendered, never a raw log field.
      Source: `executable_heartbeat.sh:81-96`.
      Pin: `B3: a stale canary reports unhealthy, the stopped-daemon case a one-shot would miss`
           at test/integration/osquery-heartbeat.bats:162
      also `B5: no canary at all reports unhealthy as MISSING, never a blind checkmark`
           at test/integration/osquery-heartbeat.bats:189
      also `implausible-future: a canary far in the future reports unhealthy IMPLAUSIBLE, not healthy`
           at test/integration/osquery-heartbeat.bats:325

S204. Every heartbeat message is sent with severity CRIT and an EMPTY sound, so it selects the
      `#priority` route while staying locally silent and marked `tier=muted` on the wire.
      Source: `executable_heartbeat.sh:70-80, :97-101`.
      Pin: `B2: the healthy message is silent (empty sound), a proof-of-life never pings`
           at test/integration/osquery-heartbeat.bats:152
      also `B4: the unhealthy message is also silent, the heartbeat never pings even degraded`
           at test/integration/osquery-heartbeat.bats:179

S205. The send is fire-and-forget (`|| true`) and the heartbeat advances no state, so a hard send
      failure never fails the run.
      Source: `executable_heartbeat.sh:51, :80, :101`.
      Pin: `fire-and-forget: a hard send failure never fails the heartbeat (exit 0)`
           at test/integration/osquery-heartbeat.bats:380

S206. The script runs `main` only when executed, not when sourced, so a test can exercise the canary
      seam in isolation.
      Source: `executable_heartbeat.sh:105-109`.
      Pin: `seam: newest_canary_timestamp returns the newest validated integer, else empty`
           at test/integration/osquery-heartbeat.bats:363

### 8.13 uptime-watchdog.sh

Nothing in `test/` runs this file. Every statement below is UNPINNED; the source citation is the only
authority, and `test/fixtures/osquery-watchdog-lib.bash` (628 lines, 41 functions) is the harness the
deleted suite used.

S207. The watchdog only READS: it probes the queue counts but never drains, hashes manifested files
      but never repairs, and renders only known labels, validated numerics and static text.
      Source: `executable_uptime-watchdog.sh:24-29`.
      Pin: UNPINNED.

S208. The cardinal invariant is fail-safe toward paging: any ambiguous or failed check resolves to a
      CRIT, never a silent all-healthy.
      Source: `executable_uptime-watchdog.sh:19-22`.
      Pin: UNPINNED.

S209. The cross-run state is validated as a WHOLE FILE, slurped to exactly ONE top-level object,
      because a concatenated stream fans a query out per document and a trailing `{}` would collapse
      a corrupt read back to one clean value that passes its guard.
      Source: `executable_uptime-watchdog.sh:116-133`.
      Pin: UNPINNED.

S210. An unwritable state directory is itself a paged problem, probed up front, because an
      unpersistable state silently disables both streak alarms.
      Source: `executable_uptime-watchdog.sh:86-91, :138-149`.
      Pin: UNPINNED.

S211. Probe 1 requires a trustworthy clock, then `pgrep -fq '/opt/osquery/.*osqueryd'`, then a canary
      inside the freshness window in either direction. Missing, stale and implausible each page with
      their own wording, and the default branch pages.
      Source: `executable_uptime-watchdog.sh:151-181`.
      Pin: UNPINNED.

S212. Probe 2 checks six named agents (every osquery LaunchAgent except the watchdog itself, which is
      loaded by definition when running). A failing `launchctl print` is "not loaded" and pages.
      Source: `executable_uptime-watchdog.sh:46-58, :195-199`.
      Pin: UNPINNED.

S213. A nonzero last exit is a crash-loop candidate ONLY when launchd's `runs` counter advanced, so a
      daily agent's frozen exit does not page every tick; two failing re-runs (streak >= 2) is the
      loop and one is a tolerated transient.
      Source: `executable_uptime-watchdog.sh:201-232`.
      Pin: UNPINNED.

S214. The `(never exited)` sentinel is matched ANCHORED, so a malformed value that merely contains it
      is an unknown state that pages rather than a healthy free pass; an absent or unparseable
      last-exit field pages too.
      Source: `executable_uptime-watchdog.sh:192-193, :219-237`.
      Pin: UNPINNED.

S215. Probe 3 GETs the `#priority` route with no signing header, so the HMAC key never reaches that
      wire; 2xx or 405 is healthy and 000, 404 or 5xx pages.
      Source: `executable_uptime-watchdog.sh:33-36, :242-250`.
      Pin: UNPINNED.

S216. Probe 4 pages unconditionally on ANY dead-letter, pages on a sustained pending backlog (grown
      across two consecutive checks), and pages on an unreadable count for either queue.
      Source: `executable_uptime-watchdog.sh:252-286`.
      Pin: UNPINNED.

S217. Probe 5 runs the manifest audit, guarded by `declare -F`, and treats an absent seam as the
      `unavailable` refusal rather than aborting the tick.
      Source: `executable_uptime-watchdog.sh:67-80, :304-310`.
      Pin: UNPINNED.

S218. A refusal token is validated against `^[a-z]{1,32}$` before it can select a message, and an
      unexpected token still pages through the default arm.
      Source: `executable_uptime-watchdog.sh:314-318, :427-454`.
      Pin: UNPINNED.

S219. The divergence COUNT counts report lines (one per diverging column, not per file), and a count
      that cannot be read falls back to the refusal path rather than reading as zero.
      Source: `executable_uptime-watchdog.sh:319-328`.
      Pin: UNPINNED.

S220. The KINDS in a report are named by walking a FIXED seven-word vocabulary and asking whether each
      appears as a line prefix, so what reaches a page body is one of seven literals written in the
      watchdog, never a token lifted out of the report, and the paths stay unrendered.
      Source: `executable_uptime-watchdog.sh:329-352, :419-426`.
      Pin: UNPINNED.

S221. The audit is page-once on a FINGERPRINT: a sha256 over the sorted report. The same fingerprint
      on two consecutive ticks is the confirmation threshold, a fingerprint already paged for does not
      page again, a changed fingerprint restarts the confirmation, and a clean audit forgets both.
      Source: `executable_uptime-watchdog.sh:354-417`.
      Pin: UNPINNED.

S222. An unhashable report pages EVERY tick rather than risk going silent, because without a
      fingerprint a repeat cannot be told from a new condition.
      Source: `executable_uptime-watchdog.sh:377-404`.
      Pin: UNPINNED.

S223. The streak is clamped at 99 on both read and write.
      Source: `executable_uptime-watchdog.sh:392-394, :407-408`.
      Pin: UNPINNED.

S224. A healthy tick persists the refreshed baselines and exits 0 silently; a persist failure there
      only forgets one cycle of streak memory.
      Source: `executable_uptime-watchdog.sh:467-472`.
      Pin: UNPINNED.

S225. An unhealthy tick sends ONE CRIT page with a sound, titled `🔴 **CRITICAL**` plus
      ` (N issues)` above one problem, listing one bullet per problem plus a fixed diagnose line and a
      restart line.
      Source: `executable_uptime-watchdog.sh:474-485`.
      Pin: UNPINNED.

S226. Notify-before-persist: the state advances only after `send_alert` durably queues the page; a
      hard store failure leaves the state as-is, prints to stderr and exits 1 so the next tick
      re-detects, and a persist failure after a successful send also exits 1.
      Source: `executable_uptime-watchdog.sh:487-499`.
      Pin: UNPINNED.

### 8.14 pipeline-audit.sh

Nothing in `test/` runs this file either. The orphan harness is
`test/fixtures/osquery-manifest-lib.bash` (175 lines, 17 functions).

S227. `pipeline_audit_scan` returns 0 having printed one `<kind> <path>` line per diverging COLUMN in
      manifest order, or returns 1 having printed a single refusal TOKEN. No output plus return 0 is
      the only all-clear.
      Source: `executable_pipeline-audit.sh:124-201`.
      Pin: UNPINNED.

S228. The seven divergence kinds are `content`, `mode`, `owner`, `missing`, `irregular`, `oversize`
      and `unreadable`; the six refusal tokens are `missing`, `unavailable`, `untrustworthy`,
      `malformed`, `overlong` and `budget`.
      Source: `executable_pipeline-audit.sh:128-146`.
      Pin: UNPINNED.

S229. A path that drifted on two columns is reported TWICE under two kinds, deliberately, so an
      escalation from mode drift to content tamper changes the watchdog's fingerprint.
      Source: `executable_pipeline-audit.sh:308-319`.
      Pin: UNPINNED.

S230. BOTH manifests are audited on the same tick and a refusal on either refuses the WHOLE scan,
      dropping the findings gathered so far, because a partial list beside a refusal token invites
      being read as complete.
      Source: `executable_pipeline-audit.sh:188-200`.
      Pin: UNPINNED.

S231. The reused seam is checked BY NAME (`declare -F _pipeline_manifest_is_trustworthy`) and reports
      `unavailable` rather than resolving paths against a missing helper.
      Source: `executable_pipeline-audit.sh:170-176`.
      Pin: UNPINNED.

S232. The verdict helper is sourced CONDITIONALLY, because an unconditional source of a deleted file
      would abort the watchdog mid-tick under errexit and page nothing at all.
      Source: `executable_pipeline-audit.sh:40-50`.
      Pin: UNPINNED.

S233. An absent, unreadable or EMPTY manifest is `missing`; a manifest that is not root-owned or is
      group/world-writable is `untrustworthy`. Both refuse before anything is judged.
      Source: `executable_pipeline-audit.sh:210-220`.
      Pin: UNPINNED.

S234. Each line is matched WHOLE against `^([0-9a-fA-F]{64}) ([0-7]{4}) ([0-9]{1,10}) (/.+)$`, never
      split into fields, so a path with spaces survives and a content-only manifest in the older
      format reports `malformed` rather than being read as if the missing columns did not matter.
      Source: `executable_pipeline-audit.sh:222-255`.
      Pin: UNPINNED.

S235. All four captures are taken in ONE go, because `BASH_REMATCH` is global and the next `[[ =~ ]]`
      anywhere in the loop overwrites it.
      Source: `executable_pipeline-audit.sh:256-265`.
      Pin: UNPINNED.

S236. Three bounds each fail toward a page: `MAX_ENTRIES` (500) refuses a longer manifest WHOLE,
      `MAX_BYTES` (8388608) skips hashing an oversized file under its own kind, and
      `BUDGET_SECONDS` (60) is ONE deadline SHARED across both manifests so adding a manifest cannot
      extend a tick.
      Source: `executable_pipeline-audit.sh:75-92, :179-198, :242-251, :292-293`.
      Pin: UNPINNED.

S237. Every bound is validated as a plain decimal and clamped on BOTH sides, so a hostile environment
      cannot turn a bound into unbounded work; a zero budget is legitimate and refuses immediately.
      Source: `executable_pipeline-audit.sh:94-109 _pipeline_audit_clamp`, `:181-184`.
      Pin: UNPINNED.

S238. A symlink at a manifested path is `irregular`, never followed and never hashed through to a
      referent's pristine bytes.
      Source: `executable_pipeline-audit.sh:266-276`.
      Pin: UNPINNED.

S239. Mode and owner are read from the VERDICT helper's readers, never a local stat, and are compared
      for EVERY readable regular file INCLUDING one too large to hash, so growing a file cannot buy
      silence on its permissions.
      Source: `executable_pipeline-audit.sh:277-320`.
      Pin: UNPINNED.

S240. The hash is cut with parameter expansion rather than a piped `awk`, because at one fork per
      manifested file that is most of the tick's cost, and an implausible digest reports `unreadable`.
      Source: `executable_pipeline-audit.sh:294-306`.
      Pin: UNPINNED.

### 8.15 firewall-gatekeeper-monitor.sh, the posture poller

Nothing in `test/` runs this file. The orphan harness is `test/fixtures/osquery-poller-lib.bash`
(664 lines, 26 functions). Every statement below is UNPINNED.

S241. One combined `osqueryi --json` query per tick reads firewall `global_state`, Gatekeeper
      `assessments_enabled` and `screenlock enabled` together, so a tick costs one osqueryi startup.
      Source: `executable_firewall-gatekeeper-monitor.sh:121-135`.
      Pin: UNPINNED.

S242. Screen-lock detection lives HERE and not in the root pack, because the `screenlock` table is
      scoped to the logged-in user and the root daemon never returns a row.
      Source: `executable_firewall-gatekeeper-monitor.sh:12-16`.
      Pin: UNPINNED.

S243. The combined query's exit status is captured, never erased: a nonzero exit empties the read
      regardless of what it printed, so the whole trio routes to the gap gate.
      Source: `executable_firewall-gatekeeper-monitor.sh:118-131`.
      Pin: UNPINNED.

S244. Every probe runs under a per-probe deadline (`gtimeout`, then `timeout`, then unbounded when
      neither exists), default `OSQUERY_POSTURE_TIMEOUT` 20 s, so a WEDGED tool becomes a monitoring
      gap rather than silent blindness.
      Source: `executable_firewall-gatekeeper-monitor.sh:90-112 run_bounded`.
      Pin: UNPINNED.

S245. Every probe binary is named by ABSOLUTE PATH by default (`fdesetup`, `csrutil`, `sysadminctl`,
      `defaults`, `pgrep`, `plutil`, `readlink`), because the LaunchAgent PATH is minimal.
      Source: `executable_firewall-gatekeeper-monitor.sh:33-40`.
      Pin: UNPINNED.

S246. The indeterminate-on-nonzero discipline: a probe that exits NONZERO is INDETERMINATE whatever it
      printed, and a zero-exit probe whose needles hit conflicting values is indeterminate too.
      Source: `executable_firewall-gatekeeper-monitor.sh:175-210 classify_probe`.
      Pin: UNPINNED.

S247. `classify_pgrep` pairs status and output symmetrically: exit 0 with a well-formed pid list is
      `running`, exit 1 with NO output is `stopped`, and every other combination, including a
      status/output mismatch in either direction, is indeterminate.
      Source: `executable_firewall-gatekeeper-monitor.sh:212-230`.
      Pin: UNPINNED.

S248. `fdesetup_status` enumerates five exact message forms; "Off, but will be enabled after the next
      restart" maps to OFF because deferred enablement is a real exposure, while "On, but needs to be
      restarted to finish" maps to ON.
      Source: `executable_firewall-gatekeeper-monitor.sh:319-334`.
      Pin: UNPINNED.

S249. `defaults_autologin` asks the DECLARATION, not the effective state: exit 0 means the
      `autoLoginUser` key exists and is `on` whatever the value, only the canonical does-not-exist
      diagnostic is `off`, and any other nonzero is indeterminate.
      Source: `executable_firewall-gatekeeper-monitor.sh:341-358`.
      Pin: UNPINNED.

S250. The LuLu active-profile guard runs ONCE per tick, in the PARENT shell before any read, because
      `read_control` runs in a command substitution whose state dies with the subshell.
      Source: `executable_firewall-gatekeeper-monitor.sh:244-260, :570-586`.
      Pin: UNPINNED.

S251. The guard is fail-closed on key PRESENCE: only the ABSENCE of `currentProfile` in the base
      preferences is trusted, and an active or unconfirmable profile makes every `lulu_rule` read
      indeterminate, with its cause spelled out beside the ids it blinded.
      Source: `executable_firewall-gatekeeper-monitor.sh:232-260, :699-707`.
      Pin: UNPINNED.

S252. `lulu_rule_in_archive` is EXISTENCE-ONLY: it converts the rules archive to stdout with
      `plutil -convert xml1 -o -` (never writing the file), matches the exact XML-escaped `<string>`
      element, and a zero-exit conversion that printed nothing stays indeterminate rather than absent.
      Source: `executable_firewall-gatekeeper-monitor.sh:262-291`.
      Pin: UNPINNED.

S253. `lulu_rule_resolved_present` resolves the declared launcher with `readlink -f` FIRST and
      searches for the RESOLVED binary, because LuLu keys rules on the executing Mach-O; an
      unresolvable launcher is indeterminate, never absent.
      Source: `executable_firewall-gatekeeper-monitor.sh:408-428`.
      Pin: UNPINNED.

S254. `load_controls` re-validates the DEPLOYED controls file before any probe runs and fails closed
      on: a missing file, a file that is not exactly ONE top-level JSON array, a non-integer count, a
      ZERO-record array, a malformed or colliding id, a tier other than `verify`, an unknown reader,
      an out-of-domain expect, a target on a targetless reader or missing on a targeted one, a
      non-absolute or multi-line target, and a missing description.
      Source: `executable_firewall-gatekeeper-monitor.sh:451-552`.
      Pin: UNPINNED.

S255. A refused controls file is refused WHOLE: every array loaded before the offending record is
      cleared, so a half-validated file is never half-monitored.
      Source: `executable_firewall-gatekeeper-monitor.sh:554-564`.
      Pin: UNPINNED.

S256. Ids become baseline field names, so they may appear once and must not shadow the built-in trio.
      Source: `executable_firewall-gatekeeper-monitor.sh:490-501`.
      Pin: UNPINNED.

S257. The monitoring gap is PER MEMBER: the marker stores the space-separated set of gapped members
      already paged for, so an ongoing gap stays quiet while a NEW member gapping during it still
      pages, and a recovered member that re-gaps pages again.
      Source: `executable_firewall-gatekeeper-monitor.sh:588-625 page_gap_once`.
      Pin: UNPINNED.

S258. A gap on one member never blinds the others: every member that read cleanly is still compared,
      paged and persisted, with only the gapped members' baseline fields preserved from the prior.
      Source: `executable_firewall-gatekeeper-monitor.sh:659-718, :769-810`.
      Pin: UNPINNED.

S259. The gap members are the built-in trio as a UNIT (`posture_query`), the controls file
      (`controls_file`), and each indeterminate control by its own id.
      Source: `executable_firewall-gatekeeper-monitor.sh:677-698`.
      Pin: UNPINNED.

S260. A prior baseline is trusted only when it is mode 600, slurps to exactly ONE top-level object,
      and carries three in-domain built-in scalars (firewall 0/1/2, Gatekeeper 0/1, screenlock 0/1).
      Source: `executable_firewall-gatekeeper-monitor.sh:627-657`.
      Pin: UNPINNED.

S261. Each control's prior is trusted INDEPENDENTLY and only when it sits in its reader's domain AND
      was recorded under the SAME declared `expect` and the SAME `target`, which is what makes adding
      or repointing a control a quiet upgrade rather than a page storm or a silent no-op.
      Source: `executable_firewall-gatekeeper-monitor.sh:729-767`.
      Pin: UNPINNED.

S262. With the trio unreadable AND no trusted prior there is nothing to anchor to, so the tick stops
      after the gap page rather than writing a baseline the next tick would distrust.
      Source: `executable_firewall-gatekeeper-monitor.sh:720-727`.
      Pin: UNPINNED.

S263. With no trusted prior, each already-off protection and each already-deviant control pages as a
      FIRST-OBSERVATION exposure; a fully healthy first observation seeds the baseline silently.
      Source: `executable_firewall-gatekeeper-monitor.sh:889-923`.
      Pin: UNPINNED.

S264. With a trusted prior, a page fires only on a protection turning OFF or a control LEAVING its
      declared value. Steady-deviant is silent (the regression already paged and the baseline is the
      page-once marker), and a return to the declared value is silent recovery.
      Source: `executable_firewall-gatekeeper-monitor.sh:949-994`.
      Pin: UNPINNED.

S265. A baseline persist FAILURE pages a degraded-monitor gap once through its own marker and exits
      1, because a stale baseline could mask the next real change permanently.
      Source: `executable_firewall-gatekeeper-monitor.sh:824-841 persist_baseline`.
      Pin: UNPINNED.

S266. Every value read from the system crosses into a notification body ONLY through `sanitize_span`:
      newlines, carriage returns and tabs flattened, backslash, backtick, dollar and both quote
      characters removed, capped at 160 characters, then wrapped in an inline-code span.
      Source: `executable_firewall-gatekeeper-monitor.sh:60-88`.
      Pin: UNPINNED.

S267. One page per tick even when several protections deviated together, titled `🔴 **CRITICAL**`
      plus ` · N` above one block, and the baseline advances only after the send succeeds.
      Source: `executable_firewall-gatekeeper-monitor.sh:843-863 page_crit_and_persist`, `:996-998`.
      Pin: UNPINNED.

### 8.16 tailscale-monitor.sh

Nothing in `test/` runs this file. The orphan harness is `test/fixtures/osquery-tailscale-lib.bash`
(386 lines, 29 functions). Every statement below is UNPINNED.

S268. A PUBLIC funnel is active exactly when an `AllowFunnel` entry is boolean true at ANY depth, read
      from `--json` rather than the human text, so a tailnet-only `serve` is correctly inactive.
      Source: `executable_tailscale-monitor.sh:11-19, :241-249`.
      Pin: UNPINNED.

S269. An `AllowFunnel` that is not a map, or an entry value that is not a boolean, is an UNEXPECTED
      shape and resolves to a gap, never a silent inactive.
      Source: `executable_tailscale-monitor.sh:241-259`.
      Pin: UNPINNED.

S270. A missing binary, a nonzero status exit, empty output, or non-JSON output each page a monitoring
      gap once through the `.gap` marker and exit, because a blind public-exposure monitor is itself
      CRIT.
      Source: `executable_tailscale-monitor.sh:21-24, :196-233`.
      Pin: UNPINNED.

S271. The status command runs under a bound (`gtimeout`, then `timeout`, default
      `OSQUERY_TAILSCALE_TIMEOUT` 10 s), so a wedged tailscaled becomes a gap rather than skipped
      ticks.
      Source: `executable_tailscale-monitor.sh:200-215`.
      Pin: UNPINNED.

S272. The prior baseline is read as a WHOLE FILE, slurped to exactly ONE object, and only
      `active` or `inactive` is a valid value; anything else is no trustworthy baseline.
      Source: `executable_tailscale-monitor.sh:152-179`.
      Pin: UNPINNED.

S273. A PRESENT state file with an invalid value is CORRUPT, distinct from an ABSENT one, and fires
      ONE CRIT corruption gap on a non-paging tick before the state is repaired.
      Source: `executable_tailscale-monitor.sh:168-179, :274-283`.
      Pin: UNPINNED.

S274. A present persist-gap marker distrusts the on-disk baseline entirely, because a failed
      close-write can leave an `active` baseline that would mask a later reopen.
      Source: `executable_tailscale-monitor.sh:181-191`.
      Pin: UNPINNED.

S275. A page fires on a fresh off-to-on transition, on a first-observation active funnel, and on an
      active read recovering from a blind window; a steady active funnel and a closing funnel are
      both silent.
      Source: `executable_tailscale-monitor.sh:264-272`.
      Pin: UNPINNED.

S276. The exposed `SNI:port` values are attacker-influenceable and cross into the body through the
      same chokepoint the page renderer uses: backticks stripped, `\r\n\t` squashed, capped at 200
      characters, wrapped in an inline-code span, deduplicated.
      Source: `executable_tailscale-monitor.sh:113-135 render_exposure`.
      Pin: UNPINNED.

S277. A valid read clears the gap marker so a future gap pages again.
      Source: `executable_tailscale-monitor.sh:261-262`.
      Pin: UNPINNED.

S278. Notify-before-persist on both paths: the baseline advances only after the send succeeds, and a
      persist failure pages a degraded-monitor gap once and exits 1.
      Source: `executable_tailscale-monitor.sh:96-111, :137-150`.
      Pin: UNPINNED.

S279. Every body string is apostrophe-free and a literal backtick is built through a `bt` variable,
      because `shfmt -s` would single-quote a no-expansion string and shellcheck would then flag the
      bare backticks.
      Source: `executable_tailscale-monitor.sh:45-53`.
      Pin: UNPINNED.

### 8.17 digest.sh, the daily digest builder

S280. An absent, zero-byte or whitespace-only spool produces no message and exits 0.
      Source: `executable_digest.sh:164-188 main`.
      Pin: `an absent digest store produces no message and exits 0`
           at test/integration/osquery-digest-builder.bats:308
      also `a zero-byte digest store produces no message and exits 0`
           at test/integration/osquery-digest-builder.bats:313
      also `a whitespace-only digest store produces no message and exits 0`
           at test/integration/osquery-digest-builder.bats:318

S281. A run with records sends exactly ONE silent message and then rotates the claimed batch to
      `<store>.last`.
      Source: `executable_digest.sh:226-230`, `:62-69 rotate_to_last`.
      Pin: `a store with records sends exactly one silent digest, then rotates the batch to .last`
           at test/integration/osquery-digest-builder.bats:323

S282. The batch is claimed ATOMICALLY by renaming the live store to
      `<store>.<epoch>.<pid>.build`, so findings appended during the build land in a fresh store; the
      pid is what stops two same-second runs colliding.
      Source: `executable_digest.sh:39-44 rotated_work_file`, `:168-173`.
      Pin: `the work-file name includes the pid, so same-second claims from different runs do not
      collide` at test/integration/osquery-digest-builder.bats:608

S283. An ERR trap restores the batch for any build failure BEFORE the send, and the trap is cleared
      once the last build step is done so the send outcome is handled explicitly.
      Source: `executable_digest.sh:175-210`.
      Pin: `a build failure before the send restores the rotated batch to the live store`
           at test/integration/osquery-digest-builder.bats:343

S284. Restore APPENDS rather than overwrites, so a finding the alerter appended during the build is
      preserved, and the work file is removed only when the append succeeded.
      Source: `executable_digest.sh:46-55 restore_batch`.
      Pin: `restore preserves a finding appended to the fresh store during the build`
           at test/integration/osquery-digest-builder.bats:585

S285. `send_alert` returning nonzero (its write-ahead persist failed) restores the batch; any STORED
      outcome rotates it to `.last`, because durability is then delegated to the store and the drain.
      Source: `executable_digest.sh:226-230`.
      Pin: `a hard send failure (persist failed) restores the batch to the live store for retry`
           at test/integration/osquery-digest-builder.bats:549
      also `a stored send (delivery pending, rc 0) rotates the batch to .last, not restored`
           at test/integration/osquery-digest-builder.bats:569

S286. Findings group by detector; each group renders a header carrying its TRUE count, then up to
      `DIGEST_MAX_BULLETS_PER_GROUP` (10) bullets, then a `+K more` roll-up.
      Source: `executable_digest.sh:123-127`.
      Pin: `findings across three detectors render as three grouped blocks with header and count`
           at test/integration/osquery-digest-builder.bats:358
      also `a detector with more findings than the bullet cap shows N bullets and a +K more roll-up`
           at test/integration/osquery-digest-builder.bats:372

S287. At most `DIGEST_MAX_GROUPS` (12) groups render, with an `and K more detector group(s)` marker,
      so a busy day cannot lose whole trailing groups to a silent mid-line cut.
      Source: `executable_digest.sh:140-143`.
      Pin: `more detector groups than the group cap show M blocks and an and-K-more marker`
           at test/integration/osquery-digest-builder.bats:384

S288. The body is codepoint-capped at `DIGEST_MAX_BODY_CHARS` (1800) INSIDE jq with a truncation
      marker, so there is no `head -c` pipe to cut a multibyte character or take SIGPIPE.
      Source: `executable_digest.sh:144-146`.
      Pin: `the body is codepoint-capped with an honest truncation marker`
           at test/integration/osquery-digest-builder.bats:395
      also `an oversized body is capped inside jq and sent once, with no broken-pipe failure`
           at test/integration/osquery-digest-builder.bats:422

S289. Each field is truncated at `DIGEST_MAX_FIELD_CHARS` (240) in the sanitize chokepoint, so one
      giant value cannot fill the body cap and crowd out every other detector.
      Source: `executable_digest.sh:113-115`.
      Pin: `an oversized field is truncated in the sanitize chokepoint and cannot crowd out other
      groups` at test/integration/osquery-digest-builder.bats:481

S290. All four caps are env-overridable named knobs, and a non-numeric value falls back to its default
      rather than reaching `--argjson` as invalid JSON and killing the daily digest.
      Source: `executable_digest.sh:32-37 numeric_or`, `:96-106`.
      Pin: `the group and bullet caps are env-overridable named constants`
           at test/integration/osquery-digest-builder.bats:443
      also `a non-numeric cap env value falls back to the default instead of failing the render`
           at test/integration/osquery-digest-builder.bats:639

S291. Every attacker-influenceable field is sanitized (backticks stripped, `\r\n\t` squashed, capped)
      and then WRAPPED in an inline-code span, so a crafted newline cannot forge a markdown line and a
      mention or link renders as inert text.
      Source: `executable_digest.sh:107-125`.
      Pin: `a crafted identity cannot inject an extra markdown line into the digest body`
           at test/integration/osquery-digest-builder.bats:459
      also `an attacker-controlled field renders inside a code span, so a mention or link is inert`
           at test/integration/osquery-digest-builder.bats:470

S292. Lines are parsed PER LINE with `try fromjson catch empty`, so one torn line is skipped instead
      of failing the whole run, and a valid-JSON wrong-shape line is coerced with `// "?" | tostring`
      rather than aborting on a non-string `gsub`.
      Source: `executable_digest.sh:128-133`.
      Pin: `a torn or malformed spool line is skipped, so the day's digest still builds`
           at test/integration/osquery-digest-builder.bats:500
      also `a valid-JSON wrong-shape line is coerced, not fatal, and the digest still sends`
           at test/integration/osquery-digest-builder.bats:530

S293. An all-torn batch renders an empty body, which is preserved to `.last` and sent as NOTHING,
      rather than a misleading silent "N item(s)" with no content.
      Source: `executable_digest.sh:194-201`.
      Pin: `an all-torn store renders an empty body, so it sends nothing and preserves the batch to
      .last` at test/integration/osquery-digest-builder.bats:653

S294. The item count comes from a torn-safe `grep -c .`, never a JSON parse.
      Source: `executable_digest.sh:203-206`.
      Pin: `a store with records sends exactly one silent digest, then rotates the batch to .last`
           at test/integration/osquery-digest-builder.bats:323

S295. Orphaned `.build` files from a killed run are swept back into the live store BEFORE the empty
      gate, because a signal does not fire the ERR trap and no later run would consult them.
      Source: `executable_digest.sh:153-162`.
      Pin: `an orphaned .build from a killed run is swept back into the next digest`
           at test/integration/osquery-digest-builder.bats:621

S296. The message is sent CRIT with an EMPTY sound, so it selects the `#priority` route while staying
      locally silent and `tier=muted` on the wire, and `.last` is chmod 600 because it holds full
      filesystem paths.
      Source: `executable_digest.sh:212-230`, `:62-69`.
      Pin: `a store with records sends exactly one silent digest, then rotates the batch to .last`
           at test/integration/osquery-digest-builder.bats:323

S297. Cadence is owned entirely by the LaunchAgent: there is no internal time gate, so a manual run
      behaves exactly like the scheduled one, and `main` runs only when executed, not when sourced.
      Source: `executable_digest.sh:12-15, :233-238`.
      Pin: `a build failure before the send restores the rotated batch to the live store`
           at test/integration/osquery-digest-builder.bats:343 (the suite sources the file to force a
      build failure, which only works because of the guard)

### 8.18 allowlist.sh, the one writer

Nothing in `test/` runs this file. The orphan harness is `test/fixtures/osquery-allowlist-lib.bash`
(198 lines, 14 functions). Every statement below is UNPINNED.

S298. The interface is exactly `-a <label>`, `-d <label>` and `-l`; anything else prints usage to
      stderr and exits 2.
      Source: `executable_allowlist.sh:36-39 usage`, `:325-359`.
      Pin: UNPINNED.

S299. Both mutating verbs take a BLOCKING kernel lock (`lockf -s`, no `-t 0`) around the whole
      read, capture, rewrite and publish critical section, because curation must serialize rather
      than skip; a lockf-less host proceeds unlocked and any other setup failure fails closed.
      Source: `executable_allowlist.sh:41-68 take_allowlist_write_lock`, `:344-352`.
      Pin: UNPINNED.

S300. Every external command spawned under that lock closes fd 9, and `9>&-` is added ONLY to forked
      externals, never to a builtin or function call, which would release the lock early.
      Source: `executable_allowlist.sh:51-56`, and every call site.
      Pin: UNPINNED.

S301. A line counts as an entry only when it holds EXACTLY ONE JSON value, the same rule the consumer
      applies, so a line holding two concatenated tuples cannot survive a rewrite the consumer honors
      for nothing.
      Source: `executable_allowlist.sh:70-84 entry_label, line_is_one_tuple`.
      Pin: UNPINNED.

S302. A non-comment line that is not exactly one JSON tuple REFUSES the whole rewrite with a reason on
      stderr; comments and blanks are preserved verbatim.
      Source: `executable_allowlist.sh:86-113 _without_label`.
      Pin: UNPINNED.

S303. A label must match `^[A-Za-z0-9][A-Za-z0-9._@-]+$`, and `com.apple.*` is refused
      case-insensitively (including the dotless prefix) so a system-daemon page cannot be suppressed.
      Source: `executable_allowlist.sh:209-219 is_valid_label`.
      Pin: UNPINNED.

S304. `-a` captures the identity from the SAME `launchd` table a finding comes from, and fails CLOSED
      when the label has no loaded agent or when the plist sha256 capture does not yield 64 lowercase
      hex characters: an unpinned tuple is never writer-produced.
      Source: `executable_allowlist.sh:221-252 allow_label`.
      Pin: UNPINNED.

S305. A leading `$HOME` is relativized to `~/` so the committed file stays user-agnostic.
      Source: `executable_allowlist.sh:253-255`.
      Pin: UNPINNED.

S306. `-a` refreshes in place: it drops any existing tuple for the label and appends the freshly
      captured one, so re-adding updates the identity and never duplicates it, and an unchanged
      identity reproduces the source byte for byte (a true no-op).
      Source: `executable_allowlist.sh:256-277`.
      Pin: UNPINNED.

S307. Curation edits the chezmoi SOURCE, then applies THAT ONE TARGET, then invokes the manifest
      runner directly, because a targeted apply does not fire `run_after` scripts and an out-of-band
      write would be erased by the next apply and would suppress nothing in the meantime.
      Source: `executable_allowlist.sh:115-207 publish_allowlist`.
      Pin: UNPINNED.

S308. A failed apply is ROLLED BACK to the previous source bytes with nothing deployed; a failed
      manifest refresh is NOT recoverable that way and is reported loudly with a nonzero exit, because
      until the manifest is refreshed every user LaunchAgent pages (the safe direction).
      Source: `executable_allowlist.sh:157-206`.
      Pin: UNPINNED.

S309. `-d` on a label that was never allowed is a clean no-op: exit 0, nothing deployed, no manifest
      refresh, a note on stdout, so a caller can deny unconditionally. The SOURCE is what is
      consulted.
      Source: `executable_allowlist.sh:280-308 deny_label`.
      Pin: UNPINNED.

S310. `-l` reads the DEPLOYED file, because that is the one the alerter consults, and prints entry
      lines verbatim while skipping comments and blanks.
      Source: `executable_allowlist.sh:310-321 list_entries`.
      Pin: UNPINNED.

### 8.19 drift-verdict.sh, the converge decision core

Every function here is a total function of its arguments: no stat, no cmp, no filesystem, no clock, no
privilege. It is the one part of the pipeline already split the way the pns refactor charter asks for.

S311. `osquery_converge_file_verdict <kind> <content-equal> <mode> <uid> <gid>` prints exactly one
      token: `ok`, `absent`, `irregular`, `unreadable`, `content`, `mode`, `owner` or `group`.
      Everything but `ok` means the caller reinstalls.
      Source: `osquery-converge/drift-verdict.sh:86-113`.
      Pin: `test_a_file_matching_on_content_mode_owner_and_group_has_not_drifted`
           at test/unit/osquery-converge-drift-verdict.test.sh:37

S312. Nothing at the path is `absent`.
      Source: `osquery-converge/drift-verdict.sh:88-92`.
      Pin: `test_nothing_at_the_path_reads_as_absent`
           at test/unit/osquery-converge-drift-verdict.test.sh:41

S313. A symlink, a directory, or any non-file kind at a file path is `irregular`, because the TYPE is
      the only column that can tell: `cmp` follows a link so the content compares equal, and BSD stat
      lstats so the mode and owner describe the link, which its author sets with `chmod -h`.
      Source: `osquery-converge/drift-verdict.sh:70-85, :93-98`.
      Pin: `test_a_symlink_standing_where_the_config_belongs_reads_as_irregular`
           at test/unit/osquery-converge-drift-verdict.test.sh:45
      also `test_a_directory_standing_where_the_config_belongs_reads_as_irregular`
           at test/unit/osquery-converge-drift-verdict.test.sh:51

S314. An empty content-equality answer, or any unreadable attribute, is `unreadable`, never a match: a
      stat that failed is not evidence of a healthy file and reinstalling is the only safe direction.
      Source: `osquery-converge/drift-verdict.sh:38-46, :99-107`.
      Pin: `test_a_state_that_could_not_be_read_reads_as_unreadable_never_as_ok`
           at test/unit/osquery-converge-drift-verdict.test.sh:74

S315. Differing bytes are `content`; the desired file attributes are mode 0644, uid 0 and gid 0, and
      the desired directory mode is 0755.
      Source: `osquery-converge/drift-verdict.sh:26-31, :108-112`.
      Pin: `test_differing_bytes_read_as_content_drift`
           at test/unit/osquery-converge-drift-verdict.test.sh:55
      also `test_correct_bytes_under_a_world_writable_mode_read_as_drift_not_as_ok`
           at test/unit/osquery-converge-drift-verdict.test.sh:59
      also `test_correct_bytes_owned_by_a_non_root_user_read_as_drift`
           at test/unit/osquery-converge-drift-verdict.test.sh:66
      also `test_correct_bytes_owned_by_a_non_wheel_group_read_as_drift`
           at test/unit/osquery-converge-drift-verdict.test.sh:70

S316. Precedence is fixed: content drift is reported AHEAD of an attribute that also drifted, because
      the token is what the operator reads in the repair line and content is the more serious.
      Source: `osquery-converge/drift-verdict.sh:83-112`.
      Pin: `test_content_drift_is_reported_ahead_of_an_attribute_that_also_drifted`
           at test/unit/osquery-converge-drift-verdict.test.sh:81

S317. `osquery_converge_directory_verdict` uses the same tokens minus `content`, and carries the role
      the old setup script's unconditional `install -d` had, which is what buys the quiet no-op.
      Source: `osquery-converge/drift-verdict.sh:115-138`.
      Pin: `test_a_directory_at_0755_root_wheel_has_not_drifted`
           at test/unit/osquery-converge-drift-verdict.test.sh:89
      also `test_a_group_writable_directory_reads_as_drift`
           at test/unit/osquery-converge-drift-verdict.test.sh:93
      also `test_a_missing_directory_reads_as_absent`
           at test/unit/osquery-converge-drift-verdict.test.sh:99
      also `test_a_file_standing_where_the_packs_directory_belongs_reads_as_irregular`
           at test/unit/osquery-converge-drift-verdict.test.sh:103

S318. `osquery_converge_restart_verdict` prints `restart` when ANY verdict is not `ok`, `no-restart`
      otherwise, and no arguments means nothing was examined, which justifies no bounce.
      Source: `osquery-converge/drift-verdict.sh:140-159`.
      Pin: `test_nothing_drifted_means_no_restart`
           at test/unit/osquery-converge-drift-verdict.test.sh:109
      also `test_any_single_drifted_path_warrants_a_restart`
           at test/unit/osquery-converge-drift-verdict.test.sh:113
      also `test_a_drifted_path_in_the_last_position_still_warrants_a_restart`
           at test/unit/osquery-converge-drift-verdict.test.sh:117
      also `test_an_empty_verdict_list_means_no_restart`
           at test/unit/osquery-converge-drift-verdict.test.sh:123

### 8.20 osquery-converge.sh

S319. No drift means no privileged call, no restart and NO OUTPUT.
      Source: `executable_osquery-converge.sh:14-18, :720-780 main`.
      Pin: `a converged tree is a silent no-op: nothing printed, nothing privileged, no restart`
           at test/unit/osquery-converge.bats:288

S320. Drift means one line per repaired path naming what was wrong, from a CLOSED label vocabulary, so
      a repair line never carries a token lifted out of something observed.
      Source: `executable_osquery-converge.sh:370-384 drift_label`, `:571-586`.
      Pin: `a repair says which file it repaired and why`
           at test/unit/osquery-converge.bats:486

S321. A wiped file, and a wiped pack, are reinstalled and the daemon restarted, so a partial wipe is
      fully repaired.
      Source: `executable_osquery-converge.sh:763-779`.
      Pin: `a wiped file is reinstalled and the daemon is restarted`
           at test/unit/osquery-converge.bats:296
      also `a wiped pack is reinstalled too, so a partial wipe is fully repaired`
           at test/unit/osquery-converge.bats:305

S322. Correct bytes under a wrong mode, owner or group are reinstalled, not passed over, because right
      bytes under a 0666 mode is the same escalation with an extra step.
      Source: `executable_osquery-converge.sh:59-66, :554-564`.
      Pin: `correct bytes under a world-writable mode are reinstalled, not passed over`
           at test/unit/osquery-converge.bats:312
      also `a path owned by a non-root user is reinstalled, files and directories alike`
           at test/unit/osquery-converge.bats:321
      also `a group-writable target directory is repaired to 0755 root:wheel`
           at test/unit/osquery-converge.bats:496

S323. The attribute reader asks BSD for `%p`, never `%Lp`, so a setuid bit reads as drift rather than
      as a matching 0644, and the value is shape-checked before it is sliced to four digits.
      Source: `executable_osquery-converge.sh:337-353 probe_attributes`.
      Pin: `a setuid bit on a live file reads as drift, not as a matching 0644`
           at test/unit/osquery-converge.bats:332

S324. A symlink standing at a config path is REPLACED by a regular file, because `install` replaces a
      destination symlink rather than writing through it (measured).
      Source: `executable_osquery-converge.sh:314-335 probe_kind`, `:582-586 repair_file`.
      Pin: `a symlink standing at the config path is replaced by a regular file`
           at test/unit/osquery-converge.bats:344

S325. A symlink standing where a DIRECTORY belongs is REFUSED, never repaired, because `install -d`
      follows a preplanted link (measured: it chmods the referent, exits 0 and leaves the link).
      Source: `executable_osquery-converge.sh:744-755`.
      Pin: `a symlink standing where the target directory belongs is refused, never repaired through`
           at test/unit/osquery-converge.bats:624
      also `a symlink standing where the packs directory belongs claims nothing and repairs nothing`
           at test/unit/osquery-converge.bats:641

S326. BOTH directory verdicts are taken before EITHER is acted on, so an irregular second directory is
      refused with no privileged call having been made on the first.
      Source: `executable_osquery-converge.sh:740-762`.
      Pin: `a symlink standing where the packs directory belongs claims nothing and repairs nothing`
           at test/unit/osquery-converge.bats:641

S327. One `install` call carries owner, group AND mode, never a tee-then-chmod pair, because a file
      existing between those two steps would carry the creating umask for that window.
      Source: `executable_osquery-converge.sh:579-586`.
      Pin: `the install carries owner, group and mode in ONE call`
           at test/unit/osquery-converge.bats:361

S328. Every privileged command is named by ABSOLUTE PATH (`/usr/bin/sudo`, `/usr/bin/install`), never
      a PATH lookup, because `sudo -n` preserves the caller's PATH and this host's PATH leads with an
      operator-writable directory.
      Source: `executable_osquery-converge.sh:68-84, :183-184`.
      Pin: `the privileged install names /usr/bin/install, never a PATH lookup`
           at test/unit/osquery-converge.bats:370

S329. The one RESOLVED privileged command, `osqueryctl`, is trusted only when its CONTAINING DIRECTORY
      is owned by uid 0 and is not group- or world-writable; a relative resolution is refused too.
      Source: `executable_osquery-converge.sh:246-312`.
      Pin: `an osqueryctl resolved into a directory root does not own is refused, never handed to
      sudo` at test/unit/osquery-converge.bats:382
      also `an osqueryctl resolved into a group-writable directory is refused too`
           at test/unit/osquery-converge.bats:393

S330. Four environment overrides are TEST-ONLY seams gated behind `OSQUERY_CONVERGE_TEST_SEAM=1`, and
      the gate tests PRESENCE so an override naming the default is refused too.
      Source: `executable_osquery-converge.sh:99-137`.
      Pin: `a seam variable set without the test seam is refused, so it is not a production knob`
           at test/unit/osquery-converge.bats:407

S331. The other half of the gate: with the seam engaged, the two seams whose DEFAULTS ARE PRODUCTION
      must be given explicitly, or a harness that set only some of them would converge the live
      machine out of a sandbox.
      Source: `executable_osquery-converge.sh:139-161`.
      Pin: `the test seam without a target directory is refused, never defaulted to /var/osquery`
           at test/unit/osquery-converge.bats:425
      also `the test seam without a sudo is refused too, so root is never the real one`
           at test/unit/osquery-converge.bats:437

S332. Every byte root writes is read out of a PRIVATE 0700 copy created per run, never out of the
      deployed staging tree, because `install` reads its source as root and the check and the read are
      far apart.
      Source: `executable_osquery-converge.sh:419-442, :444-467, :527-533`.
      Pin: `the privileged install reads from a private staging copy, not from the deployed tree`
           at test/unit/osquery-converge.bats:445
      also `the content comparison reads the private copy, not the deployed staging tree`
           at test/unit/osquery-converge.bats:462

S333. The desired-state file list is NAMED, never globbed, and a file in the staging tree the list
      does not name is a loud REFUSAL matched on its exact relative path, not a pattern or basename.
      Source: `executable_osquery-converge.sh:196-214, :497-512`.
      Pin: `a file in the desired tree the tool does not install is refused, not ignored`
           at test/unit/osquery-converge.bats:523
      also `a planted file is matched on its exact relative path, not on a pattern or a basename`
           at test/unit/osquery-converge.bats:548
      also `a symlink planted in the desired tree under an unlisted name is refused too`
           at test/unit/osquery-converge.bats:536

S334. A missing desired file is a loud failure rather than a silent skip, and a symlink anywhere in
      the desired tree is refused rather than installed through.
      Source: `executable_osquery-converge.sh:497-525`.
      Pin: `a desired file that is not deployed is a loud failure, never a silent skip`
           at test/unit/osquery-converge.bats:512
      also `a symlink in the desired tree is refused rather than installed through`
           at test/unit/osquery-converge.bats:600

S335. Any component of the path leading to the staging directory being a symlink is refused, walked
      component by component rather than compared against a canonical path, because `find` does not
      descend a symlinked ARGUMENT and `[[ -L ]]` resolves the directory component before the leaf.
      Source: `executable_osquery-converge.sh:386-417 assert_no_symlink_component`.
      Pin: `a staging directory that is itself a symlink is refused, not followed`
           at test/unit/osquery-converge.bats:665
      also `a staging directory reached through a symlinked PARENT is refused too`
           at test/unit/osquery-converge.bats:682

S336. The staging listing is MATERIALIZED under an explicit status check, never read through a process
      substitution, so a partial `find` is a refusal rather than a converge from a tree nothing
      examined.
      Source: `executable_osquery-converge.sh:477-488`.
      Pin: `a staging tree the tool cannot fully read is a refusal, not a silent pass`
           at test/unit/osquery-converge.bats:694

S337. A desired file that cannot be READ, and a LIVE file that cannot be COMPARED, are each never
      treated as converged: `cmp`'s error status 2 yields an empty answer, which the verdict reads as
      unreadable.
      Source: `executable_osquery-converge.sh:355-368 probe_content_equality`, `:527-533`.
      Pin: `a desired file that cannot be read is never treated as converged`
           at test/unit/osquery-converge.bats:565
      also `a LIVE file that cannot be compared is reinstalled, never counted as converged`
           at test/unit/osquery-converge.bats:578

S338. Only the drifted file is reinstalled, so a repair is not a rewrite of everything.
      Source: `executable_osquery-converge.sh:763-767`.
      Pin: `only the drifted file is reinstalled, so a repair is not a rewrite of everything`
           at test/unit/osquery-converge.bats:479

S339. A missing packs directory is created before the packs are installed into it.
      Source: `executable_osquery-converge.sh:722, :756-762`.
      Pin: `a missing packs directory is created before the packs are installed into it`
           at test/unit/osquery-converge.bats:503

S340. An absent osqueryctl is a QUIET no-op, because a machine that does not run osquery has no daemon
      to converge for and no vendor layout to converge into.
      Source: `executable_osquery-converge.sh:296-312, :725-729`.
      Pin: `osquery not being installed at all is a quiet no-op`
           at test/unit/osquery-converge.bats:608

S341. An unknown argument is an ERROR with usage and exit 2, never a silent fallthrough to a full
      privileged converge.
      Source: `executable_osquery-converge.sh:234-240`.
      Pin: `an unknown argument is a usage error, never a silent full converge`
           at test/unit/osquery-converge.bats:708

S342. The vendor plist is checked FIRST, before anything is stopped, as `! -L` AND `-f`; a symlink
      there is refused because `osqueryctl start` would copy its referent into `/Library/LaunchDaemons`
      and load it as root, and a missing one is refused because a stop would leave osqueryd GONE.
      Source: `executable_osquery-converge.sh:652-674`.
      Pin: `a missing vendor plist refuses the restart and never stops the daemon`
           at test/unit/osquery-converge.bats:771
      also `a symlink standing in for the vendor plist refuses the restart`
           at test/unit/osquery-converge.bats:782

S343. `osqueryctl config-check` runs BEFORE the stop, so a config the daemon cannot parse is a loud
      refusal with the previous daemon still up on its previous configuration.
      Source: `executable_osquery-converge.sh:676-683`.
      Pin: `a config the daemon cannot parse refuses the restart and never stops the daemon`
           at test/unit/osquery-converge.bats:797

S344. The stop is GUARDED (a fresh host legitimately has nothing to stop) and the start is UNGUARDED,
      because a stop-succeeds/start-fails pair used to leave the daemon gone while the script printed
      success.
      Source: `executable_osquery-converge.sh:685-702`.
      Pin: `a stop that fails with no daemon running does not stop the run, because a fresh host has
      nothing to stop` at test/unit/osquery-converge.bats:717
      also `a start that fails is FATAL even while a daemon is still running`
           at test/unit/osquery-converge.bats:749
      also `a start that fails after a successful stop leaves no success line`
           at test/unit/osquery-converge.bats:762

S345. A restart is claimed only on measured evidence: the ppid-1 parent pid must be DIFFERENT from the
      one running before the stop, because `osqueryctl stop` is a `launchctl unload` that logs a
      failure while exiting 0.
      Source: `executable_osquery-converge.sh:588-630`.
      Pin: `a daemon whose parent pid never changed is not a restart, however the stop exited`
           at test/unit/osquery-converge.bats:728
      also `a restart is claimed only when the parent pid actually changed`
           at test/unit/osquery-converge.bats:741
      also `the restart is judged on the ppid-1 parent, never on an arbitrary worker`
           at test/unit/osquery-converge.bats:832

S346. Present once is not alive: the new parent must stay the SAME pid for the whole settle window,
      polled across it rather than checked at its end, because the vendor plist's KeepAlive would
      replace a crashing daemon a minute later.
      Source: `executable_osquery-converge.sh:632-648, :704-716`.
      Pin: `a daemon that never comes back is a loud failure, not a reported success`
           at test/unit/osquery-converge.bats:804
      also `a daemon that dies inside the settle window is a loud failure`
           at test/unit/osquery-converge.bats:811
      also `a daemon that comes back under a NEW pid inside the settle window is a failure`
           at test/unit/osquery-converge.bats:821

S347. Both restart bounds are validated by shape and defaulted on anything surprising (deadline 30 s,
      settle 5 s), counted in 0.25 s ticks because bash has no wait-with-timeout and stock macOS ships
      no `timeout(1)`.
      Source: `executable_osquery-converge.sh:216-229`.
      Pin: UNPINNED. The suite drives the bounds through the seams rather than asserting the defaults.

S348. The daemon's log directory is created unprivileged when missing and is deliberately NOT folded
      into the restart decision, because bouncing the root daemon over a missing directory would be
      heavier than the condition warrants.
      Source: `executable_osquery-converge.sh:769-776`.
      Pin: `a missing log directory is created, because the daemon logs into it`
           at test/unit/osquery-converge.bats:842
      also `creating the log directory does not bounce the root daemon`
           at test/unit/osquery-converge.bats:849

S349. The private stage is removed by an EXIT trap using `rm -rf` and not `trash`, guarded on a
      non-empty path that is really a directory.
      Source: `executable_osquery-converge.sh:432-442, :782`.
      Pin: UNPINNED.

### 8.21 The manifest runner and the detection configuration

S350. `run_after_05` is a PLAIN script, not a template, so it runs on every apply flavor, and darwin is
      gated at runtime.
      Source: `.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh:52-56, :123`.
      Pin: UNPINNED.

S351. It runs in the earliest after-phase slot because the WatchPaths alerter judges a finding exactly
      once, so the manifests must be current before the alerter looks at the change the apply caused.
      Source: `run_after_05:57-60`.
      Pin: UNPINNED.

S352. ONE managed listing and ONE dump serve both arms, each materialized to a file under an EXPLICIT
      status check, because a process substitution discards the producer's status and a partial set
      would root-install a manifest missing tuples over a complete one.
      Source: `run_after_05:141-167, :259-284`.
      Pin: UNPINNED.

S353. The nested `chezmoi dump` MUST run against a throwaway `--persistent-state`, because this script
      runs inside an apply that already holds that lock, and the throwaway state is SEEDED with the
      config template's hash so the dump does not print a config-changed warning on every apply.
      Source: `run_after_05:226-258`.
      Pin: UNPINNED.

S354. An empty path list for EITHER arm aborts BEFORE the dump, because `chezmoi dump` with no
      arguments dumps the entire target state and would render every keepassxc template from an
      unattended apply.
      Source: `run_after_05:210-222`.
      Pin: UNPINNED.

S355. The pipeline arm runs FIRST and installs before the bin arm starts, so a failure in the bin arm
      cannot leave the pipeline manifest stale.
      Source: `run_after_05:48-50, :384-385`.
      Pin: UNPINNED.

S356. Each hash is captured into a VARIABLE and its status checked EXPLICITLY, because under pipefail
      a failed `chezmoi cat` still lets `shasum` print the hash of an EMPTY stream, which is a
      well-formed 64-hex string no emptiness check would catch.
      Source: `run_after_05:322-341`.
      Pin: UNPINNED.

S357. The mode is chezmoi's decimal `perm`, validated as digits and range-bound to the twelve
      permission bits BEFORE the octal conversion, with `10#` forcing base ten on both uses.
      Source: `run_after_05:342-353`.
      Pin: UNPINNED.

S358. An EMPTY render never overwrites a good manifest, and only a real content change warrants the
      privileged write (`cmp -s` first, since the deployed manifest is world-readable).
      Source: `run_after_05:356-381`.
      Pin: UNPINNED.

S359. The path array arrives by NAME through a nameref and every local is prefixed with the function
      name, so a caller's array can never collide and make the nameref alias a local.
      Source: `run_after_05:310-320`.
      Pin: UNPINNED. The test that pinned that failure mode
      (`test/test-system/nameref-guards.sh`, cited at `:313`) no longer exists.

S360. The scheduled detection set is 25 query names across four packs plus two top-level queries, and
      the normalizer's allowlist is the second, independent copy of that list.
      Source: `osquery-converge/desired/osquery.conf.tmpl:8-38`, the four `packs/*.conf`,
      `results-alerter/normalize.sh:53-60`.
      Pin: `an unrecognized query name never becomes a finding, whether it arrives packed or
      top-level` at test/unit/osquery-normalize-and-digest-store.bats:79 (pins the allowlist, not its
      agreement with the config)

S361. `heartbeat_canary` is the only snapshot query, at 600 s, and it lands in
      `osqueryd.snapshots.log` which the alerter never reads.
      Source: `osquery-converge/desired/osquery.conf.tmpl:26-31`.
      Pin: `B1: a fresh canary sends exactly one CRIT message that reads healthy`
           at test/integration/osquery-heartbeat.bats:140

S362. Eight `file_paths` categories are watched (`ssh`, `allowlist_file`, `pipeline_integrity`,
      `managed_bin`, `launch_agents`, `launch_daemons`, `sudoers`, `sshd_config`) and four of them are
      additionally content-hashed; `managed_bin` and `allowlist_file` deliberately are NOT, so their
      events carry no digest and take the atomic-rename path.
      Source: `osquery-converge/desired/osquery.conf.tmpl:39-84`.
      Pin: UNPINNED. The three-way agreement test between this set, the manifest filter and
      `_pipeline_is_tracked` was deleted under the 2026-08-05 ruling.

S363. `filevault_off` is DIFFERENTIAL, not snapshot, because a snapshot result lands in a log the
      alerter does not read and therefore never paged.
      Source: `packs/security-policy-regression.conf:33-38`.
      Pin: `C2: differential filevault_off added (not snapshot) fires a CRIT page`
           at test/e2e/osquery-alerter-criteria.bats:128

S364. `agent_exposure_changed` is PATTERN-based, not a fixed port list: any process whose cmdline
      matches `mcp`, or a listener on 5432, 6767 or 8644, or a path matching `hermes`, bound off
      loopback. Only real loopback and the port-0 placeholder are excluded; IPv6 link-local stays
      included because it is reachable on the same link.
      Source: `packs/agent-attack-surface.conf.tmpl:3-8`.
      Pin: `C3a: agent_exposure_changed added pages`
           at test/e2e/osquery-alerter-criteria.bats:135

S365. `agent_secretfile_changed` watches the two TRUE secrets by file METADATA (size, mtime, ctime,
      inode), never a content hash, because `results.log` is group-readable and a secret's digest must
      never be written there. `agent_authfile_changed` watches the three non-secret configs by hash.
      Source: `packs/agent-attack-surface.conf.tmpl:9-19`.
      Pin: `C7: a paged agent_secretfile_changed body shows the basename only, never the path or
      sha256` at test/e2e/osquery-alerter-criteria.bats:211

S366. `agent_binary_changed` is LOG-ONLY and makes no promise of content-change detection: codex
      exceeds osquery's `read_max` so its sha256 is always empty, and paseo is unsigned so its only
      signal is the coarse size, inode and mtime tuple.
      Source: `packs/agent-attack-surface.conf.tmpl:21-26`, `results-alerter/route.sh:179`.
      Pin: `the extension arms honor the untrusted-signing promotion and the log-only arms ignore it`
           at test/unit/osquery-route.bats:412

S367. `osquery.flags` carries the CLI-only flags: `--disable_extensions=true` removes the
      extension-autoload surface, the endpoint-security pair is enabled with four muted path prefixes,
      and the `logger_rotate` trio caps the filesystem logger at five files of 10 MB, which is what
      bounds the alerter's full-replay path.
      Source: `osquery-converge/desired/osquery.flags:1-12`,
      `executable_results-alerter.sh:129-134`.
      Pin: UNPINNED.

S368. `run_after_50` is a plain script that exec's the deployed converge tool, and a tool that is not
      deployed is a LOUD stderr line with exit 0 rather than an aborted apply.
      Source: `.chezmoiscripts/run_after_50-setup-osquery.sh:42-58`.
      Pin: UNPINNED.


## 9. Counts

Computed over this document on 2026-09-05 with `grep -c '^S[0-9][0-9][0-9]\. '` and
`grep -c '^      Pin: UNPINNED'`.

| Count                                             | Value |
| ------------------------------------------------- | ----- |
| Statements                                        | 368   |
| Pinned                                            | 169   |
| UNPINNED                                          | 199   |
| Distinct tests referenced                         | 184   |
| Test cases in the corpus that cover this pipeline | 186   |
| Statements the port drops (section 6)             | 46    |

Only two of the 186 cases go uncited, both in `test/e2e/osquery-alerter-criteria.bats`: the pair that
asserts the retired flat `launch-allowlist.txt` is not consulted and that the unified
`OSQUERY_LAUNCHD_ALLOWLIST` variable is what the entry reads (`:178`, `:189`).

The 368 statements are an inventory of what the bash does, not proof that a port matches it. A
statement is checkable only where a test names it, and 199 are not; for those, a Rust test written
from the statement's prose checks the porter's reading of the bash, not the bash. The plan's section
4, rule 1, therefore requires a bash-derived acceptance example (the exact input and the output the
running script produced) for every UNPINNED statement a pull request moves, captured before the Rust
test that interprets it is written.

The pinned share concentrates in six tools (the alerter entry and its four stages, the digest
builder, the heartbeat, the converge tool and its verdict core, and the two dispatch counters). Five
tools carry zero coverage (the watchdog, the manifest audit, the poller, the funnel monitor and the
allowlist writer), and the two largest files, `alert-dispatch.sh` (1263 lines) and
`firewall-gatekeeper-monitor.sh` (998 lines), hold roughly a third of the unpinned statements
between them. Five harness libraries for exactly those tools survive as orphans under
`test/fixtures/` (`osquery-watchdog-lib.bash` 628 lines, `osquery-poller-lib.bash` 664,
`osquery-tailscale-lib.bash` 386, `osquery-allowlist-lib.bash` 198, `osquery-manifest-lib.bash`
175), loaded by nothing; they are the closest thing to a specification those tools have and the
plan's red-first tests for them start from what those files assert.
