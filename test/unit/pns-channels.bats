#!/usr/bin/env bats
# The pns channels, each driven with a stubbed delivery binary.
#
# These spawn the channel scripts, because a channel IS a process in this
# architecture: the plugin contract is "an executable taking a JSON event on
# stdin", so testing it any other way would test something else. Nothing here
# touches a network, a key, or a clock; curl and terminal-notifier are stubs
# that record their arguments.

setup() {
  PNS="$(cd "$BATS_TEST_DIRNAME/../.." && pwd)/dot_local/libexec/pns"
  STUBS="$BATS_TEST_TMPDIR/bin"
  mkdir -p "$STUBS"
  # curl stub: record argv and stdin, then print whatever the test asked for.
  cat >"$STUBS/curl" <<STUB
#!/usr/bin/env bash
printf '%s\n' "\$*" >>"$BATS_TEST_TMPDIR/curl.argv"
cat >>"$BATS_TEST_TMPDIR/curl.body"
[[ -n \${STUB_HTTP_CODE:-} ]] && printf '%s' "\$STUB_HTTP_CODE"
exit \${STUB_CURL_RC:-0}
STUB
  cat >"$STUBS/terminal-notifier" <<STUB
#!/usr/bin/env bash
printf '%s\n' "\$*" >"$BATS_TEST_TMPDIR/notifier.argv"
STUB
  chmod +x "$STUBS"/*
  PATH="$STUBS:$PATH"
  export PATH
  AUTH="$BATS_TEST_TMPDIR/auth.json"
  printf '{"moshi_secret":"m-tok","hermes_secret":"h-key"}\n' >"$AUTH"
  export RELAY_AUTH_FILE="$AUTH"
  export RELAY_MOSHI_URL="https://example.invalid/moshi"
  export RELAY_HERMES_URL="https://example.invalid/relay"
}

# event <mode> [pane]: one JSON event on stdin, as the engine builds it.
event() {
  jq -cn --arg m "${1:-async}" --arg p "${2:-}" \
    '{agent:"claude", state:"done", project:"dotfiles", branch:"main",
      detail:"a summary", title:"claude · done · dotfiles",
      message:"(main) the full untrimmed summary",
      preview:"(main) the trimmed preview",
      pane:$p, mode:$m}'
}

# --- moshi -----------------------------------------------------------------

@test "moshi posts the token and the PREVIEW, never the full message" {
  event async | "$PNS/channels/executable_moshi.sh"
  run jq -e '.token == "m-tok" and .message == "(main) the trimmed preview"' "$BATS_TEST_TMPDIR/curl.body"
  [ "$status" -eq 0 ]
}

@test "moshi sends the body on stdin, never on argv, so no token reaches the process table" {
  event async | "$PNS/channels/executable_moshi.sh"
  run grep -q 'm-tok' "$BATS_TEST_TMPDIR/curl.argv"
  [ "$status" -ne 0 ]
}

@test "moshi is silently unavailable when the file holds no token" {
  printf '{"hermes_secret":"h-key"}\n' >"$AUTH"
  run bash -c "$(printf '%q' "$(command -v jq)") --version >/dev/null; $(printf '%q' "$PNS/channels/executable_moshi.sh") <<<'$(event async)'"
  [ "$status" -eq 0 ]
  [ ! -f "$BATS_TEST_TMPDIR/curl.body" ]
}

@test "moshi exits 0 even when delivery fails" {
  STUB_CURL_RC=7 run bash -c "$(printf '%q' "$PNS/channels/executable_moshi.sh") <<<'$(event async)'"
  [ "$status" -eq 0 ]
}

# --- hermes ----------------------------------------------------------------

@test "hermes signs the body and carries the FULL message, not the preview" {
  event async | "$PNS/channels/executable_hermes.sh"
  run grep -q 'X-Webhook-Signature' "$BATS_TEST_TMPDIR/curl.argv"
  [ "$status" -eq 0 ]
  run jq -e '.detail == "(main) the full untrimmed summary" and .agent == "claude"' "$BATS_TEST_TMPDIR/curl.body"
  [ "$status" -eq 0 ]
}

@test "hermes signs the SYNC post too, which has its own curl call" {
  STUB_HTTP_CODE=204 run bash -c "$(printf '%q' "$PNS/channels/executable_hermes.sh") <<<'$(event sync)'"
  run grep -q 'X-Webhook-Signature' "$BATS_TEST_TMPDIR/curl.argv"
  [ "$status" -eq 0 ]
}

@test "hermes never puts the signing key on argv" {
  event async | "$PNS/channels/executable_hermes.sh"
  run grep -q 'h-key' "$BATS_TEST_TMPDIR/curl.argv"
  [ "$status" -ne 0 ]
}

@test "hermes says nothing on the async path" {
  run bash -c "$(printf '%q' "$PNS/channels/executable_hermes.sh") <<<'$(event async)'"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

@test "hermes reports a 2xx on the sync path" {
  STUB_HTTP_CODE=204 run bash -c "$(printf '%q' "$PNS/channels/executable_hermes.sh") <<<'$(event sync)'"
  [[ "$output" == *"posted HTTP 204"* ]]
}

@test "hermes reports a 401 as a FAILURE, because a swallowed one empties the channel silently" {
  STUB_HTTP_CODE=401 run bash -c "$(printf '%q' "$PNS/channels/executable_hermes.sh") <<<'$(event sync)'"
  [[ "$output" == *"FAILED HTTP 401"* ]]
}

@test "hermes names the gateway when no response arrived at all" {
  STUB_HTTP_CODE=000 run bash -c "$(printf '%q' "$PNS/channels/executable_hermes.sh") <<<'$(event sync)'"
  [[ "$output" == *"is the hermes gateway up"* ]]
}

@test "hermes reports an empty status rather than passing over it" {
  run bash -c "$(printf '%q' "$PNS/channels/executable_hermes.sh") <<<'$(event sync)'"
  [[ "$output" == *"no HTTP status at all"* ]]
}

@test "hermes SAYS it skipped when the signing key is missing, on the sync path" {
  printf '{"moshi_secret":"m-tok"}\n' >"$AUTH"
  run bash -c "$(printf '%q' "$PNS/channels/executable_hermes.sh") <<<'$(event sync)'"
  [ "$status" -eq 0 ]
  [[ "$output" == *"SKIPPED"* ]]
}

@test "hermes stays quiet about a missing key on the async path" {
  printf '{"moshi_secret":"m-tok"}\n' >"$AUTH"
  run bash -c "$(printf '%q' "$PNS/channels/executable_hermes.sh") <<<'$(event async)'"
  [ "$status" -eq 0 ]
  [ -z "$output" ]
}

# --- macos-banner ----------------------------------------------------------

@test "the banner's click switches to the pane's WORKSPACE before focusing the pane" {
  # One herdr server, many workspaces, one Ghostty window: `agent focus` alone
  # moves focus inside the pane's workspace while the SCREEN keeps showing
  # whichever workspace the operator was in, so a cross-workspace click went
  # nowhere (measured 2026-08-06). The workspace id is the pane id's prefix.
  # Both commands ride the click; nothing moves focus at notify time.
  event async 'wW:p21' | "$PNS/channels/executable_macos-banner.sh"
  run grep -q 'herdr workspace focus wW; herdr agent focus wW:p21' "$BATS_TEST_TMPDIR/notifier.argv"
  [ "$status" -eq 0 ]
}

@test "the banner still fires when there is no pane, with an inert execute" {
  event async '' | "$PNS/channels/executable_macos-banner.sh"
  [ -f "$BATS_TEST_TMPDIR/notifier.argv" ]
  run grep -q 'herdr agent focus' "$BATS_TEST_TMPDIR/notifier.argv"
  [ "$status" -ne 0 ]
}

@test "the banner exits 0 when terminal-notifier is not installed" {
  # Pins the OUTCOME, not the availability guard: with the guard removed the
  # call would still fail into `|| true`, so no test can tell the two apart
  # from outside. The guard is there to skip pointless work, not to change
  # behavior, and a mutation sweep is how that got said out loud.
  rm -f "$STUBS/terminal-notifier"
  run bash -c "$(printf '%q' "$PNS/channels/executable_macos-banner.sh") <<<'$(event async)'"
  [ "$status" -eq 0 ]
}
