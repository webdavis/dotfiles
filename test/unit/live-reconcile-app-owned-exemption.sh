#!/usr/bin/env bash
# live-reconcile-app-owned-exemption.sh: an on-demand skill whose store entry
# links at another application's content is a documented exemption, not a
# divergence to resolve.
#
# WHY THIS EXISTS. Measured on dresden 2026-08-05, preflighting gate 3 before
# the operator's apply: live-reconcile reported cua-driver as a divergence "to
# resolve before the cutover proceeds" and exited 1, and gate 3 dies on that
# exit. The resolution it asked for is one docs/runbooks/agent-skills-store.md
# forbids: cua-driver's store entry is a symlink into ~/.cua-driver, and writing
# an overlay through it would modify content this repository does not own. The
# condition is permanent, so gate 3 could never have passed.
#
# WHAT THE FIX MUST NOT DO is exempt every symlink. A store entry that links
# INSIDE ~/.agents belongs to the skills updater's generation exchange, and that
# one is a real handoff gap worth reporting. The two cases below are the whole
# point: the discriminator is where the link goes, not that a link exists.
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TOOL="$REPO_ROOT/scripts/live-reconcile.sh"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -x $TOOL ]] || fail "$TOOL is missing or not executable"

# A sandbox HOME with the two link shapes plus a real directory, so all three
# arms of the branch are exercised in one run.
home="$work/home"
store="$home/.agents/skills"
mkdir -p "$store" "$home/.agents/.skills-current/skills/updater-owned" \
  "$home/.otherapp/skills/app-owned" "$store/plain-dir"

# app-owned: links OUT of ~/.agents  -> documented exemption
ln -s "$home/.otherapp/skills/app-owned" "$store/app-owned"
# updater-owned: links INSIDE ~/.agents -> still a blocker
ln -s "$home/.agents/.skills-current/skills/updater-owned" "$store/updater-owned"
# plain-dir gets a real overlay written, which proves the happy path still works

# The tool derives its repo handle from $HOME with no env seam, so the sandbox
# needs the checkout at the path it expects. Building it here rather than adding
# an override to production keeps the real path derivation under test.
sandbox_repo="$home/workspaces/Ivy/webdavis/dotfiles"
mkdir -p "$sandbox_repo/dot_agents"
git -C "$sandbox_repo" init -q

# A lock declaring all three on-demand, with the tables the tool reads present
# and empty so nothing else in the run has an opinion.
cat >"$home/.agents/custom-skill-lock.json" <<'JSON'
{
  "version": 1,
  "tiers": {
    "app-owned": "on-demand",
    "updater-owned": "on-demand",
    "plain-dir": "on-demand"
  },
  "npxTracked": {},
  "clawhubTracked": {},
  "forks": {},
  "hermesProfiles": {},
  "hermesRegistry": {},
  "claudeDelivery": {},
  "superpowersRouting": {}
}
JSON

# The committed lock must match the deployed one, or that unrelated divergence
# fires and this test would be reading a run that stopped for another reason.
cp "$home/.agents/custom-skill-lock.json" "$sandbox_repo/dot_agents/custom-skill-lock.json"

out="$work/out.txt"
HOME="$home" "$TOOL" --dry-run >"$out" 2>&1 || true

# 1. The app-owned link is exempt: named as such, and NOT counted as a
#    divergence. Both halves matter, because a tool that prints an exemption
#    while still counting it keeps gate 3 unpassable.
grep -q "exempt: on-demand skill app-owned" "$out" ||
  fail "an app-owned store link was not reported as an exemption:"$'\n'"$(cat "$out")"
grep -E 'DIVERGENCE.*app-owned' "$out" >/dev/null &&
  fail "an app-owned store link is still counted as a divergence, so gate 3 stays unpassable"

# 2. The updater-owned link IS still a blocker. Without this the fix would
#    exempt every symlink and hide a real handoff gap.
grep -E "DIVERGENCE.*updater-owned" "$out" >/dev/null ||
  fail "a link inside the agents store stopped being reported; the exemption over-reached to every symlink"

# 3. A real directory still gets its overlay planned, so cases 1 and 2 are not
#    passing because the whole overlay pass stopped running.
grep -q 'plain-dir' "$out" ||
  fail "the overlay pass no longer reaches a plain on-demand directory"

# 4. The discriminator itself is pinned. An edit that drops the target check
#    reintroduces the unpassable gate, and cases 1 through 3 could still pass on
#    a fixture whose links all happen to point the same way.
grep -q 'link_target' "$TOOL" ||
  fail "live-reconcile no longer inspects where a store link points; the exemption cannot be distinguishing anything"

printf 'live-reconcile-app-owned-exemption: OK (app-owned link exempt and uncounted, updater-owned link still blocks, plain dir still planned, discriminator pinned)\n'
