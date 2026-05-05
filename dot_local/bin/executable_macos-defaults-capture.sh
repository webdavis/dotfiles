#!/usr/bin/env bash
# macos-defaults-capture.sh — append a live setting to macos_defaults.yaml.
#
# Reads the current value+type via `defaults read-type` + `defaults read`,
# normalizes, appends to the YAML if not already tracked. If the entry is
# already tracked AND the live value matches: no-op (exit 0). If the entry
# is already tracked but the live value DIVERGES: exit 2 (drift) — resolve
# via `just defaults-apply` (revert) or hand-edit YAML (capture intent).
#
# Usage: macos-defaults-capture.sh <domain> <key> [--host current]
#
# Exit codes:
#   0 — appended, or already in sync
#   1 — key not currently set on this Mac
#   2 — YAML has a different value than disk (drift; resolve before re-running)
#   3 — malformed args

set -euo pipefail

DATA_FILE="${HOME}/.local/share/chezmoi/.chezmoidata/macos_defaults.yaml"

usage() {
  printf 'usage: macos-defaults-capture <domain> <key> [--host current]\n' >&2
  exit 3
}

[[ $# -lt 2 || $# -gt 4 ]] && usage

domain="$1"
key="$2"
shift 2

# Optional host argument. Three accepted forms:
#   --host=current  (single token, what the justfile recipe emits)
#   --host current  (two tokens, what a direct CLI invocation might use)
#   (omitted)       (global storage, no -currentHost flag)
host=""
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
    *)
      usage
      ;;
  esac
done

if [[ ! -r $DATA_FILE ]]; then
  printf 'error: cannot read %s\n' "$DATA_FILE" >&2
  exit 2
fi

# Read live type. `defaults read-type` outputs e.g. "Type is boolean".
host_flag=()
[[ -n $host ]] && host_flag=(-currentHost)

if ! raw_type="$(defaults "${host_flag[@]}" read-type "$domain" "$key" 2>/dev/null)"; then
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

raw_value="$(defaults "${host_flag[@]}" read "$domain" "$key")"

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

# Check whether (domain, key, host) is already in the YAML.
existing_value="$(yq eval -r \
  ".macos.defaults[] | select(.domain == \"$domain\" and .key == \"$key\" and ((.host // \"\") == \"$host\")) | .value" \
  "$DATA_FILE")"

if [[ -n $existing_value ]]; then
  # Already tracked. Compare.
  case "$schema_type" in
    bool)
      existing_norm="$existing_value"
      live_norm="$yaml_value"
      ;;
    string)
      existing_norm="\"$existing_value\""
      live_norm="$yaml_value"
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
    printf 'drift: %s %s — yaml=%s disk=%s\n' "$domain" "$key" "$existing_value" "$raw_value" >&2
    # shellcheck disable=SC2016
    printf '  resolve via `just defaults-apply` (revert) or hand-edit YAML.\n' >&2
    exit 2
  fi
fi

# Append a new record.
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT

yq eval \
  ".macos.defaults += [{\"domain\": \"$domain\", \"key\": \"$key\", \"type\": \"$schema_type\", \"value\": $yaml_value$([[ -n $host ]] && printf ', "host": "%s"' "$host")}]" \
  "$DATA_FILE" >"$tmp"

mv "$tmp" "$DATA_FILE"
trap - EXIT

printf 'captured: %s %s = %s (type=%s%s)\n' "$domain" "$key" "$raw_value" "$schema_type" \
  "$([[ -n $host ]] && printf ' host=%s' "$host")"
