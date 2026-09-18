#!/usr/bin/env bash
# relay-codex-hooks: idempotently add relay's Codex notifications (done + blocked)
# to ~/.codex/hooks.json, preserving herdr's integration entry. herdr owns its
# SessionStart hook (and regenerates it on update); we only ever add our two.
# Safe to re-run; re-heals relay's entries after herdr re-installs its hook.
set -euo pipefail

hooks="$HOME/.codex/hooks.json"
agent="$HOME/.cargo/bin/pns"
[[ -x $agent ]] || exit 0 # the engine is not deployed yet; nothing to wire

done_cmd="PNS_PRODUCER=codex $agent hook stop"
blocked_cmd="PNS_PRODUCER=codex $agent hook blocked"

# Read the existing config. Require EXACTLY one object root whose "hooks" is an object; heal an
# empty/whitespace/absent file from the {"hooks":{}} default; on any OTHER malformed input (multiple
# concatenated roots, a non-object root, a non-object "hooks") warn and leave the file untouched -- never
# overwrite a human's broken-but-recoverable file with our guess. -s slurps every root into an array so
# a two-root file has length 2 and is rejected instead of silently merged twice.
base='{"hooks":{}}'
if [[ -f $hooks ]]; then
  raw="$(cat "$hooks")"
  if [[ -n ${raw//[[:space:]]/} ]]; then
    if candidate="$(printf '%s' "$raw" | jq -es 'if length==1 and (.[0]|type=="object") and (.[0].hooks|type=="object") then .[0] else error end' 2>/dev/null)"; then
      base="$candidate"
    else
      printf 'relay-codex-hooks: %s is not a single object with an object "hooks" field; leaving it untouched.\n' "$hooks" >&2
      exit 0
    fi
  fi
fi

# Migrate only complete commands this installer generated for this home/event.
# That list includes the retired PNS_AGENT spelling of the current command, so a
# deployed row is rewritten in place to PNS_PRODUCER rather than left beside the
# new one.
# Keep handler and group metadata; collapse duplicates only when both agree.
# Conflicting customizations leave the original file untouched for review.
merged="$(printf '%s' "$base" | jq \
  --arg root "$HOME" --arg d "$done_cmd" --arg b "$blocked_cmd" '
  def migrate($event; $cmd; $action):
    (if $action == "stop" then "done" else $action end) as $legacy_action |
    [$cmd,
      (("PNS_PRODUCER", "PNS_AGENT", "RELAY_AGENT") + "=codex " + $root +
        ("/.cargo/bin/pns", "/.local/libexec/pns/pns") + " hook " + $action),
      ("RELAY_AGENT=codex " + $root + "/" +
        (".local/bin/relay-agent.sh", ".local/libexec/pns/codex-hooks/relay-agent.sh",
         ".local/libexec/pns/hooks/relay-agent.sh") + " " + $legacy_action),
      ("PNS_AGENT=codex " + $root + "/.local/libexec/pns/hooks/relay-agent.sh " + $legacy_action)
    ] as $owned |
    .hooks[$event] = (
      reduce (.hooks[$event] // [])[] as $entry
        ({entries: [], owner: null};
          reduce $entry.hooks[] as $handler
            (.kept = [];
              if $handler.type == "command" and ($owned | any(. == $handler.command)) then
                ($handler | .command = $cmd) as $updated |
                {group: ($entry | del(.hooks)), handler: $updated} as $identity |
                if .owner == null then .owner = $identity | .kept += [$updated]
                elif .owner == $identity then .
                else error("pns " + $event + " hook metadata differs; leaving hooks.json untouched") end
              else .kept += [$handler] end) |
          .kept as $kept |
          if ($kept | length) > 0 or ($entry.hooks | length) == 0 then
            .entries += [($entry | .hooks = $kept)]
          else . end) |
      if .owner == null then .entries + [{hooks: [{type: "command", command: $cmd}]}]
      else .entries end);
  migrate("Stop"; $d; "stop") | migrate("PermissionRequest"; $b; "blocked")
')" || exit 0

# Validate the merged candidate before writing: it must still be an object with an object "hooks".
# SINGLENESS is not re-checked here and does not need to be: this value came out of the jq above,
# which emitted one document from the one document the -es read at line 24 admitted. The check that
# a STREAM never gets in is that read, not this one.
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
