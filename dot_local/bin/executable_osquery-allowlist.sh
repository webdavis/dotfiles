#!/usr/bin/env bash
#
# osquery-allowlist.sh — the ONE writer for the launchd page-allowlist: the labels
# whose new *user* LaunchAgents digest instead of paging. Every caller (manual
# curation now, the PR#2 tap-button bot and /osquery skill later) goes through this
# single security boundary, so all label validation lives here.
#
#   osquery-allowlist.sh -a <label>   # allow: add a label (idempotent)
#   osquery-allowlist.sh -d <label>   # deny: remove a label
#   osquery-allowlist.sh -l           # list the current allowlist
#
# System daemons (/Library/LaunchDaemons) page by path in the alerter's gate
# regardless of this file, so Apple/system labels are refused here (allowlisting
# them would be a false suppression). One bare label per line — the reader matches
# with grep -qxF, so an inline comment on a label line would break the match.
set -euo pipefail

ALLOWLIST="${OSQUERY_LAUNCHD_ALLOWLIST:-$HOME/.config/osquery/page-launchd-allowlist.txt}"

usage() {
  printf 'usage: %s -a <label> | -d <label> | -l\n' "${0##*/}" >&2
  exit 2
}

# A real launchd label starts alphanumeric, then allows . _ @ - (so
# homebrew.mxcl.postgresql@17 passes) and nothing else — no wildcards, paths,
# spaces, or empties. Apple/system labels are refused outright.
is_valid_label() {
  [[ $1 =~ ^[A-Za-z0-9][A-Za-z0-9._@-]+$ ]] || return 1
  [[ $1 == com.apple.* ]] && return 1
  return 0
}

allow_label() {
  local label="$1"
  if ! is_valid_label "$label"; then
    printf 'refused (invalid or system label): %s\n' "$label" >&2
    exit 1
  fi
  mkdir -p "$(dirname "$ALLOWLIST")"
  touch "$ALLOWLIST"
  if grep -qxF -- "$label" "$ALLOWLIST"; then
    printf 'already allowed: %s\n' "$label"
    return 0
  fi
  printf '%s\n' "$label" >>"$ALLOWLIST"
  printf 'allowed: %s\n' "$label"
}

deny_label() {
  local label="$1"
  if ! is_valid_label "$label"; then
    printf 'refused (invalid or system label): %s\n' "$label" >&2
    exit 1
  fi
  if [[ ! -f $ALLOWLIST ]] || ! grep -qxF -- "$label" "$ALLOWLIST"; then
    printf 'not present: %s\n' "$label"
    return 0
  fi
  local temp_file
  temp_file=$(mktemp)
  grep -vxF -- "$label" "$ALLOWLIST" >"$temp_file" || true
  mv -f "$temp_file" "$ALLOWLIST"
  printf 'denied: %s\n' "$label"
}

list_labels() {
  if [[ ! -s $ALLOWLIST ]]; then
    printf 'launchd page-allowlist is empty (%s)\n' "$ALLOWLIST"
    return 0
  fi
  printf 'launchd page-allowlist (%s):\n' "$ALLOWLIST"
  sed -e 's/#.*//' -e 's/[[:space:]]*$//' "$ALLOWLIST" | grep -v '^[[:space:]]*$' | sed 's/^/  • /' || true
}

action=""
label=""
while getopts ':a:d:l' option; do
  case "$option" in
    a)
      action="allow"
      label="$OPTARG"
      ;;
    d)
      action="deny"
      label="$OPTARG"
      ;;
    l) action="list" ;;
    :)
      printf 'option -%s requires a label\n' "$OPTARG" >&2
      usage
      ;;
    *) usage ;;
  esac
done

case "$action" in
  allow) allow_label "$label" ;;
  deny) deny_label "$label" ;;
  list) list_labels ;;
  *) usage ;;
esac
