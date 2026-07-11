#!/usr/bin/env bash
# update-skills-required-prereq.sh, a missing REQUIRED prerequisite is a REQUIRED
# FAILURE, not a silent success (Wave 3a item 4). The audit found: with a
# non-empty lock table but its prerequisite binary absent, the phase returned
# success, so the weekly stamp was written and the first-install retry marker was
# removed while the skills stayed absent. For each required phase whose lock
# table is non-empty and whose prerequisite command is missing, the run must
# record a required failure, WITHHOLD the weekly success stamp, and let
# --install-only exit non-zero (which preserves the first-install retry marker).
#
#   clawhub    -> clawhubTracked (install AND update passes)
#   hermes     -> hermesRegistry (weekly registry-update phase)
#   routing    -> superpowersRouting (assert-hermes-superpowers-routing.sh)
#
# The run is a FULL run under UPDATE_SKILLS_FORCE=1 (idle-gate bypassed) with a
# HERMETIC PATH: a sandbox bin holding only the tools the script needs, so the
# real clawhub/hermes (both installed on this host) are absent unless the case
# explicitly stubs them.
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
STORE="$HOME/.agents/skills"
mkdir -p "$STORE"
STAMP="$HOME/.local/state/update-skills/last-success"

# Hermetic sandbox bin: symlink jq (only in /opt/homebrew/bin, alongside the real
# clawhub) into it, and stub npx/date/alerter. The real clawhub/hermes live in
# dirs we deliberately keep OFF this PATH, so command -v fails for them unless a
# case adds a stub here.
sbin="$tmp/sbin"
mkdir -p "$sbin"
ln -s "$(command -v jq)" "$sbin/jq"
cat >"$sbin/npx" <<'EOF'
#!/usr/bin/env bash
echo stub
EOF
cat >"$sbin/date" <<'EOF'
#!/usr/bin/env bash
case "$1" in
  +%H) printf '%s\n' "${FAKE_HOUR:-04}" ;;
  +%u) printf '%s\n' "${FAKE_DOW:-1}" ;;
  +%G-%V) printf '%s\n' "${FAKE_WEEK:-2026-28}" ;;
  *) exec /bin/date "$@" ;;
esac
EOF
cat >"$sbin/alerter" <<'EOF'
#!/usr/bin/env bash
:
EOF
chmod +x "$sbin"/npx "$sbin"/date "$sbin"/alerter
HPATH="$sbin:/usr/bin:/bin"

write_lock() { # $1 = claw json, $2 = registry json, $3 = routing json
  cat >"$HOME/.agents/custom-skill-lock.json" <<EOF
{
  "version": 2,
  "tiers": {},
  "hermesProfiles": {},
  "hermesRegistry": $2,
  "npxTracked": {},
  "clawhubTracked": $1,
  "superpowersRouting": $3,
  "forks": {}
}
EOF
}

CLAW_ONE='{"foo": {"slug": "@o/foo", "registry": "https://clawhub.ai"}}'
REG_ONE='{"bar": {"profiles": ["default"], "source": "clawhub", "identifier": "c/bar", "lockKey": "bar"}}'
ROUTING_ONE='{"map": {"a": "b"}}'
EMPTY='{}'

run_full() {
  rm -rf "$HOME/.local/state"
  OUT="$(PATH="$HPATH" UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" 2>&1)" ||
    fail "full run exited non-zero (a required failure must not abort the run): $OUT"
}

# ── A) clawhubTracked non-empty, clawhub ABSENT → required failure, no stamp ──
write_lock "$CLAW_ONE" "$EMPTY" "$EMPTY"
run_full
grep -qi 'REQUIRED-FAILURE.*clawhub' <<<"$OUT" ||
  fail "a missing clawhub with a non-empty clawhubTracked did not record a required failure: $OUT"
grep -qi 'WITHHOLDING' <<<"$OUT" || fail "clawhub-absent run did not withhold the stamp: $OUT"
[[ ! -f $STAMP ]] || fail "the weekly stamp was written despite an absent clawhub with work to do"

# ── B) hermesRegistry non-empty, hermes ABSENT → required failure, no stamp ──
write_lock "$EMPTY" "$REG_ONE" "$EMPTY"
run_full
grep -qi 'REQUIRED-FAILURE.*hermes' <<<"$OUT" ||
  fail "a missing hermes with a non-empty hermesRegistry did not record a required failure: $OUT"
[[ ! -f $STAMP ]] || fail "the weekly stamp was written despite an absent hermes with work to do"

# ── C) superpowersRouting non-empty, routing script ABSENT → required failure ─
write_lock "$EMPTY" "$EMPTY" "$ROUTING_ONE"
run_full
grep -qi 'REQUIRED-FAILURE.*routing' <<<"$OUT" ||
  fail "a missing routing script with a non-empty superpowersRouting did not record a required failure: $OUT"
[[ ! -f $STAMP ]] || fail "the weekly stamp was written despite an absent routing script with work to do"

# ── D) --install-only exits NON-ZERO when clawhub is absent (marker preserved) ─
write_lock "$CLAW_ONE" "$EMPTY" "$EMPTY"
if PATH="$HPATH" UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --install-only >/dev/null 2>&1; then
  fail "--install-only returned success with a required prerequisite (clawhub) absent; the first-install marker would be wrongly cleared"
fi

# ── E) control: non-empty tables but prerequisites PRESENT → no required failure,
#      stamp IS written. ───────────────────────────────────────────────────
mkdir -p "$STORE/foo" # clawhub skill already present, so the install pass skips it
printf -- '---\nname: foo\ndescription: fixture\n---\n' >"$STORE/foo/SKILL.md"
cat >"$sbin/clawhub" <<'EOF'
#!/usr/bin/env bash
echo "ok"
EOF
cat >"$sbin/hermes" <<'EOF'
#!/usr/bin/env bash
echo "ok"
EOF
mkdir -p "$HOME/.local/bin"
cat >"$HOME/.local/bin/assert-hermes-superpowers-routing.sh" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
chmod +x "$sbin/clawhub" "$sbin/hermes" "$HOME/.local/bin/assert-hermes-superpowers-routing.sh"
write_lock "$CLAW_ONE" "$REG_ONE" "$ROUTING_ONE"
run_full
grep -qi 'REQUIRED-FAILURE' <<<"$OUT" &&
  fail "a run with all prerequisites present recorded a spurious required failure: $OUT"
[[ -f $STAMP ]] || fail "a clean run with present prerequisites did not write the weekly stamp: $OUT"

echo "update-skills-required-prereq: OK"
