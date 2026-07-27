#!/usr/bin/env bash
# macos-defaults-drift.sh, read-only drift checker for tracked macOS defaults.
#
# Compares each record in .chezmoidata/macos_defaults.yaml against the live
# value via `defaults [-currentHost] read` (user scope) or a read of the
# record's resolved system plist path (system scope). Prints a tab-aligned
# table of drifted rows, plus one row per INDETERMINATE system-scope record:
# a plist this user cannot read gets the <unreadable> marker, distinct from
# <unset>, and is counted separately, never as drift and never skipped.
# Never writes.
#
# Exit codes:
#   0: no drift (indeterminate rows, if any, are reported but not counted)
#   1: drift detected
#   2: data file missing or unreadable, or a record failed validation

set -euo pipefail
shopt -s lastpipe

# shellcheck source=dot_local/bin/macos-defaults-lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/macos-defaults-lib.sh"

DATA_FILE="$(macos_defaults_data_file)" || exit $?
require_readable_data_file "$DATA_FILE" || exit $?

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

# Both counters mutate inside the pipeline's while loop, which is why lastpipe
# is required above: without it the loop runs in a subshell and both would
# read 0 after the loop, a silent false negative.
drift_count=0
indeterminate_count=0
header_printed=0
print_header() {
  if ((header_printed == 0)); then
    printf 'DOMAIN\tKEY\tEXPECTED\tACTUAL\n'
    header_printed=1
  fi
}

# Each record arrives as one unit-separated line: domain, key, type, value,
# host, scope, plist_path. A record that fails validation (unknown scope, a
# meaningless field pairing, a relative plist_path) aborts with the data-file
# status 2 rather than being misread. A system-scope record has THREE read
# outcomes: the value, <unset> (genuinely not set, compared as drift like any
# other value), and <unreadable> (indeterminate: reported as its own row,
# counted separately, never as drift and never skipped).
# Note: yq emits a single newline for an empty array; the inline guard below
# skips that empty row so the script exits 0 cleanly when nothing is tracked.
defaults_records_unit_separated "$DATA_FILE" |
  while IFS=$'\x1f' read -r domain key type value host scope plist_path; do
    [[ -z $domain ]] && continue
    scope="$(validate_record_scope "$scope" "$host" "$plist_path")" || exit 2
    expected="$(normalize "$type" "$value")"
    if [[ $scope == system ]]; then
      resolved_plist_path="$(resolve_system_plist_path "$domain" "$plist_path")" || exit 2
      actual="$(system_defaults_read_actual "$resolved_plist_path" "$key")"
      if [[ $actual == '<unreadable>' ]]; then
        print_header
        printf '%s\t%s\t%s\t%s\n' "$domain" "$key" "$expected" '<unreadable>'
        indeterminate_count=$((indeterminate_count + 1))
        continue
      fi
    elif [[ -n $host ]]; then
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

if ((indeterminate_count > 0)); then
  printf '\n%d indeterminate row(s): unreadable, not counted as drift.\n' "$indeterminate_count" >&2
fi
if ((drift_count > 0)); then
  printf '\n%d drift row(s) detected.\n' "$drift_count" >&2
  exit 1
fi
exit 0
