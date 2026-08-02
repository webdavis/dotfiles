#!/usr/bin/env bash
#
# relay.sh --remote-only: the LOG delivery path, symmetric with --local-only.
#
# --local-only means "no webhooks, banner only". --remote-only is its mirror:
# the hermes (Discord) leg ONLY, with no macOS banner and no phone push. The
# weekly unattended jobs need it because their entries are a RECORD, not an
# alert: a Monday-slot run satisfies the phone push's idle threshold by
# definition, so without this flag every heartbeat would pop a banner and buzz
# the phone. --local-only is the wrong lever (it suppresses the hermes leg too).
#
# On that path ONLY the POST is SYNCHRONOUS with a short deadline and prints its
# delivery outcome, because a 401 or a 404 backgrounded into /dev/null leaves the
# log channel empty and makes a healthy job look dead. It must still NEVER fail
# the caller: exit 0 whatever the HTTP status.
#
# Unit test: stub-driven (curl, terminal-notifier), no sleeps, no polling. The
# absence of polling is itself load-bearing -- the status line can only be
# printed by a call that was WAITED for, so it is the synchrony proof.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RELAY="$REPO_ROOT/dot_local/bin/executable_relay.sh"

fail() {
  printf 'relay-remote-only: FAIL -- %s\n' "$*" >&2
  exit 1
}

# refute <regex> <haystack> <message> -- an explicit negative assertion.
# `! grep ...` under `set -e` never fails a test (errexit ignores an inverted
# status), and this repo has shipped that bug; this helper is the refutation
# form every negative check here goes through.
refute() {
  local pattern="$1" haystack="$2" message="$3"
  if grep -qE "$pattern" <<<"$haystack"; then
    printf '=== haystack ===\n%s\n' "$haystack" >&2
    fail "$message"
  fi
}

refute_file() {
  local path="$1" message="$2"
  if [[ -e $path ]]; then
    printf '=== %s ===\n%s\n' "$path" "$(cat "$path" 2>/dev/null)" >&2
    fail "$message"
  fi
}

[[ -x $RELAY ]] || fail "not executable: $RELAY"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

MOSHI_URL="http://moshi.test/hook"
HERMES_URL="http://hermes.test/unattended-upgrades"

# curl stub: records argv + stdin, emits the -w http_code the caller asked for
# (FAKE_HTTP_CODE), and exits FAKE_CURL_RC so a transport failure is simulable.
cat >"$tmp/curl" <<'MOCK'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"$CURL_ARGV_LOG"
cat >>"$CURL_STDIN_LOG"
printf '%s' "${FAKE_HTTP_CODE:-200}"
exit "${FAKE_CURL_RC:-0}"
MOCK
cat >"$tmp/terminal-notifier" <<'MOCK'
#!/usr/bin/env bash
printf 'ARGV: %s\n' "$*" >>"$TN_LOG"
MOCK
chmod +x "$tmp/curl" "$tmp/terminal-notifier"

printf '{"moshi_secret":"MOSHI-TOKEN-FIXTURE","hermes_secret":"HERMES-KEY-FIXTURE"}' >"$tmp/auth.json"
printf '{}' >"$tmp/auth-empty.json"

# run_relay <run-label> [relay args...] -- fresh logs per run, captured stdout,
# captured stderr, captured rc. RELAY_IDLE_SECS=999 = "away", so the phone push
# is WANTED on every run: a suppressed moshi call then proves --remote-only did
# it, not the presence gate.
RELAY_STDOUT="" RELAY_STDERR="" RELAY_RC=0
CURL_ARGV_LOG="" CURL_STDIN_LOG="" TN_LOG=""
run_relay() {
  local label="$1"
  shift
  CURL_ARGV_LOG="$tmp/$label.curl-argv"
  CURL_STDIN_LOG="$tmp/$label.curl-stdin"
  TN_LOG="$tmp/$label.tn"
  export CURL_ARGV_LOG CURL_STDIN_LOG TN_LOG
  RELAY_STDOUT="$(
    PATH="$tmp:$PATH" RELAY_AUTH_FILE="${AUTH_FILE:-$tmp/auth.json}" \
      RELAY_MOSHI_URL="$MOSHI_URL" RELAY_HERMES_URL="$HERMES_URL" \
      RELAY_IDLE_SECS=999 \
      bash "$RELAY" "$@" 2>"$tmp/$label.stderr"
  )"
  RELAY_RC=$?
  RELAY_STDERR="$(cat "$tmp/$label.stderr")"
  wait 2>/dev/null
}

# ── 1. --remote-only delivers the hermes leg and NOTHING else ────────────────
run_relay remote-basic --remote-only --agent update-skills --state completed \
  --project dresden --detail "nothing changed" --pane wW:p8
[[ $RELAY_RC -eq 0 ]] || fail "--remote-only exit was $RELAY_RC, must always be 0"
grep -qF "$HERMES_URL" "$tmp/remote-basic.curl-argv" ||
  fail "--remote-only did not call the hermes route"
refute "moshi\.test" "$(cat "$tmp/remote-basic.curl-argv")" \
  "--remote-only pushed to the phone (moshi); the log channel must be silent on the phone"
refute_file "$tmp/remote-basic.tn" \
  "--remote-only fired a macOS banner; the log channel must not pop a notification"
grep -qF 'X-Webhook-Signature:' "$tmp/remote-basic.curl-argv" ||
  fail "--remote-only sent no HMAC signature header"

# ── 2. The POST is SYNCHRONOUS and reports its outcome. The status line is the
#      synchrony proof: an HTTP code can only be printed by a call that was
#      waited for. Asserted with NO polling loop -- if the POST were
#      backgrounded, relay would have exited before the code existed. ─────────
grep -qE '^relay: posted HTTP 200$' <<<"$RELAY_STDOUT" ||
  fail "--remote-only printed no 'posted HTTP 200' line on stdout (got: $RELAY_STDOUT)"

# ── 3. A REFUSED delivery is reported, not swallowed. 401 (wrong key) and 404
#      (route not loaded in the gateway) are the two ways this feature dies
#      silently; both must land in the caller's run log. ──────────────────────
FAKE_HTTP_CODE=401 run_relay remote-401 --remote-only --agent update-skills \
  --state completed --project dresden --detail "nothing changed"
[[ $RELAY_RC -eq 0 ]] || fail "a 401 broke the always-exit-0 contract (rc=$RELAY_RC)"
grep -qE '^relay: post FAILED HTTP 401$' <<<"$RELAY_STDOUT" ||
  fail "a 401 was not reported on stdout (got: $RELAY_STDOUT)"
refute 'posted HTTP' "$RELAY_STDOUT" "a 401 was reported as a successful post"

FAKE_HTTP_CODE=404 run_relay remote-404 --remote-only --agent update-skills \
  --state completed --project dresden --detail "nothing changed"
[[ $RELAY_RC -eq 0 ]] || fail "a 404 broke the always-exit-0 contract (rc=$RELAY_RC)"
grep -qE '^relay: post FAILED HTTP 404$' <<<"$RELAY_STDOUT" ||
  fail "a 404 was not reported on stdout (got: $RELAY_STDOUT)"

# Transport failure: curl exits non-zero and reports 000 (gateway down). The
# always-exit-0 contract holds and the failure is still reported.
FAKE_HTTP_CODE=000 FAKE_CURL_RC=7 run_relay remote-down --remote-only \
  --agent update-skills --state completed --project dresden --detail "nothing changed"
[[ $RELAY_RC -eq 0 ]] || fail "a dead gateway broke the always-exit-0 contract (rc=$RELAY_RC)"
grep -qE '^relay: post FAILED HTTP 000' <<<"$RELAY_STDOUT" ||
  fail "a dead gateway was not reported on stdout (got: $RELAY_STDOUT)"

# ── 4. NOTHING SENT is reported too. With no hermes key the leg cannot fire; a
#      silent exit 0 there is indistinguishable from a delivered entry, which is
#      the exact bug class this whole feature exists to end. ──────────────────
AUTH_FILE="$tmp/auth-empty.json" run_relay remote-nokey --remote-only \
  --agent update-skills --state completed --project dresden --detail "nothing changed"
[[ $RELAY_RC -eq 0 ]] || fail "a missing hermes key broke the always-exit-0 contract (rc=$RELAY_RC)"
grep -qE '^relay: post SKIPPED' <<<"$RELAY_STDOUT" ||
  fail "a missing hermes key produced NO line at all; silence reads as a delivered entry (got: $RELAY_STDOUT)"
refute_file "$tmp/remote-nokey.curl-argv" "a keyless --remote-only run still called a webhook"

# ── 5. --local-only + --remote-only suppress every channel. That is the honest
#      conjunction (each flag suppresses a set), and it must SAY so rather than
#      exit 0 having done nothing. ────────────────────────────────────────────
run_relay remote-both --local-only --remote-only --agent update-skills \
  --state completed --project dresden --detail "nothing changed" --pane wW:p8
[[ $RELAY_RC -eq 0 ]] || fail "--local-only --remote-only broke the always-exit-0 contract (rc=$RELAY_RC)"
grep -qE '^relay: post SKIPPED' <<<"$RELAY_STDOUT" ||
  fail "--local-only --remote-only sent nothing and said nothing (got: $RELAY_STDOUT)"
refute_file "$tmp/remote-both.curl-argv" "--local-only --remote-only still called a webhook"
refute_file "$tmp/remote-both.tn" "--local-only --remote-only still fired a banner"

# ── 6. Flag-parse safety (the F4 bug class): a value-taking flag whose next
#      token is --remote-only must NOT swallow it as the value. Swallowing it
#      would leave remote_only empty and pop a banner + phone push on every
#      weekly heartbeat -- silently, since the entry would still be delivered. ─
run_relay remote-f4 --agent update-skills --state completed --project dresden \
  --detail "nothing changed" --pane --remote-only
[[ $RELAY_RC -eq 0 ]] || fail "--pane --remote-only broke the always-exit-0 contract (rc=$RELAY_RC)"
grep -qiF 'pane' <<<"$RELAY_STDERR" ||
  fail "no stderr warning that --pane lacked its value"
refute_file "$tmp/remote-f4.tn" \
  "--remote-only was consumed as the --pane value; a banner fired anyway"
refute "moshi\.test" "$(cat "$tmp/remote-f4.curl-argv" 2>/dev/null || true)" \
  "--remote-only was consumed as the --pane value; the phone was pushed anyway"
grep -qE '^relay: posted HTTP 200$' <<<"$RELAY_STDOUT" ||
  fail "--pane --remote-only lost the remote-only delivery entirely"

# ── 7. The ALERT path is untouched. Without --remote-only the hermes POST stays
#      fire-and-forget: no status line, and the banner + phone still fire. A
#      synchronous alert path would add a 5s stall to ~15 pushes per weekly run. ─
run_relay alert-path --agent update-skills --state exhausted --project skills \
  --detail "something broke" --pane wW:p8
[[ $RELAY_RC -eq 0 ]] || fail "the alert path exit was $RELAY_RC"
refute '^relay: (posted|post FAILED|post SKIPPED)' "$RELAY_STDOUT" \
  "the alert path started reporting a delivery status; it must stay fire-and-forget"
# The backgrounded channels are grandchildren relay does not wait for; poll for
# them (this is the one place polling is correct -- asynchrony is the contract).
for ((i = 0; i < 100; i++)); do
  [[ -s "$tmp/alert-path.tn" ]] && grep -qF "moshi.test" "$tmp/alert-path.curl-argv" 2>/dev/null && break
  sleep 0.05
done
grep -qF "moshi.test" "$tmp/alert-path.curl-argv" ||
  fail "the alert path lost its phone push"
grep -qF 'herdr agent focus wW:p8' "$tmp/alert-path.tn" ||
  fail "the alert path lost its macOS banner"

# ── 8. No secret material anywhere the operator or a log can see. The hermes
#      key must never reach argv or stdout/stderr; only the HMAC of the body may
#      leave, and the moshi token may only ride in the moshi BODY (which
#      --remote-only never sends). ────────────────────────────────────────────
all_stdout=""
all_stderr=""
all_argv=""
for label in remote-basic remote-401 remote-404 remote-down remote-nokey remote-both remote-f4; do
  [[ -f "$tmp/$label.stderr" ]] && all_stderr+="$(cat "$tmp/$label.stderr")"$'\n'
  [[ -f "$tmp/$label.curl-argv" ]] && all_argv+="$(cat "$tmp/$label.curl-argv")"$'\n'
done
all_stdout="$RELAY_STDOUT"
for pattern in 'HERMES-KEY-FIXTURE' 'MOSHI-TOKEN-FIXTURE'; do
  refute "$pattern" "$all_argv" "a secret reached curl's argv ($pattern)"
  refute "$pattern" "$all_stderr" "a secret reached stderr ($pattern)"
  refute "$pattern" "$all_stdout" "a secret reached stdout ($pattern)"
done
# The remote-only body itself must carry no key: it is {agent,state,project,detail}
# and the key only ever signs it.
refute 'HERMES-KEY-FIXTURE' "$(cat "$tmp/remote-basic.curl-stdin")" \
  "the hermes signing key leaked into the posted body"

printf 'relay-remote-only: OK\n'
