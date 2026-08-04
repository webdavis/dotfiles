#!/usr/bin/env bash
# update-skills-install-retry-marker.sh: an install-only run that did NOT do the
# work must stay retryable across applies, and the three outcomes must stay
# distinguishable.
#
# The updater exits 75 (EX_TEMPFAIL) when it attempted nothing because another
# run held the serialize lock, 1 on a required-phase failure, and 0 on success.
# The run_onchange_after_64 wrapper reads those: 75 and 1 both PRESERVE/CREATE
# its retry marker (so the rendered content changes and run_onchange re-fires on
# the next apply) and neither exits non-zero, which would abort the whole apply;
# 0 clears the marker.
#
# The original defect this pins: __gen_install_only_attempt returned 0 when it
# skipped the work, so the wrapper treated an uninstalled roster addition as
# success, cleared its marker, and chezmoi had already consumed the
# run_onchange trigger, so the next apply never retried.
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"
TMPL="$REPO_ROOT/.chezmoiscripts/run_onchange_after_64-update-skills-first-install.sh.tmpl"
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

# ── Part A: the updater's exit codes ─────────────────────────────────────────
HOME="$tmp/uhome"
export HOME
export UPDATE_SKILLS_GMV="$GMV_BIN"
mkdir -p "$HOME/.agents/skills"
AGENTS="$HOME/.agents"
CURRENT="$AGENTS/.skills-current"
LOCK="$AGENTS/custom-skill-lock.json"

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
NPX_LOG="$tmp/npx.log"
: >"$NPX_LOG"
cat >"$stub/npx" <<EOF
#!/usr/bin/env bash
set -euo pipefail
printf 'npx %s\n' "\$*" >>"$NPX_LOG"
prev=""; skills=()
for a in "\$@"; do [[ \$prev == --skill ]] && skills+=("\$a"); prev="\$a"; done
for s in "\${skills[@]}"; do
  mkdir -p "\$HOME/.agents/skills/\$s"
  printf -- '---\nname: %s\n---\n# lane\n' "\$s" >"\$HOME/.agents/skills/\$s/SKILL.md"
done
EOF
chmod +x "$stub/npx"
export PATH="$stub:$PATH"

# Establish a live generation with alpha.
write_lock alpha
mkdir -p "$AGENTS/skills/alpha"
printf -- '---\nname: alpha\n---\n# seed\n' >"$AGENTS/skills/alpha/SKILL.md"
printf '{"skills":{"alpha":{}}}\n' >"$AGENTS/.skill-lock.json"
UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" >/dev/null 2>&1 || fail "part A setup full run failed"
id_setup="$(jq -r '.id' "$CURRENT/generation.json")"

# beta absent + another run holding the serialize lock -> exit 75, nothing done.
write_lock alpha beta
lockfile="$AGENTS/.update-skills.lock"
: >"$lockfile"
holder_held="$tmp/lock-held"
holder_release="$tmp/lock-release"
rm -f "$holder_held"
: >"$holder_release"
(
  exec 9>>"$lockfile"
  /usr/bin/lockf -s -t 0 9 2>/dev/null || exit 1
  : >"$holder_held"
  while [[ -e $holder_release ]]; do sleep 0.05; done
) &
holder_pid=$!
for ((i = 0; i < 100; i++)); do
  [[ -e $holder_held ]] && break
  sleep 0.05
done
if [[ ! -e $holder_held ]]; then
  rm -f "$holder_release"
  wait "$holder_pid" 2>/dev/null || true
  fail "could not stage a held serialize lock; the contention case did not run"
fi
set +e
out_defer="$(bash "$SCRIPT" --install-only 2>&1)"
rc_defer=$?
set -e
rm -f "$holder_release"
wait "$holder_pid" 2>/dev/null || true
[[ $rc_defer -eq 75 ]] ||
  fail "install-only under lock contention did not exit the distinct code 75 (got $rc_defer): $out_defer"
grep -qi 'deferring' <<<"$out_defer" ||
  fail "the deferred run did not log the deferral: $out_defer"
[[ "$(jq -r '.id' "$CURRENT/generation.json")" == "$id_setup" ]] ||
  fail "the deferred run still exchanged the generation"
[[ ! -e "$AGENTS/skills/beta" && ! -L "$AGENTS/skills/beta" ]] ||
  fail "the deferred run still installed beta"

# A hard required failure must NOT masquerade as a defer: exit 1, not 75.
# (npx add fails -> a required-phase failure -> exit 1.)
cat >"$stub/npx" <<'EOF'
#!/usr/bin/env bash
echo "npx boom" >&2
exit 1
EOF
chmod +x "$stub/npx"
set +e
out_fail="$(bash "$SCRIPT" --install-only 2>&1)"
rc_fail=$?
set -e
[[ $rc_fail -eq 1 ]] ||
  fail "a hard install-only failure did not exit 1 (got $rc_fail): $out_fail"

# ── Part B: the wrapper's marker handling per exit code ──────────────────────
sbox="$tmp/home"
mkdir -p "$sbox/.local/bin"
MARKER="$sbox/.local/state/skills/first-install-pending"
cat >"$sbox/.local/bin/update-skills.sh" <<'EOF'
#!/usr/bin/env bash
exit "${FAKE_UPDATER_RC:-0}"
EOF
chmod +x "$sbox/.local/bin/update-skills.sh"
render() { HOME="$sbox" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$TMPL" >"$1"; }
runner="$tmp/runner.sh"
render "$runner" || fail "rendering the first-install wrapper failed"

# deferred (75): marker created, wrapper exits 0 (apply not aborted).
rm -rf "$sbox/.local/state"
FAKE_UPDATER_RC=75 HOME="$sbox" bash "$runner" ||
  fail "the wrapper aborted the apply on a deferred (75) install-only (must exit 0)"
[[ -f $MARKER ]] ||
  fail "a deferred install-only left no retry marker (next apply will not re-fire)"

# a second deferral keeps the marker present (still retryable).
FAKE_UPDATER_RC=75 HOME="$sbox" bash "$runner" ||
  fail "the wrapper aborted the apply on the second deferral"
[[ -f $MARKER ]] || fail "the retry marker vanished after a second deferral"

# the install finally completes: marker removed.
FAKE_UPDATER_RC=0 HOME="$sbox" bash "$runner" ||
  fail "the wrapper exited non-zero on the completing run"
[[ ! -e $MARKER ]] || fail "the retry marker was not cleared once the install completed"

# a hard failure (1) still bumps a marker and exits 0 (unchanged contract).
rm -rf "$sbox/.local/state"
FAKE_UPDATER_RC=1 HOME="$sbox" bash "$runner" ||
  fail "the wrapper aborted the apply on a hard install failure (must exit 0)"
[[ -f $MARKER ]] || fail "a hard failure left no retry marker"

echo "update-skills-install-retry-marker: OK"
