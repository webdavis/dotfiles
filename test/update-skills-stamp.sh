#!/usr/bin/env bash
# update-skills-stamp.sh, the weekly success stamp must be written ONLY when a
# run had zero REQUIRED-phase failures. A required failure (here: an npx install
# that fails) leaves the stamp absent so a later Monday slot retries, and, only
# when no future slot remains this Monday, fires the loud exhaustion alert. A
# failure on any other day withholds the stamp but claims no exhaustion.
#
# The real script runs unmodified in a sandbox. FORCE bypasses the idle-gate (so
# the run proceeds), an npx stub is made to FAIL the install of a tracked skill,
# and date/alerter are stubbed so the slot-aware branch and the alert are
# observable.
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
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

# npx stub: FAILS `add` when FAKE_NPX_ADD_FAIL is set (a required install
# failure); otherwise materialises the store dir. `update` always succeeds.
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
  if [[ -n \${FAKE_NPX_ADD_FAIL:-} ]]; then echo "npx add boom" >&2; exit 1; fi
  mkdir -p "\$HOME/.agents/skills/\$skill"
  printf -- '---\nname: %s\ndescription: fixture\n---\n' "\$skill" >"\$HOME/.agents/skills/\$skill/SKILL.md"
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

# run <npx_add_fail> <hour> <dow> [sched], the 4th arg "--scheduled" marks a
# LaunchAgent run; otherwise the run is manual (never claims exhaustion).
run() {
  local -a run_args=()
  [[ ${4:-} == "--scheduled" ]] && run_args=(--scheduled)
  rm -rf "$HOME/.local/state"
  : >"$ALERTER_LOG"
  OUT="$(FAKE_NPX_ADD_FAIL="$1" FAKE_HOUR="$2" FAKE_DOW="$3" UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" "${run_args[@]}" 2>&1)" ||
    fail "run exited non-zero (a required failure must not abort the run): $OUT"
}

# 1) Required failure on the last SCHEDULED Monday slot (23:00) → no stamp, loud
#    exhaustion alert.
run 1 23 1 --scheduled
[[ ! -f $STAMP ]] || fail "the success stamp was written despite a required-phase failure"
grep -qi 'WITHHOLDING' <<<"$OUT" || fail "a required failure did not log a stamp-withhold: $OUT"
grep -qi 'EXHAUSTED' <<<"$OUT" || fail "a required failure on the last scheduled slot did not log exhaustion: $OUT"
[[ -s $ALERTER_LOG ]] || fail "a required failure on the last scheduled slot did not fire the alerter"

# 2) Required failure on a NON-Monday SCHEDULED catch-up → no stamp; a later day
#    means the Monday budget is spent, so exhaustion IS claimed.
run 1 16 3 --scheduled
[[ ! -f $STAMP ]] || fail "the success stamp was written despite a required-phase failure (catch-up)"
grep -qi 'WITHHOLDING' <<<"$OUT" || fail "a catch-up required failure did not log a stamp-withhold: $OUT"
[[ -s $ALERTER_LOG ]] || fail "a scheduled catch-up required failure did not claim exhaustion: $(cat "$ALERTER_LOG")"

# 2b) Required failure on a MANUAL last-Monday-slot run → no stamp, but NO
#     exhaustion alert (a manual run never claims scheduled-budget exhaustion).
run 1 16 1
[[ ! -f $STAMP ]] || fail "the success stamp was written despite a required-phase failure (manual)"
grep -qi 'WITHHOLDING' <<<"$OUT" || fail "a manual required failure did not log a stamp-withhold: $OUT"
[[ ! -s $ALERTER_LOG ]] || fail "a manual required failure claimed scheduled-budget exhaustion: $(cat "$ALERTER_LOG")"

# 3) Zero required failures → the stamp IS written (ISO week plus the custom-lock
#    and updater hashes, so a roster or updater change un-stamps the week).
run "" 16 1 --scheduled
[[ -f $STAMP ]] || fail "a clean run did not write the success stamp"
[[ "$(<"$STAMP")" == "2026-28 "* ]] || fail "the stamp does not begin with the pinned ISO week: $(<"$STAMP")"
stamp_fields="$(wc -w <"$STAMP" | tr -d ' ')"
[[ $stamp_fields -eq 3 ]] || fail "the stamp is not <week> <lock-hash> <updater-hash> (got $stamp_fields fields): $(<"$STAMP")"

echo "update-skills-stamp: OK"
