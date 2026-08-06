#!/usr/bin/env bash
# live-reconcile.sh: converge the live skills fan-out to what the committed
# lock declares.
#
# D1 gate 3 runs this by absolute path, --dry-run first and then live, to prove
# a from-scratch machine converges identically (SP2 plan, Phase E
# fix/live-reconcile-from-scratch and binding checklist item 18). It exists
# because the PR #36 convergence was performed ad hoc on the live machine and
# the draft script was never tracked.
#
# It owns exactly the DECLARATIVE fan-out, which is the part that drifts
# additively:
#
#   - the Claude Code skill symlinks into the store
#   - the hermes profile symlinks, both directions: plant what hermesProfiles
#     declares, prune undeclared links INTO the store (a hub-owned real
#     directory is never touched)
#   - the Codex on-demand policy overlay on real store directories
#   - the superpowers routing re-assert, delegated to its own script
#
# It does NOT install skills and does not rewrite the deployed lock: those
# belong to update-skills.sh and to chezmoi. Divergence it will not fix is
# reported and exits non-zero in both modes, so gate 3's dry run stops the
# cutover before the live run touches anything.
set -euo pipefail

repo="$HOME/workspaces/Ivy/webdavis/dotfiles"
STORE="$HOME/.agents/skills"
COMMITTED_LOCK="$repo/dot_agents/custom-skill-lock.json"
DEPLOYED_LOCK="$HOME/.agents/custom-skill-lock.json"
ROUTING_SCRIPT="$HOME/.local/libexec/unattended-upgrades/agent-skills/assert-hermes-superpowers-routing.sh"
CODEX_POLICY='policy:
  allow_implicit_invocation: false'

DRY_RUN=0
ACTIONS=0
BLOCKERS=0

usage() {
  printf 'usage: live-reconcile.sh [--dry-run]\n' >&2
  printf '\n' >&2
  printf '  Converges the live skills fan-out to the committed lock.\n' >&2
  printf '  --dry-run  print the plan and change nothing\n' >&2
  exit 2
}

for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY_RUN=1 ;;
    *) usage ;;
  esac
done

say() { printf '%s\n' "$*"; }

# A divergence this tool fixes (or would fix, in a dry run).
plan() {
  ACTIONS=$((ACTIONS + 1))
  if [[ $DRY_RUN -eq 1 ]]; then
    printf '  would %s\n' "$*"
  else
    printf '  %s\n' "$*"
  fi
}

# A divergence this tool refuses to fix, because another lane owns it.
blocker() {
  BLOCKERS=$((BLOCKERS + 1))
  printf 'DIVERGENCE (not fixed here): %s\n' "$*" >&2
}

die() {
  printf 'REFUSED: %s\n' "$*" >&2
  exit 1
}

[[ -d "$repo/.git" ]] ||
  die "the repo handle $repo is not a git checkout"
command -v jq >/dev/null 2>&1 || die "jq is required to read the skills lock"
[[ -f $COMMITTED_LOCK ]] || die "no committed lock at $COMMITTED_LOCK"

scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

# The lock decides what gets DELETED below, so its shape is validated before any
# of it is believed. A malformed value does not announce itself: jq's `.[][]?`
# and `index($p)` both answer a string quietly, exit 0, and hand back a wanted
# set that is simply missing entries, which the prune then reads as "undeclared"
# and removes. Refusing on shape is what stops a typo in the lock from deleting
# live links.
jq -e '
  (.tiers // {}) as $t | (.hermesProfiles // {}) as $h
  | ($t | type == "object") and ($h | type == "object")
    and ([$t[] | type] | all(. == "string"))
    and ([$h[] | type] | all(. == "array"))
    and ([$h[][] | type] | all(. == "string"))
' "$COMMITTED_LOCK" >/dev/null 2>&1 ||
  die "$COMMITTED_LOCK is malformed: tiers must map names to strings and hermesProfiles must map names to arrays of profile names. Nothing is pruned against a lock that cannot be read exactly"

if [[ $DRY_RUN -eq 1 ]]; then
  say "live-reconcile: DRY RUN, nothing will be written."
else
  say "live-reconcile: converging the live fan-out to $COMMITTED_LOCK"
fi

# ── the deployed lock must be the committed lock ───────────────────────────
# chezmoi owns that file; a difference means the apply did not land, and every
# check below would then be measured against the wrong desired state.
if [[ ! -f $DEPLOYED_LOCK ]]; then
  blocker "$DEPLOYED_LOCK is missing; chezmoi apply has not deployed the skills lock"
elif ! cmp -s "$COMMITTED_LOCK" "$DEPLOYED_LOCK"; then
  blocker "$DEPLOYED_LOCK differs from the committed lock; chezmoi apply owns that file"
fi

# ── every roster skill has a store entry ───────────────────────────────────
roster=()
roster_list="$scratch/roster"
jq -r '.tiers // {} | keys[]' "$COMMITTED_LOCK" >"$roster_list" ||
  die "could not read the tiers table out of $COMMITTED_LOCK"
while IFS= read -r skill; do
  [[ -n $skill ]] || continue
  roster+=("$skill")
done <"$roster_list"
[[ ${#roster[@]} -gt 0 ]] || die "the lock's tiers table is empty; there is no roster to converge"

present=()
for skill in "${roster[@]}"; do
  if [[ -d "$STORE/$skill" || -L "$STORE/$skill" ]]; then
    present+=("$skill")
  else
    blocker "roster skill '$skill' has no store entry at $STORE/$skill; installing is the update-skills lane's job"
  fi
done

# ── link_to <link> <target> : plant or repair one relative symlink ─────────
link_to() {
  local link="$1" target="$2" current=''
  [[ -L $link ]] && current="$(readlink "$link")"
  if [[ $current == "$target" ]]; then
    return 0
  fi
  if [[ -e $link && ! -L $link ]]; then
    blocker "$link exists and is not a symlink; it is owned by something else and is left alone"
    return 0
  fi
  plan "link $link -> $target"
  [[ $DRY_RUN -eq 1 ]] && return 0
  mkdir -p "$(dirname "$link")"
  rm -f "$link"
  ln -s "$target" "$link" || die "could not create $link"
}

# ── Claude Code fan-out: every roster skill present in the store ───────────
for skill in "${present[@]}"; do
  link_to "$HOME/.claude/skills/$skill" "../../.agents/skills/$skill"
done

# ── hermes fan-out, both directions ────────────────────────────────────────
# "default" is ~/.hermes/skills; any other profile is
# ~/.hermes/profiles/<name>/skills, one level deeper, so the relative target
# differs. Fan-out is driven ENTIRELY by hermesProfiles: an empty list means
# the store copy reaches no hermes profile.
# The profile universe is the lock's profiles UNION the profile directories that
# already exist. A profile only reachable through the lock disappears from the
# walk the moment its last skill is de-mapped, and its stale store links then
# survive forever while this reports convergence. update-skills.sh walks the
# same union for the same reason.
profiles=()
profile_list="$scratch/profiles"
{
  jq -r '.hermesProfiles // {} | [.[][]?] | unique | .[]' "$COMMITTED_LOCK" ||
    die "could not read hermesProfiles out of $COMMITTED_LOCK"
  [[ -d "$HOME/.hermes/skills" ]] && printf 'default\n'
  if [[ -d "$HOME/.hermes/profiles" ]]; then
    for dir in "$HOME/.hermes/profiles"/*/; do
      [[ -d "${dir}skills" ]] || continue
      dir="${dir%/}"
      printf '%s\n' "${dir##*/}"
    done
  fi
} | LC_ALL=C sort -u >"$profile_list"
while IFS= read -r profile; do
  [[ -n $profile ]] || continue
  profiles+=("$profile")
done <"$profile_list"

for profile in "${profiles[@]}"; do
  if [[ $profile == "default" ]]; then
    link_dir="$HOME/.hermes/skills"
    prefix="../../.agents/skills"
  else
    link_dir="$HOME/.hermes/profiles/$profile/skills"
    prefix="../../../../.agents/skills"
  fi

  # The wanted set decides what the prune below DELETES, so a partial read of it
  # is destructive: jq streams rows and then fails, the loop sees a short list,
  # and every skill after the failure looks undeclared. Materialize it through a
  # checked command so a failure refuses instead of pruning.
  wanted=()
  wanted_list="$scratch/wanted-$profile"
  jq -r --arg p "$profile" '.hermesProfiles // {} | to_entries[]
    | select((.value // []) | index($p) != null) | .key' "$COMMITTED_LOCK" >"$wanted_list" ||
    die "could not read the hermesProfiles mapping for profile '$profile'; nothing is pruned on a partial read"
  while IFS= read -r skill; do
    [[ -n $skill ]] || continue
    [[ -d "$STORE/$skill" || -L "$STORE/$skill" ]] || continue
    wanted+=("$skill")
    link_to "$link_dir/$skill" "$prefix/$skill"
  done <"$wanted_list"

  # Prune the other direction: a symlink INTO the store that the lock no longer
  # declares. Real directories are hub-owned installs and are never touched.
  [[ -d $link_dir ]] || continue
  for link in "$link_dir"/*; do
    [[ -L $link ]] || continue
    target="$(readlink "$link")"
    case "$target" in
      *".agents/skills/"*) ;;
      *) continue ;; # a link to something else entirely: not ours to prune
    esac
    name="${link##*/}"
    declared=0
    for skill in ${wanted[@]+"${wanted[@]}"}; do
      [[ $skill == "$name" ]] && declared=1 && break
    done
    [[ $declared -eq 1 ]] && continue
    plan "prune undeclared store link $link"
    [[ $DRY_RUN -eq 1 ]] && continue
    rm -f "$link" || die "could not prune $link"
  done
done

# ── Codex on-demand policy overlay ─────────────────────────────────────────
# On-demand skills carry `allow_implicit_invocation: false` so Codex never
# auto-invokes them. A store entry that is a SYMLINK points at content this
# repo does not own (the generation directory, or an app-owned pack), so a
# missing overlay there is reported and never written through the link.
while IFS= read -r skill; do
  [[ -n $skill ]] || continue
  [[ -d "$STORE/$skill" || -L "$STORE/$skill" ]] || continue
  overlay="$STORE/$skill/agents/openai.yaml"
  if grep -q 'allow_implicit_invocation: false' "$overlay" 2>/dev/null; then
    continue
  fi
  if [[ -L "$STORE/$skill" ]]; then
    # WHERE the link points decides whether this is a defect or the documented
    # design. A link OUT of ~/.agents points at content another application
    # owns, and docs/runbooks/agent-skills-store.md is explicit that such an
    # entry never gets an overlay, because writing through the link would
    # modify content this repository does not own. cua-driver is that case and
    # it is permanent: reporting it as a divergence "to resolve before the
    # cutover proceeds" asked for a resolution that must never happen, and
    # since the tool exits non-zero on any blocker, gate 3 could never pass.
    #
    # A link that stays INSIDE ~/.agents is the generation exchange's own, and
    # the updater owns it. That one keeps its blocker: another lane really is
    # responsible, and staying silent would hide a real handoff gap.
    link_target="$(cd "$(dirname "$STORE/$skill")" && readlink "$STORE/$skill")"
    case "$link_target" in
      /*) resolved="$link_target" ;;
      *) resolved="$STORE/$link_target" ;;
    esac
    case "$resolved" in
      "$HOME"/.agents/*)
        blocker "on-demand skill '$skill' has no Codex overlay and its store entry links inside the agents store; the skills updater owns that content"
        ;;
      *)
        say "  exempt: on-demand skill $skill is app-owned content at $resolved, so no Codex overlay is written through the link (documented in docs/runbooks/agent-skills-store.md)"
        ;;
    esac
    continue
  fi
  plan "assert the Codex overlay for on-demand skill $skill ($overlay)"
  [[ $DRY_RUN -eq 1 ]] && continue
  mkdir -p "$STORE/$skill/agents"
  if [[ -f $overlay ]]; then
    # An upstream openai.yaml carries its own metadata: append, never overwrite.
    printf '\n%s\n' "$CODEX_POLICY" >>"$overlay" || die "could not append the Codex overlay for $skill"
  else
    printf '%s\n' "$CODEX_POLICY" >"$overlay" || die "could not write the Codex overlay for $skill"
  fi
done < <(jq -r '.tiers // {} | to_entries[] | select(.value == "on-demand") | .key' "$COMMITTED_LOCK")

# ── superpowers routing re-assert ──────────────────────────────────────────
# The hermes mirror's routing patches are re-applied by their own script. A dry
# run probes it read-only with --check.
if jq -e '(.superpowersRouting // {} | length) > 0' "$COMMITTED_LOCK" >/dev/null 2>&1; then
  if [[ ! -x $ROUTING_SCRIPT ]]; then
    blocker "$ROUTING_SCRIPT is absent but the lock declares superpowers routing; the hermes mirror cannot be asserted"
  elif [[ $DRY_RUN -eq 1 ]]; then
    if "$ROUTING_SCRIPT" --check >/dev/null 2>&1; then
      say "  superpowers routing already asserted"
    else
      plan "re-assert the superpowers routing patches on the hermes mirror"
    fi
  else
    "$ROUTING_SCRIPT" || die "the superpowers routing re-assert failed"
    say "  superpowers routing asserted"
  fi
fi

# ── verdict ────────────────────────────────────────────────────────────────
if [[ $BLOCKERS -gt 0 ]]; then
  printf 'live-reconcile: %d divergence(s) this tool does not fix; resolve them before the cutover proceeds\n' \
    "$BLOCKERS" >&2
  exit 1
fi
if [[ $ACTIONS -eq 0 ]]; then
  say "live-reconcile: converged, 0 actions."
elif [[ $DRY_RUN -eq 1 ]]; then
  say "live-reconcile: $ACTIONS action(s) planned. Re-run without --dry-run to apply."
else
  say "live-reconcile: $ACTIONS action(s) applied."
fi
