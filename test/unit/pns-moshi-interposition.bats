#!/usr/bin/env bats
# moshi-hook interposition: pns registers as the harness hook and FORWARDS to
# moshi-hook rather than competing with it, so the phone keeps its approve/deny
# round trip while pns keeps the presence gating moshi has none of.
#
# WHAT IS PINNED HERE IS THE PIPE. moshi-hook itself, the harness registrations
# and the chezmoi scripts that write them are third-party and deployment, and
# nothing below touches them: a stub named moshi-hook records the argv and the
# stdin it was handed and emits a scripted stdout and exit code.
#
# WHY THESE SPAWN. The whole contract is a process boundary (stdin in, stdout
# and exit code out, unmodified), so a sourced function would be testing
# something else. The runs happen ONCE in setup_file, concurrently, and each
# test reads a recording; the pattern and the reason are pns-relay-agent.bats's.

setup_file() {
  T="$BATS_FILE_TMPDIR"
  export T
  PNS="$BATS_TEST_DIRNAME/../../dot_local/libexec/pns"
  export GATE="$PNS/hooks/executable_moshi-gate.sh"
  export HOOK="$PNS/hooks/executable_relay-agent.sh"
  # A hostile environment must not reach either script: these are the seams
  # that repoint the decision core, the gate and the summarizer.
  unset PNS_HELPERS_DIR PNS_MOSHI_GATE RELAY_AGENT CODEX_BIN RELAY_CODEX_HOME
  unset RELAY_FORCE_PHONE RELAY_DESK_IDLE_SECS
  export RELAY_SUMMARIZING=1
  export PNS_MOSHI_GATE="$GATE"

  mkdir -p "$T/bin"
  # The moshi-hook stub: argv NUL separated, stdin verbatim, scripted reply.
  cat >"$T/bin/moshi-hook" <<'STUB'
#!/usr/bin/env bash
printf '%s\0' "$@" >"$MOSHI_ARGV_OUT"
cat >"$MOSHI_STDIN_OUT"
[[ -n ${STUB_MOSHI_OUT:-} ]] && printf '%s' "$STUB_MOSHI_OUT"
exit "${STUB_MOSHI_RC:-0}"
STUB
  # relay.sh stands in as a stub recording its argv and whether the hook asked
  # it to stand down on the phone leg.
  cat >"$T/bin/relay-stub.sh" <<'STUB'
#!/usr/bin/env bash
printf '%s\0' "$@" >"$RELAY_ARGV_OUT"
printf '%s' "${RELAY_SKIP_PHONE:-}" >"$RELAY_SKIP_OUT"
STUB
  chmod +x "$T/bin/moshi-hook" "$T/bin/relay-stub.sh"
  export MOSHI_HOOK_BIN="$T/bin/moshi-hook"
  export RELAY_BIN="$T/bin/relay-stub.sh"

  # Payloads carrying bytes a re-encode would move: non-ASCII, an escaped
  # quote, a backslash. The gate's copy ends in a newline (it never reads the
  # stream, so the newline must survive); relay-agent's does not, because that
  # hook reads the payload with $(cat), which strips trailing newlines by
  # definition, and a fixture that pretended otherwise could only ever fail.
  printf '%s\n' '{"cwd":"/nowhere/webdavis/dotfiles","hook_event_name":"PermissionRequest","message":"brew \"install\" naïve C:\\path"}' >"$T/payload-nl.json"
  printf '%s' '{"cwd":"/nowhere/webdavis/dotfiles","hook_event_name":"PermissionRequest","message":"brew \"install\" naïve C:\\path"}' >"$T/payload.json"

  # --- the gate, driven directly, as pi and omp drive it -------------------
  gate away claude-hook RELAY_IDLE_SECS=99999 STUB_MOSHI_OUT='{"decision":"allow"}' &
  gate at_desk claude-hook RELAY_IDLE_SECS=0 &
  gate denied codex-hook RELAY_IDLE_SECS=99999 STUB_MOSHI_RC=2 &
  gate no_moshi claude-hook RELAY_IDLE_SECS=99999 MOSHI_HOOK_BIN="$T/bin/nothing-here" &
  gate bad_sub 'rm -rf /' RELAY_IDLE_SECS=99999 &

  # --- relay-agent, the hook the harnesses actually fire -------------------
  agent blocked_away blocked RELAY_IDLE_SECS=99999 STUB_MOSHI_RC=7 &
  agent done_away done RELAY_IDLE_SECS=99999 &
  agent blocked_no_moshi blocked RELAY_IDLE_SECS=99999 MOSHI_HOOK_BIN="$T/bin/nothing-here" &
  agent blocked_unknown blocked RELAY_IDLE_SECS=99999 RELAY_AGENT=weekly &
  wait
}

# gate <name> <subcommand> [VAR=VALUE...]: one gate run, recorded.
gate() {
  local name="$1" sub="$2" status=0
  shift 2
  env MOSHI_ARGV_OUT="$T/$name.argv" MOSHI_STDIN_OUT="$T/$name.stdin" "$@" \
    "$GATE" "$sub" <"$T/payload-nl.json" >"$T/$name.out" 2>"$T/$name.err" || status=$?
  printf '%s' "$status" >"$T/$name.status"
}

# agent <name> <state> [VAR=VALUE...]: one relay-agent run, recorded.
agent() {
  local name="$1" state="$2" status=0
  shift 2
  env MOSHI_ARGV_OUT="$T/$name.argv" MOSHI_STDIN_OUT="$T/$name.stdin" \
    RELAY_ARGV_OUT="$T/$name.relay.argv" RELAY_SKIP_OUT="$T/$name.skip" "$@" \
    "$HOOK" "$state" <"$T/payload.json" >"$T/$name.out" 2>"$T/$name.err" || status=$?
  printf '%s' "$status" >"$T/$name.status"
}

exit_status() { cat "$T/$1.status"; }
argv_of() { tr '\0' '\n' <"$T/$1.argv"; }

# refute_forwarded <name>: fail, loudly, if moshi was invoked at all. Written
# as a plain call rather than an inverted test because errexit and bats both
# pass over an inverted pipeline, which is how a refutation goes dead.
refute_forwarded() {
  if [[ -e "$T/$1.argv" ]]; then
    printf 'expected no moshi-hook call, got: %s\n' "$(argv_of "$1")" >&2
    return 1
  fi
  return 0
}

# relay_flag <name> <flag>: the value relay.sh was handed, or non-zero.
relay_flag() {
  local i
  local -a argv=()
  [[ -s "$T/$1.relay.argv" ]] || return 1
  mapfile -d '' -t argv <"$T/$1.relay.argv"
  for ((i = 0; i < ${#argv[@]}; i++)); do
    if [[ ${argv[i]} == "$2" ]]; then
      printf '%s' "${argv[i + 1]-}"
      return 0
    fi
  done
  return 1
}

# --- the gate: the pipe itself ----------------------------------------------

@test "the payload reaches moshi byte for byte, trailing newline included" {
  # THE central bug this design exists to avoid: a hook that reads the stream
  # and forgets to write it back leaves moshi parsing nothing, and moshi then
  # silently does nothing at all.
  run cmp "$T/payload-nl.json" "$T/away.stdin"
  [ "$status" -eq 0 ]
}

@test "moshi is invoked with the harness subcommand it was handed" {
  [ "$(argv_of away)" = "claude-hook" ]
}

@test "moshi's stdout is passed through unmodified" {
  [ "$(cat "$T/away.out")" = '{"decision":"allow"}' ]
}

@test "moshi's exit code is passed through, because that is what carries the decision" {
  [ "$(exit_status denied)" = 2 ]
}

@test "at the keyboard moshi is never invoked, and the harness prompts as usual" {
  refute_forwarded at_desk
  [ "$(exit_status at_desk)" = 0 ]
}

@test "with moshi-hook not installed nothing is forwarded and the gate still exits 0" {
  refute_forwarded no_moshi
  [ "$(exit_status no_moshi)" = 0 ]
}

@test "a subcommand that is not a harness hook is never handed to moshi" {
  # moshi-hook's top-level positional is a PATH, so an unvetted word here is an
  # argument this repo does not own reaching a third-party binary's filesystem
  # argument. The gate is fed its subcommand by a generated extension file, and
  # that file is regenerated by an upgrade.
  refute_forwarded bad_sub
  [ "$(exit_status bad_sub)" = 0 ]
}

# --- relay-agent: the same pipe, after the hook has read the payload --------

@test "a blocking event forwards the payload moshi's way even though the hook already read it" {
  run cmp "$T/payload.json" "$T/blocked_away.stdin"
  [ "$status" -eq 0 ]
}

@test "a blocking event returns moshi's exit code to the harness" {
  [ "$(exit_status blocked_away)" = 7 ]
}

@test "a blocking event names the harness it came from as moshi's subcommand" {
  [ "$(argv_of blocked_away)" = "claude-hook" ]
}

@test "the notification still goes out while moshi holds the approval card" {
  [ "$(relay_flag blocked_away --state)" = blocked ]
}

@test "pns skips its own phone push when moshi is carrying the card, so the phone gets one" {
  [ "$(cat "$T/blocked_away.skip")" = 1 ]
}

@test "a non-blocking event never pays for the round trip" {
  refute_forwarded done_away
  [ "$(exit_status done_away)" = 0 ]
}

@test "with moshi-hook not installed the hook still delivers its own phone push, and exits 0" {
  refute_forwarded blocked_no_moshi
  [ "$(exit_status blocked_no_moshi)" = 0 ]
  [ "$(relay_flag blocked_no_moshi --state)" = blocked ]
  [ "$(cat "$T/blocked_no_moshi.skip")" = "" ]
}

@test "a harness pns does not know is never handed to moshi as a subcommand" {
  # RELAY_AGENT names the harness and reaches this hook from a config file, so
  # an unknown value must not become `moshi-hook <value>-hook`.
  refute_forwarded blocked_unknown
  [ "$(exit_status blocked_unknown)" = 0 ]
}
