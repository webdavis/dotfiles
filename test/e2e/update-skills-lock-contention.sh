#!/usr/bin/env bash
# update-skills-lock-contention.sh (fix-A F1): the serialize-lock acquisition
# must distinguish CONTENTION from a HARD acquisition failure. The pre-fix code
# `if ! __update_skills_acquire_lock; then log "another run in progress"; exit 0`
# collapsed both into a silent exit 0, so:
#   - contention (another run holds the lock, lockf EX_TEMPFAIL 75) during an
#     --install-only run exited 0, which the first-install wrapper reads as
#     "installed, clear the marker" — losing a deferred install; and
#   - a NON-contention failure (unwritable ~/.agents so `exec 9>>` fails) also
#     exited 0 silently, with no required-failure accounting and no alert.
# The fix captures the status: contention exits the distinct retryable 75 in
# ANY mode (the wrapper preserves its retry marker), and any other non-zero is a
# REQUIRED failure (loud + relay, no stamp, non-zero exit, marker preserved).
#
# darwin-only mechanism: the lock uses /usr/bin/lockf; without it the script
# proceeds unlocked and there is no contention to observe, so skip.
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"
fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

if [[ ! -x /usr/bin/lockf ]]; then
  echo "update-skills-lock-contention: SKIP (no /usr/bin/lockf; contention is darwin-only)"
  exit 0
fi

# shellcheck source=test/fixtures/exchange-tool.lib.sh
source "$REPO_ROOT/test/fixtures/exchange-tool.lib.sh"
GMV_BIN="$(resolve_exchange_tool)" ||
  fail "no GNU coreutils mv with a working --exchange on PATH (need gmv or mv)"

tmp="$(mktemp -d)"
cleanup() {
  chmod -R u+rwx "$tmp" 2>/dev/null || true
  rm -rf "$tmp"
}
trap cleanup EXIT

HOME="$tmp/home"
export HOME
export UPDATE_SKILLS_GMV="$GMV_BIN"
mkdir -p "$HOME/.agents/skills"
AGENTS="$HOME/.agents"
CURRENT="$AGENTS/.skills-current"
LOCK="$AGENTS/custom-skill-lock.json"
LOCKFILE="$AGENTS/.update-skills.lock"

write_lock() {
  local tiers="" npx="" n
  for n in "$@"; do
    tiers+="\"$n\": \"core\", "
    npx+="\"$n\": {\"repo\": \"fixture/pack\"}, "
  done
  cat >"$LOCK" <<EOF
{
  "version": 2,
  "tiers": {${tiers%, }},
  "hermesProfiles": {},
  "hermesRegistry": {},
  "npxTracked": {${npx%, }},
  "clawhubTracked": {},
  "forks": {}
}
EOF
}

stub="$tmp/stub"
mkdir -p "$stub"
cat >"$stub/npx" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
prev=""; skills=()
for a in "$@"; do [[ $prev == --skill ]] && skills+=("$a"); prev="$a"; done
cli_lock="${XDG_STATE_HOME:-$HOME/.local/state}/skills/.skill-lock.json"
mkdir -p "$(dirname "$cli_lock")"
[[ -f $cli_lock ]] || printf '{"version":3,"skills":{}}\n' >"$cli_lock"
for s in "${skills[@]}"; do
  mkdir -p "$HOME/.agents/skills/$s"
  printf -- '---\nname: %s\n---\n# lane\n' "$s" >"$HOME/.agents/skills/$s/SKILL.md"
  jq --arg s "$s" '.skills[$s] = {source: "github:fixture/pack", agents: ["claude-code","codex"]}' \
    "$cli_lock" >"$cli_lock.tmp" && mv "$cli_lock.tmp" "$cli_lock"
done
EOF
# no-op alerter: the real one blocks for its --timeout waiting for interaction.
printf '#!/usr/bin/env bash\nexit 0\n' >"$stub/alerter"
chmod +x "$stub/npx" "$stub/alerter"
export PATH="$stub:$PATH"

# ── Establish a live generation with alpha (releases the lock on exit) ────────
write_lock alpha
mkdir -p "$AGENTS/skills/alpha"
printf -- '---\nname: alpha\n---\n# seed\n' >"$AGENTS/skills/alpha/SKILL.md"
printf '{"skills":{"alpha":{}}}\n' >"$AGENTS/.skill-lock.json"
UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" >/dev/null 2>&1 || fail "setup full run failed"
[[ -f "$CURRENT/generation.json" ]] || fail "setup did not publish a generation"
id_before="$(jq -r '.id' "$CURRENT/generation.json")"

# ── Part A: a real held lock during --install-only exits the retryable 75 ─────
# beta is genuine install-only work; the exchange must be deferred (75), not
# reported as a success (0), so the wrapper keeps its retry marker.
write_lock alpha beta
exec 8>>"$LOCKFILE"
/usr/bin/lockf -s -t 0 8 || fail "the test could not pre-acquire the serialize lock"
set +e
out_contend="$(bash "$SCRIPT" --install-only 8>&- 2>&1)"
rc_contend=$?
set -e
exec 8>&- # release the held lock

[[ $rc_contend -eq 75 ]] ||
  fail "contention during --install-only did not exit the retryable 75 (got $rc_contend): $out_contend"
grep -qiE 'another run holds the lock|deferring' <<<"$out_contend" ||
  fail "the contended run did not log a deferral: $out_contend"
[[ "$(jq -r '.id' "$CURRENT/generation.json")" == "$id_before" ]] ||
  fail "the contended run mutated the live generation"
[[ ! -e "$AGENTS/skills/beta" && ! -L "$AGENTS/skills/beta" ]] ||
  fail "the contended run still installed beta"

# ── Part B: a NON-contention acquisition failure is a required failure ────────
# An unwritable ~/.agents makes `exec 9>>` fail (the lock file cannot be
# created): a hard acquisition failure must record a required failure and exit
# 1 (never a silent exit 0).
rm -f "$LOCKFILE" # ensure the lock file must be CREATED (so the open fails)
chmod 500 "$AGENTS"
set +e
out_hard="$(bash "$SCRIPT" --install-only 2>&1)"
rc_hard=$?
set -e
chmod 700 "$AGENTS"

[[ $rc_hard -eq 1 ]] ||
  fail "a hard lock-acquisition failure did not exit 1 (got $rc_hard): $out_hard"
grep -qi 'REQUIRED-FAILURE' <<<"$out_hard" ||
  fail "a hard lock-acquisition failure recorded no required failure: $out_hard"

echo "update-skills-lock-contention: OK"
