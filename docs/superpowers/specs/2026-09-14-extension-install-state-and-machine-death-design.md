# Extension install-state monitoring and off-host machine-death detection

Status: design, written 2026-09-14 while the operator was asleep. NOT approved. Nothing is built and
no code changed. Every choice made in the operator's place is listed in "Assumptions made in the
operator's place" with its alternative, and the decisions they owe are in "Open questions".

Ledger bullet this answers, under "Recover the remaining design from PR #24":

> Preserve deferred install-state kernel-extension monitoring and off-host machine-death detection.
> The former needs reconciliation with July's decision to alert on untrusted loaded extensions; do
> not restore June's obsolete delivery block. The latter needs an external host and remains homelab
> scope: a watchdog on the monitored Mac cannot detect that Mac disappearing from outside it. Keep
> intentional log-only `es_launchd_writes` handling and accepted residual risks out of the
> implementation queue.

The two halves share nothing except their origin in the June 2026 osquery alerting work, so they are
designed separately below and only the assumptions and open questions are pooled.

## Constraints that bind this design

From the repository's own rules and the recorded operator rulings, in the order they matter here:

- **posture is a shippable product.** Nothing in it may assume this repository exists, and its watch
  list is user-configured: posture ships recommended paths, the user owns the final list. Deriving a
  watched set from `chezmoi managed` is a dotfiles-only coupling that must not survive into posture.
- **Optimal over cheap** (operator, 2026-09-05). Rank designs by result quality, never by effort.
  State the cost, do not let it pick.
- **No first-workable answers, and a manual operator step is a design failure** (operator,
  2026-07-11), tempered by the fact that one half of this task genuinely cannot be automated from the
  monitored host.
- **This repository builds no removal mechanisms** (operator, 2026-08-02).
- **We test the behavior of tools we wrote, and nothing else** (operator, 2026-08-05). Declarations
  stay unguarded on purpose, so a config-only half of a change is caught by review, not by a gate.
- **Rust files target 300 lines and never exceed 500**, unit tests included.
- A change to either templated desired-state file under `dot_local/libexec/posture/converge/desired/`
  needs a FULL `chezmoi apply`. The known-good manifest records the new render while the deployed copy
  still holds the old one, so the pipeline audit pages a CRIT on every tick until that apply lands.

## Part one: install-state kernel-extension monitoring

### What is already decided, and what the record actually says

**June 2026 moved the loaded-extension detector to log-only, on measurement.** The decision addendum
(`docs/superpowers/decisions/2026-06-10-osquery-alerting-v2-decision-addendum.md` at PR #24's head
`2202dcbf`, D-V2-3) records the reason in one line: "657 real load/unload events; the
`kernel_extensions` table is *loaded* kexts (load on demand). Load-state is a firehose and the
**wrong signal**; Apple-filtering masks the count but not the signal. Too noisy even for digest."

**June also recorded the replacement it did not build.** The same document (D-V2-9, question 3) and
the master specification (`2026-06-10-osquery-alerting-master-spec-v2.md` section 14, question 3) both
say: "kext redesign: future install-state detector for page; log-only now", elaborated as "a future
install-state detector (differential over `/Library/Extensions` - the staged kext DB) could be
page/core. Not in v2." Section 13 of that specification lists "the install-state kext redesign" as
deferred. That deferral is what this ledger bullet is preserving.

**July 2026 changed the loaded-extension tier without touching install state.** The operator ruling
dated 2026-07-22 lives in the code and its tests, not in a document. Current behavior:

- `dot_local/libexec/osquery/results-alerter/route.sh:180-183`: "A newly-loaded kernel extension: an
  UNTRUSTED one (enrichment promoted it to CRIT above) pages; a signed one stays log-only, its base
  tier. Operator ruling 2026-07-22."
- `route.sh:192-200`: a signed system extension digests, an untrusted one pages, same ruling.
- The Rust port carries the same shape: `posture/crates/posture-domain/src/gate.rs:105`
  (`KernelExtensionsNew => critical`, where `critical` is the page-if-promoted binding built at
  `gate.rs:93-97`) and `gate.rs:107-113` for system extensions.
- Pinned as specification items S049 and S050 in
  `docs/superpowers/specs/2026-09-05-posture-behavioral-specification.md`, and by
  `kernel_extension_without_promotion_remains_log_only` and
  `system_extensions_digest_without_untrusted_promotion` in the posture gate tests.

So July raised the loaded detector from "never pages" to "pages when the signing verdict is
untrusted". It did not answer the install-state question, and nothing since has.

### The reconciliation, stated plainly

Load state and install state are different questions, and the July ruling answers only the first.
Measured on this machine on 2026-09-14:

| Question                       | Source                       | Today's answer                        |
| ------------------------------ | ---------------------------- | ------------------------------------- |
| Which are loaded right now?    | `kernel_extensions`, 252     | One non-Apple row, `__kernel__`       |
| Which kernel ones are present? | `/Library/Extensions`        | HighPointIOP, HighPointRR, SoftRAID   |
| Which are staged for loading?  | `/Library/StagedExtensions`  | `Library/Filesystems/macfuse.fs`      |
| Which system ones are present? | `system_extensions` table    | Karabiner DriverKit, OBS camera, LuLu |

Commands and output, so this is checkable rather than asserted:

```
osqueryi --json "SELECT count(*) AS loaded FROM kernel_extensions;"          -> 252
osqueryi --json "SELECT name, version, path FROM kernel_extensions
                 WHERE name NOT LIKE 'com.apple.%';"                          -> one row, __kernel__
kmutil showloaded --list-only | grep -v com.apple                             -> no extension rows
ls -1 /Library/Extensions                                                     -> 3 .kext bundles
find /Library/StagedExtensions -maxdepth 3 -name '*.kext' -o -name '*.fs'     -> macfuse.fs
```

Four third-party kernel extensions are installed or staged on this host and **zero are loaded**. The
loaded table, the only thing the July ruling gates on, currently says nothing at all about any of
them. That is the gap, and it is not a theoretical one: an extension installed today and loaded on
demand next month is invisible for a month, and when it does load, the page depends on a signing
verdict that a legitimately signed but vulnerable driver passes.

The install-state set is also the opposite of a firehose. Modification times:

```
/Library/Extensions                 2026-03-19
/Library/Extensions/HighPointIOP.kext   2025-11-22
/Library/Extensions/HighPointRR.kext    2025-11-22
/Library/Extensions/SoftRAID.kext       2023-02-09
/Library/StagedExtensions           2025-12-25
  .../Library/Filesystems/macfuse.fs    2024-02-11
```

One change to the directory in the last six months, and bundles dating to 2023 and 2025. A detector
keyed on writes to that set is rare, boot-persistent and actionable, which is exactly the profile the
June specification reserved for the page tier.

System extensions need no equivalent work. The `system_extensions` table already reports install and
activation state directly (the `state` column reads `activated_enabled` for all three third-party
entries), which is why that detector digests rather than being log-only. Nothing is added for it.

### Approaches considered

**Approach 1: query the install directories with a scheduled differential.** osquery has no installed
or staged kernel-extension table, so this means a `file` table query over `/Library/Extensions/%%`
plus `/Library/StagedExtensions/%%` on an interval, differential on path and hash. It works, it is
self-contained, and it costs one new scheduled query. The cost is latency (an interval, not an event)
and a second mechanism doing what file integrity monitoring already does in this pipeline.

**Approach 2: watch the install directories with the file integrity monitoring already in place.** The
pipeline already carries eight watched categories under `file_paths` in
`dot_local/libexec/posture/converge/desired/osquery.conf.tmpl`, feeding the `file_events_recent`
detector on a ten-second interval, with `--enable_file_events=true` in `osquery.flags`. It is proven
on system-owned directories: production results carry 108 rows for the `launch_daemons` category,
which watches `/Library/LaunchDaemons/%%`. Adding one category is a declaration plus one gate arm.
Events arrive in near real time and the render already has a watched-file field block.

**Approach 3: watch the kernel extension management database instead.**
`/private/var/db/KernelExtensionManagement` holds `Staging`, the auxiliary kernel collection and
`DextRecordTable.plist`, which is closer to "approved to load" than either directory above. It is
rejected for two concrete reasons: Endpoint Security file integrity monitoring already mutes that
subtree through `--es_fim_mute_path_prefix=/private/var/folders,/private/tmp,/private/var/db,...`, so
one of the two publishers is deliberately blind there; and the auxiliary kernel collection is rebuilt
by operating-system updates as well as by extension approvals, so it carries churn the two install
directories do not.

**Recommendation: approach 2.** It reuses the mechanism, it is event-driven, and the whole change is
one configuration declaration, one enumeration variant, one gate arm and one shared helper.

### Recommended design

Behaviors, each written so a test can be red before the code exists.

**B1. A watched category for extension install state.** `file_paths` gains `extension_install` with
two globs, `/Library/Extensions/%%` and `/Library/StagedExtensions/%%`. The staged tree is watched at
its root rather than at `Library/Extensions` beneath it, because the only staged bundle on this
machine sits at `Library/Filesystems/macfuse.fs` and a narrower glob would miss every staged
filesystem. The paths are posture's recommended set and the user's configuration is authoritative;
nothing derives them from `chezmoi managed`. They are NOT added to `file_paths_hashes`: hashing feeds
the integrity verdict that the manifested categories consume, and these bundles are third-party files
with no known-good manifest to compare against.

**B2. A `FileCategory::ExtensionInstall` variant**, mapped from the `category` column string
`extension_install`. The existing `FileCategory::Other => GateOutcome::LogOnly` arm
(`gate.rs:159`) means the configuration half alone is silent rather than wrong, which is the safe
direction for a half-applied change. Pin the mapping, and pin that the existing eight categories keep
their outcomes.

**B3. The gate arm pages on any event in the category, with no direction gate.** Justification is the
measured base rate: one write to the install set in six months. A delete is worth a page too, because
an extension being removed from under the operator is as interesting as one arriving, and there is no
churn to drown it. This deliberately avoids plumbing the file-event verb into the gate: `GateColumns`
carries `launchd`, `file_category` and `target_path` today, the verb lives in `columns.action` and is
already rendered by `watched_file_fields`, and the existing `SshdConfig => page` arm sets the
precedent for an arm that ignores direction.

**B4. Findings sharing a subject collapse to one page block.** `render_page`
(`posture/crates/posture-domain/src/page.rs:95`) emits one block per critical finding up to
`BLOCK_LIMIT = 8`, under a `BODY_LIMIT = 1900` body cap. Each installed bundle here holds five files,
so one extension install produces roughly five to ten watched-file findings, which would consume the
entire block budget and push a concurrent unrelated critical finding behind the
"... and N more CRITICAL finding(s)" marker. That is a real defect, not a cosmetic one: an
attacker-triggered install would evict the evidence of whatever else fired in the same batch.

So `render_page` groups by subject before it blocks. The subject of a watched-file finding is the
bundle root, derived by truncating `target_path` at the first `/`-delimited component whose name ends
in `.kext`, `.fs`, `.bundle`, `.app`, `.systemextension`, `.dext` or `.appex`, and falling back to the
whole target path when no such component exists. Findings with the same detector and the same subject
render one block that names how many rows it represents. This is also exactly what the sibling ledger
bullet asks for ("per-run grouping of repeated findings about the same subject"), so it is written
once in `posture-domain` and every detector inherits it rather than being special-cased here.

Behaviors to pin: five rows inside one bundle render one block and the block says five; two different
bundles in one batch render two blocks; two different detectors never merge even with an equal
subject; nine distinct subjects still render eight blocks plus the marker; a target path with no
bundle component is its own subject.

**B5. The enrichment path for a watched extension file is the bundle root.** Specification item S032
records that `ep` for `file_events_recent` is `target_path`, and S141 records that the enricher
assesses a `.kext` suffix directly but needs a Mach-O verdict otherwise. A create inside
`Contents/MacOS/` therefore gets the inner binary assessed rather than the extension, so the signing
fact on the page describes the wrong object. Setting `ep` to the derived bundle root fixes that and
reuses B4's derivation, one function with two callers.

**B6. The loaded-extension tiers do not move.** `kernel_extensions_new` stays log-only unless
enrichment promotes it, `system_extensions_new` stays digest unless enrichment promotes it, and the
July 2026-07-22 ruling is untouched. Install state is additive, keyed on a different detector. Pin by
leaving S049, S050 and their tests unchanged and green.

### Why June's delivery block is not restored

The obsolete block is Task 6 of `2026-06-10-osquery-alerting-master-plan-v1.md`, "New kernel / system
extension (promote to PAGE)". It proposed rewriting the pack queries to be differential on the
identity column with an Apple name filter, promoting both to the page set, and rendering a dedicated
header and next-step through the delivery path of the time. Three reasons it stays retired:

1. **Its premise was measured false four days later.** D-V2-3 rejected exactly this shape on 657
   load/unload events, recording that Apple-filtering "masks the count but not the signal". Today's
   measurement agrees from the other direction: the Apple-filtered query returns one row, and that
   row is the kernel itself, so the filter that was supposed to make load state quiet makes it empty.
2. **July already supplied the load-state answer**, and it is better than Task 6's: page when the
   signing verdict is untrusted, rather than page on every new identifier.
3. **Its delivery path no longer exists.** Task 6 rendered into the page body that
   `osquery-alert-dispatch.sh` posted to a Hermes `#priority` route. Delivery is pns now: posture is a
   producer that pipes a request into `pns submit --json`
   (`posture/crates/posture-adapters/src/pns_producer.rs:35`) and pns owns the four destinations.
   Restoring the block would reintroduce a dispatch design, not just a detector.

### Failure modes and accepted noise

- **A macFUSE or SoftRAID upgrade pages.** Re-staging or reinstalling a third-party extension rewrites
  bundle files and fires the category. Accepted: the bundles changed twice in three years, the event
  is operator-initiated, and a page that says "your kernel extension changed" moments after you
  upgraded it is correct rather than noisy. Deliberately NOT allowlistable: the allowlist is keyed on
  launchd identity, and adding a path-keyed allowlist would create a new trust surface for a
  once-a-year event.
- **System Integrity Protection on, on someone else's machine.** `/Library/StagedExtensions` is
  restricted when System Integrity Protection is enabled. It is disabled on this host (`csrutil
  status` reports disabled), so whether osqueryd as root still receives events there is UNVERIFIED
  here. A silent no-op is the failure, and nothing in the pipeline audit would notice it. Record it as
  a known limit of the shipped recommendation.
- **A pipeline problem hides the page.** Unchanged from every other detector: the uptime watchdog
  covers a dead or wedged pipeline, and the who-watches-the-watchers circularity remains an accepted
  residual from June.
- **Approval without a write.** An extension already present on disk that is later approved and staged
  by the operating system produces no event in either watched directory if the staging happens under
  `/private/var/db/KernelExtensionManagement`. Out of scope, see approach 3 above.

### Security

`target_path` is attacker-controlled. It flows into the subject derivation and into the page body.
Two properties must hold and must be pinned with hostile fixtures rather than assumed:

- The derivation is a pure prefix truncation on `/`-delimited components, with no shell and no regular
  expression over the raw value, so a bundle named with an embedded newline or tab cannot split a page
  into extra lines. The page already routes variable fields through one sanitize chokepoint
  (`sanitize::squash` at `page/fields.rs:107`); the subject must go through the same one.
- The subject is used for grouping and for the enrichment path only, never for a trust decision, so a
  crafted name can merge a finding's block with another of the same detector at worst. It can never
  suppress a page, because the arm pages unconditionally. Say this in the code comment, because the
  reverse would be a silent bypass.

The repository's own review history is the reason to insist on the fixture: the PR #548 review found
a formatter test with no hostile fixture, where reverting the command left every test green.

## Part two: off-host machine-death detection

### Why it cannot be built on the monitored host, verified

Every delivery path originates on dresden:

- the Discord log posts to `http://127.0.0.1:8644/webhooks/pns`
  (`pns/crates/pns-adapters/src/destinations/hermes.rs:21`), a gateway running on the same machine;
- the phone card spawns the local `moshi-hook` binary
  (`pns/crates/pns-adapters/src/moshi_hook.rs:36-38`);
- the banner and the lamps are local by construction.

A dead host therefore produces silence, and silence is the normal state of a healthy edge-triggered
pipeline. That is the exact reason the uptime watchdog exists, and it is also why the watchdog cannot
close this gap: it is a process on the host whose death it would have to report.

**There is no second always-on computer.** `tailscale status` on 2026-09-14 returns three nodes:

```
100.77.192.92   dresden                stephen@  macOS  -
100.70.242.14   dresden-tailscale-gui  stephen@  macOS  offline, last seen 82d ago
100.109.58.54   mister                 stephen@  iOS    -
```

One live Mac, one stale node for the same Mac's retired graphical Tailscale client, and an iPhone.
The ledger's claim that this half "needs an external host" is correct as stated.

**The monitored host is a laptop that sleeps, and that constraint reshapes the whole feature.**
`system_profiler` reports MacBookPro17,1 (Apple M1). `pmset -g custom` reports `sleep 0` on alternating
current and `sleep 1` on battery, so it sleeps whenever it is unplugged and idle. The consequence for
any beacon comes straight from `/usr/share/man/man5/launchd.plist.5`:

- `StartInterval`: "If the system is asleep during the time of the next scheduled interval firing,
  that interval will be missed due to shortcomings in kqueue(3)."
- `StartCalendarInterval`: "Unlike cron which skips job invocations when the computer is asleep,
  launchd will start the job the next time the computer wakes up. If multiple intervals transpire
  before the computer is woken, those events will be coalesced into one event upon wake from sleep."

So the 15-minute uptime watchdog (`StartInterval 900`, `com.webdavis.osquery-uptime-watchdog.plist`)
is the wrong beacon carrier: it stops beaconing across every closed lid. The daily heartbeat
(`StartCalendarInterval` at 09:00 local, `com.webdavis.osquery-heartbeat.plist`, running
`posture heartbeat`) is the right one: a missed daily beacon means the machine never woke that day,
which is the actual signal wanted.

That reframes the feature honestly. On a laptop, off-host machine-death detection is a day-granularity
tripwire for theft, destruction or a host held by someone else. It is not a liveness monitor, and any
design that promises minutes will page on every trip and every weekend.

### Approaches considered

**Approach 1: a second host the operator owns.** The optimal target and the one the June boundary
document assumed (D-V2-10 lists "an off-host consumer for cross-host machine-death" under later,
homelab automation). No new third party, the alert path stays inside the tailnet, and the suppression
for planned downtime lives somewhere reachable. Blocked today: no such host exists.

**Approach 2: a scheduled GitHub Actions workflow as the consumer.** Uses infrastructure already
trusted with this repository's source and checks, so it adds no new vendor. Verified against GitHub's
documentation: the shortest schedule is once every five minutes, the `schedule` event "can be delayed
during periods of high loads of GitHub Actions workflow runs", and "in a public repository, scheduled
workflows are automatically disabled when no repository activity has occurred in 60 days". This
repository is public (`gh-axi repo view` reports `visibility: public`) and is pushed daily, so the
inactivity disable is moot in practice but is a real failure mode during a long quiet period, which is
precisely when the operator might be away.

The sharp edge is where the beacon lands. A public repository means a committed liveness timestamp
publishes when the operator's machine is on and off, which is an occupancy signal about a home, not
just a technical fact. The beacon must therefore land somewhere private: a private gist that dresden
updates, read by the workflow through the API and judged on its `updated_at`, is the smallest private
store GitHub offers. That in turn puts a write-scoped token on dresden.

**Approach 3: a hosted dead-man's-switch service.** Healthchecks is the reference implementation.
Verified from its self-hosting documentation: Berkeley Software Distribution 3-clause license,
self-hostable, and self-hosting requires "Python 3.12+, Django 6.0, PostgreSQL or MySQL", meaning the
self-hosted form needs the same missing host as approach 1. The hosted form needs no host at all and
is purpose-built, with real routing and escalation. Its price is a new vendor holding the occupancy
signal and owning the one alert path that exists when everything else is gone.

**Approach 4: the phone as the consumer.** Parked on platform grounds rather than investigated: iOS
gives no reliable background schedule to an application that would have to notice an absence, and the
device is the alert destination in every other path, not a scheduler. No moshi documentation was
checked for this, so treat the moshi half as unexamined rather than ruled out.

**Approach 5: the network appliance.** pns already authenticates to the router
(`router_url = "https://192.168.1.1"` with an API key in KeePassXC) to read device presence, so there
is an always-on non-Mac device on the network. Not proposed: running code on a third-party appliance
is a homelab decision, and this repository's rule is to configure third-party tools through their own
supported options only.

### Recommended scope, which is not a build

The ledger is right that this stays out of the implementation queue, and the reason is stronger than
"no host": the beacon's shape is decided by which consumer wins, so a beacon built first is dead code
whose interface is guessed. Recommend recording the scope below, answering the consumer question, and
then building the beacon and the consumer together in one slice.

The shape, so that day is a wiring job rather than a redesign:

1. **The beacon rides the daily heartbeat**, for the launchd reason above. `posture heartbeat` already
   runs on a calendar interval, already reaches pns, and already exists on every machine posture is
   installed on.
2. **The beacon is liveness only, and is sent regardless of the heartbeat's health verdict.** A
   pipeline problem must not masquerade as machine death, and a pipeline problem must not hide machine
   death. So it is a separate side effect of the same run, not a field of the health alert.
3. **The consumer pages after two consecutive missing daily beacons**, roughly 48 hours. Anything
   tighter pages on travel and on long sleeps. State the ceiling in the notification itself so the
   operator reads it as a tripwire rather than a monitor.
4. **Planned-downtime suppression lives on the consumer, never on dresden.** A suppression stored on
   the dead machine is unreachable by definition. This is the one piece that cannot be a dotfiles
   change, and it is the reason approach 3 looks better than approach 2 for a while: a hosted service
   has a pause button, a workflow needs one built.
5. **The consumer's alert cannot go through pns**, because pns is on the dead machine. Discord is the
   one existing surface that pushes to the phone from off-host. That is not a violation of "pns owns
   delivery"; it is the single alert whose whole purpose is that pns cannot carry it. Say so where the
   consumer is configured, so a later reader does not "fix" it back through pns.

## Out of scope, explicitly

- **Restoring June's Task 6 block.** Covered above. The Apple-filtered differential over the loaded
  extension table, its dedicated render and its retired dispatch path all stay retired.
- **Re-tiering `es_launchd_writes`.** It stays log-only regardless of the enrichment verdict. The
  reason is in `route.sh:172-179`: it is the raw Endpoint Security write event already covered by the
  `persistence_launchd` differential, which pages through the allowlist and enrichment, so honoring
  its promotion would double-signal the same subject. The ledger asks for this to stay out of the
  queue and this design does not touch it.
- **The June accepted residuals.** The who-watches-the-watchers circularity, a root attacker writing
  to `/System` on this System-Integrity-Protection-off host, the un-attestable editable Hermes tree,
  the base64 at-rest spool body, and allowlist authority equalling Discord account authority. Recorded
  as accepted, not queued.
- **`/System/Library/Extensions`.** 721 entries, Apple-owned, rewritten by operating-system updates.
- **The kernel extension management database and the auxiliary kernel collection.** Approach 3 above.
- **FleetDM, fleet beaconing and Wazuh research**, the approval interface, and the Hermes investigation
  workflow. Each is its own ledger bullet in the same section.
- **Any removal mechanism** for the retired June artifacts.

## Assumptions made in the operator's place

Each is a choice this document made because the operator was asleep. The alternative is stated so a
single word reverses it.

1. **Install-state monitoring is worth building, and is additive to the July ruling.** Alternative:
   close the deferral and leave extensions exactly as they are, on the grounds that four installed
   extensions are known and a fifth arriving is something the operator would notice. Cost of the
   alternative: a staged extension stays invisible until it loads, and a legitimately signed one never
   pages at all.
2. **The new category pages unconditionally.** Alternative: mirror the loaded-extension arm and page
   only on an untrusted signing verdict. Cost: a legitimately signed but vulnerable driver, which is
   the realistic modern kernel-extension attack, would at most digest.
3. **Any event pages, including a delete, with no direction gate.** Alternative: page only on create
   and update, which needs the file-event verb plumbed into the gate as a new field.
4. **The subject collapse lands in `render_page` and is shared by every detector**, so it also answers
   the sibling ledger bullet about per-run grouping. Alternative: collapse only extension-install
   findings at judge time and leave that bullet open.
5. **`Page.count` keeps counting critical findings while the body groups them.** Alternative: count
   subjects, so a five-file install reads "1 CRITICAL". The first keeps the number honest against
   `results.log`; the second reads better on a phone.
6. **The staged tree is watched at its root**, because the only staged bundle here is under
   `Library/Filesystems`. Alternative: the narrower `Library/Extensions` glob, which would miss every
   staged filesystem.
7. **Machine-death stays out of the queue and no beacon is built ahead of a chosen consumer.**
   Alternative: build the beacon now with a configurable endpoint. Cost: an interface guessed before
   the consumer that defines it exists.
8. **If interim coverage is wanted before a homelab host exists, the scheduled workflow is preferred
   over a hosted service**, because GitHub is already trusted with this repository. Alternative: the
   hosted service, which is purpose-built and has a pause button, at the price of a new vendor holding
   a home-occupancy signal.
9. **The overdue threshold is two missed daily beacons, about 48 hours.** Alternative: one missed
   beacon, halving detection time and paging whenever a trip or a long sleep spans a day boundary.

## Open questions

1. Build install-state extension monitoring, or close the deferral and keep extensions as they are?
   (Gating for part one.)
2. Unconditional page, or signing-gated like the loaded-extension arm?
3. Does the subject collapse land as the shared per-run grouping mechanism, closing the sibling ledger
   bullet, or scoped to this detector only?
4. Does `Page.count` count findings or subjects?
5. Is a macFUSE or SoftRAID upgrade paging acceptable, or is a path-keyed extension allowlist wanted,
   accepting a new trust surface?
6. Is there, or will there be, a second always-on host, and on what horizon? (Gating for part two: the
   answer decides whether any interim is built at all.)
7. If an interim is wanted: scheduled GitHub Actions workflow with a private gist beacon, or a hosted
   dead-man's-switch service?
8. What is the longest period the machine is legitimately dark (travel, a weekend away)? That sets the
   threshold and the false-page rate.
9. Where does planned-downtime suppression live, given it cannot live on dresden?

## Verification, when either half is approved

- **The install-state page is real, not rendered.** Create a throwaway bundle under
  `/Library/Extensions`, confirm a page arrives naming that bundle, and confirm the body carries one
  block rather than five. Remove it and confirm the delete also pages.
- **The collapse does not evict.** Fire one bundle install alongside one unrelated critical finding in
  the same batch and confirm both blocks appear, with no "and N more" marker.
- **The loaded-extension tiers did not move.** S049 and S050 and their existing tests stay green with
  no edits, and a signed loaded extension still produces nothing.
- **The hostile name.** A bundle whose name carries a newline and a tab renders one block and splits no
  lines.
- **The half-applied change is silent.** Deploy the configuration without the enumeration variant and
  confirm the category falls to log-only rather than to a wrong tier.
- **The apply.** Editing either templated desired-state file needs a full `chezmoi apply`; verify the
  known-good manifest and the deployed copy agree afterwards, and expect a CRIT from the pipeline
  audit on every tick until it lands.
- **The beacon, if built.** Close the lid for longer than one calendar day and confirm the consumer
  pages; wake the machine inside the window and confirm it does not.
