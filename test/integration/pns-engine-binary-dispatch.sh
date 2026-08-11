#!/usr/bin/env bash
# The R2d differential gate as a standing check: the SAME dispatch assertions
# that pin the bash engine must pass against the Rust engine binary, so a
# broken or no-op binary fails the build instead of shipping behind a green
# bash-only suite. HOME is isolated so a live operator config cannot change
# the roster under the assertions.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

cargo build --quiet --manifest-path "$REPO_ROOT/dot_local/share/pns/Cargo.toml"

HOME="$scratch" PNS_RELAY_BIN="$REPO_ROOT/dot_local/share/pns/target/debug/pns" \
  bats "$REPO_ROOT/test/unit/pns-channel-dispatch.bats"

# Phase 2: native dispatch reachability. With no channels-dir override, the
# banner leg must deliver NATIVELY: the PATH-stubbed terminal-notifier records
# the spawn and a decoy executable channel must stay silent. This is the one
# branch the stub-driven bats above can never reach, because they set
# PNS_CHANNELS_DIR, the exact condition under which executables win by design.
stub_bin="$scratch/bin"
decoy_dir="$scratch/.local/libexec/pns/channels"
mkdir -p "$stub_bin" "$decoy_dir"
printf '#!/usr/bin/env bash\nprintf "%%s\\n" "$*" >"%s/notifier.args"\n' "$scratch" \
  >"$stub_bin/terminal-notifier"
printf '#!/usr/bin/env bash\ncat >"%s/decoy.event"\n' "$scratch" \
  >"$decoy_dir/macos-banner.sh"
chmod +x "$stub_bin/terminal-notifier" "$decoy_dir/macos-banner.sh"

env -u PNS_CHANNELS_DIR HOME="$scratch" PATH="$stub_bin:$PATH" RELAY_IDLE_SECS=99999 \
  "$REPO_ROOT/dot_local/share/pns/target/debug/pns" \
  --agent claude --state 'done' --detail x --local-only

[[ -f "$scratch/notifier.args" ]] || {
  echo "native banner did not spawn the notifier" >&2
  exit 1
}
grep -q -- '-title' "$scratch/notifier.args" || {
  echo "the notifier spawn carried no title" >&2
  exit 1
}
[[ ! -f "$scratch/decoy.event" ]] || {
  echo "the decoy executable channel fired; native did not win" >&2
  exit 1
}

# Phase 3: native moshi. RELAY_MOSHI_URL points at the crate's own one-shot
# capture binary (std only, built by the same cargo build), and the captured
# raw request must carry the JSON content type, a valid JSON body, the token
# and the title: the moshi leg went native, with the secret in the body and
# never on argv. No interpreter, so there is no cold start to diagnose.
printf '{"moshi_secret":"tok-integration"}\n' >"$scratch/auth.json"
"$REPO_ROOT/dot_local/share/pns/target/debug/http-capture" \
  "$scratch/port" "$scratch/capture" 2>"$scratch/server.err" &
server_pid=$!
for _ in $(seq 1 60); do
  [[ -s "$scratch/port" ]] && break
  sleep 0.5
done
[[ -s "$scratch/port" ]] || {
  echo "capture server never bound; its stderr:" >&2
  cat "$scratch/server.err" >&2 || true
  exit 1
}

# Proxy-hermetic, and the binary's own output is captured: the token must
# reach the capture server and NOTHING else, including this engine's stdout
# and stderr. The token is matched through a pattern file so it never rides
# an argv of this script's own children either.
printf 'tok-integration\n' >"$scratch/token.pattern"
env -u PNS_CHANNELS_DIR -u http_proxy -u https_proxy -u HTTP_PROXY -u HTTPS_PROXY \
  -u ALL_PROXY -u NO_PROXY \
  HOME="$scratch" PATH="$stub_bin:$PATH" \
  RELAY_IDLE_SECS=99999 RELAY_AUTH_FILE="$scratch/auth.json" \
  RELAY_MOSHI_URL="http://127.0.0.1:$(cat "$scratch/port")" \
  "$REPO_ROOT/dot_local/share/pns/target/debug/pns" \
  --agent claude --state 'done' --detail x \
  >"$scratch/pns.out" 2>"$scratch/pns.err"

wait "$server_pid" 2>/dev/null || true
grep -qi 'Content-Type: application/json' "$scratch/capture" 2>/dev/null || {
  echo "the request carried no JSON content type" >&2
  exit 1
}
tr -d '\r' <"$scratch/capture" | sed -n '/^$/,$p' | sed '1d' | jq -e . >/dev/null || {
  echo "the posted body is not JSON" >&2
  exit 1
}
grep -qf "$scratch/token.pattern" "$scratch/capture" 2>/dev/null || {
  echo "native moshi did not post the token" >&2
  exit 1
}
grep -q 'claude' "$scratch/capture" || {
  echo "the post carried no title" >&2
  exit 1
}
if grep -qf "$scratch/token.pattern" "$scratch/pns.out" "$scratch/pns.err" 2>/dev/null; then
  echo "the token leaked into the engine's own output" >&2
  exit 1
fi
[[ ! -s "$scratch/pns.out" ]] || {
  echo "the alert path printed to stdout; async legs must be silent" >&2
  exit 1
}

# The same path against a dead endpoint: exit 0 and SILENCE, because the only
# thing worth reporting would carry the token.
env -u PNS_CHANNELS_DIR -u http_proxy -u https_proxy -u HTTP_PROXY -u HTTPS_PROXY \
  -u ALL_PROXY -u NO_PROXY \
  HOME="$scratch" PATH="$stub_bin:$PATH" \
  RELAY_IDLE_SECS=99999 RELAY_AUTH_FILE="$scratch/auth.json" \
  RELAY_MOSHI_URL="http://127.0.0.1:1" \
  "$REPO_ROOT/dot_local/share/pns/target/debug/pns" \
  --agent claude --state 'done' --detail x \
  >"$scratch/pns-dead.out" 2>"$scratch/pns-dead.err"
[[ ! -s "$scratch/pns-dead.err" ]] || {
  echo "a failed post said something; the failure path must be silent" >&2
  exit 1
}

# Phase 4: native hermes. The capture binary again, this time on the log
# path: --remote-only makes hermes the whole plan, sync, so the engine must
# print the posted line, and the captured X-Webhook-Signature must equal
# openssl's own HMAC of the captured body, proving the signature covers the
# exact bytes that were sent.
printf '{"hermes_secret":"gate-signing-key"}\n' >"$scratch/auth.json"
: >"$scratch/port"
"$REPO_ROOT/dot_local/share/pns/target/debug/http-capture" \
  "$scratch/port" "$scratch/hermes.capture" 2>"$scratch/server.err" &
server_pid=$!
for _ in $(seq 1 60); do
  [[ -s "$scratch/port" ]] && break
  sleep 0.5
done
[[ -s "$scratch/port" ]] || {
  echo "hermes capture server never bound; its stderr:" >&2
  cat "$scratch/server.err" >&2 || true
  exit 1
}

env -u PNS_CHANNELS_DIR -u http_proxy -u https_proxy -u HTTP_PROXY -u HTTPS_PROXY \
  -u ALL_PROXY -u NO_PROXY \
  HOME="$scratch" PATH="$stub_bin:$PATH" \
  RELAY_AUTH_FILE="$scratch/auth.json" \
  RELAY_HERMES_URL="http://127.0.0.1:$(cat "$scratch/port")" \
  "$REPO_ROOT/dot_local/share/pns/target/debug/pns" \
  --agent weekly --state 'done' --detail ran --remote-only \
  >"$scratch/hermes.out" 2>"$scratch/hermes.err"

wait "$server_pid" 2>/dev/null || true
[[ "$(cat "$scratch/hermes.out")" == "relay: posted HTTP 200" ]] || {
  echo "sync hermes stdout is not exactly the posted line:" >&2
  cat "$scratch/hermes.out" >&2
  exit 1
}
body="$(tr -d '\r' <"$scratch/hermes.capture" | sed -n '/^$/,$p' | sed '1d')"
sent_signature="$(tr -d '\r' <"$scratch/hermes.capture" |
  sed -n 's/^[Xx]-[Ww]ebhook-[Ss]ignature: //p' | head -1)"
expected_signature="$(printf '%s' "$body" |
  openssl dgst -sha256 -hmac 'gate-signing-key' | sed 's/^.*= //')"
[[ -n $sent_signature && $sent_signature == "$expected_signature" ]] || {
  echo "the signature does not match openssl's HMAC of the captured body" >&2
  echo "sent: $sent_signature expected: $expected_signature" >&2
  exit 1
}

# Phase 4b: the gateway answers 401 (a rotated signing key). Sync must name
# the status, because "no response" would send the operator to restart a
# healthy gateway instead of fixing the key.
: >"$scratch/port"
"$REPO_ROOT/dot_local/share/pns/target/debug/http-capture" \
  "$scratch/port" "$scratch/hermes-401.capture" 401 2>"$scratch/server.err" &
server_pid=$!
for _ in $(seq 1 60); do
  [[ -s "$scratch/port" ]] && break
  sleep 0.5
done
[[ -s "$scratch/port" ]] || {
  echo "the 401 capture server never bound" >&2
  exit 1
}
env -u PNS_CHANNELS_DIR -u http_proxy -u https_proxy -u HTTP_PROXY -u HTTPS_PROXY \
  -u ALL_PROXY -u NO_PROXY \
  HOME="$scratch" PATH="$stub_bin:$PATH" \
  RELAY_AUTH_FILE="$scratch/auth.json" \
  RELAY_HERMES_URL="http://127.0.0.1:$(cat "$scratch/port")" \
  "$REPO_ROOT/dot_local/share/pns/target/debug/pns" \
  --agent weekly --state 'done' --detail ran --remote-only \
  >"$scratch/hermes-401.out" 2>/dev/null
wait "$server_pid" 2>/dev/null || true
[[ "$(cat "$scratch/hermes-401.out")" == "relay: post FAILED HTTP 401" ]] || {
  echo "a 401 must read as HTTP 401, not as a downed gateway:" >&2
  cat "$scratch/hermes-401.out" >&2
  exit 1
}
