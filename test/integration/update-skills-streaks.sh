#!/usr/bin/env bash
# update-skills-streaks.sh: per-skill failure streaks (Wave 3a fix4 brief step
# 6): {last_failed_week, consecutive_failed_weeks} per skill, incremented at
# most once per ISO WEEK (not per hourly slot), reset on verified success,
# escalated alert wording at 2 consecutive weeks. Convergence never stops: the
# failing run still exits 0 and the next slot retries.
#
# The real script runs unmodified in a sandbox: an npx stub whose add fails
# while a marker file exists (file-based: the lanes run under env -i, which
# strips test env vars), a pinned date stub for the ISO week, and an alerter
# stub capturing the wording.
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

HOME="$tmp/home"
export HOME
mkdir -p "$HOME/.agents/skills"
STREAKS="$HOME/.local/state/update-skills/skill-failure-streaks.json"
STAMP="$HOME/.local/state/update-skills/last-success"

cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{
  "version": 2,
  "tiers": {"flaky": "core"},
  "hermesProfiles": {"flaky": []},
  "hermesRegistry": {},
  "npxTracked": {"flaky": {"repo": "fixture/flaky"}},
  "clawhubTracked": {},
  "forks": {}
}
EOF

stub="$tmp/stub"
mkdir -p "$stub"
ALERTER_LOG="$tmp/alerter.log"
cat >"$stub/npx" <<EOF
#!/usr/bin/env bash
mode=""
skill=""
prev=""
for a in "\$@"; do
  case "\$a" in add) mode=add ;; update) mode=update ;; esac
  [[ \$prev == "--skill" ]] && skill="\$a"
  prev="\$a"
done
if [[ \$mode == "add" ]]; then
  if [[ -e "$tmp/npx-add-fail" ]]; then echo "npx add boom" >&2; exit 1; fi
  mkdir -p "\$HOME/.agents/skills/\$skill"
  printf -- '---\nname: %s\ndescription: fixture\n---\n' "\$skill" >"\$HOME/.agents/skills/\$skill/SKILL.md"
  # like the real CLI (skills 1.5.16): record the add in the XDG global lock
  cli_lock="\${XDG_STATE_HOME:-\$HOME/.local/state}/skills/.skill-lock.json"
  mkdir -p "\$(dirname "\$cli_lock")"
  [[ -f \$cli_lock ]] || printf '{"version":3,"skills":{}}\n' >"\$cli_lock"
  jq --arg s "\$skill" '.skills[\$s] = {source: "github:fixture"}' \
    "\$cli_lock" >"\$cli_lock.tmp" && mv "\$cli_lock.tmp" "\$cli_lock"
fi
echo stub
EOF
cat >"$stub/alerter" <<EOF
#!/usr/bin/env bash
printf 'alerter %s\n' "\$*" >>"$ALERTER_LOG"
EOF
cat >"$stub/date" <<'EOF'
#!/usr/bin/env bash
case "$1" in
  +%H) printf '%s\n' "${FAKE_HOUR:-04}" ;;
  +%u) printf '%s\n' "${FAKE_DOW:-1}" ;;
  +%G-%V) printf '%s\n' "${FAKE_WEEK:-2026-28}" ;;
  *) exec /bin/date "$@" ;;
esac
EOF
chmod +x "$stub"/*
export PATH="$stub:$PATH"

# run <fail?> <week>
run() {
  if [[ -n $1 ]]; then touch "$tmp/npx-add-fail"; else rm -f "$tmp/npx-add-fail"; fi
  : >"$ALERTER_LOG"
  rm -f "$STAMP" # FORCE bypasses the stamp anyway; keep worlds independent
  OUT="$(FAKE_WEEK="$2" UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" 2>&1)" ||
    fail "run exited non-zero (streaks must never stop convergence): $OUT"
}

streak_of() { jq -r --arg n "$1" '.[$n].consecutive_failed_weeks // 0' "$STREAKS" 2>/dev/null || echo 0; }
week_of() { jq -r --arg n "$1" '.[$n].last_failed_week // ""' "$STREAKS" 2>/dev/null || echo ""; }

# 1) First failing week: streak = 1, no escalation.
run 1 "2026-28"
[[ "$(streak_of flaky)" == "1" ]] || fail "first failing week did not set the streak to 1: $(cat "$STREAKS" 2>/dev/null)"
[[ "$(week_of flaky)" == "2026-28" ]] || fail "last_failed_week not recorded: $(cat "$STREAKS" 2>/dev/null)"
if grep -qi 'multiple weeks' "$ALERTER_LOG"; then
  fail "a single failing week fired the escalated alert wording: $(cat "$ALERTER_LOG")"
fi

# 2) A second failing SLOT in the SAME week: streak stays 1 (at most one
#    increment per week, not per hourly slot).
run 1 "2026-28"
[[ "$(streak_of flaky)" == "1" ]] ||
  fail "a second slot in the same week double-incremented the streak: $(cat "$STREAKS")"

# 3) A failing run in the NEXT week: streak = 2, escalated alert wording fires.
run 1 "2026-29"
[[ "$(streak_of flaky)" == "2" ]] || fail "second failing week did not raise the streak to 2: $(cat "$STREAKS")"
grep -qi 'multiple weeks' "$ALERTER_LOG" ||
  fail "a 2-week streak did not fire the escalated alert wording: $(cat "$ALERTER_LOG" 2>/dev/null)"
grep -q 'flaky' "$ALERTER_LOG" || fail "the escalated alert does not name the failing skill: $(cat "$ALERTER_LOG")"
grep -qi 'STREAK' <<<"$OUT" || fail "the run log does not carry the streak line: $OUT"

# 4) A verified success resets the streaks; a later failure starts over at 1.
run "" "2026-30"
[[ "$(streak_of flaky)" == "0" ]] ||
  fail "a verified success did not reset the streak: $(cat "$STREAKS" 2>/dev/null)"
run 1 "2026-31"
[[ "$(streak_of flaky)" == "1" ]] ||
  fail "the post-reset failure did not restart the streak at 1: $(cat "$STREAKS")"

echo "update-skills-streaks: OK"
