#!/usr/bin/env bash
#
# homebrew-weekly-upgrade.sh: failure alerts, and the weekly RECORD.
#
# This helper relays NOTHING today, success or failure. A weekly brew upgrade can
# fail every step and never tell anyone: the only trace is a line in
# ~/.local/log/homebrew/weekly-upgrade.log that nobody opens. So it gains both
# halves of the same rule the skills updater follows. Act on one, record the
# other:
#
#   - FAILURES go to the EXISTING relay route, which lands them in the priority
#     channel, exactly like every other alert on this machine.
#   - Every scheduled run, including one that upgraded nothing, posts a RECORD to
#     the separate #unattended-upgrades channel, carrying its own gap figure.
#
# And it gains a --scheduled marker, mirroring update-skills.sh. Without one,
# `just brew-upgrade` on a Wednesday would post a weekly entry and a dead
# LaunchAgent would look alive, which inverts the signal the record carries.
#
# End-to-end: the real helper with brew, mas, tailscaled and relay stubbed at the
# boundary. No real upgrades, no sleeps.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HELPER="$REPO_ROOT/dot_local/bin/executable_homebrew-weekly-upgrade.sh"

fail() {
  printf 'homebrew-weekly-record: FAIL -- %s\n' "$*" >&2
  exit 1
}

refute() {
  local pattern="$1" haystack="$2" message="$3"
  if grep -qE "$pattern" <<<"$haystack"; then
    printf '=== haystack ===\n%s\n' "$haystack" >&2
    fail "$message"
  fi
}

[[ -x $HELPER ]] || fail "helper not executable: $HELPER"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

HOME="$tmp/home"
export HOME
mkdir -p "$HOME/.local/bin"

RELAY_LOG="$tmp/relay.log"
export RELAY_LOG
: >"$RELAY_LOG"
# One line per call carrying the route it used, so an entry's multi-line body
# stays greppable and the ALERT route and the LOG route are distinguishable.
cat >"$HOME/.local/bin/relay.sh" <<'STUB'
#!/usr/bin/env bash
printf 'CALL url=%s ARGV %s\n' "${RELAY_HERMES_URL:-<default>}" "$(printf '%s ' "$@" | tr '\n' ' ')" >>"$RELAY_LOG"
printf 'relay: posted HTTP 200\n'
exit 0
STUB
chmod +x "$HOME/.local/bin/relay.sh"

export UNATTENDED_LOG_HERMES_URL="http://hermes.test/webhooks/unattended-upgrades"

# brew stub: `list --versions` prints whatever BREW_VERSIONS holds, so a run's
# before/after package set is controllable; any subcommand named in BREW_FAIL
# exits non-zero.
cat >"$tmp/brew" <<'MOCK'
#!/usr/bin/env bash
if [[ ${1:-} == "list" ]]; then
  printf '%s\n' "${BREW_VERSIONS:-}"
  exit 0
fi
for bad in $BREW_FAIL; do
  [[ ${1:-} == "$bad" ]] && exit 1
done
echo "mock brew $*"
exit 0
MOCK
cat >"$tmp/mas" <<'MOCK'
#!/usr/bin/env bash
if [[ ${1:-} == "list" ]]; then
  printf '%s\n' "${MAS_VERSIONS:-}"
  exit 0
fi
echo "mock mas $*"
exit 0
MOCK
chmod +x "$tmp/brew" "$tmp/mas"

STATE_DIR="$HOME/.local/state/homebrew-weekly-upgrade"
MARKER="$STATE_DIR/last-success-at"

RUN_OUTPUT=""
RUN_RC=0
lock_seq=0
# run_helper [args...] -- a fresh lock file per run so the serialize lock never
# self-contends across scenarios.
run_helper() {
  : >"$RELAY_LOG"
  lock_seq=$((lock_seq + 1))
  RUN_OUTPUT="$(HOMEBREW_WEEKLY_BREW="$tmp/brew" HOMEBREW_WEEKLY_MAS="$tmp/mas" \
    HOMEBREW_WEEKLY_TAILSCALED="/nonexistent" \
    HOMEBREW_WEEKLY_LOCKFILE="$tmp/lock.$lock_seq" \
    BREW_FAIL="${BREW_FAIL:-}" BREW_VERSIONS="${BREW_VERSIONS:-}" MAS_VERSIONS="${MAS_VERSIONS:-}" \
    bash "$HELPER" "$@" 2>&1)"
  RUN_RC=$?
}

log_entries() { grep -F "url=$UNATTENDED_LOG_HERMES_URL " "$RELAY_LOG" || true; }
alert_entries() { grep -F 'url=<default> ' "$RELAY_LOG" || true; }
log_entry_count() { log_entries | grep -c 'ARGV' || true; }

export BREW_FAIL=""
export BREW_VERSIONS="jq 1.7.1
yq 4.53.3"
export MAS_VERSIONS="497799835 Xcode (16.2)"

# ── 1. A clean SCHEDULED run posts a record: the class, the run timestamp, and
#      the gap, which on a machine with no recorded success reads as NEVER. ──
rm -rf "$HOME/.local/state"
run_helper --scheduled
[[ $RUN_RC -eq 0 ]] || fail "a clean run exited $RUN_RC: $RUN_OUTPUT"
entries="$(log_entries)"
[[ -n $entries ]] || fail "a clean scheduled run posted no record: $RUN_OUTPUT"
grep -qF -- '--remote-only' <<<"$entries" ||
  fail "the record was not posted with --remote-only (it would banner + buzz every week): $entries"
grep -qF -- '--state completed' <<<"$entries" || fail "the record did not carry the completed class: $entries"
grep -qF -- '--agent homebrew-weekly-upgrade' <<<"$entries" ||
  fail "the record does not name the job it is about: $entries"
grep -qE 'run at [0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z' <<<"$entries" ||
  fail "the record carries no ISO 8601 UTC run timestamp: $entries"
grep -qiF 'NEVER RECORDED' <<<"$entries" ||
  fail "the first record does not state that no successful run has been recorded here: $entries"
# A clean run alerts NOBODY. The alert route is for things to act on.
refute 'url=<default>' "$(cat "$RELAY_LOG")" "a clean run sent an alert"

# ── 2. It changed NOTHING, and says so. Suppressing the empty entry throws away
#      the reason the channel is worth having: on a no-change week the gap figure
#      is the only information the entry carries. ─────────────────────────────
grep -qF 'formulae and casks: 0 of 2 tracked entries changed' <<<"$entries" ||
  fail "a run that upgraded nothing did not report zero package changes: $entries"
grep -qF 'App Store apps: 0 of 1 tracked entries changed' <<<"$entries" ||
  fail "a run that upgraded nothing did not report zero App Store changes: $entries"

# ── 3. The success marker is written, so a later entry reports a real gap. ───
[[ -s $MARKER ]] || fail "a successful run did not record its timestamp at $MARKER"

# ── 4. UPGRADES are named with their version transition. Homebrew does report
#      versions, so unlike the npx skills lane this record can be specific. ───
rm -rf "$HOME/.local/state"
cat >"$tmp/brew" <<'MOCK'
#!/usr/bin/env bash
if [[ ${1:-} == "list" ]]; then
  if [[ -e "$UPGRADE_MARKER" ]]; then printf '%s\n' "$BREW_AFTER"; else printf '%s\n' "$BREW_BEFORE"; fi
  exit 0
fi
[[ ${1:-} == "upgrade" ]] && : >"$UPGRADE_MARKER"
echo "mock brew $*"
exit 0
MOCK
chmod +x "$tmp/brew"
export UPGRADE_MARKER="$tmp/upgraded"
export BREW_BEFORE="jq 1.7.1
yq 4.53.3"
export BREW_AFTER="jq 1.8.0
yq 4.53.3
ripgrep 14.1.1"
rm -f "$UPGRADE_MARKER"
RUN_OUTPUT="$(HOMEBREW_WEEKLY_BREW="$tmp/brew" HOMEBREW_WEEKLY_MAS="$tmp/mas" \
  HOMEBREW_WEEKLY_TAILSCALED="/nonexistent" HOMEBREW_WEEKLY_LOCKFILE="$tmp/lock.upgrade" \
  MAS_VERSIONS="$MAS_VERSIONS" bash "$HELPER" --scheduled 2>&1)"
entries="$(log_entries)"
grep -qF 'jq 1.7.1 -> 1.8.0' <<<"$entries" ||
  fail "an upgraded formula's version transition was not reported: $entries | $RUN_OUTPUT"
grep -qF 'ripgrep (added)' <<<"$entries" ||
  fail "a newly installed formula was not reported: $entries"
refute 'yq 4' "$entries" "an unchanged formula was listed as changed"

# ── 5. FAILURES go to the EXISTING alert route, and the record still goes out
#      saying how many steps failed. A failing weekly upgrade that tells nobody
#      is the gap this closes. ─────────────────────────────────────────────────
rm -rf "$HOME/.local/state"
cat >"$tmp/brew" <<'MOCK'
#!/usr/bin/env bash
if [[ ${1:-} == "list" ]]; then printf '%s\n' "${BREW_VERSIONS:-}"; exit 0; fi
for bad in $BREW_FAIL; do
  [[ ${1:-} == "$bad" ]] && exit 1
done
echo "mock brew $*"
exit 0
MOCK
chmod +x "$tmp/brew"
BREW_FAIL="upgrade cleanup" run_helper --scheduled
[[ $RUN_RC -ne 0 ]] || fail "a run with failed steps exited 0"
alerts="$(alert_entries)"
[[ -n $alerts ]] || fail "a failing weekly upgrade sent NO alert: $RUN_OUTPUT"
grep -qF -- '--agent homebrew-weekly-upgrade' <<<"$alerts" || fail "the alert does not name the job: $alerts"
grep -qE -- '--state [a-z-]+' <<<"$alerts" || fail "the alert carries no state: $alerts"
refute '[-][-]remote-only' "$alerts" "the alert used the record route's flag; it must land in the priority channel"
# Anchored to the step LIST, not to loose prose: "Homebrew upgrade" contains the
# substring "brew upgrade", so a bare grep for it passes on an alert that dropped
# the list entirely. Both failed steps must be named.
grep -qE 'failed step\(s\):[^.]*brew upgrade' <<<"$alerts" ||
  fail "the alert does not name which step failed, so there is nothing to act on: $alerts"
grep -qE 'failed step\(s\):[^.]*brew cleanup' <<<"$alerts" ||
  fail "the alert named only the first failed step: $alerts"
entries="$(log_entries)"
grep -qE 'failed steps: [1-9]' <<<"$entries" ||
  fail "the record does not state the failed-step count: $entries"
# A failing run must NOT claim a successful run for the gap figure.
[[ ! -e $MARKER ]] || fail "a failing run recorded itself as the last SUCCESSFUL run"

# ── 6. A MANUAL run (what `just brew-upgrade` does) posts NO record. Otherwise a
#      hand-run makes a dead LaunchAgent look alive. It still ALERTS on failure:
#      a failure is a failure whoever started it. ──────────────────────────────
rm -rf "$HOME/.local/state"
BREW_FAIL="" run_helper
[[ $RUN_RC -eq 0 ]] || fail "a clean manual run exited $RUN_RC"
[[ "$(log_entry_count)" -eq 0 ]] || fail "a MANUAL run posted a weekly record: $(log_entries)"
BREW_FAIL="upgrade" run_helper
[[ "$(log_entry_count)" -eq 0 ]] || fail "a MANUAL failing run posted a weekly record: $(log_entries)"
[[ -n "$(alert_entries)" ]] || fail "a MANUAL failing run sent no alert; a failure is a failure whoever started it"

# ── 7. One record per ISO week: `just brew-upgrade --scheduled` twice, or a
#      launchd catch-up landing beside the real slot, must not double-post. ────
rm -rf "$HOME/.local/state"
BREW_FAIL="" run_helper --scheduled
[[ "$(log_entry_count)" -eq 1 ]] || fail "the first scheduled run posted $(log_entry_count) entries"
run_helper --scheduled
[[ "$(log_entry_count)" -eq 0 ]] || fail "a second scheduled run in the same week posted again"

# ── 8. LOCK CONTENTION is a deferral, not a failure: nothing was attempted, so
#      it is recorded rather than alerted. ─────────────────────────────────────
rm -rf "$HOME/.local/state"
: >"$tmp/held.lock"
lock_holder_out="$tmp/holder.out"
(
  exec 9>>"$tmp/held.lock"
  /usr/bin/lockf -s -t 0 9 2>/dev/null || exit 1
  : >"$lock_holder_out"
  while [[ -e "$tmp/hold-me" ]]; do sleep 0.05; done
) &
holder_pid=$!
: >"$tmp/hold-me"
for ((i = 0; i < 100; i++)); do
  [[ -e $lock_holder_out ]] && break
  sleep 0.05
done
if [[ -e $lock_holder_out ]]; then
  : >"$RELAY_LOG"
  HOMEBREW_WEEKLY_BREW="$tmp/brew" HOMEBREW_WEEKLY_MAS="$tmp/mas" \
    HOMEBREW_WEEKLY_TAILSCALED="/nonexistent" HOMEBREW_WEEKLY_LOCKFILE="$tmp/held.lock" \
    BREW_VERSIONS="$BREW_VERSIONS" bash "$HELPER" --scheduled >/dev/null 2>&1
  contended_rc=$?
  rm -f "$tmp/hold-me"
  wait "$holder_pid" 2>/dev/null || true
  [[ $contended_rc -eq 75 ]] || fail "lock contention exited $contended_rc, want 75"
  entries="$(log_entries)"
  grep -qF -- '--state deferred' <<<"$entries" ||
    fail "lock contention did not record a deferral: $entries"
  refute 'url=<default>' "$(cat "$RELAY_LOG")" "lock contention alerted; nothing was attempted, so it is a record not an alert"
else
  rm -f "$tmp/hold-me"
  wait "$holder_pid" 2>/dev/null || true
  fail "could not stage a held lock; the contention case did not run"
fi

# ── 9. An unknown argument is an error, not a silent no-op that skips the
#      record. A typo'd marker in the plist would otherwise run every week and
#      quietly post nothing. ────────────────────────────────────────────────────
run_helper --schedluled
[[ $RUN_RC -ne 0 ]] || fail "an unknown argument exited 0"
grep -qiE 'usage|unknown' <<<"$RUN_OUTPUT" ||
  fail "an unknown argument produced no usage message: $RUN_OUTPUT"

printf 'homebrew-weekly-record: OK\n'
