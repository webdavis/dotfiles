#!/usr/bin/env bash
#
# render_page (results-alerter/render-page.sh) builds the #priority page body from
# the enriched findings: it selects the CRIT ones and renders each into a focused
# block (header + decision-relevant fields + a next-step), returning
# {pcount, pbody}. It is display-only - it never changes detection or severity.
#
# Criterion 7 (basename-only), pinned hard here: a secret/auth-file finding
# (agent_authfile_changed, agent_secretfile_changed) shows ONLY the file's
# basename in the payload, never its full path and never a sha256/digest. The
# digest spool may keep full paths privately (B8), but the notification body -
# which fans out to Discord - must not.
#
# Caps preserved from c69baab: each rendered value is backtick-sanitized and
# capped at 240 chars; the page is capped at 8 blocks (plus an "N more" marker)
# and then hard-capped at 1900 chars, so an over-long page can never wedge the
# 2000-char Discord budget.
#
# Unit test: fixture CRIT findings in, assert the body shape, the basename-only
# privacy, and every cap.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HELPER="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/render-page.sh"

fail() {
  printf 'osquery-render-page: FAIL -- %s\n' "$*" >&2
  exit 1
}

[[ -f $HELPER ]] || fail "missing helper: $HELPER"

# render_page reads enriched-finding NDJSON on stdin and prints the {pcount, pbody}
# JSON object. A fresh subshell keeps the sourcing side-effect-free.
render_page_run() { bash -c "source '$HELPER'; render_page"; }

# --- normal CRIT block + basename-only privacy for secret/auth findings --------
normal_block_and_basename_privacy() {
  local out pbody pcount
  out="$(
    render_page_run <<'EOF'
{"q":"new_admin_user","act":"added","sev":"CRIT","cols":{"username":"eve","uid":"501"},"ep":""}
{"q":"agent_secretfile_changed","act":"added","sev":"CRIT","cols":{"path":"/Users/x/.config/relay/webhook-secret","sha256":"cafebabecafebabecafebabecafebabecafebabecafebabecafebabecafebabe"},"ep":""}
{"q":"agent_authfile_changed","act":"added","sev":"CRIT","cols":{"path":"/Users/x/.codex/config.toml","sha256":"deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"},"ep":""}
EOF
  )"
  pcount="$(jq -r '.pcount' <<<"$out")"
  pbody="$(jq -r '.pbody' <<<"$out")"

  [[ $pcount -eq 3 ]] || fail "expected pcount 3, got $pcount"

  # A normal CRIT finding renders header + fields + next-step.
  [[ $pbody == *"New administrator account"* ]] || fail "normal finding: missing header -- $pbody"
  [[ $pbody == *"**User:**"* && $pbody == *"eve"* ]] || fail "normal finding: missing user field -- $pbody"
  [[ $pbody == *"admin access"* ]] || fail "normal finding: missing next-step -- $pbody"

  # Criterion 7: the secret file shows by BASENAME only. The basename is present;
  # the full path and the sha256 are ABSENT from the page body.
  [[ $pbody == *"webhook-secret"* ]] || fail "secret finding: basename missing -- $pbody"
  [[ $pbody != *"/Users/x/.config/relay"* ]] || fail "criterion 7 VIOLATED: secret full path leaked into pbody -- $pbody"
  [[ $pbody != *"cafebabe"* ]] || fail "criterion 7 VIOLATED: secret sha256 leaked into pbody -- $pbody"

  # Same for the auth-file finding.
  [[ $pbody == *"config.toml"* ]] || fail "auth finding: basename missing -- $pbody"
  [[ $pbody != *"/Users/x/.codex/config.toml"* ]] || fail "criterion 7 VIOLATED: auth full path leaked into pbody -- $pbody"
  [[ $pbody != *"deadbeef"* ]] || fail "criterion 7 VIOLATED: auth sha256 leaked into pbody -- $pbody"
}

# --- 240-char field cap --------------------------------------------------------
field_cap_truncates_an_over_long_value() {
  local long out pbody
  long="$(printf 'a%.0s' {1..300})"
  out="$(printf '{"q":"new_admin_user","act":"added","sev":"CRIT","cols":{"username":"%s","uid":"1"},"ep":""}\n' "$long" | render_page_run)"
  pbody="$(jq -r '.pbody' <<<"$out")"
  [[ $pbody == *"(truncated)"* ]] || fail "field cap: expected a (truncated) marker -- $pbody"
  [[ $pbody != *"$long"* ]] || fail "field cap: the full 300-char value was not truncated -- $pbody"
}

# --- 8-block cap ---------------------------------------------------------------
block_cap_limits_to_eight_blocks() {
  local findings=() i out pcount pbody
  for i in $(seq 1 10); do
    findings+=("{\"q\":\"new_admin_user\",\"act\":\"added\",\"sev\":\"CRIT\",\"cols\":{\"username\":\"user$i\",\"uid\":\"$i\"},\"ep\":\"\"}")
  done
  out="$(printf '%s\n' "${findings[@]}" | render_page_run)"
  pcount="$(jq -r '.pcount' <<<"$out")"
  pbody="$(jq -r '.pbody' <<<"$out")"
  [[ $pcount -eq 10 ]] || fail "block cap: pcount should count ALL crit findings (10), got $pcount"
  [[ $pbody == *"and 2 more CRITICAL finding(s)"* ]] || fail "block cap: expected an 'and 2 more' marker -- $pbody"
  # Exactly 8 rendered header blocks (the 9th and 10th are summarized by the marker).
  local headers
  headers="$(grep -c 'New administrator account' <<<"$pbody" || true)"
  [[ $headers -eq 8 ]] || fail "block cap: expected 8 rendered blocks, got $headers"
}

# --- 1900-char page cap --------------------------------------------------------
page_cap_hard_limits_length() {
  local wide findings=() i out body_len pbody
  wide="$(printf 'b%.0s' {1..240})"
  for i in $(seq 1 8); do
    findings+=("{\"q\":\"new_admin_user\",\"act\":\"added\",\"sev\":\"CRIT\",\"cols\":{\"username\":\"$wide$i\",\"uid\":\"$i\"},\"ep\":\"\"}")
  done
  out="$(printf '%s\n' "${findings[@]}" | render_page_run)"
  pbody="$(jq -r '.pbody' <<<"$out")"
  body_len="$(jq -r '.pbody | length' <<<"$out")"
  [[ $pbody == *"truncated to fit the 2000-char limit"* ]] || fail "page cap: expected the 1900-char final cap marker -- length=$body_len"
  [[ $body_len -lt 2000 ]] || fail "page cap: pbody must be under 2000 chars, got $body_len"
}

normal_block_and_basename_privacy
field_cap_truncates_an_over_long_value
block_cap_limits_to_eight_blocks
page_cap_hard_limits_length

printf 'osquery-render-page: OK (CRIT block header/fields/nextstep; basename-only secret+auth files with no path/sha256; 240-char field cap; 8-block cap; 1900-char page cap)\n'
