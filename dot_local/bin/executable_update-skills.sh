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
#   --dry-run           log what would change, change nothing
#   --install-only      run only the npx + clawhub install passes for absent
#                       skills + symlink fan-out + Codex overlay re-assert +
#                       superpowers routing re-assert (used by tests and
#                       fresh-machine bootstrap; skips the weekly npx and
#                       clawhub updates, the hermes registry-update phase, and
#                       the fork drift-check)
#   --check-forks-only  run only the fork/vendored upstream drift-check
# Env: UPDATE_SKILLS_FORCE=1 bypasses the idle-gate AND the weekly success stamp.
#      The idle-gate makes this script refuse to swap skill folders while an
#      INTERACTIVE harness session (claude/codex/hermes) is running, so a skill is
#      never yanked out from under a live session — persistent background daemons
#      (the hermes gateway, Claude Code's bg helpers, the Codex app-server) do NOT
#      defer it (see __update_skills_harness_active). The weekly run is scheduled
#      across four Monday slots; a per-week success stamp
#      (~/.local/state/update-skills/last-success) makes the extra slots no-ops
#      after one succeeds, and the last slot alerts if it still cannot run. FORCE=1
#      accepts the swap risk for test runs and deliberate manual runs.
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
SUCCESS_STAMP="$STATE_DIR/last-success" # ISO year-week (%G-%V) of the last fully successful weekly run
# The plist fires four Monday retry slots — 04:00/08:00/12:00/16:00 (see
# Library/LaunchAgents/com.webdavis.update-skills.plist.tmpl). This is the hour
# of the LAST slot: a deferral here means the weekly retry budget is exhausted,
# so the run alerts LOUDLY instead of failing silent. Keep in sync with the plist.
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
for arg in "$@"; do
  case "$arg" in
    --dry-run) DRYRUN="--dry-run" ;;
    --install-only) INSTALL_ONLY=1 ;;
    --check-forks-only) CHECK_FORKS_ONLY=1 ;;
    *)
      printf 'update-skills: unknown argument: %s\n' "$arg" >&2
      exit 2
      ;;
  esac
done

log() { printf '[update-skills] %s\n' "$*"; }

# idle-gate discriminator: is an INTERACTIVE agent-harness session using the
# store right now? Returns 0 (defer — never swap a skill out from under a live
# session) for an interactive Claude Code / Codex / hermes session, 1 (proceed)
# when only persistent background daemons are up.
#
# The old gate was `pgrep -x claude || -x codex || -x hermes`, which deferred on
# ANY match, INCLUDING persistent daemons — so the weekly run could defer forever
# (the audit logged two such deferrals). We instead read full argv (ps -xo args=)
# and key on the EXECUTABLE (argv[0] basename), excluding the daemon shapes.
#
# Ground truth (dresden, read-only ps, 2026-07-10):
#   DAEMONS (proceed) — none is an interactive session:
#     python -m hermes_cli.main gateway run --replace   (hermes gateway: argv[0]
#         is `python`, so it is ignored outright — and note `pgrep -x hermes`
#         never even matched it, comm=python; the OLD gate's real defer-forever
#         driver on THIS machine was long-lived `claude --remote-control` bridges)
#     .../claude --bg-spare | daemon run | --bg-pty-host   (Claude bg helpers)
#     codex app-server [--analytics-default-enabled]       (Codex app server)
#   INTERACTIVE (defer):
#     /opt/homebrew/bin/claude [--remote-control|resume|-p ...]  (a Claude TUI)
#     codex [resume <id>]                                        (a Codex CLI session)
#     .../hermes -c <prompt> | hermes -p <profile>               (an interactive hermes run)
#
# Keying on argv[0]'s basename means prompt text or paths that merely CONTAIN
# "codex"/"claude" (a `node .../codex-companion.mjs` helper, a bash -c carrying a
# prompt about codex) never false-positive — argv[0] there is `node`/`bash`.
__update_skills_is_interactive_harness() {
  local args="$1" cmd base
  cmd="${args%% *}" # argv[0] (the executable)
  base="${cmd##*/}" # its basename
  case "$base" in
    claude | codex | hermes) ;; # a candidate harness binary
    *) return 1 ;;              # anything else (python gateway, node, bash, GUI helpers) is not
  esac
  # Exclude the daemon shapes by the rest of argv.
  case "$args" in
    *hermes_cli.main*gateway* | *'hermes gateway'*) return 1 ;; # hermes gateway
    *'claude --bg-'* | *'claude daemon run'*) return 1 ;;       # Claude bg spare/daemon/pty-host
    *'codex app-server'*) return 1 ;;                           # Codex app server
  esac
  return 0
}

__update_skills_harness_active() {
  local args
  while IFS= read -r args; do
    [[ -n $args ]] || continue
    __update_skills_is_interactive_harness "$args" && return 0
  done < <(ps -xo args= 2>/dev/null)
  return 1
}

# updater-owned link = a SYMLINK whose literal target points under
# ~/.agents/skills. The literal readlink target is matched (not a resolved path),
# so this holds for a DANGLING link too — its string still points into the store.
# ONLY such links are ever replaced or removed by convergence; real dirs
# (hub-owned registry dirs, hermes's catalog) and non-store symlinks are never
# touched, and neither is any name in a profile the lock does not map.
__update_skills_is_owned_link() {
  local path="$1" target
  [[ -L $path ]] || return 1
  target="$(readlink "$path" 2>/dev/null || true)"
  case "$target" in
    *.agents/skills/*) return 0 ;;
  esac
  return 1
}

# Converge one managed dir to a desired {name -> "$prefix/$name"} set:
#   converge_dir <dir> <target_prefix> <desired_name>...
# create a missing desired link; REPLACE an updater-owned link whose target
# differs (wrong-target, incl. dangling — the additive `[[ -e ]] || ln -s`
# crashed on a dangling link); REMOVE an updater-owned link no longer desired
# (stale). A real dir/file (hub-owned/catalog) at a managed name, and any
# non-store symlink, are left untouched. A no-op convergence is silent.
converge_dir() {
  local dir="$1" prefix="$2"
  shift 2
  local -a desired=("$@")
  local skill target path current name is_desired old_target
  mkdir -p "$dir"
  # 1) create or repair every desired link
  if [[ ${#desired[@]} -gt 0 ]]; then
    for skill in "${desired[@]}"; do
      target="$prefix/$skill"
      path="$dir/$skill"
      if [[ -L $path ]]; then
        current="$(readlink "$path" 2>/dev/null || true)"
        [[ $current == "$target" ]] && continue # already correct
        if __update_skills_is_owned_link "$path"; then
          ln -sfn "$target" "$path" # replace wrong-target / dangling updater-owned link
          log "converge: replaced $path (was $current, now $target)"
        else
          log "converge: WARN $path is a non-store symlink at a managed name; leaving it (resolve by hand)"
        fi
      elif [[ -e $path ]]; then
        : # a real dir/file (hub-owned or catalog) at this name — never overwrite
      else
        ln -s "$target" "$path"
        log "converge: created $path -> $target"
      fi
    done
  fi
  # 2) remove updater-owned links that are no longer desired (stale drift)
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
    if __update_skills_is_owned_link "$path"; then
      old_target="$(readlink "$path" 2>/dev/null || true)"
      rm -f "$path"
      log "converge: removed stale $path (was $old_target)"
    fi
  done
}

# Claude fan-out: every store skill (the full roster) gets a ~/.claude/skills
# link. Claude is not profile-scoped — tiering there is the settings
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
# this machine). A [] mapping — or a missing table — gets no hermes link: the
# deliberate "not available in hermes from the store" state, not an error.
# Collision-named skills (humanizer, hyperframes) never fan out: hermes's catalog
# wins those names (operator ruling), so a stale store link at such a name IS
# removed by convergence, but creating one never happens. Only the profiles the
# lock names are walked — a profile the lock does not map is never touched.
HERMES_COLLISION_NAMES=(humanizer hyperframes)
is_hermes_collision_name() {
  local collision_entry
  for collision_entry in "${HERMES_COLLISION_NAMES[@]}"; do
    [[ $collision_entry == "$1" ]] && return 0
  done
  return 1
}
converge_hermes_skills() {
  [[ -f $CUSTOM_SKILL_LOCK ]] || return 0
  local profile link_dir prefix skill
  local -a profiles=() desired=()
  while IFS= read -r profile; do
    [[ -n $profile ]] && profiles+=("$profile")
  done < <(jq -r '.hermesProfiles // {} | [.[][]?] | unique | .[]' "$CUSTOM_SKILL_LOCK" 2>/dev/null)
  [[ ${#profiles[@]} -gt 0 ]] || return 0
  for profile in "${profiles[@]}"; do
    if [[ $profile == "default" ]]; then
      link_dir="$HERMES"
      prefix="../../.agents/skills"
    else
      link_dir="$HERMES_PROFILES/$profile/skills"
      prefix="../../../../.agents/skills"
    fi
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
  [[ -x $routing_script ]] || return 0
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
    mkdir -p "$STORE/$skill/agents"
    if [[ -f $overlay_file ]]; then
      printf '\n%s\n' "$policy" >>"$overlay_file"
      log "appended codex overlay policy to upstream openai.yaml: $skill"
    else
      printf '%s\n' "$policy" >"$overlay_file"
      log "asserted codex overlay: $skill"
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
  if ! command -v hermes >/dev/null 2>&1; then
    log "hermes not on PATH; skipping the hermes registry-update phase"
    return 0
  fi
  local relay_script="$HOME/.local/bin/relay.sh"
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

mkdir -p "$STORE" "$CLAUDE" "$HERMES"

# serialize: one run at a time (mkdir is atomic + system-shipped; flock is absent on macOS)
if [[ -d $LOCKDIR ]] && find "$LOCKDIR" -prune -mmin +120 2>/dev/null | grep -q .; then rm -rf "$LOCKDIR"; fi # steal stale lock (>2h: crashed run)
if ! mkdir "$LOCKDIR" 2>/dev/null; then
  log "another run in progress; exiting"
  exit 0
fi
trap 'rm -rf "$LOCKDIR"' EXIT

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

# idle-gate: defer while an INTERACTIVE harness session is using the store, so a
# skill is never swapped out from under a live session. Persistent background
# daemons (the hermes gateway, Claude Code's bg helpers, the Codex app-server) do
# NOT count — blocking on them is what deferred the weekly run forever (see
# __update_skills_harness_active). UPDATE_SKILLS_FORCE=1 bypasses (tests, manual
# runs). On the LAST Monday slot the retry budget is spent, so a deferral there
# alerts LOUDLY rather than failing silent.
if [[ ${UPDATE_SKILLS_FORCE:-} != "1" ]] && [[ $DRYRUN != "--dry-run" ]] && __update_skills_harness_active; then
  log "an interactive harness session (claude/codex/hermes) is using the store; deferring this run"
  if [[ "$(date +%H)" == "$UPDATE_SKILLS_LAST_SLOT_HOUR" ]]; then
    log "EXHAUSTED: the last Monday retry slot (${UPDATE_SKILLS_LAST_SLOT_HOUR}:00) still deferred — the weekly skills update did not run this week"
    if command -v alerter >/dev/null 2>&1; then
      alerter --timeout 30 --title "update-skills" \
        --message "Weekly skills update deferred every Monday slot — an agent session was always active. Run it by hand when idle (~/.local/bin/update-skills.sh)." \
        --sound default >/dev/null 2>&1 || true
    fi
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
      fi
    else
      log "install $skill: npx add failed ($repo) (continuing)"
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
  if ! command -v clawhub >/dev/null 2>&1; then
    log "clawhub not on PATH; skipping the clawhub install pass"
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
      fi
    else
      log "install $skill: clawhub install failed ($slug) (continuing)"
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
  if ! command -v clawhub >/dev/null 2>&1; then
    log "clawhub not on PATH; skipping the clawhub update pass"
    return 0
  fi
  local relay_script="$HOME/.local/bin/relay.sh"
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
  exit 0
fi

# 1) refresh every npx-tracked store skill in place from its upstream
if [[ $DRYRUN == "--dry-run" ]]; then
  log "would run: npx skills update --global"
else
  log "npx skills update --global"
  npx --yes skills@latest update --global -y 2>&1 | tr -d '\r' | tail -3 || log "npx update reported issues (continuing)"
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
#    exactly the hermes profile skills dirs its hermesProfiles mapping names —
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

# record this week's success so the remaining Monday slots no-op — the run used
# to defer forever with no stamp and no bounded retry budget (audit Fix 1)
if [[ $DRYRUN != "--dry-run" ]]; then
  mkdir -p "$STATE_DIR"
  date +%G-%V >"$SUCCESS_STAMP"
fi

log "done${DRYRUN:+ (dry-run)}"
