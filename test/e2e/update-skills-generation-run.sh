#!/usr/bin/env bash
# update-skills-generation-run.sh: end-to-end proof that the generation path IS
# the default weekly run (Wave 3a fix4 cutover). A machine with the old flat
# store runs one plain full run and comes out in the generation topology:
#   1. Migration: tracked flat real dirs become stable store symlinks into a
#      complete ~/.agents/.skills-current generation; the flat .skill-lock.json
#      becomes a symlink into it; vendored dirs and foreign links untouched.
#   2. The weekly attempt built a candidate, ran the lanes (per-repo explicit
#      `skills add`, never bulk update), validated, and published: store content
#      is the lanes' refreshed copy.
#   3. Post-publish: Claude + hermes fan-out resolve through the store links,
#      the on-demand overlay is present through the store path, the lock link
#      resolves, and the roster-aware weekly stamp was written.
#   4. No staging leftovers; exactly one retained previous generation at most.
#   5. A second run is green and leaves the topology intact.
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

cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{
  "version": 2,
  "tiers": {"alpha": "on-demand", "claw": "core"},
  "hermesProfiles": {"alpha": ["default"], "claw": []},
  "hermesRegistry": {},
  "npxTracked": {"alpha": {"repo": "fixture/alpha"}},
  "clawhubTracked": {"claw": {"slug": "@fixture/claw", "registry": "https://clawhub.example"}},
  "forks": {}
}
EOF

# The old flat store: tracked real dirs + a vendored dir + the flat npx lock.
mkdir -p "$HOME/.agents/skills/alpha"
printf -- '---\nname: alpha\n---\n# flat legacy\n' >"$HOME/.agents/skills/alpha/SKILL.md"
printf 'LEGACY' >"$HOME/.agents/skills/alpha/local.txt"
mkdir -p "$HOME/.agents/skills/claw/.clawhub"
printf -- '---\nname: claw\n---\n' >"$HOME/.agents/skills/claw/SKILL.md"
printf '{"slug":"claw"}\n' >"$HOME/.agents/skills/claw/.clawhub/origin.json"
mkdir -p "$HOME/.agents/skills/vendored"
printf 'VENDORED' >"$HOME/.agents/skills/vendored/keep.txt"
printf '{"skills":{"alpha":{}}}\n' >"$HOME/.agents/.skill-lock.json"

# Stubs. npx refreshes SKILL.md (leaves other files); logs argv for the
# per-repo-add / never-bulk-update assertions.
stub="$tmp/stub"
mkdir -p "$stub"
NPX_LOG="$tmp/npx.log"
cat >"$stub/npx" <<EOF
#!/usr/bin/env bash
set -euo pipefail
printf 'npx %s\n' "\$*" >>"$NPX_LOG"
mode=""; prev=""; skills=()
for a in "\$@"; do
  case "\$a" in add) mode=add ;; update) mode=update ;; esac
  [[ \$prev == --skill ]] && skills+=("\$a")
  prev="\$a"
done
if [[ \$mode == add ]]; then
  for s in "\${skills[@]}"; do
    mkdir -p "\$HOME/.agents/skills/\$s"
    printf -- '---\nname: %s\n---\n# refreshed by lane\n' "\$s" >"\$HOME/.agents/skills/\$s/SKILL.md"
  done
fi
EOF
cat >"$stub/clawhub" <<EOF
#!/usr/bin/env bash
set -euo pipefail
printf 'clawhub %s\n' "\$*" >>"$tmp/clawhub.log"
echo "stub: up to date"
EOF
chmod +x "$stub/npx" "$stub/clawhub"
export PATH="$stub:$PATH"

run() { UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" 2>&1; }
out="$(run)" || fail "full run exited non-zero: $out"

AGENTS="$HOME/.agents"
CURRENT="$AGENTS/.skills-current"

# 1) Generation topology after one plain full run.
[[ -d $CURRENT && ! -L $CURRENT ]] || fail "no live .skills-current generation after the full run"
[[ -f "$CURRENT/generation.json" ]] || fail "the live generation has no ready marker"
for name in alpha claw; do
  [[ -L "$AGENTS/skills/$name" ]] || fail "store/$name is not a symlink after the full run"
  [[ "$(readlink "$AGENTS/skills/$name")" == "../.skills-current/skills/$name" ]] ||
    fail "store/$name points at the wrong target: $(readlink "$AGENTS/skills/$name")"
  [[ -f "$AGENTS/skills/$name/SKILL.md" ]] || fail "store/$name does not resolve"
done
[[ -d "$AGENTS/skills/vendored" && ! -L "$AGENTS/skills/vendored" ]] ||
  fail "the vendored dir was converted (must stay outside the generation)"
[[ "$(cat "$AGENTS/skills/vendored/keep.txt")" == "VENDORED" ]] || fail "vendored content changed"
[[ -L "$AGENTS/.skill-lock.json" ]] || fail ".skill-lock.json is not a symlink after the full run"
grep -q '"alpha"' "$AGENTS/.skill-lock.json" || fail "the lock symlink does not resolve to lock content"

# 2) The lanes refreshed the content (per-repo explicit add), and migration
#    preserved the legacy sibling file through the clone.
grep -q '# refreshed by lane' "$AGENTS/skills/alpha/SKILL.md" ||
  fail "the npx lane did not refresh alpha (store content is not the lane's copy)"
[[ "$(cat "$AGENTS/skills/alpha/local.txt")" == "LEGACY" ]] ||
  fail "the legacy sibling file did not survive migration + lanes"
grep -qE 'skills@latest add fixture/alpha .*--skill alpha' "$NPX_LOG" ||
  fail "the npx lane did not run an explicit per-repo add: $(cat "$NPX_LOG")"
if grep -qE 'update (--global|-g)' "$NPX_LOG"; then
  fail "the run invoked the bulk npx update (forbidden; per-repo add only): $(cat "$NPX_LOG")"
fi

# 3) Fan-out + overlay + stamp.
[[ -L "$HOME/.claude/skills/alpha" && -f "$HOME/.claude/skills/alpha/SKILL.md" ]] ||
  fail "Claude fan-out for alpha does not resolve"
[[ -L "$HOME/.hermes/skills/alpha" && -f "$HOME/.hermes/skills/alpha/SKILL.md" ]] ||
  fail "hermes default fan-out for alpha does not resolve"
grep -q 'allow_implicit_invocation: false' "$AGENTS/skills/alpha/agents/openai.yaml" ||
  fail "the on-demand overlay is missing through the store path"
stamp="$HOME/.local/state/update-skills/last-success"
[[ -f $stamp ]] || fail "a fully successful run did not write the weekly stamp"
[[ "$(wc -w <"$stamp" | tr -d ' ')" == "3" ]] ||
  fail "the stamp is not <week> <lock-hash> <updater-hash>: $(cat "$stamp")"

# 4) No staging leftovers; at most one retained previous generation.
shopt -s nullglob
staging=("$AGENTS/.skills-generations"/*/home)
garbage=("$AGENTS/.skills-generations"/*.garbage.* "$AGENTS/skills"/*.garbage.* "$AGENTS/skills"/.*.migrating.*)
shopt -u nullglob
[[ ${#staging[@]} -eq 0 ]] || fail "staging leftovers survived the run: ${staging[*]}"
[[ ${#garbage[@]} -eq 0 ]] || fail "garbage leftovers survived the run: ${garbage[*]}"
retained="$(find "$AGENTS/.skills-generations" -maxdepth 2 -name generation.json 2>/dev/null | wc -l | tr -d ' ')"
[[ $retained -le 1 ]] || fail "more than one previous generation retained: $retained"

# 5) A second run is green and the topology holds (FORCE bypasses the stamp).
out2="$(run)" || fail "second full run exited non-zero: $out2"
[[ "$(readlink "$AGENTS/skills/alpha")" == "../.skills-current/skills/alpha" ]] ||
  fail "store/alpha drifted on the second run"
[[ -f "$AGENTS/skills/alpha/SKILL.md" ]] || fail "store/alpha stopped resolving on the second run"

echo "update-skills-generation-run: OK"
