#!/usr/bin/env bash
# update-skills: keep the canonical skills store (~/.agents/skills) complete and
# fresh via the GENERATION-EXCHANGE model.
#
# The store holds exactly the roster this repo declares (see
# ~/.agents/custom-skill-lock.json), so the registered-skill count in the
# harnesses does not grow when this runs. Every npx- and clawhub-tracked skill
# lives inside ONE live generation directory, ~/.agents/.skills-current (real
# dirs under skills/, the npx CLI lock .skill-lock.json, and generation.json as
# the ready marker); the store names ~/.agents/skills/<name> are stable literal
# symlinks into it, and ~/.agents/.skill-lock.json is a symlink to its lock.
# The weekly run builds a CANDIDATE generation as a fake HOME under
# ~/.agents/.skills-generations/<id>/home, runs the package-CLI lanes against it
# under env -i (HOME/XDG/TMPDIR/npm cache pinned inside the candidate), validates
# the whole candidate, and publishes with ONE atomic renameat2 RENAME_EXCHANGE
# (GNU mv --exchange --no-copy -T). Exactly one previous generation is retained.
#
# HONEST GUARANTEE: per-lookup completeness and cross-skill coherence per
# generation: any path resolution during or after the exchange yields a
# complete tree from exactly one generation. A session that cached a CANONICAL
# (resolved) path keeps a complete previous generation for at least a week (one
# retained generation); after pruning it gets a clean ENOENT, never partial
# content. Out-of-band writers (the HyperFrames workflows self-update the store
# via `npx hyperframes skills update`, upstream-controlled, no supported
# disable) bypass any local design exactly as they do today; the weekly run
# detects that drift in recovery and re-absorbs it into the next candidate. OUR
# updater's own operations are atomic end to end.
#
# The roster's provenance kinds, and who refreshes each:
#   - npx-tracked (npxTracked table): installed and refreshed by the official
#      npx `skills` CLI from an official upstream, latest from main (no pin).
#      The build lane runs an explicit `npx skills add <repo> --skill <name>
#      --agent claude-code --agent codex -g -y` per repo group against the
#      CANDIDATE (never the bulk `skills update`, whose lock-walk logs some
#      failures at exit 0), which also reconciles lock-absent roster skills.
#   - clawhub-tracked (clawhubTracked table): installed and refreshed by the
#      `clawhub` CLI from a ClawHub upstream (npx cannot source ClawHub;
#      `npx skills add` is GitHub-only). The lane installs an absent skill in a
#      throwaway --workdir and moves the CLI's nested @owner/<name> output flat
#      into the candidate store (its .clawhub/origin.json travels along and
#      pins the owner), then refreshes present ones in place with a bare
#      `clawhub --workdir <candidate>/.agents --dir skills update <name>`. See
#      __gen_lane_clawhub for the local-changes refusal ladder.
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
#   --dry-run           read-only preview: NEVER invokes either package CLI (the
#                       npx CLI treats `update --help` as a real update, observed
#                       live), zero writes; reports roster-vs-lock and
#                       roster-vs-generation drift, the fan-out convergence
#                       preview, and would-run/would-defer
#   --install-only      ADDITIVE bootstrap: build and publish a candidate whose
#                       EXISTING skills are byte-clones of current (no updates)
#                       plus genuinely absent roster skills added; never migrates
#                       a flat store, never replaces existing store content; the
#                       fan-out CREATES missing links only (used by tests and the
#                       fresh-machine apply-time bootstrap)
#   --check-forks-only  run only the fork/vendored upstream drift-check
#   --scheduled         mark this as a LaunchAgent (scheduled) run; only a
#                       scheduled run with no later slot remaining this week
#                       claims retry-budget exhaustion (a manual run never does)
#   --build-lanes       INTERNAL: this process is the env -i sub-invocation that
#                       runs the build lanes inside a candidate fake HOME
# Env: UPDATE_SKILLS_FORCE=1 bypasses the idle-gate AND the weekly success stamp.
#      The idle-gate (activity-based, fail-closed) makes this script refuse to
#      swap skill folders only while a harness (claude/codex/hermes) shows RECENT
#      activity, meaning the newest mtime among its per-turn activity files is within
#      IDLE_THRESHOLD (default 15 min). It runs UNATTENDED: an always-up bridge no
#      longer defers the weekly run forever; a machine quiet for the window
#      proceeds (see __update_skills_should_defer). Override the window with
#      UPDATE_SKILLS_IDLE_THRESHOLD (seconds) and each harness's probe dir with
#      UPDATE_SKILLS_{CLAUDE,CODEX,HERMES}_ACTIVITY_DIR (tests do). The weekly run
#      is scheduled across 24 hourly Monday slots; a per-week success stamp
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
# The roster (desired state) this repo wants, deployed by chezmoi. Normally
# ~/.agents/custom-skill-lock.json; the --build-lanes sub-invocation (run inside
# a candidate fake HOME) is handed the REAL lock path via UPDATE_SKILLS_LOCK_PATH
# so it reads the desired roster while writing only into the candidate.
CUSTOM_SKILL_LOCK="${UPDATE_SKILLS_LOCK_PATH:-$AGENTS/custom-skill-lock.json}"
CLAUDE="$HOME/.claude/skills"
HERMES="$HOME/.hermes/skills"            # the default profile (Bob)
HERMES_PROFILES="$HOME/.hermes/profiles" # specialist profiles: <name>/skills
LOCKFILE="$AGENTS/.update-skills.lock"
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
# (GNU mv --exchange --no-copy -T), so any lookup during or after the swap yields a
# complete tree from exactly one generation. Candidate generations are built as a
# fake HOME under .skills-generations/<id>/home, on the SAME device as
# .skills-current so the same-filesystem exchange works. Exactly one previous
# generation is retained (a session that cached a resolved path keeps a complete
# tree for at least a week); older ones are garbage-renamed then deleted.
SKILLS_CURRENT="$AGENTS/.skills-current"
GENERATIONS="$AGENTS/.skills-generations"
SKILL_LOCK_LINK="$AGENTS/.skill-lock.json"
GENERATION_META_NAME="generation.json"
# The exchange tool (a GNU coreutils mv with a working --exchange; BSD /bin/mv
# lacks it) is resolved at RUN TIME by __gen_resolve_exchange_tool, never a
# hardcoded host path: a macOS host carries it as Homebrew's gmv, while the Nix
# devshell (CI) provides GNU mv as plain mv and has no /opt/homebrew. Candidate
# order is the UPDATE_SKILLS_GMV override (tests), then gmv, then mv on PATH; a
# candidate is accepted only when --version says GNU coreutils AND a functional
# probe swap succeeds. The accepted tool is cached here for the rest of the
# run; empty means not resolved yet.
GEN_EXCHANGE_TOOL=""
# This script's own path, for the env -i re-invocation that runs the build lanes
# inside a candidate fake HOME (see __gen_run_lanes / --build-lanes).
UPDATE_SKILLS_SELF="${BASH_SOURCE[0]}"
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
# The plist fires 24 hourly Monday retry slots (00:00..23:00; see
# Library/LaunchAgents/com.webdavis.update-skills.plist.tmpl). This is the hour
# of the LAST slot: a scheduled deferral at/after it, or a coalesced catch-up on
# a later weekday, means the weekly retry budget is exhausted, so the run alerts
# LOUDLY instead of failing silent. Keep in sync with the plist.
readonly UPDATE_SKILLS_LAST_SLOT_HOUR="23"
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
BUILD_LANES="" # internal: run the generation build lanes inside a candidate HOME
for arg in "$@"; do
  case "$arg" in
    --dry-run) DRYRUN="--dry-run" ;;
    --install-only) INSTALL_ONLY=1 ;;
    --check-forks-only) CHECK_FORKS_ONLY=1 ;;
    --scheduled) SCHEDULED=1 ;;
    --build-lanes) BUILD_LANES=1 ;;
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
# deferred run. The plist fires 24 hourly Monday slots (00..23); launchd may
# COALESCE a missed slot and deliver it on a later day (a catch-up), which is
# also the week's last scheduled chance. So a later slot remains ONLY when today
# is Monday BEFORE the last slot hour; Monday at/after 23:00, or any later
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

# The weekly success stamp value: the ISO year-week PLUS the custom-lock hash and
# the updater hash. A roster change (custom-lock) or an updater change after a
# Monday success no longer matches the stamp, so the week UN-STAMPS and a later
# slot rebuilds. The stamp thus means "this EXACT desired state already succeeded
# this week", not merely "some run succeeded this week" (brief: stamp inputs).
__update_skills_stamp_value() {
  printf '%s %s %s' "$(date +%G-%V)" "$(__gen_custom_lock_hash)" "$(__gen_updater_hash)"
}

# A sortable, collision-resistant generation id: epoch seconds + pid + random.
# Sortable-by-time is what lets prune keep the newest previous and delete older.
__gen_new_id() { printf '%s-%s-%s' "$(date +%s)" "$$" "${RANDOM}${RANDOM}"; }

# Two paths are on the same filesystem (renameat2 RENAME_EXCHANGE needs that).
# %d is the device number, but the flag spelling differs by stat flavor: GNU
# stat takes -c (its -f means file-SYSTEM status, whose %d is the format code;
# comparing that reports same-device paths as different); BSD stat takes -f.
# Probe the GNU spelling first, fall back to BSD. ADVISORY ONLY: callers
# attempt the exchange regardless and treat its outcome as authoritative; this
# check only shapes the pre-flight warning.
__gen_same_device() {
  local a b
  if a="$(stat -c %d "$1" 2>/dev/null)"; then
    b="$(stat -c %d "$2" 2>/dev/null)" || return 1
  else
    a="$(stat -f %d "$1" 2>/dev/null)" || return 1
    b="$(stat -f %d "$2" 2>/dev/null)" || return 1
  fi
  [[ -n $a && $a == "$b" ]]
}

# A candidate exchange tool is capable iff --version says GNU coreutils AND a
# real probe swap in a private temp dir succeeds (--exchange --no-copy -T with
# the swapped content verified). The functional probe is the authority: a GNU
# mv too old for --exchange, or a filesystem without atomic-swap support, both
# fail here and the candidate is rejected.
__gen_exchange_tool_capable() {
  local tool="$1" probe rc=1
  command -v "$tool" >/dev/null 2>&1 || return 1
  "$tool" --version 2>/dev/null | head -1 | grep -q 'GNU coreutils' || return 1
  probe="$(mktemp -d)" || return 1
  mkdir -p "$probe/a" "$probe/b" || {
    rm -rf "$probe"
    return 1
  }
  printf 'a' >"$probe/a/marker"
  printf 'b' >"$probe/b/marker"
  if "$tool" --exchange --no-copy -T "$probe/a" "$probe/b" 2>/dev/null &&
    [[ "$(cat "$probe/a/marker" 2>/dev/null)" == "b" &&
    "$(cat "$probe/b/marker" 2>/dev/null)" == "a" ]]; then
    rc=0
  fi
  rm -rf "$probe"
  return $rc
}

# Resolve (and cache for this run) the exchange tool: the UPDATE_SKILLS_GMV
# override first, then gmv, then mv on PATH. Returns 1 (cache left empty) when
# no capable tool exists; callers then fail LOUDLY, never partially.
__gen_resolve_exchange_tool() {
  [[ -n $GEN_EXCHANGE_TOOL ]] && return 0
  local candidate
  for candidate in ${UPDATE_SKILLS_GMV:+"$UPDATE_SKILLS_GMV"} gmv mv; do
    if __gen_exchange_tool_capable "$candidate"; then
      GEN_EXCHANGE_TOOL="$candidate"
      return 0
    fi
  done
  return 1
}

# THE atomic swap primitive: renameat2 RENAME_EXCHANGE via the resolved GNU mv.
# Logs loudly and returns 1 when no capable tool exists. --no-copy guarantees a
# cross-device attempt fails cleanly instead of degrading to a partial copy, so
# a non-zero return always means "nothing changed".
#   __gen_exchange <path-a> <path-b>
__gen_exchange() {
  if ! __gen_resolve_exchange_tool; then
    log "exchange: no GNU coreutils mv with a working --exchange on PATH (tried:${UPDATE_SKILLS_GMV:+ $UPDATE_SKILLS_GMV,} gmv, mv)"
    return 1
  fi
  "$GEN_EXCHANGE_TOOL" --exchange --no-copy -T "$1" "$2" 2>/dev/null
}

# Write generation.json LAST, as the ready marker. Its presence + matching
# hashes is what recovery uses to tell a complete candidate from a leftover, and
# its buildMode ("full" | "additive") records whether the lanes ran a FULL
# refresh or an ADDITIVE (install-only) build, so weekly recovery never reuses an
# additive candidate as a weekly refresh (an additive clone carries stale
# byte-copies of the existing skills). Defaults to "full" when the caller does
# not specify a mode (migration and the reuse fixtures build complete full
# generations).
#   __gen_write_meta <generation-dir> <id> [build-mode]
__gen_write_meta() {
  local dir="$1" id="$2" build_mode="${3:-full}" meta="$1/$GENERATION_META_NAME"
  jq -n \
    --arg id "$id" \
    --arg createdAt "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    --arg customLockHash "$(__gen_custom_lock_hash)" \
    --arg updaterHash "$(__gen_updater_hash)" \
    --arg buildMode "$build_mode" \
    '{id: $id, createdAt: $createdAt, customLockHash: $customLockHash, updaterHash: $updaterHash, buildMode: $buildMode}' \
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

# Post-publish reconciliation for a re-absorbed competing-writer name: the store
# still holds the redundant real dir (its content was cloned into the now-live
# generation), so garbage-destroy it and plant the stable store symlink. The
# content is preserved in the generation, so this is non-destructive.
__gen_absorb_store_link() {
  local name="$1"
  local link="$STORE/$name"
  if [[ -d $link && ! -L $link ]]; then
    __gen_garbage_destroy "$link"
  fi
  __gen_plant_store_link "$name"
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
  __gen_is_complete "$candidate" || {
    log "publish: candidate $candidate is not complete (no ready marker)"
    return 1
  }
  # First publish on a machine with no live generation yet (fresh bootstrap):
  # a plain rename onto the absent path is atomic and there is no previous
  # generation to retain.
  if [[ ! -e $SKILLS_CURRENT && ! -L $SKILLS_CURRENT ]]; then
    mkdir -p "$AGENTS"
    if mv "$candidate" "$SKILLS_CURRENT" 2>/dev/null; then
      return 0
    fi
    log "publish: could not rename the candidate onto the absent $SKILLS_CURRENT"
    return 1
  fi
  [[ -d $SKILLS_CURRENT && ! -L $SKILLS_CURRENT ]] || {
    log "publish: $SKILLS_CURRENT is not a real directory"
    return 1
  }
  # Pre-flight ADVISORY only: warn on an apparent device mismatch, but let the
  # exchange itself be the authority (--no-copy makes a cross-device attempt a
  # clean failure, never a partial operation).
  __gen_same_device "$candidate" "$SKILLS_CURRENT" ||
    log "publish: WARN candidate and .skills-current look like different devices; attempting the exchange anyway"
  old_id="$(__gen_meta_field "$SKILLS_CURRENT" id)"
  [[ -n $old_id ]] || old_id="pre-$(__gen_new_id)" # a first-migrated current may predate meta
  local retained="$GENERATIONS/$old_id"
  # R2-3c: refuse a retention path that CONTAINS the candidate, BEFORE the
  # exchange. If the live generation's id equals the candidate's workspace id
  # (the post-exchange crash signature), retaining the displaced previous
  # generation at $GENERATIONS/<old_id> would garbage-destroy the workspace
  # that holds the very generation the exchange just published. Refusing here
  # leaves the live generation genuinely untouched.
  case "$candidate/" in
    "$retained"/*)
      log "publish: FATAL the retention path $retained contains the candidate; refusing to publish (live generation untouched)"
      return 1
      ;;
  esac
  # R2-3b: record the in-flight exchange BEFORE it lands, so a crash anywhere
  # in this window is disambiguated by recovery (marker + live id). An
  # unwritable marker refuses the publish while the live generation is still
  # untouched (fail closed).
  mkdir -p "$GENERATIONS"
  local marker="$GENERATIONS/$GEN_EXCHANGE_MARKER_NAME"
  local candidate_workspace_id
  candidate_workspace_id="$(__gen_meta_field "$candidate" id)"
  if ! jq -n --arg oldId "$old_id" --arg workspaceId "$candidate_workspace_id" \
    '{oldId: $oldId, workspaceId: $workspaceId}' >"$marker" 2>/dev/null; then
    log "publish: FATAL could not write the exchange-in-flight marker; refusing to publish (live generation untouched)"
    return 1
  fi
  # THE atomic publish: renameat2 RENAME_EXCHANGE. After it, .skills-current is
  # the new generation and $candidate holds the complete PREVIOUS generation.
  if ! __gen_exchange "$candidate" "$SKILLS_CURRENT"; then
    log "publish: atomic exchange failed; live generation untouched"
    rm -f "$marker"
    return 1
  fi
  # Retain the displaced previous generation under its id (garbage-destroy any
  # name collision first so the rename lands cleanly). R2-3d: a retention
  # failure is FATAL: the exchange landed (the refreshed generation IS live),
  # but the transaction did not complete, so no success is reported and the
  # caller records a required failure (no stamp). The marker stays for
  # recovery to finish the cleanup.
  __gen_garbage_destroy "$retained"
  if ! mv "$candidate" "$retained" 2>/dev/null; then
    log "publish: FATAL the displaced previous generation could not be retained; the refreshed generation is live but this run reports failure (no stamp)"
    return 1
  fi
  __gen_prune_generations "$old_id"
  rm -f "$marker"
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

# ---------------------------------------------------------------------------
# FAIL-CLOSED roster gate (R2-2). The roster lock is the authority on what the
# generation should hold; if it is missing, unparseable, or schema-broken, the
# empty tracked set it degrades to would make the candidate builder drop every
# skill, validation pass on zero names, and the delist pruner remove every
# store link: an EMPTY publication stamped as success. So before ANY candidate
# mutation the run VALIDATES the lock and SNAPSHOTS it to a run-private copy;
# every later read in the transaction goes through the snapshot, and the LIVE
# lock's hash is re-checked against the snapshot before publish and before
# stamping (a mid-run chezmoi apply must not publish a candidate built from
# the old roster, nor stamp the week for a roster that changed underneath).
# ---------------------------------------------------------------------------
GEN_ROSTER_SOURCE=""        # the real deployed lock path (hash re-checks read this)
GEN_ROSTER_SNAPSHOT_FILE="" # the run-private snapshot (all roster reads go here)
GEN_ROSTER_HASH=""          # sha256 of the snapshot at run start

# Minimal structural schema: a top-level object whose tracked tables (and the
# tiers table) are objects when present. A wrong-typed table would make the
# jq key-walks silently yield nothing, which is exactly the degraded-empty
# failure this gate exists to refuse.
__gen_roster_schema_ok() {
  jq -e '(type == "object")
    and ((.npxTracked // {}) | type == "object")
    and ((.clawhubTracked // {}) | type == "object")
    and ((.tiers // {}) | type == "object")' "$1" >/dev/null 2>&1
}

# Validate the live roster lock and snapshot it for the transaction. On
# success CUSTOM_SKILL_LOCK points at the snapshot (so the candidate build,
# validation, lanes, and fan-out all read one immutable roster) and
# GEN_ROSTER_HASH records its content hash. Any validation step failing, or
# the live lock changing while being copied, is a refused run (caller fails
# closed; the live store and generation are untouched).
__gen_snapshot_roster() {
  GEN_ROSTER_SOURCE="$CUSTOM_SKILL_LOCK"
  if [[ ! -f $CUSTOM_SKILL_LOCK ]]; then
    log "roster gate: $CUSTOM_SKILL_LOCK is missing; refusing to treat an absent roster as 'no skills wanted'"
    return 1
  fi
  if ! __gen_roster_schema_ok "$CUSTOM_SKILL_LOCK"; then
    log "roster gate: $CUSTOM_SKILL_LOCK is unparseable or schema-broken; refusing to build from a degraded-empty roster"
    return 1
  fi
  local source_hash snapshot
  source_hash="$(__gen_hash_file "$CUSTOM_SKILL_LOCK")"
  snapshot="$(mktemp "${TMPDIR:-/tmp}/update-skills-roster.XXXXXX")" || return 1
  if ! cp "$CUSTOM_SKILL_LOCK" "$snapshot"; then
    rm -f "$snapshot"
    return 1
  fi
  # Torn-copy guard: the snapshot must re-validate and hash-match the source
  # as it was read; a concurrent writer mid-copy is a refused run.
  if ! __gen_roster_schema_ok "$snapshot" ||
    [[ "$(__gen_hash_file "$snapshot")" != "$source_hash" ]]; then
    log "roster gate: the roster lock changed while being snapshotted; refusing this run"
    rm -f "$snapshot"
    return 1
  fi
  GEN_ROSTER_SNAPSHOT_FILE="$snapshot"
  GEN_ROSTER_HASH="$source_hash"
  CUSTOM_SKILL_LOCK="$snapshot"
  return 0
}

# True while the LIVE roster lock is still byte-identical to the run-start
# snapshot. Publish and stamp are gated on this; with no snapshot taken (a
# mode that never mutates), it passes vacuously.
__gen_roster_unchanged() {
  [[ -n $GEN_ROSTER_HASH ]] || return 0
  [[ "$(__gen_hash_file "$GEN_ROSTER_SOURCE")" == "$GEN_ROSTER_HASH" ]]
}

# True when <name> is a currently-tracked generation skill (npx or clawhub).
# The tracked set is the roster's authority on what the generation should hold;
# a name that has been DELISTED from the lock is no longer tracked and must not
# be carried forward into a new candidate or left live in the store.
__gen_name_is_tracked() {
  local query="$1" tracked_name
  while IFS= read -r tracked_name; do
    [[ $tracked_name == "$query" ]] && return 0
  done < <(__gen_tracked_names)
  return 1
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
#   GEN_REABSORB[]      = tracked names whose store entry is a REAL DIR where a
#                         link is expected (a competing writer, e.g. the
#                         HyperFrames self-updater, or an interrupted migration):
#                         re-absorb that content into this run's candidate.
#   GEN_REUSE_CANDIDATE = a complete, unpublished candidate whose generation.json
#                         matches the current desired state: the main flow may
#                         publish it instead of rebuilding.
# The self-healed cases: incomplete staging leftovers are garbage-destroyed;
# published-generation link drift (stale .skill-lock.json link or store links)
# is repaired; partial-prune garbage is swept; retained generations beyond the
# newest one are pruned.
# ---------------------------------------------------------------------------
GEN_REABSORB=()
GEN_REUSE_CANDIDATE=""
# The exchange-in-flight marker (R2-3b): written by __gen_publish just before
# the atomic exchange, removed after retention completes. Its presence tells
# recovery a publish died mid-transaction; comparing the LIVE generation's id
# with the marker's oldId disambiguates which side of the exchange the crash
# hit, so recovery can COMPLETE the retention instead of mistaking the
# displaced old generation for a reusable candidate. A dotfile name keeps it
# invisible to the "$GENERATIONS"/* walks (sweep, prune, recovery).
GEN_EXCHANGE_MARKER_NAME=".exchange-in-flight"
__gen_recover_exchange_marker() {
  local marker="$GENERATIONS/$GEN_EXCHANGE_MARKER_NAME"
  [[ -f $marker ]] || return 0
  local m_old m_ws live_id ws_agents
  m_old="$(jq -r '.oldId // ""' "$marker" 2>/dev/null || true)"
  m_ws="$(jq -r '.workspaceId // ""' "$marker" 2>/dev/null || true)"
  live_id="$(__gen_meta_field "$SKILLS_CURRENT" id)"
  if [[ -z $m_old || -z $m_ws ]]; then
    log "recovery: dropping an unreadable exchange-in-flight marker"
    rm -f "$marker"
    return 0
  fi
  if [[ $live_id == "$m_old" ]]; then
    # Crash BEFORE the exchange landed: nothing was published; the workspace
    # is an ordinary candidate and the normal walk assesses it.
    log "recovery: a publish died before its exchange landed; dropping the marker"
    rm -f "$marker"
    return 0
  fi
  # The exchange LANDED but retention did not complete: the workspace holds
  # the DISPLACED previous generation. Complete the retention so the walk
  # never sees the old generation as a candidate.
  ws_agents="$GENERATIONS/$m_ws/home/.agents"
  if [[ -d $ws_agents && "$(__gen_meta_field "$ws_agents" id)" == "$m_old" ]]; then
    log "recovery: completing the interrupted retention of previous generation $m_old"
    __gen_garbage_destroy "$GENERATIONS/$m_old"
    if mv "$ws_agents" "$GENERATIONS/$m_old" 2>/dev/null; then
      __gen_garbage_destroy "$GENERATIONS/$m_ws" # the emptied workspace shell
    else
      log "recovery: could not complete the retention; leaving the workspace for the walk to discard"
    fi
  fi
  rm -f "$marker"
}
__gen_recover() {
  GEN_REABSORB=()
  GEN_REUSE_CANDIDATE=""
  __gen_recover_exchange_marker
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
        if __gen_is_complete "$cand_agents" && __gen_meta_matches_desired "$cand_agents" &&
          [[ "$(__gen_meta_field "$cand_agents" buildMode)" == "full" ]] &&
          [[ "$(__gen_meta_field "$cand_agents" id)" == "$id" ]]; then
          # A complete FULL candidate matching desired state: reusable by the
          # weekly refresh (one is enough to publish). An ADDITIVE (install-only)
          # candidate is deliberately NOT reused here: its existing skills are
          # stale byte-clones, so publishing it as the weekly result would ship
          # unrefreshed content and stamp the week a success. It falls through to
          # deletion; the weekly path then builds a fresh full candidate.
          #
          # The meta id must equal the WORKSPACE dir name (R2-3a): a genuine
          # candidate is built with UPDATE_SKILLS_GEN_ID == its workspace id,
          # while a post-exchange crash leaves the DISPLACED OLD generation
          # (whose meta id is the old one) under the new workspace. Reusing
          # that would publish the old generation back over the refreshed one.
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
        # Repair only a link we plausibly own: a stale generation-form target or
        # a DANGLING link. A RESOLVING foreign symlink (e.g. app-owned content
        # at a tracked name) is left alone with a WARN, never replanted.
        local link_target
        link_target="$(readlink "$link" 2>/dev/null || true)"
        if [[ $link_target == ../.skills-current/* || ! -e $link ]]; then
          __gen_plant_store_link "$name" || true
        else
          log "recovery: WARN store/$name is a foreign symlink ($link_target); leaving it"
        fi
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
      if __gen_exchange "$staging" "$SKILLS_CURRENT"; then
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
      if __gen_exchange "$link_stub" "$link"; then
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
    if __gen_exchange "$link_stub" "$SKILL_LOCK_LINK"; then
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

# ---------------------------------------------------------------------------
# CANDIDATE BUILD + LANES + VALIDATION (brief steps 2-4).
# ---------------------------------------------------------------------------
# Outputs of __gen_build_candidate, consumed by the run orchestration and tests.
GEN_CANDIDATE_HOME=""
GEN_CANDIDATE_AGENTS=""

# Build the candidate generation at .skills-generations/<id>/home/.agents: a fake
# HOME whose .agents/skills starts as cp -c clones of the CURRENT generation,
# absorbing any competing-writer real-dir drift recorded in GEN_REABSORB (its
# content wins over the current generation's copy), with the current .skill-lock.json
# seeded. Sets GEN_CANDIDATE_HOME / GEN_CANDIDATE_AGENTS. Returns 1 on any error.
#   __gen_build_candidate <id>
__gen_build_candidate() {
  local id="$1"
  local home="$GENERATIONS/$id/home"
  local agents="$home/.agents"
  __gen_garbage_destroy "$GENERATIONS/$id"
  mkdir -p "$agents/skills" || return 1
  # Clone the current generation's skills (real dirs) into the candidate, but
  # only names still TRACKED by the roster. A skill DELISTED from the lock (e.g.
  # a revoked or compromised one) is NOT carried forward, so it leaves the
  # generation on publish and its store link and fan-out links are dropped.
  if [[ -d "$SKILLS_CURRENT/skills" ]]; then
    local skill_path name
    for skill_path in "$SKILLS_CURRENT/skills"/*; do
      [[ -d $skill_path ]] || continue
      name="${skill_path##*/}"
      __gen_name_is_tracked "$name" || {
        log "candidate: skill $name is no longer tracked; not carrying it forward (delisted)"
        continue
      }
      cp -c -R "$skill_path" "$agents/skills/$name" 2>/dev/null ||
        cp -R "$skill_path" "$agents/skills/$name" || return 1
    done
  fi
  # Absorb competing-writer drift: a store real-dir's content overrides the clone.
  local reabsorb
  for reabsorb in "${GEN_REABSORB[@]:-}"; do
    [[ -n $reabsorb ]] || continue
    [[ -d "$STORE/$reabsorb" && ! -L "$STORE/$reabsorb" ]] || continue
    __gen_garbage_destroy "$agents/skills/$reabsorb"
    cp -c -R "$STORE/$reabsorb" "$agents/skills/$reabsorb" 2>/dev/null ||
      cp -R "$STORE/$reabsorb" "$agents/skills/$reabsorb" || return 1
  done
  # Any tracked store entry that is a REAL DIR and still absent from the clone
  # (a flat pre-migration store under --install-only, which never migrates) is
  # byte-cloned in, so it counts as EXISTING: the additive lanes skip it and
  # validation sees its real content. The store real dir itself stays untouched.
  local tracked_name
  while IFS= read -r tracked_name; do
    [[ -n $tracked_name ]] || continue
    [[ -e "$agents/skills/$tracked_name" ]] && continue
    [[ -d "$STORE/$tracked_name" && ! -L "$STORE/$tracked_name" ]] || continue
    cp -c -R "$STORE/$tracked_name" "$agents/skills/$tracked_name" 2>/dev/null ||
      cp -R "$STORE/$tracked_name" "$agents/skills/$tracked_name" || return 1
  done < <(__gen_tracked_names)
  # Seed the npx lock from the current generation (or an empty object).
  if [[ -f "$SKILLS_CURRENT/.skill-lock.json" ]]; then
    cp -c "$SKILLS_CURRENT/.skill-lock.json" "$agents/.skill-lock.json" 2>/dev/null ||
      cp "$SKILLS_CURRENT/.skill-lock.json" "$agents/.skill-lock.json" || return 1
  else
    printf '{}\n' >"$agents/.skill-lock.json" || return 1
  fi
  GEN_CANDIDATE_HOME="$home"
  GEN_CANDIDATE_AGENTS="$agents"
  log "candidate generation $id built at $GEN_CANDIDATE_AGENTS (home $GEN_CANDIDATE_HOME)"
  return 0
}

# Per-skill failure capture for the streak accounting (brief step 6). The lanes
# run inside the env -i sub-invocation, so failed skill names are appended to a
# file inside the candidate's .agents dir; the parent reads it back before
# discarding the failed candidate. Never published: a candidate with failures is
# always discarded, and a clean build removes the file before the ready marker.
GEN_FAILED_SKILLS_FILE_NAME=".lane-failed-skills"
record_failed_skill() {
  printf '%s\n' "$1" >>"$AGENTS/$GEN_FAILED_SKILLS_FILE_NAME" 2>/dev/null || true
}

# npx lane (brief step 3): explicit `skills add <repo> --skill <name> ...` per
# npxTracked entry, GROUPED by repo (NOT a bulk `update`, whose lock-walk logs
# some failures at exit 0). Operating on $STORE, which in --build-lanes mode is
# the candidate's store (HOME points there). This reconciles lock-absent roster
# skills too, since `add` installs-or-refreshes every entry. Each failure is a
# required failure (the whole candidate is discarded on any).
# Install-only builds (UPDATE_SKILLS_LANES_ADDITIVE=1) narrow every repo group to
# the skills ABSENT from the candidate store, so existing skills stay the
# byte-clones of current the candidate started as (no updates, additive only).
__gen_lane_npx() {
  [[ -f $CUSTOM_SKILL_LOCK ]] || return 0
  local additive="${UPDATE_SKILLS_LANES_ADDITIVE:-}"
  local -a repos=()
  local repo
  while IFS= read -r repo; do
    [[ -n $repo ]] && repos+=("$repo")
  done < <(jq -r '.npxTracked // {} | [.[].repo] | unique | .[]' "$CUSTOM_SKILL_LOCK" 2>/dev/null)
  local -a skill_args group_names
  local name
  for repo in "${repos[@]:-}"; do
    [[ -n $repo ]] || continue
    skill_args=()
    group_names=()
    while IFS= read -r name; do
      [[ -n $name ]] || continue
      if [[ -n $additive && -e "$STORE/$name" ]]; then
        continue # additive build: keep the existing byte-clone, never refresh
      fi
      skill_args+=(--skill "$name")
      group_names+=("$name")
    done < <(jq -r --arg r "$repo" '.npxTracked // {} | to_entries[]
      | select(.value.repo == $r) | .key' "$CUSTOM_SKILL_LOCK" 2>/dev/null)
    [[ ${#skill_args[@]} -gt 0 ]] || continue
    if npx --yes skills@latest add "$repo" "${skill_args[@]}" \
      --agent claude-code --agent codex -g -y 2>&1 | tr -d '\r' | tail -3; then
      log "npx add: $repo (${#group_names[@]} skills)"
    else
      log "npx add failed: $repo (continuing; candidate will be discarded)"
      record_required_failure "npx add $repo failed"
      for name in "${group_names[@]}"; do record_failed_skill "$name"; done
    fi
  done
}

# clawhub lane against the candidate store: install any absent clawhub-tracked
# skill (throwaway --workdir, flatten the nested @owner/<name>), then refresh
# every present one in place. Telemetry off, never --force. A separate scratch
# workdir keeps the store lock free of @owner phantom keys.
__gen_lane_clawhub() {
  [[ -f $CUSTOM_SKILL_LOCK ]] || return 0
  jq -e '.clawhubTracked // {} | length > 0' "$CUSTOM_SKILL_LOCK" >/dev/null 2>&1 || return 0
  local additive="${UPDATE_SKILLS_LANES_ADDITIVE:-}"
  if ! command -v clawhub >/dev/null 2>&1; then
    log "clawhub not on PATH but clawhubTracked is non-empty; candidate cannot be completed"
    record_required_failure "clawhub missing with a non-empty clawhubTracked table (build lane)"
    return 0
  fi
  local skill slug registry tmp_workdir installed_dir overlay_file update_output
  local -a clawhub_cmd
  while IFS=$'\t' read -r -u3 skill slug registry; do
    if [[ ! -e "$STORE/$skill" ]]; then
      [[ -n $slug ]] || continue
      tmp_workdir="$(mktemp -d)"
      clawhub_cmd=(clawhub --no-input --workdir "$tmp_workdir" --dir skills)
      [[ -n $registry ]] && clawhub_cmd+=(--registry "$registry")
      if "${clawhub_cmd[@]}" install "$slug" 2>&1 | tail -2; then
        installed_dir="$tmp_workdir/skills/$slug"
        [[ -d $installed_dir ]] || installed_dir="$tmp_workdir/skills/$skill"
        if [[ -d $installed_dir ]]; then
          mv "$installed_dir" "$STORE/$skill"
          log "clawhub install: $skill from $slug"
        else
          record_required_failure "clawhub install $skill produced no store dir"
          record_failed_skill "$skill"
        fi
      else
        record_required_failure "clawhub install $skill failed"
        record_failed_skill "$skill"
      fi
      rm -rf "$tmp_workdir"
      continue
    fi
    # An additive (install-only) build keeps every existing byte-clone untouched.
    [[ -n $additive ]] && continue
    # present: refresh in place (bare name resolves via origin.json)
    [[ -d "$STORE/$skill" && ! -L "$STORE/$skill" ]] || continue
    rm -f "$STORE/$skill/.DS_Store"
    if ! update_output="$(clawhub --no-input --workdir "$AGENTS" --dir skills update "$skill" 2>&1)"; then
      record_required_failure "clawhub update $skill failed"
      record_failed_skill "$skill"
      printf '%s\n' "$update_output"
      continue
    fi
    if printf '%s\n' "$update_output" | grep -q 'local changes'; then
      overlay_file="$STORE/$skill/agents/openai.yaml"
      if [[ -f $overlay_file && "$(<"$overlay_file")" == "$CODEX_POLICY" ]]; then
        rm "$overlay_file"
        rmdir "$STORE/$skill/agents" 2>/dev/null || true
        if update_output="$(clawhub --no-input --workdir "$AGENTS" --dir skills update "$skill" 2>&1)" &&
          ! printf '%s\n' "$update_output" | grep -q 'local changes'; then
          continue
        fi
        mkdir -p "$STORE/$skill/agents"
        printf '%s\n' "$CODEX_POLICY" >"$overlay_file"
      fi
      record_required_failure "clawhub update $skill refused over local changes"
      record_failed_skill "$skill"
    fi
  done 3< <(jq -r '.clawhubTracked // {} | to_entries[]
    | [.key, (.value.slug // ""), (.value.registry // "")] | @tsv' \
    "$CUSTOM_SKILL_LOCK" 2>/dev/null)
}

# Codex overlays against the candidate store: every on-demand skill carries
# agents/openai.yaml with allow_implicit_invocation disabled (append when the
# upstream ships its own openai.yaml, never overwrite). Idempotent.
__gen_assert_overlays() {
  [[ -f $CUSTOM_SKILL_LOCK ]] || return 0
  local skill overlay_file
  while IFS= read -r skill; do
    [[ -d "$STORE/$skill" && ! -L "$STORE/$skill" ]] || continue
    overlay_file="$STORE/$skill/agents/openai.yaml"
    if [[ -f $overlay_file ]] && grep -q 'allow_implicit_invocation: false' "$overlay_file"; then
      continue
    fi
    mkdir -p "$STORE/$skill/agents" || {
      record_required_failure "candidate overlay dir for $skill could not be created"
      continue
    }
    if [[ -f $overlay_file ]]; then
      printf '\n%s\n' "$CODEX_POLICY" >>"$overlay_file" ||
        record_required_failure "candidate overlay append for $skill failed"
    else
      printf '%s\n' "$CODEX_POLICY" >"$overlay_file" ||
        record_required_failure "candidate overlay write for $skill failed"
    fi
  done < <(jq -r '.tiers // {} | to_entries[] | select(.value == "on-demand") | .key' "$CUSTOM_SKILL_LOCK" 2>/dev/null)
}

# --build-lanes body: runs INSIDE the candidate fake HOME (env -i, HOME set by
# __gen_run_lanes). $STORE etc. resolve to the candidate. Runs the three build
# lanes, writes generation.json LAST as the ready marker, and exits non-zero on
# any required failure so the parent discards the whole candidate.
__gen_do_build_lanes() {
  local id="${UPDATE_SKILLS_GEN_ID:-$(__gen_new_id)}"
  # Record the mode these lanes ran, so recovery can tell a full weekly refresh
  # from an additive install-only build (the ready marker is written only after
  # the lanes of THIS mode complete and validate clean).
  local build_mode="full"
  [[ -n ${UPDATE_SKILLS_LANES_ADDITIVE:-} ]] && build_mode="additive"
  mkdir -p "$STORE"
  rm -f "$AGENTS/$GEN_FAILED_SKILLS_FILE_NAME"
  log "build lane: npx"
  __gen_lane_npx
  log "build lane: clawhub"
  __gen_lane_clawhub
  log "build lane: codex overlays"
  __gen_assert_overlays
  if [[ $REQUIRED_FAILURES -gt 0 ]]; then
    # No ready marker for a failed build: the candidate is incomplete by
    # construction and recovery deletes it if the parent crashes first. The
    # failed-skills file stays for the parent to read before the discard.
    return 1
  fi
  rm -f "$AGENTS/$GEN_FAILED_SKILLS_FILE_NAME"
  # The ready marker goes at .agents/generation.json (one level above skills/),
  # written LAST, stamped with the mode these lanes ran.
  __gen_write_meta "$AGENTS" "$id" "$build_mode"
}

# Parent side: run the build lanes against a candidate home under env -i, with
# HOME, every XDG_* dir, TMPDIR, and the npm cache/config pinned INSIDE the
# candidate, so a lane can only write into the candidate (isolation). PATH is
# passed through so npx/clawhub/jq/GNU mv resolve (and tests can prepend stubs).
#   __gen_run_lanes <candidate-home> <id> [additive]
# A non-empty third argument runs the lanes ADDITIVELY (install-only builds:
# only skills absent from the candidate are installed; nothing is refreshed).
# Returns the re-invocation's exit status (non-zero = discard the candidate).
__gen_run_lanes() {
  local home="$1" id="$2" additive="${3:-}"
  mkdir -p "$home/.cache" "$home/.config" "$home/.local/share" "$home/.local/state" "$home/.tmp" "$home/.npm"
  env -i \
    PATH="$PATH" \
    HOME="$home" \
    XDG_CACHE_HOME="$home/.cache" \
    XDG_CONFIG_HOME="$home/.config" \
    XDG_DATA_HOME="$home/.local/share" \
    XDG_STATE_HOME="$home/.local/state" \
    TMPDIR="$home/.tmp" \
    npm_config_cache="$home/.npm" \
    UPDATE_SKILLS_GMV="${GEN_EXCHANGE_TOOL:-${UPDATE_SKILLS_GMV:-}}" \
    UPDATE_SKILLS_GEN_ID="$id" \
    UPDATE_SKILLS_LOCK_PATH="$CUSTOM_SKILL_LOCK" \
    UPDATE_SKILLS_LANES_ADDITIVE="$additive" \
    UPDATE_SKILLS_BUILD_LANES=1 \
    bash "$UPDATE_SKILLS_SELF" --build-lanes
}

# Validate a fully-built candidate generation (brief step 4): every roster
# tracked skill present with a SKILL.md, on-demand overlays in place, expected
# origin metadata (clawhub skills carry .clawhub/origin.json), the npx lock is
# valid JSON, and the ready marker is present. Returns 0 valid, 1 invalid (the
# caller garbage-renames the candidate and records a required failure, never a
# partial promotion).
#   __gen_validate_candidate <candidate-agents-dir>
__gen_validate_candidate() {
  local agents="$1"
  local skills="$agents/skills"
  [[ -d $skills ]] || {
    log "validate: candidate has no skills dir"
    return 1
  }
  __gen_is_complete "$agents" || {
    log "validate: candidate has no ready marker"
    return 1
  }
  # npx lock must be valid JSON.
  jq -e . "$agents/.skill-lock.json" >/dev/null 2>&1 || {
    log "validate: candidate .skill-lock.json is not valid JSON"
    return 1
  }
  local name
  # every npx- and clawhub-tracked roster skill present with a SKILL.md
  while IFS= read -r name; do
    [[ -n $name ]] || continue
    [[ -d "$skills/$name" ]] || {
      log "validate: tracked skill $name is missing from the candidate"
      record_failed_skill_parent "$name"
      return 1
    }
    [[ -f "$skills/$name/SKILL.md" ]] || {
      log "validate: tracked skill $name has no SKILL.md"
      record_failed_skill_parent "$name"
      return 1
    }
  done < <(__gen_tracked_names)
  # clawhub-tracked skills carry origin metadata
  while IFS= read -r name; do
    [[ -n $name ]] || continue
    [[ -f "$skills/$name/.clawhub/origin.json" ]] || {
      log "validate: clawhub skill $name is missing .clawhub/origin.json"
      record_failed_skill_parent "$name"
      return 1
    }
  done < <(jq -r '.clawhubTracked // {} | keys[]?' "$CUSTOM_SKILL_LOCK" 2>/dev/null)
  # on-demand skills present in the candidate carry the Codex overlay (a vendored
  # on-demand skill lives outside the generation, so it is absent here and skipped)
  while IFS= read -r name; do
    [[ -n $name ]] || continue
    [[ -d "$skills/$name" ]] || continue
    grep -q 'allow_implicit_invocation: false' "$skills/$name/agents/openai.yaml" 2>/dev/null || {
      log "validate: on-demand skill $name is missing its Codex overlay"
      record_failed_skill_parent "$name"
      return 1
    }
  done < <(jq -r '.tiers // {} | to_entries[] | select(.value == "on-demand") | .key' "$CUSTOM_SKILL_LOCK" 2>/dev/null)
  return 0
}

# ---------------------------------------------------------------------------
# RUN ORCHESTRATION (brief steps 5-6): store-link reconcile, live overlay
# verification, the weekly attempt, the install-only attempt, drift reporting,
# and per-skill failure streaks.
# ---------------------------------------------------------------------------

# Post-publish store-link reconciliation for every tracked name present in the
# live generation. Full runs (additive="") also absorb competing-writer real
# dirs recorded by recovery (their content is already inside the published
# generation) and repair stale generation-form or dangling links. Additive runs
# (install-only) only plant links for names with NO store entry at all, so
# nothing existing is ever replaced. A resolving foreign symlink is never
# touched in either mode.
#   __gen_reconcile_store_links [additive]
__gen_reconcile_store_links() {
  local additive="${1:-}" name link target reabsorbed
  while IFS= read -r name; do
    [[ -n $name ]] || continue
    [[ -d "$SKILLS_CURRENT/skills/$name" ]] || continue
    link="$STORE/$name"
    if [[ ! -e $link && ! -L $link ]]; then
      __gen_plant_store_link "$name" || record_required_failure "store link for $name could not be planted"
      continue
    fi
    [[ -n $additive ]] && continue # additive: never replace anything existing
    if [[ -d $link && ! -L $link ]]; then
      # A competing-writer real dir recorded by recovery was absorbed into the
      # published generation; return the store name to link topology.
      reabsorbed=""
      local n
      for n in "${GEN_REABSORB[@]:-}"; do
        if [[ -n $n && $n == "$name" ]]; then reabsorbed=1; fi
      done
      if [[ -n $reabsorbed ]]; then
        __gen_absorb_store_link "$name" || record_required_failure "store/$name could not be re-absorbed"
      else
        log "WARN: store/$name is a real dir not seen by recovery; leaving it (next run re-absorbs)"
      fi
      continue
    fi
    if [[ -L $link ]] && ! __gen_store_link_correct "$name"; then
      target="$(readlink "$link" 2>/dev/null || true)"
      if [[ $target == ../.skills-current/* || ! -e $link ]]; then
        __gen_plant_store_link "$name" || record_required_failure "store link for $name could not be repaired"
      else
        log "WARN: store/$name is a foreign symlink ($target); leaving it"
      fi
    fi
  done < <(__gen_tracked_names)
}

# Post-publish (full runs only): remove obsolete UPDATER-OWNED generation store
# links whose skill is no longer tracked. After a delisted skill leaves the
# published generation (not carried forward by the candidate build), its store
# symlink at $STORE/<name> -> ../.skills-current/skills/<name> dangles; removing
# it drops the skill from Claude/hermes fan-out convergence, which derives its
# desired set from the store. Only an updater-owned generation link (recognized
# by __gen_store_link_correct's exact target form) for a NON-tracked name is
# removed: a foreign real dir, a vendored real dir, cua-driver's app-owned
# symlink, and any non-updater symlink all fail that predicate and survive, and
# a still-tracked name is always kept. Never deletes through a foreign symlink.
__gen_prune_delisted_store_links() {
  [[ -d $STORE ]] || return 0
  local link name
  for link in "$STORE"/*; do
    [[ -L $link ]] || continue # real dirs (foreign/vendored) are outside the generation
    name="${link##*/}"
    __gen_store_link_correct "$name" || continue # foreign/app-owned symlink: never through it
    __gen_name_is_tracked "$name" && continue    # still tracked: keep
    if rm -f "$link"; then
      log "prune: removed delisted store link $name (no longer tracked; dropped from fan-out)"
    else
      record_required_failure "delisted store link $name could not be removed"
    fi
  done
}

# Live-pass Codex overlay handling (brief step 3, overlays): tier overlays are
# ASSERTED in the candidate only; the live pass VERIFIES them through the store
# links and records a required failure when one is missing; it never writes
# through a store link. A vendored on-demand skill (a real store dir outside the
# generation) still gets the old write-if-missing assert (additive, chezmoi owns
# the committed copy). App-owned store symlinks (not generation links) carry no
# overlay by documented asymmetry.
__gen_verify_live_overlays() {
  [[ -f $CUSTOM_SKILL_LOCK ]] || return 0
  local skill overlay_file
  while IFS= read -r skill; do
    [[ -n $skill ]] || continue
    if [[ -L "$STORE/$skill" ]]; then
      __gen_store_link_correct "$skill" || continue # app-owned/foreign link: never through it
      overlay_file="$STORE/$skill/agents/openai.yaml"
      if ! grep -q 'allow_implicit_invocation: false' "$overlay_file" 2>/dev/null; then
        log "OVERLAY MISSING: on-demand skill $skill has no Codex overlay in the live generation (never written through store links; the next candidate re-asserts it)"
        record_required_failure "live overlay missing for $skill"
        record_failed_skill_parent "$skill"
      fi
      continue
    fi
    [[ -d "$STORE/$skill" ]] || continue
    # vendored real dir: keep the additive write-if-missing assert
    overlay_file="$STORE/$skill/agents/openai.yaml"
    if [[ -f $overlay_file ]] && grep -q 'allow_implicit_invocation: false' "$overlay_file"; then
      continue
    fi
    if ! mkdir -p "$STORE/$skill/agents"; then
      record_required_failure "codex overlay dir for $skill could not be created"
      continue
    fi
    if [[ -f $overlay_file ]]; then
      if printf '\n%s\n' "$CODEX_POLICY" >>"$overlay_file"; then
        log "appended codex overlay policy to upstream openai.yaml: $skill"
      else
        record_required_failure "codex overlay append for $skill failed"
      fi
    elif printf '%s\n' "$CODEX_POLICY" >"$overlay_file"; then
      log "asserted codex overlay: $skill"
    else
      record_required_failure "codex overlay write for $skill failed"
    fi
  done < <(jq -r '.tiers // {} | to_entries[] | select(.value == "on-demand") | .key' "$CUSTOM_SKILL_LOCK" 2>/dev/null)
}

# Parent-side per-skill failure capture (validation failures, live overlay
# verification, migration exchanges). The lanes' subprocess failures arrive via
# the candidate's failed-skills file and are merged in the weekly attempt.
GEN_FAILED_SKILLS=()
record_failed_skill_parent() { GEN_FAILED_SKILLS+=("$1"); }
__gen_merge_lane_failures() {
  local file="$1" name
  [[ -f $file ]] || return 0
  while IFS= read -r name; do
    [[ -n $name ]] && GEN_FAILED_SKILLS+=("$name")
  done <"$file"
}

# --dry-run drift report (brief Modes): NEVER invokes either package CLI (the
# npx CLI treats `update --help` as a real update, observed live). Reports
# roster-vs-lock drift (npx-tracked roster skills absent from the npx CLI lock)
# and roster-vs-generation drift (tracked roster skills absent from the live
# generation, or no generation at all: migration pending). Zero writes.
__gen_dryrun_drift_report() {
  [[ -f $CUSTOM_SKILL_LOCK ]] || {
    log "drift: no custom-skill-lock.json; nothing to compare"
    return 0
  }
  local name
  if [[ -f $SKILL_LOCK_LINK ]]; then
    while IFS= read -r name; do
      [[ -n $name ]] || continue
      jq -e --arg n "$name" '.skills | has($n)' "$SKILL_LOCK_LINK" >/dev/null 2>&1 ||
        log "drift: roster skill $name is absent from the npx lock (the explicit per-repo add reconciles it)"
    done < <(jq -r '.npxTracked // {} | keys[]?' "$CUSTOM_SKILL_LOCK" 2>/dev/null)
  else
    log "drift: no npx lock present"
  fi
  if __gen_is_complete "$SKILLS_CURRENT"; then
    while IFS= read -r name; do
      [[ -n $name ]] || continue
      [[ -d "$SKILLS_CURRENT/skills/$name" ]] ||
        log "drift: roster skill $name is absent from the live generation (the next full run adds it)"
    done < <(__gen_tracked_names)
  else
    log "drift: no live generation yet; the next full run migrates the flat store"
  fi
}

# Per-skill failure streaks (brief step 6): {last_failed_week,
# consecutive_failed_weeks} per skill in one JSON map, incremented at most once
# per ISO WEEK (not per hourly slot), reset on verified success, escalated alert
# wording at 2 consecutive weeks. Convergence never stops: streaks only change
# the alert wording, never gate a retry.
STREAK_FILE="$STATE_DIR/skill-failure-streaks.json"
__gen_update_failure_streaks() {
  [[ ${#GEN_FAILED_SKILLS[@]} -gt 0 ]] || return 0
  local week name streaks entry_week entry_count
  week="$(date +%G-%V)"
  mkdir -p "$STATE_DIR" 2>/dev/null || return 0
  streaks="$(cat "$STREAK_FILE" 2>/dev/null || true)"
  jq -e . <<<"$streaks" >/dev/null 2>&1 || streaks='{}'
  local -a escalated=()
  local -a seen=()
  local dup
  for name in "${GEN_FAILED_SKILLS[@]}"; do
    [[ -n $name ]] || continue
    dup=""
    local s
    for s in "${seen[@]:-}"; do
      if [[ -n $s && $s == "$name" ]]; then dup=1; fi
    done
    if [[ -n $dup ]]; then continue; fi
    seen+=("$name")
    entry_week="$(jq -r --arg n "$name" '.[$n].last_failed_week // ""' <<<"$streaks")"
    entry_count="$(jq -r --arg n "$name" '.[$n].consecutive_failed_weeks // 0' <<<"$streaks")"
    [[ $entry_count =~ ^[0-9]+$ ]] || entry_count=0
    if [[ $entry_week == "$week" ]]; then
      : # already counted this week (a later hourly slot); no double increment
    else
      entry_count=$((entry_count + 1))
      streaks="$(jq --arg n "$name" --arg w "$week" --argjson c "$entry_count" \
        '.[$n] = {last_failed_week: $w, consecutive_failed_weeks: $c}' <<<"$streaks")"
    fi
    if [[ $entry_count -ge 2 ]]; then
      escalated+=("$name ($entry_count weeks)")
      log "STREAK: skill $name has failed $entry_count consecutive weekly cycles"
    fi
  done
  printf '%s\n' "$streaks" >"$STREAK_FILE" 2>/dev/null || true
  if [[ ${#escalated[@]} -gt 0 ]]; then
    __update_skills_alert "Weekly skills update: still failing after multiple weeks for ${escalated[*]}. The updater keeps retrying weekly, but these skills need eyes (~/.local/log/skills/)."
  fi
}
__gen_reset_failure_streaks() {
  if [[ -f $STREAK_FILE ]]; then
    printf '{}\n' >"$STREAK_FILE" 2>/dev/null || true
  fi
}

# The full-run weekly attempt (brief steps 2-5): reuse a recovered complete
# matching candidate, or build one; run the lanes; validate; publish with the
# atomic exchange; reconcile the store links. ANY failure discards the WHOLE
# candidate (no partial promotion), records a required failure (loud + relay),
# and leaves the live generation untouched; the next slot retries.
__gen_weekly_attempt() {
  local relay_script="$HOME/.local/bin/relay.sh"
  local id candidate_home candidate_agents id_dir
  if [[ -n $GEN_REUSE_CANDIDATE ]] && __gen_validate_candidate "$GEN_REUSE_CANDIDATE"; then
    candidate_agents="$GEN_REUSE_CANDIDATE"
    id_dir="$(dirname "$(dirname "$candidate_agents")")"
    log "reusing the recovered complete candidate at $candidate_agents"
    if ! __gen_roster_unchanged; then
      record_required_failure "the roster lock changed mid-run; refusing to publish the recovered candidate (built from the old roster)"
      __gen_garbage_destroy "$id_dir"
      return 1
    fi
    if __gen_publish "$candidate_agents"; then
      __gen_garbage_destroy "$id_dir"
      __gen_reconcile_store_links
      __gen_prune_delisted_store_links
      __gen_plant_lock_link || record_required_failure "lock link could not be planted after publish"
      return 0
    fi
    record_required_failure "publish of the recovered candidate failed"
    __gen_garbage_destroy "$id_dir"
    return 1
  fi
  id="$(__gen_new_id)"
  if ! __gen_build_candidate "$id"; then
    record_required_failure "candidate build failed"
    __gen_garbage_destroy "$GENERATIONS/$id"
    return 1
  fi
  candidate_home="$GEN_CANDIDATE_HOME"
  candidate_agents="$GEN_CANDIDATE_AGENTS"
  if ! __gen_run_lanes "$candidate_home" "$id"; then
    __gen_merge_lane_failures "$candidate_agents/$GEN_FAILED_SKILLS_FILE_NAME"
    record_required_failure "build lanes failed; the whole candidate is discarded (no partial promotion)"
    __gen_garbage_destroy "$GENERATIONS/$id"
    if [[ -x $relay_script ]]; then
      "$relay_script" --agent update-skills --state build-failed --project skills \
        --detail "the weekly candidate build lanes failed; the live generation is untouched and the next slot retries" || true
    fi
    return 1
  fi
  if ! __gen_validate_candidate "$candidate_agents"; then
    record_required_failure "candidate validation failed; the whole candidate is discarded (no partial promotion)"
    __gen_garbage_destroy "$GENERATIONS/$id"
    if [[ -x $relay_script ]]; then
      "$relay_script" --agent update-skills --state validation-failed --project skills \
        --detail "the weekly candidate failed validation; the live generation is untouched and the next slot retries" || true
    fi
    return 1
  fi
  if ! __gen_roster_unchanged; then
    record_required_failure "the roster lock changed mid-run; refusing to publish a candidate built from the old roster"
    __gen_garbage_destroy "$GENERATIONS/$id"
    return 1
  fi
  if ! __gen_publish "$candidate_agents"; then
    record_required_failure "publish failed; no success recorded (the publish log above says whether the exchange landed)"
    __gen_garbage_destroy "$GENERATIONS/$id"
    return 1
  fi
  __gen_garbage_destroy "$GENERATIONS/$id" # the emptied build workspace shell
  __gen_reconcile_store_links
  __gen_prune_delisted_store_links
  __gen_plant_lock_link || record_required_failure "lock link could not be planted after publish"
  return 0
}

# The install-only attempt (brief Modes): builds and publishes a candidate whose
# EXISTING skills are byte-clones of current (no updates) plus genuinely absent
# roster skills added. Never migrates a flat store, never replaces existing
# store content: link planting is additive (absent names only).
#
# The absent-skill set is computed FIRST. install-only is purely additive, so
# with nothing absent there is nothing to install AND nothing to publish: the
# publish path always exchanges the WHOLE live generation, which would displace a
# concurrent out-of-band write into the retained generation (prunable) and switch
# a reader reopening a stable path to a new generation mid-session. So return
# before building or publishing when nothing is absent. When something IS absent
# and a live generation already exists, the publish exchange is gated behind the
# idle gate exactly like the weekly run; a fresh machine with no live generation
# publishes by a plain rename (no exchange, no readers) and is never gated, which
# is what keeps the apply-time bootstrap unattended.
__gen_install_only_attempt() {
  local id candidate_home candidate_agents
  local -a absent=()
  local tracked_name
  while IFS= read -r tracked_name; do
    [[ -n $tracked_name ]] || continue
    [[ -e "$STORE/$tracked_name" || -L "$STORE/$tracked_name" ]] && continue
    absent+=("$tracked_name")
  done < <(__gen_tracked_names)
  if [[ ${#absent[@]} -eq 0 ]]; then
    log "install-only: nothing absent; no changes"
    return 0
  fi
  if __gen_is_complete "$SKILLS_CURRENT" &&
    [[ ${UPDATE_SKILLS_FORCE:-} != "1" ]] && __update_skills_should_defer; then
    # A live generation exists, so publishing an absent skill EXCHANGES it. A
    # harness shows recent activity (or the probe errored, fail-closed): defer
    # the exchange to a later run rather than swap the generation under a live
    # session. The additive fan-out convergence still runs in the caller.
    log "install-only: ${#absent[@]} absent skill(s) to add, but a harness shows recent activity; deferring the generation exchange to a later run"
    return 0
  fi
  id="$(__gen_new_id)"
  if ! __gen_build_candidate "$id"; then
    record_required_failure "install-only candidate build failed"
    __gen_garbage_destroy "$GENERATIONS/$id"
    return 1
  fi
  candidate_home="$GEN_CANDIDATE_HOME"
  candidate_agents="$GEN_CANDIDATE_AGENTS"
  if ! __gen_run_lanes "$candidate_home" "$id" additive; then
    __gen_merge_lane_failures "$candidate_agents/$GEN_FAILED_SKILLS_FILE_NAME"
    record_required_failure "install-only lanes failed; candidate discarded"
    __gen_garbage_destroy "$GENERATIONS/$id"
    return 1
  fi
  if ! __gen_validate_candidate "$candidate_agents"; then
    record_required_failure "install-only candidate failed validation; candidate discarded"
    __gen_garbage_destroy "$GENERATIONS/$id"
    return 1
  fi
  if ! __gen_roster_unchanged; then
    record_required_failure "the roster lock changed mid-run; refusing to publish the install-only candidate (built from the old roster)"
    __gen_garbage_destroy "$GENERATIONS/$id"
    return 1
  fi
  if ! __gen_publish "$candidate_agents"; then
    record_required_failure "install-only publish failed; no success recorded (the publish log above says whether the exchange landed)"
    __gen_garbage_destroy "$GENERATIONS/$id"
    return 1
  fi
  __gen_garbage_destroy "$GENERATIONS/$id"
  __gen_reconcile_store_links additive
  # The lock link is planted only when nothing exists at the path (a flat lock
  # file is migration's job, and install-only never migrates).
  if [[ ! -e $SKILL_LOCK_LINK && ! -L $SKILL_LOCK_LINK ]]; then
    __gen_plant_lock_link || true
  fi
  return 0
}

# serialize: one run at a time, via the KERNEL. macOS ships /usr/bin/lockf
# (lockf(1), flock(2)-backed): acquisition opens $LOCKFILE on fd 9 and
# test-acquires with `lockf -s -t 0 9` (the man page's fd synopsis; -t 0 =
# non-blocking, exit 75 = EX_TEMPFAIL when another process holds it). The kernel
# grants the lock to exactly one process and releases it automatically when
# every copy of the fd closes (normal exit, crash, or kill alike), so the
# stale-lock/two-owner class the previous hand-rolled mkdir-owner-token lock
# kept re-admitting is structurally gone: no owner token, no liveness probing,
# no dead-owner reclaim, no EXIT-trap cleanup. The lock FILE persists on disk
# by design (the fd form implies lockf's -k keep semantics, which the man page
# recommends for lock ordering); its existence does NOT mean the lock is held,
# only a live open fd does. The absolute /usr/bin/lockf path is used because
# the Nix devshell's PATH does not carry macOS's /usr/bin tools. Defined above
# the lib-only gate so the concurrency regression can drive
# __update_skills_acquire_lock directly from real subshells.
__update_skills_acquire_lock() {
  # Non-darwin (no /usr/bin/lockf): proceed unlocked, loudly. The weekly
  # LaunchAgent that creates concurrent scheduled runs is darwin-only, so on
  # Linux only deliberate manual runs exist and serialization is the operator's
  # responsibility; wedging every Linux run on a missing macOS tool would be
  # worse than the notice.
  if [[ ! -x /usr/bin/lockf ]]; then
    log "no /usr/bin/lockf on this host; proceeding without the serialize lock (the scheduled runs that contend are darwin-only)"
    return 0
  fi
  mkdir -p "$AGENTS" 2>/dev/null || return 1
  # Hold fd 9 for the remainder of this process's lifetime; the kernel releases
  # the lock when the process exits. A failed open (unwritable .agents) is a
  # failed acquisition: the caller defers, never proceeds unlocked on darwin.
  exec 9>>"$LOCKFILE" || return 1
  /usr/bin/lockf -s -t 0 9
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

# --build-lanes: this process IS the env -i sub-invocation running inside a
# candidate fake HOME (see __gen_run_lanes). Run only the generation build lanes
# against the candidate store and exit; the parent handles recovery, validation,
# publish, fan-out, and the idle gate. No lock, no stamp, no idle gate here.
if [[ -n $BUILD_LANES ]]; then
  __gen_do_build_lanes
  exit $?
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
# (an unreadable process table or a probe error counts as ACTIVE). This is the
# UNATTENDED norm: the weekly run proceeds whenever the machine has been quiet for
# IDLE_THRESHOLD, even with a bridge up.
#
# PR-facing tradeoff: the gate reads mtimes, not a lock the harnesses hold, so
# there is a narrow race: a session that starts writing in the millisecond after
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
#   2 = ps could not be read (or yielded nothing; it must at minimum list this
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
#   0 = ACTIVE  = a file newer than the sentinel exists, OR the probe errored
#                 (unreadable dir / scan failure): fail closed and treat as active
#   1 = STALE   = the dir is present and readable but its newest file is older
#                 than the window
#   2 = ABSENT  = the dir does not exist: this harness is not installed, so it
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
#   Claude Code: ~/.claude/projects/**/<session>.jsonl, the transcript that is
#     appended per turn; the FILE mtime is the signal (directory mtimes do NOT
#     propagate on append: verified live, newest file mtime == wall clock during
#     an active turn while the enclosing dir mtime lagged ~45s).
#   Codex: ~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl, the per-turn rollout
#     transcript (verified: mtimes advance per turn, stale while idle).
#   Hermes: ~/.hermes/logs/agent.log (and siblings), the gateway's per-run/turn
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
  # Epoch-to-stamp spelling differs by date flavor: GNU date takes -d @<epoch>,
  # BSD date takes -r <epoch> (and its -d is the kernel DST flag, so the flavor
  # is detected via --version, which only GNU date supports, instead of by
  # trying -d). Both render LOCAL time, matching touch -t below.
  if date --version >/dev/null 2>&1; then
    stamp="$(date -d "@$cutoff" +%Y%m%d%H%M.%S 2>/dev/null)" || stamp=""
  else
    stamp="$(date -r "$cutoff" +%Y%m%d%H%M.%S 2>/dev/null)" || stamp=""
  fi
  [[ -n $stamp ]] || {
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

if [[ $DRYRUN == "--dry-run" ]]; then
  # A dry run is a READ-ONLY contention check (item 5): it never creates or
  # deletes lock state and tolerates an absent .agents parent. The probe runs
  # in a SUBSHELL: it opens the existing lock file read-only (no create, no
  # truncate) and test-acquires; the subshell's exit closes the fd, so a
  # momentary success is released instantly and nothing on disk changes. An
  # unreadable lock file cannot be probed and previews as would-defer
  # (fail-closed), matching the real run's failed-open deferral.
  if [[ ! -e $LOCKFILE ]]; then
    log "would run: no lock is held"
  elif [[ ! -x /usr/bin/lockf ]]; then
    log "would run: no /usr/bin/lockf on this host (the real run proceeds unlocked; scheduled contention is darwin-only)"
  elif (exec 9<"$LOCKFILE" && /usr/bin/lockf -s -t 0 9) 2>/dev/null; then
    log "would run: the existing lock file is not held (leftover from a finished or crashed run)"
  else
    log "would defer: a live run holds the lock"
  fi
else
  if ! __update_skills_acquire_lock; then
    log "another run in progress; exiting"
    exit 0
  fi
  # No release path: the kernel drops the lock when this process exits, however
  # it exits. The lock file itself is deliberately never deleted (see the
  # acquisition comment: deleting it would let a later opener lock a fresh
  # inode while an older holder still locks the unlinked one, i.e. two owners).
  #
  # FAIL-CLOSED roster gate (R2-2): the mutation modes (weekly and
  # install-only) validate + snapshot the roster lock BEFORE anything runs. A
  # missing/unparseable/schema-broken roster, or a VALID roster whose tracked
  # set is empty while the live generation still holds skills (a delist-all is
  # indistinguishable from corruption), is a refused run: loud required
  # failure, relay alert, exit 1 (which also keys the first-install wrapper's
  # retry marker), and the live store/generation/fan-out untouched.
  # --check-forks-only mutates nothing and keeps its tolerant no-op contract.
  if [[ -z $CHECK_FORKS_ONLY ]]; then
    if ! __gen_snapshot_roster; then
      record_required_failure "roster lock validation failed (missing, unparseable, or schema-broken); no build, no publish, no prune, no stamp"
      __update_skills_alert "update-skills refused to run: the roster lock at $GEN_ROSTER_SOURCE is missing or broken. Fix the deployed custom-skill-lock.json (chezmoi apply) and re-run."
      exit 1
    fi
    __update_skills_live_skill_count=0
    if [[ -d "$SKILLS_CURRENT/skills" ]]; then
      for __update_skills_live_entry in "$SKILLS_CURRENT/skills"/*; do
        [[ -d $__update_skills_live_entry ]] && __update_skills_live_skill_count=$((__update_skills_live_skill_count + 1))
      done
    fi
    __update_skills_tracked_count=0
    while IFS= read -r __update_skills_tracked_probe; do
      [[ -n $__update_skills_tracked_probe ]] && __update_skills_tracked_count=$((__update_skills_tracked_count + 1))
    done < <(__gen_tracked_names)
    if [[ $__update_skills_tracked_count -eq 0 && $__update_skills_live_skill_count -gt 0 ]]; then
      record_required_failure "the roster tracks ZERO skills but the live generation holds $__update_skills_live_skill_count; refusing to clone-filter/prune (a delist-everything roster is treated as corruption, not intent)"
      __update_skills_alert "update-skills refused to run: the roster lock tracks no skills while the live generation is non-empty. If delisting everything is intended, remove the generation by hand; otherwise restore the roster."
      exit 1
    fi
    # The run-private snapshot is a temp file; remove it on any exit. The
    # trailing `true` keeps the trap from ever altering the exit status.
    trap '[[ -n ${GEN_ROSTER_SNAPSHOT_FILE:-} ]] && rm -f "$GEN_ROSTER_SNAPSHOT_FILE"; true' EXIT
  fi
  # RECOVERY (brief step 1) runs under the lock, BEFORE the stamp early-exit and
  # the idle gate, so a crash-window leftover self-heals even on a slot that
  # then early-exits. It deletes incomplete staging, marks a reusable complete
  # candidate, repairs the stable store/lock links, records competing-writer
  # real dirs for re-absorption, and prunes generation garbage. Dry runs never
  # reach here (the dry branch above is read-only).
  __gen_recover
fi

# weekly success stamp: the 24 Monday plist slots share one stamp; once a slot
# completes a full run for the CURRENT desired state this week, the remaining
# slots are no-ops. The stamp is the ISO week PLUS the custom-lock and updater
# hashes, so a roster or updater change after a Monday success un-stamps the week
# and the next slot rebuilds. A deferral writes no stamp, so the next slot
# retries. FORCE and dry-run bypass; install-only / check-forks-only never
# consult or write it.
if [[ -z $INSTALL_ONLY ]] && [[ -z $CHECK_FORKS_ONLY ]] && [[ $DRYRUN != "--dry-run" ]] &&
  [[ ${UPDATE_SKILLS_FORCE:-} != "1" ]] &&
  [[ -f $SUCCESS_STAMP && "$(cat "$SUCCESS_STAMP" 2>/dev/null)" == "$(__update_skills_stamp_value)" ]]; then
  log "weekly skills update already succeeded this week for the current roster; nothing to do"
  exit 0
fi

# idle-gate (activity-based, fail-closed): defer only while a harness shows
# RECENT activity (within IDLE_THRESHOLD), so the weekly run is UNATTENDED; an
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
    __update_skills_alert "Weekly skills update deferred on every scheduled slot (the machine had agent activity at every Monday slot). Run it by hand when idle (~/.local/bin/update-skills.sh)."
  fi
  exit 0
fi

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

# --dry-run (brief Modes): a read-only preview that NEVER invokes either package
# CLI (the npx CLI treats `update --help` as a real update, observed live) and
# makes ZERO writes. It reports roster-vs-lock and roster-vs-generation drift,
# the fan-out convergence preview (would create/replace/remove, pure readlink
# logic), and the would-run/would-defer lock preview printed above.
if [[ $DRYRUN == "--dry-run" ]]; then
  __gen_dryrun_drift_report
  converge_claude_skills
  converge_hermes_skills
  refresh_app_owned_cua_pack    # its dry branch logs the would-run line only
  assert_superpowers_routing    # --dry-run probe of the routing script (read-only)
  update_hermes_registry_skills # its dry branch logs would-update lines only
  check_fork_drift              # its dry branch logs would-drift-check lines only
  log "done (dry-run)"
  exit 0
fi

# --install-only (brief Modes): build and publish an ADDITIVE candidate whose
# existing skills are byte-clones of the current generation plus genuinely
# absent roster skills added. Never migrates a flat store, never replaces
# existing store content. Safe under a live session (nothing existing is
# swapped), which is what lets the fresh-machine bootstrap run at apply time.
if [[ -n $INSTALL_ONLY ]]; then
  __gen_install_only_attempt || true
  converge_claude_skills
  converge_hermes_skills
  __gen_verify_live_overlays
  assert_superpowers_routing
  log "done (install-only)"
  # Signal any required-phase failure to the caller (the first-install
  # chezmoiscript keys its retry marker on this non-zero exit).
  if [[ $REQUIRED_FAILURES -gt 0 ]]; then
    log "install-only finished with $REQUIRED_FAILURES required-phase failure(s)"
    exit 1
  fi
  exit 0
fi

# FULL WEEKLY RUN (brief steps 1-6), the generation-exchange path:
# 1) Migration: first run on a machine with the old flat store converts every
#    tracked real dir to a stable store symlink into a freshly built
#    .skills-current generation (per-entry atomic exchange; idle-gated full
#    runs only, never --install-only).
if __gen_migration_needed; then
  log "migrating the flat store to the generation layout"
  __gen_migrate || record_required_failure "flat-store migration failed"
  # Recovery ran before the generation existed; re-run it so a reusable
  # candidate or competing-writer drift is assessed against the migrated state.
  __gen_recover
fi

# 2-5) Build the candidate generation (a fake HOME under .skills-generations),
#      run the npx + clawhub + overlay lanes against it under env -i, validate
#      the WHOLE candidate, and publish with ONE atomic exchange. Any failure
#      discards the whole candidate; the live generation is untouched.
log "weekly generation attempt"
__gen_weekly_attempt || log "the weekly generation attempt failed; the live generation is unchanged (a later slot retries)"

# Post-publish live passes (never write through store links):
refresh_app_owned_cua_pack

# CONVERGE the fan-out: every store skill is symlinked into Claude, and into
# exactly the hermes profile skills dirs its hermesProfiles mapping names.
converge_claude_skills
converge_hermes_skills

# VERIFY the Codex overlays through the store links (asserted in the candidate;
# a missing one here is a required failure, never an in-place write); vendored
# real dirs keep the additive write-if-missing assert.
__gen_verify_live_overlays

# re-assert the superpowers->hermes routing patches on the hermes mirror
assert_superpowers_routing

# hermes registry-update phase (hub-owned skills, independent source)
log "hermes registry updates"
update_hermes_registry_skills

# watch the vendored/fork upstreams (alert-only)
log "fork drift-check"
check_fork_drift

# Record this week's success ONLY when zero required phases failed. The stamp is
# the ISO year-week key (date +%G-%V) PLUS the custom-lock and updater hashes
# (see __update_skills_stamp_value). %G (not %Y) keeps a year-boundary week
# correct: the days of ISO week 01 that fall in late December carry the NEXT
# year's %G, and the late-December days of week 52/53 carry the current %G, so the
# key never collides or splits across the boundary (52/53/01 verified). The two
# hashes make the stamp mean "this exact desired state succeeded this week", so a
# roster or updater change after a Monday success un-stamps the week and the next
# slot rebuilds. When a required phase failed we WITHHOLD the stamp, so a later
# scheduled slot retries; and for a scheduled run with no slot remaining this
# week we alert (the retry budget is spent). A dry run records nothing.
if [[ $DRYRUN != "--dry-run" ]]; then
  if [[ $REQUIRED_FAILURES -eq 0 ]] && ! __gen_roster_unchanged; then
    # R2-2 stamp-time re-check: the roster changed AFTER the publish re-check
    # (the last window). Publishing already happened against the snapshot, so
    # live state is consistent; but stamping would mark THIS week done for a
    # roster that no longer matches, so withhold and let the next slot rebuild.
    log "WITHHOLDING the weekly success stamp: the roster lock changed after this run's snapshot; the next slot rebuilds against the new roster"
  elif [[ $REQUIRED_FAILURES -eq 0 ]]; then
    mkdir -p "$STATE_DIR"
    __update_skills_stamp_value >"$SUCCESS_STAMP"
    # A verified success resets every per-skill failure streak.
    __gen_reset_failure_streaks
  else
    log "WITHHOLDING the weekly success stamp: $REQUIRED_FAILURES required-phase failure(s) this run; a later scheduled slot will retry"
    # Per-skill failure streaks: incremented at most once per ISO week (not per
    # hourly slot); a skill at 2+ consecutive failed weeks escalates the alert
    # wording. Convergence never stops: the next slot always retries.
    __gen_update_failure_streaks
    if __update_skills_scheduled_budget_exhausted; then
      log "EXHAUSTED: required-phase failures on the last scheduled slot for this week; the weekly skills update did not fully succeed this week"
      __update_skills_alert "Weekly skills update finished with $REQUIRED_FAILURES required-phase failure(s) and no scheduled slot remains this week. Check ~/.local/log/skills/."
    fi
  fi
fi

log "done${DRYRUN:+ (dry-run)}"
