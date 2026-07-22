#!/usr/bin/env bash
#
# The agent attack surface is watched: the rendered config ships the
# agent-attack-surface pack and the root setup installs it. Three queries make
# up the pack: an off-loopback exposure watch over the agent control-plane
# ports and patterns (paging), a hash watch over the agent auth/credential
# files, and an honest log-only signature watch over the agent CLI binaries.
# Render-driven: chezmoi renders the templates exactly as at apply time and jq
# asserts the shipped JSON, so a template regression fails here before it
# silently stops the root daemon from loading its config.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." || exit 1 && pwd)"
cd "$REPO_ROOT" || exit 1

if ! command -v chezmoi >/dev/null 2>&1; then
  printf 'SKIP: chezmoi not found (run inside the nix dev shell)\n'
  exit 0
fi

render_home="$(mktemp -d)"
trap 'rm -rf "$render_home"' EXIT
render() { HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$1"; }

fails=0
fail() {
  printf 'FAIL: %s\n' "$*" >&2
  fails=$((fails + 1))
}

PACK_TEMPLATE=".chezmoitemplates/osquery/packs/agent-attack-surface.conf"
CONF_TEMPLATE=".chezmoitemplates/osquery/osquery.conf"
SETUP_SCRIPT=".chezmoiscripts/run_onchange_before_50-setup-osquery.sh.tmpl"

# --- (a) osquery.conf ships the pack in its packs map -------------------------
conf_json="$(render "$CONF_TEMPLATE")" || fail "osquery.conf failed to render"
pack_path="$(jq -r '.packs["agent-attack-surface"] // empty' <<<"$conf_json")"
[[ $pack_path == "/var/osquery/packs/agent-attack-surface.conf" ]] ||
  fail "osquery.conf packs map must point agent-attack-surface at /var/osquery/packs/agent-attack-surface.conf (got '${pack_path:-absent}')"

# --- (b) the pack renders to valid JSON with the three queries ----------------
if [[ ! -f $PACK_TEMPLATE ]]; then
  fail "missing pack template: $PACK_TEMPLATE"
else
  pack_json="$(render "$PACK_TEMPLATE")" || fail "pack template failed to render"
  jq empty <<<"$pack_json" 2>/dev/null || fail "rendered pack is not valid JSON"

  # No em-dash anywhere in the shipped pack (descriptions included).
  if grep -q $'\xe2\x80\x94' <<<"$pack_json"; then
    fail "the rendered pack contains an em-dash"
  fi

  # query_field <name> <jq-path> -- print one field of one pack query.
  query_field() {
    jq -r --arg q "$1" ".queries[\$q]$2 // empty" <<<"$pack_json"
  }

  # agent_exposure_changed: the off-loopback agent exposure watch (pages).
  exposure_query="$(query_field agent_exposure_changed .query)"
  [[ -n $exposure_query ]] || fail "agent_exposure_changed: query missing"
  [[ "$(query_field agent_exposure_changed .interval)" == "600" ]] ||
    fail "agent_exposure_changed: expected interval 600"
  [[ "$(query_field agent_exposure_changed .platform)" == "darwin" ]] ||
    fail "agent_exposure_changed: expected platform darwin"
  # Excludes ONLY real loopback (127.0.0.0/8 and ::1) and the port-0
  # placeholder; link-local (fe80) must NOT be excluded (it is off-box
  # reachable, the FX9 lesson).
  grep -qF "lp.address NOT IN ('127.0.0.1', '::1')" <<<"$exposure_query" ||
    fail "agent_exposure_changed: lost the exact-loopback exclusion"
  grep -qF "NOT LIKE '127.%'" <<<"$exposure_query" ||
    fail "agent_exposure_changed: lost the 127.0.0.0/8 exclusion"
  grep -qF "lp.port != 0" <<<"$exposure_query" ||
    fail "agent_exposure_changed: lost the port-0 placeholder exclusion"
  grep -qiE "fe80" <<<"$exposure_query" &&
    fail "agent_exposure_changed: must not exclude link-local (fe80)"
  # The pattern clauses: MCP by cmdline, hermes by path, and the fixed
  # control-plane/data ports.
  grep -qF "cmdline LIKE '%mcp%'" <<<"$exposure_query" ||
    fail "agent_exposure_changed: lost the MCP cmdline pattern"
  grep -qF "path LIKE '%hermes%'" <<<"$exposure_query" ||
    fail "agent_exposure_changed: lost the hermes path pattern"
  for port in 5432 6767 8644; do
    grep -qE "(\(|, )$port(,|\))" <<<"$exposure_query" ||
      fail "agent_exposure_changed: lost port $port from the port clause"
  done

  # agent_authfile_changed: the auth/credential file hash watch.
  authfile_query="$(query_field agent_authfile_changed .query)"
  [[ -n $authfile_query ]] || fail "agent_authfile_changed: query missing"
  [[ "$(query_field agent_authfile_changed .interval)" == "600" ]] ||
    fail "agent_authfile_changed: expected interval 600"
  for watched_suffix in \
    "/.config/osquery/webhook-secret" \
    "/.paseo/daemon-keypair.json" \
    "/.paseo/cli-client-id" \
    "/.hermes/.env" \
    "/.codex/config.toml"; do
    grep -qF "$watched_suffix" <<<"$authfile_query" ||
      fail "agent_authfile_changed: not watching $watched_suffix"
  done

  # agent_binary_changed: the honest log-only binary watch (signature columns
  # are the real signal; the description carries the honest-coverage limits).
  binary_query="$(query_field agent_binary_changed .query)"
  [[ -n $binary_query ]] || fail "agent_binary_changed: query missing"
  [[ "$(query_field agent_binary_changed .interval)" == "3600" ]] ||
    fail "agent_binary_changed: expected interval 3600"
  for binary in "/opt/homebrew/bin/codex" "/opt/homebrew/bin/paseo"; do
    grep -qF "$binary" <<<"$binary_query" ||
      fail "agent_binary_changed: not watching $binary"
  done
  for signature_column in "s.cdhash" "s.team_identifier" "s.signed"; do
    grep -qF "$signature_column" <<<"$binary_query" ||
      fail "agent_binary_changed: lost signature column $signature_column"
  done
  grep -qF "LEFT JOIN signature" <<<"$binary_query" ||
    fail "agent_binary_changed: lost the LEFT JOIN on signature"
fi

# --- (c) the root setup installs the pack -------------------------------------
grep -qF "install_root /var/osquery/packs/agent-attack-surface.conf" "$SETUP_SCRIPT" ||
  fail "setup script has no install_root block for the pack"
grep -qF 'includeTemplate "osquery/packs/agent-attack-surface.conf"' "$SETUP_SCRIPT" ||
  fail "setup script does not render the pack via includeTemplate"

if ((fails > 0)); then
  printf '%d agent-attack-surface config assertion(s) failed\n' "$fails" >&2
  exit 1
fi
printf 'PASS: the agent-attack-surface pack ships (three queries, honest exclusions) and the root setup installs it\n'
