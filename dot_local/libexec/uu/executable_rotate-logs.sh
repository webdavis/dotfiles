#!/usr/bin/env bash

set -euo pipefail

readonly rotate_at_bytes=10485760
readonly archives_kept=5
readonly log_directory="$HOME/.local/log"
readonly logs=(
  "$log_directory/atuin-daemon.log"
  "$log_directory/osquery/digest.log"
  "$log_directory/osquery/firewall-gatekeeper-monitor.log"
  "$log_directory/osquery/heartbeat.log"
  "$log_directory/osquery/results-alerter.log"
  "$log_directory/osquery/tailscale-monitor.log"
  "$log_directory/osquery/uptime-watchdog.log"
  "$log_directory/pns-daemon.log"
  "$log_directory/scalebar/scalebar.log"
  "$log_directory/yt-dlp/pot-provider.log"
)

log_is_a_regular_file() {
  local log=$1
  [[ -f $log && ! -L $log ]]
}

size_of() {
  local file=$1
  local size
  size="$(wc -c <"$file")"
  printf '%s\n' "$((size))"
}

log_is_under_rotation_size() {
  local size=$1
  ((size < rotate_at_bytes))
}

archive_of() {
  local log=$1 index=$2
  printf '%s.%s.gz\n' "$log" "$index"
}

shift_archives() {
  local log=$1
  rm -f "$(archive_of "$log" "$archives_kept")" || return 1
  local index
  for ((index = archives_kept - 1; index >= 1; index--)); do
    if [[ -e $(archive_of "$log" "$index") ]]; then
      mv -f "$(archive_of "$log" "$index")" "$(archive_of "$log" $((index + 1)))" || return 1
    fi
  done
}

compress_into_newest_archive() {
  local log=$1
  local newest_archive partial_archive
  newest_archive="$(archive_of "$log" 1)"
  partial_archive="$newest_archive.partial"
  if ! (umask 077 && gzip -c <"$log" >"$partial_archive") || [[ ! -s $partial_archive ]]; then
    rm -f "$partial_archive"
    return 1
  fi
  mv -f "$partial_archive" "$newest_archive"
}

truncate_in_place() {
  local log=$1
  : >"$log"
}

rotate_log() {
  local log=$1
  shift_archives "$log" && compress_into_newest_archive "$log" && truncate_in_place "$log"
}

main() {
  local log size
  local failures=0
  for log in "${logs[@]}"; do
    if ! log_is_a_regular_file "$log"; then
      continue
    fi
    size="$(size_of "$log")"
    if log_is_under_rotation_size "$size"; then
      continue
    fi
    if ! rotate_log "$log"; then
      printf 'error[rotate-failed]: could not rotate %s.\n' "$log" >&2
      failures=$((failures + 1))
      continue
    fi
    printf 'rotated %s: %s bytes\n' "$log" "$size"
  done

  if ((failures > 0)); then
    exit 1
  fi
}

main "$@"
