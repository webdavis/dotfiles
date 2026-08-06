#!/usr/bin/env bats
# relay.sh is the pns ENGINE: it renders the event, decides which channels fire,
# and hands each one a JSON object on stdin. This pins that routing.
#
# WHY THIS EXISTS. relay.sh runs on every agent notification and had NO
# behavioral test when the channel extraction landed: test/e2e/relay.sh,
# test/unit/relay-remote-only.sh and test/relay-hermes-route.sh all went with
# the 2026-08-05 purge for being slow. Stub channels make the same assertions
# fast, because nothing here needs a network, a key, or a sleep.

setup() {
  REPO_ROOT="$(cd "$BATS_TEST_DIRNAME/../.." && pwd)"
  RELAY="$REPO_ROOT/dot_local/libexec/pns/executable_relay.sh"
  CHANNELS="$BATS_TEST_TMPDIR/channels"
  mkdir -p "$CHANNELS"
  # Stub channels record the event they were handed, then exit 0.
  local name
  for name in moshi hermes macos-banner; do
    printf '#!/usr/bin/env bash\ncat >"%s/%s.event"\n' "$BATS_TEST_TMPDIR" "$name" >"$CHANNELS/$name.sh"
    chmod +x "$CHANNELS/$name.sh"
  done
}

# relay <args...>: fire the engine against the stubs, away from the desk.
relay() {
  rm -f "$BATS_TEST_TMPDIR"/*.event
  PNS_CHANNELS_DIR="$CHANNELS" RELAY_IDLE_SECS=99999 \
    "$RELAY" "$@" >"$BATS_TEST_TMPDIR/out" 2>"$BATS_TEST_TMPDIR/err"
}

fired() { [[ -f "$BATS_TEST_TMPDIR/$1.event" ]]; }
refute_fired() { [[ ! -f "$BATS_TEST_TMPDIR/$1.event" ]]; }
mode_of() { jq -r '.mode' <"$BATS_TEST_TMPDIR/$1.event"; }

@test "the alert path reaches every channel" {
  relay --agent claude --state done --project dotfiles --detail 'a summary'
  fired moshi
  fired hermes
  fired macos-banner
}

@test "hermes is async on the alert path, so delivery stays off the caller's critical path" {
  relay --agent claude --state done --detail x
  [ "$(mode_of hermes)" = async ]
}

@test "a channel is handed the RENDERED event, not the raw arguments" {
  relay --agent claude --state done --project dotfiles --branch main --detail 'a summary'
  run jq -e '.title != "" and .message != "" and .preview != "" and .agent == "claude"' \
    "$BATS_TEST_TMPDIR/moshi.event"
  [ "$status" -eq 0 ]
}

@test "--local-only keeps the banner and reaches nothing off the machine" {
  relay --agent claude --state done --detail x --local-only
  fired macos-banner
  refute_fired moshi
  refute_fired hermes
}

@test "--remote-only delivers through hermes alone" {
  relay --agent weekly --state done --project skills --detail ran --remote-only
  fired hermes
  refute_fired moshi
  refute_fired macos-banner
}

@test "hermes is SYNC on the log path, which is what makes an undelivered entry visible" {
  relay --agent weekly --state done --detail ran --remote-only
  [ "$(mode_of hermes)" = sync ]
}

@test "both narrowing flags together deliver nothing and say so" {
  relay --agent x --state done --detail y --local-only --remote-only
  refute_fired hermes
  run grep -q SKIPPED "$BATS_TEST_TMPDIR/out"
  [ "$status" -eq 0 ]
}

@test "at the desk the phone is skipped and ONLY the phone" {
  rm -f "$BATS_TEST_TMPDIR"/*.event
  PNS_CHANNELS_DIR="$CHANNELS" RELAY_IDLE_SECS=0 \
    "$RELAY" --agent claude --state done --detail x >/dev/null 2>&1
  refute_fired moshi
  fired hermes
  fired macos-banner
}

@test "RELAY_SKIP_PHONE drops the phone and ONLY the phone" {
  # The caller (hooks/relay-agent.sh forwarding a blocking event) has already
  # raised the card on the phone through moshi-hook's own round trip, so the
  # push here would be the same event twice; the banner and the paper trail
  # are still wanted.
  rm -f "$BATS_TEST_TMPDIR"/*.event
  PNS_CHANNELS_DIR="$CHANNELS" RELAY_IDLE_SECS=99999 RELAY_SKIP_PHONE=1 \
    "$RELAY" --agent claude --state blocked --detail x >/dev/null 2>&1
  refute_fired moshi
  fired hermes
  fired macos-banner
}

@test "RELAY_SKIP_PHONE beats RELAY_FORCE_PHONE" {
  # "I have already sent it" is more specific than a standing override, and the
  # override is the one thing that could reintroduce the double push.
  rm -f "$BATS_TEST_TMPDIR"/*.event
  PNS_CHANNELS_DIR="$CHANNELS" RELAY_IDLE_SECS=0 RELAY_SKIP_PHONE=1 RELAY_FORCE_PHONE=1 \
    "$RELAY" --agent claude --state blocked --detail x >/dev/null 2>&1
  refute_fired moshi
}

@test "RELAY_FORCE_PHONE overrides presence" {
  rm -f "$BATS_TEST_TMPDIR"/*.event
  PNS_CHANNELS_DIR="$CHANNELS" RELAY_IDLE_SECS=0 RELAY_FORCE_PHONE=1 \
    "$RELAY" --agent claude --state done --detail x >/dev/null 2>&1
  fired moshi
}

@test "a channel that fails neither fails the caller nor suppresses its siblings" {
  printf '#!/usr/bin/env bash\nexit 9\n' >"$CHANNELS/moshi.sh"
  chmod +x "$CHANNELS/moshi.sh"
  relay --agent claude --state done --detail x
  fired hermes
  fired macos-banner
}

@test "an absent channel is simply not installed" {
  rm -f "$CHANNELS/hermes.sh"
  relay --agent claude --state done --detail x
  fired macos-banner
}
