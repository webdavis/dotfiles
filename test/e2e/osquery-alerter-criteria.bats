#!/usr/bin/env bats
# The slice-6 acceptance criteria (from the slice-4/5 reviews), driven END-TO-END
# through the REAL entry script (executable_results-alerter.sh). Each test feeds
# synthetic results.log rows under a temp HOME with the real pipeline helpers, a
# recording send_alert spy, a stubbed enricher, and a seeded page-allowlist, then
# asserts the DELIVERED outcome: a CRIT page (and its body), a digest-spool entry,
# or nothing. This is the whole-pipeline regression guard - every criterion is
# also unit-pinned in a helper suite; here it must COMPOSE through the entry.

setup() {
  REPO="$BATS_TEST_DIRNAME/../.."
  ENTRY="$REPO/dot_local/libexec/osquery/executable_results-alerter.sh"
  HELPER_SRC="$REPO/dot_local/libexec/osquery/results-alerter"

  HOME_DIR="$(mktemp -d)"
  export HOME="$HOME_DIR"
  mkdir -p "$HOME/.local/libexec/osquery/results-alerter" "$HOME/.local/state" \
    "$HOME/.local/log/osquery" "$HOME/.config/osquery" "$HOME/Library/LaunchAgents" "$HOME/bin"
  cp "$HELPER_SRC"/*.sh "$HOME/.local/libexec/osquery/results-alerter/"

  # Recording send_alert spy (records severity, title, and the full detail/pbody).
  export SEND_ALERT_SPY="$HOME/send_alert.log"
  : >"$SEND_ALERT_SPY"
  cat >"$HOME/.local/libexec/osquery/alert-dispatch.sh" <<'STUB'
# shellcheck shell=bash
send_alert() {
  {
    printf 'CALL\tseverity=%s\ttitle=%s\n' "$1" "$2"
    printf 'DETAIL-START\n%s\nDETAIL-END\n' "$3"
  } >>"$SEND_ALERT_SPY"
  return "${SEND_ALERT_RC:-0}"
}
STUB

  # Stubbed enricher: UNTRUSTED (exit 10) when the inspected path contains
  # UNTRUSTED, else a trusted authority (exit 0).
  cat >"$HOME/enrich-stub.sh" <<'STUB'
#!/usr/bin/env bash
case "$1" in
  *UNTRUSTED*) printf 'UNSIGNED'; exit 10 ;;
  *) printf 'signed: Apple'; exit 0 ;;
esac
STUB
  chmod +x "$HOME/enrich-stub.sh"
  export OSQUERY_ENRICH_SCRIPT="$HOME/enrich-stub.sh"

  # Seeded page-allowlist at the NEW default path with two own-agent entries
  # (empty sha256 -> the hash dimension is skipped). One clean, one whose plist
  # path carries UNTRUSTED so the enrichment override can be exercised.
  cat >"$HOME/.config/osquery/page-launchd-allowlist.txt" <<EOF
{"label":"com.good","path":"~/Library/LaunchAgents/com.good.plist","program":"~/bin/good","sha256":""}
{"label":"com.evil","path":"~/Library/LaunchAgents/com.evilUNTRUSTED.plist","program":"~/bin/evil","sha256":""}
EOF

  export OSQUERY_RESULTS_LOG="$HOME/.local/log/osquery/osqueryd.results.log"
  export OSQUERY_RESULTS_OFFSET="$HOME/.local/state/osquery-results-offset"
  export OSQUERY_DIGEST_STORE="$HOME/.local/state/osquery-digest-spool/digest.ndjson"
  : >"$OSQUERY_RESULTS_LOG"
}

teardown() { rm -rf "$HOME_DIR"; }

log_inode() { ls -i "$OSQUERY_RESULTS_LOG" | awk '{print $1}'; }
seed_cursor() { printf '%s %s\n' "$(log_inode)" "0" >"$OSQUERY_RESULTS_OFFSET"; }
append_row() { printf '%s\n' "$1" >>"$OSQUERY_RESULTS_LOG"; }
run_entry() { run bash "$ENTRY"; [ "$status" -eq 0 ]; }

# feed <row>...: seed a valid cursor, append the rows, run the entry.
feed() {
  seed_cursor
  local row
  for row in "$@"; do append_row "$row"; done
  run_entry
}

assert_crit_page() { grep -q 'severity=CRIT' "$SEND_ALERT_SPY"; }
assert_no_page() { ! grep -q '^CALL' "$SEND_ALERT_SPY"; }
pbody_has() { grep -qF -- "$1" "$SEND_ALERT_SPY"; }
pbody_lacks() { ! grep -qF -- "$1" "$SEND_ALERT_SPY"; }
digest_has() { grep -qF -- "$1" "$OSQUERY_DIGEST_STORE"; }

# --- Criterion 1: new_admin_user added -> a CRIT page ------------------------
@test "C1: new_admin_user added fires a CRIT page" {
  feed '{"name":"new_admin_user","action":"added","columns":{"username":"eve","uid":"501"}}'
  assert_crit_page
  pbody_has 'New administrator account'
}

# --- Criterion 2: differential filevault_off added -> a CRIT page ------------
@test "C2: differential filevault_off added (not snapshot) fires a CRIT page" {
  feed '{"name":"pack_security-policy-regression_filevault_off","action":"added","columns":{}}'
  assert_crit_page
  pbody_has 'FileVault turned OFF'
}

# --- Criterion 3: agent detectors route to page/page/digest -----------------
@test "C3a: agent_exposure_changed added pages" {
  feed '{"name":"pack_agent-attack-surface_agent_exposure_changed","action":"added","columns":{"name":"nc","address":"0.0.0.0","port":"4444"}}'
  assert_crit_page
  pbody_has 'Agent port exposed off-loopback'
}

@test "C3b: agent_secretfile_changed pages" {
  feed '{"name":"pack_agent-attack-surface_agent_secretfile_changed","action":"added","columns":{"path":"/Users/x/.config/relay/webhook-secret","sha256":"cafebabecafebabecafebabecafebabecafebabecafebabecafebabecafebabe"}}'
  assert_crit_page
  pbody_has 'Agent secret file changed'
}

@test "C3c: agent_authfile_changed (config.toml) does NOT page, lands in the digest spool" {
  feed '{"name":"pack_agent-attack-surface_agent_authfile_changed","action":"added","columns":{"path":"/Users/x/.codex/config.toml","sha256":"deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"}}'
  assert_no_page
  digest_has 'config.toml'
}

# --- Criterion 4: allowlist end-to-end + enrichment override ----------------
@test "C4a: a persistence agent fully matching an allowlisted own-agent tuple is suppressed" {
  feed "{\"name\":\"pack_intrusion-detection_persistence_launchd\",\"action\":\"added\",\"columns\":{\"label\":\"com.good\",\"path\":\"$HOME/Library/LaunchAgents/com.good.plist\",\"program\":\"$HOME/bin/good\"}}"
  assert_no_page
}

@test "C4b: the same allowlisted label with a different program pages (reused label)" {
  feed "{\"name\":\"pack_intrusion-detection_persistence_launchd\",\"action\":\"added\",\"columns\":{\"label\":\"com.good\",\"path\":\"$HOME/Library/LaunchAgents/com.good.plist\",\"program\":\"$HOME/bin/EVIL\"}}"
  assert_crit_page
  pbody_has 'New startup item'
}

@test "C4c: an unknown user LaunchAgent pages (default-deny, operator ruling)" {
  feed "{\"name\":\"pack_intrusion-detection_persistence_launchd\",\"action\":\"added\",\"columns\":{\"label\":\"com.unknown\",\"path\":\"$HOME/Library/LaunchAgents/com.unknown.plist\",\"program\":\"$HOME/bin/unknown\"}}"
  assert_crit_page
  pbody_has 'New startup item'
}

@test "C4d: an allowlisted-but-untrusted program pages (enrichment beats suppression)" {
  feed "{\"name\":\"pack_intrusion-detection_persistence_launchd\",\"action\":\"added\",\"columns\":{\"label\":\"com.evil\",\"path\":\"$HOME/Library/LaunchAgents/com.evilUNTRUSTED.plist\",\"program\":\"$HOME/bin/evil\"}}"
  assert_crit_page
  pbody_has 'New startup item'
}

# --- Criterion 5: the allowlist is read from the NEW path, not the old one ---
@test "C5: the old flat launch-allowlist.txt is NOT consulted (the entry reads the new path/env)" {
  # Move the SAME allowlist entries to the OLD flat path only; the new path is
  # empty. A matching agent must PAGE, proving the entry does not read the old path.
  rm -f "$HOME/.config/osquery/page-launchd-allowlist.txt"
  cat >"$HOME/.config/osquery/launch-allowlist.txt" <<EOF
{"label":"com.good","path":"~/Library/LaunchAgents/com.good.plist","program":"~/bin/good","sha256":""}
EOF
  feed "{\"name\":\"pack_intrusion-detection_persistence_launchd\",\"action\":\"added\",\"columns\":{\"label\":\"com.good\",\"path\":\"$HOME/Library/LaunchAgents/com.good.plist\",\"program\":\"$HOME/bin/good\"}}"
  assert_crit_page   # not suppressed: the old path is ignored
}

@test "C5b: the unified OSQUERY_LAUNCHD_ALLOWLIST env var is what the entry reads" {
  # Point the env var at a custom file (new path empty); a matching agent is
  # suppressed only if the entry honors the env var.
  rm -f "$HOME/.config/osquery/page-launchd-allowlist.txt"
  local custom="$HOME/custom-allowlist.txt"
  cat >"$custom" <<EOF
{"label":"com.good","path":"~/Library/LaunchAgents/com.good.plist","program":"~/bin/good","sha256":""}
EOF
  export OSQUERY_LAUNCHD_ALLOWLIST="$custom"
  feed "{\"name\":\"pack_intrusion-detection_persistence_launchd\",\"action\":\"added\",\"columns\":{\"label\":\"com.good\",\"path\":\"$HOME/Library/LaunchAgents/com.good.plist\",\"program\":\"$HOME/bin/good\"}}"
  assert_no_page   # suppressed via the env-var path
}

# --- Criterion 6: a pipeline_integrity change with no manifest -> page -------
@test "C6: a pipeline_integrity file change with no manifest pages (fail-open)" {
  feed "{\"name\":\"file_events_recent\",\"action\":\"added\",\"columns\":{\"category\":\"pipeline_integrity\",\"target_path\":\"$HOME/.local/libexec/osquery/results-alerter/normalize.sh\",\"sha256\":\"abc\",\"action\":\"UPDATED\"}}"
  assert_crit_page
  pbody_has 'Security tooling changed'
}

# --- Criterion 7: basename-only; no full path, no sha256 in the payload ------
@test "C7: a paged agent_secretfile_changed body shows the basename only, never the path or sha256" {
  feed '{"name":"pack_agent-attack-surface_agent_secretfile_changed","action":"added","columns":{"path":"/Users/x/.config/relay/webhook-secret","sha256":"cafebabecafebabecafebabecafebabecafebabecafebabecafebabecafebabe"}}'
  assert_crit_page
  pbody_has 'webhook-secret'                 # the basename is present
  pbody_lacks '/Users/x/.config/relay'       # the full path is NOT in the payload
  pbody_lacks 'cafebabe'                      # the sha256 is NOT in the payload
}
