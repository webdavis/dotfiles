#!/usr/bin/env bash
# update-skills-first-install.sh, a fresh machine must reproduce the full skills
# store at apply time, not wait for the weekly Monday LaunchAgent (RunAtLoad is
# false, so the agent's first chance is Monday, or never, given the idle-gate).
#
# Asserts:
#   1. the run_onchange chezmoiscript invokes the DEPLOYED updater with
#      --install-only;
#   2. its rendered content re-hashes when the lock changes, so run_onchange
#      re-fires on any roster edit;
#   3. --install-only against an empty HOME installs ABSENT skills and skips
#      present ones, and it runs even while an agent session is live, because
#      --install-only only ADDS absent skills (never swaps a folder) and is
#      therefore exempt from the idle-gate.
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_TMPL="$REPO_ROOT/.chezmoiscripts/run_onchange_after_64-update-skills-first-install.sh.tmpl"
UPDATER="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# ── 1. the chezmoiscript renders and invokes the deployed updater --install-only.
[[ -f $SCRIPT_TMPL ]] || fail "first-install chezmoiscript not found: $SCRIPT_TMPL"
# --source pins the render to this checkout (hermetic; mirrors treefmt.nix).
rendered="$(CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$SCRIPT_TMPL")" ||
  fail "chezmoi execute-template failed on the first-install script"
printf '%s\n' "$rendered" | grep -q 'update-skills\.sh' ||
  fail "the first-install script does not reference the deployed updater update-skills.sh: $rendered"
printf '%s\n' "$rendered" | grep -q -- '--install-only' ||
  fail "the first-install script does not pass --install-only to the updater: $rendered"

# ── 2. content re-hashes when the lock changes (so run_onchange re-fires). Build
#      a minimal fixture source with the script + updater + a lock, render twice
#      with different locks, and require the rendered bytes to differ.
fixture_src="$(mktemp -d)"
mkdir -p "$fixture_src/.chezmoiscripts" "$fixture_src/dot_agents" "$fixture_src/dot_local/bin"
tmpl_name="$(basename "$SCRIPT_TMPL")"
cp "$SCRIPT_TMPL" "$fixture_src/.chezmoiscripts/$tmpl_name"
cp "$UPDATER" "$fixture_src/dot_local/bin/executable_update-skills.sh"
render_fixture() {
  CI=1 chezmoi --source "$fixture_src" execute-template --no-tty <"$fixture_src/.chezmoiscripts/$tmpl_name"
}
printf '{"version":2,"tiers":{"alpha":"core"}}\n' >"$fixture_src/dot_agents/custom-skill-lock.json"
render_a="$(render_fixture)" || fail "fixture render A failed"
printf '{"version":2,"tiers":{"bravo":"core"}}\n' >"$fixture_src/dot_agents/custom-skill-lock.json"
render_b="$(render_fixture)" || fail "fixture render B failed"
rm -rf "$fixture_src"
[[ $render_a != "$render_b" ]] ||
  fail "the rendered first-install script did not change when the lock changed (run_onchange would not re-fire)"

# ── 3. empty HOME: --install-only installs absent, skips present, under a live
#      session (the idle-gate must not block a swap-free install pass).
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
HOME="$tmp/home"
export HOME
mkdir -p "$HOME/.agents/skills/already"
printf -- '---\nname: already\ndescription: present\n---\n' >"$HOME/.agents/skills/already/SKILL.md"
touch "$HOME/.agents/skills/already/local.marker"
cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{
  "version": 2,
  "tiers": {"freshskill": "core", "already": "core"},
  "hermesProfiles": {"freshskill": [], "already": []},
  "hermesRegistry": {},
  "npxTracked": {
    "freshskill": {"repo": "fixture/freshskill"},
    "already": {"repo": "fixture/already"}
  },
  "clawhubTracked": {},
  "forks": {}
}
EOF

stub="$tmp/stub"
mkdir -p "$stub"
NPX_LOG="$tmp/npx.log"
cat >"$stub/npx" <<EOF
#!/usr/bin/env bash
set -euo pipefail
printf 'npx %s\n' "\$*" >>"$NPX_LOG"
mode=""
skill=""
prev=""
for arg in "\$@"; do
  case "\$arg" in add) mode="add" ;; esac
  [[ \$prev == "--skill" ]] && skill="\$arg"
  prev="\$arg"
done
if [[ \$mode == "add" && -n \$skill ]]; then
  mkdir -p "\$HOME/.agents/skills/\$skill"
  printf -- '---\nname: %s\ndescription: fixture\n---\n' "\$skill" >"\$HOME/.agents/skills/\$skill/SKILL.md"
fi
echo "stub npx"
EOF
# ps stub: an ACTIVE Claude session. Proves --install-only runs anyway (exempt).
cat >"$stub/ps" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' '/opt/homebrew/bin/claude --remote-control'
EOF
# alerter/date stubs: keep any gate path (pre-fix) from firing a real
# notification and pin the hour off the last slot.
cat >"$stub/alerter" <<'EOF'
#!/usr/bin/env bash
:
EOF
cat >"$stub/date" <<'EOF'
#!/usr/bin/env bash
case "$1" in
  +%H) echo 04 ;;
  +%G-%V) echo 2026-28 ;;
  *) exec /bin/date "$@" ;;
esac
EOF
chmod +x "$stub"/*
export PATH="$stub:$PATH"

# No UPDATE_SKILLS_FORCE: the swap-free install pass must run despite the active
# claude stub (a fresh machine is often provisioned from a live session).
out="$(bash "$UPDATER" --install-only 2>&1)" ||
  fail "--install-only exited non-zero under an active session: $out"
[[ -f "$HOME/.agents/skills/freshskill/SKILL.md" ]] ||
  fail "absent skill freshskill was not installed by the first-install pass (idle-gate blocked --install-only?): $out"
[[ -f "$HOME/.agents/skills/already/local.marker" ]] ||
  fail "present skill 'already' was reinstalled by the install pass (marker lost)"
grep -q 'add fixture/freshskill' "$NPX_LOG" ||
  fail "npx add was not invoked for the absent skill: $(cat "$NPX_LOG" 2>/dev/null)"
if grep -q 'add fixture/already' "$NPX_LOG"; then
  fail "npx add was invoked for an already-present skill (install pass must skip present skills)"
fi

echo "update-skills-first-install: OK"
