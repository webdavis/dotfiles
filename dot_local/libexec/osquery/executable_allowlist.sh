#!/usr/bin/env bash
#
# allowlist.sh - the ONE writer for the launchd page-allowlist: the *user* LaunchAgents
# whose new persistence digests instead of paging. Every caller (manual curation now, the
# tap-button bot and /osquery skill later) goes through this single security boundary, so
# all validation lives here.
#
#   allowlist.sh -a <label>   # allow: capture <label>'s identity and add it
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
  printf 'usage: %s -a <label>\n' "${0##*/}" >&2
  exit 2
}

# A real launchd label starts alphanumeric, then allows . _ @ - (so
# homebrew.mxcl.postgresql@17 passes) and nothing else - no wildcards, paths,
# spaces, or empties. Apple/system labels are refused outright.
is_valid_label() {
  [[ $1 =~ ^[A-Za-z0-9][A-Za-z0-9._@-]+$ ]] || return 1
  # Refuse Apple/system labels case-insensitively (and the dotless prefix), so a
  # COM.APPLE.* variant can't slip past and falsely suppress a system-daemon page.
  local lower="${1,,}"
  [[ $lower == com.apple.* || $lower == com.apple ]] && return 1
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
  # Capture the label's known-good identity from the SAME launchd table the finding comes from,
  # so a future persistence_launchd row matches the stored tuple exactly. A label with no loaded
  # LaunchAgent captures a degraded label-only entry (empty path/program) that the alerter will
  # NOT suppress on - fail-safe, and a warning tells the operator to re-run once it is loaded.
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
  jq -cn --arg label "$label" --arg path "$rel_path" --arg program "$rel_prog" --arg sha256 "$sha" \
    '{label:$label, path:$path, program:$program, sha256:$sha256}' >>"$ALLOWLIST"
  if [[ -z $abs_path || -z $abs_prog ]]; then
    printf 'allowed (label-only, degraded): %s - no loaded LaunchAgent found; re-run once it is loaded to capture its identity, or it will NOT be suppressed\n' "$label" >&2
  else
    printf 'allowed: %s -> %s\n' "$label" "$abs_prog"
  fi
}

action=""
label=""
while getopts ':a:' option; do
  case "$option" in
    a)
      action="allow"
      label="$OPTARG"
      ;;
    :)
      printf 'option -%s requires a label\n' "$OPTARG" >&2
      usage
      ;;
    *) usage ;;
  esac
done

case "$action" in
  allow) allow_label "$label" ;;
  *) usage ;;
esac
