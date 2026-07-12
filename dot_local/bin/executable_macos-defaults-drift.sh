#!/usr/bin/env bash
# macos-defaults-drift.sh — read-only drift checker for tracked macOS defaults.
#
# Compares each record in .chezmoidata/macos_defaults.yaml against the live
# value via `defaults [-currentHost] read`. Prints a tab-aligned table of
# drifted rows only. Never writes.
#
# Exit codes:
#   0 — no drift
#   1 — drift detected
#   2 — data file missing or unreadable

set -euo pipefail
shopt -s lastpipe

# shellcheck source=dot_local/bin/macos-defaults-lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/macos-defaults-lib.sh"

DATA_FILE="$(macos_defaults_data_file)" || {
  printf 'error: cannot resolve the chezmoi source dir for macos_defaults.yaml\n' >&2
  exit 2
}

require_readable_data_file "$DATA_FILE"

# Normalize a value for comparison. macOS stores bools as 0/1; YAML ships them
# as true/false. Strings/ints/floats compare directly.
normalize() {
  local type="$1" value="$2"
  case "$type" in
    bool)
      case "$value" in
        true | yes | 1) printf '1' ;;
        false | no | 0) printf '0' ;;
        *) printf '%s' "$value" ;;
      esac
      ;;
    *) printf '%s' "$value" ;;
  esac
}

drift_count=0
header_printed=0
print_header() {
  if ((header_printed == 0)); then
    printf 'DOMAIN\tKEY\tEXPECTED\tACTUAL\n'
    header_printed=1
  fi
}

# Each record arrives as one TSV line: domain<TAB>key<TAB>type<TAB>value<TAB>host.
# Note: yq emits a single newline for an empty array; the inline guard below
# skips that empty row so the script exits 0 cleanly when nothing is tracked.
defaults_records_tsv "$DATA_FILE" |
  while IFS=$'\t' read -r domain key type value host; do
    [[ -z $domain ]] && continue
    expected="$(normalize "$type" "$value")"
    if [[ -n $host ]]; then
      actual="$(defaults -currentHost read "$domain" "$key" 2>/dev/null || printf '<unset>')"
    else
      actual="$(defaults read "$domain" "$key" 2>/dev/null || printf '<unset>')"
    fi
    if [[ $expected != "$actual" ]]; then
      print_header
      printf '%s\t%s\t%s\t%s\n' "$domain" "$key" "$expected" "$actual"
      drift_count=$((drift_count + 1))
    fi
  done

if ((drift_count > 0)); then
  printf '\n%d drift row(s) detected.\n' "$drift_count" >&2
  exit 1
fi
exit 0
