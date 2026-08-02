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
# Unit test: the reporting functions in isolation via UPDATE_SKILLS_LIB_ONLY=1,
# driven off fixture snapshot files. No network, no generation machinery, no
# sleeps.
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

NPX_CAVEAT='no version number is knowable'
CLAWHUB_CAVEAT='a version number is knowable here'

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
row() { printf '%s\t%s\t%s' "$1" "$2" "$3"; }

snapshot="$tmp/before"
__update_skills_change_snapshot >"$snapshot" || fail "the snapshot exited non-zero"
grep -qxF "$(row alpha npx aaa1)" "$snapshot" ||
  fail "the snapshot did not read the npx lane's folder hash: $(cat "$snapshot")"
grep -qxF "$(row gamma clawhub 1.0.0)" "$snapshot" ||
  fail "the snapshot did not read the clawhub lane's installed version: $(cat "$snapshot")"

# A store with no generation lock at all must still produce a snapshot (a fresh
# machine), not abort the run that is trying to report on itself.
mv "$SKILLS_CURRENT/.skill-lock.json" "$tmp/stashed-lock.json"
__update_skills_change_snapshot >"$tmp/nolock" || fail "the snapshot aborted with no generation lock"
grep -qxF "$(row gamma clawhub 1.0.0)" "$tmp/nolock" ||
  fail "the clawhub lane vanished when the npx lock was absent: $(cat "$tmp/nolock")"
mv "$tmp/stashed-lock.json" "$SKILLS_CURRENT/.skill-lock.json"

# ── Lane lines. Each fixture pair is written directly so the assertion is about
#    the REPORT, not about the generation machinery. ────────────────────────────
before="$tmp/b"
after="$tmp/a"

write_snapshot() { # <file> <name:lane:fingerprint>...
  local file="$1"
  shift
  : >"$file"
  local spec name lane fingerprint
  for spec in "$@"; do
    IFS=: read -r name lane fingerprint <<<"$spec"
    printf '%s\n' "$(row "$name" "$lane" "$fingerprint")" >>"$file"
  done
}

npx_line() { __update_skills_lane_line "$before" "$after" npx "$NPX_CAVEAT"; }
clawhub_line() { __update_skills_lane_line "$before" "$after" clawhub "$CLAWHUB_CAVEAT"; }

# NOTHING CHANGED. The count and the total must both be right: "0 of 0" would
# read as a clean week on an empty store, which is the "looks like success when
# nothing happened" shape.
write_snapshot "$before" "alpha:npx:aaa1" "beta:npx:bbb1"
write_snapshot "$after" "alpha:npx:aaa1" "beta:npx:bbb1"
line="$(npx_line)"
[[ $line == "npx lane: 0 of 2 tracked skills changed. $NPX_CAVEAT" ]] ||
  fail "unchanged npx lane rendered as: '$line'"

# A CHANGED folder hash names the skill. No hash is printed: it is 64 hex
# characters that tell the reader nothing.
write_snapshot "$after" "alpha:npx:aaa2" "beta:npx:bbb1"
line="$(npx_line)"
grep -qF 'npx lane: 1 of 2 tracked skills changed (alpha).' <<<"$line" ||
  fail "a changed npx hash did not name the skill: '$line'"
refute 'aaa2|aaa1' "$line" "the npx line printed a folder hash, which tells the reader nothing"
grep -qF "$NPX_CAVEAT" <<<"$line" ||
  fail "the npx line dropped the caveat that no version number is knowable: '$line'"

# ADDED and REMOVED are changes too, and the removal is the one most worth
# seeing: a skill that silently left the store is exactly what this record is for.
write_snapshot "$before" "alpha:npx:aaa1"
write_snapshot "$after" "alpha:npx:aaa1" "delta:npx:ddd1"
grep -qF '1 of 2 tracked skills changed (delta (added)).' <<<"$(npx_line)" ||
  fail "an added skill was not reported: '$(npx_line)'"
write_snapshot "$before" "alpha:npx:aaa1" "beta:npx:bbb1"
write_snapshot "$after" "alpha:npx:aaa1"
grep -qF 'beta (removed)' <<<"$(npx_line)" ||
  fail "a removed skill was not reported: '$(npx_line)'"

# The CLAWHUB lane reports the version transition, because it is the only lane
# that has one.
write_snapshot "$before" "gamma:clawhub:1.0.0" "epsilon:clawhub:3.1.4"
write_snapshot "$after" "gamma:clawhub:2.5.0" "epsilon:clawhub:3.1.4"
line="$(clawhub_line)"
grep -qF 'clawhub lane: 1 of 2 tracked skills changed (gamma 1.0.0 -> 2.5.0).' <<<"$line" ||
  fail "the clawhub version transition was not reported: '$line'"
grep -qF "$CLAWHUB_CAVEAT" <<<"$line" || fail "the clawhub line dropped its caveat: '$line'"

# The lanes do not bleed into each other: one snapshot file holds both.
write_snapshot "$before" "alpha:npx:aaa1" "gamma:clawhub:1.0.0"
write_snapshot "$after" "alpha:npx:aaa2" "gamma:clawhub:1.0.0"
line="$(npx_line)"
grep -qF 'npx lane: 1 of 1 tracked skills changed (alpha).' <<<"$line" ||
  fail "the npx lane counted a clawhub skill: '$line'"
refute 'gamma' "$line" "the npx lane named a clawhub skill"
line="$(clawhub_line)"
grep -qF 'clawhub lane: 0 of 1 tracked skills changed.' <<<"$line" ||
  fail "the clawhub lane counted an npx skill: '$line'"

# ── A whole-store move must not blow past Discord's 2000-character message cap
#    and take the gap figure with it. The names are capped and the remainder is
#    counted, so the entry stays deliverable and still states the true total. ──
: >"$before"
: >"$after"
for i in $(seq 1 40); do
  printf '%s\n' "$(row "$(printf 'skill%02d' "$i")" npx old)" >>"$before"
  printf '%s\n' "$(row "$(printf 'skill%02d' "$i")" npx new)" >>"$after"
done
line="$(npx_line)"
grep -qF 'npx lane: 40 of 40 tracked skills changed' <<<"$line" ||
  fail "the capped line lost the true totals: '$line'"
grep -qE 'and 28 more' <<<"$line" ||
  fail "the capped line did not count the names it withheld: '$line'"
[[ ${#line} -lt 800 ]] ||
  fail "a whole-store move rendered ${#line} characters; Discord caps a message at 2000"

printf 'update-skills-change-report: OK\n'
