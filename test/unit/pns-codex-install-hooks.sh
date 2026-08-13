#!/usr/bin/env bash
# relay-codex-hooks.sh: adds the engine's stop+blocked hooks to ~/.codex/hooks.json, preserves
# herdr's SessionStart, idempotent.
# -e included deliberately: without it an unguarded invocation below cannot
# fail this test, which is how a broken installer would pass.
set -euo pipefail
script="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/dot_local/libexec/pns/hooks/codex/executable_install-hooks.sh"
[[ -x $script ]] || {
  echo "relay-codex-hooks: FAIL -- not executable" >&2
  exit 1
}
home="$(mktemp -d)"
trap 'rm -rf "$home"' EXIT
mkdir -p "$home/.codex" "$home/.local/libexec/pns/hooks"
# fake engine so the script's -x guard passes
printf '#!/usr/bin/env bash\n' >"$home/.local/libexec/pns/pns"
chmod +x "$home/.local/libexec/pns/pns"
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
jq -e '[.hooks.Stop[]?.hooks[]?.command] | any(test("pns hook stop"))' <<<"$got" >/dev/null || {
  echo "relay-codex-hooks: FAIL -- relay done not added" >&2
  exit 1
}
jq -e '[.hooks.PermissionRequest[]?.hooks[]?.command] | any(test("pns hook blocked"))' <<<"$got" >/dev/null || {
  echo "relay-codex-hooks: FAIL -- relay blocked not added" >&2
  exit 1
}
# idempotent: a second run must not duplicate relay's Stop entry
HOME="$home" bash "$script"
n="$(jq '[.hooks.Stop[]?.hooks[]?.command | select(test("pns hook stop"))] | length' "$home/.codex/hooks.json")"
[[ $n -eq 1 ]] || {
  echo "relay-codex-hooks: FAIL -- not idempotent (Stop relay count=$n)" >&2
  exit 1
}
m="$(jq '[.hooks.PermissionRequest[]?.hooks[]?.command | select(test("pns hook blocked"))] | length' "$home/.codex/hooks.json")"
[[ $m -eq 1 ]] || {
  echo "relay-codex-hooks: FAIL -- not idempotent (PermissionRequest relay count=$m)" >&2
  exit 1
}

# FIX F5a (empty/whitespace hooks.json -> heal from default, never write a blank file): an empty file
# must NOT pass through the merge jq as rc=0-with-no-output and get written back blank. It heals from
# {"hooks":{}} and gains relay's two entries.
h5="$home/f5empty"
mkdir -p "$h5/.codex" "$h5/.local/libexec/pns/hooks"
printf '#!/usr/bin/env bash\n' >"$h5/.local/libexec/pns/pns"
chmod +x "$h5/.local/libexec/pns/pns"
: >"$h5/.codex/hooks.json"
HOME="$h5" bash "$script" >/dev/null 2>&1 || {
  echo "relay-codex-hooks: FAIL -- run errored on an empty hooks.json" >&2
  exit 1
}
jq -e 'type=="object" and (.hooks|type=="object")' "$h5/.codex/hooks.json" >/dev/null 2>&1 || {
  echo "relay-codex-hooks: FAIL -- empty input did not heal to a valid object (blank file written?)" >&2
  exit 1
}
jq -e '[.hooks.Stop[]?.hooks[]?.command] | any(test("pns hook stop"))' "$h5/.codex/hooks.json" >/dev/null || {
  echo "relay-codex-hooks: FAIL -- empty input healed but the relay done entry is missing" >&2
  exit 1
}

# FIX F5b (multiple JSON roots -> preserve untouched): a hooks.json with two concatenated object roots is
# malformed; the script warns and leaves the file byte-for-byte untouched, never writing two merged roots.
h6="$home/f6multi"
mkdir -p "$h6/.codex" "$h6/.local/libexec/pns/hooks"
printf '#!/usr/bin/env bash\n' >"$h6/.local/libexec/pns/pns"
chmod +x "$h6/.local/libexec/pns/pns"
printf '{"hooks":{}}{"hooks":{}}' >"$h6/.codex/hooks.json"
before6="$(cat "$h6/.codex/hooks.json")"
warn6="$(HOME="$h6" bash "$script" 2>&1 >/dev/null)"
rc6=$?
[[ $rc6 -eq 0 ]] || {
  echo "relay-codex-hooks: FAIL -- a multi-root file broke exit 0 (rc=$rc6)" >&2
  exit 1
}
[[ "$(cat "$h6/.codex/hooks.json")" == "$before6" ]] || {
  echo "relay-codex-hooks: FAIL -- a malformed multi-root file was modified instead of preserved" >&2
  exit 1
}
grep -qi "untouched" <<<"$warn6" || {
  echo "relay-codex-hooks: FAIL -- no warning for the malformed multi-root file" >&2
  exit 1
}

# FIX F6 (fresh-install Codex trust advisory): Codex ignores new/changed non-managed hooks until reviewed
# and trusted via /hooks. When a run ADDS or CHANGES a handler, the script loudly advises trusting via
# /hooks; an idempotent re-run (no content change) stays silent. It never synthesizes trust, never a bypass.
h7="$home/f7trust"
mkdir -p "$h7/.codex" "$h7/.local/libexec/pns/hooks"
printf '#!/usr/bin/env bash\n' >"$h7/.local/libexec/pns/pns"
chmod +x "$h7/.local/libexec/pns/pns"
cat >"$h7/.codex/hooks.json" <<'JSON'
{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"bash herdr-agent-state.sh session"}]}]}}
JSON
warn_add="$(HOME="$h7" bash "$script" 2>&1 >/dev/null)"
grep -qi "/hooks" <<<"$warn_add" || {
  echo "relay-codex-hooks: FAIL -- no /hooks trust advisory after adding relay handlers" >&2
  exit 1
}
grep -qi "trust" <<<"$warn_add" || {
  echo "relay-codex-hooks: FAIL -- the trust advisory does not mention trusting" >&2
  exit 1
}
warn_noop="$(HOME="$h7" bash "$script" 2>&1 >/dev/null)"
grep -qi "/hooks" <<<"$warn_noop" && {
  echo "relay-codex-hooks: FAIL -- the trust advisory repeated on an idempotent no-change re-run" >&2
  exit 1
}
echo "relay-codex-hooks: OK"
