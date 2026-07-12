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

# Read the existing config. Require EXACTLY one object root whose "hooks" is an object; heal an
# empty/whitespace/absent file from the {"hooks":{}} default; on any OTHER malformed input (multiple
# concatenated roots, a non-object root, a non-object "hooks") warn and leave the file untouched -- never
# overwrite a human's broken-but-recoverable file with our guess. -s slurps every root into an array so
# a two-root file has length 2 and is rejected instead of silently merged twice.
base='{"hooks":{}}'
if [[ -f $hooks ]]; then
  raw="$(cat "$hooks")"
  if [[ -n "${raw//[[:space:]]/}" ]]; then
    if candidate="$(printf '%s' "$raw" | jq -es 'if length==1 and (.[0]|type=="object") and (.[0].hooks|type=="object") then .[0] else error end' 2>/dev/null)"; then
      base="$candidate"
    else
      printf 'relay-codex-hooks: %s is not a single object with an object "hooks" field; leaving it untouched.\n' "$hooks" >&2
      exit 0
    fi
  fi
fi

merged="$(printf '%s' "$base" | jq \
  --arg d "$done_cmd" --arg b "$blocked_cmd" '
  def ensure($event; $cmd):
    .hooks[$event] = ((.hooks[$event] // [])
      | if any(.[]?.hooks[]?; .command == $cmd) then .
        else . + [{hooks: [{type: "command", command: $cmd}]}] end);
  ensure("Stop"; $d) | ensure("PermissionRequest"; $b)
')" || exit 0

# Validate the merged candidate before writing: it must still be a single object with an object "hooks".
printf '%s' "$merged" | jq -e 'type=="object" and (.hooks|type=="object")' >/dev/null 2>&1 || {
  printf 'relay-codex-hooks: refusing to write a non-object merge result; leaving %s untouched.\n' "$hooks" >&2
  exit 0
}

mkdir -p "$(dirname "$hooks")"
tmp="$(mktemp "${hooks}.XXXXXX")"
printf '%s\n' "$merged" >"$tmp"
mv "$tmp" "$hooks"

# Codex ignores new or changed non-managed hooks until reviewed and trusted via /hooks in an interactive
# Codex session. If this run ADDED or CHANGED a handler (the merged result differs semantically from what
# was there), advise the operator loudly -- once, only on real change; an idempotent re-run stays silent.
# We never synthesize trust state or pass any bypass flag; trusting is the operator's explicit action.
if [[ "$(printf '%s' "$merged" | jq -S . 2>/dev/null)" != "$(printf '%s' "$base" | jq -S . 2>/dev/null)" ]]; then
  printf 'relay-codex-hooks: added or changed Codex hooks in %s. Codex will IGNORE them until you review and trust them -- open Codex and run /hooks to approve.\n' "$hooks" >&2
fi
