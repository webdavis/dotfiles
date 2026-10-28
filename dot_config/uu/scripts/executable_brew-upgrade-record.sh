#!/usr/bin/env bash

set -euo pipefail

readonly record_folder="$HOME/.local/state/homebrew-weekly-upgrade"
readonly upgrade_record="$record_folder/last-upgrade-changes.tsv"
readonly snapshot="$record_folder/installed-versions.tsv"
readonly exit_failure=1

print_error() {
  local kind=$1
  local message=$2
  printf 'error[%s]: %s\n' "$kind" "$message" >&2
}

upgrade_record_header() {
  date -u '+%s%t%Y-%m-%dT%H:%M:%SZ'
}

installed_versions() {
  brew list --versions | awk 'NF > 1 { name = $1; $1 = ""; print name "\t" substr($0, 2) }'
}

changed_rows() {
  local before=$1
  local after=$2
  awk -F '\t' '
    NF < 2 { next }
    FILENAME == ARGV[1] { previous[$1] = $2; next }
    { seen[$1] = 1 }
    !($1 in previous) { print $1 "\tadded\t\t" $2; next }
    previous[$1] != $2 { print $1 "\tchanged\t" previous[$1] "\t" $2 }
    END { for (name in previous) if (!(name in seen)) print name "\tremoved\t" previous[name] "\t" }
  ' <(printf '%s\n' "$before") <(printf '%s\n' "$after") | sort
}

change_lines() {
  local rows=$1
  awk -F '\t' '
    $2 == "added" { print $1 ": added" }
    $2 == "removed" { print $1 ": removed" }
    $2 == "changed" { print $1 ": " $3 " to " $4 }
  ' <<<"$rows"
}

write_upgrade_record() {
  local header=$1
  local rows=$2
  mkdir -p "${upgrade_record%/*}" &&
    printf '%s\n' "$header" ${rows:+"$rows"} >"$upgrade_record.tmp" &&
    mv "$upgrade_record.tmp" "$upgrade_record"
}

snapshot_exists() {
  [[ -f $snapshot ]]
}

save_snapshot() {
  local versions=$1
  printf '%s\n' "$versions" >"$snapshot.tmp" && mv "$snapshot.tmp" "$snapshot"
}

main() {
  local header after before="" rows=""

  header="$(upgrade_record_header)"
  if ! after="$(installed_versions)"; then
    print_error listing-failed "brew list --versions failed, so this run records no changes"
    exit "$exit_failure"
  fi

  if snapshot_exists; then
    before="$(<"$snapshot")"
    rows="$(changed_rows "$before" "$after")"
  fi

  if ! write_upgrade_record "$header" "$rows" || ! save_snapshot "$after"; then
    print_error record-failed "could not write $record_folder"
    exit "$exit_failure"
  fi
  change_lines "$rows"
}

main "$@"
