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
#   - One entry per class per ISO week (so two at most, and only in a week that
#     defers before it completes). 24 hourly Monday slots would otherwise post 24.
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

# restage_lock [extra-json-member] -- the roster the run reads. A function
# because section 9 deliberately corrupts the file and later sections need it
# back, and because section 12 needs the same roster plus one extra table.
restage_lock() {
  cat >"$HOME/.agents/custom-skill-lock.json" <<EOF
{
  "version": 2,
  "tiers": {"anchor": "core", "vaulted": "on-demand"},
  "hermesProfiles": {},
  "hermesRegistry": {},
  "npxTracked": {"anchor": {"repo": "fixture/pack"}},
  "clawhubTracked": {"vaulted": {"slug": "@fixture/vaulted", "registry": "https://clawhub.ai"}},
  "forks": {}${1:+,
  $1}
}
EOF
}
# A superpowersRouting table with no ~/.local/bin/assert-hermes-superpowers-routing.sh
# deployed: a REQUIRED phase that runs after the publish, so it fails the run
# without stopping it short of the record.
restage_lock_with_routing() {
  restage_lock '"superpowersRouting": {"writing-plans": "hermes-writing-plans"}'
}
restage_lock
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

# Declared before the stubs because two of them bake this path in (the lanes run
# under `env -i`, so a stub cannot read it from the environment).
ACT_CLAUDE="$HOME/act/claude"

stub_dir="$tmp/stubs"
mkdir -p "$stub_dir"
# ps stub. Normally it answers FAKE_PS. When the mid-run arming marker exists it
# switches to reporting an agent process as soon as the npx stub has run, which
# is how section 14 makes a harness "turn active during the build": the top-level
# activity check sees a quiet machine and proceeds, and the pre-exchange check
# sees an active one. Both paths are the real script's; only the world it
# observes changes underneath it.
cat >"$stub_dir/ps" <<EOF
#!/usr/bin/env bash
if [[ -e "$tmp/arm-mid-run-defer" && -e "$tmp/agent-arrived" ]]; then
  printf '%s\n' '/opt/homebrew/bin/claude --remote-control'
  exit 0
fi
printf '%s\n' "\${FAKE_PS:-}"
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
# npx stub. It also carries section 14's trigger: the build lanes are the one
# place that runs between the top-level activity check and the pre-exchange one,
# so this is where a harness can plausibly turn active mid-run. The paths are
# baked in rather than read from the environment because the lanes run under
# `env -i`.
cat >"$stub_dir/npx" <<EOF
#!/usr/bin/env bash
echo "stub npx"
if [[ -e "$tmp/arm-mid-run-defer" ]]; then
  mkdir -p "$ACT_CLAUDE"
  : >"$ACT_CLAUDE/live.jsonl"
  : >"$tmp/agent-arrived"
fi
exit 0
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
GUARD="$HOME/.local/state/update-skills/log-week-claims"

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
# The entry names the MACHINE. The channel aggregates both weekly jobs and the
# daemon-host role is expected to move to a second Mac, so an entry that does not
# say which machine it is about cannot be investigated.
this_host="$(hostname -s 2>/dev/null || printf '%s' "${HOSTNAME:-unknown-host}")"
grep -qF -- "--project $this_host" <<<"$entries" ||
  fail "the entry does not name the host it is about (expected --project $this_host): $entries"

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
# ...and what it cannot SEE. The weekly run also refreshes the cua-driver pack
# through the app's own updater and updates the hermes-registry-owned skills, and
# this record reads neither. An entry that lists two lanes while implying it
# covers the whole run claims a completeness it does not have, which is the same
# defect as implying a version number exists.
grep -qiF 'cua-driver' <<<"$entries" ||
  fail "the entry does not say the cua-driver pack is outside what this record can see: $entries"
grep -qiF 'hermes' <<<"$entries" ||
  fail "the entry does not say the hermes-owned skills are outside what this record can see: $entries"

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

# ── 10. LOCK CONTENTION is the second way a slot ends up attempting nothing, and
#       it reaches a DIFFERENT record call site than the harness-activity
#       deferral above (the lock is taken before the activity check, so nothing
#       else in this file can reach it). Deferred is the class that actually
#       fires on this machine, so every path that produces one is pinned. It is a
#       record and not an alert: nothing was attempted, so there is nothing to
#       act on. ────────────────────────────────────────────────────────────────
reset_state
export FAKE_WEEK="2026-38"
restage_lock
lockfile="$HOME/.agents/.update-skills.lock"
: >"$lockfile"
holder_held="$tmp/lock-held"
holder_release="$tmp/lock-release"
rm -f "$holder_held"
: >"$holder_release"
(
  exec 9>>"$lockfile"
  /usr/bin/lockf -s -t 0 9 2>/dev/null || exit 1
  : >"$holder_held"
  while [[ -e $holder_release ]]; do sleep 0.05; done
) &
holder_pid=$!
for ((i = 0; i < 100; i++)); do
  [[ -e $holder_held ]] && break
  sleep 0.05
done
if [[ ! -e $holder_held ]]; then
  rm -f "$holder_release"
  wait "$holder_pid" 2>/dev/null || true
  fail "could not stage a held serialize lock; the contention case did not run"
fi
harness_absent
: >"$RELAY_LOG"
contended_rc=0
FAKE_PS="$QUIET_WORLD" bash "$SCRIPT" --scheduled >/dev/null 2>&1 || contended_rc=$?
rm -f "$holder_release"
wait "$holder_pid" 2>/dev/null || true
[[ $contended_rc -eq 75 ]] || fail "lock contention exited $contended_rc, want 75"
entries="$(log_entries)"
grep -qF -- '--state deferred' <<<"$entries" ||
  fail "a slot that deferred on the serialize lock recorded nothing; a week spent entirely in contention would leave the channel empty: $entries"
grep -qiE 'lock' <<<"$entries" ||
  fail "the contention entry does not say the serialize lock is why nothing was attempted: $entries"
refute 'url=<default>' "$(cat "$RELAY_LOG")" \
  "lock contention alerted; nothing was attempted, so it is a record and not something to act on"

# ── 11. A roster that tracks ZERO skills is a refusal too, and a DIFFERENT call
#       site from the unparseable-roster refusal in section 9. Both refuse before
#       any mutation, so both would otherwise leave the week silent. ───────────
reset_state
export FAKE_WEEK="2026-39"
cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{
  "version": 2,
  "tiers": {},
  "hermesProfiles": {},
  "hermesRegistry": {},
  "npxTracked": {},
  "clawhubTracked": {},
  "forks": {}
}
EOF
harness_absent
run_updater "$QUIET_WORLD" --scheduled
entries="$(log_entries)"
grep -qF -- '--state deferred' <<<"$entries" ||
  fail "a run refused for tracking zero skills left the record channel empty: $entries | $RUN_OUTPUT"
grep -qiE 'zero|refus' <<<"$entries" ||
  fail "the zero-roster entry does not say why nothing was attempted: $entries"
# ...and it does not assert a delivery nothing observed. The alert path is
# fire-and-forget: it backgrounds its POST and discards the HTTP result, so
# "an alert also went to the priority channel" is a claim this run cannot make.
refute 'alert also went' "$entries" \
  "the record asserts an alert delivery that nothing observed"
grep -qiE 'attempted|not observed|not confirmed' <<<"$entries" ||
  fail "the record does not say the alert was only attempted: $entries"
grep -qF -- '--agent update-skills' <<<"$(alert_entries)" ||
  fail "a zero-roster refusal sent no alert on the existing route: $RUN_OUTPUT"

# ── 12. A run that REACHED THE END with required-phase failures still records,
#       and the entry states the COUNT and that the weekly stamp was withheld.
#       Both are numbers/claims that would otherwise be printed by a format
#       string nothing reads: a constant 0 and a hardcoded "stamp was written"
#       both render a plausible-looking clean week. The failure is injected by
#       declaring a superpowersRouting table with no routing script deployed,
#       which is a required phase that runs AFTER the publish, so the run still
#       reaches the record. ──────────────────────────────────────────────────
reset_state
export FAKE_WEEK="2026-40"
restage_lock_with_routing
harness_absent
run_updater "$QUIET_WORLD" --scheduled
entries="$(log_entries)"
grep -qF -- '--state completed' <<<"$entries" ||
  fail "a run that reached the end with a required-phase failure posted no entry: $RUN_OUTPUT"
grep -qE 'required-phase failures: [1-9]' <<<"$entries" ||
  fail "the entry does not state the required-phase failure count, so a failing week reads as a clean one: $entries"
grep -qiE 'stamp was WITHHELD' <<<"$entries" ||
  fail "the entry claims the weekly stamp was written when it was withheld: $entries"
# ...and the clean run in section 4 said the opposite, so neither wording is a
# constant.
reset_state
export FAKE_WEEK="2026-41"
restage_lock
run_updater "$QUIET_WORLD" --scheduled
entries="$(log_entries)"
grep -qF 'required-phase failures: 0' <<<"$entries" ||
  fail "a clean run did not report zero required-phase failures: $entries | $RUN_OUTPUT"
grep -qiF 'stamp was written' <<<"$entries" ||
  fail "a clean run did not report that the weekly stamp was written: $entries"

# ── 13. A slot that finds the week already finished still leaves a message when
#       the week has none. Normally this entry is a no-op, because the completed
#       run that finished the week also claimed the guard; it exists for the week
#       whose completed entry failed to write that guard, which is exactly the
#       state staged here (the guard is removed, the success stamp is left). A
#       call site that only ever fires on a rare state is the one that rots. ────
rm -rf "$GUARD"
run_updater "$QUIET_WORLD" --scheduled
entries="$(log_entries)"
grep -qF -- '--state deferred' <<<"$entries" ||
  fail "a slot that found the week already complete, with no entry recorded for it, posted nothing: $entries | $RUN_OUTPUT"
grep -qiE 'already completed' <<<"$entries" ||
  fail "the no-op entry does not say the week was already finished: $entries"

# ── 14. A harness that turns active DURING the build defers the generation
#       exchange, and that is its own record call site: the run got past the
#       top-level activity check, so none of the deferral cases above reach it.
#       It is also the deferral that matters most to read, because the live
#       generation is left untouched after a full candidate build. The build
#       lanes are the seam: the npx stub plants the activity the pre-exchange
#       check then sees. ────────────────────────────────────────────────────
reset_state
export FAKE_WEEK="2026-42"
restage_lock
harness_absent
rm -f "$tmp/agent-arrived"
: >"$tmp/arm-mid-run-defer"
run_updater "$QUIET_WORLD" --scheduled
rm -f "$tmp/arm-mid-run-defer" "$tmp/agent-arrived"
grep -qiE 'exchange' <<<"$RUN_OUTPUT" ||
  fail "the run never reached the generation exchange, so this case did not test what it claims: $RUN_OUTPUT"
entries="$(log_entries)"
grep -qF -- '--state deferred' <<<"$entries" ||
  fail "a run whose generation exchange deferred recorded nothing: $entries | $RUN_OUTPUT"
grep -qiE 'nothing was published' <<<"$entries" ||
  fail "the exchange-deferral entry does not say the live generation is unchanged: $entries"

# ── 15. A CORRUPT successful-run marker must not take the run down with it. The
#       gap is read at start-up, before the lock, the alert paths and every
#       record call site, and this script runs under set -e: an epoch of
#       `0837000000` (a truncated or half-written marker; two of the ten digits
#       do it) is read by bash arithmetic as octal and aborts. The failure mode
#       is the worst available -- the job stops entirely and says nothing, from a
#       line whose only job is bookkeeping. ─────────────────────────────────────
reset_state
export FAKE_WEEK="2026-43"
restage_lock
harness_absent
mkdir -p "$(dirname "$MARKER")"
printf '0837000000 2026-07-10T12:00:00Z\n' >"$MARKER"
run_updater "$QUIET_WORLD" --scheduled
entries="$(log_entries)"
grep -qF -- '--state completed' <<<"$entries" ||
  fail "a leading-zero epoch in the successful-run marker ended the run before it recorded: $RUN_OUTPUT"
refute 'value too great for base' "$RUN_OUTPUT" \
  "the run leaked a bash arithmetic error reading its own successful-run marker"

# ── 16. An unwritable state dir must not end the run AFTER the publish and
#       BEFORE the record. The success stamp was written by an unguarded
#       redirection, and this script runs under set -e, so a state dir that could
#       not be written stopped the run right there: after the new generation was
#       already live, before the record, before the alert, and before anything
#       said so. The publish is the one thing that had already happened. ───────
reset_state
export FAKE_WEEK="2026-44"
restage_lock
harness_absent
mkdir -p "$HOME/.local/state"
chmod 500 "$HOME/.local/state"
run_updater "$QUIET_WORLD" --scheduled
chmod 700 "$HOME/.local/state"
entries="$(log_entries)"
grep -qF -- '--state completed' <<<"$entries" ||
  fail "an unwritable state dir ended the run before it could record anything: $RUN_OUTPUT"
grep -qiE 'stamp[^.]*(could not|WITHHELD|not written)' <<<"$entries" ||
  fail "the entry claims the weekly stamp was written when it could not be: $entries"
grep -qF -- '--agent update-skills' <<<"$(alert_entries)" ||
  fail "a run that could not mark the week done alerted nobody: $RUN_OUTPUT"

printf 'update-skills-weekly-record: OK (every deferral and refusal path records: activity, lock contention, an unparseable roster, a zero roster, an already-finished week and a mid-run exchange deferral; a completed run reports both lanes, what it cannot see, the required-phase count and what became of the weekly stamp; 24 slots post one entry; manual and dry runs post none; failures still alert the existing route; a corrupt marker and an unwritable state dir do not end the run before it records)\n'
