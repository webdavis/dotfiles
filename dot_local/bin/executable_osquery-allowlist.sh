#!/usr/bin/env bash
#
# osquery-allowlist.sh — the ONE writer for the launchd page-allowlist: the *user*
# LaunchAgents whose new persistence digests instead of paging. Every caller (manual
# curation now, the PR#2 tap-button bot and /osquery skill later) goes through this
# single security boundary, so all validation lives here.
#
#   osquery-allowlist.sh -a <label>   # allow: capture <label>'s identity and add/refresh it
#   osquery-allowlist.sh -d <label>   # deny: remove the entry for <label>
#   osquery-allowlist.sh -l           # list the current allowlist
#
# R2-1: an entry is a TUPLE, not a bare label. Suppressing on the label alone let an attacker
# reuse an allowlisted label but point the plist at a malicious program and be silently
# suppressed. `-a` captures the label's KNOWN-GOOD identity (canonical plist path + program +
# plist sha256) from the SAME launchd table a persistence_launchd finding comes from, so the
# alerter suppresses ONLY a full-tuple match and PAGES a reused label. One NDJSON tuple per
# line: {"label","path","program","sha256"}; a leading $HOME is stored as ~/ (user-agnostic).
#
# System daemons (/Library/LaunchDaemons) page by path in the alerter's gate regardless of this
# file, so Apple/system labels are refused here (allowlisting them would be a false suppression).
set -euo pipefail

ALLOWLIST="${OSQUERY_LAUNCHD_ALLOWLIST:-$HOME/.config/osquery/page-launchd-allowlist.txt}"
OSQUERYI="${OSQUERYI:-$(command -v osqueryi || echo /usr/local/bin/osqueryi)}"

usage() {
  printf 'usage: %s -a <label> | -d <label> | -l\n' "${0##*/}" >&2
  exit 2
}

# The JSON label of an allowlist line, or empty for a comment/blank/non-JSON line.
entry_label() { jq -r '.label // empty' <<<"$1" 2>/dev/null || true; }

# A real launchd label starts alphanumeric, then allows . _ @ - (so
# homebrew.mxcl.postgresql@17 passes) and nothing else — no wildcards, paths,
# spaces, or empties. Apple/system labels are refused outright.
is_valid_label() {
  [[ $1 =~ ^[A-Za-z0-9][A-Za-z0-9._@-]+$ ]] || return 1
  # Refuse Apple/system labels case-insensitively (and the dotless prefix), so a
  # COM.APPLE.* variant can't slip past and falsely suppress a system-daemon page.
  local lower="${1,,}"
  [[ $lower == com.apple.* || $lower == com.apple ]] && return 1
  return 0
}

# Rewrite the allowlist, preserving comment/blank lines and dropping any tuple for <label>
# (used by both -a refresh and -d). Reads $ALLOWLIST, writes the filtered result to stdout.
_without_label() {
  local drop="$1" line
  [[ -f $ALLOWLIST ]] || return 0
  while IFS= read -r line || [[ -n $line ]]; do
    case "$line" in
      '' | '#'*)
        printf '%s\n' "$line"
        continue
        ;;
    esac
    [[ "$(entry_label "$line")" == "$drop" ]] && continue
    printf '%s\n' "$line"
  done <"$ALLOWLIST"
}

allow_label() {
  local label="$1"
  if ! is_valid_label "$label"; then
    printf 'refused (invalid or system label): %s\n' "$label" >&2
    exit 1
  fi
  mkdir -p "$(dirname "$ALLOWLIST")"
  touch "$ALLOWLIST"
  # Capture the label's known-good identity from the SAME launchd table the finding comes from,
  # so a future persistence_launchd row matches the stored tuple exactly. A label with no loaded
  # LaunchAgent captures a degraded label-only entry (empty path/program) that the alerter will
  # NOT suppress on — fail-safe, and a warning tells the operator to re-run once it is loaded.
  local row abs_path abs_prog sha rel_path rel_prog
  row=$("$OSQUERYI" --json \
    "SELECT path, COALESCE(NULLIF(program,''), program_arguments) AS program FROM launchd WHERE label = '$label';" \
    2>/dev/null | jq -c '.[0] // empty' 2>/dev/null) || row=""
  abs_path=$(jq -r '.path // ""' <<<"$row" 2>/dev/null || true)
  abs_prog=$(jq -r '.program // ""' <<<"$row" 2>/dev/null || true)
  sha=""
  [[ -n $abs_path && -f $abs_path ]] && sha=$(shasum -a 256 "$abs_path" 2>/dev/null | awk '{print $1}')
  # Relativize a leading $HOME to ~/ (keeps the file user-agnostic; the alerter re-expands it).
  rel_path="${abs_path/#"$HOME"\//\~/}"
  rel_prog="${abs_prog//"$HOME"\//\~/}"
  local temp
  temp=$(mktemp)
  _without_label "$label" >"$temp"
  jq -cn --arg label "$label" --arg path "$rel_path" --arg program "$rel_prog" --arg sha256 "$sha" \
    '{label:$label, path:$path, program:$program, sha256:$sha256}' >>"$temp"
  mv -f "$temp" "$ALLOWLIST"
  if [[ -z $abs_path || -z $abs_prog ]]; then
    printf 'allowed (label-only, degraded): %s — no loaded LaunchAgent found; re-run once it is loaded to capture its identity, or it will NOT be suppressed\n' "$label" >&2
  else
    printf 'allowed: %s → %s\n' "$label" "$abs_prog"
  fi
}

deny_label() {
  local label="$1"
  if ! is_valid_label "$label"; then
    printf 'refused (invalid or system label): %s\n' "$label" >&2
    exit 1
  fi
  if [[ ! -f $ALLOWLIST ]] || [[ -z "$(grep -F "\"label\":\"$label\"" "$ALLOWLIST" 2>/dev/null || true)" ]]; then
    printf 'not present: %s\n' "$label"
    return 0
  fi
  local temp
  temp=$(mktemp)
  _without_label "$label" >"$temp"
  mv -f "$temp" "$ALLOWLIST"
  printf 'denied: %s\n' "$label"
}

list_labels() {
  if [[ ! -s $ALLOWLIST ]]; then
    printf 'launchd page-allowlist is empty (%s)\n' "$ALLOWLIST"
    return 0
  fi
  printf 'launchd page-allowlist (%s):\n' "$ALLOWLIST"
  local line label program
  while IFS= read -r line || [[ -n $line ]]; do
    case "$line" in
      '' | '#'*) continue ;;
    esac
    label=$(entry_label "$line")
    [[ -n $label ]] || continue
    program=$(jq -r '.program // ""' <<<"$line" 2>/dev/null || true)
    printf '  • %s  →  %s\n' "$label" "${program:-<no identity captured>}"
  done <"$ALLOWLIST"
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
