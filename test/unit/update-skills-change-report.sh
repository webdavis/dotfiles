#!/usr/bin/env bash
#
# update-skills.sh: what "changed" is allowed to mean in the weekly record.
#
# The obvious answer, "skill, old version, new version", does not exist for most
# of the store. Measured against the live ~/.agents/.skill-lock.json, every one
# of its entries carries exactly source, sourceType, sourceUrl, skillPath,
# skillFolderHash, installedAt, updatedAt: no version, no commit. The npx lane
# installs the latest commit from main with no pin, so there is no version number
# to report for those skills, and an entry implying one would be a record that
# claims a completeness it does not have.
#
# So the two lanes report differently, and the difference is the point:
#   - npx: the change unit is the skillFolderHash moving. Changed or unchanged,
#     plus an explicit statement that no version number is knowable.
#   - clawhub: its CLI writes .clawhub/origin.json carrying installedVersion,
#     which is the ONLY place a version number exists anywhere in the store, so
#     those skills get a real old -> new transition.
#
# Unit test: the SNAPSHOT reader in isolation via UPDATE_SKILLS_LIB_ONLY=1. The
# rendering it feeds is shared with homebrew-weekly-upgrade.sh and is covered by
# test/unit/unattended-log-lib.sh. No network, no generation machinery, no sleeps.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"

fail() {
  printf 'update-skills-change-report: FAIL -- %s\n' "$*" >&2
  exit 1
}

refute() {
  local pattern="$1" haystack="$2" message="$3"
  if grep -qE "$pattern" <<<"$haystack"; then
    printf '=== haystack ===\n%s\n' "$haystack" >&2
    fail "$message"
  fi
}

[[ -r $SCRIPT ]] || fail "not readable: $SCRIPT"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

HOME="$tmp/home"
export HOME
mkdir -p "$HOME/.agents/skills"

export UPDATE_SKILLS_LIB_ONLY=1
# shellcheck source=dot_local/bin/executable_update-skills.sh
source "$SCRIPT"

# ── The SNAPSHOT reads both lanes from the places that actually hold the data:
#    the npx CLI's generation lock, and each clawhub skill's own origin marker.
#    Reading the marker rather than the roster is what makes the clawhub lane
#    self-describing. ─────────────────────────────────────────────────────────
mkdir -p "$SKILLS_CURRENT"
cat >"$SKILLS_CURRENT/.skill-lock.json" <<'EOF'
{"version": 1, "skills": {
  "alpha": {"source": "o/r", "sourceType": "github", "skillFolderHash": "aaa1"},
  "beta":  {"source": "o/r", "sourceType": "github", "skillFolderHash": "bbb1"}
}}
EOF
mkdir -p "$STORE/gamma/.clawhub"
printf '{"slug":"gamma","installedVersion":"1.0.0"}\n' >"$STORE/gamma/.clawhub/origin.json"

# Rows are built and matched through printf '\t' rather than embedded literal
# tabs, so a whitespace-mangling edit cannot silently turn these into
# space-separated rows the reader would never match.
row() { printf '%s\t%s' "$1" "$2"; }

snapshot="$tmp/before"
__update_skills_change_snapshot npx >"$snapshot" || fail "the npx snapshot exited non-zero"
__update_skills_change_snapshot clawhub >>"$snapshot" || fail "the clawhub snapshot exited non-zero"
grep -qxF "$(row alpha aaa1)" "$snapshot" ||
  fail "the snapshot did not read the npx lane's folder hash: $(cat "$snapshot")"
grep -qxF "$(row gamma 1.0.0)" "$snapshot" ||
  fail "the snapshot did not read the clawhub lane's installed version: $(cat "$snapshot")"

# A store with no generation lock at all must still produce a snapshot (a fresh
# machine), not abort the run that is trying to report on itself.
mv "$SKILLS_CURRENT/.skill-lock.json" "$tmp/stashed-lock.json"
__update_skills_change_snapshot npx >"$tmp/nolock" || fail "the npx snapshot aborted with no generation lock"
[[ ! -s "$tmp/nolock" ]] || fail "an absent generation lock produced npx rows: $(cat "$tmp/nolock")"
__update_skills_change_snapshot clawhub >"$tmp/nolock" || fail "the clawhub snapshot aborted with no generation lock"
grep -qxF "$(row gamma 1.0.0)" "$tmp/nolock" ||
  fail "the clawhub lane vanished when the npx lock was absent: $(cat "$tmp/nolock")"
mv "$tmp/stashed-lock.json" "$SKILLS_CURRENT/.skill-lock.json"

# A PUBLISHER-CONTROLLED version cannot forge a row. installedVersion is written
# by whoever published the skill, and the snapshot is line-and-tab delimited, so
# a newline in it splits one skill into two rows: an entry the operator never
# installed appears in the record AND the denominator grows, so one real skill
# reads as "1 of 2 tracked entries changed".
printf '{"slug":"gamma","installedVersion":"1.0.0\\nforged\\t9.9.9"}\n' \
  >"$STORE/gamma/.clawhub/origin.json"
__update_skills_change_snapshot clawhub >"$tmp/forged" || fail "the clawhub snapshot exited non-zero"
[[ "$(grep -c . "$tmp/forged")" -eq 1 ]] ||
  fail "a newline in a publisher-chosen version forged an extra snapshot row: $(cat "$tmp/forged")"
[[ "$(awk -F'\t' '{print NF}' "$tmp/forged")" -eq 2 ]] ||
  fail "a tab in a publisher-chosen version forged an extra column: $(cat "$tmp/forged")"
printf '{"slug":"gamma","installedVersion":"1.0.0"}\n' >"$STORE/gamma/.clawhub/origin.json"

# An unknown lane is an ERROR, not an empty snapshot: a silently empty lane
# would render as "0 of 0 tracked entries changed", which reads as a clean week.
if __update_skills_change_snapshot bogus >"$tmp/bogus" 2>/dev/null; then
  fail "an unknown lane produced a snapshot instead of failing"
fi

# ── The RENDERING lives in unattended-log-lib.sh and is covered by
#    test/unit/unattended-log-lib.sh; this file's job is the two SOURCES the
#    snapshot reads, which are specific to this store's layout. ──────────────
printf 'update-skills-change-report: OK (the npx lane reads the folder hash out of the generation lock and the clawhub lane reads each installed version out of the skill own origin marker; a publisher-chosen version cannot forge a row or a column; an absent generation lock still yields a snapshot; an unknown lane is an error, not an empty file)\n'
