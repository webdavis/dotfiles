#!/usr/bin/env bash

set -euo pipefail

readonly exit_failure=1
readonly exit_pending=100

print_error() {
  local kind=$1
  local message=$2
  printf 'error[%s]: %s\n' "$kind" "$message" >&2
}

daemon_version() {
  local shown long
  shown="$(tailscale version --daemon 2>/dev/null)" || true
  long="$(sed -n 's/^Daemon: *//p' <<<"$shown")"
  [[ -n $long ]] && printf '%s\n' "${long%%-*}"
}

homebrew_tailscaled() {
  local prefix
  prefix="$(brew --prefix)" && printf '%s/opt/tailscale/bin/tailscaled\n' "$prefix"
}

homebrew_version() {
  local tailscaled=$1
  local shown
  shown="$("$tailscaled" --version)" && [[ -n $shown ]] && printf '%s\n' "${shown%%$'\n'*}"
}

sudo_was_refused() {
  local complaint=$1
  [[ $complaint == *password* || $complaint == *terminal* ]]
}

main() {
  local old new tailscaled complaint status=0

  if ! old="$(daemon_version)"; then
    print_error daemon-unreachable "tailscale version --daemon printed no Daemon line"
    exit "$exit_failure"
  fi
  if ! tailscaled="$(homebrew_tailscaled)" || ! new="$(homebrew_version "$tailscaled")"; then
    print_error version-unknown "could not read the version of Homebrew's tailscaled"
    exit "$exit_failure"
  fi

  if [[ $old == "$new" ]]; then
    exit 0
  fi

  complaint="$(sudo "$tailscaled" install-system-daemon 2>&1 >/dev/null)" || status=$?
  if ((status == 0)); then
    printf 'tailscaled: system daemon reinstalled %s to %s\n' "$old" "$new"
    exit 0
  fi

  printf '%s\n' "$complaint" >&2
  if sudo_was_refused "$complaint"; then
    print_error needs-sudo "the system daemon runs $old and Homebrew has $new"
    exit "$exit_pending"
  fi
  print_error refresh-failed "sudo $tailscaled install-system-daemon exited $status"
  exit "$exit_failure"
}

main "$@"
