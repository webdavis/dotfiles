#!/usr/bin/env bash
#
# report-plugin-updates.sh: the record of what Claude Code's own plugin
# auto-update changed.
#
# Claude Code refreshes marketplaces and their installed plugins at startup and
# says nothing about it, so this helper is the only thing on the machine that
# can answer "what moved last week". The failure that matters here is not a
# crash: it is a run that reports NOTHING while a plugin really did move, or
# reports a confident "nothing changed" for a file it could not read. Both are
# pinned below, in both directions.
#
# End-to-end: the real helper against fixture state files, with relay stubbed at
# the boundary. No real plugins, no network, no sleeps.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HELPER="$REPO_ROOT/dot_local/bin/executable_report-plugin-updates.sh"

fail() {
  printf 'report-plugin-updates-record: FAIL -- %s\n' "$*" >&2
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
command -v jq >/dev/null 2>&1 || fail "jq is not on PATH, so the helper cannot read its input"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

HOME="$tmp/home"
export HOME
mkdir -p "$HOME/.local/bin"

# The library the helper sources sits beside it in the source tree, so the
# helper resolves it from the checkout and the sandbox HOME does not need a copy.

RELAY_LOG="$tmp/relay.log"
export RELAY_LOG
: >"$RELAY_LOG"
# One line per call carrying the route it used, so an entry's multi-line body
# stays greppable and the ALERT route and the LOG route are distinguishable.
cat >"$HOME/.local/bin/relay.sh" <<'STUB'
#!/usr/bin/env bash
printf 'CALL url=%s ARGV %s\n' "${RELAY_HERMES_URL:-<default>}" "$(printf '%s ' "$@" | tr '\n' ' ')" >>"$RELAY_LOG"
# The real relay always exits 0 and reports its delivery outcome on stdout, so
# RELAY_STUB_OUTCOME is the only way a caller can tell a delivered entry from a
# refused one.
printf '%s\n' "${RELAY_STUB_OUTCOME:-relay: posted HTTP 200}"
exit 0
STUB
chmod +x "$HOME/.local/bin/relay.sh"

export UNATTENDED_LOG_HERMES_URL="http://hermes.test/webhooks/unattended-upgrades"

STATE_DIR="$HOME/.local/state/report-plugin-updates"
SNAPSHOT="$STATE_DIR/installed-plugins.snapshot"
MARKER="$STATE_DIR/last-success-at"
PLUGIN_STATE="$tmp/installed_plugins.json"

# write_plugin_state <exa-version> [extra-json-entries] -- the fixture Claude
# Code would maintain. The shape is the live file's, verified 2026-08-03:
# schema version 2, a `plugins` object keyed by <name>@<marketplace>, each
# holding an ARRAY of install records. Three plugins cover the three fingerprint
# lanes the helper distinguishes: a real version, a version-less entry with a
# commit, and one with neither.
write_plugin_state() {
  local exa_version="$1"
  cat >"$PLUGIN_STATE" <<EOF
{
  "version": 2,
  "plugins": {
    "exa@claude-plugins-official": [
      {
        "scope": "user",
        "installPath": "/Users/somebody/.claude/plugins/cache/exa/$exa_version",
        "version": "$exa_version",
        "lastUpdated": "2026-08-01T21:05:42.338Z",
        "gitCommitSha": "bd2ccdd52ca7a35fbc2207ad266bb2a961c0e793"
      }
    ],
    "document-skills@anthropic-agent-skills": [
      {
        "scope": "user",
        "installPath": "/Users/somebody/.claude/plugins/cache/document-skills/b29e7cf65e5c",
        "version": "unknown",
        "gitCommitSha": "b29e7cf65e5cb78a5ac33d582270551bc74a14eb"
      }
    ],
    "github@claude-plugins-official": [
      {
        "scope": "user",
        "installPath": "/Users/somebody/.claude/plugins/cache/github/unknown",
        "version": "unknown",
        "lastUpdated": "2026-08-04T02:55:40.240Z"
      }
    ]
  }
}
EOF
}

RUN_OUTPUT=""
RUN_RC=0
run_helper() {
  : >"$RELAY_LOG"
  RUN_OUTPUT="$(REPORT_PLUGIN_UPDATES_STATE_FILE="$PLUGIN_STATE" \
    REPORT_PLUGIN_UPDATES_STATE_DIR="$STATE_DIR" \
    REPORT_PLUGIN_UPDATES_RELAY="$HOME/.local/bin/relay.sh" \
    RELAY_STUB_OUTCOME="${RELAY_STUB_OUTCOME:-}" \
    bash "$HELPER" "$@" 2>&1)"
  RUN_RC=$?
}

log_entries() { grep -F "url=$UNATTENDED_LOG_HERMES_URL " "$RELAY_LOG" || true; }
alert_entries() { grep -F 'url=<default> ' "$RELAY_LOG" || true; }

# ── 1. FIRST RUN records a baseline and posts NOTHING. An absent snapshot is not
#      an empty one: comparing against nothing would announce every installed
#      plugin as newly added, on a machine where nothing happened. ────────────
rm -rf "$HOME/.local/state"
write_plugin_state "3.4.0"
run_helper --scheduled
[[ $RUN_RC -eq 0 ]] || fail "the first run exited $RUN_RC: $RUN_OUTPUT"
[[ -f $SNAPSHOT ]] || fail "the first run recorded no baseline snapshot: $RUN_OUTPUT"
refute '.' "$(cat "$RELAY_LOG")" "the first run posted something; a baseline has nothing to report"
grep -qF 'exa@claude-plugins-official	3.4.0' "$SNAPSHOT" ||
  fail "the baseline does not carry the version fingerprint: $(cat "$SNAPSHOT")"
# The fingerprint falls back to the commit when the marketplace publishes no
# version, so a plugin that only moves by commit is still visible.
grep -qF 'document-skills@anthropic-agent-skills	b29e7cf65e5cb78a5ac33d582270551bc74a14eb' "$SNAPSHOT" ||
  fail "the version-less entry did not fall back to its commit: $(cat "$SNAPSHOT")"
# And it says `unknown` when there is neither, rather than inventing one.
grep -qF 'github@claude-plugins-official	unknown' "$SNAPSHOT" ||
  fail "the entry with neither a version nor a commit is not recorded as unknown: $(cat "$SNAPSHOT")"
# NOTHING from the state file but ids and fingerprints reaches the snapshot: an
# installPath is an absolute home path and has no business on a chat channel.
refute '/Users/somebody' "$(cat "$SNAPSHOT")" "an install path leaked into the snapshot"

# ── 2. A QUIET WEEK still posts, and names ZERO changes. Suppressing it would
#      make a healthy week and a dead LaunchAgent produce identical silence,
#      which is the whole reason this channel exists. What it must never do is
#      report a change that did not happen. ───────────────────────────────────
run_helper --scheduled
[[ $RUN_RC -eq 0 ]] || fail "a no-change run exited $RUN_RC: $RUN_OUTPUT"
entries="$(log_entries)"
[[ -n $entries ]] || fail "a no-change scheduled run posted no record at all: $RUN_OUTPUT"
grep -qF -- '--remote-only' <<<"$entries" ||
  fail "the record was not posted with --remote-only (it would banner and buzz every week): $entries"
grep -qF -- '--agent report-plugin-updates' <<<"$entries" ||
  fail "the record does not name the job it is about: $entries"
grep -qF -- '--state completed' <<<"$entries" || fail "the record did not carry the completed class: $entries"
grep -qE 'run at [0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z' <<<"$entries" ||
  fail "the record carries no ISO 8601 UTC run timestamp: $entries"
grep -qF 'Claude Code plugins: 0 of 3 tracked entries changed' <<<"$entries" ||
  fail "a week in which nothing moved did not report zero changes over three tracked plugins: $entries"
this_host="$(hostname -s 2>/dev/null || printf '%s' "${HOSTNAME:-unknown-host}")"
grep -qF -- "--project $this_host" <<<"$entries" ||
  fail "the record does not name the host it is about (expected --project $this_host): $entries"
refute 'url=<default>' "$(cat "$RELAY_LOG")" "a quiet week sent an alert; the alert route is for things to act on"

# ── 3. ONE PLUGIN MOVES, and the entry names it with BOTH versions. A reporter
#      that stays silent on a real change is this helper's worst possible
#      failure, so the transition itself is asserted, not merely a non-empty
#      message. The week guard is cleared first: a second entry in the same ISO
#      week is refused by design, and that rule is exercised in step 6. ───────
rm -rf "$STATE_DIR/log-week-claims"
write_plugin_state "3.5.0"
run_helper --scheduled
[[ $RUN_RC -eq 0 ]] || fail "a run reporting a change exited $RUN_RC: $RUN_OUTPUT"
entries="$(log_entries)"
[[ -n $entries ]] || fail "a plugin moved version and NOTHING was posted: $RUN_OUTPUT"
grep -qF 'Claude Code plugins: 1 of 3 tracked entries changed' <<<"$entries" ||
  fail "the entry does not report exactly one of three tracked plugins as changed: $entries"
grep -qF 'exa@claude-plugins-official' <<<"$entries" ||
  fail "the entry does not name the plugin that moved: $entries"
grep -qE '3\.4\.0.*->.*3\.5\.0' <<<"$entries" ||
  fail "the entry does not carry the old -> new transition: $entries"
refute 'document-skills@anthropic-agent-skills.*->' "$entries" \
  "a plugin that did not move was reported as changed"
# The snapshot advanced, so the next quiet week is quiet.
grep -qF 'exa@claude-plugins-official	3.5.0' "$SNAPSHOT" ||
  fail "the snapshot was not advanced after a delivered entry: $(cat "$SNAPSHOT")"
[[ -f $MARKER ]] || fail "a delivered entry did not advance the success marker"

# ── 4. A MALFORMED state file FAILS LOUDLY. The one answer it must never give
#      is "nothing changed": `{}` and an empty plugins map both parse, and both
#      would otherwise render as a clean week on a machine whose plugin
#      inventory could not be read at all. ─────────────────────────────────────
snapshot_before="$(cat "$SNAPSHOT")"
for broken in '{ "version": 2, "plugins": {} }' '{ "version": 2 }' 'this is not json' '{"version":2,"plugins":{"a@b":"nope"}}'; do
  rm -rf "$STATE_DIR/log-week-claims"
  printf '%s\n' "$broken" >"$PLUGIN_STATE"
  run_helper --scheduled
  [[ $RUN_RC -ne 0 ]] ||
    fail "an unreadable plugin state file [$broken] exited 0: $RUN_OUTPUT"
  refute "url=$UNATTENDED_LOG_HERMES_URL" "$(cat "$RELAY_LOG")" \
    "an unreadable plugin state file [$broken] still posted a record; the only change list it could build is a false one"
  [[ -n "$(alert_entries)" ]] ||
    fail "an unreadable plugin state file [$broken] alerted nobody: $RUN_OUTPUT"
  # The alert has to say WHY. `jq -e` exits non-zero on a filter that produced
  # no output and prints nothing at all, so a helper leaning on that alone
  # raises an alert whose reason field is empty, and the operator is told only
  # that something is wrong with a file they now have to read themselves.
  # Measured by mutation 2026-08-03: deleting the empty-inventory error() left
  # every other assertion in this loop green.
  grep -qE 'jq said: [^[:space:]]' <<<"$(alert_entries)" ||
    fail "the alert for [$broken] carries no reason, so nothing says what is wrong with the file: $(alert_entries)"
  [[ "$(cat "$SNAPSHOT")" == "$snapshot_before" ]] ||
    fail "an unreadable plugin state file [$broken] moved the snapshot, so the real gap is now unreportable"
done

# ── 5. A REFUSED DELIVERY does not consume the change. The gateway answering
#      401 while the snapshot advanced would lose that week's transition for
#      good: nothing else on the machine remembers it. ────────────────────────
rm -rf "$STATE_DIR/log-week-claims"
write_plugin_state "4.0.0"
RELAY_STUB_OUTCOME='relay: post FAILED HTTP 401'
run_helper --scheduled
RELAY_STUB_OUTCOME=''
[[ "$(cat "$SNAPSHOT")" == "$snapshot_before" ]] ||
  fail "a refused delivery advanced the snapshot anyway, so the change it failed to report is now invisible: $(cat "$SNAPSHOT")"
[[ -n "$(alert_entries)" ]] ||
  fail "a refused record delivery did not raise the broken-channel alert on the priority route: $RUN_OUTPUT"
# The retry proves the point: with the gateway healthy again the SAME change is
# reported, from the same untouched snapshot.
rm -rf "$STATE_DIR/log-week-claims"
run_helper --scheduled
entries="$(log_entries)"
grep -qE '3\.5\.0.*->.*4\.0\.0' <<<"$entries" ||
  fail "the retry after a refused delivery did not report the change the refused run held: $entries"
grep -qF 'exa@claude-plugins-official	4.0.0' "$SNAPSHOT" ||
  fail "the successful retry did not advance the snapshot: $(cat "$SNAPSHOT")"

# ── 6. The WEEK GUARD admits one entry per ISO week, and a run it silences must
#      not move the snapshot either: the entry that would have carried the
#      change was never sent. ────────────────────────────────────────────────
snapshot_before="$(cat "$SNAPSHOT")"
write_plugin_state "4.1.0"
run_helper --scheduled
refute "url=$UNATTENDED_LOG_HERMES_URL" "$(cat "$RELAY_LOG")" \
  "a second run in the same ISO week posted a second record"
[[ "$(cat "$SNAPSHOT")" == "$snapshot_before" ]] ||
  fail "a run the week guard silenced still consumed the change by moving the snapshot"

# ── 7. A MANUAL run posts nothing and changes nothing, so a dead LaunchAgent
#      cannot be made to look alive by an operator running this by hand. ──────
rm -rf "$STATE_DIR/log-week-claims"
snapshot_before="$(cat "$SNAPSHOT")"
run_helper
[[ $RUN_RC -eq 0 ]] || fail "a manual run exited $RUN_RC: $RUN_OUTPUT"
refute '.' "$(cat "$RELAY_LOG")" "a manual run posted to a channel"
[[ "$(cat "$SNAPSHOT")" == "$snapshot_before" ]] ||
  fail "a manual run moved the snapshot, which would swallow the change the next scheduled run should report"
# It still SHOWS the comparison, which is what makes it useful by hand.
grep -qF 'Claude Code plugins:' <<<"$RUN_OUTPUT" ||
  fail "a manual run printed no comparison, so there is no way to check this helper by hand: $RUN_OUTPUT"

# ── 8. An unknown argument is an error, never a silent fallthrough: a typo'd
#      marker in the plist would otherwise run weekly and post nothing, which
#      looks exactly like a dead LaunchAgent. ────────────────────────────────
run_helper --schedule
[[ $RUN_RC -eq 2 ]] || fail "an unknown argument did not exit 2 (got $RUN_RC): $RUN_OUTPUT"
grep -qF 'unknown argument' <<<"$RUN_OUTPUT" ||
  fail "the unknown-argument error does not name the problem: $RUN_OUTPUT"

# ── 9. A DOUBLED inventory is REFUSED. jq reads its input as a SEQUENCE of JSON
#      documents, so a file holding the same object twice parses fine and the
#      key-walk emits rows from BOTH copies. Left unchecked the snapshot carries
#      two fingerprints for one plugin, and a machine where nothing moved reports
#      a version transition every single week. Concatenation is what a crashed or
#      racing writer leaves behind, and it is the shape most parsers reject. ────
rm -rf "$STATE_DIR/log-week-claims"
write_plugin_state "5.0.0"
snapshot_before="$(cat "$SNAPSHOT")"
sed 's/5\.0\.0/9.9.9/g' "$PLUGIN_STATE" >"$tmp/second-document.json"
cat "$tmp/second-document.json" >>"$PLUGIN_STATE"
run_helper --scheduled
[[ $RUN_RC -ne 0 ]] ||
  fail "a doubled inventory exited 0; two documents were read as one reading: $RUN_OUTPUT"
refute "url=$UNATTENDED_LOG_HERMES_URL" "$(cat "$RELAY_LOG")" \
  "a doubled inventory still posted a record, so a machine where nothing moved reports a transition"
[[ -n "$(alert_entries)" ]] ||
  fail "a doubled inventory alerted nobody: $RUN_OUTPUT"
[[ "$(cat "$SNAPSHOT")" == "$snapshot_before" ]] ||
  fail "a doubled inventory moved the snapshot: $(cat "$SNAPSHOT")"

# ── 10. A MALFORMED install record is refused, not filtered out. Records used to
#       be selected by scope BEFORE their shape was checked, so a record whose
#       key reads `scop` instead of `scope` simply vanished from the reading and
#       the entry announced the plugin as REMOVED. A typo in a file this helper
#       only reads must never be reported as something leaving the machine. ────
rm -rf "$STATE_DIR/log-week-claims"
write_plugin_state "5.0.0"
awk '/"scope": "user"/ && !done { sub(/"scope"/, "\"scop\""); done = 1 } { print }' \
  "$PLUGIN_STATE" >"$tmp/typo.json"
grep -qF '"scop"' "$tmp/typo.json" || fail "the fixture for the typo case did not actually mistype a scope key"
cp "$tmp/typo.json" "$PLUGIN_STATE"
run_helper --scheduled
[[ $RUN_RC -ne 0 ]] ||
  fail "an install record with a mistyped scope key exited 0: $RUN_OUTPUT"
refute 'exa@claude-plugins-official.*removed' "$(log_entries)" \
  "a mistyped scope key was reported as the plugin being removed"
refute "url=$UNATTENDED_LOG_HERMES_URL" "$(cat "$RELAY_LOG")" \
  "an install record this helper could not read still produced a record"
[[ -n "$(alert_entries)" ]] ||
  fail "an install record with a mistyped scope key alerted nobody: $RUN_OUTPUT"
[[ "$(cat "$SNAPSHOT")" == "$snapshot_before" ]] ||
  fail "an install record with a mistyped scope key moved the snapshot: $(cat "$SNAPSHOT")"

# ── 11. NO USER-SCOPE RECORDS is legitimately empty, not unreadable. Uninstalling
#       the last user-scope plugin while a project-scope one remains leaves a file
#       that parses and describes a real machine state, and the removal is the
#       single most worth-seeing line this record can carry. Refusing it raises
#       plugin-state-unreadable every week from then on and reports the removal
#       never. ─────────────────────────────────────────────────────────────────
rm -rf "$HOME/.local/state"
write_plugin_state "6.0.0"
run_helper --scheduled
[[ -f $SNAPSHOT ]] || fail "the baseline for the empty-user-scope case was not recorded: $RUN_OUTPUT"
rm -rf "$STATE_DIR/log-week-claims"
sed 's/"scope": "user"/"scope": "project"/g' "$PLUGIN_STATE" >"$tmp/project-only.json"
cp "$tmp/project-only.json" "$PLUGIN_STATE"
run_helper --scheduled
[[ $RUN_RC -eq 0 ]] ||
  fail "an inventory with no user-scope records exited $RUN_RC; an empty reading is not an unreadable one: $RUN_OUTPUT"
refute 'url=<default>' "$(cat "$RELAY_LOG")" \
  "an inventory with no user-scope records raised the unreadable alert instead of reporting the removals"
entries="$(log_entries)"
grep -qF 'Claude Code plugins: 3 of 3 tracked entries changed' <<<"$entries" ||
  fail "the removal of every user-scope plugin was not reported as three changes over three entries: $entries"
grep -qF '(removed)' <<<"$entries" ||
  fail "the entry does not mark the plugins as removed: $entries"

# ── 12. A SNAPSHOT PATH THAT IS NOT A REGULAR FILE is refused loudly. A directory
#       there (a stray mkdir, a restore that recreated the tree wrong) reads as
#       "no baseline yet" on every run, takes every copy INSIDE itself, and exits
#       0 having recorded nothing: a machine that looks healthy and permanently
#       reports nothing at all. ──────────────────────────────────────────────────
rm -rf "$HOME/.local/state"
mkdir -p "$SNAPSHOT"
write_plugin_state "7.0.0"
run_helper --scheduled
[[ $RUN_RC -ne 0 ]] ||
  fail "a snapshot path that is a directory exited 0, so nothing will ever be compared or reported: $RUN_OUTPUT"
[[ -n "$(alert_entries)" ]] ||
  fail "a snapshot path that is a directory alerted nobody: $RUN_OUTPUT"
refute "url=$UNATTENDED_LOG_HERMES_URL" "$(cat "$RELAY_LOG")" \
  "a run that could not use its snapshot path still posted a record"
shopt -s nullglob dotglob
snapshot_dir_contents=("$SNAPSHOT"/*)
shopt -u nullglob dotglob
[[ ${#snapshot_dir_contents[@]} -eq 0 ]] ||
  fail "the run wrote its reading INSIDE the directory sitting at the snapshot path: ${snapshot_dir_contents[*]}"
rmdir "$SNAPSHOT"

# ── 13. THE SNAPSHOT IS REPLACED, NEVER OVERWRITTEN IN PLACE. A plain copy onto
#       the live snapshot truncates it before it has the new content, so a run
#       interrupted mid-write (a full disk, a reboot, a killed launchd job)
#       leaves a short file behind and the NEXT run reads every plugin missing
#       from it as newly added: a fabricated change list, the one thing this
#       record must never produce. Rename is what makes the swap all-or-nothing,
#       and a rename gives the path a new inode while a copy keeps the old one,
#       so the inode is what this asserts. ────────────────────────────────────
# GNU form first, BSD fallback second. The BSD flag means "file system status"
# under GNU coreutils, so it SUCCEEDS with the wrong output instead of failing
# over, and the fallback would never run.
inode_of() { stat -c %i "$1" 2>/dev/null || stat -f %i "$1"; }
rm -rf "$HOME/.local/state"
write_plugin_state "8.0.0"
run_helper --scheduled
[[ -f $SNAPSHOT ]] || fail "the baseline for the atomic-replacement case was not recorded: $RUN_OUTPUT"
snapshot_inode_before="$(inode_of "$SNAPSHOT")"
rm -rf "$STATE_DIR/log-week-claims"
write_plugin_state "8.1.0"
run_helper --scheduled
[[ $RUN_RC -eq 0 ]] || fail "the run that should have replaced the snapshot exited $RUN_RC: $RUN_OUTPUT"
grep -qF 'exa@claude-plugins-official	8.1.0' "$SNAPSHOT" ||
  fail "the snapshot does not carry the new reading: $(cat "$SNAPSHOT")"
[[ "$(inode_of "$SNAPSHOT")" != "$snapshot_inode_before" ]] ||
  fail "the snapshot was written in place rather than renamed over; a write interrupted halfway leaves a truncated snapshot and the next run reports every plugin as newly added"

# ── 14. A BASELINE THAT CANNOT BE PERSISTED is alerted, not just logged. A run
#       that reads fine and cannot remember what it read finds no snapshot again
#       next week, records another baseline, and reports nothing for as long as
#       the state directory stays unwritable. The local log is the one place
#       nobody looks, so this goes on the route that buzzes. ─────────────────
rm -rf "$HOME/.local/state"
mkdir -p "$STATE_DIR"
chmod 555 "$STATE_DIR"
write_plugin_state "9.0.0"
run_helper --scheduled
chmod 755 "$STATE_DIR"
[[ $RUN_RC -ne 0 ]] ||
  fail "a run that could not persist its baseline exited 0: $RUN_OUTPUT"
[[ -n "$(alert_entries)" ]] ||
  fail "a run that could not persist its baseline alerted nobody, so it reports nothing every week in silence: $RUN_OUTPUT"
grep -qF "$SNAPSHOT" <<<"$(alert_entries)" ||
  fail "the alert does not name the path that could not be written: $(alert_entries)"

# ── 15. A RUN THAT CANNOT RECORD ITSELF does not exit 0. The success marker is
#       what the next entry measures its gap from, so a delivered entry followed
#       by an unwritable marker leaves the channel claiming a gap from a run that
#       never happened. The library warns and returns 0 by design (a job must not
#       die over its own bookkeeping), so the helper has to check the marker
#       itself rather than lean on errexit to notice. ─────────────────────────
rm -rf "$HOME/.local/state"
write_plugin_state "10.0.0"
run_helper --scheduled
[[ -f $SNAPSHOT ]] || fail "the baseline for the marker case was not recorded: $RUN_OUTPUT"
rm -rf "$STATE_DIR/log-week-claims" "$MARKER"
mkdir -p "$MARKER"
write_plugin_state "10.1.0"
run_helper --scheduled
rmdir "$MARKER"
[[ $RUN_RC -ne 0 ]] ||
  fail "a run that could not record its own success exited 0, so the next entry reports a gap from a run that did not happen: $RUN_OUTPUT"
grep -qF "$MARKER" <<<"$RUN_OUTPUT" ||
  fail "the failure does not name the marker it could not write: $RUN_OUTPUT"

# The repo requires errexit everywhere, and this helper spent its first release
# without it. Pinned in source because the failures errexit catches are the ones
# nobody has thought of yet, which is exactly what no behavioural test can reach.
grep -qF 'set -euo pipefail' "$HELPER" ||
  fail "the helper does not run under set -euo pipefail"

# ── 16. THE BASELINE IS ESTABLISHED AT DEPLOY TIME, not at the first scheduled
#       run. The apply that deploys this record is also the apply that turns on
#       marketplace auto-updates, so a machine set up on Tuesday whose plugin
#       moves on Wednesday would have had Monday record the NEW version as its
#       baseline and report nothing: the first transition, lost for good and
#       invisible by construction. --seed-baseline is what the apply-time loader
#       calls to close that window. ──────────────────────────────────────────
rm -rf "$HOME/.local/state"
write_plugin_state "11.0.0"
run_helper --seed-baseline
[[ $RUN_RC -eq 0 ]] || fail "--seed-baseline exited $RUN_RC: $RUN_OUTPUT"
[[ -f $SNAPSHOT ]] || fail "--seed-baseline recorded no baseline: $RUN_OUTPUT"
grep -qF 'exa@claude-plugins-official	11.0.0' "$SNAPSHOT" ||
  fail "the seeded baseline does not hold the reading taken at deploy time: $(cat "$SNAPSHOT")"
refute '.' "$(cat "$RELAY_LOG")" "the deploy-time seed posted to a channel; it has nothing to report yet"

# Seeding again leaves the baseline alone. Applies are routine and a plugin may
# well have moved since the last one, so a seed that overwrote would swallow
# exactly the transition it exists to capture.
write_plugin_state "11.5.0"
run_helper --seed-baseline
[[ $RUN_RC -eq 0 ]] || fail "a second --seed-baseline exited $RUN_RC: $RUN_OUTPUT"
grep -qF 'exa@claude-plugins-official	11.0.0' "$SNAPSHOT" ||
  fail "a second seed overwrote the baseline, swallowing the change since the first: $(cat "$SNAPSHOT")"

# And the first scheduled run reports what moved between the deploy and it.
rm -rf "$STATE_DIR/log-week-claims"
run_helper --scheduled
entries="$(log_entries)"
grep -qE '11\.0\.0.*->.*11\.5\.0' <<<"$entries" ||
  fail "the transition between the deploy-time seed and the first scheduled run was not reported: $entries"

# The two modes are different jobs, so asking for both is an error rather than a
# quiet win for whichever the code happens to check first.
run_helper --scheduled --seed-baseline
[[ $RUN_RC -eq 2 ]] || fail "--scheduled together with --seed-baseline did not exit 2 (got $RUN_RC): $RUN_OUTPUT"

# ── 17. THE DEPLOY-TIME SEED DOES NOT PAGE. A machine whose plugin inventory
#       does not exist yet has no first transition to lose and nobody at a
#       keyboard to act on an alert, and every fresh machine reaches this line
#       during its first apply. The scheduled run stays the loud path. ────────
rm -rf "$HOME/.local/state"
rm -f "$PLUGIN_STATE"
run_helper --seed-baseline
[[ $RUN_RC -ne 0 ]] || fail "a seed with no inventory to read exited 0: $RUN_OUTPUT"
refute '.' "$(cat "$RELAY_LOG")" \
  "the deploy-time seed alerted about an inventory Claude Code has not written yet; every fresh machine would page during its first apply"
[[ ! -f $SNAPSHOT ]] || fail "a seed that read nothing still recorded a baseline"

# The asymmetry is the point: the SCHEDULED run on the same unreadable file does
# alert, because by then it is a weekly record that cannot report.
run_helper --scheduled
[[ -n "$(alert_entries)" ]] ||
  fail "the scheduled run did not alert on an inventory it could not read: $RUN_OUTPUT"

echo "report-plugin-updates-record: OK"
