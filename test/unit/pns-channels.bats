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
  # herdr stub: answers `pane list` with one focused pane, HERDR_STUB_FOCUSED.
  # Without it every banner test would consult the OPERATOR'S live herdr, and
  # a test's verdict would depend on which pane they happen to be reading.
  cat >"$STUBS/herdr" <<'STUB'
#!/usr/bin/env bash
[[ -n ${HERDR_STUB_FOCUSED:-} ]] || exit 1
printf '{"result":{"panes":[{"pane_id":"%s","focused":true},{"pane_id":"zz:p0","focused":false}]}}\n' "$HERDR_STUB_FOCUSED"
STUB
  # pgrep + nettop stubs: the real probe samples the operator's live network
  # counters for a full second, so no test here may reach it. nettop records
  # that it was called, which is what pins the probe to the one idle band where
  # it can change a verdict.
  cat >"$STUBS/pgrep" <<'STUB'
#!/usr/bin/env bash
[[ -n ${PGREP_STUB_PIDS:-} ]] || exit 1
printf '%s\n' "$PGREP_STUB_PIDS"
STUB
  # lsappinfo stub: `front` prints a fixed ASN, `info` a CFBundleIdentifier
  # line from LSAPPINFO_STUB_BUNDLE (exit 1 unset = lsappinfo cannot answer).
  cat >"$STUBS/lsappinfo" <<'STUB'
#!/usr/bin/env bash
case "$1" in
  front) [[ -n ${LSAPPINFO_STUB_BUNDLE:-} ]] || exit 1; printf 'ASN:0x0-0xtest:\n' ;;
  info) [[ -n ${LSAPPINFO_STUB_BUNDLE:-} ]] || exit 1; printf '"CFBundleIdentifier"="%s"\n' "$LSAPPINFO_STUB_BUNDLE" ;;
  *) exit 1 ;;
esac
STUB
  cat >"$STUBS/nettop" <<STUB
#!/usr/bin/env bash
printf '%s\n' "\$*" >>"$BATS_TEST_TMPDIR/nettop.argv"
printf '%s' "\${NETTOP_STUB_CSV:-}"
STUB
  chmod +x "$STUBS"/*
  PATH="$STUBS:$PATH"
  export PATH
  # A temp HOME: the phone-attention marker is a real file at a real path, and
  # a test must neither read the operator's nor plant one under it.
  HOME="$BATS_TEST_TMPDIR/home"
  export HOME
  export MARKER="$HOME/.local/state/pns/phone-attention.marker"
  mkdir -p "${MARKER%/*}"
  # The idle seam, defaulted so NO test reaches the real ioreg. Unset, the
  # probe reads the operator's live HIDIdleTime, which is both a live probe in
  # a unit test and a verdict that changes with whoever last touched the
  # keyboard. 999 is the FAIL-OPEN value (past any desk threshold, so the
  # banner fires), which is what an unknown idle already means here; a test
  # that needs the operator present overrides it per case.
  export RELAY_IDLE_SECS=999
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
  # The ABSOLUTE path: the click runs in a bare launchd context whose PATH
  # cannot resolve herdr, so a bare name dies silently (proven live 2026-08-07).
  run grep -q "$STUBS/herdr workspace focus wW; $STUBS/herdr agent focus wW:p21" "$BATS_TEST_TMPDIR/notifier.argv"
  [ "$status" -eq 0 ]
}

@test "the click activates the terminal the pane actually lives in, not a hardcoded one" {
  # The env assignment has to ride the side of the pipe that runs the CHANNEL:
  # a VAR=x prefix on `a | b` binds to a alone, so prefixing the event builder
  # leaves the channel reading the operator's real terminal id.
  __CFBundleIdentifier='test.terminal' \
    run bash -c "$(printf '%q' "$PNS/channels/executable_macos-banner.sh") <<<'$(event async wW:p21)'"
  run grep -q -- '-activate test.terminal' "$BATS_TEST_TMPDIR/notifier.argv"
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

@test "a banner for the pane the operator is WATCHING is suppressed" {
  # ALL THREE presence conditions hold here: recently-touched Mac, the
  # terminal is the key window, and the event's pane is herdr's focused pane.
  # Every other suppression test below knocks out exactly one of the three.
  HERDR_STUB_FOCUSED='wW:p21' LSAPPINFO_STUB_BUNDLE='test.terminal' __CFBundleIdentifier='test.terminal' RELAY_IDLE_SECS=5 \
    run bash -c "$(printf '%q' "$PNS/channels/executable_macos-banner.sh") <<<'$(event async wW:p21)'"
  [ "$status" -eq 0 ]
  [ ! -f "$BATS_TEST_TMPDIR/notifier.argv" ]
}

@test "another app as the key window fires the banner, even with the pane focused" {
  # run bash -c, not an env-prefixed pipeline: a VAR=x prefix on `a | b` binds
  # to a only, and the channel on the right would fail-open for the wrong
  # reason (an unset herdr stub) while the test stays green by luck.
  HERDR_STUB_FOCUSED='wW:p21' LSAPPINFO_STUB_BUNDLE='company.thebrowser.Browser' __CFBundleIdentifier='test.terminal' RELAY_IDLE_SECS=5 \
    run bash -c "$(printf '%q' "$PNS/channels/executable_macos-banner.sh") <<<'$(event async wW:p21)'"
  [ -f "$BATS_TEST_TMPDIR/notifier.argv" ]
}

@test "an operator idle past the threshold fires the banner, watching or not" {
  HERDR_STUB_FOCUSED='wW:p21' LSAPPINFO_STUB_BUNDLE='test.terminal' __CFBundleIdentifier='test.terminal' RELAY_IDLE_SECS=999 \
    run bash -c "$(printf '%q' "$PNS/channels/executable_macos-banner.sh") <<<'$(event async wW:p21)'"
  [ -f "$BATS_TEST_TMPDIR/notifier.argv" ]
}

@test "an lsappinfo that cannot answer fails OPEN: the banner fires" {
  HERDR_STUB_FOCUSED='wW:p21' __CFBundleIdentifier='test.terminal' RELAY_IDLE_SECS=5 \
    run bash -c "$(printf '%q' "$PNS/channels/executable_macos-banner.sh") <<<'$(event async wW:p21)'"
  [ -f "$BATS_TEST_TMPDIR/notifier.argv" ]
}

@test "no known terminal identity fails OPEN: the banner fires" {
  # env -u: the test itself runs under a terminal, so the inherited value must
  # be stripped, not merely left alone, to model a headless-started herdr.
  HERDR_STUB_FOCUSED='wW:p21' LSAPPINFO_STUB_BUNDLE='test.terminal' RELAY_IDLE_SECS=5 \
    run bash -c "printf '%s' '$(event async wW:p21)' | env -u __CFBundleIdentifier $(printf '%q' "$PNS/channels/executable_macos-banner.sh")"
  [ "$status" -eq 0 ]
  [ -f "$BATS_TEST_TMPDIR/notifier.argv" ]
}

@test "PNS_TERMINAL_BUNDLE_ID beats the inherited terminal identity" {
  # Override says the terminal is the browser, and the browser IS front, so
  # this suppresses; with the inherited value winning instead it would fire.
  HERDR_STUB_FOCUSED='wW:p21' LSAPPINFO_STUB_BUNDLE='company.thebrowser.Browser' __CFBundleIdentifier='test.terminal' \
    PNS_TERMINAL_BUNDLE_ID='company.thebrowser.Browser' RELAY_IDLE_SECS=5 \
    run bash -c "$(printf '%q' "$PNS/channels/executable_macos-banner.sh") <<<'$(event async wW:p21)'"
  [ ! -f "$BATS_TEST_TMPDIR/notifier.argv" ]
}

@test "a banner for an UNfocused pane still fires" {
  HERDR_STUB_FOCUSED='wW:p99' event async 'wW:p21' | "$PNS/channels/executable_macos-banner.sh"
  [ -f "$BATS_TEST_TMPDIR/notifier.argv" ]
}

@test "a herdr that cannot answer fails OPEN: the banner fires" {
  event async 'wW:p21' | "$PNS/channels/executable_macos-banner.sh"
  [ -f "$BATS_TEST_TMPDIR/notifier.argv" ]
}
