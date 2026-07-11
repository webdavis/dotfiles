#!/usr/bin/env bash
# update-skills: keep the canonical skills store (~/.agents/skills) complete and fresh.
#
# The store holds exactly the roster this repo declares (see
# ~/.agents/custom-skill-lock.json), so the registered-skill count in the
# harnesses does not grow when this runs. The roster's provenance kinds, and
# who refreshes each:
#   - npx-tracked (npxTracked table): the store copy is installed and refreshed
#      by the official npx `skills` CLI from an official upstream, latest from
#      main (no pin). This script INSTALLS any that are absent with
#      `npx skills add <repo> --skill <name> --agent claude-code --agent codex
#      -g -y` — the multi-agent form lands the real dir in the store and plants
#      a relative ~/.claude/skills symlink (no ~/.codex dir; Codex reads the
#      store natively). The hermes symlinks are planted here. The weekly
#      `npx skills update -g` pass REFRESHES them all in place.
#   - clawhub-tracked (clawhubTracked table): the store copy is installed and
#      refreshed by the `clawhub` CLI from a ClawHub upstream (npx cannot
#      source ClawHub — `npx skills add` is GitHub-only). This script INSTALLS
#      any that are absent: `clawhub install @owner/<name>` nests its output as
#      <dir>/@owner/<name> (v0.23.1, verified live), so the install runs in a
#      throwaway --workdir and the nested dir is MOVED flat into the store as
#      <store>/<name> — its .clawhub/origin.json travels with it and pins the
#      owner, which is what lets the weekly pass refresh IN PLACE with a bare
#      `clawhub --workdir ~/.agents --dir skills update <name>` (bare-name
#      update resolves via origin.json even when the name is ambiguous on the
#      registry). The temp-workdir install also keeps the store's
#      .clawhub/lock.json free of phantom @owner-keyed entries, whose presence
#      would make a slug-form update recreate the nested dir. Fan-out (Claude +
#      hermes symlinks) is planted here, same as the npx lane. See
#      update_clawhub_tracked for the local-changes refusal ladder.
#   - vendored (dot_agents/skills/, committed): third-party copies refreshed by
#      `chezmoi apply`, never by this script. Two sub-kinds: (a) forks-table
#      entries whose upstreams the weekly run drift-checks and alerts on — the
#      deliberate content forks moshi/herdr and the npx-can't-install-full-tree
#      case elevenlabs (its SKILL.md sits at the repo root beside a scripts/
#      dir npx drops, even with --full-depth); (b) plain committed dirs with no
#      forks entry — today only tiktok-crawling, a ClawHub skill left vendored
#      because hermes owns its hub copy (hermesRegistry) and its hub name
#      differs from the roster name.
#   - app-owned symlink (cua-driver): the store entry is a symlink into the
#      app's own skill dir; the app owns the content, and the weekly run
#      refreshes the pack via `cua-driver skills update` (the app's own
#      updater; see refresh_app_owned_cua_pack).
#
# The store serves Claude/Codex always and hermes in two lanes. Symlinks fan
# out to Claude (~/.claude/skills — every store skill) and to hermes per the
# lock's hermesProfiles map ("default" = ~/.hermes/skills, any other profile
# name = ~/.hermes/profiles/<name>/skills, [] = deliberately absent). hermes
# fan-out is driven ENTIRELY by hermesProfiles: a non-empty mapping means
# symlink the store copy into those profiles, [] means do not. Collision-named
# skills (humanizer, hyperframes) never fan out at all: hermes's catalog wins
# those names, the store copies serve Claude/Codex only. The skills hermes OWNS from a registry (hermesRegistry table) are
# hub-owned dirs hermes-side that the weekly hermes phase keeps fresh via
# `hermes -p <profile> skills update <lockKey>` — a store symlink must never
# shadow those paths, which is why hermesRegistry and the non-empty
# hermesProfiles set are disjoint. Codex needs no fan-out: it scans
# $HOME/.agents/skills natively (developers.openai.com/codex/skills), and a
# ~/.codex symlink would surface every skill twice — its tiering is the
# agents/openai.yaml policy overlay that the lock's tiers table drives (see
# assert_codex_overlays below).
#
# Usage: update-skills [--dry-run] [--install-only] [--check-forks-only]
#   --dry-run           log what would change, write nothing to the filesystem
#   --install-only      ADDITIVE bootstrap: run only the npx + clawhub install
#                       passes for absent skills, then the symlink fan-out (which
#                       here CREATES missing links only, never replacing a
#                       wrong-target link or removing a stale one), the Codex
#                       overlay re-assert, and the superpowers routing re-assert
#                       (used by tests and fresh-machine bootstrap; skips the
#                       weekly npx and clawhub updates, the hermes registry-update
#                       phase, and the fork drift-check)
#   --check-forks-only  run only the fork/vendored upstream drift-check
#   --scheduled         mark this as a LaunchAgent (scheduled) run; only a
#                       scheduled run with no later slot remaining this week
#                       claims retry-budget exhaustion (a manual run never does)
# Env: UPDATE_SKILLS_FORCE=1 bypasses the idle-gate AND the weekly success stamp.
#      The idle-gate (activity-based, fail-closed) makes this script refuse to
#      swap skill folders only while a harness (claude/codex/hermes) shows RECENT
#      activity — the newest mtime among its per-turn activity files is within
#      IDLE_THRESHOLD (default 15 min). It runs UNATTENDED: an always-up bridge no
#      longer defers the weekly run forever; a machine quiet for the window
#      proceeds (see __update_skills_should_defer). Override the window with
#      UPDATE_SKILLS_IDLE_THRESHOLD (seconds) and each harness's probe dir with
#      UPDATE_SKILLS_{CLAUDE,CODEX,HERMES}_ACTIVITY_DIR (tests do). The weekly run
#      is scheduled across four Monday slots; a per-week success stamp
#      (~/.local/state/update-skills/last-success) makes the extra slots no-ops
#      after one succeeds, and the last scheduled slot alerts if it still cannot
#      run. FORCE=1 accepts the swap risk for test runs and deliberate manual runs.
set -euo pipefail

# This script clones and inspects git repos in temp dirs (fork drift-check). If
# a caller (e.g. a git hook) leaked GIT_DIR/GIT_INDEX_FILE into our environment,
# those clones would silently operate on the caller's repository instead — unset
# them.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

AGENTS="$HOME/.agents"
STORE="$AGENTS/skills"
CUSTOM_SKILL_LOCK="$AGENTS/custom-skill-lock.json" # the skills this repo wants, deployed by chezmoi (ours)
CLAUDE="$HOME/.claude/skills"
HERMES="$HOME/.hermes/skills"            # the default profile (Bob)
HERMES_PROFILES="$HOME/.hermes/profiles" # specialist profiles: <name>/skills
LOCKDIR="$AGENTS/.update-skills.lock.d"
STATE_DIR="$HOME/.local/state/update-skills"
SUCCESS_STAMP="$STATE_DIR/last-success"               # ISO year-week (%G-%V) of the last fully successful weekly run
SCHEDULED_WEEK_STAMP="$STATE_DIR/last-scheduled-week" # ISO week of the last SCHEDULED attempt (item 6)

# Generation-exchange store model (Wave 3a fix4). The LIVE generation is a REAL
# directory .skills-current holding skills/<name> real dirs, the npx CLI lock
# .skill-lock.json, and generation.json (the READY marker, written last: id +
# createdAt + custom-lock hash + updater hash). The store ~/.agents/skills/<name>
# are stable literal symlinks into ../.skills-current/skills/<name>, and
# ~/.agents/.skill-lock.json is a symlink into .skills-current/.skill-lock.json.
# Both keep resolving across a publish because .skills-current is a stable PATH
# whose CONTENTS are swapped by ONE renameat2 RENAME_EXCHANGE
# (gmv --exchange --no-copy -T), so any lookup during or after the swap yields a
# complete tree from exactly one generation. Candidate generations are built as a
# fake HOME under .skills-generations/<id>/home, on the SAME device as
# .skills-current so the same-filesystem exchange works. Exactly one previous
# generation is retained (a session that cached a resolved path keeps a complete
# tree for at least a week); older ones are garbage-renamed then deleted.
SKILLS_CURRENT="$AGENTS/.skills-current"
GENERATIONS="$AGENTS/.skills-generations"
SKILL_LOCK_LINK="$AGENTS/.skill-lock.json"
GENERATION_META_NAME="generation.json"
# GNU coreutils mv (has --exchange; BSD /bin/mv does not). Tests can override.
GMV="${UPDATE_SKILLS_GMV:-gmv}"
# Activity-based idle gate (Wave 3a fix3). The gate judges recent harness
# ACTIVITY, not mere process existence, so the weekly run is UNATTENDED (on the
# daily driver a `claude --remote-control` bridge is always up; deferring on its
# existence alone would defer forever). The window and each harness's probe dir
# are env-overridable (defaults below); tests point the dirs at a tmp HOME and
# shrink the window. IDLE_THRESHOLD is in SECONDS (default 15 minutes). The probe
# dirs are the empirically verified per-turn activity locations on this machine
# (see __update_skills_activity_state).
IDLE_THRESHOLD_SECONDS="${UPDATE_SKILLS_IDLE_THRESHOLD:-900}"
[[ $IDLE_THRESHOLD_SECONDS =~ ^[0-9]+$ ]] || IDLE_THRESHOLD_SECONDS=900
CLAUDE_ACTIVITY_DIR="${UPDATE_SKILLS_CLAUDE_ACTIVITY_DIR:-$HOME/.claude/projects}"
CODEX_ACTIVITY_DIR="${UPDATE_SKILLS_CODEX_ACTIVITY_DIR:-$HOME/.codex/sessions}"
HERMES_ACTIVITY_DIR="${UPDATE_SKILLS_HERMES_ACTIVITY_DIR:-$HOME/.hermes/logs}"
# The plist fires four Monday retry slots (04:00/08:00/12:00/16:00; see
# Library/LaunchAgents/com.webdavis.update-skills.plist.tmpl). This is the hour
# of the LAST slot: a scheduled deferral at/after it, or a coalesced catch-up on
# a later weekday, means the weekly retry budget is exhausted, so the run alerts
# LOUDLY instead of failing silent. Keep in sync with the plist.
readonly UPDATE_SKILLS_LAST_SLOT_HOUR="16"
# The Codex on-demand policy overlay this script asserts into store skill dirs
# (see assert_codex_overlays) — also what the clawhub update pass recognizes as
# its OWN file when the CLI refuses over it (see update_clawhub_tracked).
readonly CODEX_POLICY=$'policy:\n  allow_implicit_invocation: false'
# The weekly registry-update phase walks exactly the profiles that own a
# registry skill in the lock (hermesRegistry) — DERIVED from the lock at run
# time (see update_hermes_registry_skills), never hardcoded, so a new profile
# added to hermesRegistry is walked automatically with no second edit to
# forget. That includes default (Bob): its un-entanglement is DONE
# (2026-07-09) — kubernetes-specialist, lobster, and todoist-cli moved to pure
# npx store ownership (operator directive: hermes no longer owns them), so no
# registry entry has a store-symlinked install path that hermes's updater path
# validator would reject. Default is walked via `hermes -p default`, exactly
# like a specialist.

DRYRUN=""
INSTALL_ONLY=""
CHECK_FORKS_ONLY=""
SCHEDULED=""
for arg in "$@"; do
  case "$arg" in
    --dry-run) DRYRUN="--dry-run" ;;
    --install-only) INSTALL_ONLY=1 ;;
    --check-forks-only) CHECK_FORKS_ONLY=1 ;;
    --scheduled) SCHEDULED=1 ;;
    *)
      printf 'update-skills: unknown argument: %s\n' "$arg" >&2
      exit 2
      ;;
  esac
done

log() { printf '[update-skills] %s\n' "$*"; }

# Required-phase failure accounting. REQUIRED phases (npx/clawhub installs and
# updates, hermes registry updates, Codex overlay re-assert, fan-out
# convergence, superpowers routing assert) keep continue-on-failure behavior
# WITHIN a run, but every failure is RECORDED here. ADVISORY phases (fork
# drift-watch, the cua-driver pack refresh) only inform and are never recorded.
# The weekly success stamp is written ONLY when zero required failures occurred,
# so a transient failure leaves the stamp absent and a later scheduled slot retries.
REQUIRED_FAILURES=0
record_required_failure() {
  REQUIRED_FAILURES=$((REQUIRED_FAILURES + 1))
  log "REQUIRED-FAILURE: $*"
}

# True when no further SCHEDULED slot remains this ISO week to retry a failed or
# deferred run. The plist fires four Monday slots (04/08/12/16); launchd may
# COALESCE a missed slot and deliver it on a later day (a catch-up), which is
# also the week's last scheduled chance. So a later slot remains ONLY when today
# is Monday BEFORE the last slot hour; Monday at/after 16:00, or any later
# weekday (a coalesced catch-up), means the scheduled budget for this week is
# spent. date +%u is 1 for Monday; base-10 forces the hour compare so 08 is not
# read as invalid octal.
__update_skills_no_scheduled_slot_remains() {
  local dow hour
  dow="$(date +%u)"
  hour="$(date +%H)"
  hour="${hour#0}"
  [[ -n $hour ]] || hour=0
  [[ $dow =~ ^[0-9]+$ ]] || return 0
  [[ $hour =~ ^[0-9]+$ ]] || hour=0
  if [[ $dow == "1" && $((10#$hour)) -lt $((10#$UPDATE_SKILLS_LAST_SLOT_HOUR)) ]]; then
    return 1 # Monday, before the last slot: a later scheduled slot remains
  fi
  return 0 # no later scheduled slot this week
}

# Exhaustion is claimed ONLY for a SCHEDULED run (the LaunchAgent passes
# --scheduled) with no later slot remaining this week. A manual run warns loudly
# elsewhere but never claims scheduled-budget exhaustion.
__update_skills_scheduled_budget_exhausted() {
  [[ -n $SCHEDULED ]] || return 1
  __update_skills_no_scheduled_slot_remains
}

# Record the ISO week of this scheduled attempt so a coalesced catch-up on a
# later day is recognized as this week's scheduled cycle (item 6). Best-effort.
__update_skills_note_scheduled_attempt() {
  [[ -n $SCHEDULED ]] || return 0
  [[ $DRYRUN == "--dry-run" ]] && return 0
  mkdir -p "$STATE_DIR" 2>/dev/null || return 0
  date +%G-%V >"$SCHEDULED_WEEK_STAMP" 2>/dev/null || true
}

# Loud alert on both channels the brief names: a local alerter notification and
# a relay push. Best-effort; a missing tool or relay never fails the run.
__update_skills_alert() {
  local detail="$1"
  if command -v alerter >/dev/null 2>&1; then
    alerter --timeout 30 --title "update-skills" --message "$detail" --sound default >/dev/null 2>&1 || true
  fi
  local relay_script="$HOME/.local/bin/relay.sh"
  if [[ -x $relay_script ]]; then
    "$relay_script" --agent update-skills --state exhausted --project skills --detail "$detail" || true
  fi
}

# ============================================================================
# Generation-exchange machinery (Wave 3a fix4). See the SKILLS_CURRENT config
# block above for the store model. These functions are dormant unless the main
# flow calls them; they are unit-tested in isolation via UPDATE_SKILLS_LIB_ONLY.
# ============================================================================

# sha256 of a file (or the empty-input hash when absent), first field only.
__gen_hash_file() {
  local path="$1"
  [[ -f $path ]] || {
    printf '%s' "-"
    return 0
  }
  shasum -a 256 "$path" 2>/dev/null | awk '{print $1}'
}

# The two hashes that define "the desired state" for a generation: the roster
# lock (what skills the repo wants + how) and this updater script (how they are
# built). A change in either must invalidate the weekly stamp and force a rebuild.
__gen_custom_lock_hash() { __gen_hash_file "$CUSTOM_SKILL_LOCK"; }
__gen_updater_hash() { __gen_hash_file "${BASH_SOURCE[0]}"; }

# A sortable, collision-resistant generation id: epoch seconds + pid + random.
# Sortable-by-time is what lets prune keep the newest previous and delete older.
__gen_new_id() { printf '%s-%s-%s' "$(date +%s)" "$$" "${RANDOM}${RANDOM}"; }

# Two paths are on the same filesystem (required for renameat2 RENAME_EXCHANGE).
__gen_same_device() {
  local a b
  a="$(stat -f %d "$1" 2>/dev/null)" || return 1
  b="$(stat -f %d "$2" 2>/dev/null)" || return 1
  [[ -n $a && $a == "$b" ]]
}

# Write generation.json LAST, as the ready marker. Its presence + matching
# hashes is what recovery uses to tell a complete candidate from a leftover.
#   __gen_write_meta <generation-dir> <id>
__gen_write_meta() {
  local dir="$1" id="$2" meta="$1/$GENERATION_META_NAME"
  jq -n \
    --arg id "$id" \
    --arg createdAt "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    --arg customLockHash "$(__gen_custom_lock_hash)" \
    --arg updaterHash "$(__gen_updater_hash)" \
    '{id: $id, createdAt: $createdAt, customLockHash: $customLockHash, updaterHash: $updaterHash}' \
    >"$meta"
}

# Read one field from a generation.json (empty when absent/unreadable).
#   __gen_meta_field <generation-dir> <field>
__gen_meta_field() {
  local meta="$1/$GENERATION_META_NAME"
  [[ -f $meta ]] || return 0
  jq -r --arg f "$2" '.[$f] // ""' "$meta" 2>/dev/null || true
}

# A generation dir is COMPLETE iff it has skills/, the npx lock, and a
# generation.json carrying a non-empty id (the ready marker was fully written).
__gen_is_complete() {
  local dir="$1"
  [[ -d "$dir/skills" ]] || return 1
  [[ -f "$dir/.skill-lock.json" ]] || return 1
  [[ -n "$(__gen_meta_field "$dir" id)" ]]
}

# A complete generation MATCHES the current desired state iff its recorded
# hashes equal the live lock+updater hashes.
__gen_meta_matches_desired() {
  local dir="$1"
  [[ "$(__gen_meta_field "$dir" customLockHash)" == "$(__gen_custom_lock_hash)" ]] || return 1
  [[ "$(__gen_meta_field "$dir" updaterHash)" == "$(__gen_updater_hash)" ]]
}

# Destroy a path the crash-safe way: rename it to a clearly-garbage sibling name
# FIRST (atomic), then rm -rf. A crash between the two leaves a *.garbage.*
# name that recovery/prune resumes deleting; nothing a live link resolves into
# ever carries a garbage name, so a half-deleted tree is never mistaken for live.
__gen_garbage_destroy() {
  local path="$1" garbage
  [[ -e $path || -L $path ]] || return 0
  garbage="${path%/}.garbage.$$.${RANDOM}"
  if mv "$path" "$garbage" 2>/dev/null; then
    rm -rf "$garbage" 2>/dev/null || true
  else
    rm -rf "$path" 2>/dev/null || true
  fi
}

# Resume any interrupted deletion: sweep *.garbage.* leftovers under a parent.
__gen_sweep_garbage() {
  local parent="$1" entry
  [[ -d $parent ]] || return 0
  for entry in "$parent"/*.garbage.*; do
    [[ -e $entry || -L $entry ]] || continue
    rm -rf "$entry" 2>/dev/null || true
  done
}

# Plant (or repair) the stable store link for one skill: ~/.agents/skills/<name>
# -> ../.skills-current/skills/<name>. Idempotent; only ever writes an
# updater-owned link, never clobbers a real dir it does not own.
__gen_plant_store_link() {
  local name="$1"
  local link="$STORE/$name"
  local want="../.skills-current/skills/$name"
  mkdir -p "$STORE"
  if [[ -L $link ]]; then
    [[ "$(readlink "$link" 2>/dev/null || true)" == "$want" ]] && return 0
    ln -sfn "$want" "$link"
    return 0
  fi
  [[ -e $link ]] && return 1 # a real dir/file we do not own; caller decides
  ln -s "$want" "$link"
}

# Plant (or repair) the ~/.agents/.skill-lock.json symlink into the live
# generation's lock. Idempotent.
__gen_plant_lock_link() {
  local want=".skills-current/.skill-lock.json"
  if [[ -L $SKILL_LOCK_LINK ]]; then
    [[ "$(readlink "$SKILL_LOCK_LINK" 2>/dev/null || true)" == "$want" ]] && return 0
  fi
  ln -sfn "$want" "$SKILL_LOCK_LINK"
}

# PUBLISH: swap a fully-built candidate generation dir into place as the new
# .skills-current with ONE atomic exchange, then retain the displaced previous
# generation and prune older ones.
#   __gen_publish <candidate-generation-dir>
# Preconditions (all checked): candidate and .skills-current are both real dirs
# on the same device, and the candidate is complete (ready marker present). On
# success .skills-current holds the new generation and the previous generation
# is retained under .skills-generations/<old-id>. Returns 0 on publish, 1 on any
# precondition failure (caller records a required failure; live state untouched).
__gen_publish() {
  local candidate="$1" old_id
  [[ -d $candidate && ! -L $candidate ]] || {
    log "publish: candidate $candidate is not a real directory"
    return 1
  }
  [[ -d $SKILLS_CURRENT && ! -L $SKILLS_CURRENT ]] || {
    log "publish: $SKILLS_CURRENT is not a real directory"
    return 1
  }
  __gen_is_complete "$candidate" || {
    log "publish: candidate $candidate is not complete (no ready marker)"
    return 1
  }
  __gen_same_device "$candidate" "$SKILLS_CURRENT" || {
    log "publish: candidate and .skills-current are on different devices; exchange impossible"
    return 1
  }
  old_id="$(__gen_meta_field "$SKILLS_CURRENT" id)"
  [[ -n $old_id ]] || old_id="pre-$(__gen_new_id)" # a first-migrated current may predate meta
  # THE atomic publish: renameat2 RENAME_EXCHANGE. After it, .skills-current is
  # the new generation and $candidate holds the complete PREVIOUS generation.
  if ! "$GMV" --exchange --no-copy -T "$candidate" "$SKILLS_CURRENT" 2>/dev/null; then
    log "publish: gmv --exchange failed; live generation untouched"
    return 1
  fi
  mkdir -p "$GENERATIONS"
  # Retain the displaced previous generation under its id (garbage-destroy any
  # name collision first so the rename lands cleanly).
  local retained="$GENERATIONS/$old_id"
  __gen_garbage_destroy "$retained"
  mv "$candidate" "$retained" 2>/dev/null || __gen_garbage_destroy "$candidate"
  __gen_prune_generations "$old_id"
  return 0
}

# Keep EXACTLY the one just-retained previous generation; garbage-destroy every
# other generation dir. Never touch a staging/home dir that may still be in use
# by the caller (those live under .skills-generations/<id>/home during a build;
# a retained generation is a bare <id> dir). The just-retained id is preserved.
#   __gen_prune_generations <keep-id>
__gen_prune_generations() {
  local keep_id="$1" entry name
  [[ -d $GENERATIONS ]] || return 0
  __gen_sweep_garbage "$GENERATIONS"
  for entry in "$GENERATIONS"/*; do
    [[ -d $entry ]] || continue
    name="${entry##*/}"
    [[ $name == "$keep_id" ]] && continue
    # A retained previous generation is a bare <id> dir with a generation.json;
    # a build workspace is <id>/home/... . Only prune retained generations here.
    [[ -f "$entry/$GENERATION_META_NAME" ]] || continue
    __gen_garbage_destroy "$entry"
  done
}

# The generation-owned skills: exactly the npx- and clawhub-tracked roster
# names. These live inside .skills-current/skills/ and their store entries are
# symlinks; vendored and app-owned skills stay real in the store, outside the
# generation.
__gen_tracked_names() {
  [[ -f $CUSTOM_SKILL_LOCK ]] || return 0
  jq -r '((.npxTracked // {}) + (.clawhubTracked // {})) | keys[]?' "$CUSTOM_SKILL_LOCK" 2>/dev/null
}

# True when a store entry is the correct migrated symlink for a tracked skill.
__gen_store_link_correct() {
  local name="$1"
  local link="$STORE/$name"
  [[ -L $link ]] || return 1
  [[ "$(readlink "$link" 2>/dev/null || true)" == "../.skills-current/skills/$name" ]]
}

# ---------------------------------------------------------------------------
# RECOVERY state table (brief step 1). Runs before the idle gate and stamp
# logic. Self-heals what it can and records two things the main flow acts on:
#   GEN_REABSORB[]      — tracked names whose store entry is a REAL DIR where a
#                         link is expected (a competing writer, e.g. the
#                         HyperFrames self-updater, or an interrupted migration):
#                         re-absorb that content into this run's candidate.
#   GEN_REUSE_CANDIDATE — a complete, unpublished candidate whose generation.json
#                         matches the current desired state: the main flow may
#                         publish it instead of rebuilding.
# The self-healed cases: incomplete staging leftovers are garbage-destroyed;
# published-generation link drift (stale .skill-lock.json link or store links)
# is repaired; partial-prune garbage is swept; retained generations beyond the
# newest one are pruned.
# ---------------------------------------------------------------------------
GEN_REABSORB=()
GEN_REUSE_CANDIDATE=""
__gen_recover() {
  GEN_REABSORB=()
  GEN_REUSE_CANDIDATE=""
  __gen_sweep_garbage "$GENERATIONS"
  __gen_sweep_garbage "$STORE"
  local entry id newest_retained="" newest_epoch=-1 epoch cand_agents
  if [[ -d $GENERATIONS ]]; then
    for entry in "$GENERATIONS"/*; do
      [[ -d $entry ]] || continue
      id="${entry##*/}"
      case "$id" in *.garbage.*) continue ;; esac
      # A build workspace: .skills-generations/<id>/home/.agents .
      if [[ -d "$entry/home" ]]; then
        cand_agents="$entry/home/.agents"
        if __gen_is_complete "$cand_agents" && __gen_meta_matches_desired "$cand_agents"; then
          # A complete candidate matching desired state: reusable (keep the
          # newest such by createdAt is overkill — one is enough to publish).
          [[ -z $GEN_REUSE_CANDIDATE ]] && GEN_REUSE_CANDIDATE="$cand_agents"
          continue
        fi
        log "recovery: deleting incomplete or stale staging $entry"
        __gen_garbage_destroy "$entry"
        continue
      fi
      # A retained previous generation: bare <id> dir with a generation.json.
      if [[ -f "$entry/$GENERATION_META_NAME" ]] && __gen_is_complete "$entry"; then
        epoch="${id%%-*}"
        [[ $epoch =~ ^[0-9]+$ ]] || epoch=0
        if [[ $epoch -gt $newest_epoch ]]; then
          [[ -n $newest_retained ]] && __gen_garbage_destroy "$newest_retained"
          newest_retained="$entry"
          newest_epoch=$epoch
        else
          __gen_garbage_destroy "$entry"
        fi
        continue
      fi
      # Anything else in the generations dir is leftover garbage.
      log "recovery: deleting leftover $entry"
      __gen_garbage_destroy "$entry"
    done
  fi
  # A published live generation: repair its stable links, and detect any tracked
  # store entry that is a REAL DIR (competing writer) to re-absorb this run.
  if __gen_is_complete "$SKILLS_CURRENT"; then
    __gen_plant_lock_link || log "recovery: could not repair the .skill-lock.json link"
    local name link
    while IFS= read -r name; do
      [[ -n $name ]] || continue
      link="$STORE/$name"
      if [[ -d $link && ! -L $link ]]; then
        log "recovery: store/$name is a real dir where a link is expected (competing writer); recording for re-absorption"
        GEN_REABSORB+=("$name")
      elif [[ ! -e $link && ! -L $link ]]; then
        # a tracked skill present in the generation but missing its store link
        if [[ -d "$SKILLS_CURRENT/skills/$name" ]]; then __gen_plant_store_link "$name" || true; fi
      elif [[ -L $link ]] && ! __gen_store_link_correct "$name"; then
        __gen_plant_store_link "$name" || true
      fi
    done < <(__gen_tracked_names)
  fi
}

# ---------------------------------------------------------------------------
# MIGRATION (brief "Migration"): first run on a machine with the old flat store
# (~/.agents/skills/<name> real dirs, ~/.agents/.skill-lock.json a real file).
# Build .skills-current from the existing tracked real dirs (clone), then per
# tracked store entry atomically EXCHANGE the real dir with a prebuilt hidden
# symlink so the store name never dangles and a crash leaves either
# complete-legacy or complete-migrated per entry. Idempotent: an entry already
# pointing at the generation is skipped. The .skill-lock.json symlink is planted
# the same exchange way. Vendored and app-owned store entries are left untouched
# (outside the generation). Returns 0 when migration ran or was already done.
# ---------------------------------------------------------------------------
__gen_migration_needed() {
  # Needed when no live generation exists yet but a flat store does.
  __gen_is_complete "$SKILLS_CURRENT" && return 1
  [[ -d $STORE ]]
}
__gen_migrate() {
  [[ -d $STORE ]] || return 0
  local id name src link_stub
  id="$(__gen_new_id)"
  # 1) Build .skills-current as a real dir from the existing tracked real dirs.
  if ! __gen_is_complete "$SKILLS_CURRENT"; then
    local staging="$GENERATIONS/migrate-$id"
    __gen_garbage_destroy "$staging"
    mkdir -p "$staging/skills"
    while IFS= read -r name; do
      [[ -n $name ]] || continue
      src="$STORE/$name"
      # Only clone a real dir; a symlink here is already migrated/app-owned.
      [[ -d $src && ! -L $src ]] || continue
      cp -c -R "$src" "$staging/skills/$name" 2>/dev/null || cp -R "$src" "$staging/skills/$name"
    done < <(__gen_tracked_names)
    # Seed the npx lock from the flat one (or an empty object).
    if [[ -f $SKILL_LOCK_LINK && ! -L $SKILL_LOCK_LINK ]]; then
      cp -c "$SKILL_LOCK_LINK" "$staging/.skill-lock.json" 2>/dev/null || cp "$SKILL_LOCK_LINK" "$staging/.skill-lock.json"
    else
      printf '{}\n' >"$staging/.skill-lock.json"
    fi
    __gen_write_meta "$staging" "$id"
    __gen_is_complete "$staging" || {
      log "migration: built staging is not complete; aborting (flat store untouched)"
      __gen_garbage_destroy "$staging"
      return 1
    }
    # Promote staging to the live .skills-current. On a fresh machine .skills-current
    # is absent, so a plain rename publishes it atomically.
    if [[ ! -e $SKILLS_CURRENT ]]; then
      mkdir -p "$GENERATIONS"
      mv "$staging" "$SKILLS_CURRENT" || {
        log "migration: could not promote staging to .skills-current"
        __gen_garbage_destroy "$staging"
        return 1
      }
    else
      # .skills-current exists but is incomplete: exchange it in, garbage the old.
      if __gen_same_device "$staging" "$SKILLS_CURRENT" &&
        "$GMV" --exchange --no-copy -T "$staging" "$SKILLS_CURRENT" 2>/dev/null; then
        __gen_garbage_destroy "$staging"
      else
        __gen_garbage_destroy "$staging"
        log "migration: could not exchange staging into an incomplete .skills-current"
        return 1
      fi
    fi
  fi
  # 2) Per tracked entry, atomically swap the flat real dir for a store symlink.
  while IFS= read -r name; do
    [[ -n $name ]] || continue
    __gen_store_link_correct "$name" && continue # idempotent: already migrated
    [[ -d "$SKILLS_CURRENT/skills/$name" ]] || continue
    link="$STORE/$name"
    if [[ -d $link && ! -L $link ]]; then
      # legacy real dir: exchange it with a prebuilt hidden symlink
      link_stub="$STORE/.$name.migrating.$$"
      __gen_garbage_destroy "$link_stub"
      ln -s "../.skills-current/skills/$name" "$link_stub"
      if __gen_same_device "$link_stub" "$link" &&
        "$GMV" --exchange --no-copy -T "$link_stub" "$link" 2>/dev/null; then
        __gen_garbage_destroy "$link_stub" # now holds the displaced real dir (garbage)
        log "migration: store/$name -> generation link (legacy dir absorbed)"
      else
        __gen_garbage_destroy "$link_stub"
        log "migration: could not exchange store/$name; leaving the legacy dir"
        record_required_failure "migration exchange for $name failed"
      fi
    elif [[ ! -e $link && ! -L $link ]]; then
      __gen_plant_store_link "$name" || true # absent: just plant the link
    fi
  done < <(__gen_tracked_names)
  # 3) Plant the .skill-lock.json symlink the same exchange way.
  if [[ -f $SKILL_LOCK_LINK && ! -L $SKILL_LOCK_LINK ]]; then
    link_stub="$AGENTS/.skill-lock.json.migrating.$$"
    __gen_garbage_destroy "$link_stub"
    ln -s ".skills-current/.skill-lock.json" "$link_stub"
    if __gen_same_device "$link_stub" "$SKILL_LOCK_LINK" &&
      "$GMV" --exchange --no-copy -T "$link_stub" "$SKILL_LOCK_LINK" 2>/dev/null; then
      __gen_garbage_destroy "$link_stub"
    else
      __gen_garbage_destroy "$link_stub"
      __gen_plant_lock_link || true
    fi
  else
    __gen_plant_lock_link || true
  fi
  return 0
}

# Lib-only sourcing gate: a test that sets UPDATE_SKILLS_LIB_ONLY=1 and sources
# this script gets the config + machinery functions above WITHOUT running the
# main flow (the lanes, idle gate, and publish orchestration below never fire).
# `return` only works in a sourced file; when the script is executed normally
# the variable is unset, so this is a no-op.
if [[ ${UPDATE_SKILLS_LIB_ONLY:-} == 1 ]]; then
  # shellcheck disable=SC2317 # exit is reached when executed (return fails outside a sourced file)
  return 0 2>/dev/null || exit 0
fi

# idle-gate discriminator (Wave 3a fix3, fail-closed). The gate judges recent
# harness ACTIVITY, not mere process existence. Rationale: argv SHAPE cannot
# prove idleness on this machine (every interactive Claude launch carries
# --remote-control, the bridge is on by default; Codex `app-server` and the
# Hermes gateway both host live turns in-process), so we cannot tell a live
# session from an idle bridge by its argv. Deferring on process existence alone
# (the round-2 gate) meant the always-up bridge deferred the weekly run FOREVER
# and forced a manual run. So the gate now: (1) if NO process resolves to a
# harness, PROCEED (fast path, evidence never probed); (2) if a harness process
# exists, probe every harness PRESENT on the machine for recent file activity and
# DEFER only while at least one is fresh (within IDLE_THRESHOLD); (3) fail closed
# — an unreadable process table or a probe error counts as ACTIVE. This is the
# UNATTENDED norm: the weekly run proceeds whenever the machine has been quiet for
# IDLE_THRESHOLD, even with a bridge up.
#
# PR-facing tradeoff: the gate reads mtimes, not a lock the harnesses hold, so
# there is a narrow race — a session that starts writing in the millisecond after
# the probe scans could have a skill folder swapped mid-turn. The window is tiny
# and the blast radius is one weekly run; the design accepts it as the unattended
# tradeoff. FOLLOW-UP (a task exists): the durable fix is a versioned skills store
# with an atomic symlink flip (a running session keeps its resolved inode; new
# sessions pick up the flipped version), after which this activity gate becomes
# belt-and-suspenders rather than the sole guard.
#
# __update_skills_effective_program resolves one `ps -xo args=` line to its
# harness name (or empty). It is interpreter-aware (item 2): when the program is
# an interpreter front (python/python3/python3.NN, node, bun, with an optional
# leading `env`), it skips the interpreter's OWN options to the module or script
# operand:
#   - `-m MOD` -> the module's leading identifier mapped to its harness
#     (hermes_cli.main -> hermes);
#   - the arg-taking python/node options -X -W -c -e --eval consume the next
#     token too;
#   - `--` ends options, the next token is the script (its basename);
#   - a bare script path -> its basename (a trailing .js/.mjs/.cjs/.py stripped).
# node/bun-fronted `claude` (npm-style installs) resolves the same way.
__update_skills_effective_program() {
  local -a tokens
  read -ra tokens <<<"$1"
  [[ ${#tokens[@]} -gt 0 ]] || return 0
  local i=0 t base module
  base="${tokens[0]##*/}"
  # strip a leading `env` (skip its VAR=val assignments and options; -u/-S take
  # an argument) to the real command.
  if [[ $base == "env" ]]; then
    i=1
    while [[ $i -lt ${#tokens[@]} ]]; do
      t="${tokens[$i]}"
      case "$t" in
        -u | -S) i=$((i + 1)) ;; # consumes the next token
        -*) : ;;                 # a lone env option
        *=*) : ;;                # a VAR=value assignment
        *) break ;;              # the command
      esac
      i=$((i + 1))
    done
    [[ $i -lt ${#tokens[@]} ]] || return 0
    base="${tokens[$i]##*/}"
  fi
  case "$base" in
    python | python[0-9] | python[0-9].[0-9]* | node | bun)
      local j=$((i + 1))
      while [[ $j -lt ${#tokens[@]} ]]; do
        t="${tokens[$j]}"
        case "$t" in
          -m)
            module="${tokens[$((j + 1))]:-}"
            module="${module%%.*}" # hermes_cli.main -> hermes_cli
            module="${module%%_*}" # hermes_cli -> hermes
            printf '%s' "$module"
            return 0
            ;;
          --)
            j=$((j + 1))
            break
            ;;
          -X | -W | -c | -e | --eval)
            j=$((j + 2)) # option consumes the next token too
            ;;
          -*)
            j=$((j + 1)) # a lone interpreter option
            ;;
          *)
            break # the script operand
            ;;
        esac
      done
      [[ $j -lt ${#tokens[@]} ]] || return 0
      base="${tokens[$j]##*/}"
      ;;
  esac
  # strip a trailing script extension so cli.js / claude.py resolve to the bin name
  base="${base%.js}"
  base="${base%.mjs}"
  base="${base%.cjs}"
  base="${base%.py}"
  printf '%s' "$base"
}

# True (0) for one argv line that resolves to an agent harness (claude/codex/
# hermes), i.e. the gate must DEFER; 1 otherwise.
__update_skills_is_interactive_harness() {
  local effective
  effective="$(__update_skills_effective_program "$1")"
  case "$effective" in
    claude | codex | hermes) return 0 ;;
    *) return 1 ;;
  esac
}

# Enumerate the process table for agent-harness processes. Returns:
#   0 = at least one agent-harness process (claude/codex/hermes) is running
#   1 = ps read cleanly and NO harness process exists (the fast-path signal)
#   2 = ps could not be read (or yielded nothing — it must at minimum list this
#       very process): FAIL CLOSED, the caller must defer.
__update_skills_harness_active() {
  local ps_output args
  if ! ps_output="$(ps -xo args= 2>/dev/null)" || [[ -z $ps_output ]]; then
    return 2
  fi
  while IFS= read -r args; do
    [[ -n $args ]] || continue
    __update_skills_is_interactive_harness "$args" && return 0
  done <<<"$ps_output"
  return 1
}

# Recent-activity probe for one harness's activity dir, against a cutoff SENTINEL
# file whose mtime is (now - IDLE_THRESHOLD). Returns:
#   0 = ACTIVE  — a file newer than the sentinel exists, OR the probe errored
#                 (unreadable dir / scan failure): fail closed and treat as active
#   1 = STALE   — the dir is present and readable but its newest file is older
#                 than the window
#   2 = ABSENT  — the dir does not exist: this harness is not installed, so it
#                 contributes NO evidence and must not block
#
# The sentinel + POSIX `-newer` is deliberate: macOS's BSD `/usr/bin/find` does
# NOT parse `-newermt "@<epoch>"` (verified: "Can't parse date/time: @…"), so a
# sentinel whose mtime is the cutoff is the portable primitive. `-print -quit`
# bails on the first newer file (cheap: a full idle scan of ~3.2k Claude
# transcripts is ~25 ms; only a fully idle machine scans them all). find exits
# non-zero on a scan error (e.g. an unreadable subtree), which we treat as ACTIVE.
#
# Empirically verified per-turn activity sources on this machine (2026-07-11):
#   Claude Code: ~/.claude/projects/**/<session>.jsonl — the transcript is
#     appended per turn; the FILE mtime is the signal (directory mtimes do NOT
#     propagate on append: verified live, newest file mtime == wall clock during
#     an active turn while the enclosing dir mtime lagged ~45s).
#   Codex: ~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl — the per-turn rollout
#     transcript (verified: mtimes advance per turn, stale while idle).
#   Hermes: ~/.hermes/logs/agent.log (and siblings) — the gateway's per-run/turn
#     log (verified: agent.log mtime advances while the gateway serves a turn).
__update_skills_activity_state() {
  local dir="$1" sentinel="$2" newer
  [[ -d $dir ]] || return 2            # not installed → no evidence
  [[ -r $dir && -x $dir ]] || return 0 # present but unreadable → fail closed
  if ! newer="$(find "$dir" -type f -newer "$sentinel" -print -quit 2>/dev/null)"; then
    return 0 # scan error → fail closed → ACTIVE
  fi
  [[ -n $newer ]] && return 0 # a file newer than the cutoff → ACTIVE
  return 1                    # every file older than the window → STALE
}

# Build a temp sentinel file whose mtime is (now - IDLE_THRESHOLD). Prints its
# path on success (rc 0); rc 1 if it could not be built (the caller fails closed).
__update_skills_make_cutoff_sentinel() {
  local cutoff sentinel stamp
  cutoff=$(($(date +%s) - IDLE_THRESHOLD_SECONDS))
  sentinel="$(mktemp)" || return 1
  # date -r <epoch> is BSD (this is a macOS LaunchAgent script); the test's date
  # stub execs the real date for -r, so this is exercised faithfully.
  stamp="$(date -r "$cutoff" +%Y%m%d%H%M.%S 2>/dev/null)" || {
    rm -f "$sentinel"
    return 1
  }
  if ! touch -t "$stamp" "$sentinel" 2>/dev/null; then
    rm -f "$sentinel"
    return 1
  fi
  printf '%s' "$sentinel"
}

# The full gate decision: return 0 to DEFER, 1 to PROCEED. See the discriminator
# comment above for the three-step rationale.
__update_skills_should_defer() {
  __update_skills_harness_active
  case $? in
    1) return 1 ;; # no harness process → PROCEED (fast path; probes untouched)
    2) return 0 ;; # ps unreadable → fail closed → DEFER
  esac
  # A harness process exists: judge idleness by ACTIVITY across every PRESENT
  # harness. DEFER as soon as one shows recent activity (or errors); PROCEED only
  # when every present harness is stale (and absent ones contribute nothing).
  local sentinel dir state defer=1
  sentinel="$(__update_skills_make_cutoff_sentinel)" || return 0 # no sentinel → fail closed
  for dir in "$CLAUDE_ACTIVITY_DIR" "$CODEX_ACTIVITY_DIR" "$HERMES_ACTIVITY_DIR"; do
    __update_skills_activity_state "$dir" "$sentinel"
    state=$?
    if [[ $state -eq 0 ]]; then # ACTIVE → DEFER (finish the loop, then clean up)
      defer=0
      break
    fi
  done
  rm -f "$sentinel"
  return $((defer == 0 ? 0 : 1))
}

# updater-owned link = a SYMLINK whose literal target is EXACTLY this user's
# store followed by a single skill basename: either the absolute "$STORE/<name>"
# or the exact relative prefix the fan-out plants for the calling dir
# ("$expected_prefix/<name>"). The literal readlink target is matched (not a
# resolved path), so this still holds for a DANGLING link. Matching the exact
# prefix (not a loose ".agents/skills/" substring) is the fix for the audit's
# false positives: a foreign link like /tmp/x/.agents/skills/y or
# /Users/other/.agents/skills/y is NOT owned and must survive. <name> is a
# single path segment, so a target reaching deeper (".../skills/a/b") is not
# owned either. ONLY owned links are ever replaced or removed by convergence.
#   __update_skills_is_owned_link <path> <expected_prefix>
__update_skills_is_owned_link() {
  local path="$1" expected_prefix="$2" target name
  [[ -L $path ]] || return 1
  target="$(readlink "$path" 2>/dev/null || true)"
  case "$target" in
    "$STORE"/*) name="${target#"$STORE"/}" ;;
    "$expected_prefix"/*) name="${target#"$expected_prefix"/}" ;;
    *) return 1 ;;
  esac
  # a single valid skill basename: no slash, no leading dot, allowed chars only
  [[ $name == "${name%%/*}" && $name =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]]
}

# Converge one managed dir to a desired {name -> "$prefix/$name"} set:
#   converge_dir <dir> <target_prefix> <desired_name>...
# create a missing desired link; REPLACE an updater-owned link whose target
# differs (wrong-target, incl. dangling: the additive `[[ -e ]] || ln -s`
# crashed on a dangling link); REMOVE an updater-owned link no longer desired
# (stale). A real dir/file (hub-owned/catalog) at a managed name, and any
# non-store symlink, are left untouched. A no-op convergence is silent.
#
# Two run modes narrow that behavior, driven by the globals $DRYRUN and
# $INSTALL_ONLY:
#   * --dry-run: make NO filesystem writes at all. Report each action as a
#     "would create/replace/remove" line and change nothing. A preview must
#     never mutate live link state.
#   * --install-only: ADDITIVE only. Create a missing desired link, but NEVER
#     replace a wrong-target link (leave it + a loud warning) and NEVER remove a
#     stale one. This is what lets the fresh-machine bootstrap run at apply time,
#     even under a live agent session, without swapping anything. Destructive
#     reconciliation (replace/remove) runs solely in the full weekly path behind
#     the idle gate.
converge_dir() {
  local dir="$1" prefix="$2"
  shift 2
  local -a desired=("$@")
  local skill target path current name is_desired old_target
  local dry="" additive=""
  [[ $DRYRUN == "--dry-run" ]] && dry=1
  [[ -n $INSTALL_ONLY ]] && additive=1
  # dry-run makes no writes, so it does not even create the managed dir.
  [[ -n $dry ]] || mkdir -p "$dir"
  # 1) create or repair every desired link
  if [[ ${#desired[@]} -gt 0 ]]; then
    for skill in "${desired[@]}"; do
      target="$prefix/$skill"
      path="$dir/$skill"
      if [[ -L $path ]]; then
        current="$(readlink "$path" 2>/dev/null || true)"
        [[ $current == "$target" ]] && continue # already correct
        if __update_skills_is_owned_link "$path" "$prefix"; then
          if [[ -n $additive ]]; then
            log "converge: WARN $path points to $current, not $target; --install-only is additive and leaves it (a full run repairs)"
          elif [[ -n $dry ]]; then
            log "converge: would replace $path (currently $current, desired $target)"
          else
            if ln -sfn "$target" "$path"; then # replace wrong-target / dangling updater-owned link
              log "converge: replaced $path (was $current, now $target)"
            else
              record_required_failure "converge could not replace $path"
            fi
          fi
        else
          log "converge: WARN $path is a non-store symlink at a managed name; leaving it (resolve by hand)"
        fi
      elif [[ -e $path ]]; then
        : # a real dir/file (hub-owned or catalog) at this name, never overwrite
      elif [[ -n $dry ]]; then
        log "converge: would create $path -> $target"
      elif ln -s "$target" "$path"; then
        log "converge: created $path -> $target"
      else
        record_required_failure "converge could not create $path"
      fi
    done
  fi
  # 2) remove updater-owned links no longer desired (stale drift). Additive
  #    --install-only never removes; only the full weekly path reconciles.
  [[ -n $additive ]] && return 0
  for path in "$dir"/*; do
    [[ -e $path || -L $path ]] || continue # skip the un-globbed literal when the dir is empty
    name="${path##*/}"
    is_desired=""
    if [[ ${#desired[@]} -gt 0 ]]; then
      for skill in "${desired[@]}"; do
        [[ $skill == "$name" ]] && {
          is_desired=1
          break
        }
      done
    fi
    [[ -n $is_desired ]] && continue
    if __update_skills_is_owned_link "$path" "$prefix"; then
      old_target="$(readlink "$path" 2>/dev/null || true)"
      if [[ -n $dry ]]; then
        log "converge: would remove stale $path (currently $old_target)"
      elif rm -f "$path"; then
        log "converge: removed stale $path (was $old_target)"
      else
        record_required_failure "converge could not remove stale $path"
      fi
    fi
  done
}

# Claude fan-out: every store skill (the full roster) gets a ~/.claude/skills
# link. Claude is not profile-scoped, tiering there is the settings
# modify-template's job, not the fan-out's.
converge_claude_skills() {
  local -a desired=()
  local skill_path skill
  for skill_path in "$STORE"/*; do
    [[ -d $skill_path || -L $skill_path ]] || continue
    skill="${skill_path##*/}"
    desired+=("$skill")
  done
  if [[ ${#desired[@]} -gt 0 ]]; then
    converge_dir "$CLAUDE" "../../.agents/skills" "${desired[@]}"
  else
    converge_dir "$CLAUDE" "../../.agents/skills"
  fi
}

# Hermes fan-out is profile-driven by the lock's hermesProfiles map. "default" is
# ~/.hermes/skills (Bob), any other name is ~/.hermes/profiles/<name>/skills
# (created here when absent, so a mapping can land before its profile exists on
# this machine). A [] mapping (or a missing table) gets no hermes link: the
# deliberate "not available in hermes from the store" state, not an error.
# Collision-named skills (humanizer, hyperframes) never fan out: hermes's catalog
# wins those names (operator ruling), so a stale store link at such a name IS
# removed by convergence, but creating one never happens. The walk universe is
# every profile the lock maps PLUS every profile with an EXISTING hermes skills
# dir on disk, so a profile whose last mapped skill was de-mapped is still walked
# and its stale updater-owned links get reaped (they would otherwise linger
# forever). Only owned links are ever removed, so a foreign file in the same dir
# survives.
HERMES_COLLISION_NAMES=(humanizer hyperframes)
is_hermes_collision_name() {
  local collision_entry
  for collision_entry in "${HERMES_COLLISION_NAMES[@]}"; do
    [[ $collision_entry == "$1" ]] && return 0
  done
  return 1
}
# The profile walk universe: names the lock maps, plus "default" and every
# specialist whose skills dir already exists on disk.
__update_skills_hermes_profile_universe() {
  jq -r '.hermesProfiles // {} | [.[][]?] | unique | .[]' "$CUSTOM_SKILL_LOCK" 2>/dev/null
  [[ -d $HERMES ]] && printf 'default\n'
  local profile_skills_dir profile_name
  if [[ -d $HERMES_PROFILES ]]; then
    for profile_skills_dir in "$HERMES_PROFILES"/*/skills; do
      [[ -d $profile_skills_dir ]] || continue
      profile_name="${profile_skills_dir%/skills}"
      printf '%s\n' "${profile_name##*/}"
    done
  fi
}
# Reject a managed hermes dir reached THROUGH a directory symlink (item 8). A
# profiles/<name> or <name>/skills symlink pointing outside ~/.hermes would let
# convergence create or REMOVE links in that foreign target, decided from the
# literal relative link text. We take the ruling's reject-symlink branch: when
# the profile dir OR its skills child is a symlink, we never converge through it
# (so the managed dir is always a real path under ~/.hermes and every removal
# stays within this user's tree). Returns 0 = safe to converge, 1 = skip.
__update_skills_hermes_dir_safe() {
  local profile="$1" link_dir="$2" profile_dir
  if [[ $profile == "default" ]]; then
    profile_dir="$HOME/.hermes"
  else
    profile_dir="$HERMES_PROFILES/$profile"
  fi
  if [[ -L $profile_dir ]]; then
    log "converge: WARN hermes profile dir $profile_dir is a symlink; skipping (never converge through a directory symlink)"
    return 1
  fi
  if [[ -L $link_dir ]]; then
    log "converge: WARN hermes skills dir $link_dir is a symlink; skipping (never converge through a directory symlink)"
    return 1
  fi
  return 0
}
converge_hermes_skills() {
  [[ -f $CUSTOM_SKILL_LOCK ]] || return 0
  local profile link_dir prefix skill
  local -a profiles=() desired=()
  # No early return: an empty universe simply walks nothing. A de-mapped profile
  # is reached via its on-disk dir even though the lock no longer names it.
  while IFS= read -r profile; do
    [[ -n $profile ]] && profiles+=("$profile")
  done < <(__update_skills_hermes_profile_universe | sort -u)
  for profile in "${profiles[@]}"; do
    if [[ $profile == "default" ]]; then
      link_dir="$HERMES"
      prefix="../../.agents/skills"
    else
      link_dir="$HERMES_PROFILES/$profile/skills"
      prefix="../../../../.agents/skills"
    fi
    __update_skills_hermes_dir_safe "$profile" "$link_dir" || continue
    desired=()
    while IFS= read -r skill; do
      [[ -n $skill ]] || continue
      is_hermes_collision_name "$skill" && continue              # collision names never fan out
      [[ -d "$STORE/$skill" || -L "$STORE/$skill" ]] || continue # only skills present in the store
      desired+=("$skill")
    done < <(jq -r --arg p "$profile" '.hermesProfiles // {} | to_entries[]
      | select((.value // []) | index($p) != null) | .key' "$CUSTOM_SKILL_LOCK" 2>/dev/null)
    if [[ ${#desired[@]} -gt 0 ]]; then
      converge_dir "$link_dir" "$prefix" "${desired[@]}"
    else
      converge_dir "$link_dir" "$prefix"
    fi
  done
}

# Superpowers→hermes routing re-assert: the hand-patched hermes-superpowers
# mirror (~/.hermes/skills/hermes-superpowers) references hermes-native
# adaptations instead of superpowers:<name>; a re-mirror stomps those patches.
# assert-hermes-superpowers-routing.sh re-applies them from the lock's
# superpowersRouting table. --check probes first so a fix can be logged LOUDLY:
# a fix means something rewrote the mirror since the last run, and the operator
# should know what. Soft-gated on the script existing (chezmoi ships it; a
# half-provisioned machine skips silently), exactly like the relay.sh gate.
assert_superpowers_routing() {
  local routing_script="$HOME/.local/bin/assert-hermes-superpowers-routing.sh"
  local relay_script="$HOME/.local/bin/relay.sh"
  local routing_output
  if [[ ! -x $routing_script ]]; then
    # A non-empty superpowersRouting table with no routing script is a REQUIRED
    # failure: the hermes mirror's routing patches would silently go un-asserted
    # (item 4). An empty table (or absent lock) means there is nothing to do.
    if [[ -f $CUSTOM_SKILL_LOCK ]] && jq -e '(.superpowersRouting // {} | length) > 0' "$CUSTOM_SKILL_LOCK" >/dev/null 2>&1; then
      log "WARN: assert-hermes-superpowers-routing.sh absent but superpowersRouting is non-empty; routing cannot be asserted"
      record_required_failure "superpowers routing script missing with a non-empty superpowersRouting table"
      if [[ -x $relay_script ]]; then
        "$relay_script" --agent update-skills --state prereq-missing --project hermes-superpowers \
          --detail "the routing-assert script is not deployed but superpowersRouting has entries; the mirror routing may drift" || true
      fi
    fi
    return 0
  fi
  if [[ $DRYRUN == "--dry-run" ]]; then
    "$routing_script" --dry-run || log "routing re-assert reported issues (continuing)"
    return 0
  fi
  if "$routing_script" --check >/dev/null 2>&1; then
    log "superpowers routing: clean"
    return 0
  fi
  log "ROUTING DRIFT: hermes-superpowers routing references no longer match the lock — re-asserting"
  if routing_output="$("$routing_script" 2>&1)"; then
    printf '%s\n' "$routing_output"
    log "ROUTING DRIFT: re-assert complete — something rewrote ~/.hermes/skills/hermes-superpowers (a superpowers re-mirror?); find out what stomped it"
    if [[ -x $relay_script ]]; then
      "$relay_script" --agent update-skills --state routing-drift --project hermes-superpowers \
        --detail "superpowers routing references were stomped and re-asserted from the lock; check what re-mirrored the tree" || true
    fi
  else
    printf '%s\n' "$routing_output"
    log "routing re-assert FAILED (continuing)"
    record_required_failure "superpowers routing re-assert failed"
  fi
}

# Codex policy overlays: every skill the lock's tiers table marks "on-demand"
# must carry agents/openai.yaml with allow_implicit_invocation disabled, so
# Codex loads it only on an explicit $name invocation
# (developers.openai.com/codex/skills). Chezmoi commits the overlay for vendored
# skills, but an npx-tracked skill loses it whenever the npx add/update replaces
# the folder wholesale, and cua-driver's folder is app-owned — so every run
# re-asserts the overlay from the tiers table for any on-demand skill present in
# the store. Additive by construction: the only path ever written is
# agents/openai.yaml, a file that already carries the policy line is left
# untouched (keeps chezmoi-managed copies drift-free), and when the upstream
# skill ships its own agents/openai.yaml (the official hyperframes-keyframes
# carries an interface: block there) the policy block is APPENDED so upstream
# metadata survives — never a whole-file overwrite.
assert_codex_overlays() {
  [[ -f $CUSTOM_SKILL_LOCK ]] || return 0
  local skill overlay_file
  local policy="$CODEX_POLICY"
  while IFS= read -r skill; do
    [[ -d "$STORE/$skill" ]] || continue
    # Never write through a store symlink (e.g. cua-driver -> ~/.cua-driver):
    # the target is app-owned content this repo must not modify. Such skills
    # simply carry no Codex overlay — documented asymmetry, not an oversight.
    [[ -L "$STORE/$skill" ]] && continue
    overlay_file="$STORE/$skill/agents/openai.yaml"
    if [[ -f $overlay_file ]] && grep -q 'allow_implicit_invocation: false' "$overlay_file"; then
      continue
    fi
    if [[ $DRYRUN == "--dry-run" ]]; then
      log "would assert codex overlay: $skill"
      continue
    fi
    if ! mkdir -p "$STORE/$skill/agents"; then
      record_required_failure "codex overlay dir for $skill could not be created"
      continue
    fi
    if [[ -f $overlay_file ]]; then
      if printf '\n%s\n' "$policy" >>"$overlay_file"; then
        log "appended codex overlay policy to upstream openai.yaml: $skill"
      else
        record_required_failure "codex overlay append for $skill failed"
      fi
    elif printf '%s\n' "$policy" >"$overlay_file"; then
      log "asserted codex overlay: $skill"
    else
      record_required_failure "codex overlay write for $skill failed"
    fi
  done < <(jq -r '.tiers // {} | to_entries[] | select(.value == "on-demand") | .key' "$CUSTOM_SKILL_LOCK" 2>/dev/null)
}

# Weekly hermes registry-update phase: for each specialist profile, update every
# skill the lock's hermesRegistry table marks hermes-owned for it — keyed by the
# entry's lockKey (never a list name: ClawHub slugs differ from frontmatter
# names, and hermes's own list output shows hub-linked skills as "local").
# Failure isolation is per skill AND per profile: one blocked/broken update logs
# a WARN (and relays it, soft-gated like fork drift) and the loop continues — the
# weekly run must never die on a single skill. "Blocked" output with exit 0 is a
# warning too: updates re-apply hermes's install gate on changed content, and a
# block needs operator eyes, not a silent pass. held: true entries are skipped
# visibly (none currently held). Never --force (bypassing a security scan needs
# per-invocation operator confirmation), never uninstall. Network-dependent, so
# --install-only never reaches it; the start-of-run idle-gate covers it like
# every other phase (a deferred run just means these updates land on the next
# run).
update_hermes_registry_skills() {
  [[ -f $CUSTOM_SKILL_LOCK ]] || return 0
  local relay_script="$HOME/.local/bin/relay.sh"
  if ! command -v hermes >/dev/null 2>&1; then
    # A non-empty hermesRegistry table with no hermes binary is a REQUIRED
    # failure: the hub-owned skills would silently go un-refreshed (item 4). An
    # empty table means there is nothing to do, so a missing hermes is harmless.
    if jq -e '(.hermesRegistry // {} | length) > 0' "$CUSTOM_SKILL_LOCK" >/dev/null 2>&1; then
      log "WARN: hermes not on PATH but hermesRegistry is non-empty; the registry-update phase cannot run"
      record_required_failure "hermes missing with a non-empty hermesRegistry table"
      if [[ -x $relay_script ]]; then
        "$relay_script" --agent update-skills --state prereq-missing --project hermes \
          --detail "hermes is not on PATH but hermesRegistry has hub-owned skills to refresh; they will drift" || true
      fi
    else
      log "hermes not on PATH; skipping the hermes registry-update phase (hermesRegistry is empty)"
    fi
    return 0
  fi
  local profile skill lock_key held update_output
  # Profiles to walk: every profile owning a registry skill — default included
  # (`hermes -p default` addresses Bob's root profile; un-entanglement done).
  local -a walk_profiles=()
  while IFS= read -r profile; do
    [[ -n $profile ]] && walk_profiles+=("$profile")
  done < <(jq -r '.hermesRegistry // {} | [.[].profiles[]?] | unique | .[]' "$CUSTOM_SKILL_LOCK")
  for profile in "${walk_profiles[@]}"; do
    # read on fd 3: the loop body runs hermes, which may consume stdin
    while IFS=$'\t' read -r -u3 skill lock_key held; do
      if [[ $held == "true" ]]; then
        log "hermes $profile/$skill: held — skipped (see the lock's hermesRegistry note)"
        continue
      fi
      if [[ $DRYRUN == "--dry-run" ]]; then
        log "would update via hermes -p $profile: $lock_key"
        continue
      fi
      if update_output="$(hermes -p "$profile" skills update "$lock_key" 2>&1)"; then
        if printf '%s\n' "$update_output" | grep -qiE 'blocked|refused'; then
          log "WARN: hermes $profile/$lock_key update was blocked/refused (continuing; never --force from automation)"
          record_required_failure "hermes $profile/$lock_key update blocked/refused"
          printf '%s\n' "$update_output"
          if [[ -x $relay_script ]]; then
            "$relay_script" --agent update-skills --state hermes-blocked --project "$profile/$lock_key" \
              --detail "hermes skills update was blocked/refused; decide by hand (never --force from automation)" || true
          fi
        else
          log "hermes $profile/$lock_key: ok"
        fi
      else
        log "WARN: hermes $profile/$lock_key update failed (continuing)"
        record_required_failure "hermes $profile/$lock_key update failed"
        printf '%s\n' "$update_output"
        if [[ -x $relay_script ]]; then
          "$relay_script" --agent update-skills --state hermes-update-failed --project "$profile/$lock_key" \
            --detail "hermes skills update exited non-zero; run it by hand to see why" || true
        fi
      fi
    done 3< <(jq -r --arg profile "$profile" '.hermesRegistry // {} | to_entries[]
      | select((.value.profiles // []) | index($profile) != null)
      | [.key, .value.lockKey, (.value.held // false | tostring)] | @tsv' \
      "$CUSTOM_SKILL_LOCK" 2>/dev/null)
  done
}

# Weekly app-owned skill-pack refresh: cua-driver's store entry is a SYMLINK
# into the app's own dir (~/.cua-driver/skills/cua-driver), so nothing here may
# ever write through it — the only sanctioned refresh is the app's own updater,
# `cua-driver skills update`, which re-fetches the versioned pack from GitHub
# Releases and re-plants the agent links (verified: `cua-driver skills status`
# links Claude Code, Codex — via the store — AND hermes itself). Gated on the
# store symlink existing (the roster's app-owned entry; also what keeps
# sandboxed tests off the real binary) and on the binary being on PATH
# (half-provisioned machines skip gracefully). Failure is a WARN, never fatal.
refresh_app_owned_cua_pack() {
  local refresh_output
  [[ -L "$STORE/cua-driver" ]] || return 0
  if ! command -v cua-driver >/dev/null 2>&1; then
    log "cua-driver not on PATH; skipping the app-owned skill-pack refresh"
    return 0
  fi
  if [[ $DRYRUN == "--dry-run" ]]; then
    log "would run: cua-driver skills update"
    return 0
  fi
  if refresh_output="$(cua-driver skills update 2>&1)"; then
    log "cua-driver skill pack: refreshed via the app's own updater"
  else
    log "WARN: cua-driver skills update failed (continuing)"
    printf '%s\n' "$refresh_output"
  fi
}

# A dry run makes no filesystem writes, so it does not pre-create these dirs.
[[ $DRYRUN == "--dry-run" ]] || mkdir -p "$STORE" "$CLAUDE" "$HERMES"

# serialize: one run at a time. macOS ships no dependable rename-onto-existing
# primitive from the shell (`mv` onto an existing dir moves INTO it, and macOS
# ships neither flock(1) nor a lockf(1) we depend on), so acquisition builds a
# STAGING lock dir with the owner token already inside it and publishes it via a
# rename that is a single-winner move onto the FINAL path: if the final lockdir
# is absent the staging dir renames onto it atomically (we win, token already
# inside); if the final lockdir already exists `mv` drops our staging INSIDE it,
# which we detect and clean up (we lost). This closes three defects the audit
# found: (a) two contenders cannot both validate one dead token and both proceed
# (reclaim is a single-winner move-aside); (b) a kill -0 success followed by a
# failed/empty ps is NOT declared dead (only a successful ps proving absence or a
# different start time is); (c) there is never a window where the lockdir exists
# without its owner token, so an ownerless lock can only be a legacy/corrupt one
# and is reclaimable, never a permanent wedge. We never steal by age.
LOCK_OWNER_FILE="$LOCKDIR/owner"
__update_skills_proc_start() {
  # normalized process start time for pid $1 (empty when the pid is gone)
  ps -o lstart= -p "$1" 2>/dev/null | tr -s ' ' | sed 's/^ *//;s/ *$//'
}
__update_skills_owner_token() { printf '%s\t%s' "$$" "$(__update_skills_proc_start "$$")"; }
__update_skills_owner_alive() {
  # $1 = recorded "PID<TAB>START". PROVABLY dead requires positive confirmation;
  # anything we cannot prove dead is treated as ALIVE so a contender never steals
  # from a possibly-live run.
  local rec="$1" rec_pid rec_start raw cur_start listing pid_line
  rec_pid="${rec%%$'\t'*}"
  rec_start="${rec#*$'\t'}"
  [[ $rec_pid =~ ^[0-9]+$ ]] || return 0                  # unparseable owner: do not steal
  [[ -n $rec_start && $rec_start != "$rec" ]] || return 0 # missing start field: do not steal
  if kill -0 "$rec_pid" 2>/dev/null; then
    # The pid exists. Only a SUCCESSFUL ps that returns a DIFFERENT start time
    # proves a recycle (original dead). A failed OR empty lstart lookup cannot
    # prove death, so we treat it as alive (fixes defect b).
    raw="$(ps -o lstart= -p "$rec_pid" 2>/dev/null)" || return 0
    cur_start="$(printf '%s' "$raw" | tr -s ' ' | sed 's/^ *//;s/ *$//')"
    [[ -n $cur_start ]] || return 0
    [[ $cur_start == "$rec_start" ]] # same start => alive (0); differs => recycled => dead (1)
    return
  fi
  # kill -0 failed (possibly dead, or EPERM for a foreign owner). Confirm with a
  # full listing that exits 0 whether or not the pid is present; if ps ITSELF
  # errors we cannot prove death, so treat as alive (fail safe).
  listing="$(ps -ax -o pid= 2>/dev/null)" || return 0
  while IFS= read -r pid_line; do
    pid_line="${pid_line//[[:space:]]/}"
    [[ $pid_line == "$rec_pid" ]] && return 0 # present after all: alive
  done <<<"$listing"
  return 1 # kill -0 failed AND ps ran and the pid is absent: provably dead
}
# Publish a fully-populated STAGING lock dir onto the final path. Return 0 iff we
# won (LOCKDIR is now our staging, owner token inside); 1 if the lock was held
# (mv dropped our staging inside it, or renaming onto a non-empty dir failed).
__update_skills_publish_lock() {
  local staging="$1" base
  base="${staging##*/}"
  if mv "$staging" "$LOCKDIR" 2>/dev/null; then
    if [[ -d "$LOCKDIR/$base" ]]; then
      rm -rf "${LOCKDIR:?}/${base:?}" # our staging was moved INSIDE a held lock: we lost
      return 1
    fi
    return 0
  fi
  rm -rf "$staging"
  return 1
}
# Move a stale lockdir aside to a unique name; the rename onto an absent target
# is a single-winner move, so exactly one reclaimer succeeds and then retries
# acquisition. The moved-aside dir is discarded.
__update_skills_reclaim_dead_lock() {
  local aside="${LOCKDIR}.dead.$$.${RANDOM}"
  if mv "$LOCKDIR" "$aside" 2>/dev/null; then
    rm -rf "$aside"
    return 0
  fi
  return 1
}
LOCK_MY_TOKEN="$(__update_skills_owner_token)"
__update_skills_acquire_lock() {
  local staging owner
  for _ in 1 2 3 4 5; do
    staging="$(mktemp -d "${LOCKDIR}.stage.XXXXXX")" || return 1
    printf '%s' "$LOCK_MY_TOKEN" >"$staging/owner"
    __update_skills_publish_lock "$staging" && return 0
    # The lock is held. Reclaim only from a dead or ownerless (legacy/corrupt)
    # owner; a live owner means another run is genuinely in progress.
    owner="$(cat "$LOCK_OWNER_FILE" 2>/dev/null || true)"
    if [[ -z $owner ]] || ! __update_skills_owner_alive "$owner"; then
      log "reclaiming the lock from a dead or ownerless owner (${owner:-<none>})"
      __update_skills_reclaim_dead_lock || true # lost the move-aside: just retry
      continue
    fi
    return 1 # held by a live owner
  done
  return 1
}
if [[ $DRYRUN == "--dry-run" ]]; then
  # A dry run is a READ-ONLY contention check (item 5): it never creates,
  # deletes, or reclaims lock state, and it tolerates an absent .agents parent.
  # It only reports whether a live lock would make the real run defer.
  if [[ -d $LOCKDIR ]]; then
    dry_lock_owner="$(cat "$LOCK_OWNER_FILE" 2>/dev/null || true)"
    if [[ -n $dry_lock_owner ]] && __update_skills_owner_alive "$dry_lock_owner"; then
      log "would defer: a live run holds the lock ($dry_lock_owner)"
    else
      log "would run: the existing lock has no live owner (stale or ownerless)"
    fi
  else
    log "would run: no lock is held"
  fi
else
  if ! __update_skills_acquire_lock; then
    log "another run in progress; exiting"
    exit 0
  fi
  # The EXIT trap removes the lock ONLY while we still own it: a later run that
  # reclaims a dead lock rewrites the owner file, and our trap must never delete
  # a lock we no longer hold (the three-writer race).
  __update_skills_release_lock() {
    local cur
    cur="$(cat "$LOCK_OWNER_FILE" 2>/dev/null || true)"
    [[ $cur == "$LOCK_MY_TOKEN" ]] && rm -rf "$LOCKDIR"
    return 0 # never let the EXIT trap's status leak into the script's exit code
  }
  trap '__update_skills_release_lock' EXIT
fi

# weekly success stamp: the four Monday plist slots share one ISO week; once a
# slot completes a full run this week, the remaining slots are no-ops. A deferral
# writes no stamp, so the next slot retries. FORCE and dry-run bypass; the
# install-only / check-forks-only partial runs never consult or write it.
if [[ -z $INSTALL_ONLY ]] && [[ -z $CHECK_FORKS_ONLY ]] && [[ $DRYRUN != "--dry-run" ]] &&
  [[ ${UPDATE_SKILLS_FORCE:-} != "1" ]] &&
  [[ -f $SUCCESS_STAMP && "$(cat "$SUCCESS_STAMP" 2>/dev/null)" == "$(date +%G-%V)" ]]; then
  log "weekly skills update already succeeded this week ($(cat "$SUCCESS_STAMP")); nothing to do"
  exit 0
fi

# idle-gate (activity-based, fail-closed): defer only while a harness shows
# RECENT activity (within IDLE_THRESHOLD), so the weekly run is UNATTENDED — an
# always-up bridge no longer defers it forever. A machine quiet for the window
# proceeds and swaps skills; a live turn defers to next slot. FAIL CLOSED: an
# unreadable process table or a probe error counts as active. UPDATE_SKILLS_FORCE=1
# bypasses everything (tests, manual runs). --install-only is EXEMPT: it only ADDS
# absent skills (never swaps a folder), so it is safe under a live session, this
# is what lets the fresh-machine bootstrap run --install-only unattended at apply
# time. On the last SCHEDULED slot the retry budget is spent, so a deferral there
# alerts LOUDLY rather than failing silent. See __update_skills_should_defer.
__update_skills_note_scheduled_attempt
if [[ -z $INSTALL_ONLY ]] && [[ ${UPDATE_SKILLS_FORCE:-} != "1" ]] && [[ $DRYRUN != "--dry-run" ]] && __update_skills_should_defer; then
  log "a harness showed recent activity (within IDLE_THRESHOLD), or the process table could not be read (fail-closed); deferring this run"
  if __update_skills_scheduled_budget_exhausted; then
    log "EXHAUSTED: the last scheduled retry slot for this week still deferred; the weekly skills update did not run this week"
    __update_skills_alert "Weekly skills update deferred on every scheduled slot (the machine had agent activity at all four Monday slots). Run it by hand when idle (~/.local/bin/update-skills.sh)."
  fi
  exit 0
fi

# 0) npx installs: any npx-tracked skill absent from the store is installed from
#    its upstream via the official npx `skills` CLI (latest from main). The
#    multi-agent form lands the real dir in ~/.agents/skills and plants the
#    relative Claude symlink; the weekly `npx skills update` pass below refreshes
#    present ones — this pass is install-only.
install_npx_tracked() {
  [[ -f $CUSTOM_SKILL_LOCK ]] || return 0
  local skill repo
  # read on fd 3: npx may consume stdin
  while IFS= read -r -u3 skill; do
    [[ -e "$STORE/$skill" ]] && continue # install only what's absent; refresh is the npx update pass's job
    repo="$(jq -r --arg skill "$skill" '.npxTracked[$skill].repo' "$CUSTOM_SKILL_LOCK")"
    if [[ -z $repo || $repo == "null" ]]; then
      log "install $skill: no npxTracked.repo recorded; skipping"
      continue
    fi
    if [[ $DRYRUN == "--dry-run" ]]; then
      log "would install via npx: $skill from $repo"
      continue
    fi
    if npx --yes skills@latest add "$repo" --skill "$skill" --agent claude-code --agent codex -g -y 2>&1 | tr -d '\r' | tail -3; then
      if [[ -d "$STORE/$skill" ]]; then
        log "installed: $skill from $repo" # fan-out is the convergence pass's job (below)
      else
        log "install $skill: npx add reported success but $STORE/$skill is absent (continuing)"
        record_required_failure "npx install $skill produced no store dir"
      fi
    else
      log "install $skill: npx add failed ($repo) (continuing)"
      record_required_failure "npx install $skill failed"
    fi
  done 3< <(jq -r '.npxTracked // {} | keys[]?' "$CUSTOM_SKILL_LOCK" 2>/dev/null)
}

# 0b) clawhub installs: any clawhub-tracked skill absent from the store is
#     installed from ClawHub. The CLI nests installs as <dir>/@owner/<name>
#     (v0.23.1), so each install runs in a throwaway --workdir and the nested
#     dir is moved FLAT into the store as <store>/<name>. The moved dir keeps
#     its .clawhub/origin.json (slug + ownerHandle + fingerprint), which is
#     what makes the weekly bare-name in-place update below deterministic even
#     for names several ClawHub users publish. Installing outside the store
#     workdir also keeps the store's .clawhub/lock.json free of @owner-keyed
#     phantom entries (a slug-form update against such an entry recreates the
#     nested dir instead of updating the flat one — verified live).
install_clawhub_tracked() {
  [[ -f $CUSTOM_SKILL_LOCK ]] || return 0
  jq -e '.clawhubTracked // {} | length > 0' "$CUSTOM_SKILL_LOCK" >/dev/null 2>&1 || return 0
  local relay_script="$HOME/.local/bin/relay.sh"
  if ! command -v clawhub >/dev/null 2>&1; then
    # A non-empty clawhubTracked table with no clawhub binary is a REQUIRED
    # failure: absent skills would silently never install (item 4).
    log "WARN: clawhub not on PATH but clawhubTracked is non-empty; the install pass cannot run"
    record_required_failure "clawhub missing with a non-empty clawhubTracked table (install pass)"
    if [[ -x $relay_script ]]; then
      "$relay_script" --agent update-skills --state prereq-missing --project clawhub \
        --detail "clawhub is not on PATH but clawhubTracked has skills to install; the store cannot be completed" || true
    fi
    return 0
  fi
  local skill slug registry tmp_workdir installed_dir
  local -a clawhub_cmd
  # read on fd 3: clawhub may consume stdin
  while IFS=$'\t' read -r -u3 skill slug registry; do
    [[ -e "$STORE/$skill" ]] && continue # install only what's absent; refresh is the update pass's job
    if [[ -z $slug ]]; then
      log "install $skill: no clawhubTracked.slug recorded; skipping"
      continue
    fi
    if [[ $DRYRUN == "--dry-run" ]]; then
      log "would install via clawhub: $skill from $slug"
      continue
    fi
    tmp_workdir="$(mktemp -d)"
    clawhub_cmd=(clawhub --no-input --workdir "$tmp_workdir" --dir skills)
    [[ -n $registry ]] && clawhub_cmd+=(--registry "$registry")
    if "${clawhub_cmd[@]}" install "$slug" 2>&1 | tail -2; then
      # Nested @owner layout today; tolerate a flat layout should a future CLI
      # version stop nesting.
      installed_dir="$tmp_workdir/skills/$slug"
      [[ -d $installed_dir ]] || installed_dir="$tmp_workdir/skills/$skill"
      if [[ -d $installed_dir ]]; then
        mv "$installed_dir" "$STORE/$skill"
        log "installed: $skill from $slug" # fan-out is the convergence pass's job (below)
      else
        log "install $skill: clawhub install reported success but produced no skill dir (continuing)"
        record_required_failure "clawhub install $skill produced no store dir"
      fi
    else
      log "install $skill: clawhub install failed ($slug) (continuing)"
      record_required_failure "clawhub install $skill failed"
    fi
    rm -rf "$tmp_workdir"
  done 3< <(jq -r '.clawhubTracked // {} | to_entries[]
    | [.key, (.value.slug // ""), (.value.registry // "")] | @tsv' \
    "$CUSTOM_SKILL_LOCK" 2>/dev/null)
}

# Weekly clawhub update pass: refresh every clawhub-tracked store copy in
# place — per skill, by its bare store-dir name, with the store as the CLI's
# workdir (origin.json pins the owner, so bare names resolve deterministically).
# Two mechanical realities handled up front (both verified live on v0.23.1):
# Finder .DS_Store litter breaks the CLI's fingerprint match, so it is scrubbed
# first; and the repo-asserted Codex overlay (agents/openai.yaml) is a local
# file the published fingerprint cannot know, so the CLI refuses with "local
# changes". When the refusal is over EXACTLY that file — byte-equal to the
# policy this script asserts — the file is set aside and the update retried
# once (assert_codex_overlays re-creates it later in the same run). Any other
# local change is a loud WARN (relayed), never overwritten: automation never
# passes --force (and never --force-install, ClawHub's scan bypass — bypassing
# a security scan needs per-invocation operator confirmation).
update_clawhub_tracked() {
  [[ -f $CUSTOM_SKILL_LOCK ]] || return 0
  jq -e '.clawhubTracked // {} | length > 0' "$CUSTOM_SKILL_LOCK" >/dev/null 2>&1 || return 0
  local relay_script="$HOME/.local/bin/relay.sh"
  if ! command -v clawhub >/dev/null 2>&1; then
    # A non-empty clawhubTracked table with no clawhub binary is a REQUIRED
    # failure: the tracked store copies would silently go un-refreshed (item 4).
    log "WARN: clawhub not on PATH but clawhubTracked is non-empty; the update pass cannot run"
    record_required_failure "clawhub missing with a non-empty clawhubTracked table (update pass)"
    if [[ -x $relay_script ]]; then
      "$relay_script" --agent update-skills --state prereq-missing --project clawhub \
        --detail "clawhub is not on PATH but clawhubTracked has skills to refresh; they will drift" || true
    fi
    return 0
  fi
  local skill overlay_file update_output retry_output
  # read on fd 3: clawhub may consume stdin
  while IFS= read -r -u3 skill; do
    # Only real store dirs: absent skills are the install pass's job, and a
    # symlinked entry would be app-owned content this script must not touch.
    [[ -d "$STORE/$skill" && ! -L "$STORE/$skill" ]] || continue
    if [[ $DRYRUN == "--dry-run" ]]; then
      log "would update via clawhub: $skill"
      continue
    fi
    rm -f "$STORE/$skill/.DS_Store"
    if ! update_output="$(clawhub --no-input --workdir "$AGENTS" --dir skills update "$skill" 2>&1)"; then
      log "WARN: clawhub update $skill failed (continuing)"
      record_required_failure "clawhub update $skill failed"
      printf '%s\n' "$update_output"
      if [[ -x $relay_script ]]; then
        "$relay_script" --agent update-skills --state clawhub-update-failed --project "$skill" \
          --detail "clawhub update exited non-zero; run it by hand to see why" || true
      fi
      continue
    fi
    if ! printf '%s\n' "$update_output" | grep -q 'local changes'; then
      log "clawhub $skill: ok"
      continue
    fi
    # Refusal ladder: only when the sole local change is our own overlay.
    overlay_file="$STORE/$skill/agents/openai.yaml"
    if [[ -f $overlay_file && "$(<"$overlay_file")" == "$CODEX_POLICY" ]]; then
      rm "$overlay_file"
      rmdir "$STORE/$skill/agents" 2>/dev/null || true
      if retry_output="$(clawhub --no-input --workdir "$AGENTS" --dir skills update "$skill" 2>&1)" &&
        ! printf '%s\n' "$retry_output" | grep -q 'local changes'; then
        log "clawhub $skill: ok (repo-asserted Codex overlay set aside; re-asserted below)"
        continue
      fi
      printf '%s\n' "$retry_output"
      # Not just our overlay after all — put it back and surface the refusal.
      mkdir -p "$STORE/$skill/agents"
      printf '%s\n' "$CODEX_POLICY" >"$overlay_file"
    fi
    log "WARN: clawhub $skill update refused over local changes (continuing; never --force from automation)"
    record_required_failure "clawhub update $skill refused over local changes"
    printf '%s\n' "$update_output"
    if [[ -x $relay_script ]]; then
      "$relay_script" --agent update-skills --state clawhub-blocked --project "$skill" \
        --detail "clawhub update refused over local changes beyond the repo's own Codex overlay; reconcile by hand (never --force from automation)" || true
    fi
  done 3< <(jq -r '.clawhubTracked // {} | keys[]?' "$CUSTOM_SKILL_LOCK" 2>/dev/null)
}

# fork/vendored upstream drift-check: for each lock forks entry, fetch the
# upstream and compare the recorded skill path's current git hash (tree hash for
# a folder, blob hash for a single-file skill like herdr's root SKILL.md)
# against lastComparedTreeHash — the hash at the last HUMAN comparison. Drift
# means the upstream shipped changes nobody has reviewed against the local copy
# yet: alert and move on. This pass only ever reads; the vendored store content
# is untouchable here by construction (nothing below writes to $STORE). An
# unreachable upstream is a logged warning, never a failure — the weekly run
# must survive a dead network.
notify_fork_drift() {
  local fork="$1" source_url="$2"
  local relay_script="$HOME/.local/bin/relay.sh"
  log "FORK DRIFT: $fork — upstream $source_url has changed since the last comparison"
  log "FORK DRIFT: compare upstream and port wanted changes into the vendored copy by hand (see CLAUDE.md, Agent Skills), then set forks[\"$fork\"].lastComparedTreeHash to the new upstream hash; the vendored copy itself was not modified"
  # Soft-gate on relay.sh, exactly like the pre-commit hook's gitleaks stage:
  # relay lands in a later slice, so its absence is a silent skip, not an error.
  if [[ -x $relay_script ]]; then
    "$relay_script" --agent update-skills --state fork-drift --project "$fork" \
      --detail "upstream $source_url changed since the last comparison; compare and port wanted changes by hand, then bump lastComparedTreeHash" || true
  fi
}

check_fork_drift() {
  [[ -f $CUSTOM_SKILL_LOCK ]] || return 0
  local fork source_url skill_path last_compared_tree_hash current_tree_hash clone_dir
  # read on fd 3: the loop body runs git, which may consume stdin
  while IFS= read -r -u3 fork; do
    source_url="$(jq -r ".forks[\"$fork\"].sourceUrl" "$CUSTOM_SKILL_LOCK")"
    skill_path="$(jq -r ".forks[\"$fork\"].skillPath" "$CUSTOM_SKILL_LOCK")"
    last_compared_tree_hash="$(jq -r ".forks[\"$fork\"].lastComparedTreeHash" "$CUSTOM_SKILL_LOCK")"
    if [[ $DRYRUN == "--dry-run" ]]; then
      log "would drift-check fork: $fork against $source_url"
      continue
    fi
    clone_dir="$(mktemp -d)"
    # --depth 1 suffices: only HEAD's tree is compared, never history
    if ! git clone --quiet --depth 1 "$source_url" "$clone_dir/repo" 2>/dev/null; then
      log "fork drift-check $fork: upstream unreachable ($source_url); skipping"
      rm -rf "$clone_dir"
      continue
    fi
    if [[ $skill_path == "." ]]; then
      current_tree_hash="$(git -C "$clone_dir/repo" rev-parse 'HEAD^{tree}')"
    else
      current_tree_hash="$(git -C "$clone_dir/repo" rev-parse "HEAD:$skill_path" 2>/dev/null || echo missing-path)"
    fi
    rm -rf "$clone_dir"
    if [[ $current_tree_hash == "$last_compared_tree_hash" ]]; then
      log "fork $fork: upstream unchanged since the last comparison"
    else
      notify_fork_drift "$fork" "$source_url"
    fi
  done 3< <(jq -r '.forks|keys[]?' "$CUSTOM_SKILL_LOCK" 2>/dev/null)
}

if [[ -n $CHECK_FORKS_ONLY ]]; then
  log "fork drift-check"
  check_fork_drift
  log "done (check-forks-only)${DRYRUN:+ (dry-run)}"
  exit 0
fi

log "npx installs (absent skills)"
install_npx_tracked

log "clawhub installs (absent skills)"
install_clawhub_tracked

if [[ -n $INSTALL_ONLY ]]; then
  converge_claude_skills
  converge_hermes_skills
  assert_codex_overlays
  assert_superpowers_routing
  log "done (install-only)${DRYRUN:+ (dry-run)}"
  # Signal any required-phase failure to the caller (the first-install
  # chezmoiscript keys its retry marker on this non-zero exit). A dry run only
  # reports, so it never fails.
  if [[ $DRYRUN != "--dry-run" && $REQUIRED_FAILURES -gt 0 ]]; then
    log "install-only finished with $REQUIRED_FAILURES required-phase failure(s)"
    exit 1
  fi
  exit 0
fi

# 1) refresh every npx-tracked store skill in place from its upstream
if [[ $DRYRUN == "--dry-run" ]]; then
  log "would run: npx skills update --global"
else
  log "npx skills update --global"
  if ! npx --yes skills@latest update --global -y 2>&1 | tr -d '\r' | tail -3; then
    log "npx update reported issues (continuing)"
    record_required_failure "npx skills update failed"
  fi
fi

# 1b) refresh every clawhub-tracked store skill in place from its ClawHub
#     upstream (see update_clawhub_tracked above — bare store names, refusal
#     ladder for the repo-asserted overlay, never --force)
log "clawhub updates (tracked skills)"
update_clawhub_tracked

# 1c) refresh the app-owned cua-driver skill pack via the app's own updater
#     (see refresh_app_owned_cua_pack above — never a write through the symlink)
refresh_app_owned_cua_pack

# 2) CONVERGE the fan-out: every store skill is symlinked into Claude, and into
#    exactly the hermes profile skills dirs its hermesProfiles mapping names,
#    creating missing links, repairing wrong/dangling targets, and removing
#    updater-owned links that drifted out of the desired set
converge_claude_skills
converge_hermes_skills

# 3) re-assert Codex policy overlays for on-demand skills — AFTER the npx
#    refresh above, which replaces npx-tracked skill folders (overlay and all)
assert_codex_overlays

# 4) re-assert the superpowers→hermes routing patches on the hermes mirror —
#    like the Codex overlays, a property that must survive anything replacing
#    files wholesale (here: a superpowers re-mirror)
assert_superpowers_routing

# 5) hermes registry-update phase — after the store refresh, so the run
#    summary reflects final state for both lanes (independent sources)
log "hermes registry updates"
update_hermes_registry_skills

# 6) watch the vendored/fork upstreams (alert-only; see the function above)
log "fork drift-check"
check_fork_drift

# Record this week's success ONLY when zero required phases failed. The stamp is
# an ISO year-week key (date +%G-%V): %G is the ISO week-numbering YEAR and %V is
# the ISO week (01-53), so the four Monday slots share one key and a slot no-ops
# once one has fully succeeded this week. %G (not %Y) is what keeps a year-
# boundary week correct: the days of ISO week 01 that fall in late December carry
# the NEXT year's %G, and the late-December days of week 52/53 carry the current
# %G, so the key never collides or splits across the boundary (52/53/01 verified).
# When a required phase failed we WITHHOLD the stamp, so a later scheduled slot
# retries; and for a scheduled run with no slot remaining this week we alert (the
# retry budget is spent). A dry run records nothing.
if [[ $DRYRUN != "--dry-run" ]]; then
  if [[ $REQUIRED_FAILURES -eq 0 ]]; then
    mkdir -p "$STATE_DIR"
    date +%G-%V >"$SUCCESS_STAMP"
  else
    log "WITHHOLDING the weekly success stamp: $REQUIRED_FAILURES required-phase failure(s) this run; a later scheduled slot will retry"
    if __update_skills_scheduled_budget_exhausted; then
      log "EXHAUSTED: required-phase failures on the last scheduled slot for this week; the weekly skills update did not fully succeed this week"
      __update_skills_alert "Weekly skills update finished with $REQUIRED_FAILURES required-phase failure(s) and no scheduled slot remains this week. Check ~/.local/log/skills/."
    fi
  fi
fi

log "done${DRYRUN:+ (dry-run)}"
