# osquery Alerter Redesign — Decision Log

Running decisions from the problem-by-problem review of the 8-problem inventory (2026-06-08).
**Supersedes stale assessments** in the spec
(`specs/2026-06-03-osquery-alerter-ingestion-model-design.md`) and plan
(`plans/2026-06-04-osquery-alerter-hardening.md`) where noted. Feeds the eventual re-spec.
Problems #2–#8 will be appended as we work through them.

______________________________________________________________________

## #1 — Duplicate alarms (one startup file → 3 identical alerts)

**Root cause:** the alerter turned each raw filesystem event into its own alert, and launchd
persistence was covered redundantly — clean differential *state* queries **and** noisy *evented*
file-watches at the same time. One test plist's create → write → delete fired 3 alerts. (They were
also all mislabeled "added" — that label bug is problem #2, tracked separately.)

### Decisions

1. **launchd → differential state model only.** Drop the evented file-watch on `launch_agents` and
   `launch_daemons` (remove from `file_paths`; = spec D2 / plan Task 7). The differential
   `persistence_launchd` + `persistence_startup_items_crontab` become the **sole** alarm source —
   one clean row per state change, nothing to dedup.
   - **Verified live (Task 1 = GO):** the `launchd` table lists **all 21** user
     `~/Library/LaunchAgents` plists — including unloaded ones — identically as user and as root.
     No coverage is lost by dropping the file-watch.

2. **`es_launchd_writes` → KEEP, as attribution enrichment (= spec D3). It is NOT broken.**
   - The spec's "returns 0 rows / blocked" is **stale**: the logs show EndpointSecurity was
     `disabled via configuration` on May 29–30; it is enabled now
     (`--disable_endpointsecurity=false`) and capturing.
   - **Verified live:** captured real writes — `/usr/libexec/PlistBuddy` →
     `com.webdavis.paseo-daemon.plist`; `Docker.app/.../install` → `com.docker.vmnetd.plist` +
     `com.docker.socket.plist`. That proves the running binary is the **official cask build**
     (Developer ID: OSQUERY LLC, with `com.apple.developer.endpoint-security.client`), ES is on,
     and Full Disk Access is granted.
   - Wire it as a **"written by &lt;process&gt;"** detail on the differential launchd finding — not
     its own alert stream. Bonus: the writer's identity is a strong trust signal ("Docker.app wrote
     this" = calm; "bash in /tmp wrote this" = alarm).
   - Caveat: it shows the same multi-event duplication (Docker plists logged ×2); harmless as
     enrichment.

3. **Bash event-coalescing (plan Task 3) → REJECTED as the fix.** It's a symptom-patch, not how
   osquery tooling handles this. Research confirms: prefer differential *state* at the source; do any
   dedup/suppression in the pipeline, never a hand-rolled seen-set in the alert script.

4. **ssh keys / sudoers / sshd_config → KEEP file_events (evented); fix the label only.** These are
   security-critical files with **no state table**, and file_events (real-time FSEvents) is the right
   tool: it catches *every* change the instant it happens, including a transient tamper-and-revert
   that a polled differential query would miss. **No coalescing** (user decision — wrong pattern) and
   **no hash-diff** (an over-correction — polling would miss transients and add latency). These files
   change rarely, so the occasional 1–2 events per edit are fine as raw, correctly-labeled events.
   The only fix needed is the action label → see **#2**.

### Net #1 outcome

launchd alarm = differential only · launch-dir file-watch = dropped · `es_launchd_writes` =
attribution enrichment · ssh/sudoers/sshd_config = **keep file_events** (real-time) + fix the label
(#2), **no coalescing, no hash-diff**.

### Corrections logged (do not re-litigate)

- osquery here is the **official cask** build (ES-entitled), **not** the Homebrew formula.
- The `launchd` table **does** enumerate user `~/Library/LaunchAgents` (incl. unloaded).
- `es_launchd_writes` **works** — the "broken" note was stale (ES had simply been disabled in config
  when the spec was written).

### Still open under #1

- None — #1 is fully decided.

______________________________________________________________________

## #2 — Wrong action label ("added" when it was created / modified / deleted)

**Root cause:** the alerter read osquery's outer differential `.action` — which is *always* `"added"`
for evented rows — instead of the real FSEvents action in `.columns.action`
(`CREATED` / `UPDATED` / `DELETED`).

**Scope after #1:** launchd is moot (now differential → real `added`/`removed`); `es_launchd_writes`
is enrichment ("written by <process>"). So the label fix lands on exactly the three remaining evented
file subjects: **ssh / sudoers / sshd_config**.

### Decisions

1. **Read the real action.** For `file_events_recent` rows, derive the action from `.columns.action`,
   not `.action` (alerter ≈ line 110). = the plan's **Task 2**, scoped to the three file subjects:

   ```jq
   (if .name == "file_events_recent" then (.columns.action // "changed")
    else (.action // "changed") end) as $act
   ```

2. **Polish — header matches the verb.** The header currently reads a generic "sudoers changed."
   Make it reflect the real action — "sudoers **modified**" / "sudoers **deleted**" / "new SSH key" —
   driven by `.columns.action`, lowercased/prettified at render time.

**Net #2 outcome:** a one-line column fix + a header verb-match. #2 is otherwise dissolved by #1
(launchd moved to differential, so no more "added" mislabel there).

______________________________________________________________________

## Architecture pivot → Reshape v2 (2026-06-09)

The detector-by-detector "fix the noisy alerter" approach (incl. #1/#2 above) is **superseded and folded
into** a tiered reshape, driven by the no-noise / ADHD constraint + two research passes + an external
build-guidance file. **Full design:** `specs/2026-06-09-osquery-alerting-reshape-design.md`.

- **Two tiers, one gate:** a tiny high-confidence **PAGE tier** → Discord (via **Hermes**, the fixed
  sink, hostname-tagged); **everything else LOG-ONLY** on disk, queryable, never pushed. Healthy host =
  zero page rows.
- **PAGE set:** new launchd persistence (+ es_launchd_writes "written by"), new SSH key (via the
  `authorized_keys` *table*), new admin/user, **protections-OFF** (firewall/SIP/Gatekeeper/FileVault),
  new **suid-root** binary, and the **agent attack-surface** (agent binary swap, network-exposure
  change, auth-file change).
- **LOG-ONLY:** drift / browser extensions / generic ports / logins / raw file_events. Borderline
  (kext/sysext, sudoers/sshd_config, launchd overrides, cron) start log-only.
- **Calibrate per host:** discard first differential run, label one week, **one-action allowlist**
  (Discord reaction / single command). **Daily heartbeat** so silence = safe.
- **Defer:** the LLM judge (Mouse) — and when added, it may gate **only the noisy tier**, never the page
  tier (prompt-injection-to-suppress risk); **Wazuh** migration; beaconing / port detection.

**How #1/#2 fold in:** #1 (launchd → differential + es attribution) is the core of page-detection 1.
#2 (label fix for ssh/sudoers/sshd_config) is now mostly moot — ssh moved to the `authorized_keys` table
(page), and sudoers/sshd_config dropped to log-only.

______________________________________________________________________

## Senior re-review (2026-06-10) → existence-based launchd, single-host scope, +10 fixes

After the model upgrade to **Fable 5**, an **ultracode multi-agent workflow** (6 adversarial lenses, each
with live read-only verification on Dresden; refute-based double-verify; run `wf_c1d04c7d-9fa`) re-reviewed
the build plan and produced **12 confirmed findings** (1 rejected: an evented admin-group detector — admin
has a clean state table, so it would re-introduce the noise decision #1 killed). The settled-design
invariants (deterministic page path, no-LLM-gating, one calm channel) were **not** re-litigated.

### Decisions

1. **launchd page detection → REVERTED to existence-based on the differential `launchd` LABEL.** The
   multi-signal evented gate a prior pass built (`launchd_write_suspicious`: resolve the plist program via
   `plutil`, codesign/quarantine/sketchy-location scoring, writer-path allowlist, ANDed) is **deleted as
   structurally wrong.** On this host the legit agents and a malware dropper are the **identical
   `interpreter + script` shape** (verified live: `com.claude.code`, all `com.webdavis.osquery-*`,
   `ai.hermes.gateway`, openclaw, etc. run via `bash`/`python`/`node` LaunchAgents), so the enricher is
   forced to `exit 0` on interpreters or it false-pages every agent — meaning the gate **never fires on a
   `bash ~/Library/.../evil.sh` LaunchAgent**, and a Developer-ID-signed Mach-O skates too (enricher
   returns rc 0 for any authority). The one existence-level detector (`persistence_launchd`) had been
   demoted to log-only → **planted persistence was silent end-to-end**, on the host whose stated primary
   threat IS agent-planted persistence. **This restores the spec (spec:41) and decision #1.** New page
   condition: a **new launchd `label`** appears → system LaunchDaemons (`path LIKE '%/LaunchDaemons/%'`)
   page ALWAYS; user LaunchAgents page unless the label is in a per-host **label** allowlist. Empirical
   grounding: the *entire* `results.log` history holds only **2 distinct writer processes**
   (`/Applications/Docker.app/.../install` ×8, `/usr/libexec/PlistBuddy` ×1) and **3 distinct labels** —
   so a writer-PATH allowlist provably hands a bypass the moment calibration allowlists PlistBuddy (a
   LOLBin), whereas a LABEL allowlist of those 3 still pages any new label regardless of how it was written
   or signed. `es_launchd_writes` + signing/quarantine stay **enrichment TEXT only** ("written by
   `<process>`"), never a page suppressor (es_process_file_events has no `label`/`program` column anyway).

2. **SSH page → REVERTED from the hourly `authorized_keys` table to real-time `file_events`** (restores
   decision #4). The table is a polled differential and **misses a sub-hour add-key → log-in → remove-key**;
   `file_events` (FSEvents) catches the transient. Page only on CREATED/UPDATED to a basename
   `authorized_keys`/`authorized_keys2` (NOT DELETED — a revert's trailing delete and a legit key removal
   stay quiet). The `authorized_keys` **table** becomes the enrichment lookup (username/algorithm/key_file
   only — never the key or sha256).

3. **SINGLE-HOST scope (Stephen, 2026-06-10).** This implementation targets **only Dresden** and is
   provisioned via this chezmoi/dotfiles repo. **Multi-host is explicitly NOT built here** — when Hermes
   moves to the homelab NUC, this implementation **migrates out of dotfiles** into homelab automation
   (Ansible/K8s), which is where multi-host fan-in is built. Dissolves the "fleet rollout pages 10/11 hosts
   into a dead loopback" finding (there is no second host with this applied). All "fleet / 11 machines"
   framing is removed from the spec and plan; the deferred section records the homelab migration (and
   cross-host machine-death detection, which is impossible host-locally) as belonging to that future layer.

4. **Ten supporting fixes folded into the plan** (all confirmed, design-fit-judged against the no-noise
   bar): (a) `pytest` added to the flake; `just test` runs **bats-only** until Task 9 creates the plugin
   tests, then extends to both legs in place (was red Tasks 0–8). (b) **Hostname** injected once in the
   shared `send_alert` as a structured body field (`{event_type, host, alert}`) — keeps the
   `X-Request-ID = sha256(body)` dedup coherent; all four producers inherit it. (c) **Durable delivery
   spool**: a page that dies on a transient 429/5xx is written to `~/.local/state/osquery-alert-spool/`
   (NDJSON, 600) and re-POSTed with its same body/sig/`X-Request-ID` (idempotent ≤1h) on the next alerter
   run AND at each watchdog tick; the watchdog pages if an entry is older than one tick. (d) Heartbeat
   `RunAtLoad=true` so a boot/reload after 09:00 still emits the day's ✅ (extra ✅ on reboot is acceptable;
   a missing ✅ is not). (e) The **uptime watchdog now also asserts the heartbeat LaunchAgent is loaded**
   (the component whose silence is the safety signal). (f) **Agent binary hash-watch → the RESOLVED native
   binary**, not the launcher/interpreter symlink (codex: the vendored aarch64 binary, not
   `/opt/homebrew/bin/codex`); for Hermes's editable source tree, gate an `es_process_file_events` rule on
   an untrusted+unknown writer, or **state the gap plainly** (an editable install can't be hash-attested)
   rather than leave a blind spot. (g) Task 7 detectors get explicit intervals (agent_binary 3600,
   agent_exposure 600). (h) Task 3 bats fixtures rebuilt as **valid plists with loud failure** (the
   `plutil -insert` on `<plist/>` was failing silently → tests passed by accident) — now label-existence
   tests. (i) Spec deferred section records cross-host machine-death as part of the homelab layer. (j) The
   spec's stale "differential + es-enrichment" launchd wording is now correct again (the plan reverted to
   match it) and open-item #1's "v1 = CLI" line is removed (the allowlist UX is the Hermes plugin).
