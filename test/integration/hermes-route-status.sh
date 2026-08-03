#!/usr/bin/env bash
#
# run_after_68-hermes-relay-route-status: the apply-time reminder that a webhook
# route hermes is meant to serve is not actually being served.
#
# This script had NO coverage at all, which matters more now that a second route
# depends on it. Two ways a route dies quietly:
#
#   1. It is absent from ~/.hermes/config.yaml (the apply did not reach the
#      encrypted config). Every entry then 404s and the channel stays empty.
#   2. It is IN config.yaml but the running gateway never loaded it, because
#      hermes reads its config at start-up. Same symptom.
#
# The record channel makes the second case worse than it was for alerts: an alert
# that fails to arrive is usually noticed because the thing it was about is still
# broken, while a record that fails to arrive looks exactly like the healthy job
# it was reporting on having nothing to say.
#
# It stays a REMINDER, never an auto-restart: `hermes gateway restart` drains
# in-flight runs for up to 180s, and routes change almost never.
#
# Integration: renders the real template with the host chezmoi, then runs the
# rendered script against a fake HOME with curl and hermes stubbed. No network,
# no gateway, no restart.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEMPLATE="$REPO_ROOT/.chezmoiscripts/run_after_68-hermes-relay-route-status.sh.tmpl"
LIB="$REPO_ROOT/dot_local/bin/unattended-log-lib.sh"

fail() {
  printf 'hermes-route-status: FAIL -- %s\n' "$*" >&2
  exit 1
}

refute() {
  local pattern="$1" haystack="$2" message="$3"
  if grep -qE "$pattern" <<<"$haystack"; then
    printf '=== haystack ===\n%s\n' "$haystack" >&2
    fail "$message"
  fi
}

[[ -f $TEMPLATE ]] || fail "missing template: $TEMPLATE"
[[ -r $LIB ]] || fail "missing library: $LIB"
command -v chezmoi >/dev/null 2>&1 || fail "chezmoi is not on PATH; the template cannot be rendered"
command -v yq >/dev/null 2>&1 || fail "yq is not on PATH; the script under test needs it"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# ── Route-name agreement. The library posts to a path segment, the config
#    declares a route block by that name, and this script probes it. A rename in
#    one place alone is a permanent 404 that relay dutifully reports into a run
#    log nobody reads. ────────────────────────────────────────────────────────
log_route="$(sed -n 's/^UNATTENDED_LOG_ROUTE="\([^"]*\)".*/\1/p' "$LIB" | head -1)"
[[ -n $log_route ]] || fail "could not read UNATTENDED_LOG_ROUTE out of $LIB"
# Read the PROBE LIST, not the file. Both route names appear several times in
# this template's own comments, so a plain `grep -qF <route> $TEMPLATE` passes on
# a template that documents a route it no longer probes -- an assertion that
# cannot fail. The list itself is what has to agree with the library.
probe_list="$(sed -n 's/^expected_routes=(\(.*\))[[:space:]]*$/\1/p' "$TEMPLATE" | head -1)"
[[ -n $probe_list ]] ||
  fail "could not read the expected_routes list out of $TEMPLATE (has the probe list been renamed?)"
grep -qE "(^| )$log_route( |$)" <<<"$probe_list" ||
  fail "run_after_68 does not probe the '$log_route' route the log library posts to; it probes: $probe_list"
grep -qE '(^| )relay( |$)' <<<"$probe_list" ||
  fail "run_after_68 no longer probes the alert route; it probes: $probe_list"

# ── The stale comment. It cited run_after_67 as the step that applies and
#    migrates the hermes config; 67 is the rotate-logs LaunchAgent loader and has
#    nothing to do with hermes. A comment that names the wrong neighbour sends
#    the next reader to the wrong file. ──────────────────────────────────────
refute 'run_after_67' "$(cat "$TEMPLATE")" \
  "run_after_68 still cites run_after_67 as the hermes config step; 67 is the rotate-logs loader"

rendered="$tmp/status.sh"
render_home="$(mktemp -d)"
HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
  <"$TEMPLATE" >"$rendered" || fail "chezmoi failed to render the template"
rm -rf "$render_home"
[[ -s $rendered ]] || fail "the template rendered empty on this host (darwin); the reminder would never run"

stub_dir="$tmp/stubs"
mkdir -p "$stub_dir"
CURL_LOG="$tmp/curl.log"
HERMES_LOG="$tmp/hermes.log"
export CURL_LOG HERMES_LOG
# curl stub: answers with the code named for the route in the URL, via
# CODE_<route-with-dashes-as-underscores>, defaulting to 200.
cat >"$stub_dir/curl" <<'STUB'
#!/usr/bin/env bash
url=""
for arg in "$@"; do
  case "$arg" in http*) url="$arg" ;; esac
done
printf '%s\n' "$url" >>"$CURL_LOG"
route="${url##*/}"
var="CODE_${route//-/_}"
printf '%s' "${!var:-200}"
STUB
# hermes stub: nothing here may ever restart the gateway.
cat >"$stub_dir/hermes" <<'STUB'
#!/usr/bin/env bash
printf 'hermes %s\n' "$*" >>"$HERMES_LOG"
STUB
chmod +x "$stub_dir/curl" "$stub_dir/hermes"

fake_home="$tmp/home"
mkdir -p "$fake_home/.hermes"

# CONFIG_PORT is what the fixture declares. It is deliberately NOT 8644 by
# default: 8644 is also the script's own fallback, so a fixture on the default
# port cannot tell "reads the port out of the config" from "ignores the config
# and hardcodes 8644", and the library's comment leans on run_after_68 probing
# the live port as the check that catches a port drift.
CONFIG_PORT=8899
write_config() { # <route>...
  local route
  {
    printf 'platforms:\n  webhook:\n    enabled: true\n    extra:\n      host: 127.0.0.1\n      port: %s\n      routes:\n' "$CONFIG_PORT"
    for route in "$@"; do
      printf '        %s:\n          deliver: discord\n          deliver_only: true\n' "$route"
    done
  } >"$fake_home/.hermes/config.yaml"
}

STATUS_OUT=""
STATUS_RC=0
run_status() {
  : >"$CURL_LOG"
  : >"$HERMES_LOG"
  STATUS_OUT="$(HOME="$fake_home" PATH="$stub_dir:$PATH" bash "$rendered" 2>&1)"
  STATUS_RC=$?
}

# ── 1. Both routes present and served -> silent, exit 0. A reminder that fires
#      on a healthy machine is a reminder that gets ignored. ─────────────────
write_config relay "$log_route"
run_status
[[ $STATUS_RC -eq 0 ]] || fail "a healthy gateway exited $STATUS_RC"
[[ -z ${STATUS_OUT//[[:space:]]/} ]] || fail "a healthy gateway produced output: $STATUS_OUT"
[[ "$(grep -c . "$CURL_LOG")" -eq 2 ]] ||
  fail "expected both routes to be probed, got: $(cat "$CURL_LOG")"
# ...on the port the CONFIG declares. A gateway moved off 8644 would otherwise be
# probed at 8644, answer nothing, and the reminder would stay silent forever on
# the one machine that needed it.
for route in relay "$log_route"; do
  grep -qxF "http://127.0.0.1:$CONFIG_PORT/webhooks/$route" "$CURL_LOG" ||
    fail "the $route probe did not use the port the config declares ($CONFIG_PORT): $(cat "$CURL_LOG")"
done

# A config whose port is not a number falls back to 8644 rather than reaching
# curl inside the URL, where it would build a nonsense endpoint that always
# answers 000 and never reports anything.
CONFIG_PORT='not-a-port'
write_config relay "$log_route"
run_status
[[ $STATUS_RC -eq 0 ]] || fail "a non-numeric port exited $STATUS_RC"
grep -qxF "http://127.0.0.1:8644/webhooks/relay" "$CURL_LOG" ||
  fail "a non-numeric port did not fall back to 8644: $(cat "$CURL_LOG")"
CONFIG_PORT=8899
write_config relay "$log_route"

# ── 2. The LOG route is missing from config.yaml -> named and reported. Before
#      this, the script gated on relay alone and a config without the log route
#      passed silently while every record 404'd. ────────────────────────────
write_config relay
run_status
[[ $STATUS_RC -eq 0 ]] || fail "a missing route exited $STATUS_RC; this is a reminder, not a gate"
grep -qF "$log_route" <<<"$STATUS_OUT" ||
  fail "a config with no '$log_route' route was not reported: '$STATUS_OUT'"
grep -qiE 'chezmoi apply|not in|missing' <<<"$STATUS_OUT" ||
  fail "the report does not say what to do about the missing route: '$STATUS_OUT'"

# ── 3. The ALERT route is missing -> named too. Symmetry matters: the script
#      used to treat a missing relay route as "nothing to check". ────────────
write_config "$log_route"
run_status
grep -qF 'relay' <<<"$STATUS_OUT" || fail "a config with no relay route was not reported: '$STATUS_OUT'"

# ── 4. In config but 404 from the gateway -> the restart reminder, naming the
#      route that is not loaded, and NOT accusing the one that is. ───────────
write_config relay "$log_route"
CODE_unattended_upgrades=404 run_status
[[ $STATUS_RC -eq 0 ]] || fail "a 404 exited $STATUS_RC"
grep -qF "$log_route" <<<"$STATUS_OUT" || fail "the 404 route was not named: '$STATUS_OUT'"
grep -qF 'hermes gateway restart' <<<"$STATUS_OUT" ||
  fail "the 404 report does not name the remedy: '$STATUS_OUT'"
refute '404.*relay[^-]|relay.*is in config' "$STATUS_OUT" \
  "a 404 on one route accused the other, which is being served"

# ── 5. It NEVER restarts anything. `hermes gateway restart` drains in-flight
#      runs for up to 180s; an apply-time script must not do that. ───────────
[[ ! -s $HERMES_LOG ]] || fail "the reminder invoked hermes: $(cat "$HERMES_LOG")"

# ── 6. A non-404 answer is not a restart reminder. 401 means the route IS
#      loaded and the key disagrees, which a restart does not fix. ───────────
CODE_relay=401 CODE_unattended_upgrades=401 run_status
refute 'gateway restart' "$STATUS_OUT" "a 401 was reported as a route that needs a restart"

# ── 7. Absent prerequisites stay a silent no-op, exit 0. A machine without
#      hermes or without yq must not have its apply shout at it. ─────────────
rm -f "$fake_home/.hermes/config.yaml"
run_status
[[ $STATUS_RC -eq 0 ]] || fail "an absent config exited $STATUS_RC"
[[ -z ${STATUS_OUT//[[:space:]]/} ]] || fail "an absent config produced output: $STATUS_OUT"
write_config relay "$log_route"
noyq="$tmp/noyq"
mkdir -p "$noyq"
cp "$stub_dir/curl" "$noyq/curl"
STATUS_OUT="$(HOME="$fake_home" PATH="$noyq:/usr/bin:/bin" bash "$rendered" 2>&1)"
STATUS_RC=$?
[[ $STATUS_RC -eq 0 ]] || fail "a host without yq exited $STATUS_RC"
[[ -z ${STATUS_OUT//[[:space:]]/} ]] || fail "a host without yq produced output: $STATUS_OUT"

printf 'hermes-route-status: OK\n'
