#!/usr/bin/env bash
# The slice-6 acceptance criteria (from the slice-4/5 reviews), driven END-TO-END
# through the REAL entry script (executable_results-alerter.sh). Each test feeds
# synthetic results.log rows under a temp HOME with the real pipeline helpers, a
# recording send_alert spy, a stubbed enricher, and a seeded page-allowlist, then
# asserts the DELIVERED outcome: a CRIT page (and its body), a digest-spool entry,
# or nothing. This is the whole-pipeline regression guard: every criterion is
# also unit-pinned in a helper suite; here it must COMPOSE through the entry.
#
# bashunit SOURCES this file, so it carries no `set -euo pipefail` and no
# executable bit: both belong to a script that runs on its own, and either one
# here would reach into the runner's own shell. The shebang stays for shellcheck
# and for editors, and is never executed. test/validate-tests.sh pins that shape;
# `just test-e2e` runs it.
#
# Every check below is a real bashunit assertion. bashunit runs each test function
# under `set +euo pipefail`, so the bats file's bare `grep -q` helpers and its
# `! grep` refutes would report nothing and pass silently: a `!`-inverted command
# is exempt from errexit even where errexit is on, and a helper that merely
# returns 1 is just as invisible. The page refute keeps its anchored `^CALL`
# pattern (assert_file_not_contains is a fixed-string grep, which cannot express
# an anchor) and reports the offending lines through assert_same.
#
# NOTE ON READING A FAILURE: a bashunit test stops at its first failed assertion,
# so a later assertion in the same test is only reached while the earlier ones
# hold. Each refute below was verified on its own against a broken subject, not
# just behind the positive assertion above it.

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ENTRY="$REPO_ROOT/dot_local/libexec/osquery/executable_results-alerter.sh"
HELPER_SRC="$REPO_ROOT/dot_local/libexec/osquery/results-alerter"

function set_up() {
  HOME_DIR="$(mktemp -d)"
  # Record ownership only after our own mktemp, so tear_down removes this path
  # and never a pre-set or inherited HOME_DIR.
  _CRITERIA_OWNED_DIR="$HOME_DIR"
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

  # The allowlist decides whether an unknown user LaunchAgent pages, so the verdict
  # refuses to suppress unless the root-owned pipeline-integrity manifest vouches
  # for the file it just read. bind_allowlist writes the tuple a real apply's
  # manifest refresh leaves behind, for whichever allowlist a test points the
  # alerter at; the seeded default is bound here so the suppression criteria are
  # exercised in the state production is actually in. The manifest never names the
  # pipeline scripts, so criterion 6 still sees no tuple for them and pages.
  export OSQUERY_PIPELINE_MANIFEST="$HOME/pipeline-known-good.sha256"
  export OSQUERY_PIPELINE_SETTLE_SECONDS=0
  bind_allowlist "$HOME/.config/osquery/page-launchd-allowlist.txt"

  export OSQUERY_RESULTS_LOG="$HOME/.local/log/osquery/osqueryd.results.log"
  export OSQUERY_RESULTS_OFFSET="$HOME/.local/state/osquery-results-offset"
  export OSQUERY_DIGEST_STORE="$HOME/.local/state/osquery-digest-spool/digest.ndjson"
  : >"$OSQUERY_RESULTS_LOG"
}

function tear_down() {
  [[ -n ${_CRITERIA_OWNED_DIR:-} ]] || return 0
  rm -rf "$_CRITERIA_OWNED_DIR"
  unset _CRITERIA_OWNED_DIR
}

# _bless_path <path> - append the path's current tuple to the sandbox manifest.
# An UNPINNED allowlist entry carries no hash of its own, so the manifest is what
# vouches for the bytes at its path. On a real machine that holds because chezmoi
# deploys those own-agent plists and manifests them in the same apply, so the
# fixture blesses them too. Without this an unpinned entry could never suppress
# here, for a reason that has nothing to do with the criterion under test.
_bless_path() {
  local raw mode
  raw=$(stat -c '%a' "$1" 2>/dev/null || stat -f '%OLp' "$1" 2>/dev/null) || return 0
  mode="000$raw"
  printf '%s %s %s %s\n' \
    "$(shasum -a 256 "$1" | awk '{print $1}')" "${mode: -4}" "$(id -u)" "$1" \
    >>"$OSQUERY_PIPELINE_MANIFEST"
}

# bind_allowlist <allowlist-file> - make the manifest vouch for this allowlist and
# for every unpinned entry's plist, so allowlist_verdict can genuinely suppress.
bind_allowlist() {
  chmod 600 "$1"
  printf '%s 0600 %s %s\n' \
    "$(shasum -a 256 "$1" | awk '{print $1}')" "$(id -u)" "$1" \
    >"$OSQUERY_PIPELINE_MANIFEST"
  # Every unpinned entry's plist, created if the fixture never made one.
  local plist
  while IFS= read -r plist; do
    [[ -n $plist ]] || continue
    plist="${plist/#\~\//$HOME/}"
    [[ -e $plist ]] || {
      mkdir -p "$(dirname "$plist")"
      printf 'FIXTURE PLIST\n' >"$plist"
    }
    _bless_path "$plist"
  done < <(jq -r 'select((.sha256 // "") == "") | .path' "$1" 2>/dev/null || true)
}

# The cursor's inode field, read the way the entry itself reads it. GNU and BSD
# stat spell the inode under different flags, so `ls -i` is the one call that
# runs on both; the entry carries the same disable, for the same reason, on the
# same call.
# shellcheck disable=SC2012  # a fixed mktemp path this file created; ls -i is safe and portable
log_inode() { ls -i "$OSQUERY_RESULTS_LOG" | awk '{print $1}'; }
seed_cursor() { printf '%s %s\n' "$(log_inode)" "0" >"$OSQUERY_RESULTS_OFFSET"; }
append_row() { printf '%s\n' "$1" >>"$OSQUERY_RESULTS_LOG"; }

# run_entry - run the alerter once and assert it exited 0, the way the bats
# `run bash "$ENTRY"; [ "$status" -eq 0 ]` pair did. The merged output is kept
# only to print it when the status is wrong, which is more than the bats form
# offered: it captured the output and then discarded it.
run_entry() {
  local status=0 output
  output="$(bash "$ENTRY" 2>&1)" || status=$?
  [[ $status -eq 0 ]] || printf 'entry exited %s:\n%s\n' "$status" "$output"
  assert_exit_code 0 "" "$status"
}

# feed <row>...: seed a valid cursor, append the rows, run the entry.
feed() {
  seed_cursor
  local row
  for row in "$@"; do append_row "$row"; done
  run_entry
}

assert_crit_page() { assert_file_contains "$SEND_ALERT_SPY" 'severity=CRIT'; }
# The refute keeps the bats `^CALL` anchor, which a fixed-string assertion cannot
# express, and reports the matching lines as the assertion's "actual" so a failure
# names the page that should not have fired.
assert_no_page() { assert_same '' "$(grep -e '^CALL' "$SEND_ALERT_SPY" || true)"; }
pbody_has() { assert_file_contains "$SEND_ALERT_SPY" "$1"; }
pbody_lacks() { assert_file_not_contains "$SEND_ALERT_SPY" "$1"; }
digest_has() { assert_file_contains "$OSQUERY_DIGEST_STORE" "$1"; }

# --- Criterion 1: new_admin_user added -> a CRIT page ------------------------
function test_c1_new_admin_user_added_fires_a_crit_page() {
  feed '{"name":"new_admin_user","action":"added","columns":{"username":"eve","uid":"501"}}'
  assert_crit_page
  pbody_has 'New administrator account'
}

# --- Criterion 2: differential filevault_off added -> a CRIT page ------------
function test_c2_differential_filevault_off_added_not_snapshot_fires_a_crit_page() {
  feed '{"name":"pack_security-policy-regression_filevault_off","action":"added","columns":{}}'
  assert_crit_page
  pbody_has 'FileVault turned OFF'
}

# --- Criterion 3: agent detectors route to page/page/digest -----------------
function test_c3a_agent_exposure_changed_added_pages() {
  feed '{"name":"pack_agent-attack-surface_agent_exposure_changed","action":"added","columns":{"name":"nc","address":"0.0.0.0","port":"4444"}}'
  assert_crit_page
  pbody_has 'Agent port exposed off-loopback'
}

function test_c3b_agent_secretfile_changed_pages() {
  feed '{"name":"pack_agent-attack-surface_agent_secretfile_changed","action":"added","columns":{"path":"/Users/x/.config/pns/webhook-secret","sha256":"cafebabecafebabecafebabecafebabecafebabecafebabecafebabecafebabe"}}'
  assert_crit_page
  pbody_has 'Agent secret file changed'
}

function test_c3c_agent_authfile_changed_does_not_page_and_lands_in_the_digest_spool() {
  feed '{"name":"pack_agent-attack-surface_agent_authfile_changed","action":"added","columns":{"path":"/Users/x/.codex/config.toml","sha256":"deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"}}'
  assert_no_page
  digest_has 'config.toml'
}

# --- Criterion 4: allowlist end-to-end + enrichment override ----------------
function test_c4a_a_persistence_agent_fully_matching_an_allowlisted_own_agent_tuple_is_suppressed() {
  feed "{\"name\":\"pack_intrusion-detection_persistence_launchd\",\"action\":\"added\",\"columns\":{\"label\":\"com.good\",\"path\":\"$HOME/Library/LaunchAgents/com.good.plist\",\"program\":\"$HOME/bin/good\"}}"
  assert_no_page
}

function test_c4b_the_same_allowlisted_label_with_a_different_program_pages() {
  feed "{\"name\":\"pack_intrusion-detection_persistence_launchd\",\"action\":\"added\",\"columns\":{\"label\":\"com.good\",\"path\":\"$HOME/Library/LaunchAgents/com.good.plist\",\"program\":\"$HOME/bin/EVIL\"}}"
  assert_crit_page
  pbody_has 'New startup item'
}

function test_c4c_an_unknown_user_launch_agent_pages_by_default_deny() {
  feed "{\"name\":\"pack_intrusion-detection_persistence_launchd\",\"action\":\"added\",\"columns\":{\"label\":\"com.unknown\",\"path\":\"$HOME/Library/LaunchAgents/com.unknown.plist\",\"program\":\"$HOME/bin/unknown\"}}"
  assert_crit_page
  pbody_has 'New startup item'
}

function test_c4d_an_allowlisted_but_untrusted_program_pages_because_enrichment_beats_suppression() {
  feed "{\"name\":\"pack_intrusion-detection_persistence_launchd\",\"action\":\"added\",\"columns\":{\"label\":\"com.evil\",\"path\":\"$HOME/Library/LaunchAgents/com.evilUNTRUSTED.plist\",\"program\":\"$HOME/bin/evil\"}}"
  assert_crit_page
  pbody_has 'New startup item'
}

# --- Criterion 5: the allowlist is read from the NEW path, not the old one ---
function test_c5_the_old_flat_launch_allowlist_file_is_not_consulted() {
  # Move the SAME allowlist entries to the OLD flat path only; the new path is
  # empty. A matching agent must PAGE, proving the entry does not read the old path.
  rm -f "$HOME/.config/osquery/page-launchd-allowlist.txt"
  cat >"$HOME/.config/osquery/launch-allowlist.txt" <<EOF
{"label":"com.good","path":"~/Library/LaunchAgents/com.good.plist","program":"~/bin/good","sha256":""}
EOF
  feed "{\"name\":\"pack_intrusion-detection_persistence_launchd\",\"action\":\"added\",\"columns\":{\"label\":\"com.good\",\"path\":\"$HOME/Library/LaunchAgents/com.good.plist\",\"program\":\"$HOME/bin/good\"}}"
  assert_crit_page # not suppressed: the old path is ignored
}

function test_c5b_the_unified_launchd_allowlist_env_var_is_what_the_entry_reads() {
  # Point the env var at a custom file (new path empty); a matching agent is
  # suppressed only if the entry honors the env var.
  rm -f "$HOME/.config/osquery/page-launchd-allowlist.txt"
  local custom="$HOME/custom-allowlist.txt"
  cat >"$custom" <<EOF
{"label":"com.good","path":"~/Library/LaunchAgents/com.good.plist","program":"~/bin/good","sha256":""}
EOF
  export OSQUERY_LAUNCHD_ALLOWLIST="$custom"
  bind_allowlist "$custom" # the verdict only honors an allowlist the manifest vouches for
  feed "{\"name\":\"pack_intrusion-detection_persistence_launchd\",\"action\":\"added\",\"columns\":{\"label\":\"com.good\",\"path\":\"$HOME/Library/LaunchAgents/com.good.plist\",\"program\":\"$HOME/bin/good\"}}"
  assert_no_page # suppressed via the env-var path
}

# --- Criterion 6: a pipeline_integrity change with no manifest -> page -------
function test_c6_a_pipeline_integrity_file_change_with_no_manifest_pages() {
  feed "{\"name\":\"file_events_recent\",\"action\":\"added\",\"columns\":{\"category\":\"pipeline_integrity\",\"target_path\":\"$HOME/.local/libexec/osquery/results-alerter/normalize.sh\",\"sha256\":\"abc\",\"action\":\"UPDATED\"}}"
  assert_crit_page
  pbody_has 'Security tooling changed'
}

# --- Criterion 7: basename-only; no full path, no sha256 in the payload ------
function test_c7_a_paged_agent_secretfile_body_shows_the_basename_only_never_the_path_or_sha256() {
  feed '{"name":"pack_agent-attack-surface_agent_secretfile_changed","action":"added","columns":{"path":"/Users/x/.config/pns/webhook-secret","sha256":"cafebabecafebabecafebabecafebabecafebabecafebabecafebabecafebabe"}}'
  assert_crit_page
  pbody_has 'webhook-secret'         # the basename is present
  pbody_lacks '/Users/x/.config/pns' # the full path is NOT in the payload
  pbody_lacks 'cafebabe'             # the sha256 is NOT in the payload
}
