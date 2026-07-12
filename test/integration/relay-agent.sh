#!/usr/bin/env bash
# relay-agent.sh: maps a hook payload + state to relay.sh args (project, pane, done-snippet).
set -uo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
agent="$root/dot_local/bin/executable_relay-agent.sh"
[[ -x $agent ]] || {
  echo "relay-agent: FAIL -- not executable" >&2
  exit 1
}
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
export RELAY_CODEX_HOME="$tmp/codex-home" # keep tests off the real ~/.config/relay/codex-home
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

# whole-turn + FIX F3 (prompt via stdin, never argv): codex receives ALL assistant text since the last
# user prompt, fed on STDIN so the transcript (up to 8000 chars) is never a ps-visible argv positional,
# and the invocation uses --ephemeral (no persisted session) with the `-` stdin positional.
: >"$tmp/args"
cat >"$tmp/codexcap" <<'MOCK'
#!/usr/bin/env bash
printf '%s\n' "$@" >"$CODEX_ARGV_FILE"
cat >"$CODEX_CAP_FILE"
printf 'done|captured\n'
MOCK
chmod +x "$tmp/codexcap"
cat >"$tmp/tw.jsonl" <<'JL'
{"type":"user","message":{"role":"user","content":[{"type":"text","text":"do the thing"}]}}
{"type":"assistant","message":{"content":[{"type":"text","text":"FIRSTBLOCK alpha"}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{}}]}}
{"type":"assistant","message":{"content":[{"type":"text","text":"SECONDBLOCK omega"}]}}
JL
printf '{"cwd":"/x/dotfiles","transcript_path":"%s/tw.jsonl"}' "$tmp" |
  PATH="$tmp:$PATH" RELAY_BIN="$tmp/relay.sh" RELAY_ARGS_FILE="$tmp/args" \
    CODEX_BIN="$tmp/codexcap" CODEX_CAP_FILE="$tmp/cap" CODEX_ARGV_FILE="$tmp/argv" \
    bash "$agent" "done" >/dev/null 2>&1
cap=""
[[ -f "$tmp/cap" ]] && cap="$(<"$tmp/cap")"
[[ $cap == *FIRSTBLOCK* && $cap == *SECONDBLOCK* ]] || {
  echo "relay-agent: FAIL -- whole-turn prompt (stdin) missing a block (got: ${cap:0:80})" >&2
  exit 1
}
argv=""
[[ -f "$tmp/argv" ]] && argv="$(<"$tmp/argv")"
[[ $argv == *FIRSTBLOCK* || $argv == *SECONDBLOCK* ]] && {
  echo "relay-agent: FAIL -- transcript text leaked onto codex argv (ps-visible)" >&2
  exit 1
}
grep -qx -- "--ephemeral" <<<"$argv" || {
  echo "relay-agent: FAIL -- codex not invoked with --ephemeral (session files would persist)" >&2
  exit 1
}
grep -qx -- "-" <<<"$argv" || {
  echo "relay-agent: FAIL -- codex not invoked with the '-' stdin positional" >&2
  exit 1
}

# FIX 2 (whole-file jq -rs slurp -> line-by-line): a single half-written trailing JSONL
# line must not discard the whole summary. The last valid assistant reply is still
# extracted, skipping the unterminated final line. CODEX_BIN=false -> python trim path.
: >"$tmp/args"
{
  printf '%s\n' '{"type":"assistant","message":{"content":[{"type":"text","text":"COMPLETE reply survives."}]}}'
  printf '%s' '{"type":"assistant","message":{"content":[{"type":"text","text":"truncated line no clos'
} >"$tmp/ttrunc.jsonl"
printf '{"cwd":"/x/dotfiles","transcript_path":"%s/ttrunc.jsonl"}' "$tmp" |
  PATH="$tmp:$PATH" RELAY_BIN="$tmp/relay.sh" RELAY_ARGS_FILE="$tmp/args" CODEX_BIN=false \
    bash "$agent" "done" >/dev/null 2>&1
trunc_detail="$(tr '\0' '\n' <"$tmp/args" | awk 'f{print; exit} $0=="--detail"{f=1}')"
[[ $trunc_detail == *"COMPLETE reply survives"* ]] || {
  echo "relay-agent: FAIL -- a truncated last JSONL line discarded the whole summary: '$trunc_detail'" >&2
  exit 1
}

# FIX F7 (scalar JSON line must not kill the parse): a line that is valid JSON but a bare SCALAR (e.g. a
# quoted string) survives fromjson? and then .type indexing on a string hard-errors, discarding the whole
# summary. select(type=="object") drops non-object lines so the last valid assistant reply still survives.
: >"$tmp/args"
{
  printf '%s\n' '{"type":"assistant","message":{"content":[{"type":"text","text":"SCALAR-SURVIVOR reply."}]}}'
  printf '%s\n' '"a bare json string line"'
} >"$tmp/tscalar.jsonl"
printf '{"cwd":"/x/dotfiles","transcript_path":"%s/tscalar.jsonl"}' "$tmp" |
  PATH="$tmp:$PATH" RELAY_BIN="$tmp/relay.sh" RELAY_ARGS_FILE="$tmp/args" CODEX_BIN=false \
    bash "$agent" "done" >/dev/null 2>&1
scalar_detail="$(tr '\0' '\n' <"$tmp/args" | awk 'f{print; exit} $0=="--detail"{f=1}')"
[[ $scalar_detail == *"SCALAR-SURVIVOR"* ]] || {
  echo "relay-agent: FAIL -- a bare scalar JSON line killed the parse and discarded the summary: '$scalar_detail'" >&2
  exit 1
}

# CHARACTERIZATION (baseline quirk, retained; SP3 owns): the last-user-turn boundary
# detection only recognizes a user entry whose content is a string OR whose FIRST
# content block is type=="text". A user turn that is purely a tool_result (content[0]
# .type=="tool_result") is NOT treated as a boundary, so assistant text from BEFORE
# that tool_result bleeds into the "last turn" summary. Pin current behavior: the
# pre-tool_result assistant text (BLEED) shows up in the snippet. CODEX_BIN=false so
# the python trim path (not codex) produces the detail.
: >"$tmp/args"
cat >"$tmp/tb.jsonl" <<'JL'
{"type":"user","message":{"content":[{"type":"text","text":"first question"}]}}
{"type":"assistant","message":{"content":[{"type":"text","text":"BLEED old answer."}]}}
{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"x","content":"result"}]}}
{"type":"assistant","message":{"content":[{"type":"text","text":"FRESH new answer."}]}}
JL
printf '{"cwd":"/x/dotfiles","transcript_path":"%s/tb.jsonl"}' "$tmp" |
  PATH="$tmp:$PATH" RELAY_BIN="$tmp/relay.sh" RELAY_ARGS_FILE="$tmp/args" CODEX_BIN=false \
    bash "$agent" "done" >/dev/null 2>&1
tb_detail="$(tr '\0' '\n' <"$tmp/args" | awk 'f{print; exit} $0=="--detail"{f=1}')"
[[ $tb_detail == *BLEED* ]] || {
  echo "relay-agent: FAIL -- tool_result boundary quirk drifted (BLEED no longer bleeds in): '$tb_detail'" >&2
  exit 1
}

# FIX F3 (private relay Codex home): the home symlinks the live Codex auth, so it must be created
# owner-only (umask 077) -- never group/other readable. A freshly created home is checked here.
: >"$tmp/args"
rm -rf "$tmp/codex-home"
printf '{"cwd":"/x/dotfiles","transcript_path":"%s/t.jsonl"}' "$tmp" |
  PATH="$tmp:$PATH" RELAY_BIN="$tmp/relay.sh" RELAY_ARGS_FILE="$tmp/args" CODEX_BIN=false \
    bash "$agent" "done" >/dev/null 2>&1
perms="$(stat -f '%Sp' "$tmp/codex-home" 2>/dev/null || stat -c '%A' "$tmp/codex-home" 2>/dev/null || true)"
[[ $perms == drwx------ ]] || {
  echo "relay-agent: FAIL -- relay Codex home is not owner-only (perms: '$perms')" >&2
  exit 1
}
cfg_perms="$(stat -f '%Sp' "$tmp/codex-home/config.toml" 2>/dev/null || stat -c '%A' "$tmp/codex-home/config.toml" 2>/dev/null || true)"
[[ $cfg_perms == -rw------- ]] || {
  echo "relay-agent: FAIL -- relay Codex config.toml is not owner-only (perms: '$cfg_perms')" >&2
  exit 1
}

# CHARACTERIZATION (DEFERRED to SP3, pin only): today's extractor reads Claude-shaped assistant records
# (.type=="assistant" | .message.content[] text). A current Codex rollout stores assistant output as
# response_item records under .payload, which the extractor does not read, so a Codex-format transcript
# yields an EMPTY summary and the generic (detail-less) notification still fires, exit 0. SP3 owns the
# schema work; this pins the baseline so SP3 replaces a known behavior. CODEX_BIN=false (empty reply also
# skips the codex-summarize path).
: >"$tmp/args"
cat >"$tmp/tcodex.jsonl" <<'JL'
{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"codex did work under the new schema"}]}}
JL
out_codexschema="$(
  printf '{"cwd":"/x/dotfiles","transcript_path":"%s/tcodex.jsonl"}' "$tmp" |
    PATH="$tmp:$PATH" RELAY_BIN="$tmp/relay.sh" RELAY_ARGS_FILE="$tmp/args" CODEX_BIN=false \
      bash "$agent" "done" 2>&1
  echo "rc=$?"
)"
grep -q "rc=0" <<<"$out_codexschema" || {
  echo "relay-agent: FAIL -- a Codex-schema transcript broke exit 0" >&2
  exit 1
}
cs_args="$(tr '\0' '\n' <"$tmp/args")"
grep -qx -- "--detail" <<<"$cs_args" && {
  echo "relay-agent: FAIL -- Codex response_item schema quirk drifted (extractor now reads .payload)" >&2
  exit 1
}
echo "relay-agent: OK"
