#!/usr/bin/env bash
# update-skills-postexchange-crash.sh (R2-3): a crash AFTER the publish
# exchange but BEFORE retention must not roll the old generation back in, and
# must never delete the refreshed generation. The crash window leaves the OLD
# generation under the candidate workspace (the exchange displaced it there);
# pre-fix, recovery reused it on a hash+buildMode match without checking that
# the workspace id equals the meta id, published the OLD generation back over
# the refreshed one, and the retention path (whose target CONTAINED the
# candidate) then garbage-destroyed the refreshed generation while the run
# returned success and stamped.
#
# Cases:
#   1. recovery (lib): the displaced old generation is NOT marked reusable
#      (workspace id != meta id), and the live refreshed generation is intact;
#   2. recovery (lib) with the exchange-in-flight marker: retention is
#      COMPLETED (old generation moved to its retained slot), marker removed;
#   3. publish (lib): a retention path that contains the candidate is refused
#      BEFORE the exchange (live untouched, candidate intact);
#   4. publish (lib): a retention move failure is FATAL (publish returns
#      non-zero, so the caller records a required failure and never stamps);
#   5. end to end: a full run over the fabricated crash state never makes the
#      OLD content live and never writes a stamp claiming the rolled-back
#      state succeeded.
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"
fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# shellcheck source=test/fixtures/exchange-tool.lib.sh
source "$REPO_ROOT/test/fixtures/exchange-tool.lib.sh"
GMV_BIN="$(resolve_exchange_tool)" ||
  fail "no GNU coreutils mv with a working --exchange on PATH (need gmv or mv)"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

NEW_ID="2000000100-42-1111"
OLD_ID="1000000100-41-9999"

# Build one sandbox home holding the fabricated crash state: the LIVE
# generation is the REFRESHED one (id NEW_ID), and the candidate workspace
# $GENERATIONS/NEW_ID/home/.agents holds the DISPLACED OLD generation (meta id
# OLD_ID) - exactly what the exchange leaves when the process dies before
# retention. $1 = target home dir.
build_crash_state() {
  local home="$1" agents="$1/.agents"
  rm -rf "$home"
  mkdir -p "$agents/skills"
  cat >"$agents/custom-skill-lock.json" <<'EOF'
{
  "version": 2,
  "tiers": {"alpha": "core"},
  "hermesProfiles": {},
  "hermesRegistry": {},
  "npxTracked": {"alpha": {"repo": "fixture/pack"}},
  "clawhubTracked": {},
  "forks": {}
}
EOF
  # live = refreshed generation NEW_ID
  mkdir -p "$agents/.skills-current/skills/alpha"
  printf -- '---\nname: alpha\n---\n# refreshed\n' >"$agents/.skills-current/skills/alpha/SKILL.md"
  printf '{"skills":{"alpha":{}}}\n' >"$agents/.skills-current/.skill-lock.json"
  # workspace NEW_ID holds the displaced OLD generation (meta id OLD_ID)
  mkdir -p "$agents/.skills-generations/$NEW_ID/home/.agents/skills/alpha"
  printf -- '---\nname: alpha\n---\n# seed\n' \
    >"$agents/.skills-generations/$NEW_ID/home/.agents/skills/alpha/SKILL.md"
  printf '{"skills":{"alpha":{}}}\n' \
    >"$agents/.skills-generations/$NEW_ID/home/.agents/.skill-lock.json"
  # store + lock links
  ln -s "../.skills-current/skills/alpha" "$agents/skills/alpha"
  ln -s ".skills-current/.skill-lock.json" "$agents/.skill-lock.json"
}

# Write both generation.json files with LIVE-matching hashes; must run inside
# the sourced-lib environment (uses __gen_write_meta so hashes always match the
# current roster + updater). $1 = agents dir.
write_crash_metas() {
  local agents="$1"
  __gen_write_meta "$agents/.skills-current" "$NEW_ID" full
  __gen_write_meta "$agents/.skills-generations/$NEW_ID/home/.agents" "$OLD_ID" full
}

# ── Case 1: recovery must not mark the displaced old generation reusable ─────
home1="$tmp/home1"
build_crash_state "$home1"
out1="$(
  HOME="$home1" UPDATE_SKILLS_LIB_ONLY=1 bash -s "$SCRIPT" "$home1" <<'INNER'
set -euo pipefail
script="$1"; home="$2"
set --
# shellcheck disable=SC1090
source "$script"
__gen_write_meta "$home/.agents/.skills-current" "2000000100-42-1111" full
__gen_write_meta "$home/.agents/.skills-generations/2000000100-42-1111/home/.agents" "1000000100-41-9999" full
__gen_recover
printf 'REUSE=%s\n' "${GEN_REUSE_CANDIDATE:-<none>}"
printf 'LIVE_ID=%s\n' "$(__gen_meta_field "$SKILLS_CURRENT" id)"
INNER
)"
grep -q 'REUSE=<none>' <<<"$out1" ||
  fail "case 1: recovery marked the displaced OLD generation reusable (workspace id != meta id must refuse): $out1"
grep -q "LIVE_ID=$NEW_ID" <<<"$out1" ||
  fail "case 1: the live refreshed generation did not survive recovery: $out1"
[[ "$(cat "$home1/.agents/.skills-current/skills/alpha/SKILL.md")" == *"# refreshed"* ]] ||
  fail "case 1: the refreshed live content was altered by recovery"

# ── Case 2: with the exchange-in-flight marker, retention is COMPLETED ───────
home2="$tmp/home2"
build_crash_state "$home2"
out2="$(
  HOME="$home2" UPDATE_SKILLS_LIB_ONLY=1 bash -s "$SCRIPT" "$home2" <<'INNER'
set -euo pipefail
script="$1"; home="$2"
set --
# shellcheck disable=SC1090
source "$script"
__gen_write_meta "$home/.agents/.skills-current" "2000000100-42-1111" full
__gen_write_meta "$home/.agents/.skills-generations/2000000100-42-1111/home/.agents" "1000000100-41-9999" full
jq -n --arg oldId "1000000100-41-9999" --arg workspaceId "2000000100-42-1111" \
  '{oldId: $oldId, workspaceId: $workspaceId}' \
  >"$home/.agents/.skills-generations/.exchange-in-flight"
__gen_recover
printf 'REUSE=%s\n' "${GEN_REUSE_CANDIDATE:-<none>}"
printf 'LIVE_ID=%s\n' "$(__gen_meta_field "$SKILLS_CURRENT" id)"
INNER
)"
grep -q 'REUSE=<none>' <<<"$out2" ||
  fail "case 2: recovery reused the displaced OLD generation despite the marker: $out2"
grep -q "LIVE_ID=$NEW_ID" <<<"$out2" ||
  fail "case 2: the live refreshed generation did not survive marker recovery: $out2"
[[ -d "$home2/.agents/.skills-generations/$OLD_ID" ]] ||
  fail "case 2: retention was not completed (no retained $OLD_ID after marker recovery)"
[[ "$(cat "$home2/.agents/.skills-generations/$OLD_ID/skills/alpha/SKILL.md")" == *"# seed"* ]] ||
  fail "case 2: the retained previous generation lost its content"
[[ ! -f "$home2/.agents/.skills-generations/.exchange-in-flight" ]] ||
  fail "case 2: the exchange-in-flight marker survived recovery"

# ── Case 3: publish refuses a retention path that contains the candidate ─────
home3="$tmp/home3"
build_crash_state "$home3"
out3="$(
  HOME="$home3" UPDATE_SKILLS_GMV="$GMV_BIN" UPDATE_SKILLS_LIB_ONLY=1 \
    bash -s "$SCRIPT" "$home3" <<'INNER'
set -euo pipefail
script="$1"; home="$2"
set --
# shellcheck disable=SC1090
source "$script"
# live current meta id EQUALS the candidate's workspace id, so the retention
# target $GENERATIONS/<old_id> would CONTAIN the candidate.
__gen_write_meta "$home/.agents/.skills-current" "2000000100-42-1111" full
__gen_write_meta "$home/.agents/.skills-generations/2000000100-42-1111/home/.agents" "1000000100-41-9999" full
if __gen_publish "$home/.agents/.skills-generations/2000000100-42-1111/home/.agents"; then
  printf 'PUBLISH=accepted\n'
else
  printf 'PUBLISH=refused\n'
fi
printf 'LIVE_ID=%s\n' "$(__gen_meta_field "$SKILLS_CURRENT" id)"
INNER
)"
grep -q 'PUBLISH=refused' <<<"$out3" ||
  fail "case 3: publish accepted a retention path containing the candidate: $out3"
grep -q "LIVE_ID=$NEW_ID" <<<"$out3" ||
  fail "case 3: the refused publish still exchanged the live generation: $out3"
[[ -f "$home3/.agents/.skills-generations/$NEW_ID/home/.agents/skills/alpha/SKILL.md" ]] ||
  fail "case 3: the refused publish deleted the candidate workspace content"
[[ "$(cat "$home3/.agents/.skills-current/skills/alpha/SKILL.md")" == *"# refreshed"* ]] ||
  fail "case 3: the refused publish altered live content"

# ── Case 4: a retention move failure is FATAL (publish returns non-zero) ─────
home4="$tmp/home4"
build_crash_state "$home4"
# A DISTINCT workspace id (a normal fresh candidate) so only the retention mv
# is at fault; the stubbed mv fails exactly that move.
out4="$(
  HOME="$home4" UPDATE_SKILLS_GMV="$GMV_BIN" UPDATE_SKILLS_LIB_ONLY=1 \
    bash -s "$SCRIPT" "$home4" <<'INNER'
set -euo pipefail
script="$1"; home="$2"
set --
# shellcheck disable=SC1090
source "$script"
cand="$home/.agents/.skills-generations/fresh-77-7/home/.agents"
mkdir -p "$cand/skills/alpha"
printf -- '---\nname: alpha\n---\n# fresh\n' >"$cand/skills/alpha/SKILL.md"
printf '{"skills":{"alpha":{}}}\n' >"$cand/.skill-lock.json"
__gen_write_meta "$home/.agents/.skills-current" "2000000100-42-1111" full
__gen_write_meta "$cand" "fresh-77-7" full
# Fail exactly the retention move (candidate -> retained slot); every other mv
# is the real one.
# shellcheck disable=SC2317
mv() {
  if [[ ${2:-} == */.skills-generations/2000000100-42-1111 ]]; then
    return 1
  fi
  command mv "$@"
}
if __gen_publish "$cand"; then
  printf 'PUBLISH=success\n'
else
  printf 'PUBLISH=fatal\n'
fi
printf 'LIVE_ID=%s\n' "$(__gen_meta_field "$SKILLS_CURRENT" id)"
INNER
)"
grep -q 'PUBLISH=fatal' <<<"$out4" ||
  fail "case 4: a failed retention move was not FATAL (publish claimed success, so the caller would stamp): $out4"
grep -q 'LIVE_ID=fresh-77-7' <<<"$out4" ||
  fail "case 4: the exchange did not land before the retention failure (unexpected fixture drift): $out4"

# ── Case 5: end to end, the crash state never rolls back or stamps stale ─────
home5="$tmp/home5"
build_crash_state "$home5"
# metas need the lib to compute matching hashes
HOME="$home5" UPDATE_SKILLS_LIB_ONLY=1 bash -s "$SCRIPT" "$home5" <<'INNER'
set -euo pipefail
script="$1"; home="$2"
set --
# shellcheck disable=SC1090
source "$script"
__gen_write_meta "$home/.agents/.skills-current" "2000000100-42-1111" full
__gen_write_meta "$home/.agents/.skills-generations/2000000100-42-1111/home/.agents" "1000000100-41-9999" full
INNER
stub="$tmp/stub"
mkdir -p "$stub"
cat >"$stub/npx" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
prev=""
skills=()
for a in "$@"; do
  [[ $prev == --skill ]] && skills+=("$a")
  prev="$a"
done
for s in "${skills[@]}"; do
  mkdir -p "$HOME/.agents/skills/$s"
  printf -- '---\nname: %s\n---\n# lane\n' "$s" >"$HOME/.agents/skills/$s/SKILL.md"
done
EOF
chmod +x "$stub/npx"
out5="$(HOME="$home5" PATH="$stub:$PATH" UPDATE_SKILLS_GMV="$GMV_BIN" UPDATE_SKILLS_FORCE=1 \
  bash "$SCRIPT" 2>&1)" || fail "case 5: the full run exited non-zero: $out5"
live_md="$home5/.agents/.skills-current/skills/alpha/SKILL.md"
[[ -f $live_md ]] || fail "case 5: no live alpha SKILL.md after the run"
[[ "$(cat "$live_md")" != *"# seed"* ]] ||
  fail "case 5: the OLD generation content was rolled back to live: $out5"
stamp5="$home5/.local/state/update-skills/last-success"
if [[ -f $stamp5 ]]; then
  # A stamp is only legitimate for a run whose live result is FRESH (the lane
  # content), never the rolled-back seed.
  [[ "$(cat "$live_md")" == *"# lane"* || "$(cat "$live_md")" == *"# refreshed"* ]] ||
    fail "case 5: a success stamp was written while stale content is live"
fi

echo "update-skills-postexchange-crash: OK"
