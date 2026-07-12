#!/usr/bin/env bash
# relay.sh: fans out to moshi + hermes + local, failure-separated, exits 0.
set -uo pipefail
relay="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/dot_local/bin/executable_relay.sh"
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
# "state / project" header that the title already carries (wastes the phone/banner preview line).
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
# The hermes curl is a backgrounded grandchild (relay exits before it lands), so `wait` cannot see it;
# poll up to ~5s for the captured body/sig, matching the neighboring sections, before asserting.
for ((i = 0; i < 100; i++)); do
  [[ -s "$tmp/hbody" && -s "$tmp/hsig" ]] && break
  sleep 0.05
done
wait 2>/dev/null
expected_sig="$(python3 -c 'import hmac, hashlib, sys
sys.stdout.write(hmac.new(b"HSECRET", open(sys.argv[1], "rb").read(), hashlib.sha256).hexdigest())' "$tmp/hbody" 2>/dev/null)"
[[ -n $expected_sig && "$(cat "$tmp/hsig")" == "$expected_sig" ]] || {
  echo "relay: FAIL -- X-Webhook-Signature is not HMAC-SHA256(body, secret)" >&2
  exit 1
}
# FIX 4 (missing flag value -> graceful degrade): a value-taking flag as the LAST argument
# must not abort the notification path. It warns to stderr, ignores the valueless flag, still
# exits 0, and the channels whose flags DID parse still fire.
cat >"$tmp/curl" <<MOCK
#!/usr/bin/env bash
echo "ARGV: \$*" >>"$tmp/curl-noval.log"
MOCK
chmod +x "$tmp/curl"
noval_err="$(
  PATH="$tmp:$PATH" RELAY_AUTH_FILE="$tmp/auth.json" \
    RELAY_MOSHI_URL="http://moshi.test/hook" RELAY_HERMES_URL="http://hermes.test/relay" \
    bash "$relay" --agent claude --state "done" --project x --pane 2>&1 >/dev/null
)"
noval_rc=$?
for ((i = 0; i < 100; i++)); do
  grep -q hermes.test "$tmp/curl-noval.log" 2>/dev/null && grep -q moshi.test "$tmp/curl-noval.log" 2>/dev/null && break
  sleep 0.05
done
wait 2>/dev/null
[[ $noval_rc -eq 0 ]] || {
  echo "relay: FAIL -- a value-taking flag with no value broke the always-exit-0 contract (rc=$noval_rc)" >&2
  exit 1
}
grep -qi "pane" <<<"$noval_err" || {
  echo "relay: FAIL -- no stderr warning for the missing --pane value" >&2
  exit 1
}
grep -q moshi.test "$tmp/curl-noval.log" || {
  echo "relay: FAIL -- moshi dropped when a trailing flag lacked its value" >&2
  exit 1
}
grep -q hermes.test "$tmp/curl-noval.log" || {
  echo "relay: FAIL -- hermes dropped when a trailing flag lacked its value" >&2
  exit 1
}

# FIX 1 (fail-closed idle probe -> fail OPEN): with RELAY_IDLE_SECS unset, a failing
# or HIDIdleTime-less ioreg probe must NOT abort the script. Unknown idle = "user away",
# so every channel still fires. The stub prints lines without HIDIdleTime, so grep -m1
# finds nothing and the probe pipeline exits non-zero (the abort trigger under set -e).
cat >"$tmp/ioreg" <<'MOCK'
#!/usr/bin/env bash
echo "no idle field here"
MOCK
chmod +x "$tmp/ioreg"
cat >"$tmp/curl" <<MOCK
#!/usr/bin/env bash
echo "ARGV: \$*" >>"$tmp/curl-idle.log"
MOCK
chmod +x "$tmp/curl"
: >"$tmp/tn.log"
unset RELAY_IDLE_SECS
out_idle="$(
  PATH="$tmp:$PATH" RELAY_AUTH_FILE="$tmp/auth.json" RELAY_IOREG="$tmp/ioreg" \
    RELAY_MOSHI_URL="http://moshi.test/hook" RELAY_HERMES_URL="http://hermes.test/relay" \
    bash "$relay" --agent claude --state "done" --project x --detail "idle probe failed" --pane wW:p8 2>&1
  echo "rc=$?"
)"
for ((i = 0; i < 100; i++)); do
  grep -q hermes.test "$tmp/curl-idle.log" 2>/dev/null && grep -q moshi.test "$tmp/curl-idle.log" 2>/dev/null && break
  sleep 0.05
done
wait 2>/dev/null
grep -q "rc=0" <<<"$out_idle" || {
  echo "relay: FAIL -- a failed idle probe aborted the script (fail-closed)" >&2
  exit 1
}
grep -q moshi.test "$tmp/curl-idle.log" || {
  echo "relay: FAIL -- fail-open idle: phone push dropped when idle is unknown" >&2
  exit 1
}
grep -q hermes.test "$tmp/curl-idle.log" || {
  echo "relay: FAIL -- fail-open idle: Discord log dropped when idle is unknown" >&2
  exit 1
}
grep -q "herdr agent focus wW:p8" "$tmp/tn.log" || {
  echo "relay: FAIL -- fail-open idle: local notification dropped when idle is unknown" >&2
  exit 1
}
export RELAY_IDLE_SECS=999

# FIX F2a (garbled probe line -> fail OPEN): a PRESENT but non-numeric HIDIdleTime field must not be
# awk-coerced to 0 ("actively typing") and silently suppress the phone push. Presence is unknown, so
# every channel including moshi fires, exit 0. RELAY_IDLE_SECS unset so the probe path runs.
cat >"$tmp/ioreg" <<'MOCK'
#!/usr/bin/env bash
echo '"HIDIdleTime" = notanumber'
MOCK
chmod +x "$tmp/ioreg"
cat >"$tmp/curl" <<MOCK
#!/usr/bin/env bash
echo "ARGV: \$*" >>"$tmp/curl-garbled.log"
MOCK
chmod +x "$tmp/curl"
unset RELAY_IDLE_SECS
out_garbled="$(
  PATH="$tmp:$PATH" RELAY_AUTH_FILE="$tmp/auth.json" RELAY_IOREG="$tmp/ioreg" \
    RELAY_MOSHI_URL="http://moshi.test/hook" RELAY_HERMES_URL="http://hermes.test/relay" \
    bash "$relay" --agent claude --state "done" --project x --detail "garbled idle" 2>&1
  echo "rc=$?"
)"
for ((i = 0; i < 100; i++)); do
  grep -q moshi.test "$tmp/curl-garbled.log" 2>/dev/null && grep -q hermes.test "$tmp/curl-garbled.log" 2>/dev/null && break
  sleep 0.05
done
wait 2>/dev/null
grep -q "rc=0" <<<"$out_garbled" || {
  echo "relay: FAIL -- a garbled idle probe line broke exit 0" >&2
  exit 1
}
grep -q moshi.test "$tmp/curl-garbled.log" || {
  echo "relay: FAIL -- a garbled idle line was coerced to 0 and suppressed the phone push" >&2
  exit 1
}
grep -q hermes.test "$tmp/curl-garbled.log" || {
  echo "relay: FAIL -- garbled idle: Discord log dropped" >&2
  exit 1
}

# FIX F2b (non-numeric threshold -> fail OPEN): a non-numeric RELAY_DESK_IDLE_SECS must not abort the
# bash arithmetic comparison under set -u (rc=1). The threshold is validated as decimal digits BEFORE
# the arithmetic; an invalid threshold = presence unknown = deliver everything, exit 0.
cat >"$tmp/curl" <<MOCK
#!/usr/bin/env bash
echo "ARGV: \$*" >>"$tmp/curl-thresh.log"
MOCK
chmod +x "$tmp/curl"
out_thresh="$(
  PATH="$tmp:$PATH" RELAY_AUTH_FILE="$tmp/auth.json" RELAY_IDLE_SECS=5 RELAY_DESK_IDLE_SECS=not-a-number \
    RELAY_MOSHI_URL="http://moshi.test/hook" RELAY_HERMES_URL="http://hermes.test/relay" \
    bash "$relay" --agent claude --state "done" --project x --detail "bad threshold" 2>&1
  echo "rc=$?"
)"
for ((i = 0; i < 100; i++)); do
  grep -q moshi.test "$tmp/curl-thresh.log" 2>/dev/null && grep -q hermes.test "$tmp/curl-thresh.log" 2>/dev/null && break
  sleep 0.05
done
wait 2>/dev/null
grep -q "rc=0" <<<"$out_thresh" || {
  echo "relay: FAIL -- a non-numeric RELAY_DESK_IDLE_SECS aborted the notification path (rc!=0)" >&2
  exit 1
}
grep -q moshi.test "$tmp/curl-thresh.log" || {
  echo "relay: FAIL -- a non-numeric threshold suppressed the phone push instead of failing open" >&2
  exit 1
}
grep -q hermes.test "$tmp/curl-thresh.log" || {
  echo "relay: FAIL -- non-numeric threshold: Discord log dropped" >&2
  exit 1
}
export RELAY_IDLE_SECS=999

# FIX F4 (missing value consumes a following recognized flag): `--pane --local-only` must NOT let
# --local-only become the pane VALUE (which would leave local_only empty and fire both webhooks despite
# the caller asking for local-only). A value-taking flag whose next token is a RECOGNIZED option is
# treated as missing-value: warn, ignore the valueless flag, do NOT consume the next token. Result:
# --local-only takes effect -> NEITHER webhook fires, the local notification DOES.
cat >"$tmp/curl" <<MOCK
#!/usr/bin/env bash
echo "ARGV: \$*" >>"$tmp/curl-f4.log"
MOCK
chmod +x "$tmp/curl"
: >"$tmp/tn.log"
f4_err="$(
  PATH="$tmp:$PATH" RELAY_AUTH_FILE="$tmp/auth.json" \
    RELAY_MOSHI_URL="http://moshi.test/hook" RELAY_HERMES_URL="http://hermes.test/relay" \
    bash "$relay" --agent claude --state "done" --project x --pane --local-only 2>&1 >/dev/null
)"
f4_rc=$?
wait 2>/dev/null
[[ $f4_rc -eq 0 ]] || {
  echo "relay: FAIL -- --pane --local-only broke exit 0 (rc=$f4_rc)" >&2
  exit 1
}
grep -qi "pane" <<<"$f4_err" || {
  echo "relay: FAIL -- no stderr warning that --pane lacked its value" >&2
  exit 1
}
[[ -f "$tmp/curl-f4.log" ]] && {
  echo "relay: FAIL -- --local-only was consumed as the pane value; a webhook fired" >&2
  exit 1
}
grep -q "terminal-notifier\|herdr agent focus\|ARGV" "$tmp/tn.log" || {
  echo "relay: FAIL -- --local-only local notification did not fire" >&2
  exit 1
}

# CHARACTERIZATION (baseline quirk, retained; SP3 owns): the arg parser silently
# ignores an UNRECOGNIZED flag (the *) shift ;; branch) rather than erroring, which
# deviates from the house "unknown arg is an error" rule. A notification path must
# never abort on a caller typo, so this leniency is deliberate. Pin it: an unknown
# flag does not break exit 0 and the recognized channels still fire.
cat >"$tmp/curl" <<MOCK
#!/usr/bin/env bash
echo "ARGV: \$*" >>"$tmp/curl-unknown.log"
MOCK
chmod +x "$tmp/curl"
: >"$tmp/tn.log"
out_unknown="$(
  PATH="$tmp:$PATH" RELAY_AUTH_FILE="$tmp/auth.json" \
    RELAY_MOSHI_URL="http://moshi.test/hook" RELAY_HERMES_URL="http://hermes.test/relay" \
    bash "$relay" --bogus whatever --agent claude --state "done" --project x --pane wW:p8 2>&1
  echo "rc=$?"
)"
wait 2>/dev/null
grep -q "rc=0" <<<"$out_unknown" || {
  echo "relay: FAIL -- an unknown flag broke exit 0 (quirk drift)" >&2
  exit 1
}
grep -q "herdr agent focus wW:p8" "$tmp/tn.log" || {
  echo "relay: FAIL -- an unknown flag suppressed the local channel (quirk drift)" >&2
  exit 1
}
echo "relay: OK"
