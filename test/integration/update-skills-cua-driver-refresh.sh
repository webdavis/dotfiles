#!/usr/bin/env bash
# update-skills-cua-driver-refresh.sh — proves the weekly app-owned skill-pack
# refresh: cua-driver's store entry is a symlink into the app's own dir, and
# the ONLY sanctioned refresh is the app's own updater (`cua-driver skills
# update`, which re-fetches the pack from GitHub Releases). The real script
# runs unmodified in a sandbox: scratch HOME plus PATH stubs for `cua-driver`
# and `npx` that record argv instead of touching the network. Assertions:
#   1. A full run invokes `cua-driver skills update` exactly once.
#   2. --install-only never invokes it (network-dependent pass).
#   3. --dry-run reports a would-run line and invokes it zero times.
#   4. No store symlink for cua-driver => zero invocations (the refresh is
#      gated on the roster actually delivering the app-owned entry — this is
#      also what keeps other sandboxed tests off the REAL binary).
#   5. cua-driver off PATH => graceful skip (exit 0, skip line), never a
#      failure — half-provisioned machines must survive the weekly run.
#   6. Failure isolation: the stub exits non-zero, the run logs a WARN and
#      still exits 0 (one broken pass must never kill the weekly run).
set -euo pipefail

# When git runs a hook such as pre-commit (this test runs under one via
# `just test`), it exports GIT_DIR/GIT_INDEX_FILE, which point every later git
# command at the OUTER repository. Unset them so nothing here can reach it.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

scratch_dir="$(mktemp -d)"
trap 'rm -rf "$scratch_dir"' EXIT

# Scratch HOME: the script derives every path from $HOME.
HOME="$scratch_dir/home"
export HOME
mkdir -p "$HOME/.agents/skills"

# Minimal lock: no npx-tracked skills, no forks, no hermes registry — the only
# refreshable thing in this sandbox is the app-owned pack.
cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{
  "version": 2,
  "tiers": {},
  "hermesProfiles": {},
  "hermesRegistry": {},
  "npxTracked": {},
  "forks": {}
}
EOF

# The app-owned shape: a real pack dir under the app's home, and the store
# entry is a SYMLINK to it (mirrors chezmoi's symlink_cua-driver.tmpl).
mkdir -p "$HOME/.cua-driver/skills/cua-driver"
printf -- '---\nname: cua-driver\ndescription: fixture\n---\n' >"$HOME/.cua-driver/skills/cua-driver/SKILL.md"
ln -s "$HOME/.cua-driver/skills/cua-driver" "$HOME/.agents/skills/cua-driver"

# PATH stubs: cua-driver records argv (fails on demand via a flag file); npx
# records and succeeds so the full run never reaches the network.
stub_dir="$scratch_dir/stubs"
mkdir -p "$stub_dir"
cua_log="$scratch_dir/cua-argv.log"
fail_flag="$scratch_dir/cua-fail"
cat >"$stub_dir/cua-driver" <<EOF
#!/usr/bin/env bash
printf '%s\n' "\$*" >>"$cua_log"
if [[ -e "$fail_flag" ]]; then
  echo "stub: release fetch exploded" >&2
  exit 1
fi
echo "stub: skill pack refreshed"
EOF
cat >"$stub_dir/npx" <<EOF
#!/usr/bin/env bash
printf 'npx %s\n' "\$*" >>"$scratch_dir/npx.log"
echo "stub: nothing to update"
EOF
chmod +x "$stub_dir/cua-driver" "$stub_dir/npx"
export PATH="$stub_dir:$PATH"

# ── 1. full run invokes the app's own updater, exactly once ────────────────
output="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" 2>&1)" || fail "full run exited non-zero: $output"
grep -Fxq -- "skills update" "$cua_log" ||
  fail "full run never invoked 'cua-driver skills update'; got: $(cat "$cua_log" 2>/dev/null)"
[[ "$(wc -l <"$cua_log" | tr -d ' ')" == "1" ]] ||
  fail "expected exactly 1 cua-driver invocation, got: $(cat "$cua_log")"

# ── 2. --install-only never reaches the refresh ────────────────────────────
: >"$cua_log"
UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --install-only >/dev/null 2>&1 || fail "--install-only run failed"
[[ ! -s $cua_log ]] || fail "--install-only invoked cua-driver: $(cat "$cua_log")"

# ── 3. --dry-run reports and never invokes ─────────────────────────────────
: >"$cua_log"
dry_output="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --dry-run 2>&1)" || fail "--dry-run run failed"
[[ ! -s $cua_log ]] || fail "--dry-run invoked cua-driver: $(cat "$cua_log")"
printf '%s\n' "$dry_output" | grep -q "would run: cua-driver skills update" ||
  fail "--dry-run did not report the would-run line: $dry_output"

# ── 4. no store symlink => the refresh is gated off ────────────────────────
: >"$cua_log"
rm "$HOME/.agents/skills/cua-driver"
UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" >/dev/null 2>&1 || fail "full run without the store symlink failed"
[[ ! -s $cua_log ]] ||
  fail "the refresh ran without a store cua-driver symlink (gating lost — sandboxed tests would hit the real binary): $(cat "$cua_log")"
ln -s "$HOME/.cua-driver/skills/cua-driver" "$HOME/.agents/skills/cua-driver"

# ── 5. cua-driver off PATH: graceful skip ──────────────────────────────────
no_cua_dir="$scratch_dir/no-cua"
mkdir -p "$no_cua_dir"
cp "$stub_dir/npx" "$no_cua_dir/npx"
ln -s "$(command -v jq)" "$no_cua_dir/jq"
ln -s "$(command -v git)" "$no_cua_dir/git"
missing_output="$(UPDATE_SKILLS_FORCE=1 PATH="$no_cua_dir:/usr/bin:/bin" bash "$SCRIPT" 2>&1)" ||
  fail "run without cua-driver on PATH exited non-zero: $missing_output"
printf '%s\n' "$missing_output" | grep -qi "cua-driver.*skip" ||
  fail "missing cua-driver was not reported as a skip: $missing_output"

# ── 6. failure isolation: a broken refresh warns and the run survives ──────
: >"$cua_log"
touch "$fail_flag"
fail_output="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" 2>&1)" ||
  fail "a failing cua-driver refresh killed the run: $fail_output"
printf '%s\n' "$fail_output" | grep -i "warn" | grep -qi "cua-driver" ||
  fail "no WARN naming the failed cua-driver refresh: $fail_output"

echo "update-skills-cua-driver-refresh: OK"
