#!/usr/bin/env bash
# relay-agent.sh: maps a hook payload + state to relay.sh args (project, pane, done-snippet).
set -uo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
agent="$root/dot_local/bin/executable_relay-agent.sh"
[[ -x $agent ]] || {
  echo "relay-agent: FAIL -- not executable" >&2
  exit 1
}
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
# mock relay.sh records its argv NUL-separated
cat >"$tmp/relay.sh" <<'MOCK'
#!/usr/bin/env bash
printf '%s\0' "$@" >"$RELAY_ARGS_FILE"
MOCK
chmod +x "$tmp/relay.sh"
# minimal assistant-text transcript (last text block = "done thinking")
printf '%s\n' '{"type":"assistant","message":{"content":[{"type":"text","text":"done thinking"}]}}' >"$tmp/t.jsonl"
out="$(
  printf '{"cwd":"/x/dotfiles","transcript_path":"%s/t.jsonl"}' "$tmp" |
    PATH="$tmp:$PATH" RELAY_BIN="$tmp/relay.sh" RELAY_ARGS_FILE="$tmp/args" HERDR_PANE_ID="wW:p8" \
      bash "$agent" "done" 2>&1
  echo "rc=$?"
)"
grep -q "rc=0" <<<"$out" || {
  echo "relay-agent: FAIL -- exit not 0" >&2
  exit 1
}
args="$(tr '\0' '\n' <"$tmp/args")"
grep -qx -- "--state" <<<"$args" || {
  echo "relay-agent: FAIL -- state flag" >&2
  exit 1
}
grep -qx -- "done" <<<"$args" || {
  echo "relay-agent: FAIL -- state value" >&2
  exit 1
}
grep -qx -- "dotfiles" <<<"$args" || {
  echo "relay-agent: FAIL -- project" >&2
  exit 1
}
grep -qx -- "wW:p8" <<<"$args" || {
  echo "relay-agent: FAIL -- pane" >&2
  exit 1
}
grep -qx -- "done thinking" <<<"$args" || {
  echo "relay-agent: FAIL -- snippet" >&2
  exit 1
}
echo "relay-agent: OK"
