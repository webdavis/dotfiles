#!/usr/bin/env bash
# relay-codex-hooks: idempotently add relay's Codex notifications (done + blocked)
# to ~/.codex/hooks.json, preserving herdr's integration entry. herdr owns its
# SessionStart hook (and regenerates it on update); we only ever add our two.
# Safe to re-run; re-heals relay's entries after herdr re-installs its hook.
set -euo pipefail

hooks="$HOME/.codex/hooks.json"
agent="$HOME/.local/bin/relay-agent.sh"
[[ -x $agent ]] || exit 0 # relay-agent not deployed yet; nothing to wire

done_cmd="RELAY_AGENT=codex $agent done"
blocked_cmd="RELAY_AGENT=codex $agent blocked"

base='{"hooks":{}}'
[[ -f $hooks ]] && base="$(cat "$hooks")"

merged="$(printf '%s' "$base" | jq \
  --arg d "$done_cmd" --arg b "$blocked_cmd" '
  def ensure($event; $cmd):
    .hooks[$event] = ((.hooks[$event] // [])
      | if any(.[]?.hooks[]?; .command == $cmd) then .
        else . + [{hooks: [{type: "command", command: $cmd}]}] end);
  ensure("Stop"; $d) | ensure("PermissionRequest"; $b)
')" || exit 0

mkdir -p "$(dirname "$hooks")"
tmp="$(mktemp "${hooks}.XXXXXX")"
printf '%s\n' "$merged" >"$tmp"
mv "$tmp" "$hooks"
