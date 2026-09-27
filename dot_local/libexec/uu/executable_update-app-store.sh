#!/usr/bin/env bash

set -euo pipefail

readonly app_store_brewfile="$HOME/.local/state/homebrew/mas.Brewfile"
readonly step_time_limit_seconds=180
readonly timed_out_status=124

print_error() {
  local kind=$1
  local message=$2
  printf 'error[%s]: %s\n' "$kind" "$message" >&2
}

print_step_error() {
  local step=$1
  local status=$2
  if ((status == timed_out_status)); then
    print_error timed-out "$step did not finish within $step_time_limit_seconds seconds"
  else
    print_error step-failed "$step failed with exit $status"
  fi
}

installed_apps() {
  mas list --json | jq -r '[.name, .version] | @tsv'
}

app_store_apps_are_declared() {
  [[ -s $app_store_brewfile ]]
}

upgrade_apps() {
  timeout "$step_time_limit_seconds" mas upgrade >/dev/null
}

install_missing_apps() {
  timeout "$step_time_limit_seconds" brew bundle --no-upgrade --file="$app_store_brewfile" >/dev/null
}

change_lines() {
  local before=$1
  local after=$2
  awk -F '\t' '
    NF < 2 { next }
    FILENAME == ARGV[1] { previous[$1] = $2; next }
    { seen[$1] = 1 }
    !($1 in previous) { print $1 ": added"; next }
    previous[$1] != $2 { print $1 ": " previous[$1] " → " $2 }
    END { for (name in previous) if (!(name in seen)) print name ": removed" }
  ' <(printf '%s\n' "$before") <(printf '%s\n' "$after") | sort
}

main() {
  local failed=no
  local before="" after status
  local before_was_read=no

  if before="$(installed_apps)"; then
    before_was_read=yes
  else
    print_error listing-failed "mas list failed before the upgrade, so this run lists no changes"
    failed=yes
  fi

  status=0
  upgrade_apps || status=$?
  if ((status != 0)); then
    print_step_error "mas upgrade" "$status"
    failed=yes
  fi

  if app_store_apps_are_declared; then
    status=0
    install_missing_apps || status=$?
    if ((status != 0)); then
      print_step_error "installing missing apps from $app_store_brewfile" "$status"
      failed=yes
    fi
  fi

  if ! after="$(installed_apps)"; then
    print_error listing-failed "mas list failed after the upgrade, so this run lists no changes"
    failed=yes
  elif [[ $before_was_read == yes ]]; then
    change_lines "$before" "$after"
  fi

  if [[ $failed == yes ]]; then
    exit 1
  fi
}

main "$@"
