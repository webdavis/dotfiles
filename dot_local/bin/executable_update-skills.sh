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
#      entries whose upstreams the weekly run drift-checks and alerts on, the
#      deliberate content forks moshi/herdr and the npx-can't-install-full-tree
#      case elevenlabs (its SKILL.md sits at the repo root beside a scripts/
#      dir npx drops, even with --full-depth); (b) plain committed dirs with no
#      forks entry, today only tiktok-crawling, a ClawHub skill left vendored
#      because hermes owns its hub copy (hermesRegistry) and its hub name
#      differs from the roster name.
#   - app-owned symlink (cua-driver): the store entry is a symlink into the
#      app's own skill dir; the app owns the content, and the weekly run
#      refreshes the pack via `cua-driver skills update` (the app's own
#      updater; see refresh_app_owned_cua_pack).
#
# The store serves Claude/Codex always and hermes in two lanes. Symlinks fan
# out to Claude (~/.claude/skills, every store skill) and to hermes per the
# lock's hermesProfiles map ("default" = ~/.hermes/skills, any other profile
# name = ~/.hermes/profiles/<name>/skills, [] = deliberately absent). hermes
# fan-out is driven ENTIRELY by hermesProfiles: a non-empty mapping means
# symlink the store copy into those profiles, [] means do not. Collision-named
# skills (humanizer, hyperframes) never fan out at all: hermes's catalog wins
# those names, the store copies serve Claude/Codex only. The skills hermes OWNS from a registry (hermesRegistry table) are
# hub-owned dirs hermes-side that the weekly hermes phase keeps fresh via
# `hermes -p <profile> skills update <lockKey>`, a store symlink must never
# shadow those paths, which is why hermesRegistry and the non-empty
# hermesProfiles set are disjoint. Codex needs no fan-out: it scans
# $HOME/.agents/skills natively (developers.openai.com/codex/skills), and a
# ~/.codex symlink would surface every skill twice, its tiering is the
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
# Env: UPDATE_SKILLS_FORCE=1 bypasses the weekly success stamp, so a manual run
#      rebuilds a week that already succeeded. The weekly run is scheduled across
#      24 hourly Monday slots; the per-week success stamp
#      (~/.local/state/update-skills/last-success) makes the extra slots no-ops
#      after one succeeds, and the last scheduled slot alerts when required
#      phases still failed. Runs are serialized against each other by the kernel
#      lock on ~/.agents/.update-skills.lock, never by harness activity: see the
#      note at the weekly flow below for why a live session is not a reason to
#      hold off.
set -euo pipefail

# This script clones and inspects git repos in temp dirs (fork drift-check). If
# a caller (e.g. a git hook) leaked GIT_DIR/GIT_INDEX_FILE into our environment,
# those clones would silently operate on the caller's repository instead, unset
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
# The weekly RECORD posted to the #unattended-upgrades channel (see
# unattended-log-lib.sh for the entry shape and why it exists). The success
# STAMP above records only an ISO week plus two hashes, with no wall-clock time
# in it, so the gap figure needs its own marker; log-week-claims is the guard that
# keeps 24 hourly Monday slots from becoming 24 messages.
LOG_SUCCESS_MARKER="$STATE_DIR/last-success-at"
LOG_WEEK_GUARD="$STATE_DIR/log-week-claims"

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
# The plist fires 24 hourly Monday retry slots (00:00..23:00; see
# Library/LaunchAgents/com.webdavis.update-skills.plist.tmpl). This is the hour
# of the LAST slot: a scheduled failure at/after it, or a coalesced catch-up on
# a later weekday, means the weekly retry budget is exhausted, so the run alerts
# LOUDLY instead of failing silent. Keep in sync with the plist.
readonly UPDATE_SKILLS_LAST_SLOT_HOUR="23"
# The Codex on-demand policy overlay this script asserts into store skill dirs
# (see assert_codex_overlays), also what the clawhub update pass recognizes as
# its OWN file when the CLI refuses over it (see update_clawhub_tracked).
readonly CODEX_POLICY=$'policy:\n  allow_implicit_invocation: false'
# The weekly registry-update phase walks exactly the profiles that own a
# registry skill in the lock (hermesRegistry), DERIVED from the lock at run
# time (see update_hermes_registry_skills), never hardcoded, so a new profile
# added to hermesRegistry is walked automatically with no second edit to
# forget. That includes default (Bob): its un-entanglement is DONE
# (2026-07-09), kubernetes-specialist, lobster, and todoist-cli moved to pure
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

# ── The weekly RECORD (task #89). Sourced, not duplicated: homebrew-weekly-upgrade.sh
# posts the same entry shape and the two must not drift. A missing library is
# LOUD and never fatal -- this job's work matters more than its bookkeeping, but
# a silently absent record is exactly the invisibility the record exists to end.
UNATTENDED_LOG_LIB="$(dirname "${BASH_SOURCE[0]}")/unattended-log-lib.sh"
UNATTENDED_LOG_AVAILABLE=""
if [[ -r $UNATTENDED_LOG_LIB ]]; then
  # shellcheck source=dot_local/bin/unattended-log-lib.sh
  source "$UNATTENDED_LOG_LIB"
  UNATTENDED_LOG_AVAILABLE=1
else
  printf '[update-skills] WARNING: %s is missing; no weekly record will be posted (run chezmoi apply)\n' \
    "$UNATTENDED_LOG_LIB" >&2
fi

# The entry's opening lines (this run's timestamp and the gap to the previous
# success), captured ONCE at start-up from ONE clock reading. Start-up because
# the gap must be read BEFORE this run can overwrite the marker, or every
# successful entry would report a gap of zero; one reading because a timestamp
# taken at delivery would sit hours away from the gap printed under it.
LOG_ENTRY_HEADER=""
if [[ -n $UNATTENDED_LOG_AVAILABLE ]]; then
  LOG_ENTRY_HEADER="$(unattended_log_entry_header "$LOG_SUCCESS_MARKER")"
fi

# __update_skills_record <class> <body> -- post ONE weekly record entry.
#
# `class` is `completed` (the run reached the end) or `deferred` (nothing was
# attempted: a deferral or a refusal). FAILURES are not a class here: they keep
# going to the existing alert route via __update_skills_alert so they land in the
# priority channel. Act on one, record the other.
#
# Gated on --scheduled, which only the LaunchAgent passes. A manual run must
# never post, because an operator running this by hand on a Wednesday would make
# a dead LaunchAgent look alive, which inverts the one signal the record carries.
# --dry-run is gated out for the same reason it never relays anywhere else: a
# preview must have no side effects, and a push reaches a channel. Do not "fix"
# either gate.
__update_skills_record() {
  local class="$1" body="$2" detail
  [[ -n $SCHEDULED ]] || return 0
  [[ $DRYRUN == "--dry-run" ]] && return 0
  [[ -n $UNATTENDED_LOG_AVAILABLE ]] || return 0
  if ! unattended_log_claim_week "$LOG_WEEK_GUARD" "$class"; then
    log "weekly record: this ISO week already has a '$class'-or-better entry; not posting again"
    return 0
  fi
  detail="$(printf '%s\n%s' "$LOG_ENTRY_HEADER" "$body")"
  # The week is claimed BEFORE the attempt, so two overlapping slots cannot both
  # post, and GIVEN BACK when the attempt failed, so the week is not marked done
  # with nothing sent. A week whose every slot fails therefore retries on each
  # slot and delivers nothing, which is the truthful outcome; the moment one slot
  # succeeds the rest go quiet.
  if ! unattended_log_post update-skills "$class" "$(unattended_log_host)" "$detail"; then
    unattended_log_release_week "$LOG_WEEK_GUARD" "$class"
    log "weekly record: the entry was NOT delivered; this week stays unclaimed so a later slot retries"
    unattended_log_alert_delivery_failure "$LOG_WEEK_GUARD" update-skills
  fi
  return 0
}

# ── What "changed" can honestly mean here.
#
# The obvious answer, "skill, old version, new version", does not exist for most
# of the store. The npx CLI's lock entry schema is source, sourceType, sourceUrl,
# skillPath, skillFolderHash, installedAt, updatedAt: no version, no commit, and
# the lane installs the latest commit from main with NO pin. Measured against the
# live lock, not one of its entries carries a version field. So for that lane the
# honest change unit is the skillFolderHash moving, and the entry says outright
# that a version number is not knowable there.
#
# The clawhub lane is different: its CLI drops .clawhub/origin.json into each
# skill it installs, carrying installedVersion. That is the only place a version
# number exists anywhere in the store, so those skills get a real
# old -> new transition. Reading the marker rather than the roster also makes the
# lane self-describing: a skill IS clawhub-tracked exactly when it carries one.
#
# The RENDERING lives in unattended-log-lib.sh, shared with
# homebrew-weekly-upgrade.sh: the two weekly jobs must report in the same shape,
# and two copies of that logic would drift into two different-looking logs.

# __update_skills_change_snapshot <lane> -- one "<name><TAB><fingerprint>" line
# per tracked skill in that lane, the input shape unattended_log_change_line
# reads. Taken before the weekly attempt and again after it; the difference is
# what the entry reports. Always exits 0 for a known lane: a record that cannot
# be computed must not fail the run it is recording. An UNKNOWN lane is an error,
# because an empty snapshot would render as "0 of 0 changed", which reads as a
# clean week.
__update_skills_change_snapshot() {
  local lane="$1" lock="$SKILLS_CURRENT/.skill-lock.json" origin name version unreadable=""
  case "$lane" in
    npx)
      # An ABSENT lock is a fresh machine with nothing installed yet, which is a
      # true empty answer. A lock that is present and unreadable is not: that
      # status is passed on, so the entry can say it could not look rather than
      # rendering "0 of 0 tracked entries changed" on a store it never read.
      if [[ -r $lock ]]; then
        jq -r '(.skills // {}) | to_entries[] | [.key, (.value.skillFolderHash // "-")] | @tsv' \
          "$lock" 2>/dev/null
      fi
      ;;
    clawhub)
      for origin in "$STORE"/*/.clawhub/origin.json; do
        [[ -r $origin ]] || continue
        name="${origin#"$STORE"/}"
        name="${name%%/*}"
        # A marker this run could not READ is not a skill with no version, and
        # masking it to `-` made the two readings of an unreadable marker
        # compare equal, so the lane rendered "0 of N tracked entries changed"
        # for a version nothing ever read. The status is passed on instead,
        # exactly as the npx lane passes on an unparseable lock. Two shapes
        # reach here: jq refusing malformed JSON, and a zero-byte marker (a
        # truncated or half-written file), which jq accepts while producing no
        # value at all. A valid marker is never empty, so the size test cannot
        # cry wolf on a real one.
        if [[ ! -s $origin ]] ||
          ! version="$(jq -r '(.installedVersion // "-") | tostring' "$origin" 2>/dev/null)" ||
          [[ -z $version ]]; then
          unreadable=1
          continue
        fi
        # ONE line per skill, and exactly two columns, whatever the marker holds.
        # installedVersion is publisher-controlled: a newline in it forges a
        # second entry AND inflates the denominator the operator reads (one real
        # skill rendering as "1 of 2"), and a tab forges a column. The npx lane
        # above cannot do this because @tsv escapes both.
        printf '%s\t%s\n' \
          "$(printf '%s' "$name" | tr -d '[:cntrl:]')" \
          "$(printf '%s' "$version" | tr -d '[:cntrl:]')"
      done
      # ONE unreadable marker fails the whole lane, exactly as one unparseable
      # lock fails the npx lane. Dropping just that skill from the snapshot
      # would be worse than the mask it replaces: unreadable on the AFTER
      # reading alone renders the skill as `(removed)`, which is the single most
      # alarming line this record can print, invented from a file it could not
      # open.
      [[ -z $unreadable ]] || return 1
      ;;
    *)
      printf 'update-skills: unknown change-snapshot lane: %s\n' "$lane" >&2
      return 1
      ;;
  esac
  # No `return 0` here: the READING's status is the answer. Forcing zero is what
  # turned a lock this run could not parse into "an empty store", and an empty
  # store renders as a clean week.
}

# Per-lane readability, mirroring homebrew-weekly-upgrade.sh: a lane is NOT
# COMPARED when either of its two readings failed, because half a comparison is
# not one.
LOG_NPX_SNAPSHOT_OK=1
LOG_CLAWHUB_SNAPSHOT_OK=1
# What could not be read, per lane. A NOT COMPARED line has to name the thing
# that actually broke: when the workspace itself could not be allocated, both
# readings were fine, and sending the operator to check them wastes the one
# actionable sentence in the entry.
LOG_NPX_SOURCE='reading the generation lock'
LOG_CLAWHUB_SOURCE='reading the clawhub origin markers'
LOG_SNAPSHOT_WORKSPACE_SOURCE='creating the record snapshot workspace (mktemp -d)'
# __update_skills_take_snapshot <lane> <phase> -- one reading, and a note when it
# failed. Never fatal: the record must not break the run it reports on.
__update_skills_take_snapshot() {
  local lane="$1" phase="$2"
  __update_skills_change_snapshot "$lane" >"$LOG_CHANGE_DIR/$lane.$phase" 2>/dev/null && return 0
  case "$lane" in
    npx) LOG_NPX_SNAPSHOT_OK="" ;;
    clawhub) LOG_CLAWHUB_SNAPSHOT_OK="" ;;
  esac
  log "weekly record: the $lane snapshot ($phase) could not be read; that lane will be reported as NOT COMPARED"
  return 0
}

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

# True when no further SCHEDULED slot remains this ISO week to retry a failed
# run. The plist fires 24 hourly Monday slots (00..23); launchd may
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
    # 9>&- for the same reason relay_fork_advisory carries it: relay DETACHES
    # channels that outlive this run, a kernel flock on fd 9 is held until the
    # LAST copy of the fd closes, and an inherited copy in a detached curl keeps
    # the lock held after the updater exits, so the next slot defers over a
    # competing run that does not exist. This wrapper is reached from under that
    # same lock (the lock-failure, roster-refusal and exhaustion paths).
    "$relay_script" --agent update-skills --state exhausted --project skills --detail "$detail" 9>&- || true
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
  # F4: snapshot the OUTGOING generation's owned names BEFORE the exchange, so
  # the delist pruner can tell an updater-owned (generation) store dir from a
  # genuinely foreign one after the swap. Empty on a fresh-machine first
  # publish (no previous generation).
  GEN_PREV_OWNED_NAMES=()
  if [[ -d "$SKILLS_CURRENT/skills" ]]; then
    local __prev_owned
    for __prev_owned in "$SKILLS_CURRENT/skills"/*; do
      [[ -d $__prev_owned ]] || continue
      GEN_PREV_OWNED_NAMES+=("${__prev_owned##*/}")
    done
  fi
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
  # name collision first so the rename lands cleanly). R2-3d / F7: a retention
  # failure is FATAL but DISTINCT, the exchange LANDED (the refreshed
  # generation IS live) and the candidate workspace now holds the ONLY copy of
  # the previous generation, with the marker recording the pending retention.
  # Return the distinct code 2 so the caller PRESERVES the workspace and marker
  # (never garbage-destroys them) for recovery to finish; a plain failure (1) is
  # reserved for "exchange never landed, live untouched". The marker stays.
  __gen_garbage_destroy "$retained"
  if ! mv "$candidate" "$retained" 2>/dev/null; then
    log "publish: FATAL the displaced previous generation could not be retained; the refreshed generation is live but this run reports failure (no stamp). Preserving the workspace and marker for recovery."
    return 2
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
#
# F2: a PRESENT table must be an OBJECT, `.npxTracked // {}` substitutes on
# null AND false (jq: `false // {}` -> `{}`), so `npxTracked: false` (or null,
# a string, an array) would coerce to an empty object, pass the old check, and
# silently drop every npx skill. Reject a present-but-non-object table (an
# absent key stays legal: it degrades to a genuinely empty table). Also
# validate ENTRY schemas: every npx entry carries a non-empty string `repo`,
# every clawhub entry a non-empty string `slug` and `registry`. A malformed
# entry is a required failure, never a silently skipped skill.
#
# `forks` (the drift-watch's whole input) is deliberately NOT gated here, and
# that omission is the design, not an oversight. NOTHING in the mutating path
# reads it: a typo there can only degrade an ADVISORY report, and this script
# already classifies the drift-watch as advisory (see record_required_failure).
# Gating it turned a hand-edit typo in the one field an operator edits by hand
# every time they clear a drift (an unquoted lastComparedTreeHash) into a total
# refusal of the weekly update: no build, no publish, no prune, no stamp, on
# every remaining slot, under an alert that blames the DEPLOYED lock when the
# committed source is what is wrong. The shape is enforced where it costs
# nothing instead: test/unit/skills-roster-fanout.sh fails the BUILD on a
# malformed committed forks entry, and check_fork_drift validates the table and
# every entry at RUN time, reporting each failure loudly and per entry without
# refusing the run that publishes and prunes.
#
# `claudeDelivery` IS validated here, unlike `forks`, because the MUTATING Claude
# fan-out reads it (converge_claude_skills subtracts the "none" set from the store
# links). __update_skills_claude_undelivered fails OPEN by design: a jq that
# errored yields an empty undelivered set, which quietly RESTORES an exempted
# skill's ~/.claude link. A deployed claudeDelivery that is an array or a string,
# or an object whose values are not "none", drives exactly that fail-open. Reject
# a present-but-non-object table and any value other than the string "none"
# (absent means the default, delivered), so a malformed delivery table refuses the
# run loudly instead of silently restoring a de-delivered skill.
#
# The gate reads the lock SLURPED, and the -s is the load-bearing part.
# `jq -e '<filter>' file` reads a STREAM of values and evaluates the filter once
# per document, so its exit status is the LAST document's verdict while every
# extractor downstream still reads them ALL. A roster with a second top-level
# `{}` appended therefore passed every check here on the trailing empty object
# (an absent table is legal) and the run went on to read the real tables out of
# the first document: with claudeDelivery emptied in that first document, the
# undelivered-name reader returned nothing and convergence RECREATED a
# deliberately absent Claude link before stamping the week a success. Slurping
# and requiring exactly one value is the single-value test; the same shape
# guards ~/.claude/settings.json in run_before_12-quarantine-unparseable-claude-settings.sh
# and the forks table in __gen_fork_lock_single_document below.
__gen_roster_schema_ok() {
  jq -e -s '
    def object_or_absent($k): (has($k) | not) or (.[$k] | type == "object");
    def nonempty_string($v): ($v | type) == "string" and ($v | length) > 0;
    length == 1 and (.[0] |
      (type == "object")
      and object_or_absent("npxTracked")
      and object_or_absent("clawhubTracked")
      and object_or_absent("tiers")
      and object_or_absent("claudeDelivery")
      and ((.npxTracked // {}) | to_entries
        | all((.value | type == "object") and nonempty_string(.value.repo)))
      and ((.clawhubTracked // {}) | to_entries
        | all((.value | type == "object")
          and nonempty_string(.value.slug) and nonempty_string(.value.registry)))
      and ((.claudeDelivery // {}) | to_entries | all(.value == "none")))
  ' "$1" >/dev/null 2>&1
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

# F4: true when <name> was owned by the OUTGOING generation (captured by
# __gen_publish before the exchange). Distinguishes an updater-owned store dir a
# competing writer clobbered from a genuinely foreign real dir.
__gen_name_was_generation_owned() {
  local query="$1" owned
  for owned in "${GEN_PREV_OWNED_NAMES[@]:-}"; do
    [[ -n $owned && $owned == "$query" ]] && return 0
  done
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
# RECOVERY state table (brief step 1). Runs before the stamp logic. Self-heals what it can and records two things the main flow acts on:
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
# F4: the set of skill names the OUTGOING generation owned, captured by
# __gen_publish immediately before the exchange (from $SKILLS_CURRENT/skills/).
# It is the provenance the delist pruner needs: a real dir at a store name that
# was generation-owned but is no longer tracked was updater-owned (a delisted
# skill an out-of-band writer clobbered into a real dir) and is quarantined,
# whereas a genuinely FOREIGN real dir (never a generation skill) is preserved.
GEN_PREV_OWNED_NAMES=()
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
      # F7: retention still cannot complete. KEEP the marker AND the workspace
      # (it holds the ONLY copy of the previous generation) so a later recovery
      # retries; the staging walk excludes a marker-named workspace. Never drop
      # the marker here, dropping it would let the walk delete the workspace.
      log "recovery: could not complete the retention; KEEPING the workspace and marker for a later retry (previous generation preserved)"
      return 0
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
  # F7: if the exchange-in-flight marker still exists after the marker handler
  # ran, its retention could not complete; its workspace holds the ONLY copy of
  # the previous generation and must be EXCLUDED from the normal staging walk
  # (which would otherwise delete it as stale, id != workspace).
  local pending_retention_ws=""
  if [[ -f "$GENERATIONS/$GEN_EXCHANGE_MARKER_NAME" ]]; then
    pending_retention_ws="$(jq -r '.workspaceId // ""' "$GENERATIONS/$GEN_EXCHANGE_MARKER_NAME" 2>/dev/null || true)"
  fi
  local entry id newest_retained="" newest_epoch=-1 epoch cand_agents
  if [[ -d $GENERATIONS ]]; then
    for entry in "$GENERATIONS"/*; do
      [[ -d $entry ]] || continue
      id="${entry##*/}"
      case "$id" in *.garbage.*) continue ;; esac
      if [[ -n $pending_retention_ws && $id == "$pending_retention_ws" ]]; then
        log "recovery: preserving workspace $id (retention pending; marker kept for a later retry)"
        continue
      fi
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
# The install-only force-reinstall set (R2-5), declared here (above the lib-only
# gate) so __gen_run_lanes can read it even when a test drives the lanes
# directly. Populated only by __gen_install_only_attempt; empty for weekly runs.
GEN_INSTALL_FORCE_REINSTALL=()

# Build the candidate generation at .skills-generations/<id>/home/.agents: a fake
# HOME whose .agents/skills starts as cp -c clones of the CURRENT generation,
# absorbing any competing-writer real-dir drift recorded in GEN_REABSORB (its
# content wins over the current generation's copy), with the current .skill-lock.json
# seeded. Sets GEN_CANDIDATE_HOME / GEN_CANDIDATE_AGENTS. Returns 1 on any error.
#
# The second argument is the build MODE (default "full"). A FULL (weekly) build
# clone-filters to TRACKED names only, so a delisted skill leaves the generation
# on publish and the full run's delist pruner drops its store/fan-out links
# (delisting is a full-run responsibility, where fan-out convergence also
# reaps the links). An ADDITIVE (install-only) build clones EVERY current entry
# unchanged (R2-7): install-only is strictly additive and never runs the
# pruner, so filtering here would orphan a delisted entry's store link and
# Claude fan-out with nothing to reap them.
#   __gen_build_candidate <id> [full|additive]
__gen_build_candidate() {
  local id="$1" mode="${2:-full}"
  local home="$GENERATIONS/$id/home"
  local agents="$home/.agents"
  __gen_garbage_destroy "$GENERATIONS/$id"
  mkdir -p "$agents/skills" || return 1
  # Clone the current generation's skills (real dirs) into the candidate. FULL
  # runs carry only TRACKED names forward (a delisted skill is dropped);
  # ADDITIVE runs clone every entry unchanged (see the mode note above).
  if [[ -d "$SKILLS_CURRENT/skills" ]]; then
    local skill_path name
    for skill_path in "$SKILLS_CURRENT/skills"/*; do
      [[ -d $skill_path ]] || continue
      name="${skill_path##*/}"
      if [[ $mode == "full" ]] && ! __gen_name_is_tracked "$name"; then
        log "candidate: skill $name is no longer tracked; not carrying it forward (delisted)"
        continue
      fi
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

# True when a skill is on the install-only FORCE-REINSTALL list (R2-5): an
# additive build normally keeps every existing byte-clone, but a skill whose
# live topology drifted (a missing SKILL.md, a lock-absent entry) must be
# reinstalled even though its cloned dir is "present". The parent passes the
# newline-separated set via UPDATE_SKILLS_FORCE_REINSTALL to the env -i lanes.
__gen_lane_force_reinstall() {
  local query="$1" one
  [[ -n ${UPDATE_SKILLS_FORCE_REINSTALL:-} ]] || return 1
  while IFS= read -r one; do
    [[ -n $one && $one == "$query" ]] && return 0
  done <<<"$UPDATE_SKILLS_FORCE_REINSTALL"
  return 1
}

# npx lane (brief step 3): explicit `skills add <repo> --skill <name> ...` per
# npxTracked entry, GROUPED by repo (NOT a bulk `update`, whose lock-walk logs
# some failures at exit 0). Operating on $STORE, which in --build-lanes mode is
# the candidate's store (HOME points there). This reconciles lock-absent roster
# skills too, since `add` installs-or-refreshes every entry. Each failure is a
# required failure (the whole candidate is discarded on any).
# Install-only builds (UPDATE_SKILLS_LANES_ADDITIVE=1) narrow every repo group to
# the skills ABSENT from the candidate store (or on the force-reinstall list),
# so existing healthy skills stay the byte-clones of current the candidate
# started as (no updates, additive only).
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
      if [[ -n $additive && -e "$STORE/$name" ]] && ! __gen_lane_force_reinstall "$name"; then
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
    # R2-5 repair: a present-but-drifted clawhub skill on the force-reinstall
    # list is removed here so the absent-install branch below reinstalls it
    # fresh (an additive build would otherwise keep the broken byte-clone).
    if [[ -n $additive ]] && __gen_lane_force_reinstall "$skill" && [[ -e "$STORE/$skill" ]]; then
      rm -rf "${STORE:?}/${skill:?}"
    fi
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

# F6: remove the updater-owned Codex policy block (the two lines `policy:` then
# `  allow_implicit_invocation: false`) from an openai.yaml, preserving any
# upstream metadata. When nothing but the block (and blank lines) remains, the
# file is removed (and an emptied agents dir). Idempotent; a no-op when the
# block is absent. Returns 0 on success.
__gen_strip_codex_policy() {
  local overlay_file="$1" agents_dir stripped
  [[ -f $overlay_file ]] || return 0
  grep -q 'allow_implicit_invocation: false' "$overlay_file" 2>/dev/null || return 0
  stripped="$(awk '
    {
      if (held) {
        held = 0
        if ($0 == "  allow_implicit_invocation: false") next
        print "policy:"
      }
      if ($0 == "policy:") { held = 1; next }
      print
    }
    END { if (held) print "policy:" }
  ' "$overlay_file")"
  while [[ $stripped == *$'\n' ]]; do stripped="${stripped%$'\n'}"; done
  if [[ -z ${stripped//[[:space:]]/} ]]; then
    rm -f "$overlay_file"
    agents_dir="${overlay_file%/openai.yaml}"
    rmdir "$agents_dir" 2>/dev/null || true
    return 0
  fi
  printf '%s\n' "$stripped" >"$overlay_file"
}

# Codex overlays against the candidate store: every on-demand skill carries
# agents/openai.yaml with allow_implicit_invocation disabled (append when the
# upstream ships its own openai.yaml, never overwrite). SYMMETRICALLY (F6),
# every CORE skill has any updater-owned policy block REMOVED, so an
# on-demand -> core tier change reconciles instead of leaving a stale block.
# Idempotent.
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
  # F6 symmetric pass: a core skill must NOT carry the updater policy block.
  while IFS= read -r skill; do
    [[ -d "$STORE/$skill" && ! -L "$STORE/$skill" ]] || continue
    __gen_strip_codex_policy "$STORE/$skill/agents/openai.yaml"
  done < <(jq -r '.tiers // {} | to_entries[] | select(.value == "core") | .key' "$CUSTOM_SKILL_LOCK" 2>/dev/null)
}

# Reconcile the candidate's published npx lock (R2-4). Two facts drive this:
# the candidate SEEDS .agents/.skill-lock.json as a wholesale copy of the
# previous published lock (so delisted keys survive in it), and the child npx
# CLI reads/writes its global lock at $XDG_STATE_HOME/skills/.skill-lock.json
# (verified empirically against skills 1.5.16 with a pinned XDG_STATE_HOME:
# the CLI never touches ~/.agents/.skill-lock.json when XDG_STATE_HOME is
# set), so the lane's lock writes land INSIDE the candidate home but not in
# the published file. After the lanes: overlay the CLI-written entries onto
# the seeded copy (capturing every install this build performed), and on a
# FULL build drop every key outside the npxTracked set (delisting is a
# full-run responsibility; an additive build keeps existing keys untouched).
#   __gen_reconcile_candidate_npx_lock <full|additive>
__gen_reconcile_candidate_npx_lock() {
  local mode="$1"
  local candidate_lock="$AGENTS/.skill-lock.json"
  local cli_lock="${XDG_STATE_HOME:-$HOME/.local/state}/skills/.skill-lock.json"
  local base cli reconciled
  # Each input must be EXACTLY ONE JSON value, not the stream `jq -e .` accepts:
  # both are handed to --argjson below, which refuses a stream outright, so an
  # unreadable-for-our-purpose lock has to reach the '{}' fallback here instead
  # of failing the whole reconcile. `length == 1` (not `<= 1`) keeps the empty
  # file on the fallback path too, where the old `jq -e .` already put it.
  base="$(cat "$candidate_lock" 2>/dev/null || printf '{}')"
  jq -e -s 'length == 1' <<<"$base" >/dev/null 2>&1 || base='{}'
  cli='{}'
  if [[ -f $cli_lock ]]; then
    cli="$(cat "$cli_lock" 2>/dev/null || printf '{}')"
    jq -e -s 'length == 1' <<<"$cli" >/dev/null 2>&1 || cli='{}'
  fi
  if ! reconciled="$(jq -n \
    --argjson base "$base" \
    --argjson cli "$cli" \
    --arg mode "$mode" \
    --slurpfile roster "$CUSTOM_SKILL_LOCK" '
      ($roster[0].npxTracked // {} | keys) as $tracked
      | (if ($base | length) > 0 then $base else $cli end) as $top
      | (($base.skills // {}) + ($cli.skills // {})) as $merged
      | $top
      | .skills = (if $mode == "full"
          then ($merged | with_entries(select(.key as $k | $tracked | index($k))))
          else $merged
        end)
    ')"; then
    record_required_failure "npx lock reconcile failed (candidate will be discarded)"
    return 1
  fi
  if ! printf '%s\n' "$reconciled" >"$candidate_lock.reconcile.tmp" ||
    ! mv "$candidate_lock.reconcile.tmp" "$candidate_lock"; then
    record_required_failure "npx lock reconcile could not be written (candidate will be discarded)"
    rm -f "$candidate_lock.reconcile.tmp"
    return 1
  fi
  return 0
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
  log "build lane: npx lock reconcile"
  __gen_reconcile_candidate_npx_lock "$build_mode" || true # failure recorded; gate below discards
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
# The install-only FORCE-REINSTALL set (R2-5) is passed to the additive lanes
# so a drifted skill is reinstalled despite its "present" clone.
# Returns the re-invocation's exit status (non-zero = discard the candidate).
__gen_run_lanes() {
  local home="$1" id="$2" additive="${3:-}"
  mkdir -p "$home/.cache" "$home/.config" "$home/.local/share" "$home/.local/state" "$home/.tmp" "$home/.npm"
  local force_reinstall=""
  if [[ ${#GEN_INSTALL_FORCE_REINSTALL[@]} -gt 0 ]]; then
    printf -v force_reinstall '%s\n' "${GEN_INSTALL_FORCE_REINSTALL[@]}"
  fi
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
    UPDATE_SKILLS_FORCE_REINSTALL="$force_reinstall" \
    UPDATE_SKILLS_BUILD_LANES=1 \
    bash "$UPDATE_SKILLS_SELF" --build-lanes 9>&-
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
  # npx lock must be ONE JSON value. Slurped, because `jq -e .` accepts a STREAM
  # and every later reader of this file answers from the LAST document alone
  # (`.skills | has($n)` in the drift report and the health check), so a
  # two-document lock could publish while claiming a skill it does not hold.
  jq -e -s 'length == 1' "$agents/.skill-lock.json" >/dev/null 2>&1 || {
    log "validate: candidate .skill-lock.json is not one JSON value"
    return 1
  }
  # A FULL candidate's npx lock must hold EXACTLY the npxTracked key set
  # (R2-4): a surplus (delisted) key in the published lock would let a later
  # `npx skills update -g` reinstall a revoked skill as a real store dir. An
  # additive candidate is exempt: delisted keys legitimately survive there
  # until the next full run (delisting is a full-run responsibility).
  if [[ "$(__gen_meta_field "$agents" buildMode)" == "full" ]]; then
    local lock_keys tracked_keys
    lock_keys="$(jq -r '.skills // {} | keys | sort | join(",")' "$agents/.skill-lock.json" 2>/dev/null || true)"
    tracked_keys="$(jq -r '.npxTracked // {} | keys | sort | join(",")' "$CUSTOM_SKILL_LOCK" 2>/dev/null || true)"
    if [[ $lock_keys != "$tracked_keys" ]]; then
      log "validate: the candidate npx lock keys [$lock_keys] do not equal the npxTracked set [$tracked_keys]"
      return 1
    fi
  fi
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
    name="${link##*/}"
    # F4: a REAL DIR at a generation-owned name that is no longer tracked was
    # updater-owned (a delisted skill an out-of-band writer clobbered into a
    # real dir); recovery never re-absorbed it (it walks only tracked names),
    # so it would otherwise survive and stay in the fan-out. Quarantine it. A
    # genuinely FOREIGN real dir (never a generation skill, e.g. a vendored copy
    # or an unrelated user dir) is NOT in GEN_PREV_OWNED_NAMES and is preserved.
    if [[ -d $link && ! -L $link ]]; then
      if ! __gen_name_is_tracked "$name" && __gen_name_was_generation_owned "$name"; then
        log "prune: quarantining delisted generation-owned real dir $name (updater-owned, clobbered by an out-of-band writer; dropped from fan-out)"
        __gen_garbage_destroy "$link"
      fi
      continue # foreign/vendored real dirs (not generation-owned) survive
    fi
    [[ -L $link ]] || continue                   # anything else (a stray file): leave it
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
  # ONE JSON OBJECT or start over. `jq -e .` accepted a stream (whose per-name
  # reads below then yield one line per document, so no week ever compares equal
  # and every slot re-increments) and accepted a bare scalar (whose `.[$n]` read
  # is a jq error, fatal under errexit inside the command substitution).
  jq -e -s 'length == 1 and (.[0] | type == "object")' <<<"$streaks" >/dev/null 2>&1 || streaks='{}'
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
    local reuse_publish_rc=0
    __gen_publish "$candidate_agents" || reuse_publish_rc=$?
    if [[ $reuse_publish_rc -eq 0 ]]; then
      __gen_garbage_destroy "$id_dir"
      __gen_reconcile_store_links
      __gen_prune_delisted_store_links
      __gen_plant_lock_link || record_required_failure "lock link could not be planted after publish"
      return 0
    elif [[ $reuse_publish_rc -eq 2 ]]; then
      # F7: the exchange landed but retention is incomplete; the workspace holds
      # the ONLY copy of the previous generation and the marker records the
      # pending retention. PRESERVE both (never garbage-destroy) so recovery
      # finishes it on a later run. No stamp.
      record_required_failure "publish of the recovered candidate landed but retention is incomplete; preserving the workspace and marker for recovery (no stamp)"
      return 1
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
  local build_publish_rc=0
  __gen_publish "$candidate_agents" || build_publish_rc=$?
  if [[ $build_publish_rc -eq 2 ]]; then
    # F7: exchange landed, retention incomplete, preserve the workspace and
    # marker (the only copy of the previous generation) for recovery. No stamp.
    record_required_failure "publish landed but retention is incomplete; preserving the workspace and marker for recovery (no stamp)"
    return 1
  elif [[ $build_publish_rc -ne 0 ]]; then
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

# Live HEALTH of one roster skill (R2-5). Mere path existence is NOT health:
# a store link can RESOLVE while its generation target's SKILL.md is gone, and
# a core<->on-demand tier change drifts the overlay with no absent path at all.
# Prints a drift REASON (empty when healthy) so install-only can decide
# no-op-vs-repair and, for content/link drift, force a reinstall. Reasons:
#   absent   - no store entry at all (also a repair, installed fresh)
#   link     - the store entry is not the correct generation symlink, or it
#              does not resolve into the current generation
#   skillmd  - the resolved skill has no SKILL.md
#   lock     - an npx-tracked skill missing from the published npx lock (a
#              later `npx skills update -g` could reinstall/revoke off it)
#   overlay  - an on-demand skill missing its required Codex tier overlay
# The first four are CONTENT/LINK drift (force a reinstall); overlay drift is
# fixed by a plain rebuild (the candidate re-asserts overlays).
__gen_roster_skill_health() {
  local name="$1"
  if [[ ! -e "$STORE/$name" && ! -L "$STORE/$name" ]]; then
    printf 'absent'
    return 0
  fi
  # Before any live generation exists (a fresh/flat machine that install-only
  # bootstraps but never migrates), a present real dir with a SKILL.md is
  # legitimately healthy: the store-symlink topology is the first full weekly
  # run's job, so demanding it here would reinstall every present skill. Only
  # once a live generation exists is the full topology an invariant.
  if ! __gen_is_complete "$SKILLS_CURRENT"; then
    [[ -f "$STORE/$name/SKILL.md" ]] || {
      printf 'skillmd'
      return 0
    }
    return 0
  fi
  __gen_store_link_correct "$name" || {
    printf 'link'
    return 0
  }
  if [[ ! -d "$SKILLS_CURRENT/skills/$name" ]]; then
    printf 'link' # link is correct-form but its generation target is gone
    return 0
  fi
  [[ -f "$STORE/$name/SKILL.md" ]] || {
    printf 'skillmd'
    return 0
  }
  if jq -e --arg n "$name" '(.npxTracked // {}) | has($n)' "$CUSTOM_SKILL_LOCK" >/dev/null 2>&1; then
    if [[ -f $SKILL_LOCK_LINK ]] &&
      ! jq -e --arg n "$name" '(.skills // {}) | has($n)' "$SKILL_LOCK_LINK" >/dev/null 2>&1; then
      printf 'lock'
      return 0
    fi
  fi
  if jq -e --arg n "$name" '(.tiers // {})[$n] == "on-demand"' "$CUSTOM_SKILL_LOCK" >/dev/null 2>&1; then
    grep -q 'allow_implicit_invocation: false' "$STORE/$name/agents/openai.yaml" 2>/dev/null || {
      printf 'overlay'
      return 0
    }
  elif grep -q 'allow_implicit_invocation: false' "$STORE/$name/agents/openai.yaml" 2>/dev/null; then
    # F6 symmetric: a core (non-on-demand) skill with a lingering updater policy
    # block is unhealthy, the block must be removed. Drives a repair (the
    # candidate re-assert strips it) instead of a false no-op.
    printf 'overlay'
    return 0
  fi
  return 0
}

# The install-only attempt (brief Modes): builds and publishes a candidate whose
# EXISTING skills are byte-clones of current (no updates) plus genuinely absent
# OR unhealthy roster skills repaired. Never migrates a flat store; link
# planting is additive.
#
# The NEEDS-WORK set is computed FIRST from live health, not path existence
# (R2-5): a skill counts as work when it is absent OR its live topology has
# drifted (link/skillmd/lock/overlay). With nothing to do there is nothing to
# publish, so return before building: the publish path exchanges the WHOLE live
# generation, which would displace a concurrent out-of-band write into the
# retained generation for no gain.
#
# Content/link drift (absent/link/skillmd/lock) forces a reinstall of that
# skill even in the additive lanes (the cloned copy would otherwise be skipped
# as "present"); overlay-only drift needs no reinstall (the candidate
# re-asserts overlays and the exchange makes them live).
__gen_install_only_attempt() {
  local id candidate_home candidate_agents reason
  local -a needs_work=()
  GEN_INSTALL_FORCE_REINSTALL=()
  local tracked_name
  while IFS= read -r tracked_name; do
    [[ -n $tracked_name ]] || continue
    reason="$(__gen_roster_skill_health "$tracked_name")"
    [[ -z $reason ]] && continue
    needs_work+=("$tracked_name")
    case "$reason" in
      absent | link | skillmd | lock)
        GEN_INSTALL_FORCE_REINSTALL+=("$tracked_name")
        log "install-only: $tracked_name needs repair ($reason); will reinstall it"
        ;;
      overlay)
        log "install-only: $tracked_name needs repair ($reason); will re-assert its overlay"
        ;;
    esac
  done < <(__gen_tracked_names)
  if [[ ${#needs_work[@]} -eq 0 ]]; then
    log "install-only: every roster skill is present and healthy; no changes"
    return 0
  fi
  id="$(__gen_new_id)"
  if ! __gen_build_candidate "$id" additive; then
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
  local io_publish_rc=0
  __gen_publish "$candidate_agents" || io_publish_rc=$?
  if [[ $io_publish_rc -eq 2 ]]; then
    # F7: exchange landed, retention incomplete, preserve the workspace and
    # marker (the only copy of the previous generation) for recovery. No stamp.
    record_required_failure "install-only publish landed but retention is incomplete; preserving the workspace and marker for recovery (no stamp)"
    return 1
  elif [[ $io_publish_rc -ne 0 ]]; then
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
# main flow (the lanes and publish orchestration below never fire).
# `return` only works in a sourced file; when the script is executed normally
# the variable is unset, so this is a no-op.
if [[ ${UPDATE_SKILLS_LIB_ONLY:-} == 1 ]]; then
  # shellcheck disable=SC2317 # exit is reached when executed (return fails outside a sourced file)
  return 0 2>/dev/null || exit 0
fi

# --build-lanes: this process IS the env -i sub-invocation running inside a
# candidate fake HOME (see __gen_run_lanes). Run only the generation build lanes
# against the candidate store and exit; the parent handles recovery, validation,
# publish and fan-out. No lock and no stamp here.
if [[ -n $BUILD_LANES ]]; then
  __gen_do_build_lanes
  exit $?
fi

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
#     stale one. This is what lets the fresh-machine bootstrap run at apply time
#     without swapping anything. Destructive reconciliation (replace/remove)
#     runs solely in the full weekly path.
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

# Store names this vertical deliberately does not deliver to Claude Code: the
# lock's claudeDelivery table, value "none". The table says only what THIS
# vertical does; it names no other delivery mechanism and reads no other lock.
#
# FAIL OPEN, on purpose. An absent or unreadable lock yields an empty set, so
# the fan-out falls back to its previous behaviour (link the whole store). The
# other direction is unthinkable here: a jq that failed would otherwise mark
# every roster skill undelivered and step 2 of converge_dir would reap the
# ENTIRE Claude fan-out on one bad read.
__update_skills_claude_undelivered() {
  [[ -f $CUSTOM_SKILL_LOCK ]] || return 0
  jq -r '.claudeDelivery // {} | to_entries[] | select(.value == "none") | .key' \
    "$CUSTOM_SKILL_LOCK" 2>/dev/null || true
}

# 0 (true) when the lock's claudeDelivery table is PRESENT but not the known shape
# (an object whose every value is the string "none"). The schema gate
# (__gen_roster_schema_ok) rejects a malformed table, but it runs only in the
# mutating modes' setup; --dry-run reaches converge_claude_skills without it, and
# __update_skills_claude_undelivered fails OPEN on a wrong-shaped table (empty
# undelivered set), which would RESTORE a de-delivered skill's ~/.claude link. So
# the fan-out validates the table at the point of use and refuses (no fan-out, no
# reap) rather than fail open, in every mode. Absent or a valid "none" object is
# NOT malformed.
#
# Slurped, for the reason __gen_roster_schema_ok is: an unslurped `jq -e` reads a
# STREAM and answers for the LAST document only, so a lock with a second
# top-level `{}` appended looked table-less (and therefore fine) here while the
# reader above still read the first document's table. A multi-document lock is
# malformed for this purpose whatever its documents say.
__update_skills_claude_delivery_malformed() {
  [[ -f $CUSTOM_SKILL_LOCK ]] || return 1
  jq -e -s 'length == 1 and (.[0] |
      (has("claudeDelivery") | not)
      or ((.claudeDelivery | type == "object")
        and (.claudeDelivery | to_entries | all(.value == "none"))))' \
    "$CUSTOM_SKILL_LOCK" >/dev/null 2>&1 && return 1
  return 0
}

# Claude fan-out: every store skill (the roster minus the claudeDelivery "none"
# set) gets a ~/.claude/skills link. Claude is not profile-scoped, tiering there
# is the settings modify-template's job, not the fan-out's.
#
# The subtraction is what makes a de-delivered skill STAY de-delivered: this
# function used to link every store entry unconditionally, so removing a link by
# hand bought exactly one week before the next Monday put it back.
converge_claude_skills() {
  local -a desired=() undelivered=()
  local skill_path skill undelivered_name
  # Refuse the fan-out on a malformed claudeDelivery instead of failing open and
  # restoring a de-delivered link. No fan-out means no create AND no reap, so the
  # existing links are left exactly as they are (safe in every mode, including the
  # --dry-run preview that reaches here without the roster snapshot gate).
  if __update_skills_claude_delivery_malformed; then
    record_required_failure "converge: the roster's claudeDelivery table is malformed (not an object whose values are all \"none\"); refusing the Claude fan-out rather than fail open and restore a de-delivered link"
    return 0
  fi
  while IFS= read -r undelivered_name; do
    [[ -n $undelivered_name ]] && undelivered+=("$undelivered_name")
  done < <(__update_skills_claude_undelivered)
  for skill_path in "$STORE"/*; do
    [[ -d $skill_path || -L $skill_path ]] || continue
    skill="${skill_path##*/}"
    for undelivered_name in ${undelivered[@]+"${undelivered[@]}"}; do
      if [[ $undelivered_name == "$skill" ]]; then
        log "converge: $skill is claudeDelivery none; the Claude fan-out skips it"
        continue 2
      fi
    done
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
  log "ROUTING DRIFT: hermes-superpowers routing references no longer match the lock, re-asserting"
  if routing_output="$("$routing_script" 2>&1)"; then
    printf '%s\n' "$routing_output"
    log "ROUTING DRIFT: re-assert complete, something rewrote ~/.hermes/skills/hermes-superpowers (a superpowers re-mirror?); find out what stomped it"
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
# skill the lock's hermesRegistry table marks hermes-owned for it, keyed by the
# entry's lockKey (never a list name: ClawHub slugs differ from frontmatter
# names, and hermes's own list output shows hub-linked skills as "local").
# Failure isolation is per skill AND per profile: one blocked/broken update logs
# a WARN (and relays it, soft-gated like fork drift) and the loop continues, the
# weekly run must never die on a single skill. "Blocked" output with exit 0 is a
# warning too: updates re-apply hermes's install gate on changed content, and a
# block needs operator eyes, not a silent pass. held: true entries are skipped
# visibly (none currently held). Never --force (bypassing a security scan needs
# per-invocation operator confirmation), never uninstall. Network-dependent, so
# --install-only never reaches it; the weekly run is where it belongs.
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
  # Profiles to walk: every profile owning a registry skill, default included
  # (`hermes -p default` addresses Bob's root profile; un-entanglement done).
  local -a walk_profiles=()
  while IFS= read -r profile; do
    [[ -n $profile ]] && walk_profiles+=("$profile")
  done < <(jq -r '.hermesRegistry // {} | [.[].profiles[]?] | unique | .[]' "$CUSTOM_SKILL_LOCK")
  for profile in "${walk_profiles[@]}"; do
    # read on fd 3: the loop body runs hermes, which may consume stdin
    while IFS=$'\t' read -r -u3 skill lock_key held; do
      if [[ $held == "true" ]]; then
        log "hermes $profile/$skill: held, skipped (see the lock's hermesRegistry note)"
        continue
      fi
      if [[ $DRYRUN == "--dry-run" ]]; then
        log "would update via hermes -p $profile: $lock_key"
        continue
      fi
      if update_output="$(hermes -p "$profile" skills update "$lock_key" 9>&- 2>&1)"; then
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
# ever write through it, the only sanctioned refresh is the app's own updater,
# `cua-driver skills update`, which re-fetches the versioned pack from GitHub
# Releases and re-plants the agent links (verified: `cua-driver skills status`
# links Claude Code, Codex, via the store, AND hermes itself). Gated on the
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
  # F8: close the lock fd (9) so a long-lived child the app updater might leave
  # behind never keeps the serialize lock held after this run exits.
  if refresh_output="$(cua-driver skills update 9>&- 2>&1)"; then
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
  # F1: capture the acquisition STATUS; do not collapse contention and hard
  # failure into one silent exit 0. Contention (lockf EX_TEMPFAIL 75) is a
  # RETRYABLE deferral in EVERY mode: exit the distinct 75 so the first-install
  # wrapper preserves its retry marker and a weekly slot simply writes no stamp
  # and lets a later slot retry. Any OTHER non-zero (unwritable ~/.agents so
  # `exec 9>>` failed) is a REQUIRED failure: loud warn + relay, no stamp, a
  # non-zero exit (the wrapper keeps its marker), never a silent success.
  __update_skills_lock_rc=0
  __update_skills_acquire_lock || __update_skills_lock_rc=$?
  if [[ $__update_skills_lock_rc -eq 75 ]]; then
    log "another run holds the lock; deferring (retryable, exit 75)"
    __update_skills_record deferred "nothing was attempted: another update-skills run already holds the serialize lock, so this slot deferred (exit 75). A later slot retries."
    exit 75
  elif [[ $__update_skills_lock_rc -ne 0 ]]; then
    record_required_failure "could not acquire the serialize lock (rc $__update_skills_lock_rc; e.g. ~/.agents is not writable); no build, no publish, no stamp"
    __update_skills_alert "update-skills could not acquire its serialize lock (rc $__update_skills_lock_rc). Check that ~/.agents is writable, then re-run ~/.local/bin/update-skills.sh."
    __update_skills_record deferred "nothing was attempted: the serialize lock could not be acquired (rc $__update_skills_lock_rc), so the run refused. An alert was also attempted on the priority route; that path is fire-and-forget, so its delivery was not observed."
    exit 1
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
      __update_skills_record deferred "nothing was attempted: the run REFUSED because the roster lock at $GEN_ROSTER_SOURCE is missing, unparseable or schema-broken. An alert was also attempted on the priority route; that path is fire-and-forget, so its delivery was not observed."
      exit 1
    fi
    # F9: the snapshot succeeded, so GEN_ROSTER_SNAPSHOT_FILE is a live temp
    # file. Install its cleanup trap NOW, before any later refusal exit (the
    # zero-union guard below), so a refused run never leaks the mktemp. The
    # trailing `true` keeps the trap from ever altering the exit status.
    trap '[[ -n ${GEN_ROSTER_SNAPSHOT_FILE:-} ]] && rm -f "$GEN_ROSTER_SNAPSHOT_FILE"; true' EXIT
    __update_skills_tracked_count=0
    while IFS= read -r __update_skills_tracked_probe; do
      [[ -n $__update_skills_tracked_probe ]] && __update_skills_tracked_count=$((__update_skills_tracked_count + 1))
    done < <(__gen_tracked_names)
    # F3: a zero tracked UNION is refused UNCONDITIONALLY, independent of the
    # live-generation state. There is no legitimate empty roster (the committed
    # roster always has entries), so a zero union is corruption or a broken
    # deploy. Gating on a non-empty live generation let a fresh machine (no
    # generation) or a damaged current (present, zero skill dirs) migrate over
    # zero names, publish an EMPTY generation, and stamp success.
    if [[ $__update_skills_tracked_count -eq 0 ]]; then
      record_required_failure "the roster tracks ZERO skills (empty npx+clawhub union); refusing any mutation (there is no legitimate empty roster; a zero union is corruption or a broken deploy, not intent)"
      __update_skills_alert "update-skills refused to run: the roster lock tracks no skills. If delisting everything is truly intended, remove the generation by hand; otherwise restore the roster (chezmoi apply) and re-run."
      __update_skills_record deferred "nothing was attempted: the run REFUSED because the roster lock tracks ZERO skills, which is corruption or a broken deploy rather than intent. An alert was also attempted on the priority route; that path is fire-and-forget, so its delivery was not observed."
      exit 1
    fi
  fi
  # RECOVERY (brief step 1) runs under the lock, BEFORE the stamp early-exit, so
  # a crash-window leftover self-heals even on a slot that then early-exits. It deletes incomplete staging, marks a reusable complete
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
  # The completed entry for this week already claimed the guard, so this is
  # normally a no-op. It is here so a week whose completed entry failed to write
  # its guard still leaves one message rather than none.
  __update_skills_record deferred "nothing was attempted: this ISO week already completed successfully for the current roster and updater, so this slot was a no-op."
  exit 0
fi

# A run under a live agent session is SAFE, so there is no activity gate here.
# There used to be one: the run deferred while claude/codex/hermes showed recent
# file activity, which on a machine that is always in use meant every scheduled
# slot deferred and the update never ran at all. Two properties make the gate
# unnecessary. The publish is ONE atomic exchange with one retained generation,
# so any path resolved during or after it yields a complete tree from exactly
# one generation, never a half-written one. And a harness reads skill content at
# invocation time, so the worst a swap mid-session costs is that the next
# invocation reads the new copy. Serialization against a SECOND updater still
# matters and is kept: that is the kernel lock taken above.
__update_skills_note_scheduled_attempt

# fork/vendored upstream drift-check: for each lock forks entry, fetch the
# upstream and compare the recorded skill path's current git hash (tree hash for
# a folder, blob hash for a single-file skill) against lastComparedTreeHash, the
# hash at the last HUMAN comparison. Drift means the upstream shipped changes
# nobody has reviewed against the local copy yet: alert and move on. This pass
# only ever reads; the vendored store content is untouchable here by
# construction (nothing below writes to $STORE). An unreachable upstream is a
# reported warning, never a failure, the weekly run must survive a dead network.
#
# Everything this phase can go wrong with is ADVISORY: every outcome is a
# LOUD report (log line plus a relay push carrying its own state) and the phase
# still returns 0. It must never fail the weekly run, which by the time it
# reaches here has already published a generation and has yet to write its
# success stamp. run_fork_drift_watch is what makes that structural.
#
# The rule the reporting follows: an upstream this run did not compare is
# something the operator has to be TOLD about, whatever the reason. A skip that
# only reaches ~/.local/log/skills/ is a fork nobody is watching and nobody
# knows is unwatched, which is the failure this whole phase exists to prevent.

# The skillPath value meaning "the whole repository", where the comparison is
# against HEAD's root tree rather than a path inside it.
FORK_SKILL_PATH_WHOLE_REPO="."

# The fields the walk reads out of a forks entry. Named once so the validator,
# the reads and the operator-facing message cannot drift apart.
FORK_ENTRY_REQUIRED_FIELDS=(sourceUrl skillPath lastComparedTreeHash)

# Relay states this phase can report. Each is a distinct operator remedy, so
# they must stay distinguishable downstream, not collapse into one alert.
FORK_RELAY_STATE_DRIFT="fork-drift"                               # upstream content moved; compare and port by hand
FORK_RELAY_STATE_PATH_MISSING="fork-path-missing"                 # upstream still there; our recorded path is not
FORK_RELAY_STATE_LOCK_MALFORMED="fork-lock-broken"                # the lock itself cannot be walked
FORK_RELAY_STATE_LOCK_MISSING="fork-lock-missing"                 # there is no lock file to walk at all
FORK_RELAY_STATE_FORKS_TABLE_ABSENT="fork-table-absent"           # the lock parses but carries no forks table
FORK_RELAY_STATE_UPSTREAM_UNREACHABLE="fork-upstream-unreachable" # the fetch failed; nothing was compared
FORK_RELAY_STATE_UPSTREAM_HEADLESS="fork-upstream-headless"       # the fetch worked; the clone has no HEAD to compare against
FORK_RELAY_STATE_CLONE_UNSTAGEABLE="fork-clone-unstageable"       # no temp dir to fetch into; nothing was compared
FORK_RELAY_STATE_CLONE_TIMEOUT="fork-clone-timeout"               # the fetch never answered and was stopped; nothing was compared
FORK_RELAY_STATE_WALK_INCOMPLETE="fork-walk-incomplete"           # fewer entries reached the walk than the table holds

# --project labels for the advisories that are about the lock itself rather
# than about one named fork, so a downstream consumer can tell "the file does
# not parse" from "the forks table is the wrong shape" without reading prose.
# The `lock:` prefix keeps them OUT of the fork name space: every other push
# from this phase carries a fork name as its project, and a fork literally
# named `lock` or `forks` would otherwise be indistinguishable downstream.
# Nothing about a colon makes that structural on its own (measured: APFS
# accepts a directory named `a:b`), so these two labels are RESERVED at build
# time instead: test/unit/skills-roster-fanout.sh reads them out of this file
# and fails when a forks key claims either.
FORK_RELAY_PROJECT_LOCK_FILE="lock:file"
FORK_RELAY_PROJECT_FORKS_TABLE="lock:forks-table"

# Where a drift clone is staged, and under what name. The template is spelled
# out rather than left to a bare `mktemp -d` for two reasons. It makes the
# location honour TMPDIR (measured on macOS 26.2 / Darwin 25.2: `mktemp -d`
# ignores TMPDIR entirely, with and without -t, and always lands in the
# per-user Darwin temp dir, so a bare form is untestable and unredirectable),
# and it puts this script's name on anything it ever fails to clean up. A
# TMPDIR that is set but EMPTY is not a usable directory, so `:-` treating it
# like unset is the wanted reading here.
FORK_CLONE_FALLBACK_TMPDIR="/tmp"
FORK_CLONE_DIR_TEMPLATE="update-skills-fork-drift.XXXXXX"

# git 2.55.0, `man git`: "If this Boolean environment variable is set to false,
# git will not prompt on the terminal (e.g., when asking for HTTP
# authentication)." That is what this phase wants. The weekly run is
# unattended, and a sourceUrl that has gone private would otherwise park the
# whole update on a prompt nobody can see; false makes git fail fast with a
# message, which this phase then reports.
GIT_TERMINAL_PROMPT_FALSE="0"

# How long ONE drift clone may run before this phase stops it, and how long a
# stopped clone gets to unwind before it is killed outright. A deadline is what
# keeps the phase's promise literal: it runs in the weekly flow AFTER the
# generation exchange has published and BEFORE the success stamp is written, so
# a fetch that never answers (a remote that accepts the connection and then goes
# quiet, a transport helper waiting on input nobody can give, a proxy that
# swallows the request) does not skip one fork, it parks the whole weekly update
# and every later slot stalls at the same line. Not returning is a worse
# violation of "advisory" than any wrong report, so an upstream that has not
# answered by the deadline is stopped and REPORTED like every other upstream
# this run did not compare. GIT_TERMINAL_PROMPT=0 covers only the prompt; every
# other way a fetch can hang needs this.
#
# 5 minutes is far past a --depth 1 clone of any watched upstream (they are
# single-skill repositories) and far short of the hourly retry slot. The
# override exists for tests; a value that is not a positive whole number of
# seconds is the DEFAULT, never taken at face value: 0 would stop every clone at
# once, and a non-number would make the comparison below a shell error.
FORK_CLONE_DEADLINE_SECONDS="${UPDATE_SKILLS_FORK_CLONE_DEADLINE:-300}"
[[ $FORK_CLONE_DEADLINE_SECONDS =~ ^[1-9][0-9]*$ ]] || FORK_CLONE_DEADLINE_SECONDS=300
FORK_CLONE_STOP_GRACE_SECONDS=2
# The watchdog POLLS, because bash has no wait-with-timeout. A quarter second,
# not a whole one: the check runs once before the first sleep, so a whole-second
# interval charged every healthy clone a second it did not need. The deadline is
# counted in TICKS so the fractional interval cannot drift away from the seconds
# the operator-facing message quotes. Both sleeps this runs under accept
# fractions (measured: macOS /bin/sleep on 26.2, and GNU coreutils 9.7 in the
# flake's shell, which is what CI uses).
FORK_CLONE_POLL_INTERVAL="0.25"
FORK_CLONE_POLL_TICKS_PER_SECOND=4

# Path git reads instead of the file-based global and system config while
# cloning a drift upstream. WHY: the recorded sourceUrl is an anonymous public
# HTTPS URL, and this repo deliberately ships
# `url."git@github.com:".insteadOf = https://github.com/` (dot_gitconfig.tmpl),
# which silently converts that fetch to SSH; every SSH failure then degrades to
# "upstream unreachable", a permanently silent skip. Measured on git 2.55.0:
# GIT_CONFIG_GLOBAL covers BOTH ~/.gitconfig and $XDG_CONFIG_HOME/git/config,
# and GIT_CONFIG_SYSTEM covers /etc/gitconfig. What these clones give up along
# with the rewrite: globally configured proxies, custom CA bundles and
# credential helpers. That is the intent, an anonymous public clone needs none
# of them.
GIT_CONFIG_NEUTRALIZED_PATH="/dev/null"

# The two COMMAND-scope config channels, which the file ones above do not cover
# (measured on git 2.55.0, both redirect the clone to another repository while
# every report still names the recorded URL: a fork reported as drifted against
# an upstream that had not changed). GIT_CONFIG_COUNT=0 tells git there are no
# GIT_CONFIG_KEY_n/GIT_CONFIG_VALUE_n pairs to apply, whatever the inherited
# environment holds; an empty GIT_CONFIG_PARAMETERS is an empty list, and that
# variable is not exotic, it is how `git -c foo=bar` propagates to every
# subprocess it starts, hooks included, so any run started from inside a git
# command can carry one.
GIT_CONFIG_COUNT_NONE="0"
GIT_CONFIG_PARAMETERS_NONE=""

# Soft-gate on relay.sh, exactly like the pre-commit hook's gitleaks stage: its
# absence is a silent skip, not an error. `|| true` because an advisory
# notification must never decide the run's exit status. A relay push reaches
# the operator's phone, so it is a side effect --dry-run must not have: the dry
# preview still LOGS every finding, it just does not notify anyone about it.
#
# `9>&-` closes the serialize lock's fd for relay and everything it spawns, for
# the same reason it is on the drift clone below. relay.sh is fire-and-forget:
# it DETACHES its three channels (two `curl -m 10` and one terminal-notifier)
# and exits without waiting, so those children outlive this whole run. The lock
# is a kernel flock on fd 9 which the kernel holds until the LAST copy of the fd
# closes, so a detached child that inherited it keeps the lock held after the
# updater has exited, and the next scheduled slot defers with exit 75 over a
# competing run that does not exist. Measured with the real relay.sh against a
# blackholed endpoint: without this close the lock outlived the run by 10s
# (lsof named the detached curls as the holders), and with it the lock is free
# the moment the run exits while those same curls are still running. That widest
# window is the one a dead network opens, which is also the condition that makes
# every upstream unreachable and pushes from every state at once. Closing an fd
# that was never opened (no /usr/bin/lockf, so no lock) is a no-op.
#
# Deliberately NOT under a deadline, unlike the clone. That clone waits on a
# remote by construction; relay.sh performs no synchronous network I/O at all,
# every channel that can block is already detached behind its own `-m 10`.
# Measured with all three channels blocked for 20s: relay.sh still returned in
# 95ms. A hand-rolled deadline here would bound the WAIT and not the work (the
# channels are detached already, so killing relay does not stop them) while
# adding a kill path that can cut a push between its channels, on a call site
# that fires up to fifteen times a run.
relay_fork_advisory() {
  local state="$1" fork="$2" detail="$3"
  local relay_script="$HOME/.local/bin/relay.sh"
  if [[ $DRYRUN == "--dry-run" ]]; then
    return 0
  fi
  [[ -x $relay_script ]] || return 0
  "$relay_script" --agent update-skills --state "$state" --project "$fork" \
    --detail "$detail" 9>&- || true
}

notify_fork_drift() {
  local fork="$1" source_url="$2"
  log "FORK DRIFT: $fork, upstream $source_url has changed since the last comparison"
  log "FORK DRIFT: compare upstream and port wanted changes into the vendored copy by hand (see CLAUDE.md, Agent Skills), then set forks[\"$fork\"].lastComparedTreeHash to the new upstream hash; the vendored copy itself was not modified"
  relay_fork_advisory "$FORK_RELAY_STATE_DRIFT" "$fork" \
    "upstream $source_url changed since the last comparison; compare and port wanted changes by hand, then bump lastComparedTreeHash"
}

# The upstream is reachable and the recorded path is not in it: upstream moved
# or deleted it. Distinct from drift because the remedies are opposite. Drift
# says "bump the hash once you have compared"; here there IS no hash to bump
# under the recorded path, and bumping one would silence a comparison that has
# never happened.
notify_fork_path_missing() {
  local fork="$1" source_url="$2" skill_path="$3"
  log "FORK PATH MISSING: $fork, the recorded skillPath \"$skill_path\" no longer exists in upstream $source_url"
  log "FORK PATH MISSING: re-point forks[\"$fork\"].skillPath at the path upstream moved the skill to, and LEAVE lastComparedTreeHash alone; bumping it would silence a drift nobody has reviewed"
  relay_fork_advisory "$FORK_RELAY_STATE_PATH_MISSING" "$fork" \
    "the recorded skillPath \"$skill_path\" is gone from upstream $source_url; re-point skillPath, do not bump lastComparedTreeHash"
}

# The fetch failed, so NOTHING about this upstream was compared. Reported like
# every other outcome rather than logged and forgotten: the url-rewrite defect
# was one CAUSE of a permanently silent skip, and every other cause of a
# durable clone failure (an upstream renamed, deleted or made private, a proxy,
# DNS, a rewrite arriving through GIT_CONFIG_COUNT, which this phase documents
# as still applying) leaves the same fork unwatched forever. git's own message
# rides along because "unreachable" alone cannot tell a dead network from a
# dead URL, and those have opposite remedies.
notify_fork_upstream_unreachable() {
  local fork="$1" source_url="$2" clone_error="$3" error_line
  log "FORK UNREACHABLE: $fork, upstream $source_url could not be fetched; it was NOT drift-checked this run"
  while IFS= read -r error_line; do
    [[ -n $error_line ]] || continue
    log "FORK UNREACHABLE: $fork, git said: $error_line"
  done <<<"$clone_error"
  relay_fork_advisory "$FORK_RELAY_STATE_UPSTREAM_UNREACHABLE" "$fork" \
    "upstream $source_url could not be fetched, so $fork was not drift-checked; check the recorded sourceUrl and this host's network"
}

# The fetch worked and there is still nothing to compare against: the clone has
# no HEAD that resolves to a commit. Upstream renamed its default branch and
# left the symbolic ref pointing at the old name, or the repository is empty. A
# clone of one succeeds and merely warns that it looks empty (measured, git
# 2.55.0), after which every path lookup under HEAD fails, which is why this
# used to be reported as a missing skillPath: the remedy there is to re-point
# skillPath, and the path is not what is missing.
notify_fork_upstream_headless() {
  local fork="$1" source_url="$2"
  log "FORK NO UPSTREAM HEAD: $fork, upstream $source_url was cloned but its HEAD does not resolve to a commit; it was NOT drift-checked this run"
  log "FORK NO UPSTREAM HEAD: check the upstream's default branch (a rename leaves HEAD pointing at a branch that is gone, and an empty repository has no commit at all), and LEAVE forks[\"$fork\"].skillPath and lastComparedTreeHash alone; the recorded path is not what is missing"
  relay_fork_advisory "$FORK_RELAY_STATE_UPSTREAM_HEADLESS" "$fork" \
    "upstream $source_url cloned but its HEAD does not resolve to a commit, so $fork was not drift-checked; check the upstream's default branch, the recorded skillPath is not what is missing"
}

# The fetch was still running at its deadline and was stopped, so nothing about
# this upstream was compared. Distinct from unreachable: an unreachable upstream
# ANSWERED with a failure git can quote, while this one said nothing at all, and
# the causes are different (a stalled remote, a proxy swallowing the request, a
# transport helper waiting on input) even though both leave the fork unwatched.
notify_fork_clone_timeout() {
  local fork="$1" source_url="$2" deadline_seconds="$3"
  log "FORK CLONE TIMED OUT: $fork, upstream $source_url did not finish cloning within ${deadline_seconds}s; the clone was stopped and this upstream was NOT drift-checked this run"
  log "FORK CLONE TIMED OUT: check whether $source_url still answers an anonymous clone from this host (a stalled remote, a proxy, or a URL that has gone private and is waiting on credentials); the weekly run continued"
  relay_fork_advisory "$FORK_RELAY_STATE_CLONE_TIMEOUT" "$fork" \
    "upstream $source_url did not finish cloning within ${deadline_seconds}s and was stopped, so $fork was not drift-checked; check whether that URL still answers an anonymous clone from this host"
}

# No temp dir means no clone, which means this upstream went unchecked for a
# reason that has nothing to do with the upstream. Same reporting rule: an
# unchecked fork is reported, never merely logged.
notify_fork_clone_unstageable() {
  local fork="$1" clone_parent_dir="$2"
  log "FORK NOT CHECKED: $fork, could not create a temp dir under $clone_parent_dir to clone the upstream into; it was NOT drift-checked this run"
  relay_fork_advisory "$FORK_RELAY_STATE_CLONE_UNSTAGEABLE" "$fork" \
    "could not create a temp dir under $clone_parent_dir to stage a drift clone, so $fork was not drift-checked; check TMPDIR and free space"
}

# There is no lock, so there is no watch. Distinct from an unparseable lock:
# the remedy is to deploy the file (chezmoi apply), not to repair its JSON.
# Only the two read-only modes can reach it, since the mutating flows read a
# validated snapshot that exists by construction, and silence was exactly the
# wrong answer for the modes you run to find out whether the watch is healthy:
# "no lock" and "no drift" printed the same nothing.
notify_fork_lock_missing() {
  local lock_file="$1"
  log "fork drift-check: $lock_file does not exist; NO fork upstream is being watched this run, deploy the lock (chezmoi apply)"
  relay_fork_advisory "$FORK_RELAY_STATE_LOCK_MISSING" "$FORK_RELAY_PROJECT_LOCK_FILE" \
    "$lock_file does not exist; no fork upstream was drift-checked"
}

# Fewer keys reached the walk than the forks table holds. Nothing in this
# script can currently do that, which is the point: a feed that silently
# truncates (a jq too old for --raw-output0, a read that stops early) unwatches
# upstreams while every walked entry still reports clean, and a clean report
# over a short walk is indistinguishable from a healthy run. Counting is the
# only thing that can tell them apart.
notify_fork_walk_incomplete() {
  local walked="$1" expected="$2"
  log "fork drift-check: only $walked of $expected forks entries reached the walk; the rest were NOT drift-checked this run"
  relay_fork_advisory "$FORK_RELAY_STATE_WALK_INCOMPLETE" "$FORK_RELAY_PROJECT_FORKS_TABLE" \
    "only $walked of $expected forks entries were walked; the rest were not drift-checked"
}

# Does the lock parse as EXACTLY ONE JSON object? Asked separately from the
# forks table's shape because the two need different remedies: a file that does
# not parse is not a malformed `forks` table, and reporting it as one sends the
# operator to the wrong line of an otherwise healthy table.
#
# The count is half the question, not pedantry. jq reads a file as a SEQUENCE of
# values and a predicate reports the status of the LAST one, so `{}{}` passed
# the old `type == "object"` test and every read below then ran once per value:
# two empty objects walked nothing and printed a clean all-clear, and two
# populated ones joined each field's two answers into one string, which reported
# a reachable upstream as unreachable, walked every entry twice, and announced
# that "only 2 of 0" entries had reached the walk. Slurping is what makes the
# count askable; the lock is a few kilobytes, so reading it whole costs nothing.
lock_is_readable_json_object() {
  local lock_file="$1"
  jq -e -s 'length == 1 and (.[0] | type == "object")' "$lock_file" >/dev/null 2>&1
}

# Is there a forks table at all? Asked before its shape, because absence is NOT
# "nothing to watch". An empty OBJECT is how a lock says that deliberately; an
# absent key is what a typo (`forkss`, `Forks`) or a hand-edit that dropped the
# table leaves behind, and treating it as legal printed exactly what a healthy
# zero-drift run prints. The weekly run then published a generation, stamped the
# week a success, compared no vendored upstream, and nothing anywhere said so,
# which is the silence this whole phase exists to prevent. The committed lock
# always carries the table, and test/unit/skills-roster-fanout.sh fails when it
# stops covering every vendored skill, so an absent one on a live machine means
# the deployed file is not the committed one.
fork_table_is_present() {
  local lock_file="$1"
  jq -e 'has("forks")' "$lock_file" >/dev/null 2>&1
}

# Is the lock's forks table a shape the drift-watch can walk?
# Present-but-not-an-object is corruption, and both of its outcomes were wrong
# before this gate: false/null/a string/[] walked zero entries and reported a
# silent all-clear over an unwatched fork set, and an ARRAY made the per-entry
# jq index error out, which under `set -euo pipefail` aborted the whole run.
# Since the roster gate no longer refuses a mutating run over a malformed forks
# table (it is advisory data, see __gen_roster_schema_ok), this guard is the
# ONLY thing standing between a corrupt table and every mode of this script, not
# a tolerant backstop for two read-only modes.
fork_table_is_object() {
  local lock_file="$1"
  jq -e '.forks | type == "object"' "$lock_file" >/dev/null 2>&1
}

# Is this ENTRY an object, so the per-field reads below cannot error? Same
# failure shape as the array table, one level down: a string, array or number
# entry made `jq '.forks[$fork].sourceUrl'` fail with "Cannot index string with
# string", and a failing command substitution in an assignment aborts the whole
# run under `set -euo pipefail`. Like fork_table_is_object, this is the only
# validation a malformed entry now meets in ANY mode.
fork_entry_is_object() {
  local lock_file="$1" fork="$2"
  jq -e --arg fork "$fork" '.forks[$fork] | type == "object"' "$lock_file" >/dev/null 2>&1
}

# Which of the fields the walk reads are NOT a non-empty, control-character-free
# STRING, as a comma-separated list; empty output means the entry is walkable.
# Answered per entry so one broken entry cannot silence the others, and it names
# the FIELD because "this entry is malformed" is not a remedy anyone can execute.
#
# The TYPE half is load-bearing, not decoration. `jq -r` renders any scalar as
# text, so a hash written unquoted (12345) read back as the string "12345",
# matched no real hash, and cried FORK DRIFT every week forever; a numeric
# sourceUrl read back as "42" and was reported as an unreachable NETWORK for
# what is a broken lock; a boolean skillPath resolved as the literal path
# "true" and was reported as a path upstream had deleted. All three are
# permanent, and all three send the operator somewhere the defect is not.
#
# CONTROL CHARACTERS are the other half of the type check, and they are the
# half that is invisible. A string can be the right type and still not be the
# value it looks like: bash drops NUL bytes out of a command substitution (with
# a warning that reaches a log nobody reads) and strips trailing newlines from
# one silently, so a URL, path or hash recorded with either read back as a
# DIFFERENT, entirely plausible value. Measured on all three fields, both kinds:
# every one was compared as the healthy value and reported "upstream unchanged",
# which is a reassuring answer to a question nobody asked. None of the three
# fields can legitimately carry one, so the whole class is a broken lock entry.
#
# The caller must have established the entry is an OBJECT first: indexing a
# scalar is a jq error, not a false.
fork_entry_unusable_fields() {
  local lock_file="$1" fork="$2"
  jq -r --arg fork "$fork" '
    .forks[$fork] as $entry
    | $ARGS.positional
    | map(select(
        ($entry[.] | type) != "string"
        or $entry[.] == ""
        or ($entry[.] | explode | any(. < 32 or . == 127))))
    | join(", ")
  ' "$lock_file" --args "${FORK_ENTRY_REQUIRED_FIELDS[@]}" 2>/dev/null
}

# One report for every way an entry can be unusable, because they are one
# operator action (fix that lock entry), with the REASON carried through so the
# log can say which way it was broken. Without the reason the two callers
# produce byte-identical lines for "this is not an object at all" and "this
# field is the wrong type", and the operator has to re-derive which.
notify_fork_entry_malformed() {
  local fork="$1" reason="$2"
  log "fork drift-check $fork: the lock entry is malformed ($reason); this upstream is NOT being watched, fix the lock"
  relay_fork_advisory "$FORK_RELAY_STATE_LOCK_MALFORMED" "$fork" \
    "the forks entry for $fork is malformed ($reason); it was not drift-checked"
}

# Stop a clone that is past its deadline: TERM first so git can unwind and drop
# its lock and partial objects, KILL if it is still there after the grace. Never
# fails: a clone that exited on its own between the deadline check and here is
# the same outcome as one that took the signal.
stop_fork_clone() {
  local clone_pid="$1" waited=0
  kill -TERM "$clone_pid" 2>/dev/null || return 0
  while [[ $waited -lt $FORK_CLONE_STOP_GRACE_SECONDS ]] && kill -0 "$clone_pid" 2>/dev/null; do
    sleep 1
    waited=$((waited + 1))
  done
  if kill -0 "$clone_pid" 2>/dev/null; then
    kill -KILL "$clone_pid" 2>/dev/null || true
  fi
  wait "$clone_pid" 2>/dev/null || true
  return 0
}

# Clone a public upstream anonymously at the URL exactly as recorded, under a
# deadline. Single responsibility: fetch. --depth 1 suffices, only HEAD's tree
# is ever compared. Returns 0 when the clone finished, 2 when it was still
# running at the deadline and was stopped, 1 for any other failure; callers
# report what happened, nothing here decides anything.
#
# git's own diagnostics go to <output-file> rather than to stdout. A mute
# failure cannot tell a dead network from a renamed upstream from a config
# channel still rewriting the URL, and telling those apart is this phase's whole
# subject, but a BACKGROUND job cannot be read back through a command
# substitution: the capture would block on exactly the fetch this deadline
# exists to survive, and a killed clone can leave a transport helper holding
# that pipe open long after git is gone. A file has neither problem, and it is
# staged inside the clone dir, so discard_fork_clone takes it with everything
# else.
#
# `--` separates options from the URL: a sourceUrl the lock records with a
# leading dash would otherwise be read as a git option (`--upload-pack=...` is
# the classic), turning a data typo into a command.
#
# `9>&-` closes the serialize lock's fd in the clone's process tree, and it is
# not decoration. That lock is a kernel flock held on fd 9 for this process's
# lifetime and INHERITED by every child, and the kernel keeps it held until the
# LAST copy of the fd closes. A stopped clone can leave a transport helper
# behind (killing git does not reap a helper that never reads its stdin), so
# without this the next scheduled slot finds the lock still held by a process
# nobody is waiting on and defers with exit 75, turning one stalled upstream
# into a weekly update that never runs again. Measured: with fd 9 inherited, a
# run following a stalled clone deferred instead of walking its forks. Closing
# an fd that was never opened (no /usr/bin/lockf, so no lock) is a no-op.
clone_fork_upstream() {
  local source_url="$1" destination="$2" output_file="$3"
  local clone_pid ticks=0
  local deadline_ticks=$((FORK_CLONE_DEADLINE_SECONDS * FORK_CLONE_POLL_TICKS_PER_SECOND))
  GIT_CONFIG_GLOBAL="$GIT_CONFIG_NEUTRALIZED_PATH" \
    GIT_CONFIG_SYSTEM="$GIT_CONFIG_NEUTRALIZED_PATH" \
    GIT_CONFIG_COUNT="$GIT_CONFIG_COUNT_NONE" \
    GIT_CONFIG_PARAMETERS="$GIT_CONFIG_PARAMETERS_NONE" \
    GIT_TERMINAL_PROMPT="$GIT_TERMINAL_PROMPT_FALSE" \
    git clone --quiet --depth 1 -- "$source_url" "$destination" >"$output_file" 2>&1 9>&- &
  clone_pid=$!
  while [[ $ticks -lt $deadline_ticks ]] && kill -0 "$clone_pid" 2>/dev/null; do
    sleep "$FORK_CLONE_POLL_INTERVAL"
    ticks=$((ticks + 1))
  done
  if kill -0 "$clone_pid" 2>/dev/null; then
    stop_fork_clone "$clone_pid"
    return 2
  fi
  wait "$clone_pid" && return 0
  return 1
}

# Remove a staged clone. Fail-safe and LOUD: residue in the temp dir is worth a
# warning and is never a reason to abort an advisory phase.
discard_fork_clone() {
  local clone_dir="$1"
  rm -rf "$clone_dir" 2>/dev/null && return 0
  log "WARN: fork drift-check could not remove the staged clone at $clone_dir; it is left behind"
  return 0
}

# Print the current git hash of the recorded skill path in a cloned upstream
# and return 0; print nothing and return 1 when that path is not in HEAD.
# Detection is by git's EXIT STATUS, never by matching its output against a
# sentinel: `git rev-parse` ECHOES an unresolvable argument to stdout before
# failing (measured, git 2.55.0), so the old `|| echo missing-path` produced
# "HEAD:SKILL.md\nmissing-path", which compares equal to neither the sentinel
# nor any hash, and therefore reported content drift forever.
# Does this clone have a HEAD to compare against at all? Asked before the path
# lookup, because a clone with no resolvable HEAD fails EVERY path lookup, and
# "the path is gone" is then a report about the wrong end of the comparison.
fork_clone_head_resolves() {
  local repo_dir="$1"
  git -C "$repo_dir" rev-parse --verify --quiet 'HEAD^{commit}' >/dev/null 2>&1
}

resolve_fork_upstream_hash() {
  local repo_dir="$1" skill_path="$2" revision
  if [[ $skill_path == "$FORK_SKILL_PATH_WHOLE_REPO" ]]; then
    revision='HEAD^{tree}'
  else
    revision="HEAD:$skill_path"
  fi
  git -C "$repo_dir" rev-parse --verify --quiet "$revision" 2>/dev/null
}

check_fork_drift() {
  # WHAT THE PHASE READS vs WHAT IT NAMES. Every read below goes to
  # CUSTOM_SKILL_LOCK, which in the weekly flow is the roster SNAPSHOT taken for
  # the transaction, a temp file deleted when the process exits. Every message
  # names the DEPLOYED lock instead, because a remedy that points at a path the
  # operator cannot open is not a remedy: they were told to fix
  # /var/folders/.../update-skills-roster.XXXXXX while the deployed file (and
  # the committed source behind it) kept the typo and every later slot repeated
  # the alert. GEN_ROSTER_SOURCE is empty in the read-only modes, which never
  # snapshot, so there the two are the same file.
  local lock_name="${GEN_ROSTER_SOURCE:-$CUSTOM_SKILL_LOCK}"
  if [[ ! -f $CUSTOM_SKILL_LOCK ]]; then
    notify_fork_lock_missing "$lock_name"
    return 0
  fi
  if ! lock_is_readable_json_object "$CUSTOM_SKILL_LOCK"; then
    log "fork drift-check: $lock_name does not parse as a JSON object (a lock holds exactly one); NO fork upstream is being watched this run, fix the lock"
    relay_fork_advisory "$FORK_RELAY_STATE_LOCK_MALFORMED" "$FORK_RELAY_PROJECT_LOCK_FILE" \
      "$lock_name does not parse as a JSON object; no fork upstream was drift-checked"
    return 0
  fi
  if ! fork_table_is_present "$CUSTOM_SKILL_LOCK"; then
    log "fork drift-check: $lock_name carries no forks table; NO fork upstream is being watched this run, restore the table (a lock with deliberately nothing to watch says so with an empty one)"
    relay_fork_advisory "$FORK_RELAY_STATE_FORKS_TABLE_ABSENT" "$FORK_RELAY_PROJECT_FORKS_TABLE" \
      "$lock_name has no forks table at all; no fork upstream was drift-checked"
    return 0
  fi
  if ! fork_table_is_object "$CUSTOM_SKILL_LOCK"; then
    log "fork drift-check: the forks table in $lock_name is present but not an object; NO fork upstream is being watched this run, fix the lock"
    relay_fork_advisory "$FORK_RELAY_STATE_LOCK_MALFORMED" "$FORK_RELAY_PROJECT_FORKS_TABLE" \
      "the forks table in $lock_name is present but not an object; no fork upstream was drift-checked"
    return 0
  fi
  local fork source_url skill_path last_compared_tree_hash current_tree_hash
  local clone_parent_dir clone_dir clone_error clone_rc unusable_fields
  local expected_entries walked_entries=0
  # ONE read sizes the walk and feeds it: the count is the FIRST record of the
  # same stream the keys arrive on. Two reads was the defect. Their failures
  # cancelled, because a failed count was coerced to 0 while a failed feed
  # yielded no keys, so 0 == 0 declared a complete walk that had compared
  # nothing, no warning fired, and the weekly run went on to stamp the week a
  # success. With one stream a failure cannot hide: no header record means the
  # read failed (jq writes nothing to stdout when it errors), and a stream that
  # stops early leaves walked < expected, which the count below reports. A
  # header that is not a count is the same finding, since nothing downstream can
  # be trusted after it.
  #
  # NUL-delimited, read on fd 3. NUL because a forks key is a JSON string and
  # may hold anything a JSON string may: a key with an embedded newline split
  # into two phantom entries under a line-delimited feed, each relayed as its
  # own broken fork, while the real entry was never walked at all. fd 3 because
  # the loop body runs git, which may consume stdin.
  exec 3< <(jq --raw-output0 '.forks | (length | tostring), keys[]?' "$CUSTOM_SKILL_LOCK" 2>/dev/null)
  if ! IFS= read -r -d '' -u3 expected_entries ||
    [[ ! $expected_entries =~ ^[0-9]+$ ]]; then
    exec 3<&-
    log "fork drift-check: the forks table in $lock_name could not be read; NO fork upstream is being watched this run, fix the lock"
    relay_fork_advisory "$FORK_RELAY_STATE_LOCK_MALFORMED" "$FORK_RELAY_PROJECT_FORKS_TABLE" \
      "the forks table in $lock_name could not be read; no fork upstream was drift-checked"
    return 0
  fi
  while IFS= read -r -d '' -u3 fork; do
    walked_entries=$((walked_entries + 1))
    # Type-check the entry BEFORE reading fields out of it: a non-object entry
    # makes each of the reads below a failing command substitution, and an
    # assignment from one aborts the run under `set -euo pipefail`.
    if ! fork_entry_is_object "$CUSTOM_SKILL_LOCK" "$fork"; then
      notify_fork_entry_malformed "$fork" "it must be a JSON object, and it is not"
      continue
    fi
    unusable_fields="$(fork_entry_unusable_fields "$CUSTOM_SKILL_LOCK" "$fork")"
    if [[ -n $unusable_fields ]]; then
      notify_fork_entry_malformed "$fork" \
        "these fields must each be a non-empty JSON string with no control characters: $unusable_fields"
      continue
    fi
    source_url="$(jq -r --arg fork "$fork" '.forks[$fork].sourceUrl' "$CUSTOM_SKILL_LOCK")"
    skill_path="$(jq -r --arg fork "$fork" '.forks[$fork].skillPath' "$CUSTOM_SKILL_LOCK")"
    last_compared_tree_hash="$(jq -r --arg fork "$fork" '.forks[$fork].lastComparedTreeHash' "$CUSTOM_SKILL_LOCK")"
    if [[ $DRYRUN == "--dry-run" ]]; then
      log "would drift-check fork: $fork against $source_url"
      continue
    fi
    # A temp dir this phase cannot create is one more upstream nobody compared,
    # so it is reported like any other. Without this branch the assignment
    # fails and `set -euo pipefail` takes the run down at this line.
    clone_parent_dir="${TMPDIR:-$FORK_CLONE_FALLBACK_TMPDIR}"
    if ! clone_dir="$(mktemp -d "$clone_parent_dir/$FORK_CLONE_DIR_TEMPLATE" 2>/dev/null)"; then
      notify_fork_clone_unstageable "$fork" "$clone_parent_dir"
      continue
    fi
    clone_fork_upstream "$source_url" "$clone_dir/repo" "$clone_dir/clone-output.log"
    clone_rc=$?
    if [[ $clone_rc -eq 2 ]]; then
      discard_fork_clone "$clone_dir"
      notify_fork_clone_timeout "$fork" "$source_url" "$FORK_CLONE_DEADLINE_SECONDS"
      continue
    fi
    if [[ $clone_rc -ne 0 ]]; then
      clone_error="$(cat "$clone_dir/clone-output.log" 2>/dev/null)"
      discard_fork_clone "$clone_dir"
      notify_fork_upstream_unreachable "$fork" "$source_url" "$clone_error"
      continue
    fi
    if ! fork_clone_head_resolves "$clone_dir/repo"; then
      discard_fork_clone "$clone_dir"
      notify_fork_upstream_headless "$fork" "$source_url"
      continue
    fi
    if ! current_tree_hash="$(resolve_fork_upstream_hash "$clone_dir/repo" "$skill_path")"; then
      discard_fork_clone "$clone_dir"
      notify_fork_path_missing "$fork" "$source_url" "$skill_path"
      continue
    fi
    discard_fork_clone "$clone_dir"
    if [[ $current_tree_hash == "$last_compared_tree_hash" ]]; then
      log "fork $fork: upstream unchanged since the last comparison"
    else
      notify_fork_drift "$fork" "$source_url"
    fi
  done
  exec 3<&-
  [[ $walked_entries -eq $expected_entries ]] ||
    notify_fork_walk_incomplete "$walked_entries" "$expected_entries"
  # The phase's contract is that it always succeeds; say so rather than leaving
  # it to whatever the last statement above happened to return.
  return 0
}

# The phase BOUNDARY, and the only way this script calls the drift-watch.
# Calling check_fork_drift as the left side of an OR list is what makes
# "advisory, never fails the run" structural instead of a promise every
# statement inside has to keep one at a time: bash ignores errexit for the
# whole body of a function invoked there, so nothing inside (nor anything added
# later) can abort a weekly run that has already published a generation and has
# yet to write its success stamp. Measured on the pre-fix bytes: deleting the
# single `|| true` inside relay_fork_advisory killed the run with rc 3 at this
# phase; with this boundary the same deletion finishes the run.
#
# What the WARN is for, honestly: check_fork_drift ends in `return 0`, so with
# errexit suspended there is no path left that reaches this branch. It is here
# so a future explicit non-zero return cannot pass unnoticed, which is the one
# way this could go quiet again.
run_fork_drift_watch() {
  local watch_rc=0
  log "fork drift-check"
  check_fork_drift || watch_rc=$?
  [[ $watch_rc -eq 0 ]] ||
    log "WARN: the fork drift-watch exited $watch_rc; upstreams past that point were NOT checked this run (this phase is advisory and never fails the run)"
  return 0
}

if [[ -n $CHECK_FORKS_ONLY ]]; then
  run_fork_drift_watch
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
  run_fork_drift_watch          # its dry branch logs would-drift-check lines only
  log "done (dry-run)"
  exit 0
fi

# --install-only (brief Modes): build and publish an ADDITIVE candidate whose
# existing skills are byte-clones of the current generation plus genuinely
# absent or unhealthy roster skills repaired. Never migrates a flat store.
# Publishing swaps the whole live generation via one atomic exchange when a
# live generation exists, and by a plain rename on a fresh machine; either way
# the apply-time bootstrap runs unattended.
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
#    .skills-current generation (per-entry atomic exchange; full runs only,
#    never --install-only).
if __gen_migration_needed; then
  log "migrating the flat store to the generation layout"
  __gen_migrate || record_required_failure "flat-store migration failed"
  # Recovery ran before the generation existed; re-run it so a reusable
  # candidate or competing-writer drift is assessed against the migrated state.
  __gen_recover
fi

# Snapshot what the store holds BEFORE the exchange. The weekly record's change
# list is the difference between this and the same reading afterwards, and it has
# to be taken here because a published generation replaces the previous
# fingerprints in place.
LOG_CHANGE_DIR=""
if [[ -n $UNATTENDED_LOG_AVAILABLE ]]; then
  # GUARDED, like the success-stamp write below and for the same reason. This
  # script runs under set -e and this was a bare command substitution, so a
  # failed mktemp (an absent, unwritable or full TMPDIR) ENDED THE RUN right
  # here: after the store migration, before the generation attempt, and before
  # any record or alert existed to say so. Every remaining slot that week
  # repeats the same silent exit, so the week ends with nothing done and nothing
  # said, which from the channel is indistinguishable from a dead LaunchAgent.
  # A workspace that cannot be allocated costs the change detail, nothing else.
  #
  # The template is spelled out for the reason FORK_CLONE_DIR_TEMPLATE is: a
  # bare `mktemp -d` ignores TMPDIR on macOS (measured on 26.2 / Darwin 25.2),
  # so its location is neither redirectable nor testable, and anything this ever
  # fails to clean up should carry the name of the job that made it.
  if LOG_CHANGE_DIR="$(mktemp -d "${TMPDIR:-/tmp}/update-skills-record.XXXXXX" 2>/dev/null)" &&
    [[ -n $LOG_CHANGE_DIR ]]; then
    # Fold the snapshot dir into the roster-snapshot cleanup trap already installed
    # above (same shape: guard each name, then a trailing `true` so the trap can
    # never alter the exit status), so no exit path leaks it.
    __update_skills_cleanup() {
      [[ -n ${GEN_ROSTER_SNAPSHOT_FILE:-} ]] && rm -f "$GEN_ROSTER_SNAPSHOT_FILE"
      [[ -n ${LOG_CHANGE_DIR:-} ]] && rm -rf "$LOG_CHANGE_DIR"
      true
    }
    trap '__update_skills_cleanup' EXIT
    for __update_skills_lane in npx clawhub; do
      __update_skills_take_snapshot "$__update_skills_lane" before
    done
  else
    LOG_CHANGE_DIR=""
    LOG_NPX_SNAPSHOT_OK=""
    LOG_CLAWHUB_SNAPSHOT_OK=""
    LOG_NPX_SOURCE="$LOG_SNAPSHOT_WORKSPACE_SOURCE"
    LOG_CLAWHUB_SOURCE="$LOG_SNAPSHOT_WORKSPACE_SOURCE"
    log "WARNING: the record snapshot workspace could not be created (mktemp -d failed); the run continues and this entry will say that neither lane could be compared"
  fi
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
run_fork_drift_watch

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
LOG_STAMP_NOTE="the weekly success stamp was written"
if [[ $DRYRUN != "--dry-run" ]]; then
  if [[ $REQUIRED_FAILURES -eq 0 ]] && ! __gen_roster_unchanged; then
    # R2-2 stamp-time re-check: the roster changed AFTER the publish re-check
    # (the last window). Publishing already happened against the snapshot, so
    # live state is consistent; but stamping would mark THIS week done for a
    # roster that no longer matches, so withhold and let the next slot rebuild.
    log "WITHHOLDING the weekly success stamp: the roster lock changed after this run's snapshot; the next slot rebuilds against the new roster"
    LOG_STAMP_NOTE="the weekly success stamp was WITHHELD: the roster lock changed after this run's snapshot, so a later slot rebuilds against the new roster"
  elif [[ $REQUIRED_FAILURES -eq 0 ]]; then
    # GUARDED, both halves. This runs under set -e, and an unwritable state dir
    # made the redirection fail and ENDED THE RUN right here: after the new
    # generation was already published, before the record, before the alert, and
    # before anything said so. The publish is the one thing that had already
    # happened, which is the worst possible place to stop silently.
    if mkdir -p "$STATE_DIR" 2>/dev/null && __update_skills_stamp_value >"$SUCCESS_STAMP" 2>/dev/null; then
      # The record's own marker: the ISO week stamp above carries no wall-clock
      # time, so the gap figure in the next entry needs this. Written only for a
      # fully successful run, which is what "last successful run" has to mean.
      [[ -n $UNATTENDED_LOG_AVAILABLE ]] && unattended_log_mark_success "$LOG_SUCCESS_MARKER"
      # A verified success resets every per-skill failure streak.
      __gen_reset_failure_streaks
    else
      record_required_failure "could not write the weekly success stamp at $SUCCESS_STAMP (is $STATE_DIR writable?); the publish stands, but this week is not marked done, so every later slot repeats it"
      __update_skills_alert "update-skills published this week's generation but could NOT write its success stamp at $SUCCESS_STAMP. Check that $STATE_DIR is writable; until then every scheduled slot repeats the whole week's work."
      LOG_STAMP_NOTE="the weekly success stamp could NOT BE WRITTEN at $SUCCESS_STAMP, so every later slot repeats this week's work"
    fi
  else
    LOG_STAMP_NOTE="the weekly success stamp was WITHHELD: $REQUIRED_FAILURES required-phase failure(s) this run, so a later scheduled slot retries"
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

# The weekly RECORD for a run that reached the end. It posts whether or not
# anything changed: a run that changed nothing is precisely where the gap figure
# is the only information the entry carries, and suppressing the empty entry
# throws away the main reason the channel exists. Required-phase failures also
# reach here (they alert separately to the priority channel), so the entry states
# the count rather than implying a clean week.
#
# It is gated on the LIBRARY being available, never on the workspace: a run with
# nowhere to snapshot still has a class, a host, a timestamp, a gap, a stamp note
# and a failure count to report, and those are what a reader checks first.
if [[ -n $UNATTENDED_LOG_AVAILABLE ]]; then
  if [[ -n $LOG_CHANGE_DIR ]]; then
    for __update_skills_lane in npx clawhub; do
      __update_skills_take_snapshot "$__update_skills_lane" after
    done
  fi
  # The two lanes this record can SEE are the two store lanes it snapshots. The
  # same run also refreshes the cua-driver pack through that app's own updater
  # and updates the hermes-registry-owned skills, and it reads neither before nor
  # after. Naming two lanes while implying the whole run is the same defect as
  # implying a version number exists where none does, so the entry says what it
  # cannot see rather than leaving the reader to assume it saw everything.
  __update_skills_record completed "$(printf '%s\n%s\n%s\n%s\nrequired-phase failures: %d' \
    "$(unattended_log_change_section "$LOG_NPX_SNAPSHOT_OK" \
      "$LOG_CHANGE_DIR/npx.before" "$LOG_CHANGE_DIR/npx.after" \
      'npx-tracked skills' \
      'The change unit is the skill folder hash: this lane installs the latest commit from main with no pin and its lock records no version field, so NO VERSION NUMBER IS KNOWABLE for these skills.' \
      opaque "$LOG_NPX_SOURCE")" \
    "$(unattended_log_change_section "$LOG_CLAWHUB_SNAPSHOT_OK" \
      "$LOG_CHANGE_DIR/clawhub.before" "$LOG_CHANGE_DIR/clawhub.after" \
      'clawhub-tracked skills' \
      'This lane records an installed version, so a version number is knowable here.' \
      versions "$LOG_CLAWHUB_SOURCE")" \
    'NOT COVERED by the two lines above: the cua-driver pack (refreshed by that app own updater, through a symlink this record never reads) and the hermes-registry-owned skills (hermes owns those installs). A change to either is NOT visible here, so silence about them means nothing.' \
    "$LOG_STAMP_NOTE" \
    "$REQUIRED_FAILURES")"
fi

log "done${DRYRUN:+ (dry-run)}"
