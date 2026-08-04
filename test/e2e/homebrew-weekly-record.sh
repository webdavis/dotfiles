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
# The osquery file-integrity triage helper, sourced ONLY to read back the record
# path it expects. It is the consumer of what this job persists, and the two must
# not agree merely by copy-paste.
TRIAGE_HELPER="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/file-integrity-triage.sh"

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
if { : >&9; } 2>/dev/null; then fd9=inherited; else fd9=closed; fi
printf 'CALL url=%s fd9=%s ARGV %s\n' "${RELAY_HERMES_URL:-<default>}" "$fd9" "$(printf '%s ' "$@" | tr '\n' ' ')" >>"$RELAY_LOG"
# The real relay always exits 0 and reports its delivery outcome on stdout, so
# RELAY_STUB_OUTCOME is the only way a caller can tell a delivered entry from a
# refused one.
printf '%s\n' "${RELAY_STUB_OUTCOME:-relay: posted HTTP 200}"
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
# mas stub: `list` answers MAS_BEFORE until an upgrade has run and MAS_AFTER
# afterwards (keyed off the same marker the brew stub uses), so an App Store
# version transition is observable. MAS_AFTER defaults to MAS_BEFORE, which
# keeps the no-change sections reading as they did.
cat >"$tmp/mas" <<'MOCK'
#!/usr/bin/env bash
if [[ ${1:-} == "list" ]]; then
  # MAS_FAIL=first fails only the first reading of a run, MAS_FAIL=later fails
  # every reading after the first, so each of the two snapshot points can be
  # broken on its own. Anything else fails every reading.
  if [[ -n ${MAS_FAIL:-} ]]; then
    seen=""
    [[ -n ${MAS_FAIL_MARKER:-} && -e ${MAS_FAIL_MARKER:-} ]] && seen=1
    [[ -n ${MAS_FAIL_MARKER:-} ]] && : >"$MAS_FAIL_MARKER"
    case "$MAS_FAIL" in
      first) [[ -z $seen ]] && { printf 'Error: mas is broken\n' >&2; exit 1; } ;;
      later) [[ -n $seen ]] && { printf 'Error: mas is broken\n' >&2; exit 1; } ;;
      *) printf 'Error: mas is broken\n' >&2; exit 1 ;;
    esac
  fi
  if [[ -n ${UPGRADE_MARKER:-} && -e ${UPGRADE_MARKER:-} ]]; then
    printf '%s\n' "${MAS_AFTER:-${MAS_VERSIONS:-}}"
  else
    printf '%s\n' "${MAS_VERSIONS:-}"
  fi
  exit 0
fi
[[ ${1:-} == "upgrade" && -n ${UPGRADE_MARKER:-} ]] && : >"$UPGRADE_MARKER"
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
    RELAY_STUB_OUTCOME="${RELAY_STUB_OUTCOME:-}" LIST_FAIL_ONCE="${LIST_FAIL_ONCE:-}" \
    MAS_FAIL="${MAS_FAIL:-}" MAS_FAIL_MARKER="${MAS_FAIL_MARKER:-}" \
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
# The record names the MACHINE. The channel aggregates both weekly jobs and the
# daemon-host role is expected to move to a second Mac, so an entry that does not
# say which machine it is about cannot be investigated.
this_host="$(hostname -s 2>/dev/null || printf '%s' "${HOSTNAME:-unknown-host}")"
grep -qF -- "--project $this_host" <<<"$entries" ||
  fail "the record does not name the host it is about (expected --project $this_host): $entries"
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

# ...and a later run actually SPENDS it. Every entry carries its own gap because
# launchd coalesces missed calendar intervals, so an absent entry proves nothing
# and the figure in the newest message is the whole signal. Only the week guard
# is cleared here (not the marker), and the marker is back-dated 8 days so a gap
# read from it differs visibly from one read from this moment.
recorded_iso="$(awk '{print $2}' "$MARKER")"
printf '%s %s\n' "$(($(date +%s) - 691200))" "$recorded_iso" >"$MARKER"
rm -rf "$STATE_DIR/log-week-claims"
run_helper --scheduled
entries="$(log_entries)"
grep -qE 'last successful run: [^ ]+ \(8d 0h ago\)' <<<"$entries" ||
  fail "a later run did not report the real 8-day gap to the previous success: $entries"
refute 'NEVER RECORDED' "$entries" "the entry still claims no run was ever recorded after one succeeded"
[[ "$(awk '{print $1}' "$MARKER")" -gt "$(($(date +%s) - 691200))" ]] ||
  fail "the run did not advance the successful-run marker, so the gap would stay frozen at 8 days"

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
# python@3.12 carries TWO installed versions before the run and one after. That
# is the shape `brew list --versions` prints when a formula keeps an old keg, and
# the whole line is the fingerprint: reading only the first field would report
# "3.12.7 -> 3.12.8" for what is really "3.12.7 3.12.8 -> 3.12.8", i.e. it would
# claim a keg was removed on every week that one was ADDED.
export BREW_BEFORE="jq 1.7.1
python@3.12 3.12.7 3.12.8
yq 4.53.3"
export BREW_AFTER="jq 1.8.0
python@3.12 3.12.8
ripgrep 14.1.1
yq 4.53.3"
export MAS_AFTER="497799835 Xcode (16.3)"
rm -f "$UPGRADE_MARKER"
: >"$RELAY_LOG"
RUN_OUTPUT="$(HOMEBREW_WEEKLY_BREW="$tmp/brew" HOMEBREW_WEEKLY_MAS="$tmp/mas" \
  HOMEBREW_WEEKLY_TAILSCALED="/nonexistent" HOMEBREW_WEEKLY_LOCKFILE="$tmp/lock.upgrade" \
  MAS_VERSIONS="$MAS_VERSIONS" MAS_AFTER="$MAS_AFTER" bash "$HELPER" --scheduled 2>&1)"
entries="$(log_entries)"
# shellcheck disable=SC2016 # the backticks are Discord code-span syntax, not a substitution
grep -qF '`jq` `1.7.1` -> `1.8.0`' <<<"$entries" ||
  fail "an upgraded formula's version transition was not reported: $entries | $RUN_OUTPUT"
# shellcheck disable=SC2016 # the backticks are Discord code-span syntax, not a substitution
grep -qF '`python@3.12` `3.12.7 3.12.8` -> `3.12.8`' <<<"$entries" ||
  fail "a formula with two installed versions was fingerprinted from its first version only: $entries"
# shellcheck disable=SC2016 # the backticks are Discord code-span syntax, not a substitution
grep -qF '`ripgrep` (added)' <<<"$entries" ||
  fail "a newly installed formula was not reported: $entries"
refute 'yq 4' "$entries" "an unchanged formula was listed as changed"
# The App Store lane reports versions too, and it is keyed by app NAME rather
# than by the numeric id a reader would not recognize.
# shellcheck disable=SC2016 # the backticks are Discord code-span syntax, not a substitution
grep -qF 'App Store apps: 1 of 1 tracked entries changed (`Xcode` `16.2` -> `16.3`)' <<<"$entries" ||
  fail "an upgraded App Store app's version transition was not reported: $entries"

# ── 4b. The same run leaves a DURABLE record of what it moved, at the exact path
#       the osquery file-integrity page reads days later. That page fires when a
#       watched file leaves its known-good manifest, and a vendor update and a
#       tamper used to render the same body; the record is what lets it say
#       whether a recorded upgrade plausibly explains the file. The path is
#       asserted by SOURCING the consumer rather than by repeating a literal
#       here: a rename in one of the two scripts alone leaves that page answering
#       no-record forever, which reads exactly like a quiet month of upgrades. ──
record_path="$(bash -c 'source "$1"; printf "%s" "$OSQUERY_UPGRADE_RECORD"' _ "$TRIAGE_HELPER")"
[[ -n $record_path ]] || fail "the triage helper does not name an upgrade record path"
[[ -f $record_path ]] ||
  fail "the run left no upgrade record at the path the file-integrity page reads ($record_path): $RUN_OUTPUT"
read -r record_epoch record_iso <"$record_path"
[[ $record_epoch =~ ^[0-9]+$ ]] ||
  fail "the record does not open with an epoch the correlation can do arithmetic on: $(cat "$record_path")"
[[ $record_iso =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$ ]] ||
  fail "the record does not open with an ISO 8601 UTC stamp the page can render: $(cat "$record_path")"
grep -qF "$(printf 'jq\tchanged\t1.7.1\t1.8.0')" "$record_path" ||
  fail "the record does not carry the version transition as data: $(cat "$record_path")"
grep -qF "$(printf 'ripgrep\tadded\t\t14.1.1')" "$record_path" ||
  fail "the record does not mark a newly installed formula as added: $(cat "$record_path")"
refute '^yq' "$(cat "$record_path")" "the record lists a formula that did not move"
# The App Store lane is deliberately absent: those apps install into
# /Applications, which no known-good manifest covers, so a mas transition could
# never explain one of these pages and would only crowd out the line.
refute 'Xcode' "$(cat "$record_path")" "the record carries App Store apps, which no file-integrity page can be about"
unset MAS_AFTER

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
# Neither relay call site may hand relay the run's serialize-lock fd. The lock is
# a kernel flock on fd 9, relay detaches channels that outlive this run, and a
# flock is released only when the LAST copy of the fd closes -- so an inherited
# copy in a detached curl keeps the lock held after the helper exited and the
# next Monday defers over a run that is already gone.
refute 'fd9=inherited' "$(cat "$RELAY_LOG")" \
  "a relay call inherited the run's serialize-lock fd; a detached child would hold the lock after the run exited"
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

# ── 9. A REFUSED record must not consume the week, and the operator must hear
#      about it on the route that still works. relay exits 0 whatever the gateway
#      answered, by design, so a 401 or a 404 reads exactly like a delivered
#      entry from here: claiming the week on one leaves the week with NO entry
#      while the guard asserts it has one, and a record channel that quietly
#      stopped working looks precisely like a job with nothing to say. ─────────
rm -rf "$HOME/.local/state"
export RELAY_STUB_OUTCOME='relay: post FAILED HTTP 401'
run_helper --scheduled
[[ $RUN_RC -eq 0 ]] ||
  fail "a refused record broke the upgrade it was reporting on (rc=$RUN_RC): $RUN_OUTPUT"
[[ "$(log_entry_count)" -eq 1 ]] ||
  fail "the refused record did not reach the log route once: $(log_entries)"
grep -qF -- '--state log-channel-broken' <<<"$(alert_entries)" ||
  fail "a broken record channel raised no alert on the priority route: $(cat "$RELAY_LOG")"
run_helper --scheduled
[[ "$(log_entry_count)" -eq 1 ]] ||
  fail "a week whose record was refused did not retry on the next run; it stayed claimed with nothing sent"
refute 'log-channel-broken' "$(cat "$RELAY_LOG")" \
  "the broken-channel alert repeated inside one week"
unset RELAY_STUB_OUTCOME
run_helper --scheduled
[[ "$(log_entry_count)" -eq 1 ]] || fail "the retrying run did not post its record: $RUN_OUTPUT"
run_helper --scheduled
[[ "$(log_entry_count)" -eq 0 ]] ||
  fail "a DELIVERED record did not claim the week; every later run would post again"

# ── 10. The run timestamp is the instant the gap under it was measured from. The
#      clock stub moves an hour per reading, so a helper that samples once posts
#      the FIRST reading and one that re-reads at delivery posts a later hour.
#      Two readings is how a long run prints timestamps hours apart from the gap
#      figure printed beneath them, with nothing saying which to believe. ──────
rm -rf "$HOME/.local/state"
mkdir -p "$tmp/stubs"
export CLOCK_TICKS="$tmp/clock-ticks"
: >"$CLOCK_TICKS"
cat >"$tmp/stubs/date" <<'STUB'
#!/usr/bin/env bash
n="$(cat "$CLOCK_TICKS" 2>/dev/null || printf '0')"
[[ $n =~ ^[0-9]+$ ]] || n=0
printf '%s' "$((n + 1))" >"$CLOCK_TICKS"
epoch=$((1785000000 + n * 3600))
iso="$(printf '2026-07-25T%02d:00:00Z' "$((12 + n))")"
for arg in "$@"; do
  case "$arg" in
    "+%s %Y-%m-%dT%H:%M:%SZ") printf '%s %s\n' "$epoch" "$iso"; exit 0 ;;
    +%s) printf '%s\n' "$epoch"; exit 0 ;;
    +%Y-%m-%dT%H:%M:%SZ) printf '%s\n' "$iso"; exit 0 ;;
  esac
done
exec /bin/date "$@"
STUB
chmod +x "$tmp/stubs/date"
: >"$RELAY_LOG"
RUN_OUTPUT="$(PATH="$tmp/stubs:$PATH" HOMEBREW_WEEKLY_BREW="$tmp/brew" HOMEBREW_WEEKLY_MAS="$tmp/mas" \
  HOMEBREW_WEEKLY_TAILSCALED="/nonexistent" HOMEBREW_WEEKLY_LOCKFILE="$tmp/lock.clock" \
  BREW_FAIL="" BREW_VERSIONS="$BREW_VERSIONS" MAS_VERSIONS="$MAS_VERSIONS" \
  bash "$HELPER" --scheduled 2>&1)"
entries="$(log_entries)"
grep -qF 'run at 2026-07-25T12:00:00Z' <<<"$entries" ||
  fail "the record does not report the instant the run started: $entries | $RUN_OUTPUT"

# ── 11. A SNAPSHOT THAT COULD NOT BE READ is stated, never rendered as "nothing
#       changed". Both snapshot commands discarded their errors and produced
#       empty files, so a broken `brew list --versions` rendered the entry as
#       "0 of 0 changed, failed steps 0" -- a clean week, on a machine whose
#       package manager could not even be queried. ─────────────────────────────
rm -rf "$HOME/.local/state"
cat >"$tmp/brew" <<'MOCK'
#!/usr/bin/env bash
if [[ ${1:-} == "list" ]]; then
  printf 'Error: brew is broken\n' >&2
  exit 1
fi
echo "mock brew $*"
exit 0
MOCK
chmod +x "$tmp/brew"
run_helper --scheduled
entries="$(log_entries)"
refute 'formulae and casks: 0 of 0' "$entries" \
  "an unreadable package list rendered as a clean comparison of nothing: $entries"
grep -qiE 'formulae and casks: [^.]*(could not|failed|unknown)' <<<"$entries" ||
  fail "the record does not say the formulae snapshot could not be read: $entries"
grep -qF 'brew list --versions' <<<"$entries" ||
  fail "the record does not name the command that failed, so there is nothing to check: $entries"
# ...and the lane that still worked keeps reporting normally, so one broken
# source does not blank the whole entry.
grep -qF 'App Store apps: 0 of 1 tracked entries changed' <<<"$entries" ||
  fail "a broken brew took the working App Store section down with it: $entries"
# Restore the working stub.
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

# The SAME holds for the other lane, so neither one is covered only by the
# other's mechanism: a broken `mas list` is stated and the formulae section,
# whose source worked, still reports normally.
for when in first later; do
  rm -rf "$HOME/.local/state"
  rm -f "$tmp/mas-fail-marker"
  MAS_FAIL="$when" MAS_FAIL_MARKER="$tmp/mas-fail-marker" run_helper --scheduled
  entries="$(log_entries)"
  grep -qiE 'App Store apps: [^.]*(could not|failed|not compared)' <<<"$entries" ||
    fail "an App Store list that failed on the $when reading was not stated: $entries"
  grep -qF 'mas list' <<<"$entries" ||
    fail "the record does not name the App Store command that failed: $entries"
  grep -qF 'formulae and casks: 0 of 2 tracked entries changed' <<<"$entries" ||
    fail "a broken mas took the working formulae section down with it: $entries"
  refute '\((added|removed)\)' "$entries" \
    "a one-sided App Store reading invented a whole-lane change list: $entries"
done
rm -f "$tmp/mas-fail-marker"

# The FIRST reading counts too. A source that fails before the upgrade and
# succeeds after it has no baseline, so comparing the two would report every
# installed formula as newly added -- a whole-machine change list, invented.
rm -rf "$HOME/.local/state"
cat >"$tmp/brew" <<'MOCK'
#!/usr/bin/env bash
if [[ ${1:-} == "list" ]]; then
  if [[ -n ${LIST_FAIL_ONCE:-} && ! -e ${LIST_FAIL_ONCE:-} ]]; then
    : >"$LIST_FAIL_ONCE"
    printf 'Error: brew was busy\n' >&2
    exit 1
  fi
  printf '%s\n' "${BREW_VERSIONS:-}"
  exit 0
fi
echo "mock brew $*"
exit 0
MOCK
chmod +x "$tmp/brew"
rm -f "$tmp/list-failed-once"
LIST_FAIL_ONCE="$tmp/list-failed-once" run_helper --scheduled
entries="$(log_entries)"
grep -qiE 'formulae and casks: [^.]*(could not|failed|not compared)' <<<"$entries" ||
  fail "a run whose BEFORE reading failed compared against it anyway: $entries"
refute '\(added\)' "$entries" \
  "a missing baseline reported the installed formulae as newly added: $entries"
# ...and so does the SECOND reading, symmetrically: a source that worked before
# the upgrade and failed after it would otherwise report the whole Cellar as
# removed, which is the single most alarming line this record can print.
rm -rf "$HOME/.local/state"
cat >"$tmp/brew" <<'MOCK'
#!/usr/bin/env bash
if [[ ${1:-} == "list" ]]; then
  if [[ -n ${LIST_FAIL_ONCE:-} ]]; then
    if [[ -e $LIST_FAIL_ONCE ]]; then
      printf 'Error: brew broke mid-run\n' >&2
      exit 1
    fi
    : >"$LIST_FAIL_ONCE"
  fi
  printf '%s\n' "${BREW_VERSIONS:-}"
  exit 0
fi
echo "mock brew $*"
exit 0
MOCK
chmod +x "$tmp/brew"
rm -f "$tmp/list-failed-later"
LIST_FAIL_ONCE="$tmp/list-failed-later" run_helper --scheduled
entries="$(log_entries)"
grep -qiE 'formulae and casks: [^.]*(could not|failed|not compared)' <<<"$entries" ||
  fail "a run whose AFTER reading failed compared against it anyway: $entries"
refute '\(removed\)' "$entries" \
  "a failed second reading reported the installed formulae as removed: $entries"
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

# An EMPTY answer from a source that WORKED is not a failure: a machine with no
# App Store apps truthfully has nothing to compare, and calling that unreadable
# would cry wolf on every such machine forever.
rm -rf "$HOME/.local/state"
MAS_VERSIONS="" run_helper --scheduled
entries="$(log_entries)"
grep -qF 'App Store apps: 0 of 0 tracked entries changed' <<<"$entries" ||
  fail "an empty but successful App Store list was not reported as zero tracked entries: $entries"
refute 'App Store apps: [^.]*(could not|failed|unknown)' "$entries" \
  "an empty but successful App Store list was reported as unreadable: $entries"

# ── 12. A lock that cannot be OPENED is NOT another run holding it. Collapsing
#       every non-zero into contention posts a record blaming a holder that does
#       not exist, sends no alert, and exits 75 (retry later) for a condition
#       that will still be there next week. ────────────────────────────────────
rm -rf "$HOME/.local/state"
mkdir -p "$tmp/nolockdir"
chmod 500 "$tmp/nolockdir"
: >"$RELAY_LOG"
RUN_OUTPUT="$(HOMEBREW_WEEKLY_BREW="$tmp/brew" HOMEBREW_WEEKLY_MAS="$tmp/mas" \
  HOMEBREW_WEEKLY_TAILSCALED="/nonexistent" HOMEBREW_WEEKLY_LOCKFILE="$tmp/nolockdir/lock" \
  BREW_FAIL="" BREW_VERSIONS="$BREW_VERSIONS" MAS_VERSIONS="$MAS_VERSIONS" \
  bash "$HELPER" --scheduled 2>&1)"
unopenable_rc=$?
chmod 700 "$tmp/nolockdir"
[[ $unopenable_rc -ne 75 ]] ||
  fail "an unopenable lock exited 75, which is the code for another run holding it: $RUN_OUTPUT"
[[ $unopenable_rc -ne 0 ]] ||
  fail "an unopenable lock exited 0, so nothing ran and nothing said so: $RUN_OUTPUT"
entries="$(log_entries)"
[[ -n $entries ]] || fail "an unopenable lock recorded nothing: $RUN_OUTPUT"
refute 'already holds' "$entries" \
  "the record blames a holder that does not exist for a lock that simply could not be opened: $entries"
grep -qiE 'could not be OPENED|could not open' <<<"$entries" ||
  fail "the record does not say the lock could not be opened: $entries"
grep -qF -- '--agent homebrew-weekly-upgrade' <<<"$(alert_entries)" ||
  fail "an unopenable lock sent no alert; nothing ran and nobody was told: $(cat "$RELAY_LOG")"

# ── 13. A SNAPSHOT WORKSPACE that cannot be allocated must not delete the whole
#       record. This helper deliberately does not run under errexit, so a failed
#       `mktemp -d` (an absent, unwritable or full TMPDIR) left the directory
#       variable empty, every upgrade step still ran, and the record block,
#       guarded on that same variable, was skipped in full: the upgrade
#       happened, the success marker was written, the helper exited 0, and
#       neither channel said one word about the week. That is the exact silence
#       this record exists to end, produced by the record's own bookkeeping. ───
#       The failure is injected with a stub rather than with an unusable TMPDIR:
#       macOS mktemp IGNORES TMPDIR in the bare form and the flake devshell ships
#       GNU coreutils mktemp, which honours it, so a TMPDIR-based injection would
#       test two different things on the host and in CI. The stub fails the
#       helper's own workspace template and delegates everything else.
rm -rf "$HOME/.local/state"
mkdir -p "$tmp/stubs"
real_mktemp="$(command -v mktemp)"
[[ -x $real_mktemp ]] || fail "no mktemp on PATH to delegate to"
cat >"$tmp/stubs/mktemp" <<STUB
#!/usr/bin/env bash
if [[ -n \${MKTEMP_FAIL_TEMPLATE:-} ]]; then
  for arg in "\$@"; do
    if [[ \$arg == *"\$MKTEMP_FAIL_TEMPLATE"* ]]; then
      printf 'mktemp: mkdtemp failed on %s: No such file or directory\n' "\$arg" >&2
      exit 1
    fi
  done
fi
exec "$real_mktemp" "\$@"
STUB
chmod +x "$tmp/stubs/mktemp"
: >"$RELAY_LOG"
RUN_OUTPUT="$(PATH="$tmp/stubs:$PATH" MKTEMP_FAIL_TEMPLATE=homebrew-weekly-record \
  HOMEBREW_WEEKLY_BREW="$tmp/brew" HOMEBREW_WEEKLY_MAS="$tmp/mas" \
  HOMEBREW_WEEKLY_TAILSCALED="/nonexistent" HOMEBREW_WEEKLY_LOCKFILE="$tmp/lock.notmpdir" \
  BREW_FAIL="" BREW_VERSIONS="$BREW_VERSIONS" MAS_VERSIONS="$MAS_VERSIONS" \
  bash "$HELPER" --scheduled 2>&1)"
notmpdir_rc=$?
grep -qiE 'workspace could not be created|mktemp' <<<"$RUN_OUTPUT" ||
  fail "the injected mktemp failure never happened, so this case tested nothing: $RUN_OUTPUT"
entries="$(log_entries)"
[[ -n $entries ]] ||
  fail "a run whose snapshot workspace could not be created posted NO record; the upgrade ran and nothing said so: $RUN_OUTPUT"
grep -qF -- '--state completed' <<<"$entries" ||
  fail "the run reached the end but did not record itself as completed: $entries"
refute '0 of 0' "$entries" \
  "a record with no snapshots at all rendered as a clean comparison of nothing: $entries"
refute '\((added|removed)\)' "$entries" \
  "a record with no snapshots invented a change list: $entries"
for lane in 'formulae and casks' 'App Store apps'; do
  grep -qiE "$lane: [^.]*(could not|failed|not compared)" <<<"$entries" ||
    fail "the record does not say the $lane comparison could not be made: $entries"
done
# ...and it names the thing that actually broke. Both package commands worked on
# this run, so blaming `brew list --versions` would send the operator to check a
# command that is fine.
grep -qiE 'snapshot workspace|mktemp' <<<"$entries" ||
  fail "the record does not name the workspace allocation as what failed: $entries"
# The bookkeeping failure must not become an upgrade failure: every step ran and
# every step succeeded.
[[ $notmpdir_rc -eq 0 ]] ||
  fail "a snapshot workspace that could not be allocated failed the upgrade it was only reporting on (rc=$notmpdir_rc): $RUN_OUTPUT"
grep -qF 'ok: brew upgrade' <<<"$RUN_OUTPUT" ||
  fail "the upgrade steps did not run: $RUN_OUTPUT"

# ── 14. An unknown argument is an error, not a silent no-op that skips the
#       record. A typo'd marker in the plist would otherwise run every week and
#       quietly post nothing. ───────────────────────────────────────────────────
run_helper --schedluled
[[ $RUN_RC -ne 0 ]] || fail "an unknown argument exited 0"
grep -qiE 'usage|unknown' <<<"$RUN_OUTPUT" ||
  fail "an unknown argument produced no usage message: $RUN_OUTPUT"

printf 'homebrew-weekly-record: OK (a scheduled run records its class, host, run timestamp and gap and spends the marker on the next run; version transitions for formulae, multi-version kegs and App Store apps; failures alert the priority route while the record states the count; a snapshot that failed on either reading says NOT COMPARED per lane while an empty-but-successful one does not, and a workspace that could not be allocated still posts an entry naming what failed; a refused record leaves the week unclaimed and alerts once; a lock that cannot be opened is not reported as contention; a manual run records nothing, one record per week, and an unknown argument is an error)\n'
