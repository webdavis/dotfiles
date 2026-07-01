#!/usr/bin/env bash
# relay.sh: fans out to moshi + hermes + local, failure-separated, exits 0.
set -uo pipefail
relay="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/dot_local/bin/executable_relay.sh"
[[ -x $relay ]] || {
  echo "relay: FAIL -- not executable: $relay" >&2
  exit 1
}
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
# record-only mocks
cat >"$tmp/curl" <<MOCK
#!/usr/bin/env bash
echo "\$*" >>"$tmp/curl-argv.log"
cat >>"$tmp/curl-stdin.log"
MOCK
cat >"$tmp/terminal-notifier" <<MOCK
#!/usr/bin/env bash
echo "ARGV: \$*" >>"$tmp/tn.log"
MOCK
chmod +x "$tmp/curl" "$tmp/terminal-notifier"
printf '{"moshi_secret":"MSECRET","hermes_secret":"HSECRET"}' >"$tmp/auth.json"
# default every run to "away" (idle past threshold) so moshi fires; the presence tests override inline
export RELAY_IDLE_SECS=999
out="$(
  PATH="$tmp:$PATH" RELAY_AUTH_FILE="$tmp/auth.json" \
    RELAY_MOSHI_URL="http://moshi.test/hook" RELAY_HERMES_URL="http://hermes.test/relay" \
    bash "$relay" --agent claude --state "done" --project dotfiles --branch main \
    --detail "all green" --pane wW:p8 2>&1
  echo "rc=$?"
)"
wait 2>/dev/null
grep -q "rc=0" <<<"$out" || {
  echo "relay: FAIL -- exit not 0" >&2
  exit 1
}
grep -q "moshi.test/hook" "$tmp/curl-argv.log" || {
  echo "relay: FAIL -- moshi not called" >&2
  exit 1
}
grep -q "hermes.test/relay" "$tmp/curl-argv.log" || {
  echo "relay: FAIL -- hermes not called" >&2
  exit 1
}
grep -q "X-Webhook-Signature:" "$tmp/curl-argv.log" || {
  echo "relay: FAIL -- no HMAC header" >&2
  exit 1
}
grep -q "MSECRET" "$tmp/curl-stdin.log" || {
  echo "relay: FAIL -- moshi token not sent in body" >&2
  exit 1
}
# de-duped body: the summary is the prominent message (branch-prefixed), NOT behind a redundant
# "state — project" header that the title already carries (wastes the phone/banner preview line).
grep -qF '(main) all green' "$tmp/curl-stdin.log" || {
  echo "relay: FAIL -- de-duped summary is not the message body" >&2
  exit 1
}
grep -q "MSECRET\|HSECRET" "$tmp/curl-argv.log" && {
  echo "relay: FAIL -- secret leaked to argv" >&2
  exit 1
}
grep -q "HSECRET" "$tmp/curl-stdin.log" && {
  echo "relay: FAIL -- hermes secret leaked into the body" >&2
  exit 1
}
grep -q "herdr agent focus wW:p8" "$tmp/tn.log" || {
  echo "relay: FAIL -- local focus cmd wrong" >&2
  exit 1
}
# long summary: moshi + local get a sentence-bounded preview (phone clips mid-sentence otherwise);
# hermes/#relay keeps the FULL text (Discord has no length ceiling).
: >"$tmp/curl-stdin.log"
long="The first sentence is padded out to a comfortable length so it occupies a real share of the preview here. The second sentence is similarly padded so the two together reach the phone preview ceiling we target. The trailing third sentence must never appear in the trimmed moshi push body whatsoever."
PATH="$tmp:$PATH" RELAY_AUTH_FILE="$tmp/auth.json" RELAY_MOSHI_URL="http://moshi.test/hook" \
  RELAY_HERMES_URL="http://hermes.test/relay" bash "$relay" --agent claude --state "done" --project x --detail "$long" >/dev/null 2>&1
# the hermes curl is delayed ~50ms by the python HMAC; poll until its body lands (moshi is present by then too)
for ((i = 0; i < 100; i++)); do
  grep -q '"agent"' "$tmp/curl-stdin.log" && break
  sleep 0.05
done
grep '"agent"' "$tmp/curl-stdin.log" | grep -q 'must never appear' || {
  echo "relay: FAIL -- hermes/#relay lost the full summary" >&2
  exit 1
}
grep '"token"' "$tmp/curl-stdin.log" | grep -q 'must never appear' && {
  echo "relay: FAIL -- moshi push not trimmed (full summary would clip mid-sentence on the phone)" >&2
  exit 1
}
# presence gating: at the desk (idle below threshold) -> NO phone push; local + Discord still fire
: >"$tmp/curl-argv.log"
: >"$tmp/tn.log"
RELAY_IDLE_SECS=5 PATH="$tmp:$PATH" RELAY_AUTH_FILE="$tmp/auth.json" \
  RELAY_MOSHI_URL="http://moshi.test/hook" RELAY_HERMES_URL="http://hermes.test/relay" \
  bash "$relay" --agent claude --state "done" --project x --pane wW:p8 >/dev/null 2>&1
for ((i = 0; i < 100; i++)); do
  grep -q hermes.test "$tmp/curl-argv.log" && break
  sleep 0.05
done
grep -q moshi.test "$tmp/curl-argv.log" && {
  echo "relay: FAIL -- phone pushed while at the desk" >&2
  exit 1
}
grep -q hermes.test "$tmp/curl-argv.log" || {
  echo "relay: FAIL -- Discord log dropped while at the desk" >&2
  exit 1
}
grep -q "herdr agent focus" "$tmp/tn.log" || {
  echo "relay: FAIL -- local notification dropped while at the desk" >&2
  exit 1
}
# away (idle past the threshold) -> phone push fires
: >"$tmp/curl-argv.log"
RELAY_IDLE_SECS=900 PATH="$tmp:$PATH" RELAY_AUTH_FILE="$tmp/auth.json" \
  RELAY_MOSHI_URL="http://moshi.test/hook" RELAY_HERMES_URL="http://hermes.test/relay" \
  bash "$relay" --agent claude --state "done" --project x >/dev/null 2>&1
for ((i = 0; i < 100; i++)); do
  grep -q moshi.test "$tmp/curl-argv.log" && break
  sleep 0.05
done
grep -q moshi.test "$tmp/curl-argv.log" || {
  echo "relay: FAIL -- phone not pushed while away" >&2
  exit 1
}
# at the desk BUT RELAY_FORCE_PHONE=1 -> phone push fires anyway
: >"$tmp/curl-argv.log"
RELAY_IDLE_SECS=5 RELAY_FORCE_PHONE=1 PATH="$tmp:$PATH" RELAY_AUTH_FILE="$tmp/auth.json" \
  RELAY_MOSHI_URL="http://moshi.test/hook" RELAY_HERMES_URL="http://hermes.test/relay" \
  bash "$relay" --agent claude --state "done" --project x >/dev/null 2>&1
for ((i = 0; i < 100; i++)); do
  grep -q moshi.test "$tmp/curl-argv.log" && break
  sleep 0.05
done
grep -q moshi.test "$tmp/curl-argv.log" || {
  echo "relay: FAIL -- RELAY_FORCE_PHONE did not override desk presence" >&2
  exit 1
}
# failure separation: hermes curl fails, moshi + local still happen, exit 0
cat >"$tmp/curl" <<MOCK
#!/usr/bin/env bash
echo "ARGV: \$*" >>"$tmp/curl2.log"
[[ "\$*" == *hermes.test* ]] && exit 7
MOCK
chmod +x "$tmp/curl"
out2="$(
  PATH="$tmp:$PATH" RELAY_AUTH_FILE="$tmp/auth.json" \
    RELAY_MOSHI_URL="http://moshi.test/hook" RELAY_HERMES_URL="http://hermes.test/relay" \
    bash "$relay" --agent claude --state "done" --project x --pane wW:p8 2>&1
  echo "rc=$?"
)"
wait 2>/dev/null
grep -q "rc=0" <<<"$out2" || {
  echo "relay: FAIL -- exit not 0 on hermes failure" >&2
  exit 1
}
grep -q "moshi.test/hook" "$tmp/curl2.log" || {
  echo "relay: FAIL -- moshi dropped on hermes failure" >&2
  exit 1
}
# no secret -> no-op, exit 0
printf '{}' >"$tmp/empty.json"
out3="$(
  PATH="$tmp:$PATH" RELAY_AUTH_FILE="$tmp/empty.json" bash "$relay" --state "done" --project x 2>&1
  echo "rc=$?"
)"
grep -q "rc=0" <<<"$out3" || {
  echo "relay: FAIL -- exit not 0 with no secrets" >&2
  exit 1
}
# --local-only: clickable local fires, webhooks do NOT (even with secrets present)
cat >"$tmp/curl" <<MOCK
#!/usr/bin/env bash
echo "ARGV: \$*" >>"$tmp/curl3.log"
MOCK
chmod +x "$tmp/curl"
: >"$tmp/tn.log"
out4="$(
  PATH="$tmp:$PATH" RELAY_AUTH_FILE="$tmp/auth.json" \
    RELAY_MOSHI_URL="http://moshi.test/hook" RELAY_HERMES_URL="http://hermes.test/relay" \
    bash "$relay" --local-only --agent shell --state "done" --project x --pane wW:p8 2>&1
  echo "rc=$?"
)"
wait 2>/dev/null
grep -q "rc=0" <<<"$out4" || {
  echo "relay: FAIL -- --local-only exit not 0" >&2
  exit 1
}
grep -q "herdr agent focus wW:p8" "$tmp/tn.log" || {
  echo "relay: FAIL -- --local-only sent no local notification" >&2
  exit 1
}
[[ -f "$tmp/curl3.log" ]] && {
  echo "relay: FAIL -- --local-only called a webhook" >&2
  exit 1
}
# a jq failure must NOT break exit 0 or suppress the jq-independent local channel
cat >"$tmp/jq" <<'MOCK'
#!/usr/bin/env bash
exit 1
MOCK
chmod +x "$tmp/jq"
: >"$tmp/tn.log"
out5="$(
  PATH="$tmp:$PATH" RELAY_AUTH_FILE="$tmp/auth.json" \
    RELAY_MOSHI_URL="http://moshi.test/hook" RELAY_HERMES_URL="http://hermes.test/relay" \
    bash "$relay" --agent claude --state "done" --project x --pane wW:p8 2>&1
  echo "rc=$?"
)"
wait 2>/dev/null
grep -q "rc=0" <<<"$out5" || {
  echo "relay: FAIL -- jq failure broke exit 0" >&2
  exit 1
}
grep -q "herdr agent focus wW:p8" "$tmp/tn.log" || {
  echo "relay: FAIL -- jq failure suppressed the local channel" >&2
  exit 1
}
rm -f "$tmp/jq"
# the X-Webhook-Signature must be a CORRECT HMAC-SHA256 of the body under the secret,
# not merely present -- a wrong-but-nonempty signature passes header checks while Hermes
# silently rejects every message
cat >"$tmp/curl" <<'MOCK'
#!/usr/bin/env bash
args="$*"
body="$(cat)"
if [[ $args == *hermes* ]]; then
  printf '%s' "$body" >"$HCAP_BODY"
  sig="${args#*X-Webhook-Signature: }"
  printf '%s' "${sig%% *}" >"$HCAP_SIG"
fi
MOCK
chmod +x "$tmp/curl"
: >"$tmp/hbody"
: >"$tmp/hsig"
HCAP_BODY="$tmp/hbody" HCAP_SIG="$tmp/hsig" PATH="$tmp:$PATH" RELAY_AUTH_FILE="$tmp/auth.json" \
  RELAY_HERMES_URL="http://hermes.test/relay" RELAY_MOSHI_URL="http://moshi.test/hook" \
  bash "$relay" --agent claude --state "done" --project x >/dev/null 2>&1
wait 2>/dev/null
expected_sig="$(python3 -c 'import hmac, hashlib, sys
sys.stdout.write(hmac.new(b"HSECRET", open(sys.argv[1], "rb").read(), hashlib.sha256).hexdigest())' "$tmp/hbody" 2>/dev/null)"
[[ -n $expected_sig && "$(cat "$tmp/hsig")" == "$expected_sig" ]] || {
  echo "relay: FAIL -- X-Webhook-Signature is not HMAC-SHA256(body, secret)" >&2
  exit 1
}
echo "relay: OK"
