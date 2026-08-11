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

# Phase 3: native moshi. RELAY_MOSHI_URL points at a one-shot local capture
# server, the isolated HOME carries an auth token, and the delivered body must
# carry that token and the title: the moshi leg went native, with the secret
# in the body and never on argv.
printf '{"moshi_secret":"tok-integration"}\n' >"$scratch/auth.json"
python3 - "$scratch/port" "$scratch/capture" <<'PYEOF' &
import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        body = self.rfile.read(int(self.headers.get('Content-Length', 0)))
        if self.headers.get('Content-Type') != 'application/json':
            body = b'WRONG-CONTENT-TYPE'
        else:
            try:
                json.loads(body)
            except ValueError:
                body = b'NOT-JSON'
        with open(sys.argv[2], 'wb') as capture:
            capture.write(body)
        self.send_response(200)
        self.end_headers()

    def log_message(self, *args):
        pass

server = HTTPServer(('127.0.0.1', 0), Handler)
server.timeout = 10
with open(sys.argv[1], 'w') as port_file:
    port_file.write(str(server.server_address[1]))
server.handle_request()
PYEOF
server_pid=$!
for _ in $(seq 1 50); do
  [[ -s "$scratch/port" ]] && break
  sleep 0.1
done
[[ -s "$scratch/port" ]] || {
  echo "capture server never bound" >&2
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
