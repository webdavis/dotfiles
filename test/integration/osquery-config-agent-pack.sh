#!/usr/bin/env bash
#
# The agent attack surface is watched: the rendered config ships the
# agent-attack-surface pack and the root setup installs it. Four queries make
# up the pack: an off-loopback exposure watch over the agent control-plane
# ports and patterns (paging), a content-hash watch over the three NON-secret
# agent config files, a metadata-only watch over the two true secret files (no
# secret digest may ever reach the group-readable results.log), and an honest
# log-only signature watch over the agent CLI binaries.
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

  # agent_authfile_changed: the content-hash watch over the three NON-secret
  # agent config files. The two true secrets must NOT be here: their sha256
  # would land in the group-readable results.log.
  authfile_query="$(query_field agent_authfile_changed .query)"
  [[ -n $authfile_query ]] || fail "agent_authfile_changed: query missing"
  [[ "$(query_field agent_authfile_changed .interval)" == "600" ]] ||
    fail "agent_authfile_changed: expected interval 600"
  for watched_suffix in \
    "/.paseo/cli-client-id" \
    "/.hermes/.env" \
    "/.codex/config.toml"; do
    grep -qF "$watched_suffix" <<<"$authfile_query" ||
      fail "agent_authfile_changed: not content-hashing $watched_suffix"
  done
  for secret_suffix in \
    "/.config/osquery/webhook-secret" \
    "/.paseo/daemon-keypair.json"; do
    grep -qF "$secret_suffix" <<<"$authfile_query" &&
      fail "agent_authfile_changed: must not content-hash the secret $secret_suffix"
  done

  # agent_secretfile_changed: the two true secrets, watched by file-table
  # METADATA only, so no secret digest is ever computed into results.log.
  secretfile_query="$(query_field agent_secretfile_changed .query)"
  [[ -n $secretfile_query ]] || fail "agent_secretfile_changed: query missing"
  [[ "$(query_field agent_secretfile_changed .interval)" == "600" ]] ||
    fail "agent_secretfile_changed: expected interval 600"
  [[ "$(query_field agent_secretfile_changed .platform)" == "darwin" ]] ||
    fail "agent_secretfile_changed: expected platform darwin"
  for secret_suffix in \
    "/.config/osquery/webhook-secret" \
    "/.paseo/daemon-keypair.json"; do
    grep -qF "$secret_suffix" <<<"$secretfile_query" ||
      fail "agent_secretfile_changed: not watching the secret $secret_suffix"
  done
  grep -qF "FROM file " <<<"$secretfile_query" ||
    fail "agent_secretfile_changed: must read the file table (metadata), not a hashing table"
  for metadata_column in size mtime ctime inode; do
    grep -qE "(SELECT|,) ?[a-z, ]*\b$metadata_column\b" <<<"$secretfile_query" ||
      fail "agent_secretfile_changed: lost the $metadata_column metadata column"
  done
  grep -qi "sha256" <<<"$secretfile_query" &&
    fail "agent_secretfile_changed: must not select any content digest (sha256)"
  grep -qiE "FROM hash\b" <<<"$secretfile_query" &&
    fail "agent_secretfile_changed: must not read the hash table"

  # Config-wide: no query anywhere in the rendered conf or this pack may put a
  # secret path and a content digest in the same statement. Each query is one
  # line in jq -r output (SQL strings carry no newlines), so grep-chain counts.
  for rendered in "$conf_json" "$pack_json"; do
    leaky_count="$(jq -r '.. | .query? // empty' <<<"$rendered" |
      grep -E "webhook-secret|daemon-keypair" |
      grep -ciE "sha256|FROM hash" || true)"
    [[ ${leaky_count:-0} -eq 0 ]] ||
      fail "a rendered query pairs a secret path with a content digest ($leaky_count occurrence(s))"
  done

  # And the FIM leg: ~/.config/osquery is event-watched but must NOT be in
  # file_paths_hashes, or file_events rows for webhook-secret would carry a
  # sha256 into results.log through the side door.
  if jq -e '.file_paths_hashes | has("allowlist_file")' <<<"$conf_json" >/dev/null; then
    fail "file_paths_hashes must not hash allowlist_file (webhook-secret digest via file_events)"
  fi
  jq -r '.file_paths_hashes[][]' <<<"$conf_json" | grep -qF "/.config/osquery" &&
    fail "file_paths_hashes must not cover ~/.config/osquery (webhook-secret lives there)"

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
printf 'PASS: the agent-attack-surface pack ships (four queries, secrets metadata-only) and the root setup installs it\n'
