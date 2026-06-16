# osquery Alerter Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended)
> or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Fix the live alerter defects the root-cause investigation + red-team surfaced: the duplicate /
mislabeled alerts, plus the precision holes that let real malware land quiet or look trusted.

**Architecture:** Two scripts + one config + one allow-list move. The **alerter**
(`osquery-results-alerter.sh`) stops treating each osquery *row* as an alert (coalesce evented findings;
show the real action). The **enricher** (`osquery-enrich-finding.sh`) stops trusting signature *names*
(verify the chain) and stops auto-trusting script payloads (assess them). The **allow-list** becomes
root-owned so it can't be poisoned. One **config** change drops the redundant noisy launchd detector.

**Tech Stack:** bash, jq, osquery, `codesign`/`spctl`, chezmoi, `shellcheck`/`shfmt`.

**Sources:** ingestion-model spec (`docs/superpowers/specs/2026-06-03-osquery-alerter-ingestion-model-design.md`)
+ red-team findings **#5** (allow-list poisoning), **#7** (forged codesign), **#8** (interpreter blind-spot).

**Scope:** the deterministic alerter/enricher only. The Mouse analysis-agent is a *separate* plan and is
**not** in scope here — but this hardening is its precision prerequisite.

---

## Conventions

- **TDD:** every behavior change gets a failing test first (a synthetic `results.log` line or a crafted
  artifact fed to the enricher), then the fix, then green.
- **No-false-negative is the binding rule:** a change may make a finding *louder* (NOTICE→CRIT) but must
  never make a real threat quieter or drop it. Coalescing only merges *same-subject same-batch* rows.
- **Worktree:** all work in the worktree opened at execution; commit per task.
- Test harness lives in `test/osquery-alerter/`. Run shell via `nix develop .#run --command`.

---

## File Structure

| Path | Responsibility | Change |
|---|---|---|
| `dot_local/bin/executable_osquery-results-alerter.sh` | parse/route findings | dedup evented findings by `target_path`; label from `columns.action`; read root-owned allow-list |
| `dot_local/bin/executable_osquery-enrich-finding.sh` | signing/trust verdict | verify signature **chain** (#7); assess script payloads (#8) |
| `.chezmoitemplates/osquery/osquery.conf` | osquery config | drop `launch_agents`/`launch_daemons` from `file_paths` (D2) — **gated on Task 1** |
| `.chezmoiscripts/run_onchange_before_50-setup-osquery.sh.tmpl` | installs `/var/osquery` | also install the allow-list root-owned at `/var/osquery/launch-allowlist.txt` (#5) |
| `dot_config/osquery/launch-allowlist.txt` | known-good labels | source for the root-owned copy |
| `test/osquery-alerter/*.sh` | tests | new |

---

## Task 1 — [VERIFY] Does the `launchd` table see user `~/Library/LaunchAgents`? (gates Task 7)

**Files:** none (records into the Verified Appendix).

- [ ] **Step 1: Query the live table**

Run:
```bash
sudo osqueryi --json "SELECT path FROM launchd WHERE path LIKE '%/Library/LaunchAgents/%';" | jq 'length'
```
Expected: a count > 0 that includes **user** (`/Users/...`) LaunchAgents, not just system ones.

- [ ] **Step 2: Decision gate (record result)**

If user LaunchAgents appear → the differential `persistence_launchd`/`startup_items` queries fully cover
launchd, so **Task 7 (drop the redundant evented detector) is safe.** Record "Task 7: GO."
If they do NOT → keep `file_events` for `~/Library/LaunchAgents` (don't drop it), and instead coalesce it
(Task 3 already does). Record "Task 7: SKIP — keep evented, coalesced."

- [ ] **Step 3: Commit the appendix note**

```bash
git commit -am "docs(plan): record launchd-table coverage verification (gates Task 7)"
```

---

## Task 2 — Show the real action (created/updated/deleted), not "added"

**Files:** Modify `dot_local/bin/executable_osquery-results-alerter.sh` (the pass-1 jq, ~line 110 where
`(.action // "changed") as $act`). Test: `test/osquery-alerter/test-action-label.sh`.

- [ ] **Step 1: Write the failing test**

```bash
#!/usr/bin/env bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
ALERTER=dot_local/bin/executable_osquery-results-alerter.sh
# A DELETED file_events row must render action "DELETED" (or "deleted"), never "added".
line='{"name":"file_events_recent","action":"added","columns":{"action":"DELETED","category":"launch_agents","target_path":"/Users/x/Library/LaunchAgents/com.test.plist","time":"1"}}'
out="$(printf '%s\n' "$line" | OSQUERY_TEST_RENDER=1 bash "$ALERTER" --render-stdin 2>/dev/null)"
echo "$out" | grep -qi 'delet' || { echo "FAIL: action not shown as deleted"; exit 1; }
echo "$out" | grep -qiw 'added' && { echo "FAIL: still shows differential 'added'"; exit 1; }
echo PASS
```
*(Note: this assumes a small test seam — `--render-stdin`/`OSQUERY_TEST_RENDER` that runs pass-1+render on
stdin without posting. Step 3 adds that seam if absent.)*

- [ ] **Step 2: Run it — expect FAIL** (`still shows 'added'`).

Run: `nix develop .#run --command bash test/osquery-alerter/test-action-label.sh`

- [ ] **Step 3: Implement**

In the pass-1 jq, derive the action per-query from the real column, not the differential `.action`:
```jq
# was: (.action // "changed") as $act
( (if .name == "file_events_recent" then (.columns.action // .action)
   elif .name == "es_launchd_writes" then (.columns.event_type // .action)
   else (.action // "changed") end) ) as $act
```
If no render-on-stdin test seam exists, add a guarded branch near the top of the script:
`[[ "${1:-}" == "--render-stdin" ]] && { run pass-1+render reading stdin; exit 0; }`.

- [ ] **Step 4: Run — expect PASS.** Commit.

```bash
git add dot_local/bin/executable_osquery-results-alerter.sh test/osquery-alerter/test-action-label.sh
git commit -m "fix(osquery-alerter): label findings with the real FSEvents/ES action, not differential 'added'"
```

---

## Task 3 — Coalesce evented findings by `target_path` per run (the dedup)

**Files:** Modify `osquery-results-alerter.sh` (the bash enrichment loop). Test:
`test/osquery-alerter/test-dedup.sh`.

- [ ] **Step 1: Write the failing test**

```bash
#!/usr/bin/env bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
ALERTER=dot_local/bin/executable_osquery-results-alerter.sh
P="/Users/x/Library/LaunchAgents/com.test.persistence-probe.plist"
# 3 file_events rows for ONE path (CREATE+UPDATE+DELETE) must collapse to ONE alert.
{ printf '{"name":"file_events_recent","action":"added","columns":{"action":"CREATED","category":"launch_agents","target_path":"%s","time":"1"}}\n' "$P"
  printf '{"name":"file_events_recent","action":"added","columns":{"action":"UPDATED","category":"launch_agents","target_path":"%s","time":"1"}}\n' "$P"
  printf '{"name":"file_events_recent","action":"added","columns":{"action":"DELETED","category":"launch_agents","target_path":"%s","time":"2"}}\n' "$P"
} > /tmp/dedup-in.jsonl
n="$(OSQUERY_TEST_COUNT=1 bash "$ALERTER" --count-alerts-stdin < /tmp/dedup-in.jsonl 2>/dev/null)"
[ "$n" = "1" ] || { echo "FAIL: expected 1 alert, got $n"; exit 1; }
echo PASS
```

- [ ] **Step 2: Run — expect FAIL** (3 alerts).

- [ ] **Step 3: Implement coalescing**

In the bash loop that iterates findings, keep a seen-set keyed on `q|ep` for the evented file queries only,
and skip a path already emitted this run (keeping the highest-signal action: CREATED > UPDATED > DELETED):
```bash
declare -A _seen_path
# inside the per-finding loop, after computing $q and $ep:
if [[ "$q" == "file_events_recent" || "$q" == "es_launchd_writes" ]] && [[ -n "$ep" ]]; then
  key="$q|$ep"
  if [[ -n "${_seen_path[$key]:-}" ]]; then continue; fi   # same subject already alerted this run
  _seen_path[$key]=1
fi
```
Differential queries (`pack_*persistence*`, `*_state`, etc.) are **never** coalesced — they're one-per-change.

- [ ] **Step 4: Run — expect PASS (1 alert).** Commit.

```bash
git commit -am "fix(osquery-alerter): coalesce evented file findings by path so one file = one alert"
```

---

## Task 4 — Enricher: verify the signature CHAIN, not the name (red-team #7)

**Files:** Modify `dot_local/bin/executable_osquery-enrich-finding.sh` (`assess_code()`, the `Authority`
case ~lines 63-71). Test: `test/osquery-alerter/test-codesign-chain.sh`.

- [ ] **Step 1: Write the failing test (replicates the red-team's spoof)**

```bash
#!/usr/bin/env bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
ENRICH=dot_local/bin/executable_osquery-enrich-finding.sh
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
cp /bin/echo "$work/m"   # a real Mach-O
# self-signed cert whose CN mimics Apple, ad-hoc/untrusted chain:
openssl req -x509 -newkey rsa:2048 -nodes -keyout "$work/k.pem" -out "$work/c.pem" -days 1 \
  -subj "/CN=Apple Mac OS Application Signing" >/dev/null 2>&1
# (sign + import omitted for brevity in this stub — the test signs $work/m with the spoofed identity)
# Expect: enricher returns exit 10 (UNVERIFIED chain) — NOT exit 0 "signed: Apple".
if out="$(bash "$ENRICH" --program "$work/m" 2>/dev/null)"; rc=$?; then :; fi
[ "$rc" = "10" ] || { echo "FAIL: spoofed-name binary not flagged (rc=$rc, out=$out)"; exit 1; }
echo PASS
```

- [ ] **Step 2: Run — expect FAIL** (current code prints `signed: Apple`, rc 0).

- [ ] **Step 3: Implement chain verification**

Replace the name-only `case` with an actual chain check before trusting:
```bash
# after confirming a signature exists:
if codesign --verify --strict "$f" >/dev/null 2>&1 && spctl -a -t exec "$f" >/dev/null 2>&1; then
  printf 'signed: %s\n' "$auth"; return 0          # trusted chain
else
  printf 'signed but UNVERIFIED chain (untrusted)\n'; return 10   # name says trusted, chain does not
fi
```
The attacker-chosen `Authority` string may still be *displayed* for context, but it no longer drives the
exit code.

- [ ] **Step 4: Run — expect PASS.** Commit.

```bash
git commit -am "fix(osquery-enrich): verify codesign chain (spctl/--verify), don't trust the Authority name"
```

---

## Task 5 — Enricher: assess script payloads, don't auto-trust interpreters (red-team #8)

**Files:** Modify `osquery-enrich-finding.sh` (`is_interpreter()` branch, ~lines 85-113, which currently
`exit 0`). Test: `test/osquery-alerter/test-interpreter-payload.sh`.

- [ ] **Step 1: Write the failing test**

```bash
#!/usr/bin/env bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
ENRICH=dot_local/bin/executable_osquery-enrich-finding.sh
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
printf '#!/bin/bash\necho evil\n' > "$work/payload.sh"; chmod 0644 "$work/payload.sh"  # user-writable
xattr -w com.apple.quarantine "0001;0;test;" "$work/payload.sh" 2>/dev/null || true
# A LaunchAgent that runs /bin/bash <payload>: must NOT auto-pass. Expect rc 10 (CRIT) for a
# quarantined / user-writable script payload.
cat > "$work/a.plist" <<EOF
<?xml version="1.0"?><plist version="1.0"><dict>
<key>ProgramArguments</key><array><string>/bin/bash</string><string>$work/payload.sh</string></array></dict></plist>
EOF
if out="$(bash "$ENRICH" --plist "$work/a.plist" 2>/dev/null)"; rc=$?; then :; fi
[ "$rc" = "10" ] || { echo "FAIL: quarantined/user-writable script payload not promoted (rc=$rc)"; exit 1; }
echo PASS
```

- [ ] **Step 2: Run — expect FAIL** (current code exits 0 "payload unverified").

- [ ] **Step 3: Implement payload assessment**

In the interpreter branch, instead of unconditionally `exit 0`, assess the resolved script:
```bash
# script_path = ProgramArguments[1] (the resolved script)
if [[ -n "$script_path" && -e "$script_path" ]]; then
  if xattr -p com.apple.quarantine "$script_path" >/dev/null 2>&1 \
     || [[ -w "$script_path" && ! -O "$script_path" ]] \
     || find "$script_path" -newermt '-1 day' >/dev/null 2>&1; then
    printf 'runs UNVERIFIED script %s (quarantined/recent/writable) via %s\n' "$(basename "$script_path")" "$interp"
    return 10    # promote: a new launchd item running a suspicious script is CRIT, not NOTICE
  fi
fi
printf 'runs script %s via %s — payload checked, no red flags\n' "$(basename "$script_path")" "$interp"
return 0
```

- [ ] **Step 4: Run — expect PASS.** Commit.

```bash
git commit -am "fix(osquery-enrich): assess interpreter script payloads; promote quarantined/writable to CRIT"
```

---

## Task 6 — Root-own the launch allow-list (red-team #5)

**Files:** Modify `.chezmoiscripts/run_onchange_before_50-setup-osquery.sh.tmpl` (install the allow-list
root-owned) and `osquery-results-alerter.sh` (read from the root-owned path; never suppress a CRIT/failed-
trust finding). Test: `test/osquery-alerter/test-allowlist.sh`.

- [ ] **Step 1: Write the failing test**

```bash
#!/usr/bin/env bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
ALERTER=dot_local/bin/executable_osquery-results-alerter.sh
# A CRIT finding whose label IS on the allow-list must STILL alert (allow-list only quiets NOTICE).
grep -q 'OSQUERY_LAUNCH_ALLOWLIST.*var/osquery' "$ALERTER" \
  || { echo "FAIL: alerter does not read the root-owned /var/osquery allow-list"; exit 1; }
echo PASS
```

- [ ] **Step 2: Run — expect FAIL** (still reads `~/.config/...`).

- [ ] **Step 3a: Install the allow-list root-owned**

In the setup script, alongside the config install, add:
```bash
install_root /var/osquery/launch-allowlist.txt < "$ALLOWLIST_SOURCE"   # 0644 root:wheel via the existing helper
```

- [ ] **Step 3b: Point the alerter at it + keep CRIT exempt**

```bash
# default to the root-owned, sudo-only path:
ALLOWLIST_FILE="${OSQUERY_LAUNCH_ALLOWLIST:-/var/osquery/launch-allowlist.txt}"
```
Confirm the existing CRIT-exempt guard (allow-list never suppresses when `sev == CRIT`) is intact, so a
known-good *label* reused by a hostile *payload* (now CRIT via Task 4/5) is never silenced.

- [ ] **Step 4: Run — expect PASS.** Commit.

```bash
git commit -am "fix(osquery): root-own the launch allow-list so it can't be poisoned (red-team #5)"
```

---

## Task 7 — [GATED on Task 1] Drop the redundant evented launchd detector (ingestion-model D2)

**Files:** Modify `.chezmoitemplates/osquery/osquery.conf` (`file_paths`).

- [ ] **Step 1: Only if Task 1 = "GO".** Remove `launch_agents` and `launch_daemons` from `file_paths`
  (keep `ssh`, `sudoers`, `sshd_config` — they have no state-table alternative). The differential
  `persistence_launchd` + `persistence_startup_items_crontab` retain full launchd coverage.

- [ ] **Step 2: Validate config** (reuse the lint check from the osquery-config validation work):

Run: `chezmoi execute-template '{{ includeTemplate "osquery/osquery.conf" . }}' | jq empty && echo OK`

- [ ] **Step 3: Commit** (or, if Task 1 = SKIP, record "Task 7 skipped — launchd table didn't cover user agents").

```bash
git commit -am "refactor(osquery): launchd persistence via differential queries; drop redundant evented detector"
```

---

## Task 8 — Integration test: the original bug + the malware that used to slip

**Files:** `test/osquery-alerter/test-integration.sh`.

- [ ] **Step 1: Write the end-to-end test** (stub `curl`/`alerter`; feed a synthetic `results.log`):

```bash
#!/usr/bin/env bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
# 1) The original bug: 3 file_events rows for one plist → exactly ONE alert, labeled with a real action.
# 2) #8: a script-payload LaunchAgent (quarantined) → routes to #priority (CRIT), not the quiet channel.
# 3) #7: a spoofed-"Apple"-name unsigned binary → CRIT, not "signed: Apple".
# Assert alert count + channel routing for each. (Reuses the stubs from the existing synthetic harness.)
echo "see assertions inline"
```
Fill each assertion using the same stub pattern as Tasks 2/3 (count alerts + capture routed severity).

- [ ] **Step 2: Run — all three PASS.** Commit.

```bash
git commit -am "test(osquery-alerter): integration — dedup + #7 + #8 fixed end to end"
```

---

## Task 9 — Lint, package, final review

- [ ] **Step 1:** `nix develop .#run --command ./scripts/lint.sh` (shellcheck/shfmt the new tests + edited
  scripts; the `.worktrees` prune means lint won't reach across worktrees). Expected: green.
- [ ] **Step 2:** Add `test/` to `.chezmoiignore` (source-only, exactly like `scripts/`) so the new
  root-level test dir is **not** applied to `$HOME`. Verify: `chezmoi managed | grep -c '^test/'` → `0`.
  Then confirm chezmoi naming for any other new files; `just l` green.
- [ ] **Step 3:** Re-run the live synthetic-LaunchAgent test from the earlier session (the
  `com.test.persistence-probe.plist` harness) and confirm in #priority: **one** correctly-labeled alert.
- [ ] **Step 4: Commit + open PR** (from the worktree).

---

## Self-review checklist (run before handoff)

- Original bug (dup + label) → Tasks 2, 3, 8. ✓
- Red-team #7 (codesign chain) → Task 4. ✓  #8 (interpreter) → Task 5. ✓  #5 (allow-list) → Task 6. ✓
- ingestion-model D2 (drop redundant evented) → Task 7 (gated). ✓
- No-false-negative preserved: every change makes findings louder or equal, never quieter; coalescing
  merges only same-subject same-batch rows; CRIT is never allow-list-suppressed. ✓

---

## Dependency note

This is the **precision foundation** the Mouse analysis-agent plan
(`2026-06-03-osquery-analysis-agent.md`) depends on (a monotonic Mouse amplifies a noisy/false-positive
CRITICAL). Build this **first.**
