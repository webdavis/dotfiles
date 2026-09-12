<!-- Design produced 2026-06-03 (Opus 4.8) via the senior-architect skill, grounded in the live
config at commit ab5322d: .chezmoitemplates/osquery/osquery.conf (file_paths + schedule) and
.chezmoitemplates/osquery/packs/intrusion-detection.conf. Motivated by a duplicate-alert bug
(one LaunchAgent -> 3 file_events_recent alerts) that exposed a row-as-alert design smell.
STATUS: proposal for review — not yet implemented. Verify §6 before any code. -->

# osquery Alerter Ingestion Model — Design

**Date:** 2026-06-03
**Component:** `dot_local/bin/executable_osquery-results-alerter.sh` (+ osquery config it consumes)
**Trigger:** A single fake LaunchAgent produced **3** `file_events_recent` CRITICAL alerts (CREATED +
UPDATED from the write, DELETED from cleanup), all mislabeled `Action: added`. The same event was caught
**once, cleanly** by a differential query (`persistence_startup_items_crontab`).

## 1. Verdict

**The duplicate-alert bug is a symptom; the root cause is a model mismatch — the alerter treats one
results.log *row* as one alert, but osquery emits rows from two fundamentally different query models.**
Patching with "dedupe `file_events_recent` by path" treats the symptom and guarantees more edge-case
churn (es_launchd_writes next, then cross-run repeats, then rename `dest_filename`, …).

The fix is a **detector-ownership** decision, mostly made in **osquery config, not the alerter**:
**one primary detector per subject — a differential state query wherever a state table exists; an
evented query only for subjects with no state table.** This *removes the duplication at its source*
(launchd is already covered cleanly by `persistence_launchd` + `startup_items`), shrinks the only place
coalescing logic is needed to three subjects (ssh keys, sudoers, sshd_config), and is **provably
no-false-negative** because every removal deletes a *redundant* detector, never sole coverage.

## 2. The core abstraction: an alert is a *subject-change*, not a *row*

Today: `1 results.log row → 1 alert`. That fights osquery's data model, because osquery has two row
models:

| Model | Tables | Row semantics | Per logical change |
|---|---|---|---|
| **Differential state** | `launchd`, `startup_items`, `crontab`, `launchd_overrides`, `kernel_extensions`, `system_extensions`, `suid_bin`, `listening_ports`, `last`, `alf`, `gatekeeper`, `sip_config`, `disk_encryption`, `screenlock`, `sharing_preferences` | one row per **state transition** (`added`/`removed`) | **exactly 1 row** (clean) |
| **Evented** | `file_events` (FSEvents), `es_process_file_events` (EndpointSecurity) | one row per **raw OS event** (CREATE, then UPDATE, then DELETE…) | **N rows** (noisy) |

The right unit is the **subject** — the file, launch item, process, kext, or protection that changed —
and the alert is "this subject meaningfully changed." Differential queries already deliver exactly that.
Evented queries deliver a raw event *stream* that must be *reduced to a subject-change* before alerting.

## 3. Grounded subject → detector map (from the live config)

| Subject | Has state table? | Current detectors | Verdict |
|---|---|---|---|
| **launchd persistence** (`~/Library/LaunchAgents`, `/Library/Launch{Agents,Daemons}`) | **Yes** (`launchd`, `startup_items`) | `persistence_launchd` (diff) + `persistence_startup_items_crontab` (diff) + `file_events_recent` launch_agents/launch_daemons (**evented**) + `es_launchd_writes` (**evented**) | **4× redundant.** Differential owns it; **drop evented file_events for launch_*** |
| **launchd_overrides** | Yes (`launchd_overrides`) | `persistence_launchd_overrides` (diff) | clean — keep |
| **crontab / startup items** | Yes | `persistence_startup_items_crontab` (diff) | clean — keep |
| **kexts / sysexts / suid / ports / logins** | Yes | dedicated diff queries | clean — keep |
| **security posture** (firewall, gatekeeper, SIP, FileVault, screenlock, sharing) | Yes | security-policy pack (diff/snapshot) | clean — keep |
| **ssh keys** (`~/.ssh`) | **No** | `file_events_recent` ssh (**evented**) | evented is the only option — **keep, coalesce** |
| **sudoers** (`/etc/sudoers*`) | **No** | `file_events_recent` sudoers (**evented**) | evented only — **keep, coalesce** |
| **sshd_config** (`/etc/ssh/sshd_config*`) | **No** | `file_events_recent` sshd_config (**evented**) | evented only — **keep, coalesce** |

**Key finding:** the *only* noise source is the evented queries, and for the launchd subject the evented
coverage is **pure redundancy** layered on top of clean differential coverage.

## 4. Design decisions

- **D1 — One primary detector per subject; prefer differential.** Where a state table exists, the
  differential query is the alerting source of truth (one alert per transition, by construction).
- **D2 — Drop redundant evented launchd coverage (osquery config).** Remove `launch_agents` and
  `launch_daemons` from `file_paths` in `osquery.conf`. `persistence_launchd` + `persistence_startup_items_crontab`
  retain full detection. **This single change eliminates the 3× duplication at the source** — no alerter
  dedup needed for launchd.
- **D3 — `es_launchd_writes` becomes *enrichment*, not an alert stream.** Its unique value is **process
  attribution** ("which process wrote the plist") — something the differential `launchd`/`startup_items`
  rows cannot tell you. Correlate it *into* the launchd differential finding (by path), rather than
  emitting it as its own alert(s). **Blocked on a separate bug** (it returned 0 rows in testing — see §6).
- **D4 — Genuinely-evented subjects (ssh/sudoers/sshd_config): keep evented + reduce in the alerter.**
  (a) Derive the action from `columns.action` (CREATED/UPDATED/DELETED), not osquery's differential
  `.action` (always `added`). (b) Coalesce by `target_path` **within a run**, keeping the
  highest-signal action. These three are the *entire* remaining surface for coalescing logic.
- **D5 — Normalize, then decide.** The alerter's pass-1 maps every finding — differential or evented —
  into a canonical record `{subject, kind, action, severity_inputs}`. The existing deterministic
  severity/enrichment/routing then operates on canonical records, not raw query shapes. New queries plug
  in by declaring their subject + action source; they don't each spawn new rendering special-cases.

## 5. No-false-negative argument (the binding constraint)

- **Dropping evented launchd coverage loses no detection.** `persistence_launchd`/`startup_items` are
  *state* queries: any plist that exists is a row; a new one is an `added` transition. Empirically, the
  test plist was caught by `persistence_startup_items_crontab` with one clean row. We remove a
  **redundant** detector, never sole coverage. (Must confirm the `launchd` table enumerates *user*
  `~/Library/LaunchAgents` — §6.)
- **Coalescing (D4) cannot drop a distinct threat.** It only collapses rows that share the **same
  subject path within one batch**. Two *different* files → two alerts. A genuinely *later* change to the
  same file arrives in a separate run (separate batch) → still alerts. It collapses a create+write storm
  on one file into one alert; it never merges across subjects or suppresses a later transition.
- **Severity stays deterministic and is never lowered by coalescing** — the kept row is the
  highest-signal action, and enrichment (unsigned → CRIT promotion) runs unchanged.

## 6. Verify before implementing (do NOT skip)

1. **Does the `launchd` table enumerate user `~/Library/LaunchAgents`?** Run
   `sudo osqueryi "SELECT path FROM launchd WHERE path LIKE '%LaunchAgents%';"`. If it only sees
   system/loaded jobs, dropping file_events for `~/Library/LaunchAgents` *would* lose coverage of an
   on-disk-but-unloaded user plist → in that case keep evented for `~/Library/LaunchAgents` and coalesce
   it instead of dropping. **This is the gating check for D2.**
2. **Detection-latency tradeoff.** Differential queries fire on their **interval**; evented fires near-
   instantly. Capture the persistence-query intervals (`jq '.queries[].interval'` on the pack) — if long,
   dropping evented adds latency. Decide: shorten the persistence interval vs. accept delay. (Security
   monitor on a personal Mac: minutes is acceptable; quantify it.)
3. **`es_launchd_writes` returns nothing** (caught 0 rows for a real `~/Library/LaunchAgents` write that
   FSEvents saw). D3 depends on it working. Separate debugging task — likely the `filename` column
   semantics vs the `LIKE '%/LaunchAgents/%'` predicate, or ES not emitting for that path. Verify before
   relying on it for attribution; until fixed, the launchd alert simply lacks the "who wrote it" field
   (fail-open, no regression).
4. **`file_events` after dropping launch_*** still must cover ssh/sudoers/sshd_config — confirm those
   `file_paths` entries remain and the alerter still renders them (with the D4 action fix).

## 7. Tradeoffs

- **(+)** Eliminates the duplicate-alert class at the source; removes 3 of 4 launchd detectors; shrinks
  coalescing scope from "all evented" to 3 fixed subjects; correct action labels; new queries stop
  spawning render special-cases (D5).
- **(−)** Slightly higher launchd detection latency (interval vs instant) unless intervals are shortened.
  Loses instant process-attribution *until* the es_launchd_writes bug is fixed (acceptable: attribution
  was already broken; this doesn't regress it).
- **(−)** D5 normalization is a non-trivial refactor of the alerter's pass-1 jq — but it's the change
  that stops the edge-case treadmill, which is the stated goal.

## 8. Phased plan (after §6 verification)

1. **Config (smallest, biggest win):** D2 — drop `launch_agents`/`launch_daemons` from `file_paths`;
   confirm latency acceptable. Re-run the LaunchAgent test → expect **1** clean alert from the
   differential query, correct action.
2. **Alerter D4:** action from `columns.action`; per-run coalesce-by-path for the 3 evented subjects.
3. **Alerter D5:** normalize findings to canonical records; route on those. (Largest; do last, with a
   synthetic-results.log test harness per the TDD skill.)
4. **Separate track:** fix `es_launchd_writes` (§6.3), then wire D3 attribution enrichment.

## 9. Open questions for the human

- Acceptable launchd detection latency? (sets §6.2 / whether to shorten intervals)
- Is process-attribution (D3/ES) a must-have, or nice-to-have? (sets priority of the es bug)
- Keep evented `~/Library/LaunchAgents` as a belt-and-suspenders *second* detector (coalesced), or fully
  trust the differential query? (depends on §6.1 result)
