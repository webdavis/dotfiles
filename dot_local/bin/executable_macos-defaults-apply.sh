#!/usr/bin/env bash
# macos-defaults-apply.sh, forced reapply of tracked macOS defaults.
#
# Same defaults-write loop as the Tier 1 chezmoiscript runner, but invocable
# on demand without bumping the chezmoi hash gate. Use after fiddling in
# System Settings to revert disk state to the YAML.

set -euo pipefail
# Note: no `shopt -s lastpipe` here, the while loops below don't mutate
# outer-scope state (no counter to preserve, unlike drift.sh).

# shellcheck source=dot_local/bin/macos-defaults-lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/macos-defaults-lib.sh"

DATA_FILE="$(macos_defaults_data_file)" || exit $?
require_readable_data_file "$DATA_FILE" || exit $?

# Pre-flight: close System Settings if open (same reason as runner).
osascript -e 'tell application "System Settings" to quit' 2>/dev/null || true

# Main loop: one `defaults write` per record. A system-scope record goes
# through sudo to its resolved plist path; user-scope records write exactly as
# before. A record that fails validation (unknown scope, a meaningless field
# pairing, a relative plist_path) aborts the run with the data-file status 2
# before anything is written for it.
defaults_records_unit_separated "$DATA_FILE" |
  while IFS=$'\x1f' read -r domain key type value host scope plist_path; do
    [[ -z $domain ]] && continue
    scope="$(validate_record_scope "$scope" "$host" "$plist_path")" || exit 2
    if [[ $scope == system ]]; then
      resolved_plist_path="$(resolve_system_plist_path "$domain" "$plist_path")" || exit 2
      system_defaults_write "$resolved_plist_path" "$key" "$type" "$value"
    elif [[ -n $host ]]; then
      defaults -currentHost write "$domain" "$key" "-$type" "$value"
    else
      defaults write "$domain" "$key" "-$type" "$value"
    fi
  done

# Post-loop: restart processes per killall list.
yq eval -r '.macos.killall[]' "$DATA_FILE" |
  while read -r proc; do
    [[ -z $proc ]] && continue
    killall "$proc" 2>/dev/null || true
  done

exit 0
