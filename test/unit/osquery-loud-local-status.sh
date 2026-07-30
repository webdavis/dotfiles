#!/usr/bin/env bash
#
# _loud_local must report a TRUTHFUL delivery status instead of discarding it:
# 0 when the notifier was successfully invoked (the banner posted, or resolved
# cleanly), nonzero when no notifier exists or the invocation failed. Verified
# alerter (26.5) semantics behind the design: the alerter process lives for the
# banner's whole lifetime and exits only at resolution (dismiss, click, or
# timeout, always exit 0), and a FAILED invocation exits fast and nonzero, so a
# short bounded grace window distinguishes the two without blocking a caller
# for the banner's 60-second life. This seam is what DR-C T2 builds durable
# local notifications on; without it a failed CRIT banner is indistinguishable
# from a shown one.
#
# Unit test: stub notifier binaries on PATH, plus one self-contained errexit
# pin for the drain's one-CRIT caller.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DISPATCH="$REPO_ROOT/dot_local/libexec/osquery/executable_alert-dispatch.sh"

fail() {
  printf 'osquery-loud-local-status: FAIL -- %s\n' "$*" >&2
  exit 1
}

[[ -f $DISPATCH ]] || fail "missing dispatch library: $DISPATCH"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/bin"

# The stubs own the notifier surface: every case below manipulates ONLY
# $work/bin, prepended to a minimal PATH that keeps coreutils reachable.
stub_path="$work/bin:/usr/bin:/bin"

# run_loud_local <expected-status> <context> -- source the library under the
# stub PATH in a clean subshell and call _loud_local once.
run_loud_local() {
  local expected="$1" context="$2" got=0
  PATH="$stub_path" bash -c "source '$DISPATCH'; _loud_local 'title' 'message'" || got=$?
  if [[ $expected == "0" ]]; then
    [[ $got -eq 0 ]] || fail "$context: expected status 0, got $got"
  else
    [[ $got -ne 0 ]] || fail "$context: expected a NONZERO status, got 0"
  fi
}

# (a) Notifier present and successful: an alerter stub that exits 0 promptly
# (a banner that posted and resolved) reports success.
printf '#!/usr/bin/env bash\nexit 0\n' >"$work/bin/alerter"
chmod +x "$work/bin/alerter"
run_loud_local 0 "alerter succeeds"

# (c) Notifier invocation FAILS: an alerter that exits fast and nonzero (the
# verified failed-invocation shape, e.g. usage error 64) must NOT read as a
# delivered banner.
printf '#!/usr/bin/env bash\nexit 64\n' >"$work/bin/alerter"
chmod +x "$work/bin/alerter"
run_loud_local 1 "alerter invocation fails"

# (a2) Long-lived alerter = the banner is UP (the process lives for the
# banner's lifetime). Still-alive after the bounded grace window is success,
# and _loud_local must return well before the banner's own life ends.
printf '#!/usr/bin/env bash\nsleep 30\nexit 0\n' >"$work/bin/alerter"
chmod +x "$work/bin/alerter"
start_seconds=$SECONDS
run_loud_local 0 "alerter long-lived (banner up)"
elapsed=$((SECONDS - start_seconds))
[[ $elapsed -lt 10 ]] || fail "banner-up path blocked ${elapsed}s; must not wait out the banner"

# (b) No notifier at all: no alerter on PATH and a failing osascript must
# report nonzero, never a silent fake success.
rm -f "$work/bin/alerter"
printf '#!/usr/bin/env bash\nexit 1\n' >"$work/bin/osascript"
chmod +x "$work/bin/osascript"
run_loud_local 1 "no alerter, osascript fails"

# (b2) The osascript fallback CAN succeed: it posts and returns immediately,
# so its own exit status is the outcome.
printf '#!/usr/bin/env bash\nexit 0\n' >"$work/bin/osascript"
chmod +x "$work/bin/osascript"
run_loud_local 0 "osascript fallback succeeds"

# (d) Errexit safety of the existing one-CRIT caller: a drain pass that
# dead-letters a record fires _loud_local; with a FAILING notifier the drain
# must complete and exit 0 under set -euo pipefail, never abort mid-sweep.
command -v sqlite3 >/dev/null 2>&1 || {
  printf 'osquery-loud-local-status: OK (a, a2, b, b2, c; d skipped: no sqlite3)\n'
  exit 0
}
rm -f "$work/bin/osascript"
printf '#!/usr/bin/env bash\nexit 64\n' >"$work/bin/alerter" # notifier always fails
chmod +x "$work/bin/alerter"
export OSQUERY_UNDELIVERED_ALERTS_DB="$work/undelivered.sqlite3"
export OSQUERY_DELIVERY_LOG="$work/delivery.log"
export OSQUERY_WEBHOOK_SECRET="unit-secret"
export OSQUERY_DRAIN_MAX_ATTEMPTS=1 # the seeded row dead-letters pre-POST (no curl needed)
drain_output="$(
  PATH="$stub_path" bash -c "
    set -euo pipefail
    source '$DISPATCH'
    _osquery_store_alert_row 1000 osquery-errexit-pin \
      'http://127.0.0.1:8644/webhooks/osquery-priority' \
      \"\$(printf '{}' | base64)\"
    sqlite3 \"\$OSQUERY_UNDELIVERED_ALERTS_DB\" \
      \"UPDATE pending_alerts SET attempts=1 WHERE request_id='osquery-errexit-pin';\"
    retry_undelivered_alerts
    echo DRAIN-DONE
  "
)" || fail "the drain aborted when _loud_local failed (not errexit-safe)"
[[ $drain_output == *DRAIN-DONE* ]] || fail "the drain never reached completion past a failing _loud_local"

printf 'osquery-loud-local-status: OK (success, fast-fail, banner-up, absent, fallback, errexit-safe caller)\n'
