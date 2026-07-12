#!/usr/bin/env bash
# macos-defaults-apply.sh — forced reapply of tracked macOS defaults.
#
# Same defaults-write loop as the Tier 1 chezmoiscript runner, but invocable
# on demand without bumping the chezmoi hash gate. Use after fiddling in
# System Settings to revert disk state to the YAML.

set -euo pipefail
# Note: no `shopt -s lastpipe` here — the while loops below don't mutate
# outer-scope state (no counter to preserve, unlike drift.sh).

# shellcheck source=dot_local/bin/macos-defaults-lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/macos-defaults-lib.sh"

DATA_FILE="$(macos_defaults_data_file)" || {
  printf 'error: cannot resolve the chezmoi source dir for macos_defaults.yaml\n' >&2
  exit 2
}

require_readable_data_file "$DATA_FILE"

# Pre-flight: close System Settings if open (same reason as runner).
osascript -e 'tell application "System Settings" to quit' 2>/dev/null || true

# Main loop: one `defaults write` per record.
defaults_records_tsv "$DATA_FILE" |
  while IFS=$'\t' read -r domain key type value host; do
    [[ -z $domain ]] && continue
    if [[ -n $host ]]; then
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
