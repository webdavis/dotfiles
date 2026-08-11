#!/usr/bin/env bats
# The pns HOOKS: harness event sources, as opposed to channels, which are
# destinations. The Claude pair implements one behavior between them, a light
# pulse when a session ran long, by writing a marker on the first prompt and
# reading it on stop.

setup() {
  PNS="$(cd "$BATS_TEST_DIRNAME/../.." && pwd)/dot_local/libexec/pns"
  export PNS_STATE_DIR="$BATS_TEST_TMPDIR/state"
  export PNS_HELPERS_DIR="$PNS/helpers"
  # A stub engine and a config, so a pulse records itself instead of touching
  # lights: the pulse is the engine's own subcommand behind the config gate.
  export PNS_ENGINE_BIN="$BATS_TEST_TMPDIR/pns"
  export PNS_CONFIG_FILE="$BATS_TEST_TMPDIR/config.toml"
  printf '#!/usr/bin/env bash\nprintf "%%s\\n" "$@" >"%s/pulsed"\n' "$BATS_TEST_TMPDIR" \
    >"$PNS_ENGINE_BIN"
  chmod +x "$PNS_ENGINE_BIN"
  printf '[plugins.hue]\nenabled = true\n' >"$PNS_CONFIG_FILE"
  START="$PNS/hooks/claude/executable_user-prompt-start.sh"
  STOP="$PNS/hooks/claude/executable_stop-pulse.sh"
}

payload() { jq -cn --arg s "$1" '{session_id: $s}'; }
marker() { printf '%s/session-%s.start' "$PNS_STATE_DIR" "$1"; }
pulsed() { [[ -f "$BATS_TEST_TMPDIR/pulsed" ]]; }
# A plain refute, because a bare `! cmd` only fails the test when it happens
# to be the body's LAST command.
refute() { if "$@"; then return 1; fi; }

@test "the first prompt writes a session marker" {
  payload abc123 | "$START"
  [ -f "$(marker abc123)" ]
}

@test "the DEFAULT marker location is under HOME, never shared /tmp" {
  # PNS_STATE_DIR unset on purpose: with the override in play this asserted a
  # path the test itself had built, which is true by construction and can never
  # fail. The behavior worth pinning is the hook's OWN default, so HOME is
  # redirected and the default is what gets exercised.
  unset PNS_STATE_DIR
  HOME="$BATS_TEST_TMPDIR/home" bash -c "printf '%s' '$(payload abc123)' | $(printf '%q' "$START")"
  [ -f "$BATS_TEST_TMPDIR/home/.local/state/pns/session-abc123.start" ]
  [ ! -e "/tmp/claude-session-abc123-start" ]
}

@test "a later prompt in the same session does not reset the start time" {
  # A PLANTED value rather than a real first run plus a sleep. Sleeping to make
  # two timestamps differ is what the unit suite bans, and it would have made
  # this the slowest test in the file for no added confidence: an unchanged
  # marker is the behavior, and a value the clock could never produce proves it
  # more sharply than one that merely might differ.
  mkdir -p "$PNS_STATE_DIR"
  printf '111' >"$(marker abc123)"
  payload abc123 | "$START"
  [ "$(cat "$(marker abc123)")" = 111 ]
}

@test "a session id carrying a path traversal is refused, not turned into a filename" {
  payload '../../escaped' | "$START"
  [ ! -e "$BATS_TEST_TMPDIR/escaped" ]
  [ ! -e "$PNS_STATE_DIR/../../escaped" ]
}

@test "a payload with no session id is a silent no-op" {
  run bash -c "printf '{}' | $(printf '%q' "$START")"
  [ "$status" -eq 0 ]
  [ -z "$(ls -A "$PNS_STATE_DIR" 2>/dev/null || true)" ]
}

@test "stopping after a LONG session pulses the lights green" {
  mkdir -p "$PNS_STATE_DIR"
  printf '%s' "$(($(date +%s) - 600))" >"$(marker abc123)"
  payload abc123 | "$STOP"
  pulsed
  # The engine's own subcommand, exit code included.
  [ "$(cat "$BATS_TEST_TMPDIR/pulsed")" = "pulse
0" ]
}

@test "stopping after a SHORT session does not pulse" {
  mkdir -p "$PNS_STATE_DIR"
  printf '%s' "$(($(date +%s) - 5))" >"$(marker abc123)"
  payload abc123 | "$STOP"
  ! pulsed
}

@test "stopping consumes the marker, so a second stop cannot re-pulse" {
  mkdir -p "$PNS_STATE_DIR"
  printf '%s' "$(($(date +%s) - 600))" >"$(marker abc123)"
  payload abc123 | "$STOP"
  rm -f "$BATS_TEST_TMPDIR/pulsed"
  payload abc123 | "$STOP"
  ! pulsed
}

@test "stopping a session that was never started is a silent no-op" {
  run bash -c "$(printf '%q' "$(command -v jq)") --version >/dev/null; printf '%s' '$(payload never-seen)' | $(printf '%q' "$STOP")"
  [ "$status" -eq 0 ]
  ! pulsed
}

@test "a corrupt marker DECLINES rather than crashing the hook" {
  # Both halves matter. Before the guard, a non-numeric marker aborted the hook
  # under `set -u` ("not: unbound variable"), so it did not pulse for the wrong
  # reason: it crashed. A hook that exits non-zero is noise the harness
  # reports, so the exit status is as much the behavior as the missing pulse.
  mkdir -p "$PNS_STATE_DIR"
  printf 'not-a-timestamp' >"$(marker abc123)"
  run bash -c "printf '%s' '$(payload abc123)' | $(printf '%q' "$STOP")"
  [ "$status" -eq 0 ]
  ! pulsed
}

@test "a corrupt marker is still consumed, so it cannot wedge every later stop" {
  mkdir -p "$PNS_STATE_DIR"
  printf 'not-a-timestamp' >"$(marker abc123)"
  run bash -c "printf '%s' '$(payload abc123)' | $(printf '%q' "$STOP")"
  [ ! -f "$(marker abc123)" ]
}

# --- the engine binary form -------------------------------------------------
# The bash channel above is a ONE-element pulse, so it cannot see the binary
# form's subcommand at all. These pin the branch the repoint exists to add.

@test "a long session pulses through the binary's own subcommand, exit code included" {
  payload bin1 | "$START"
  printf '%s' "$(($(date +%s) - 400))" >"$(marker bin1)"
  payload bin1 | "$STOP"
  # Three arguments, in order: dropping the subcommand would run the ENGINE
  # with a bare "0" instead of pulsing anything.
  run cat "$BATS_TEST_TMPDIR/pulsed"
  [ "$output" = "pulse
0" ]
}

@test "with no engine installed the hook is still a silent exit 0" {
  # A hook that exits non-zero here is the one thing it must never do.
  rm -f "$PNS_ENGINE_BIN"
  payload none1 | "$START"
  printf '%s' "$(($(date +%s) - 400))" >"$(marker none1)"
  run "$STOP" <<<"$(payload none1)"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "a missing config is a silent no-pulse, exit 0" {
  # The config gate: without it the engine's pulse mode would silently do
  # nothing anyway, so the hook says the same by doing nothing.
  rm -f "$PNS_CONFIG_FILE"
  mkdir -p "$PNS_STATE_DIR"
  printf '%s' "$(($(date +%s) - 400))" >"$(marker none2)"
  run "$STOP" <<<"$(payload none2)"
  [ "$status" -eq 0 ]
}
