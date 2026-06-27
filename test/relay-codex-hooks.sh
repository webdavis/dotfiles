#!/usr/bin/env bash
# relay-codex-hooks.sh: adds relay done+blocked to ~/.codex/hooks.json, preserves
# herdr's SessionStart, idempotent.
set -uo pipefail
script="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/dot_local/bin/executable_relay-codex-hooks.sh"
[[ -x $script ]] || {
  echo "relay-codex-hooks: FAIL -- not executable" >&2
  exit 1
}
home="$(mktemp -d)"
trap 'rm -rf "$home"' EXIT
mkdir -p "$home/.codex" "$home/.local/bin"
# fake relay-agent so the script's -x guard passes
printf '#!/usr/bin/env bash\n' >"$home/.local/bin/relay-agent.sh"
chmod +x "$home/.local/bin/relay-agent.sh"
# herdr's pre-existing SessionStart entry must survive
cat >"$home/.codex/hooks.json" <<'JSON'
{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"bash herdr-agent-state.sh session"}]}]}}
JSON
HOME="$home" bash "$script" || {
  echo "relay-codex-hooks: FAIL -- run errored" >&2
  exit 1
}
got="$(cat "$home/.codex/hooks.json")"
jq -e '.hooks.SessionStart[0].hooks[0].command | test("herdr-agent-state")' <<<"$got" >/dev/null || {
  echo "relay-codex-hooks: FAIL -- herdr SessionStart not preserved" >&2
  exit 1
}
jq -e '[.hooks.Stop[]?.hooks[]?.command] | any(test("relay-agent.sh done"))' <<<"$got" >/dev/null || {
  echo "relay-codex-hooks: FAIL -- relay done not added" >&2
  exit 1
}
jq -e '[.hooks.PermissionRequest[]?.hooks[]?.command] | any(test("relay-agent.sh blocked"))' <<<"$got" >/dev/null || {
  echo "relay-codex-hooks: FAIL -- relay blocked not added" >&2
  exit 1
}
# idempotent: a second run must not duplicate relay's Stop entry
HOME="$home" bash "$script"
n="$(jq '[.hooks.Stop[]?.hooks[]?.command | select(test("relay-agent.sh done"))] | length' "$home/.codex/hooks.json")"
[[ $n -eq 1 ]] || {
  echo "relay-codex-hooks: FAIL -- not idempotent (Stop relay count=$n)" >&2
  exit 1
}
m="$(jq '[.hooks.PermissionRequest[]?.hooks[]?.command | select(test("relay-agent.sh blocked"))] | length' "$home/.codex/hooks.json")"
[[ $m -eq 1 ]] || {
  echo "relay-codex-hooks: FAIL -- not idempotent (PermissionRequest relay count=$m)" >&2
  exit 1
}
echo "relay-codex-hooks: OK"
