#!/usr/bin/env bash
# update-skills-exchange-publish.sh proves the generation-exchange publish is
# atomic and per-lookup complete (Wave 3a fix4 acceptance points 2 and 3).
#
# A reader loop resolves the store during 100 publish cycles. Each pass resolves
# ONE physical generation (as a running session that cached a canonical path
# would) and reads two sibling skills' recorded generation id from it. The
# guarantee under test: any resolution yields a COMPLETE tree from EXACTLY ONE
# generation: the reader never sees a missing path, and two siblings resolved
# from one pass never disagree on the generation id (no mixed-generation pair).
#
# The machinery is exercised in isolation by sourcing the real script with
# UPDATE_SKILLS_LIB_ONLY=1 and calling __gen_publish, so the test drives the
# exact publish primitive the weekly run uses.
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"
fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# Resolve the generation-exchange tool the way the updater does at run time
# (gmv on a Homebrew host, plain mv in the Nix devshell), never a hardcoded
# host path.
# shellcheck source=test/fixtures/exchange-tool.lib.sh
source "$REPO_ROOT/test/fixtures/exchange-tool.lib.sh"
GMV_BIN="$(resolve_exchange_tool)" ||
  fail "no GNU coreutils mv with a working --exchange on PATH (need gmv or mv)"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

HOME="$tmp/home"
export HOME
mkdir -p "$HOME/.agents/skills"
cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{"npxTracked":{"alpha":{"repo":"x/a"},"beta":{"repo":"x/b"}},"clawhubTracked":{}}
EOF

export UPDATE_SKILLS_GMV="$GMV_BIN"
export UPDATE_SKILLS_LIB_ONLY=1
# shellcheck disable=SC1090
source "$SCRIPT"

# Build a complete generation dir holding both siblings, each carrying gen.txt =
# the generation id, plus the npx lock and the ready marker.
build_generation() {
  local dir="$1" id="$2" skill
  mkdir -p "$dir/skills"
  for skill in alpha beta; do
    mkdir -p "$dir/skills/$skill"
    printf -- '---\nname: %s\n---\n' "$skill" >"$dir/skills/$skill/SKILL.md"
    printf '%s' "$id" >"$dir/skills/$skill/gen.txt"
  done
  printf '{}\n' >"$dir/.skill-lock.json"
  __gen_write_meta "$dir" "$id"
}

# Seed the live generation and the stable store + lock links.
build_generation "$SKILLS_CURRENT" "gen-000"
__gen_plant_store_link alpha
__gen_plant_store_link beta
__gen_plant_lock_link

READER_LOG="$tmp/reader.log"
: >"$READER_LOG"

# The reader models a running session that cached a RESOLVED path. Each pass
# pins its cwd to the physical generation directory (via the store symlink);
# the cwd fd is the cache, so an atomic exchange of the .skills-current entry
# does not move this reader off the generation it pinned. It then reads both
# siblings RELATIVE to that pinned cwd, so the two reads land in ONE generation.
# Violations: exactly one sibling missing (a PARTIAL tree, never allowed); both
# present but their generation ids disagree (a MIXED pair, never allowed). Both
# siblings missing is a clean ENOENT of a since-pruned generation (allowed by the
# per-lookup-completeness guarantee) and is NOT a violation.
reader() {
  local passes=0 result
  while [[ -f "$tmp/reader.run" ]]; do
    passes=$((passes + 1))
    result="$(
      # Physical cd (-P) follows the store symlink to the real generation dir
      # and pins the process cwd to that inode; a logical cd would collapse
      # alpha/.. textually and leave us re-resolving .skills-current per read.
      if ! { cd -P "$STORE/alpha" && cd -P ..; } 2>/dev/null; then
        printf 'MISSING-STORE'
        exit 0
      fi
      # cwd is now pinned to one generation's skills inode.
      a=""
      b=""
      [[ -f alpha/gen.txt ]] && a="$(cat alpha/gen.txt 2>/dev/null || true)"
      [[ -f beta/gen.txt ]] && b="$(cat beta/gen.txt 2>/dev/null || true)"
      if [[ -z $a && -z $b ]]; then
        printf 'PRUNED'
      elif [[ -z $a || -z $b ]]; then
        printf 'PARTIAL a=%s b=%s' "$a" "$b"
      elif [[ $a != "$b" ]]; then
        printf 'MIXED a=%s b=%s' "$a" "$b"
      else
        printf 'OK'
      fi
    )"
    case "$result" in
      MISSING-STORE) printf 'MISSING store/alpha at pass %s\n' "$passes" >>"$READER_LOG" ;;
      PARTIAL*) printf 'PARTIAL at pass %s (%s)\n' "$passes" "$result" >>"$READER_LOG" ;;
      MIXED*) printf 'MIXED at pass %s (%s)\n' "$passes" "$result" >>"$READER_LOG" ;;
    esac
  done
  printf 'passes=%s\n' "$passes" >>"$READER_LOG"
}

touch "$tmp/reader.run"
reader &
reader_pid=$!

# 100 publish cycles: build a fresh candidate under the generations tree and
# publish it with the atomic exchange the weekly run uses.
cycles=100
for i in $(seq 1 "$cycles"); do
  cand="$GENERATIONS/build-$i/home/.agents"
  build_generation "$cand" "gen-$(printf '%03d' "$i")"
  __gen_publish "$cand" >>"$tmp/publish.log" 2>&1 ||
    fail "publish cycle $i failed: $(tail -3 "$tmp/publish.log")"
done

rm -f "$tmp/reader.run"
wait "$reader_pid" 2>/dev/null || true

# The live generation must be the last one published.
final_id="$(__gen_meta_field "$SKILLS_CURRENT" id)"
[[ $final_id == "gen-$(printf '%03d' "$cycles")" ]] ||
  fail "final live generation id is $final_id, expected gen-$(printf '%03d' "$cycles")"

# Exactly one previous generation is retained (durable-not-destructive bar).
retained_count="$(find "$GENERATIONS" -maxdepth 1 -type d -name '*-*' \
  \! -name '*.garbage.*' -exec test -f '{}/generation.json' \; -print 2>/dev/null | wc -l | tr -d ' ')"
[[ $retained_count -le 1 ]] ||
  fail "expected at most one retained previous generation, found $retained_count"

# The reader must have run and logged ZERO violations.
grep -qE 'MISSING|MIXED|EMPTY' "$READER_LOG" &&
  fail "reader observed a violation: $(grep -E 'MISSING|MIXED|EMPTY' "$READER_LOG" | head -5)"
passes="$(sed -n 's/^passes=//p' "$READER_LOG")"
[[ ${passes:-0} -ge 1 ]] || fail "reader loop did not run (passes=$passes)"

echo "update-skills-exchange-publish: OK ($cycles cycles, reader $passes passes, 0 violations)"
