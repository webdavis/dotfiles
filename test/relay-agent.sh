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
    PATH="$tmp:$PATH" RELAY_BIN="$tmp/relay.sh" RELAY_ARGS_FILE="$tmp/args" HERDR_PANE_ID="wW:p8" CODEX_BIN=false \
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
# a non-done state takes its detail from the payload (.message // .detail), not a transcript
: >"$tmp/args"
out2="$(
  printf '{"cwd":"/x/dotfiles","message":"waiting for input"}' |
    PATH="$tmp:$PATH" RELAY_BIN="$tmp/relay.sh" RELAY_ARGS_FILE="$tmp/args" HERDR_PANE_ID="wW:p8" \
      bash "$agent" "blocked" 2>&1
  echo "rc=$?"
)"
grep -q "rc=0" <<<"$out2" || {
  echo "relay-agent: FAIL -- blocked exit not 0" >&2
  exit 1
}
args2="$(tr '\0' '\n' <"$tmp/args")"
grep -qx -- "blocked" <<<"$args2" || {
  echo "relay-agent: FAIL -- blocked state not passed" >&2
  exit 1
}
grep -qx -- "waiting for input" <<<"$args2" || {
  echo "relay-agent: FAIL -- blocked detail (.message) not passed" >&2
  exit 1
}
# LEAD: a long multi-sentence reply is trimmed at a sentence boundary, not mid-word
long='Alpha sentence is complete and padded out here to be longer. Beta sentence is likewise complete and padded out very similarly. Gamma sentence also rounds things out with a bit of padding. Delta sentence pushes the running total up near the limit now. Epsilon sentence is far beyond the budget and must never appear.'
: >"$tmp/args"
printf '%s\n' "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"$long\"}]}}" >"$tmp/t3.jsonl"
printf '{"cwd":"/x/dotfiles","transcript_path":"%s/t3.jsonl"}' "$tmp" |
  PATH="$tmp:$PATH" RELAY_BIN="$tmp/relay.sh" RELAY_ARGS_FILE="$tmp/args" CODEX_BIN=false \
    bash "$agent" "done" >/dev/null 2>&1
detail3="$(tr '\0' '\n' <"$tmp/args" | awk 'f{print; exit} $0=="--detail"{f=1}')"
[[ $detail3 == *. ]] || {
  echo "relay-agent: FAIL -- snippet not trimmed to a sentence boundary: '$detail3'" >&2
  exit 1
}
[[ $detail3 != *Epsilon* ]] || {
  echo "relay-agent: FAIL -- snippet kept the over-budget tail sentence" >&2
  exit 1
}
[[ $detail3 != *…* ]] || {
  echo "relay-agent: FAIL -- mid-word ellipsis instead of a clean sentence cut" >&2
  exit 1
}
[[ ${#detail3} -le 240 ]] || {
  echo "relay-agent: FAIL -- snippet over 240 chars (${#detail3})" >&2
  exit 1
}

# codex-primary: when CODEX_BIN yields STATE|SUMMARY, relay uses both (state may override 'done')
: >"$tmp/args"
cat >"$tmp/codexmock" <<'MOCK'
#!/usr/bin/env bash
printf 'asking|Need your decision on the API choice.\n'
MOCK
chmod +x "$tmp/codexmock"
printf '%s\n' '{"type":"assistant","message":{"content":[{"type":"text","text":"A long reply that happens to end by asking you something?"}]}}' >"$tmp/tc.jsonl"
printf '{"cwd":"/x/dotfiles","transcript_path":"%s/tc.jsonl"}' "$tmp" |
  PATH="$tmp:$PATH" RELAY_BIN="$tmp/relay.sh" RELAY_ARGS_FILE="$tmp/args" CODEX_BIN="$tmp/codexmock" \
    bash "$agent" "done" >/dev/null 2>&1
ca="$(tr '\0' '\n' <"$tmp/args")"
grep -qx -- "asking" <<<"$ca" || {
  echo "relay-agent: FAIL -- codex state not used (expected 'asking')" >&2
  exit 1
}
grep -qx -- "Need your decision on the API choice." <<<"$ca" || {
  echo "relay-agent: FAIL -- codex summary not used" >&2
  exit 1
}

# codex guard: on re-entry (RELAY_SUMMARIZING set) relay must NOT call codex, falling back instead
: >"$tmp/args"
cat >"$tmp/codexspy" <<'MOCK'
#!/usr/bin/env bash
printf 'codex-was-called' >>"$CODEX_SPY_FILE"
printf 'asking|should not be used\n'
MOCK
chmod +x "$tmp/codexspy"
printf '{"cwd":"/x/dotfiles","transcript_path":"%s/tc.jsonl"}' "$tmp" |
  PATH="$tmp:$PATH" RELAY_BIN="$tmp/relay.sh" RELAY_ARGS_FILE="$tmp/args" \
    CODEX_BIN="$tmp/codexspy" CODEX_SPY_FILE="$tmp/spy" RELAY_SUMMARIZING=1 \
    bash "$agent" "done" >/dev/null 2>&1
[[ -s "$tmp/spy" ]] && {
  echo "relay-agent: FAIL -- codex called on re-entry (loop guard missing)" >&2
  exit 1
}
echo "relay-agent: OK"
