#!/usr/bin/env bash
# live-reconcile.sh (test): the tracked live reconciliation tool.
#
# scripts/live-reconcile.sh is what D1 gate 3 runs, --dry-run then live, to
# prove a from-scratch machine converges to what the committed lock declares.
# Its contract (SP2 plan, binding checklist item 18) is a --dry-run flag,
# idempotence, and tests.
#
# What it converges: the skills fan-out the lock declares. The Claude Code
# symlinks, the hermes profile symlinks (both directions: plant what is
# declared, prune undeclared links into the store), the Codex on-demand policy
# overlay, and the superpowers routing re-assert. What it does NOT do is
# install skills or rewrite the deployed lock: those belong to the update-skills
# lane and to chezmoi, so it reports them as divergence it will not fix.
#
# Runs entirely inside a sandbox $HOME. Nothing reads or writes the live store.
#
# Cases:
#   A. --dry-run reports every planned action and writes NOTHING
#   B. the live run converges: plants, prunes, overlays, re-asserts routing
#   C. idempotence: the second live run has nothing left to do
#   D. a roster skill absent from the store is unfixable divergence (both modes)
#   E. a hub-owned real directory in a profile is never pruned
#   F. a missing overlay on a symlinked store entry is reported, never written
#      through the link
#   G. a deployed lock that differs from the committed one is unfixable
#   H. an unknown argument is usage to stderr and a non-zero exit
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TOOL="$REPO_ROOT/scripts/live-reconcile.sh"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failures=0
report() {
  local status="$1" msg="$2"
  if [[ $status == ok ]]; then
    printf '  ok   %s\n' "$msg"
  else
    printf '  FAIL %s\n' "$msg"
    failures=$((failures + 1))
  fi
}

[[ -x $TOOL ]] || {
  printf 'FAIL: missing or non-executable tool: %s\n' "$TOOL" >&2
  exit 1
}
command -v jq >/dev/null 2>&1 || {
  printf 'SKIP: jq is not on PATH; the reconcile tool cannot read the lock\n'
  exit 0
}

out_file="$work/stdout"
err_file="$work/stderr"
export ROUTING_LOG="$work/routing.log"
: >"$ROUTING_LOG"

# build_home <home> : a sandbox with a diverged skills fan-out.
#
#   store:  alpha (real dir), beta (real dir, no Codex overlay),
#           gamma (symlink to app-owned content that already carries one)
#   wanted: alpha -> hermes default, beta -> hermes profile "concerned",
#           gamma -> no hermes profile at all
#   live:   no Claude links, no hermes links, one undeclared store link to
#           prune, and one hub-owned real dir that must survive
build_home() {
  local home="$1" repo="$1/workspaces/Ivy/webdavis/dotfiles" skill
  mkdir -p "$repo/.git" "$repo/dot_agents" "$home/.agents/skills" \
    "$home/.claude/skills" "$home/.hermes/skills" \
    "$home/.hermes/profiles/concerned/skills" "$home/.local/bin" \
    "$home/app-owned/gamma/agents"
  {
    printf '{\n'
    printf '  "tiers": {"alpha": "core", "beta": "on-demand", "gamma": "on-demand"},\n'
    printf '  "hermesProfiles": {"alpha": ["default"], "beta": ["concerned"], "gamma": []},\n'
    printf '  "superpowersRouting": {"writing-plans": "hermes-writing-plans"}\n'
    printf '}\n'
  } >"$repo/dot_agents/custom-skill-lock.json"
  cp "$repo/dot_agents/custom-skill-lock.json" "$home/.agents/custom-skill-lock.json"

  for skill in alpha beta; do
    mkdir -p "$home/.agents/skills/$skill"
    printf -- '---\nname: %s\n---\n' "$skill" >"$home/.agents/skills/$skill/SKILL.md"
  done
  printf 'policy:\n  allow_implicit_invocation: false\n' >"$home/app-owned/gamma/agents/openai.yaml"
  printf -- '---\nname: gamma\n---\n' >"$home/app-owned/gamma/SKILL.md"
  ln -s "$home/app-owned/gamma" "$home/.agents/skills/gamma"

  # an undeclared link into the store, and a hub-owned real dir beside it
  ln -s '../../.agents/skills/beta' "$home/.hermes/skills/beta"
  mkdir -p "$home/.hermes/skills/hub-owned"
  printf -- '---\nname: hub-owned\n---\n' >"$home/.hermes/skills/hub-owned/SKILL.md"

  # the routing re-assert the tool delegates to
  # shellcheck disable=SC2016  # literal stub body; $vars resolve when it runs
  {
    printf '#!/usr/bin/env bash\n'
    printf 'printf "%%s\\n" "$*" >>"$ROUTING_LOG"\n'
    printf '[[ -n "${FAIL_ROUTING:-}" ]] && exit 1\n'
    printf 'exit 0\n'
  } >"$home/.local/bin/assert-hermes-superpowers-routing.sh"
  chmod +x "$home/.local/bin/assert-hermes-superpowers-routing.sh"
}

# snapshot <home> : every path under the sandbox with its type and link target,
# so a dry run can be proven to have written nothing.
snapshot() {
  local path
  while IFS= read -r path; do
    if [[ -L $path ]]; then
      printf '%s\tlink\t%s\n' "$path" "$(readlink "$path")"
    elif [[ -d $path ]]; then
      printf '%s\tdir\n' "$path"
    else
      printf '%s\tfile\t%s\n' "$path" "$(cksum <"$path" | cut -d' ' -f1)"
    fi
  done < <(find "$1" -mindepth 1 | sort)
}

run_tool() {
  local home="$1"
  shift
  RC=0
  HOME="$home" env "$@" >"$out_file" 2>"$err_file" || RC=$?
}
tool() {
  local home="$1"
  shift
  run_tool "$home" "$TOOL" "$@"
}

printf 'live-reconcile cases:\n'

# ── Case A: --dry-run changes nothing ──────────────────────────────────────
hA="$work/homeA"
build_home "$hA"
snapshot "$hA" >"$work/before"
tool "$hA" --dry-run
snapshot "$hA" >"$work/after"
if [[ $RC -eq 0 ]]; then
  report ok "A: a dry run over fixable divergence exits 0"
else
  report bad "A: dry run exited $RC (err: $(cat "$err_file"))"
fi
if diff -u "$work/before" "$work/after" >"$work/dryrun.diff"; then
  report ok "A: the dry run wrote nothing"
else
  report bad "A: the dry run mutated the sandbox:"$'\n'"$(cat "$work/dryrun.diff")"
fi
for want in '.claude/skills/alpha' '.hermes/skills/alpha' \
  '.hermes/profiles/concerned/skills/beta' '.hermes/skills/beta' \
  'agents/openai.yaml'; do
  if grep -q -- "$want" "$out_file"; then
    report ok "A: the plan names $want"
  else
    report bad "A: the plan never mentions $want (out: $(cat "$out_file"))"
  fi
done
if grep -qi 'dry run' "$out_file"; then
  report ok "A: the output says it is a dry run"
else
  report bad "A: the dry run does not announce itself"
fi
if [[ "$(sed -n '1p' "$ROUTING_LOG")" == "--check" ]]; then
  report ok "A: the routing re-assert is probed with --check, never applied"
else
  report bad "A: routing was not probed read-only (log: $(tr '\n' '|' <"$ROUTING_LOG"))"
fi

# ── Case B: the live run converges ─────────────────────────────────────────
: >"$ROUTING_LOG"
tool "$hA"
if [[ $RC -eq 0 ]]; then
  report ok "B: the live run exits 0"
else
  report bad "B: the live run exited $RC (err: $(cat "$err_file"))"
fi
if [[ "$(readlink "$hA/.claude/skills/alpha" || true)" == '../../.agents/skills/alpha' ]]; then
  report ok "B: the Claude symlink is planted with the declared relative target"
else
  report bad "B: wrong Claude symlink (got: $(readlink "$hA/.claude/skills/alpha" 2>/dev/null || printf 'absent'))"
fi
if [[ "$(readlink "$hA/.hermes/skills/alpha" || true)" == '../../.agents/skills/alpha' ]]; then
  report ok "B: the default-profile hermes symlink is planted"
else
  report bad "B: wrong default-profile hermes symlink"
fi
if [[ "$(readlink "$hA/.hermes/profiles/concerned/skills/beta" || true)" == '../../../../.agents/skills/beta' ]]; then
  report ok "B: the specialist-profile hermes symlink uses the deeper relative target"
else
  report bad "B: wrong specialist-profile hermes symlink (got: $(readlink "$hA/.hermes/profiles/concerned/skills/beta" 2>/dev/null || printf 'absent'))"
fi
if [[ ! -e "$hA/.hermes/skills/beta" ]] && [[ ! -L "$hA/.hermes/skills/beta" ]]; then
  report ok "B: the undeclared store link is pruned"
else
  report bad "B: an undeclared store link survived the live run"
fi
if grep -q 'allow_implicit_invocation: false' "$hA/.agents/skills/beta/agents/openai.yaml" 2>/dev/null; then
  report ok "B: the on-demand Codex overlay is written for a real store dir"
else
  report bad "B: no Codex overlay for the on-demand skill"
fi
if [[ "$(sed -n '1p' "$ROUTING_LOG")" == "" ]] && [[ -s $ROUTING_LOG ]]; then
  report ok "B: the routing re-assert is applied in the live run"
else
  report bad "B: routing was not applied (log: $(tr '\n' '|' <"$ROUTING_LOG"))"
fi

# ── Case E: a hub-owned real dir is never pruned ───────────────────────────
if [[ -f "$hA/.hermes/skills/hub-owned/SKILL.md" ]]; then
  report ok "E: a hub-owned real directory survives the prune"
else
  report bad "E: the prune deleted hub-owned content"
fi

# ── Case C: idempotence ────────────────────────────────────────────────────
snapshot "$hA" >"$work/before2"
tool "$hA"
snapshot "$hA" >"$work/after2"
if [[ $RC -eq 0 ]] && diff -q "$work/before2" "$work/after2" >/dev/null; then
  report ok "C: a second live run changes nothing"
else
  report bad "C: the tool is not idempotent (rc=$RC)"
fi
if grep -qiE 'converged|0 action|nothing to' "$out_file"; then
  report ok "C: the converged run says so"
else
  report bad "C: no converged report (out: $(cat "$out_file"))"
fi

# ── Case D: a roster skill missing from the store ──────────────────────────
hD="$work/homeD"
build_home "$hD"
rm -rf "$hD/.agents/skills/beta"
tool "$hD" --dry-run
if [[ $RC -eq 1 ]] && grep -q 'beta' "$err_file"; then
  report ok "D: a missing store entry is unfixable divergence in a dry run"
else
  report bad "D: a missing store entry passed the dry run (rc=$RC, err: $(cat "$err_file"))"
fi
tool "$hD"
if [[ $RC -eq 1 ]]; then
  report ok "D: the live run refuses too, rather than installing"
else
  report bad "D: the live run did not refuse (rc=$RC)"
fi

# ── Case F: never write an overlay through a store symlink ─────────────────
hF="$work/homeF"
build_home "$hF"
rm -f "$hF/app-owned/gamma/agents/openai.yaml"
snapshot "$hF" >"$work/beforeF"
tool "$hF"
snapshot "$hF" >"$work/afterF"
if [[ $RC -eq 1 ]] && grep -q 'gamma' "$err_file"; then
  report ok "F: a missing overlay behind a store symlink is reported, not fixed"
else
  report bad "F: an app-owned store symlink was not reported (rc=$RC, err: $(cat "$err_file"))"
fi
if [[ ! -e "$hF/app-owned/gamma/agents/openai.yaml" ]]; then
  report ok "F: nothing was written through the store symlink"
else
  report bad "F: the tool wrote through an app-owned store symlink"
fi

# ── Case G: the deployed lock diverges from the committed one ──────────────
hG="$work/homeG"
build_home "$hG"
printf '{"tiers": {}}\n' >"$hG/.agents/custom-skill-lock.json"
tool "$hG" --dry-run
if [[ $RC -eq 1 ]] && grep -qi 'lock' "$err_file"; then
  report ok "G: a deployed lock that differs from the committed one is unfixable"
else
  report bad "G: a diverged deployed lock passed (rc=$RC, err: $(cat "$err_file"))"
fi

# ── Case H: argument handling ──────────────────────────────────────────────
hH="$work/homeH"
build_home "$hH"
tool "$hH" --nope
if [[ $RC -ne 0 ]] && grep -qi 'usage' "$err_file"; then
  report ok "H: an unknown argument is usage to stderr and a non-zero exit"
else
  report bad "H: an unknown argument was accepted (rc=$RC)"
fi
run_tool "$work/empty-home" "$TOOL" --dry-run
if [[ $RC -ne 0 ]] && grep -q 'workspaces/Ivy/webdavis/dotfiles' "$err_file"; then
  report ok "H: the repo handle is validated before anything else"
else
  report bad "H: ran without a repo handle (rc=$RC, err: $(cat "$err_file"))"
fi

# ── Case I: an unreadable lock must never drive pruning ────────────────────
# A malformed hermesProfiles value does not announce itself: jq's `.[][]?` and
# `index($p)` both answer a string quietly and exit 0, so the wanted set comes
# back missing entries and the prune reads them as undeclared. (Checked live:
# both expressions return rc=0 against a scalar value.) The lock's shape is
# validated before anything is deleted.
hI="$work/homeI"
build_home "$hI"
lockI="$hI/workspaces/Ivy/webdavis/dotfiles/dot_agents/custom-skill-lock.json"
{
  printf '{\n'
  printf '  "tiers": {"alpha": "core", "beta": "on-demand"},\n'
  printf '  "hermesProfiles": {"alpha": ["default"], "beta": "concerned"},\n'
  printf '  "superpowersRouting": {}\n'
  printf '}\n'
} >"$lockI"
cp "$lockI" "$hI/.agents/custom-skill-lock.json"
ln -s '../../.agents/skills/alpha' "$hI/.hermes/skills/alpha"
snapshot "$hI" >"$work/beforeI"
tool "$hI"
snapshot "$hI" >"$work/afterI"
if [[ $RC -ne 0 ]] && grep -qi 'malformed' "$err_file"; then
  report ok "I: a lock whose shape cannot be trusted refuses"
else
  report bad "I: a malformed lock drove a live run (rc=$RC, err: $(cat "$err_file"))"
fi
if diff -q "$work/beforeI" "$work/afterI" >/dev/null; then
  report ok "I: nothing was pruned on the partial read"
else
  report bad "I: a partial read deleted links:"$'\n'"$(diff -u "$work/beforeI" "$work/afterI")"
fi

# ── Case J: a profile that lost its last assignment is still pruned ────────
# Walking only the profiles the lock still names means a de-mapped profile
# disappears from the walk, and its stale store links survive forever while the
# run reports convergence.
hJ="$work/homeJ"
build_home "$hJ"
lockJ="$hJ/workspaces/Ivy/webdavis/dotfiles/dot_agents/custom-skill-lock.json"
{
  printf '{\n'
  printf '  "tiers": {"alpha": "core", "beta": "on-demand", "gamma": "on-demand"},\n'
  printf '  "hermesProfiles": {"alpha": [], "beta": ["concerned"], "gamma": []},\n'
  printf '  "superpowersRouting": {}\n'
  printf '}\n'
} >"$lockJ"
cp "$lockJ" "$hJ/.agents/custom-skill-lock.json"
# the default profile's last assignment is gone, but its old link is still there
ln -s '../../.agents/skills/alpha' "$hJ/.hermes/skills/alpha"
tool "$hJ" --dry-run
if grep -q '.hermes/skills/alpha' "$out_file"; then
  report ok "J: the dry run still plans the prune for a de-mapped profile"
else
  report bad "J: a de-mapped profile fell out of the walk (out: $(cat "$out_file"))"
fi
tool "$hJ"
if [[ ! -L "$hJ/.hermes/skills/alpha" ]]; then
  report ok "J: the stale link in the de-mapped profile is pruned"
else
  report bad "J: a stale store link survived because its profile left the lock"
fi
if [[ -f "$hJ/.hermes/skills/hub-owned/SKILL.md" ]]; then
  report ok "J: hub-owned content in that profile is still untouched"
else
  report bad "J: the de-mapped-profile prune deleted hub-owned content"
fi

if [[ $failures -gt 0 ]]; then
  printf 'live-reconcile: %d assertion(s) FAILED\n' "$failures" >&2
  exit 1
fi
printf 'live-reconcile: OK (dry run writes nothing, live converges, idempotent, unfixable divergence refuses)\n'
