#!/usr/bin/env bash

set -euo pipefail

readonly upgrade_record="$HOME/.local/state/homebrew-weekly-upgrade/last-upgrade-changes.tsv"
readonly system_tailscaled="/usr/local/bin/tailscaled"

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
    $2 == "changed" { print $1 ": " $3 " → " $4 }
  ' <<<"$rows"
}

write_upgrade_record() {
  local header=$1
  local rows=$2
  mkdir -p "${upgrade_record%/*}" &&
    printf '%s\n' "$header" ${rows:+"$rows"} >"$upgrade_record.tmp" &&
    mv "$upgrade_record.tmp" "$upgrade_record"
}

homebrew_tailscaled() {
  local prefix
  prefix="$(brew --prefix)" && printf '%s/opt/tailscale/bin/tailscaled\n' "$prefix"
}

tailscale_is_installed() {
  local tailscaled=$1
  [[ -e $tailscaled ]]
}

system_daemon_runs_this_build() {
  local tailscaled=$1
  cmp -s "$tailscaled" "$system_tailscaled"
}

reinstall_system_daemon() {
  local tailscaled=$1
  sudo -n "$tailscaled" install-system-daemon >/dev/null
}

main() {
  local failed=no
  local header before="" after rows tailscaled
  local before_was_read=no

  header="$(upgrade_record_header)"
  if before="$(installed_versions)"; then
    before_was_read=yes
    if ! write_upgrade_record "$header" ""; then
      print_error record-failed "could not write $upgrade_record"
      failed=yes
    fi
  else
    print_error listing-failed "brew list --versions failed before the upgrade, so this run records no changes"
    failed=yes
  fi

  if ! brew update >/dev/null; then
    print_error update-failed "brew update failed"
    failed=yes
  fi

  if ! brew upgrade >/dev/null; then
    print_error upgrade-failed "brew upgrade failed"
    failed=yes
  fi

  if ! tailscaled="$(homebrew_tailscaled)"; then
    print_error prefix-failed "brew --prefix failed, so the Tailscale system daemon was not checked"
    failed=yes
  elif tailscale_is_installed "$tailscaled" && ! system_daemon_runs_this_build "$tailscaled"; then
    if reinstall_system_daemon "$tailscaled"; then
      printf 'tailscaled: system daemon reinstalled\n'
    else
      print_error tailscale-refresh-failed "the Tailscale system daemon still runs the old build; run: sudo $tailscaled install-system-daemon"
      failed=yes
    fi
  fi

  if ! posture converge >/dev/null; then
    print_error converge-failed "posture converge failed, so osquery may be on its default configuration; run chezmoi apply"
    failed=yes
  fi

  if ! after="$(installed_versions)"; then
    print_error listing-failed "brew list --versions failed after the upgrade, so this run records no changes"
    failed=yes
  elif [[ $before_was_read == yes ]]; then
    rows="$(changed_rows "$before" "$after")"
    if ! write_upgrade_record "$header" "$rows"; then
      print_error record-failed "could not write $upgrade_record"
      failed=yes
    fi
    change_lines "$rows"
  fi

  if [[ $failed == yes ]]; then
    exit 1
  fi
}

main "$@"
