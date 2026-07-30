#!/usr/bin/env bash
# macos-defaults-capture.sh, append a live setting to macos_defaults.yaml.
#
# Reads the current value+type via `defaults read-type` + `defaults read`,
# normalizes, appends to the YAML if not already tracked. If the entry is
# already tracked AND the live value matches: no-op (exit 0). If the entry
# is already tracked but the live value DIVERGES: exit 4 (drift), resolve
# via `just defaults-apply` (revert) or hand-edit YAML (capture intent).
#
# Every appended record declares `tier: enforce`: capture exists to track a
# value the operator just set and read back, which is the definition of a
# settable control. A control that belongs to another tier is declared by
# hand-editing the YAML, not through this tool.
#
# Usage: macos-defaults-capture.sh <domain> <key> [--host current] [--scope user|system]
#
# --scope system captures from the record's system plist path
# (/Library/Preferences/<domain>) and appends the record with `scope: system`.
# It cannot be combined with --host current: ByHost storage is per-user.
#
# Exit codes:
#   0: appended, or already in sync
#   1: key not currently set on this Mac
#   2: data file missing or unreadable
#   3: malformed args
#   4: YAML has a different value than disk (drift; resolve before re-running)

set -euo pipefail

# shellcheck source=dot_local/bin/macos-defaults-lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/macos-defaults-lib.sh"

usage() {
  printf 'usage: macos-defaults-capture <domain> <key> [--host current] [--scope user|system]\n' >&2
  exit 3
}

[[ $# -lt 2 || $# -gt 6 ]] && usage

domain="$1"
key="$2"
shift 2

# Optional host argument. Three accepted forms:
#   --host=current  (single token, what the justfile recipe emits)
#   --host current  (two tokens, what a direct CLI invocation might use)
#   (omitted)       (global storage, no -currentHost flag)
# Optional scope argument, same two spellings, defaulting to user. The value
# is validated below AFTER parsing, so a set-but-empty --scope '' is rejected
# rather than silently treated as the default.
host=""
scope="user"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --host=current)
      host="current"
      shift
      ;;
    --host)
      [[ $# -lt 2 || $2 != "current" ]] && usage
      host="current"
      shift 2
      ;;
    --scope=*)
      scope="${1#*=}"
      shift
      ;;
    --scope)
      [[ $# -lt 2 ]] && usage
      scope="$2"
      shift 2
      ;;
    *)
      usage
      ;;
  esac
done

# Scope validation is the shared library's: the scope enum, and the rule that
# system scope cannot pair with ByHost storage (per-user by definition). The
# library prints the reason; any refusal is a malformed invocation here, so it
# maps to exit 3.
if ! validate_record_scope "$scope" "$host" "" >/dev/null; then
  printf 'error: rejected --scope %s combined with --host %s\n' "${scope:-''}" "${host:-''}" >&2
  exit 3
fi

# Reject domain/key with characters outside the reverse-DNS / identifier set.
# Defends against yq-expression injection via crafted inputs even though
# macOS preference domains are constrained to this charset by the OS.
[[ $domain =~ ^[a-zA-Z0-9._-]+$ ]] || {
  printf 'error: invalid characters in domain %q\n' "$domain" >&2
  exit 3
}
[[ $key =~ ^[a-zA-Z0-9._-]+$ ]] || {
  printf 'error: invalid characters in key %q\n' "$key" >&2
  exit 3
}

# Resolved after argument validation so a malformed invocation still exits 3, not
# 2, whatever state the chezmoi source directory is in.
DATA_FILE="$(macos_defaults_data_file)" || exit $?
require_readable_data_file "$DATA_FILE" || exit $?

# Read live type. `defaults read-type` outputs e.g. "Type is boolean". A
# system-scope capture reads from the record's resolved system plist path
# (readable without sudo; /Library/Preferences is world-readable).
host_flag=()
[[ -n $host ]] && host_flag=(-currentHost)
read_target="$domain"
if [[ $scope == system ]]; then
  read_target="$(resolve_system_plist_path "$domain" "")"
fi

if ! raw_type="$(defaults "${host_flag[@]}" read-type "$read_target" "$key" 2>/dev/null)"; then
  printf 'error: %s %s is not currently set on this Mac\n' "$domain" "$key" >&2
  exit 1
fi

case "$raw_type" in
  *boolean*) schema_type="bool" ;;
  *integer*) schema_type="int" ;;
  *float*) schema_type="float" ;;
  *string*) schema_type="string" ;;
  *)
    printf 'error: unsupported defaults type %q for %s %s (only bool/int/float/string in v1 schema)\n' \
      "$raw_type" "$domain" "$key" >&2
    exit 1
    ;;
esac

raw_value="$(defaults "${host_flag[@]}" read "$read_target" "$key")"

# Normalize for YAML emission.
case "$schema_type" in
  bool)
    case "$raw_value" in
      1) yaml_value="true" ;;
      0) yaml_value="false" ;;
      *) yaml_value="$raw_value" ;;
    esac
    ;;
  string)
    # Quote the string for safe YAML emission.
    yaml_value="\"${raw_value//\"/\\\"}\""
    ;;
  *)
    yaml_value="$raw_value"
    ;;
esac

# Check whether (domain, key, host, scope) is already in the YAML. Scope is
# part of the identity: the same domain/key may be tracked at user scope AND
# at system scope, and a scope-blind match would answer "already tracked" and
# silently skip the append.
existing_value="$(yq eval -r \
  ".macos.defaults[] | select(.domain == \"$domain\" and .key == \"$key\" and ((.host // \"\") == \"$host\") and ((.scope // \"user\") == \"$scope\")) | .value" \
  "$DATA_FILE")"

if [[ -n $existing_value ]]; then
  # Already tracked. Compare.
  case "$schema_type" in
    bool)
      existing_norm="$existing_value"
      live_norm="$yaml_value"
      ;;
    string)
      # Compare bare bash strings, not YAML fragments.
      existing_norm="$existing_value"
      live_norm="$raw_value"
      ;;
    *)
      existing_norm="$existing_value"
      live_norm="$yaml_value"
      ;;
  esac
  if [[ $existing_norm == "$live_norm" ]]; then
    printf 'already tracked: %s %s = %s\n' "$domain" "$key" "$existing_value"
    exit 0
  else
    printf 'drift: %s %s, yaml=%s disk=%s\n' "$domain" "$key" "$existing_value" "$raw_value" >&2
    # shellcheck disable=SC2016
    printf '  resolve via `just defaults-apply` (revert) or hand-edit YAML.\n' >&2
    exit 4
  fi
fi

# Append a new record.
# Create temp file in the same directory as DATA_FILE so `mv` is atomic
# (mv across filesystems falls back to copy+delete and loses atomicity).
tmp="$(mktemp "${DATA_FILE}.XXXXXX")"
trap 'rm -f "$tmp"' EXIT

yq eval \
  ".macos.defaults += [{\"domain\": \"$domain\", \"key\": \"$key\", \"type\": \"$schema_type\", \"value\": $yaml_value, \"tier\": \"enforce\"$([[ -n $host ]] && printf ', "host": "%s"' "$host")$([[ $scope == system ]] && printf ', "scope": "system"')}]" \
  "$DATA_FILE" >"$tmp"

mv "$tmp" "$DATA_FILE"
trap - EXIT

printf 'captured: %s %s = %s (type=%s%s%s)\n' "$domain" "$key" "$raw_value" "$schema_type" \
  "$([[ -n $host ]] && printf ' host=%s' "$host")" \
  "$([[ $scope == system ]] && printf ' scope=system')"
