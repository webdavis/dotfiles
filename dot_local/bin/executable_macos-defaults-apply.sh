#!/usr/bin/env bash
# macos-defaults-apply.sh — forced reapply of tracked macOS defaults.
#
# Same defaults-write loop as the Tier 1 chezmoiscript runner, but invocable
# on demand without bumping the chezmoi hash gate. Use after fiddling in
# System Settings to revert disk state to the YAML.

set -euo pipefail
# Note: no `shopt -s lastpipe` here — the while loops below don't mutate
# outer-scope state (no counter to preserve, unlike drift.sh).

DATA_FILE="${HOME}/workspaces/Ivy/webdavis/dotfiles/.chezmoidata/macos_defaults.yaml"

if [[ ! -r $DATA_FILE ]]; then
  printf 'error: cannot read %s\n' "$DATA_FILE" >&2
  exit 2
fi

# Pre-flight: close System Settings if open (same reason as runner).
osascript -e 'tell application "System Settings" to quit' 2>/dev/null || true

# Main loop: one `defaults write` per record.
yq eval -r '.macos.defaults[] | [.domain, .key, .type, .value, (.host // "")] | @tsv' "$DATA_FILE" |
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
