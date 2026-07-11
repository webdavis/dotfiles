#!/usr/bin/env bash
# update-skills-lock-fd-leak.sh (fix-A F8): the serialize lock is held on fd 9,
# and `env -i` clears the environment but NOT open fds, so every child (the
# `--build-lanes` self re-exec and the package CLIs it runs) inherited fd 9. A
# long-lived grandchild left behind by a lane (an npx/clawhub daemon) then keeps
# the flock held after the updater exits, so every later run defers forever.
# The fix closes fd 9 (`9>&-`) on child invocations that do not need the lock.
#
# Regression: the npx lane leaks a background process that inherits its fds and
# outlives the whole run. After the updater exits, a fresh acquisition of the
# same lock must SUCCEED (the leaked grandchild must not still hold it).
#
# darwin-only: the lock uses /usr/bin/lockf; without it the run is unlocked and
# there is no fd to leak, so skip.
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"
fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

if [[ ! -x /usr/bin/lockf ]]; then
  echo "update-skills-lock-fd-leak: SKIP (no /usr/bin/lockf; the lock is darwin-only)"
  exit 0
fi

# shellcheck source=test/fixtures/exchange-tool.lib.sh
source "$REPO_ROOT/test/fixtures/exchange-tool.lib.sh"
GMV_BIN="$(resolve_exchange_tool)" ||
  fail "no GNU coreutils mv with a working --exchange on PATH (need gmv or mv)"

tmp="$(mktemp -d)"
LEAK_PID_FILE="$tmp/leak.pid"
cleanup() {
  [[ -f $LEAK_PID_FILE ]] && kill "$(cat "$LEAK_PID_FILE")" 2>/dev/null
  rm -rf "$tmp"
}
trap cleanup EXIT

HOME="$tmp/home"
export HOME
export UPDATE_SKILLS_GMV="$GMV_BIN"
mkdir -p "$HOME/.agents/skills"
AGENTS="$HOME/.agents"
LOCK="$AGENTS/custom-skill-lock.json"
LOCKFILE="$AGENTS/.update-skills.lock"

cat >"$LOCK" <<'EOF'
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

# npx stub: installs alpha AND leaks a background sleep that inherits the lane's
# fds (including the inherited lock fd, pre-fix) and outlives the whole run. It
# records its PID so the test can reap it.
stub="$tmp/stub"
mkdir -p "$stub"
cat >"$stub/npx" <<EOF
#!/usr/bin/env bash
set -euo pipefail
prev=""; skills=()
for a in "\$@"; do [[ \$prev == --skill ]] && skills+=("\$a"); prev="\$a"; done
cli_lock="\${XDG_STATE_HOME:-\$HOME/.local/state}/skills/.skill-lock.json"
mkdir -p "\$(dirname "\$cli_lock")"
[[ -f \$cli_lock ]] || printf '{"version":3,"skills":{}}\n' >"\$cli_lock"
for s in "\${skills[@]}"; do
  mkdir -p "\$HOME/.agents/skills/\$s"
  printf -- '---\nname: %s\n---\n# lane\n' "\$s" >"\$HOME/.agents/skills/\$s/SKILL.md"
  jq --arg s "\$s" '.skills[\$s] = {source: "github:fixture/pack", agents: ["claude-code","codex"]}' \
    "\$cli_lock" >"\$cli_lock.tmp" && mv "\$cli_lock.tmp" "\$cli_lock"
done
# The leak: a grandchild that outlives the whole updater run. Its std streams
# are redirected off the lane's output pipe (so the pipeline is not held open),
# but it still inherits the lock fd (fd 9) exactly as a real npx daemon would.
sleep 30 >/dev/null 2>&1 &
printf '%s\n' "\$!" >"$LEAK_PID_FILE"
EOF
chmod +x "$stub/npx"
export PATH="$stub:$PATH"

mkdir -p "$AGENTS/skills/alpha"
printf -- '---\nname: alpha\n---\n# seed\n' >"$AGENTS/skills/alpha/SKILL.md"
printf '{"skills":{"alpha":{}}}\n' >"$AGENTS/.skill-lock.json"

UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" >/dev/null 2>&1 || fail "the full run exited non-zero"
[[ -f $LEAK_PID_FILE ]] || fail "the npx lane never ran (no leaked grandchild recorded)"
leak_pid="$(cat "$LEAK_PID_FILE")"
kill -0 "$leak_pid" 2>/dev/null || fail "the leaked grandchild is not alive; the test cannot prove fd release"

# The updater has exited. A fresh acquisition of the SAME lock must succeed: the
# leaked grandchild must not still hold fd 9. Pre-fix, it inherited fd 9 and the
# flock stays held, so this contends (75).
set +e
(exec 7>>"$LOCKFILE" && /usr/bin/lockf -s -t 0 7)
rc_probe=$?
set -e
[[ $rc_probe -eq 0 ]] ||
  fail "the lock is still held after the run exited (rc $rc_probe): a leaked grandchild kept fd 9 open"

echo "update-skills-lock-fd-leak: OK"
