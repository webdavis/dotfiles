#!/usr/bin/env bash
#
# update-skills.sh: the weekly RECORD posted to the #unattended-upgrades channel.
#
# The job upgrades skills unattended every Monday. When something later
# misbehaves there is nothing to investigate against, because a clean week and a
# dead LaunchAgent produce identical silence. These entries are that record.
#
# What this pins, and the failure each one guards:
#
#   - A run that changed NOTHING still posts. Suppressing the empty entry throws
#     away the main reason the channel is worth having.
#   - A run that did NOTHING AT ALL posts too, as a distinct class. On this
#     machine that is the only class that will ever fire: update-skills has never
#     completed a run here, ~/.local/state/update-skills does not exist, and the
#     entire run log is five identical "deferring this run" lines. The record is
#     what makes that visible.
#   - Every entry carries its own gap, because launchd COALESCES missed calendar
#     intervals into one event on wake, so a healthy job can produce one entry
#     covering three weeks and an absent entry proves nothing.
#   - One entry per ISO week. 24 hourly Monday slots would otherwise post 24.
#   - A MANUAL run posts nothing. An operator running the script by hand on a
#     Wednesday must not make a dead LaunchAgent look alive; that inverts the
#     signal.
#   - Failures keep going to the EXISTING alert route, so they still land in the
#     priority channel. Act on one, record the other.
#
# End-to-end: the real script, a sandboxed HOME, a stub relay that records which
# ROUTE each call went to, and stub ps/date/npx as in update-skills-defer.sh.
set -euo pipefail

# git hooks (this can run under pre-commit) leak GIT_DIR/GIT_INDEX_FILE; unset so
# no child git command reaches the outer repo.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"

fail() {
  printf 'update-skills-weekly-record: FAIL -- %s\n' "$*" >&2
  exit 1
}

# Explicit refutation: `! grep` under set -e never fails a test.
refute() {
  local pattern="$1" haystack="$2" message="$3"
  if grep -qE "$pattern" <<<"$haystack"; then
    printf '=== haystack ===\n%s\n' "$haystack" >&2
    fail "$message"
  fi
}

[[ -x $SCRIPT ]] || fail "not executable: $SCRIPT"

tmp="$(mktemp -d)"
cleanup() {
  chmod -R u+rwx "$tmp" 2>/dev/null || true
  rm -rf "$tmp"
}
trap cleanup EXIT

HOME="$tmp/home"
export HOME
mkdir -p "$HOME/.agents/skills" "$HOME/.local/bin"

cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{
  "version": 2,
  "tiers": {"anchor": "core", "vaulted": "on-demand"},
  "hermesProfiles": {},
  "hermesRegistry": {},
  "npxTracked": {"anchor": {"repo": "fixture/pack"}},
  "clawhubTracked": {"vaulted": {"slug": "@fixture/vaulted", "registry": "https://clawhub.ai"}},
  "forks": {}
}
EOF
mkdir -p "$HOME/.agents/skills/anchor"
printf -- '---\nname: anchor\ndescription: fixture\n---\n' >"$HOME/.agents/skills/anchor/SKILL.md"
mkdir -p "$HOME/.agents/skills/vaulted/.clawhub"
printf -- '---\nname: vaulted\ndescription: fixture\n---\n' >"$HOME/.agents/skills/vaulted/SKILL.md"
printf '{"skills":{"anchor":{}}}\n' >"$HOME/.agents/.skill-lock.json"

# clawhub_version <version> -- restage the clawhub origin marker. This is the ONE
# place in the whole store where a version number exists; the npx lane's lock
# entries carry source/sourceType/sourceUrl/skillPath/skillFolderHash/installedAt/
# updatedAt and no version at all (measured against the live lock).
clawhub_version() {
  printf '{"version":1,"registry":"https://clawhub.ai","slug":"vaulted","ownerHandle":"fixture","installedVersion":"%s","installedAt":1783620597783,"fingerprint":"deadbeef"}\n' \
    "$1" >"$HOME/.agents/skills/vaulted/.clawhub/origin.json"
}
clawhub_version "1.0.0"

RELAY_LOG="$tmp/relay-calls.log"
export RELAY_LOG
: >"$RELAY_LOG"
# One LINE per call, so an entry's multi-line --detail body stays greppable as a
# unit: newlines inside an argument are flattened to spaces. Quoted heredoc, so
# the stub's own escapes reach the file intact and RELAY_LOG is read from the
# environment at run time.
cat >"$HOME/.local/bin/relay.sh" <<'STUB'
#!/usr/bin/env bash
printf 'CALL url=%s ARGV %s\n' "${RELAY_HERMES_URL:-<default>}" "$(printf '%s ' "$@" | tr '\n' ' ')" >>"$RELAY_LOG"
printf 'relay: posted HTTP 200\n'
exit 0
STUB
chmod +x "$HOME/.local/bin/relay.sh"

stub_dir="$tmp/stubs"
mkdir -p "$stub_dir"
cat >"$stub_dir/ps" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "${FAKE_PS:-}"
EOF
cat >"$stub_dir/date" <<'EOF'
#!/usr/bin/env bash
for arg in "$@"; do
  case "$arg" in
    +%H) printf '%s\n' "${FAKE_HOUR:-04}"; exit 0 ;;
    +%u) printf '%s\n' "${FAKE_DOW:-1}"; exit 0 ;;
    +%G-%V) printf '%s\n' "${FAKE_WEEK:-2026-31}"; exit 0 ;;
  esac
done
exec /bin/date "$@"
EOF
cat >"$stub_dir/npx" <<'EOF'
#!/usr/bin/env bash
echo "stub npx"
EOF
# clawhub stub (same shape as test/integration/update-skills-generation-lanes.sh):
# `install` materializes the skill with its origin marker, `update` is a no-op
# success. The clawhub UPDATER is not what this test is about; what matters is
# that the lane succeeds so the run reaches the tail and the record can read the
# installedVersion the marker carries.
cat >"$stub_dir/clawhub" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
wd=""; dir="skills"; mode=""; prev=""
for a in "$@"; do
  case "$prev" in --workdir) wd="$a" ;; --dir) dir="$a" ;; esac
  case "$a" in install) mode=install ;; update) mode=update ;; esac
  prev="$a"
done
args=("$@"); slug="${args[${#args[@]} - 1]}"
if [[ $mode == install ]]; then
  dest="$wd/$dir/$slug"; mkdir -p "$dest/.clawhub"
  printf -- '---\nname: %s\ndescription: fixture\n---\n' "$(basename "$slug")" >"$dest/SKILL.md"
  printf '{"slug":"%s","installedVersion":"1.0.0"}\n' "$(basename "$slug")" >"$dest/.clawhub/origin.json"
fi
EOF
cat >"$stub_dir/alerter" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
chmod +x "$stub_dir"/*
export PATH="$stub_dir:$PATH"
export FAKE_WEEK="2026-31"
export UNATTENDED_LOG_HERMES_URL="http://hermes.test/webhooks/unattended-upgrades"

ACT_CLAUDE="$HOME/act/claude"
export UPDATE_SKILLS_CLAUDE_ACTIVITY_DIR="$ACT_CLAUDE"
export UPDATE_SKILLS_CODEX_ACTIVITY_DIR="$HOME/act/codex"
export UPDATE_SKILLS_HERMES_ACTIVITY_DIR="$HOME/act/hermes"
export UPDATE_SKILLS_IDLE_THRESHOLD=900

AGENT_WORLD='/opt/homebrew/bin/claude --remote-control'
QUIET_WORLD='/usr/bin/python3 /usr/local/bin/some-tool.py --flag'

harness_active() {
  mkdir -p "$ACT_CLAUDE"
  : >"$ACT_CLAUDE/live.jsonl"
}
harness_absent() { rm -rf "$HOME/act"; }

RUN_OUTPUT=""
# run_updater <world> [args...] -- run the real script; never let a non-zero exit
# abort the harness (the script exits 75 on a deferral by contract).
run_updater() {
  local world="$1"
  shift
  : >"$RELAY_LOG"
  RUN_OUTPUT="$(FAKE_PS="$world" bash "$SCRIPT" "$@" 2>&1)" || true
}

# log_entries -- only the relay calls that went to the LOG route.
log_entries() { grep -F "url=$UNATTENDED_LOG_HERMES_URL " "$RELAY_LOG" || true; }
# alert_entries -- relay calls that used relay's DEFAULT route (the alert path).
alert_entries() { grep -F 'url=<default> ' "$RELAY_LOG" || true; }
log_entry_count() { log_entries | grep -c 'ARGV' || true; }

MARKER="$HOME/.local/state/update-skills/last-success-at"
GUARD="$HOME/.local/state/update-skills/last-log-week"

reset_state() {
  rm -rf "$HOME/.local/state"
  harness_absent
}

# ── 1. A SCHEDULED run that defers posts the DEFERRED class, and says the gap is
#      unknown because nothing has ever succeeded here. That is this machine's
#      real state, and it is the whole point of the class. ────────────────────
reset_state
harness_active
run_updater "$AGENT_WORLD" --scheduled
entries="$(log_entries)"
[[ -n $entries ]] || fail "a scheduled deferral posted NO record entry: $RUN_OUTPUT"
grep -qF -- '--remote-only' <<<"$entries" ||
  fail "the record was not posted with --remote-only (it would banner + buzz every week): $entries"
grep -qF -- '--state deferred' <<<"$entries" ||
  fail "a deferral did not post the 'deferred' class: $entries"
grep -qiE 'nothing was attempted' <<<"$entries" ||
  fail "the deferred entry does not say nothing was attempted: $entries"
grep -qiE 'NEVER RECORDED' <<<"$entries" ||
  fail "the entry does not state that no successful run has ever been recorded here: $entries"
grep -qE 'run at [0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z' <<<"$entries" ||
  fail "the entry carries no ISO 8601 UTC run timestamp: $entries"

# ── 2. The once-per-week guard. 24 hourly Monday slots must not become 24
#      messages. ────────────────────────────────────────────────────────────────
reset_state
total_entries=0
for _ in $(seq 1 24); do
  harness_active
  run_updater "$AGENT_WORLD" --scheduled
  total_entries=$((total_entries + $(log_entry_count)))
done
[[ $total_entries -eq 1 ]] ||
  fail "24 scheduled deferrals in one ISO week posted $total_entries entries, want exactly 1"

# ── 3. A COMPLETED run later in the SAME week still gets its entry. Leaving
#      "deferred, nothing attempted" as the newest message of a week the job
#      actually finished would invert the health signal the record carries. ────
harness_absent
run_updater "$QUIET_WORLD" --scheduled
entries="$(log_entries)"
grep -qF -- '--state completed' <<<"$entries" ||
  fail "a completed run in a week that already deferred posted no completed entry: $entries | $RUN_OUTPUT"

# ...and the completed entry is not repeated, nor buried by a later deferral.
run_updater "$QUIET_WORLD" --scheduled
[[ "$(log_entry_count)" -eq 0 ]] || fail "a second completed entry was posted in the same week"
harness_active
run_updater "$AGENT_WORLD" --scheduled
[[ "$(log_entry_count)" -eq 0 ]] ||
  fail "a deferral after a completed entry posted, burying the truer message"

# ── 4. A run that changed NOTHING still posts, and says so. This is the
#      deliberate opposite of the usual "do not be noisy" instinct: an entry that
#      changed nothing is precisely where the gap figure is the only information
#      the entry carries. ───────────────────────────────────────────────────────
reset_state
harness_absent
export FAKE_WEEK="2026-32"
run_updater "$QUIET_WORLD" --scheduled
entries="$(log_entries)"
grep -qF -- '--state completed' <<<"$entries" || fail "a clean run posted no entry: $entries | $RUN_OUTPUT"
# Both the CHANGED count and the TOTAL are pinned, because each binds one of the
# two snapshot points. A missing BEFORE snapshot makes every skill read as
# "(added)" ("1 of 1"); a missing AFTER snapshot makes the total zero and every
# skill read as "(removed)" ("1 of 0"). Only both snapshots taken at the right
# moments produce "0 of 1".
grep -qF 'npx-tracked skills: 0 of 1 tracked entries changed' <<<"$entries" ||
  fail "a run that changed nothing did not report 0 of 1 npx changes: $entries"
grep -qF 'clawhub-tracked skills: 0 of 1 tracked entries changed' <<<"$entries" ||
  fail "a run that changed nothing did not report 0 of 1 clawhub changes: $entries"
# And it must say what it CANNOT know. The npx lane installs latest from main
# unpinned and its lock entries carry no version field, so a version number is
# not knowable for those skills; an entry implying otherwise is worse than none.
grep -qiE 'no version number is knowable' <<<"$entries" ||
  fail "the entry does not state that no version number is knowable for the npx lane: $entries"

# ── 5. The success marker is now written, so the NEXT week's entry reports a
#      real elapsed figure rather than NEVER. ─────────────────────────────────
[[ -s $MARKER ]] || fail "a completed run did not record its successful-run timestamp at $MARKER"
export FAKE_WEEK="2026-33"
harness_active
run_updater "$AGENT_WORLD" --scheduled
entries="$(log_entries)"
grep -qE 'last successful run: [0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z \([^)]+ ago\)' <<<"$entries" ||
  fail "the entry does not carry the previous successful run and the elapsed gap: $entries"
refute 'NEVER RECORDED' "$entries" "the entry still claims no run was ever recorded after one succeeded"

# ── 6. A COMPLETED run reports the gap to the PREVIOUS success, not to itself.
#      This is an ordering constraint, not a formatting one: a completed run
#      rewrites the marker, so a gap read at post time instead of at start-up
#      would make every successful entry claim "0s ago" and the channel would
#      lose the only figure that survives launchd coalescing. The marker is
#      back-dated 8 days so the two orderings give visibly different answers.
recorded_iso="$(awk '{print $2}' "$MARKER")"
printf '%s %s\n' "$(($(/bin/date +%s) - 691200))" "$recorded_iso" >"$MARKER"
export FAKE_WEEK="2026-34"
harness_absent
run_updater "$QUIET_WORLD" --scheduled
entries="$(log_entries)"
grep -qF -- '--state completed' <<<"$entries" || fail "the back-dated week posted no completed entry: $RUN_OUTPUT"
grep -qE 'last successful run: [^ ]+ \(8d 0h ago\)' <<<"$entries" ||
  fail "a completed run reported its own timestamp as the previous success instead of the real 8-day gap: $entries"
# ...and it then advanced the marker, so the gap does not stay frozen at 8 days.
[[ "$(awk '{print $1}' "$MARKER")" -gt "$(($(/bin/date +%s) - 691200))" ]] ||
  fail "a completed run did not advance the successful-run marker"

# ── 7. A MANUAL run posts NOTHING. Otherwise a hand-run on a Wednesday makes a
#      dead LaunchAgent look alive. ───────────────────────────────────────────
reset_state
export FAKE_WEEK="2026-36"
harness_active
run_updater "$AGENT_WORLD"
[[ "$(log_entry_count)" -eq 0 ]] ||
  fail "a MANUAL run posted a weekly record: $(log_entries)"
harness_absent
run_updater "$QUIET_WORLD"
[[ "$(log_entry_count)" -eq 0 ]] ||
  fail "a MANUAL completed run posted a weekly record: $(log_entries)"

# ── 8. --dry-run posts NOTHING, deliberately: a preview must have no side
#      effects, and a relay push reaches a channel. ───────────────────────────
reset_state
run_updater "$QUIET_WORLD" --dry-run --scheduled
[[ "$(log_entry_count)" -eq 0 ]] || fail "--dry-run posted a weekly record: $(log_entries)"
[[ -e $GUARD ]] && fail "--dry-run wrote the weekly guard (a preview must not consume the week)"

# ── 9. Failures still go to the EXISTING alert route so they land in the
#      priority channel. The record channel never becomes the alert channel. ──
reset_state
export FAKE_WEEK="2026-37"
printf 'not json at all\n' >"$HOME/.agents/custom-skill-lock.json"
run_updater "$QUIET_WORLD" --scheduled
alerts="$(alert_entries)"
grep -qF -- '--agent update-skills' <<<"$alerts" ||
  fail "a refused run sent no alert on the existing route: $alerts | $RUN_OUTPUT"
refute '[-][-]remote-only' "$alerts" "the alert path started using the log route's flag"
# ...and the refusal is ALSO recorded, as the deferred/nothing-attempted class,
# so a week that only ever refused is not an empty channel.
entries="$(log_entries)"
grep -qF -- '--state deferred' <<<"$entries" ||
  fail "a refused run left the record channel empty for the week: $entries"
grep -qiE 'refus|roster' <<<"$entries" ||
  fail "the refusal entry does not say why nothing was attempted: $entries"

printf 'update-skills-weekly-record: OK\n'
