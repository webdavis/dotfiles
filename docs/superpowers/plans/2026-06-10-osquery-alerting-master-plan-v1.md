# osquery Alerting — Master Implementation Plan (v1)

> **Note (2026-06-10):** a three-tier revision supersedes this plan's *tiering* — see
> `plans/2026-06-10-osquery-alerting-master-plan-v2.md` (+ the v2 spec, tier matrix, test matrix, and decision
> addendum). This v1 remains intact as history.

> **For agentic workers:** REQUIRED SUB-SKILLS: superpowers:test-driven-development (red→green→commit;
> a change that turns a previously-green test red is a regression and is **not allowed**) and
> superpowers:subagent-driven-development (or executing-plans). Steps use `- [ ]` checkboxes.

**Goal:** Reshape the osquery alerter so the one Discord channel pages **only** on a fixed set of
proven-high-confidence threats; everything else stays log-only in `results.log`; and a recurring legit
launchd job is silenced with a one-message `allow <label>` reply in Discord.

**Scope:** **single host — Dresden (this macOS system)**, provisioned via this chezmoi/dotfiles repo.
Multi-host is explicitly out of scope: when Hermes moves to the homelab NUC this implementation migrates
*out* of dotfiles into homelab automation (Ansible/K8s), which is where multi-host fan-in is built. No
fleet framing here.

**Architecture:** Keep the daemon, packs, `send_alert` dispatch, the 60s firewall/gatekeeper poller, and
the uptime watchdog. Add **one PAGE gate** in the alerter's enrich loop: a finding dispatches only if its
query is in the PAGE set *and* it is page-worthy; everything else `continue`s (dropped from delivery, kept
in `results.log`). Delete the quiet `#osquery` dispatch. launchd persistence pages on a **new launchd
label** (existence-based, differential `launchd` table); SSH/sudoers/sshd_config and the pipeline's own
files page on **real-time `file_events`**; new admin, suid-root, new kext/sysext, and the agent surface page
on their differential/FIM detectors; a daily ✅ heartbeat; a durable delivery spool; and a Hermes **plugin**
(supported extension surface, no core patch) for one-reply allowlisting. Hermes secrets live in
`~/.hermes/.env` (chezmoi + KeePassXC); `config.yaml` is a seed-once chezmoi `create_` template.

> **Senior re-review (2026-06-10, decisions log).** A prior pass built a multi-signal launchd gate
> (`launchd_write_suspicious`: resolve the plist program via `plutil`, codesign/quarantine/writer-path
> scoring, ANDed). It is **deleted as structurally wrong** — on this host legit agents and a malware
> dropper are the identical `interpreter + script` shape, so the enricher must `exit 0` on interpreters →
> the gate never fires on a `bash ~/Library/.../evil.sh` LaunchAgent, and a Developer-ID binary skates.
> **Existence-on-label** is the fix (restores the spec + decision #1; empirically only 2 writers / 3 labels
> in all of `results.log` history). Signing/quarantine/writer ride as **enrichment text, never a page
> suppressor**.

**Tech stack:** bash, jq, osquery (official cask, ES-entitled), `codesign` (via the existing enricher),
chezmoi, **bats-core**, pytest (the plugin), shellcheck/shfmt, the Nix `run` shell.

**Spec:** `docs/superpowers/specs/2026-06-10-osquery-alerting-master-spec-v1.md` — the master design spec
(single source of design truth; its §PAGE set maps one-to-one to the tasks below).
**Decisions (rationale/history):** `decisions/2026-06-08-osquery-alerter-redesign-decisions.md` (the
2026-06-10 senior-review section).

---

## Conventions (read once)

- **TDD, strict.** Each behaviour: failing test → run red → minimal impl → run green → commit. **Every
  task ends with every test that EXISTS green** (`just test`) — never-yet-created legs are not yet wired,
  not failing. A change that turns a previously-green test red is a regression and is not allowed.
- **THE PAGE SET (single source of truth).** These `$q` values may dispatch; nothing else does:
  `persistence_launchd` (new label — system daemons always, user agents minus the label allowlist),
  `file_events_recent` (only categories `authorized_keys`/`sudoers`/`sshd_config`/`pipeline_integrity`, only
  CREATED/UPDATED), `new_admin_user`, `suid_bin_unexpected`, `kernel_extensions_new`,
  `system_extensions_new`, `agent_authfile_changed`, `agent_binary_changed`, `agent_exposure_changed`,
  `sip_state`/`filevault_state`/`screenlock_state`/`remote_access_sharing_state` (only when OFF).
  **`firewall_state`/`gatekeeper_state` are deliberately NOT here — the 60s poller owns them.**
- Bash tests use **bats** (`test/osquery-alerter/*.bats`, sharing `lib.bash`); the plugin uses **pytest** in
  root `test/` (never under `dot_hermes/`, which chezmoi would deploy). bats assertions use the set-e-safe
  `lib.bash` helpers `assert_no_page` / `assert_page_has <s>` / `assert_page_lacks <s>` (run-based — a bare
  `! cmd | grep` is unreliable under bats' `set -e`, per the bats docs).
- **The launchd allowlist is keyed on the launchd LABEL** (reverse-DNS, e.g. `com.docker.vmnetd`), stored
  one-per-line in `~/.config/osquery/page-launchd-allowlist.txt`. (The old writer-path allowlist is gone.)
- **KeePassXC pattern (verified in-repo):** `{{ (keepassxc "<System> :: <Kind> :: <detail>").Password }}`.
  Never `chezmoi add` a secret file raw.
- Line anchors below are against `dot_local/bin/executable_osquery-results-alerter.sh` /
  `…-alert-dispatch.sh` as they stand today; the implementer greps the cited anchor to confirm before editing.
- Commit per task; **no `Co-Authored-By` trailer.**

---

## File Structure

| Path | Responsibility | Change |
|---|---|---|
| `flake.nix` | dev shell | add `pkgs.bats`; add `pytest` to the python env |
| `justfile` | `just test` (bats-only until Task 10, then + pytest) | add recipe |
| `test/osquery-alerter/lib.bash` | harness: HOME fixture, 4-arg dispatch stub, enricher stub, `run_alerter`, assert helpers | create |
| `test/osquery-alerter/*.bats` | per-detector bats | create |
| `test/hermes-allowlist/test_handler.py` | the plugin's pytest (root test/, not deployed) | create |
| `dot_local/bin/executable_osquery-results-alerter.sh` | the PAGE gate + per-detector render | modify |
| `dot_local/bin/executable_osquery-alert-dispatch.sh` | hostname in body + durable spool + `_drain_spool` | modify |
| `dot_local/bin/executable_osquery-uptime-watchdog.sh` | drain spool + guard heartbeat agent | modify |
| `.chezmoitemplates/osquery/osquery.conf` | add `authorized_keys`/`pipeline_integrity` file_paths; es query stays log-only | modify |
| `.chezmoitemplates/osquery/packs/intrusion-detection.conf` | promote launchd + kext/sysext queries | modify |
| `dot_local/bin/executable_osquery-heartbeat.sh` + plist + loader | daily ✅ beat (RunAtLoad) | create |
| `dot_config/osquery/webhook-secret.tmpl` | shared HMAC, KeePassXC | create |
| `dot_hermes/plugins/osquery-allowlist/{plugin.yaml,__init__.py}` | the `allow <label>`-reply plugin | create |
| `dot_hermes/private_dot_env.tmpl` | Hermes secrets, KeePassXC; owner/channel ids | create |
| `dot_hermes/create_private_config.yaml.tmpl` | Hermes config, seed-once (`create_`); secrets via `${VAR}` | create |
| `.chezmoiignore` | add `test/`, `dot_hermes/plugins/**/__pycache__` | modify |

---

## Task 0 — bats + pytest in the flake, the harness (commits green)

**Files:** `flake.nix`, `justfile`, `test/osquery-alerter/lib.bash`, `test/osquery-alerter/test_smoke.bats`, `.chezmoiignore`.

- [ ] **Step 1:** add `pkgs.bats` to the `flake.nix` package list (next to `pkgs.shellcheck`), and add
  `pytest` to the python env so the plugin's tests run deterministically (NOT host-PATH pytest):
  change the mdformat python wrapper to `(pkgs.python312.withPackages (ps: with ps; [ mdformat mdformat-gfm pytest ]))`.
- [ ] **Step 2:** add the **bats-only** recipe to `justfile` (Task 10 extends it in place to add the pytest leg):

```make
test:
    nix develop .#run --command bats test/osquery-alerter
```

- [ ] **Step 3:** write `test/osquery-alerter/lib.bash`:

```bash
# shellcheck shell=bash
REPO_ROOT="$(git rev-parse --show-toplevel)"
ALERTER="$REPO_ROOT/dot_local/bin/executable_osquery-results-alerter.sh"

setup_fixture() {
  FIX="$(mktemp -d)"
  mkdir -p "$FIX/.local/bin" "$FIX/.local/log/osquery" "$FIX/.local/state" "$FIX/.config/osquery"
  # 4-arg stub matching the real send_alert <sev> <title> <detail> [sound].
  cat >"$FIX/.local/bin/osquery-alert-dispatch.sh" <<'STUB'
send_alert() { printf '%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "${4-}" >>"$HOME/.local/log/osquery/dispatch.log"; }
STUB
  # Enricher stub: echoes $ENRICH_VERDICT, exits $ENRICH_RC (default trusted rc 0).
  cat >"$FIX/.local/bin/osquery-enrich-finding.sh" <<'STUB'
printf '%s' "${ENRICH_VERDICT:-signed: Apple}"; exit "${ENRICH_RC:-0}"
STUB
  chmod +x "$FIX/.local/bin/osquery-enrich-finding.sh"
  : >"$FIX/.config/osquery/page-launchd-allowlist.txt"
}
teardown_fixture() { [ -n "${FIX:-}" ] && rm -rf "$FIX"; }

# run_alerter <one raw results.log JSON line>
run_alerter() {
  printf '%s\n' "$1" >"$FIX/.local/log/osquery/osqueryd.results.log"
  printf '0 0\n' >"$FIX/.local/state/osquery-results-offset"
  HOME="$FIX" \
    OSQUERY_RESULTS_LOG="$FIX/.local/log/osquery/osqueryd.results.log" \
    OSQUERY_RESULTS_OFFSET="$FIX/.local/state/osquery-results-offset" \
    bash "$ALERTER" >/dev/null 2>&1 || true
}
dispatch_log() { cat "$FIX/.local/log/osquery/dispatch.log" 2>/dev/null; }

# set-e-safe bats assertions (run-based — a bare `! cmd | grep` is unreliable under bats' set -e).
assert_no_page()    { run dispatch_log; [ -z "$output" ]; }
assert_page_has()   { run dispatch_log; [[ "$output" == *"$1"* ]]; }
assert_page_lacks() { run dispatch_log; [[ "$output" != *"$1"* ]]; }
```

> Offset note: the alerter resets `prev_offset=0` when the log's inode ≠ the state's `prev_inode`, so the
> pre-written `0 0` makes it read the whole fixture file.

- [ ] **Step 4:** write `test/osquery-alerter/test_smoke.bats` — green **today** (no expectation that depends
  on changes this plan makes):

```bash
setup() { load lib; setup_fixture; }
teardown() { teardown_fixture; }

@test "harness: an unknown query name never dispatches (filtered at pass-1)" {
  run_alerter '{"name":"zzz_unknown","action":"added","columns":{}}'
  assert_no_page
}
```

- [ ] **Step 5:** `just test` → PASS. Add `test/` and `dot_hermes/plugins/**/__pycache__` to
  `.chezmoiignore`. Verify nothing test-y is managed:

```bash
chezmoi managed | grep -E 'test/|__pycache__' && echo LEAK || echo ok   # expect: ok
```

- [ ] **Step 6:** commit.

```bash
git add flake.nix justfile test/osquery-alerter/ .chezmoiignore
git commit -m "test(osquery-alerter): bats harness + bats/pytest in the dev shell"
```

---

## Task 1 — The PAGE gate (only the page set dispatches) + hostname in every alert

**Files:** the alerter + the shared dispatch helper. Test: `test/osquery-alerter/test_gate.bats`.

- [ ] **Step 1: Failing tests:**

```bash
setup() { load lib; setup_fixture; }
teardown() { teardown_fixture; }

@test "drift never pages" {
  run_alerter '{"name":"pack_installed-software-drift_installed_apps","action":"added","columns":{"name":"Foo"}}'
  assert_no_page
}
@test "a new admin user pages (end-to-end)" {
  run_alerter '{"name":"new_admin_user","action":"added","columns":{"username":"backdoor","uid":"503"}}'
  assert_page_has "backdoor"
}
@test "a protection re-enable (NOTICE) never pages" {
  run_alerter '{"name":"pack_security-policy-regression_sip_state","action":"added","columns":{"enabled":"1"}}'
  assert_no_page
}
@test "a firewall-OFF row never pages here (the poller owns it)" {
  run_alerter '{"name":"pack_security-policy-regression_firewall_state","action":"added","columns":{"global_state":"0"}}'
  assert_no_page
}
```

- [ ] **Step 2:** `just test` → new-admin FAILs (dropped at the pass-1 name filter); protection/firewall FAIL
  (currently dispatched).
- [ ] **Step 3a — pass-1 name filter** (the `select(.name ...)` at alerter ≈ line 107): admit the bare PAGE
  names; **drop `es_launchd_writes` and `new_ssh_key`** (es is log-only enrichment; SSH is now file_events):

```jq
  | select(.name != null and ((.name | startswith("pack_")) or (.name == "file_events_recent")
      or (.name | IN("new_admin_user","agent_authfile_changed","agent_binary_changed","agent_exposure_changed"))))
```

- [ ] **Step 3b — the PAGE gate**, in the enrich loop immediately after the enrichment block (after the
  `sig=$("$ENRICH" …)` block, before the legacy launch-allowlist). `ALLOWLIST` is read once near the top of
  the script alongside the other config: `ALLOWLIST="${OSQUERY_LAUNCHD_ALLOWLIST:-$HOME/.config/osquery/page-launchd-allowlist.txt}"`.

```bash
  # PAGE gate (reshape): default-deny. Only a fixed page set dispatches; everything
  # else `continue`s — dropped from delivery but retained in osquery's results.log.
  act=$(jq -r '.act // ""' <<<"$obj")
  case "$q" in
    new_admin_user | suid_bin_unexpected | agent_authfile_changed | \
    agent_binary_changed | agent_exposure_changed | \
    kernel_extensions_new | system_extensions_new)
      sev="CRIT" ;;                                   # always page when they fire
    persistence_launchd)                              # existence-based: a NEW label pages
      [[ $act == added ]] || continue                 # removed/uninstalled = log-only
      label=$(jq -r '.cols.label // ""' <<<"$obj")
      path=$(jq -r '.cols.path // ""' <<<"$obj")
      case "$path" in
        /System/Library/*) continue ;;               # Apple OS jobs (OS-update churn) — provenance via the /System path
        */LaunchDaemons/*) sev="CRIT" ;;             # 3rd-party system daemon: always page (not label-allowlistable)
        *) grep -qxF -- "$label" "$ALLOWLIST" 2>/dev/null && continue || sev="CRIT" ;;
      esac ;;
    file_events_recent)                               # real-time security-file changes
      fa=$(jq -r '.cols.action // ""' <<<"$obj")      # the REAL FSEvents action (decision #2)
      case "$cat" in
        authorized_keys | sudoers | sshd_config | pipeline_integrity)
          case "$fa" in CREATED | UPDATED) sev="CRIT" ;; *) continue ;; esac ;;
        *) continue ;;                                # launch dirs etc → log-only
      esac ;;
    sip_state | filevault_state | screenlock_state | remote_access_sharing_state)
      [[ $sev == "CRIT" ]] || continue ;;             # page only when protection_off set sev=CRIT
    firewall_state | gatekeeper_state)
      continue ;;                                     # the 60s poller owns these
    *) continue ;;                                    # log-only
  esac
```

- [ ] **Step 3c — delete the `#osquery` dispatch** (the whole `if [[ $ocount -gt 0 ]]` block, ≈ lines 317-328).
  Only the `pcount` / #priority `send_alert CRIT` remains.
- [ ] **Step 4: hostname in every alert** — inject ONCE in the shared `send_alert`
  (`executable_osquery-alert-dispatch.sh`), so all four producers inherit it. Near the top of `send_alert`:

```bash
  local host="${OSQUERY_HOSTNAME:-$(scutil --get LocalHostName 2>/dev/null || hostname -s)}"
```

  and add it to the signed body (keeps `X-Request-ID = sha256(body)` coherent):

```bash
  body=$(jq -cn --arg h "$host" --arg t "$title" --arg d "$detail" \
    '{event_type:"osquery.alert", host:$h, alert:{title:$t, detail:$d}}')
```

  Confirm the Hermes gateway renders `alert.host` into the Discord message; if it does not, prefix
  `[$host] ` into the title instead (no gateway change needed). Update the spec Phase-0 contract note (done).

- [ ] **Step 5:** `just test` → **all PASS.** Commit.

```bash
git commit -am "feat(osquery-alerter): single PAGE gate; drop #osquery channel; hostname in dispatch body"
```

---

## Task 2 — Numeric baseline-discard (counter==0)

**Files:** the alerter pass-1. Test: `test/osquery-alerter/test_baseline.bats`.

- [ ] **Step 1: Failing tests** (osquery emits `counter` as a JSON **number**):

```bash
setup() { load lib; setup_fixture; }
teardown() { teardown_fixture; }
@test "counter 0 (baseline) does not page" {
  run_alerter '{"name":"new_admin_user","action":"added","counter":0,"columns":{"username":"backdoor"}}'
  assert_no_page
}
@test "counter 1 (real differential) does page" {
  run_alerter '{"name":"new_admin_user","action":"added","counter":1,"columns":{"username":"backdoor"}}'
  assert_page_has "backdoor"
}
```

- [ ] **Step 2:** `just test` → counter-0 FAILs (number `0` survives the absent filter).
- [ ] **Step 3:** insert the discard as the **first pipeline stage after `fromjson`**, before the
  snapshot-explode (which strips top-level `counter`):

```jq
  . as $line | (try ($line | fromjson) catch empty)
  | select(((.counter // 1) | tonumber? // 1) != 0)
```

- [ ] **Step 4:** `just test` → **all PASS.** Commit `fix(osquery-alerter): discard counter==0 baseline`.

---

## Task 3 — launchd persistence: existence-based on the LABEL

**Files:** `packs/intrusion-detection.conf` (promote the query), the alerter (render + ep-map). Test:
`test/osquery-alerter/test_launchd.bats`.

Page when a **new launchd label** appears (differential `launchd` table): **system LaunchDaemons always**;
user LaunchAgents unless the label is in `page-launchd-allowlist.txt`. The Task-1 gate already implements
this; this task wires the query, the enrichment, and the render. Signing rides as **text, never a suppressor**.

- [ ] **Step 1: Config** — in `intrusion-detection.conf`, `persistence_launchd` selects the columns the
  gate/render need and is differential. **Verified live:** `program` is empty for real jobs (the command is
  in `program_arguments`), `path` populates for all 929 rows, and `/System/Library/*` is Apple OS-update
  churn (the gate skips it):

```json
"persistence_launchd": {
  "query": "SELECT label, path, program, program_arguments FROM launchd WHERE label != '';",
  "interval": 600, "removed": true
}
```

  Leave the `es_launchd_writes` schedule entry **as-is (log-only)** — it is no longer admitted by the alerter
  pass-1 filter, so it stays in `results.log` as forensic context, never paged.

- [ ] **Step 2: ep-map + sev** in the alerter so the enricher runs on the plist (for signing TEXT) and the
  row is enrich-eligible before the gate: ep-map → `elif (.name | test("_persistence_launchd$")) then (.columns.path // "")`
  (already present); confirm `persistence_launchd` classifies **NOTICE** in the `sev` def (the
  `test("^pack_intrusion-detection_persistence_")` arm) so enrichment runs and the gate can promote it.

- [ ] **Step 3: Failing tests** — existence + label-allowlist + system-daemon (valid plists are not needed;
  the gate keys on `label`/`path`/`action`, not the plist contents):

```bash
setup() {
  load lib; setup_fixture
  echo "com.docker.vmnetd" > "$FIX/.config/osquery/page-launchd-allowlist.txt"
}
teardown() { teardown_fixture; }
NAME='pack_intrusion-detection_persistence_launchd'

@test "a NEW unknown user LaunchAgent label pages" {
  run_alerter "{\"name\":\"$NAME\",\"action\":\"added\",\"counter\":1,\"columns\":{\"label\":\"com.evil.dropper\",\"path\":\"$HOME/Library/LaunchAgents/com.evil.dropper.plist\",\"program\":\"/bin/sh\"}}"
  assert_page_has "com.evil.dropper"
}
@test "an allow-listed label does NOT page" {
  run_alerter "{\"name\":\"$NAME\",\"action\":\"added\",\"counter\":1,\"columns\":{\"label\":\"com.docker.vmnetd\",\"path\":\"$HOME/Library/LaunchAgents/com.docker.vmnetd.plist\",\"program\":\"/x\"}}"
  assert_no_page
}
@test "a NEW system LaunchDaemon label pages even if it were allow-listed (daemons aren't label-allowlistable)" {
  echo "com.evil.daemon" >> "$FIX/.config/osquery/page-launchd-allowlist.txt"
  run_alerter "{\"name\":\"$NAME\",\"action\":\"added\",\"counter\":1,\"columns\":{\"label\":\"com.evil.daemon\",\"path\":\"/Library/LaunchDaemons/com.evil.daemon.plist\",\"program\":\"/x\"}}"
  assert_page_has "com.evil.daemon"
}
@test "a NEW Apple /System LaunchDaemon (OS update) does not page" {
  run_alerter "{\"name\":\"$NAME\",\"action\":\"added\",\"counter\":1,\"columns\":{\"label\":\"com.apple.newthing\",\"path\":\"/System/Library/LaunchDaemons/com.apple.newthing.plist\",\"program\":\"/x\"}}"
  assert_no_page
}
@test "a REMOVED label (uninstall) does not page" {
  run_alerter "{\"name\":\"$NAME\",\"action\":\"removed\",\"counter\":1,\"columns\":{\"label\":\"com.gone.job\",\"path\":\"$HOME/Library/LaunchAgents/com.gone.job.plist\",\"program\":\"/x\"}}"
  assert_no_page
}
```

- [ ] **Step 4:** the gate arm is already in Task 1 Step 3b. `just test` → these pass.
- [ ] **Step 5: render** — add `persistence_launchd` branches: header "New startup item (launchd)"; fields
  `- **Label:** <label>` · `- **Command:** <program_arguments>` (`program` is empty for real jobs) · `- **Path:** <path>` · the signing verdict as a
  `- **Signing:** …` line when present (text only); nextstep "Did you set this up? If not, a login/boot job
  was planted — remove the plist. To silence a known-good job, reply `allow <label>` in the channel."
- [ ] **Step 6:** `just test` → **all PASS.** Commit
  `feat(osquery): launchd persistence pages on a new label (existence-based); signing is enrichment text`.

---

## Task 4 — SSH/sudoers/sshd_config + pipeline-integrity via real-time file_events

**Files:** `osquery.conf` (add `file_paths` categories), the alerter (render). Test:
`test/osquery-alerter/test_fileevents.bats`. The Task-1 gate arm already pages these categories on
CREATED/UPDATED; this task wires the watches + render. Restores decision #1/#4 (catches a sub-hour
tamper-then-revert the hourly table would miss) and closes the "watch the watchers" gap.

- [ ] **Step 1: Config** — in `osquery.conf` `file_paths`, ADD (sudoers/sshd_config already there; note
  `authorized_keys` is currently only under `file_paths_hashes`, so it emits no event — add it here):

```json
    "authorized_keys": [
      "{{ .chezmoi.homeDir }}/.ssh/authorized_keys",
      "{{ .chezmoi.homeDir }}/.ssh/authorized_keys2",
      "/Users/%/.ssh/authorized_keys"
    ],
    "pipeline_integrity": [
      "{{ .chezmoi.homeDir }}/.local/bin/osquery-results-alerter.sh",
      "{{ .chezmoi.homeDir }}/.local/bin/osquery-alert-dispatch.sh",
      "{{ .chezmoi.homeDir }}/.local/bin/osquery-enrich-finding.sh",
      "{{ .chezmoi.homeDir }}/.local/bin/osquery-firewall-gatekeeper-monitor.sh",
      "{{ .chezmoi.homeDir }}/.local/bin/osquery-uptime-watchdog.sh",
      "{{ .chezmoi.homeDir }}/.local/bin/osquery-heartbeat.sh",
      "{{ .chezmoi.homeDir }}/Library/LaunchAgents/com.webdavis.osquery-%%.plist"
    ]
```

- [ ] **Step 2: Failing tests** (the alerter derives the real action from `.columns.action`):

```bash
setup() { load lib; setup_fixture; }
teardown() { teardown_fixture; }
FE() { printf '{"name":"file_events_recent","action":"added","counter":1,"columns":{"category":"%s","target_path":"%s","action":"%s"}}' "$1" "$2" "$3"; }

@test "authorized_keys CREATED pages" { run_alerter "$(FE authorized_keys /Users/stephen/.ssh/authorized_keys CREATED)"; assert_page_has "authorized_keys"; }
@test "sudoers UPDATED pages"        { run_alerter "$(FE sudoers /etc/sudoers UPDATED)";                          assert_page_has "sudoers"; }
@test "pipeline_integrity UPDATED pages (watch the watchers)" { run_alerter "$(FE pipeline_integrity /Users/stephen/.local/bin/osquery-alert-dispatch.sh UPDATED)"; assert_page_has "osquery-alert-dispatch"; }
@test "authorized_keys DELETED does NOT page (a revert/your own removal stays quiet)" { run_alerter "$(FE authorized_keys /Users/stephen/.ssh/authorized_keys DELETED)"; assert_no_page; }
@test "a launch_agents file_event does NOT page (launchd existence covers persistence)" { run_alerter "$(FE launch_agents /Users/stephen/Library/LaunchAgents/x.plist CREATED)"; assert_no_page; }
```

- [ ] **Step 3:** `just test` → the page cases FAIL (gate not yet matching `$cat`/`$fa` — confirm Task-1's
  arm and that `cat`/`.cols.action` are read). Make them pass via the Task-1 file_events arm.
- [ ] **Step 4: render** — `file_events_recent` header by category: `authorized_keys`→"SSH authorized_keys
  changed", `sudoers`→"sudoers changed", `sshd_config`→"sshd_config changed", `pipeline_integrity`→"Security
  tooling changed". fields `- **File:** <target_path>` · `- **Action:** <action>`. nextstep: for
  authorized_keys "A key was written — if you didn't add it, an SSH key was planted (possibly already
  removed). Inspect and rotate."; for pipeline_integrity "Your osquery alerting code/launchd plist changed —
  if you didn't run `chezmoi apply`, the detector itself was tampered with." **Enrichment lookup** (optional,
  text only): on an authorized_keys page, join the `authorized_keys` table to show username/algorithm/key_file
  — **never** the key or sha256.
- [ ] **Step 5:** `just test` → **all PASS.** Commit
  `feat(osquery): page on real-time file_events for authorized_keys/sudoers/sshd_config + the pipeline's own files`.

> Residual (documented, not silently dropped): an attacker who modifies the alerter/dispatch script *before*
> osquery's next event fires can blind the pipeline — the irreducible "who watches the watchers" limit. It
> belongs to the deferred **off-host** consumer (homelab/Wazuh), same as cross-host machine-death.

---

## Task 5 — New admin / new user account

**Files:** `osquery.conf` (query), the alerter (render). Test: `test/osquery-alerter/test_admin.bats`.

- [ ] **Step 1: Config** — `new_admin_user` (validated: `groupname='admin'`, baseline root+stephen):

```json
"new_admin_user": {
  "query": "SELECT u.username, u.uid FROM users u JOIN user_groups ug ON u.uid=ug.uid JOIN groups g ON ug.gid=g.gid WHERE g.groupname='admin';",
  "interval": 3600, "removed": false, "platform": "darwin"
}
```

- [ ] **Step 2: Failing test:** `{username:backdoor,uid:503}` `"counter":1` → `assert_page_has backdoor`
  (the Task-1 gate already forces `new_admin_user` CRIT; this asserts the render).
- [ ] **Step 3: render** — header "New admin account"; fields `- **User:** <username>` · `- **UID:** <uid>`;
  nextstep "Did you create this account? If not, disable it now."
- [ ] **Step 4:** `just test` → PASS. Commit `feat(osquery): new admin account page detector`.

---

## Task 6 — New kernel / system extension (promote to PAGE)

**Files:** `packs/intrusion-detection.conf` (the queries), the alerter (render). Test:
`test/osquery-alerter/test_extensions.bats`. Verified live: 1 non-Apple kext, 6 stable user sysexts → a NEW
third-party extension is rare and boot-persistent. Key on a **new identifier**, filter Apple, and filter
sysext state to dodge the Tailscale-upgrade `terminated_waiting_to_uninstall` churn.

- [ ] **Step 1: Config** — make the pack queries differential on the identity column, Apple-filtered:

```json
"kernel_extensions_new": {
  "query": "SELECT name, version FROM kernel_extensions WHERE name NOT LIKE 'com.apple.%';",
  "interval": 3600, "removed": false, "platform": "darwin"
},
"system_extensions_new": {
  "query": "SELECT identifier, team FROM system_extensions WHERE state='activated_enabled' AND identifier NOT LIKE 'com.apple.%';",
  "interval": 3600, "removed": false, "platform": "darwin"
}
```

- [ ] **Step 2: Failing tests** (gate already forces these CRIT — Task 1):

```bash
@test "a new system extension pages" {
  run_alerter '{"name":"pack_intrusion-detection_system_extensions_new","action":"added","counter":1,"columns":{"identifier":"io.evil.netext","team":"XXXX"}}'
  assert_page_has "io.evil.netext"
}
@test "a new kernel extension pages" {
  run_alerter '{"name":"pack_intrusion-detection_kernel_extensions_new","action":"added","counter":1,"columns":{"name":"com.evil.kext","version":"1"}}'
  assert_page_has "com.evil.kext"
}
```

- [ ] **Step 3: render** — header "New system extension" / "New kernel extension"; fields identifier/team
  or name/version; nextstep "Did you install this? If not, **remove it** — an extension loads at boot and
  can intercept traffic. System Settings → General → Login Items & Extensions."
- [ ] **Step 4:** `just test` → PASS. Commit `feat(osquery): page on a new third-party kext/sysext`.

---

## Task 7 — Agent surface: auth-file FIM, binary integrity, network exposure

**Files:** `osquery.conf` (queries), the alerter (render). Test: `test/osquery-alerter/test_agent.bats`.
All three are forced CRIT by the Task-1 gate; this task wires the queries (with explicit intervals — the
sibling-detector gap) + render.

- [ ] **Step 1: Config** — three scheduled queries:

```json
"agent_authfile_changed": {
  "query": "SELECT path, sha256 FROM hash WHERE path IN ('/Users/stephen/.config/osquery/webhook-secret','/Users/stephen/.paseo/daemon-keypair.json','/Users/stephen/.paseo/cli-client-id','/Users/stephen/.hermes/.env','/Users/stephen/.codex/config.toml');",
  "interval": 600, "removed": false, "platform": "darwin"
},
"agent_binary_changed": {
  "query": "SELECT path, sha256 FROM hash WHERE path IN ('/Users/stephen/.local/paseo-cli/bin/paseo','/Users/stephen/.local/bin/claude-restart.sh','/opt/homebrew/lib/node_modules/@openai/codex/vendor/aarch64-apple-darwin/bin/codex');",
  "interval": 3600, "removed": false, "platform": "darwin"
},
"agent_exposure_changed": {
  "query": "SELECT lp.address, lp.port, p.name FROM listening_ports lp JOIN processes p ON lp.pid=p.pid WHERE lp.port IN (8644,8181) AND lp.address NOT IN ('127.0.0.1','::1');",
  "interval": 600, "removed": false, "platform": "darwin"
}
```

  **`agent_binary_changed` watches the RESOLVED native binaries, not launcher symlinks** — `/opt/homebrew/bin/codex`
  is a wrapper to `codex.js` (which spawns the vendored aarch64 binary above); `~/.hermes/hermes-agent/venv/bin/python`
  is a uv-managed CPython symlink that proves nothing. **Hermes's editable source tree (7,600+ files) cannot
  be hash-attested** — leave it as a stated gap (move Hermes to a pinned wheel later if integrity is wanted),
  do NOT add a noisy per-file hash. `paseo`/`claude-restart.sh` ARE small executed launcher code → keep them.

- [ ] **Step 2: Failing tests** — `agent_authfile_changed` `{path:.../daemon-keypair.json,sha256:abc}` →
  `assert_page_has daemon-keypair.json` + `assert_page_lacks abc`; `agent_binary_changed`
  `{path:.../bin/codex,sha256:def}` → `assert_page_has codex`; `agent_exposure_changed`
  `{address:0.0.0.0,port:8644,name:python}` → `assert_page_has 8644` (all `"counter":1`).
- [ ] **Step 3: render** — authfile "Agent credential changed" / `- **File:** <path>` (**never sha256**) /
  "If you didn't change this, an agent's auth was tampered with."; binary "Agent binary changed" / `- **File:** <path>`
  / "If you didn't update this agent, it may be impersonated."; exposure "Agent port exposed" /
  `- **Bind:** <address>:<port>` · `- **Process:** <name>` / "An agent gateway is reachable off-host — investigate."
- [ ] **Step 4:** `just test` → PASS. Commit `feat(osquery): agent auth-file FIM + binary integrity + exposure detectors`.

---

## Task 8 — Durable delivery spool (a dropped page is never silently lost)

**Files:** `executable_osquery-alert-dispatch.sh`, `executable_osquery-uptime-watchdog.sh`. Test:
`test/osquery-alerter/test_spool.bats`. The alerter advances its byte-offset before dispatch and `send_alert`
is fire-and-forget (3 retries → `return 0`), so a page that dies on a transient 429/5xx is gone on a headless
box. Keep the offset-before-notify ordering (it prevents whole-batch re-fire); add a spool.

- [ ] **Step 1:** in `send_alert`, replace the final `_osquery_log ERROR; return 0` (after the retry loop)
  with a spool append (atomic tmp+mv, dir 700 / file 600):

```bash
  local spool="$HOME/.local/state/osquery-alert-spool"
  mkdir -p "$spool" 2>/dev/null; chmod 700 "$spool" 2>/dev/null
  local now; now=$(date -u +%s)
  printf '%s\t%s\t%s\t%s\n' "$now" "$reqid" "$url" "$(printf '%s' "$body" | base64)" \
    >"$spool/$reqid.tmp" 2>/dev/null && mv -f "$spool/$reqid.tmp" "$spool/$reqid" 2>/dev/null
  chmod 600 "$spool/$reqid" 2>/dev/null
  _osquery_log "SPOOLED undelivered page reqid=$reqid"
  return 0
```

- [ ] **Step 2:** add `_drain_spool()` to the dispatch helper — re-POST each entry with its SAME body/sig/reqid
  (idempotent at the gateway ≤1h via `X-Request-ID`); `rm` on 2xx; discard entries older than ~55 min
  (past the dedup window a replay is a fresh, correct notify, not spam):

```bash
_drain_spool() {
  local spool="$HOME/.local/state/osquery-alert-spool" f ts reqid url b64 body sig http now
  [ -d "$spool" ] || return 0
  now=$(date -u +%s)
  for f in "$spool"/*; do
    [ -e "$f" ] || continue
    IFS=$'\t' read -r ts reqid url b64 <"$f" || { rm -f "$f"; continue; }
    if [ $((now - ts)) -gt 3300 ]; then rm -f "$f"; continue; fi
    body=$(printf '%s' "$b64" | base64 -d 2>/dev/null) || { rm -f "$f"; continue; }
    sig=$(printf '%s' "$body" | openssl dgst -sha256 -hmac "$(cat "$OSQUERY_WEBHOOK_SECRET_FILE" 2>/dev/null)" | awk '{print $NF}')
    http=$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 -X POST "$url" \
      -H 'Content-Type: application/json' -H "X-Webhook-Signature: $sig" -H "X-Request-ID: $reqid" --data "$body") || http=000
    case "$http" in 2*) rm -f "$f" ;; esac
  done
}
```

- [ ] **Step 3:** call `_drain_spool` at the **alerter start** (before reading the offset) AND at the
  **watchdog start**. Because the alerter is WatchPaths-edge-triggered (may not run for hours on a quiet
  host), the 15-min watchdog is what guarantees a spooled page drains.
- [ ] **Step 4:** tighten the watchdog: if the spool holds any entry older than one tick (15 min),
  `send_alert CRIT "osquery: N undelivered alert(s)" …`.
- [ ] **Step 5: Failing test** (curl stub: 503 thrice → a spool file appears; 200 on drain → it's deleted):
  drive `send_alert` with a `curl` stub forced to 503 and assert one file in `…/osquery-alert-spool/`; run
  `_drain_spool` with curl → 200 and assert the file is gone. (Source the real dispatch helper with
  `OSQUERY_WEBHOOK_SECRET` set and `curl`/`openssl` shimmed on `PATH`.)
- [ ] **Step 6:** `just test` → PASS. Commit `feat(osquery): durable delivery spool drained by the watchdog`.

---

## Task 9 — Daily heartbeat (survives reboot) + watchdog guards it

**Files:** `executable_osquery-heartbeat.sh`, the plist `.tmpl`, the loader, the watchdog. Test:
`test/osquery-alerter/test_heartbeat.bats`.

- [ ] **Step 1: Failing test:**

```bash
setup() { load lib; setup_fixture; cp "$(git rev-parse --show-toplevel)/dot_local/bin/executable_osquery-heartbeat.sh" "$FIX/.local/bin/osquery-heartbeat.sh" 2>/dev/null || true; }
teardown() { teardown_fixture; }
@test "heartbeat posts exactly one healthy beat" {
  [ -f "$FIX/.local/bin/osquery-heartbeat.sh" ]
  HOME="$FIX" bash "$FIX/.local/bin/osquery-heartbeat.sh"
  [ "$(dispatch_log | grep -c healthy)" -eq 1 ]
}
```

- [ ] **Step 2:** write `executable_osquery-heartbeat.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail
source "$HOME/.local/bin/osquery-alert-dispatch.sh"
send_alert CRIT "✅ osquery pipeline healthy" "Nothing to report — $(date -u +%Y-%m-%dT%H:%M:%SZ)" ""
```

- [ ] **Step 3:** write the plist `.tmpl` with **`RunAtLoad=true`** so a boot/reload after 09:00 still emits
  that day's beat (a missing ✅ is the alarm; an extra ✅ on reboot is acceptable — the gateway dedups on
  `sha256(body)` and the UTC timestamp differs, so boot beats are not deduped against the 09:00 beat):

```xml
{{ if eq .chezmoi.os "darwin" -}}
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>com.webdavis.osquery-heartbeat</string>
  <key>ProgramArguments</key><array><string>{{ .chezmoi.homeDir }}/.local/bin/osquery-heartbeat.sh</string></array>
  <key>StartCalendarInterval</key><dict><key>Hour</key><integer>9</integer><key>Minute</key><integer>0</integer></dict>
  <key>RunAtLoad</key><true/>
  <key>StandardErrorPath</key><string>{{ .chezmoi.homeDir }}/.local/log/osquery/heartbeat.err.log</string>
</dict></plist>
{{- end }}
```

  The daily ✅ is an **operator-facing affirmation**; automated pipeline-liveness stays owned by the existing
  15-min uptime-watchdog (the heartbeat does NOT page on its own absence — no central no-beat detector here;
  that would page every roaming laptop overnight and is the deferred homelab layer's job).

- [ ] **Step 4:** write the loader (mirror the watchdog's `run_onchange_after_*`: bootout old label, bootstrap
  new). Label `com.webdavis.osquery-heartbeat`.
- [ ] **Step 5: guard the heartbeat** — append `"com.webdavis.osquery-heartbeat"` to the `AGENTS` array in
  `executable_osquery-uptime-watchdog.sh` so the existing 15-min `launchctl list` loop pages if the heartbeat
  agent (whose silence is the safety signal) is unloaded. Verify with shellcheck + a grep assertion that all
  agent labels are present in `AGENTS` (no new bats scaffolding — there is no watchdog harness).
- [ ] **Step 6:** `just test` → PASS. Commit `feat(osquery): daily heartbeat (RunAtLoad) + watchdog guards it`.

---

## Task 10 — Hermes `allow <label>`-reply plugin (owner + channel guarded)

**Files:** `dot_hermes/plugins/osquery-allowlist/{plugin.yaml,__init__.py}`, `test/hermes-allowlist/test_handler.py`,
`justfile` (extend the test recipe). The allowlist is keyed on the **launchd label**.

- [ ] **Step 1:** extend the `just test` recipe in place to add the pytest leg (now that the plugin tests exist):

```make
test:
    nix develop .#run --command bash -c 'bats test/osquery-alerter && pytest -q test/hermes-allowlist'
```

- [ ] **Step 2: Failing pytest** (`test/hermes-allowlist/test_handler.py`):

```python
import types, importlib.util, pathlib
P = pathlib.Path(__file__).resolve().parents[1].parent / "dot_hermes/plugins/osquery-allowlist/__init__.py"
spec = importlib.util.spec_from_file_location("h", P); mod = importlib.util.module_from_spec(spec); spec.loader.exec_module(mod)

def _evt(text, uid, chat="CHAN"):
    return types.SimpleNamespace(text=text, message_id="m",
        source=types.SimpleNamespace(user_id=uid, platform="discord", chat_id=chat))
def _env(mp, f): mp.setenv("OSQUERY_LAUNCHD_ALLOWLIST_FILE", str(f)); mp.setenv("OSQUERY_ALLOWLIST_OWNER","OWNER"); mp.setenv("OSQUERY_ALLOWLIST_CHANNEL","CHAN")

def test_owner_in_channel_writes_label_and_skips(tmp_path, monkeypatch):
    f=tmp_path/"a.txt"; _env(monkeypatch,f)
    assert mod.osquery_allowlist(_evt("allow com.foo.bar","OWNER")) == {"action":"skip","reason":"osquery-allowlist"}
    assert f.read_text().strip() == "com.foo.bar"
def test_wrong_channel_ignored(tmp_path, monkeypatch):
    f=tmp_path/"a.txt"; _env(monkeypatch,f)
    assert mod.osquery_allowlist(_evt("allow com.x","OWNER",chat="OTHER")) is None and not f.exists()
def test_non_owner_ignored(tmp_path, monkeypatch):
    f=tmp_path/"a.txt"; _env(monkeypatch,f)
    assert mod.osquery_allowlist(_evt("allow com.x","ATTACKER")) is None and not f.exists()
def test_non_label_passes_through(tmp_path, monkeypatch):
    f=tmp_path/"a.txt"; _env(monkeypatch,f)
    assert mod.osquery_allowlist(_evt("allow me to explain","OWNER")) is None
```

- [ ] **Step 3:** `just test` → pytest FAILs. Implement `__init__.py`:

```python
"""Hermes plugin: in the security channel, reply `allow <label>` to allow-list a launchd label."""
import os, re, pathlib
_ALLOW = re.compile(r"^\s*allow\s+([A-Za-z0-9][A-Za-z0-9._-]+)\s*$", re.I)   # a launchd label token

def _path():
    return pathlib.Path(os.environ.get("OSQUERY_LAUNCHD_ALLOWLIST_FILE",
        str(pathlib.Path.home() / ".config" / "osquery" / "page-launchd-allowlist.txt")))

def osquery_allowlist(event, gateway=None, **kwargs):
    m = _ALLOW.match(getattr(event, "text", "") or "")
    if not m: return None
    owner = os.environ.get("OSQUERY_ALLOWLIST_OWNER", ""); chan = os.environ.get("OSQUERY_ALLOWLIST_CHANNEL", "")
    s = getattr(event, "source", None)
    if not owner or not chan or str(getattr(s,"user_id","")) != owner or str(getattr(s,"chat_id","")) != chan:
        return None                                   # fail closed: owner AND channel must match
    label = m.group(1)
    p = _path(); p.parent.mkdir(parents=True, exist_ok=True)
    if label not in (p.read_text().splitlines() if p.exists() else []):
        with open(p, "a") as fh: fh.write(label + "\n")
    if gateway is not None:
        try: gateway.adapters[s.platform].send(s.chat_id, f"✅ allow-listed `{label}`")
        except Exception: pass
    return {"action": "skip", "reason": "osquery-allowlist"}

def register(ctx):
    ctx.register_hook("pre_gateway_dispatch", osquery_allowlist)
```

- [ ] **Step 4:** `just test` → PASS. Add `plugin.yaml` (`name: osquery-allowlist` / `description: …`).
  **Verify the required `plugin.yaml` fields** against `~/.hermes/hermes-agent/website/docs/user-guide/features/plugins.md`.
- [ ] **Step 5:** on Dresden: `hermes plugins list | grep osquery-allowlist`; reply `allow com.test.x` in the
  security channel → ✅ + label appended; reply from another channel → nothing. Commit.

---

## Task 11 — chezmoi: plugin + secret + .env + config.yaml (`create_` template)

**Decision (research note `…/2026-06-09-hermes-config-yaml-tracking-research.md`):** secrets in `~/.hermes/.env`,
non-secret settings in `config.yaml` via `${VAR}` is Hermes's own convention; `config.yaml` is runtime-mutated
(bug #4775 can resolve `${VAR}`→plaintext on rewrite), so track it as a chezmoi **`create_` template** (written
once, never re-synced).

- [ ] **Step 1 — store the webhook secret in KeePassXC.** Verified 2026-06-09 (sha256 `d6312715…` both sides):
  `~/.config/osquery/webhook-secret` and both config.yaml route secrets are **already identical** — no
  reconciliation needed. Store the value once as `Hermes :: Webhook :: osquery` (Password). *(Stephen rotates
  all secrets after the feature ships; rotation = update KeePassXC → `chezmoi apply` interactively → restart Hermes.)*
- [ ] **Step 2 — osquery side.** `dot_config/osquery/webhook-secret.tmpl`:

```gotemplate
{{ (keepassxc "Hermes :: Webhook :: osquery").Password }}
```

- [ ] **Step 3 — `.env` (`dot_hermes/private_dot_env.tmpl` → `~/.hermes/.env`, 0600).** The complete live `.env`,
  carried verbatim with five secrets → KeePassXC:

```gotemplate
# --- non-secret settings (verbatim from the live .env) ---
TERMINAL_MODAL_IMAGE=nikolaik/python-nodejs:python3.11-nodejs20
TERMINAL_TIMEOUT=60
TERMINAL_LIFETIME_SECONDS=300
BROWSERBASE_PROXIES=true
BROWSERBASE_ADVANCED_STEALTH=false
BROWSER_SESSION_TIMEOUT=300
BROWSER_INACTIVITY_TIMEOUT=120
WEB_TOOLS_DEBUG=false
VISION_TOOLS_DEBUG=false
MOA_TOOLS_DEBUG=false
IMAGE_TOOLS_DEBUG=false
AGENT_BROWSER_EXECUTABLE_PATH=/Applications/Google Chrome.app/Contents/MacOS/Google Chrome
MESSAGING_CWD=/Users/stephen/workspaces/webdavis/uriel/agents/bob/workspace
DISCORD_ALLOWED_USERS=864174737491886090
DISCORD_HOME_CHANNEL=1484294029607833603
DISCORD_OSQUERY_CHANNEL=1510379180678975638
HASS_URL=http://homeassistant.local:8123
WEBHOOK_ENABLED=true
WEBHOOK_PORT=8644
ANTHROPIC_API_KEY=

# --- secrets (KeePassXC) ---
ELEVENLABS_API_KEY={{ (keepassxc "Hermes :: ElevenLabs :: API Key").Password }}
DISCORD_BOT_TOKEN={{ (keepassxc "Hermes :: Discord :: Bot Token").Password }}
OPENROUTER_API_KEY={{ (keepassxc "Hermes :: OpenRouter :: API Key").Password }}
TAVILY_API_KEY={{ (keepassxc "Hermes :: Tavily :: API Key").Password }}
ANTHROPIC_TOKEN={{ (keepassxc "Hermes :: Anthropic :: OAuth Token").Password }}
OSQUERY_WEBHOOK_SECRET={{ (keepassxc "Hermes :: Webhook :: osquery").Password }}

# --- osquery-allowlist plugin ---
OSQUERY_ALLOWLIST_OWNER=864174737491886090
OSQUERY_ALLOWLIST_CHANNEL=1511155844559933543
```

  `OSQUERY_ALLOWLIST_CHANNEL` is the **priority** channel (`1511155844559933543`, the `osquery-priority`
  route's `chat_id`) — where pages land and where `allow <label>` happens. `ANTHROPIC_API_KEY` stays empty
  (codex pro subscription; `ANTHROPIC_TOKEN` is a Claude OAuth token).
- [ ] **Step 3b — config.yaml as a seed-once `create_` template.** Create `dot_hermes/create_private_config.yaml.tmpl`
  from the live `~/.hermes/config.yaml` with the two route secrets → `${OSQUERY_WEBHOOK_SECRET}` (the
  `auxiliary.*.api_key` fields are empty; leave them). `create_` writes it only if `~/.hermes/config.yaml` is
  absent and never overwrites — Hermes keeps runtime ownership. Confirm Hermes still authenticates (POST →
  non-401). **Never `chezmoi add` the live config.yaml afterward** (#4775 plaintext risk).
- [ ] **Step 4 — plugin is a managed target, not an "add".** `chezmoi diff dot_hermes/plugins/` shows only the
  new dir added, nothing removed.
- [ ] **Step 5 — leak gate (value-based).** Before committing:

```bash
chezmoi execute-template < dot_hermes/private_dot_env.tmpl > /tmp/env.rendered
git add dot_hermes/private_dot_env.tmpl dot_hermes/create_private_config.yaml.tmpl dot_config/osquery/webhook-secret.tmpl
grep -nE '(KEY|TOKEN|SECRET|PASSWORD)[A-Z_]*=.+' dot_hermes/private_dot_env.tmpl | grep -v '{{' \
  && echo "LEAK: literal secret in .env.tmpl" || echo "source clean"
grep -nE '(secret|api_key|token|password):' dot_hermes/create_private_config.yaml.tmpl | grep -vE '\$\{|\{\{' \
  && echo "LEAK: inline secret in config template" || echo "config template clean"
while IFS='=' read -r k v; do [ -n "$v" ] && git grep -qF -- "$v" -- ':!*/private_dot_env.tmpl' && echo "LEAK $k"; done < /tmp/env.rendered
rm -f /tmp/env.rendered
```

  Must print `source clean` + `config template clean` and no `LEAK`. Then commit
  `feat(hermes): osquery-allowlist plugin + .env/config.yaml secrets via KeePassXC`.

---

## Task 12 — Lint, full suite, single-host calibration

- [ ] `nix develop .#run --command ./scripts/lint.sh` → green (shellcheck/shfmt the alerter/dispatch/watchdog/heartbeat;
  `chezmoi execute-template '{{ includeTemplate "osquery/osquery.conf" . }}' | jq empty`; same for the pack).
- [ ] `just test` → all bats + the plugin pytest green; **no previously-green test is red.**
- [ ] Apply on **Dresden only** (KeePassXC terminal). One-week calibration: the counter==0 first run is the
  discarded baseline (wipes all existing launchd labels); each legit new label that pages → reply `allow <label>`
  in the priority channel. Confirm a quiet channel + the daily ✅. **No fleet rollout** — multi-host is the
  deferred homelab migration. Open the PR.

---

## Self-review (master-spec-v1 §PAGE-set coverage + the 12 senior-review findings + the 3-lens sweep)

**Spec coverage (master-spec §PAGE set → task):** 1 launchd → Task 3; 2 security-config & pipeline files →
Task 4; 3 new admin → Task 5; 4 suid-root → Task 1 gate; 5 kext/sysext → Task 6; 6 agent surface → Task 7;
7 protections-off → Task 1 gate + the 60s poller. Delivery/`host` → Task 1; spool → Task 8; heartbeat →
Task 9; allowlist plugin → Task 10; secrets/chezmoi → Task 11. **Every PAGE-set detector has a task, and no
task pages anything outside the PAGE set.**

- **launchd gate defeatable** → Task 1+3 replace it with existence-on-label; bypass tests (interpreter/Dev-ID
  can't fake a non-existent label) in test_launchd. ✓
- **fleet → dead loopback** → single-host scope (Goal/Task 12); multi-host deferred to homelab. ✓
- **`just test` red Tasks 0–8** → Task 0 bats-only + pytest in the flake; Task 10 extends to both legs. ✓
- **no hostname** → Task 1 Step 4 injects `host` in the signed body. ✓
- **silent at-most-once delivery** → Task 8 spool drained by the watchdog. ✓
- **transient SSH tamper-revert** → Task 4 real-time file_events (CREATED/UPDATED, not DELETE). ✓
- **heartbeat drops on reboot** → Task 9 `RunAtLoad=true`; watchdog guards the agent. ✓
- **hash-watch covers launcher not code** → Task 7 resolved native binary; Hermes editable-tree gap stated. ✓
- **Task 7 missing intervals** → 3600 / 600 / 600. ✓
- **invalid fixture plists** → mooted (launchd tests key on label/path/action, not plist contents). ✓
- **watchdog doesn't guard heartbeat** → Task 9 Step 5. ✓
- **cross-host machine-death** → spec Deferred (homelab). ✓
- **Sweep:** kext/sysext promoted (Task 6, identifier-keyed, state-filtered); sudoers/sshd_config restored to
  page (Task 4); watch-the-watchers via `pipeline_integrity` file_events (Task 4) with the circularity
  residual documented. ✓
- **Rejected** (kept as-is): an evented admin-group detector — admin has a clean state table; would re-add noise.
- Reuses dispatch/packs/poller/watchdog; the deterministic page path and no-LLM-gating invariants are untouched.

