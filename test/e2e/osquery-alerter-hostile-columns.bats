#!/usr/bin/env bats
# Security: no attacker-controlled column value can be reinterpreted as record
# structure. The gate must route on the finding's ACTUAL fields, so an embedded
# separator (0x1F), newline, or tab in a crafted .cols.path/label/program can
# never shift field boundaries to make an unknown plist read as an allowlisted
# tuple (which would suppress it). Driven end-to-end through the REAL entry.

setup() {
  REPO="$BATS_TEST_DIRNAME/../.."
  ENTRY="$REPO/dot_local/libexec/osquery/executable_results-alerter.sh"
  HELPER_SRC="$REPO/dot_local/libexec/osquery/results-alerter"

  HOME_DIR="$(mktemp -d)"
  export HOME="$HOME_DIR"
  mkdir -p "$HOME/.local/libexec/osquery/results-alerter" "$HOME/.local/state" \
    "$HOME/.local/log/osquery" "$HOME/.config/osquery" "$HOME/Library/LaunchAgents" "$HOME/bin"
  cp "$HELPER_SRC"/*.sh "$HOME/.local/libexec/osquery/results-alerter/"

  export SEND_ALERT_SPY="$HOME/send_alert.log"
  : >"$SEND_ALERT_SPY"
  cat >"$HOME/.local/libexec/osquery/alert-dispatch.sh" <<'STUB'
# shellcheck shell=bash
send_alert() {
  { printf 'CALL\tseverity=%s\ttitle=%s\n' "$1" "$2"; printf 'DETAIL-START\n%s\nDETAIL-END\n' "$3"; } >>"$SEND_ALERT_SPY"
  return "${SEND_ALERT_RC:-0}"
}
STUB
  # Trusted enricher (so a promotion never masks a suppression bug: the page must
  # come from default-deny, not from enrichment).
  printf '#!/usr/bin/env bash\nprintf %s "signed: Apple"\nexit 0\n' >"$HOME/enrich-stub.sh"
  chmod +x "$HOME/enrich-stub.sh"
  export OSQUERY_ENRICH_SCRIPT="$HOME/enrich-stub.sh"

  # An own-agent allowlist entry the attacker will try to impersonate by injection.
  cat >"$HOME/.config/osquery/page-launchd-allowlist.txt" <<EOF
{"label":"com.good","path":"~/Library/LaunchAgents/com.good.plist","program":"~/bin/good","sha256":""}
EOF

  export OSQUERY_RESULTS_LOG="$HOME/.local/log/osquery/osqueryd.results.log"
  export OSQUERY_RESULTS_OFFSET="$HOME/.local/state/osquery-results-offset"
  : >"$OSQUERY_RESULTS_LOG"
}
teardown() { rm -rf "$HOME_DIR"; }

log_inode() { ls -i "$OSQUERY_RESULTS_LOG" | awk '{print $1}'; }
seed_cursor() { printf '%s 0\n' "$(log_inode)" >"$OSQUERY_RESULTS_OFFSET"; }
run_entry() { run bash "$ENTRY"; [ "$status" -eq 0 ]; }
assert_paged() { grep -q 'severity=CRIT' "$SEND_ALERT_SPY"; }

# HEADLINE: a persistence_launchd finding whose crafted .cols.path embeds a 0x1F
# tuple (path\x1flabel\x1fprogram) matching the allowlisted own-agent - under an
# in-band separator this splits so allowlist_verdict reads (com.good, the-good-
# path, the-good-program) and SUPPRESSES the malicious agent. It must PAGE: the
# real label is com.attacker (unknown) -> default-deny.
@test "HOSTILE-0x1F: a 0x1F-injected path cannot impersonate an allowlisted tuple; the finding pages" {
  seed_cursor
  local good_path="$HOME/Library/LaunchAgents/com.good.plist" good_prog="$HOME/bin/good"
  # .cols.path = "<good_path>com.good<good_prog>" (0x1F as  in JSON).
  printf '{"name":"pack_intrusion-detection_persistence_launchd","action":"added","columns":{"label":"com.attacker","path":"%s\\u001fcom.good\\u001f%s","program":"/attacker/mal"}}\n' \
    "$good_path" "$good_prog" >>"$OSQUERY_RESULTS_LOG"
  run_entry
  assert_paged
}

# A newline embedded in .cols.label must not truncate or split the record; the
# unknown agent still pages (default-deny), never silently lost.
@test "HOSTILE-newline: a newline in a column does not split the record; the finding pages" {
  seed_cursor
  printf '{"name":"pack_intrusion-detection_persistence_launchd","action":"added","columns":{"label":"com.attacker\\ncom.good","path":"%s/Library/LaunchAgents/evil.plist","program":"%s/bin/evil"}}\n' \
    "$HOME" "$HOME" >>"$OSQUERY_RESULTS_LOG"
  run_entry
  assert_paged
}

# A tab embedded in .cols.program must stay an opaque value; the unknown agent pages.
@test "HOSTILE-tab: a tab in a column stays opaque; the finding pages" {
  seed_cursor
  printf '{"name":"pack_intrusion-detection_persistence_launchd","action":"added","columns":{"label":"com.attacker","path":"%s/Library/LaunchAgents/evil.plist","program":"%s/bin/evil\\tcom.good"}}\n' \
    "$HOME" "$HOME" >>"$OSQUERY_RESULTS_LOG"
  run_entry
  assert_paged
}
