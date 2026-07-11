#!/usr/bin/env bash
# update-skills-exchange-tool.sh: the generation-exchange tool is resolved at
# RUN TIME on PATH, never hardcoded to a host location (/opt/homebrew/bin/gmv
# broke the Nix devshell, whose GNU coreutils mv is plain mv). Contract:
#   1. Resolution order is gmv then mv; a candidate is accepted only when
#      --version says GNU coreutils AND a functional probe performs a real
#      --exchange --no-copy -T swap. A non-GNU binary named gmv is skipped in
#      favor of a capable mv, so publishes succeed on a GNU-mv-only userland.
#   2. The resolved tool is cached per run: repeated publishes probe once.
#   3. With NO capable tool on PATH, a publish is a LOUD no-op: it fails with a
#      clear message and both the live generation and the candidate stay
#      complete (never a partial operation).
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
REAL_TOOL="$(resolve_exchange_tool)" ||
  fail "no GNU coreutils mv with a working --exchange on PATH (need gmv or mv)"
REAL_TOOL_PATH="$(command -v "$REAL_TOOL")"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

HOME="$tmp/home"
export HOME
mkdir -p "$HOME/.agents/skills"
cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{"npxTracked":{"alpha":{"repo":"x/a"}},"clawhubTracked":{}}
EOF
unset UPDATE_SKILLS_GMV

# Each case runs in its own interpreter (fresh per-run tool cache) with a
# sandbox dir PREPENDED to PATH so its gmv/mv shadow the real ones. The driver
# sources the script lib-only, seeds a live generation, builds a candidate,
# publishes, and reports the resolved tool plus the live generation id.
driver="$tmp/driver.sh"
cat >"$driver" <<DRIVER
#!/usr/bin/env bash
set -euo pipefail
cycles="\${DRIVER_CYCLES:-1}"
set -- # the sourced script parses \$@; it must see no arguments
export UPDATE_SKILLS_LIB_ONLY=1
# shellcheck disable=SC1090
source "$SCRIPT"
build_generation() {
  local dir="\$1" id="\$2"
  mkdir -p "\$dir/skills/alpha"
  printf -- '---\nname: alpha\n---\n' >"\$dir/skills/alpha/SKILL.md"
  printf '{}\n' >"\$dir/.skill-lock.json"
  __gen_write_meta "\$dir" "\$id"
}
rm -rf "\$SKILLS_CURRENT" "\$GENERATIONS"
build_generation "\$SKILLS_CURRENT" gen-live
for i in \$(seq 1 "\$cycles"); do
  cand="\$GENERATIONS/build-\$i/home/.agents"
  build_generation "\$cand" "gen-\$i"
  if ! __gen_publish "\$cand"; then
    printf 'publish-rc=1\n'
    exit 0
  fi
done
printf 'publish-rc=0\n'
printf 'resolved=%s\n' "\$GEN_EXCHANGE_TOOL"
printf 'live-id=%s\n' "\$(__gen_meta_field "\$SKILLS_CURRENT" id)"
DRIVER
chmod +x "$driver"

# ── 1. a non-GNU gmv is skipped; the capable mv is picked ──────────────────
sandbox1="$tmp/sandbox1"
mkdir -p "$sandbox1"
cat >"$sandbox1/gmv" <<'EOF'
#!/usr/bin/env bash
if [[ ${1:-} == --version ]]; then
  echo "fake mv 1.0 (not coreutils)"
  exit 0
fi
exit 1
EOF
chmod +x "$sandbox1/gmv"
ln -s "$REAL_TOOL_PATH" "$sandbox1/mv"
out="$(PATH="$sandbox1:$PATH" DRIVER_CYCLES=1 bash "$driver")" || fail "case 1 driver failed: $out"
grep -q 'publish-rc=0' <<<"$out" || fail "case 1: publish failed with a capable mv on PATH: $out"
grep -q 'resolved=mv' <<<"$out" ||
  fail "case 1: resolution did not skip the non-GNU gmv in favor of mv: $out"
grep -q 'live-id=gen-1' <<<"$out" || fail "case 1: the live generation was not published: $out"

# ── 2. the resolved tool is cached: 3 publishes probe --version once ───────
sandbox2="$tmp/sandbox2"
mkdir -p "$sandbox2"
version_calls="$tmp/version-calls"
: >"$version_calls"
cat >"$sandbox2/gmv" <<EOF
#!/usr/bin/env bash
if [[ \${1:-} == --version ]]; then
  printf 'v\n' >>"$version_calls"
fi
exec "$REAL_TOOL_PATH" "\$@"
EOF
chmod +x "$sandbox2/gmv"
out="$(PATH="$sandbox2:$PATH" DRIVER_CYCLES=3 bash "$driver")" || fail "case 2 driver failed: $out"
grep -q 'publish-rc=0' <<<"$out" || fail "case 2: publishes failed: $out"
grep -q 'live-id=gen-3' <<<"$out" || fail "case 2: the last generation is not live: $out"
probe_count="$(wc -l <"$version_calls" | tr -d ' ')"
[[ $probe_count -eq 1 ]] ||
  fail "case 2: expected exactly 1 --version probe across 3 publishes (per-run cache); got $probe_count"

# ── 3. no capable tool: publish is a loud no-op, nothing partial ───────────
sandbox3="$tmp/sandbox3"
mkdir -p "$sandbox3"
for name in gmv mv; do
  cat >"$sandbox3/$name" <<'EOF'
#!/usr/bin/env bash
if [[ ${1:-} == --version ]]; then
  echo "stub mv 1.0 (not coreutils)"
  exit 0
fi
for arg in "$@"; do
  [[ $arg == --exchange ]] && exit 1
done
exec /bin/mv "$@"
EOF
  chmod +x "$sandbox3/$name"
done
out="$(PATH="$sandbox3:$PATH" DRIVER_CYCLES=1 bash "$driver" 2>&1)" || fail "case 3 driver crashed: $out"
grep -q 'publish-rc=1' <<<"$out" || fail "case 3: publish claimed success with no capable exchange tool: $out"
grep -q 'no GNU coreutils mv with a working --exchange' <<<"$out" ||
  fail "case 3: the missing exchange tool was not reported loudly: $out"
# No partial operation: the live generation and the candidate are both complete.
[[ -f "$HOME/.agents/.skills-current/skills/alpha/SKILL.md" ]] ||
  fail "case 3: the live generation lost content on a failed publish"
live_id="$(jq -r '.id' "$HOME/.agents/.skills-current/generation.json")"
[[ $live_id == gen-live ]] || fail "case 3: the live generation changed on a failed publish (id=$live_id)"
[[ -f "$HOME/.agents/.skills-generations/build-1/home/.agents/skills/alpha/SKILL.md" ]] ||
  fail "case 3: the candidate was partially consumed by a failed publish"

echo "update-skills-exchange-tool: OK (resolution, per-run cache, loud no-op)"
