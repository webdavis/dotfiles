#!/usr/bin/env bash
# update-skills: refresh portable skills in the canonical store (~/.agents/skills),
# dispatched by provenance, each overwrite atomic. Symlinks fan out to Claude + Hermes;
# Codex reads the store natively. Forks / user-edited skills are never touched (skip-list).
#
# Usage: update-skills [--dry-run]
set -euo pipefail

AGENTS="$HOME/.agents"
STORE="$AGENTS/skills"
LOCK="$AGENTS/.skill-lock.json"
CLAUDE="$HOME/.claude/skills"
HERMES="$HOME/.hermes/skills"
LOCKDIR="$AGENTS/.update-skills.lock.d"
KW_REPO="https://github.com/anthropics/knowledge-work-plugins"
KW_CATS="bio-research cowork-plugin-management data design engineering enterprise-search finance human-resources legal marketing operations product-management sales small-business"
# forks / user-authored copies: never auto-update
SKIP="video-transcript-downloader whisply moshi herdr"
DRYRUN="${1:-}"

log() { printf '[update-skills] %s\n' "$*"; }
is_skip() { case " $SKIP " in *" $1 "*) return 0 ;; *) return 1 ;; esac }

# atomic per-skill swap: build new content in $2 (temp on same FS), then two renames
swap() {
  local n="$1" tmp="$2"
  local dst="$STORE/$n"
  if [ "$DRYRUN" = "--dry-run" ]; then
    log "would update: $n"
    rm -rf "$tmp"
    return
  fi
  [ -e "$dst" ] && mv "$dst" "$dst.bak.$$"
  mv "$tmp" "$dst"
  rm -rf "$dst.bak.$$" 2>/dev/null || true
}

ensure_symlink() {
  local n="$1"
  [ -d "$STORE/$n" ] || return 0
  [ -e "$CLAUDE/$n" ] || ln -s "../../.agents/skills/$n" "$CLAUDE/$n"
  [ -e "$HERMES/$n" ] || ln -s "../../.agents/skills/$n" "$HERMES/$n"
}

# serialize: one run at a time (mkdir is atomic + system-shipped; flock is absent on macOS)
if [ -d "$LOCKDIR" ] && find "$LOCKDIR" -prune -mmin +120 2>/dev/null | grep -q .; then rm -rf "$LOCKDIR"; fi # steal stale lock (>2h: crashed run)
if ! mkdir "$LOCKDIR" 2>/dev/null; then
  log "another run in progress; exiting"
  exit 0
fi
trap 'rm -rf "$LOCKDIR"' EXIT

# idle-gate: defer while a harness is actively using skills (belt-and-suspenders on atomic swap)
if [ "$DRYRUN" != "--dry-run" ] && { pgrep -x claude >/dev/null 2>&1 || pgrep -x codex >/dev/null 2>&1 || pgrep -x hermes >/dev/null 2>&1; }; then
  log "a harness (claude/codex/hermes) is running; deferring this run"
  exit 0
fi

# 1) npx-tracked skills (Matt Pocock, lobster, etc.)
if [ "$DRYRUN" = "--dry-run" ]; then
  log "would run: npx skills update --global"
else
  log "npx skills update --global"
  npx --yes skills@latest update --global -y 2>&1 | tr -d '\r' | tail -3 || log "npx update reported issues (continuing)"
fi
# defensive: if npx left any tracked skill as a claude real-dir, move it back to the store
if [ -f "$LOCK" ]; then
  for n in $(jq -r '.skills|keys[]?' "$LOCK" 2>/dev/null); do
    is_skip "$n" && continue
    if [ -d "$CLAUDE/$n" ] && [ ! -L "$CLAUDE/$n" ]; then
      if [ "$DRYRUN" = "--dry-run" ]; then log "would relocate $n: claude -> store"; else
        if [ -d "$STORE/$n" ]; then rm -rf "${CLAUDE:?}/${n:?}"; else mv "$CLAUDE/$n" "$STORE/$n"; fi
        ln -s "../../.agents/skills/$n" "$CLAUDE/$n"
      fi
    fi
  done
fi

# 2) vendored knowledge-work skills: sparse-clone once, refresh each existing store skill (with namespacing)
log "vendored: knowledge-work-plugins"
tmp=$(mktemp -d)
if git clone --depth 1 --filter=blob:none --sparse "$KW_REPO" "$tmp/r" >/dev/null 2>&1; then
  # shellcheck disable=SC2086  # KW_CATS is an intentional space-separated category list
  git -C "$tmp/r" sparse-checkout set $KW_CATS >/dev/null 2>&1
  for cat in $KW_CATS; do
    for sd in "$tmp/r/$cat/skills/"*/; do
      [ -d "$sd" ] || continue
      orig=$(basename "$sd")
      case "$cat/$orig" in
        marketing/competitive-brief) n=marketing-competitive-brief ;;
        product-management/competitive-brief) n=product-management-competitive-brief ;;
        legal/review-contract) n=legal-review-contract ;;
        small-business/review-contract) n=small-business-review-contract ;;
        *) n="$orig" ;;
      esac
      is_skip "$n" && continue
      [ -d "$STORE/$n" ] || continue # only refresh what we already have
      bt=$(mktemp -d)
      rsync -a "$sd" "$bt/skill/"
      swap "$n" "$bt/skill"
      rm -rf "$bt"
    done
  done
else
  log "KW repo clone failed; skipping KW refresh"
fi
rm -rf "$tmp"

# 3) vendored portables (clean copies whose npx source path is non-trivial): git-clone, find by name, swap
update_portable() {
  local n="$1" repo="$2"
  is_skip "$n" && return 0
  [ -d "$STORE/$n" ] || return 0
  local t
  t=$(mktemp -d)
  if git clone --depth 1 "$repo" "$t/r" >/dev/null 2>&1; then
    local sd
    sd=$(find "$t/r" -maxdepth 4 -type d -name "$n" 2>/dev/null | head -1)
    [ -z "$sd" ] && {
      local sm
      sm=$(find "$t/r" -maxdepth 4 -iname SKILL.md 2>/dev/null | head -1)
      [ -n "$sm" ] && sd=$(dirname "$sm")
    }
    if [ -n "$sd" ] && [ -f "$sd/SKILL.md" ]; then
      local bt
      bt=$(mktemp -d)
      rsync -a "$sd/" "$bt/skill/"
      swap "$n" "$bt/skill"
      rm -rf "$bt"
    else log "portable $n: SKILL.md not found in $repo"; fi
  else log "portable $n: clone failed"; fi
  rm -rf "$t"
}
log "vendored: portables"
update_portable frontend-design https://github.com/anthropics/skills
update_portable kubernetes-specialist https://github.com/jeffallan/claude-skills
update_portable peekaboo https://github.com/steipete/agent-scripts
update_portable web-design-guidelines https://github.com/vercel-labs/agent-skills

# 4) ensure every store skill is symlinked into Claude + Hermes
for d in "$STORE"/*/; do [ -d "$d" ] && ensure_symlink "$(basename "$d")"; done

log "done${DRYRUN:+ (dry-run)}"
