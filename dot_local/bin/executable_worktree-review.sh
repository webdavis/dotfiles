#!/usr/bin/env bash
# worktree-review.sh: pick one of this repository's worktrees and start a
# review in it, without moving the caller.
#
# An agent orchestrating several lanes sits on main and never leaves it, so
# every git read here goes through `git -C <worktree>` and nothing in this
# script changes the caller's working directory or HEAD. The review itself runs
# in a child process that does chdir into the chosen checkout, which is the
# child's own environment and not the caller's.
#
# Selection is deterministic: git is the only source of truth, there is no
# agent-to-worktree registry and no language-model call.
#
# The review program is an ARGUMENT, not a hardcoded name, for the same reason
# the herdr-process plugin stays independent of the program it hosts. `reviewr`
# is not installed on this machine; `tuicr` is, so it is the default, and a
# herdr process profile that wants something else names it in its own args.
#
#   worktree-review.sh list                     the ranked table
#   worktree-review.sh pick                     choose one, print its path
#   worktree-review.sh resume [-- cmd args...]  review the chosen one
#   worktree-review.sh refresh                  recompute the activity cache
#
# `scan` is the per-worktree unit the refresh fans out over; it is not part of
# the operator-facing surface.
#
# `resume` is what a herdr process profile runs: with no target chosen it opens
# the picker first, and every later toggle reuses the recorded selection, so
# discovery happens once.

set -euo pipefail

# The post-commit hook rewrites this file in every worktree of this repository,
# so counting it would report every lane as edited and the ranking would say
# nothing. It is generated output, not somebody's edit.
readonly GENERATED_PATHS=('graphify-out/graph.json')

readonly DEFAULT_REVIEW_COMMAND=(tuicr)

# Worktrees are scanned concurrently because the cold scan is three git
# invocations per checkout and nothing else: 300 checkouts took 13.5 s in one
# process and 2.0 s across eight. Each scan writes exactly one short line, well
# under PIPE_BUF, so the interleaved writes stay whole lines and the sort that
# follows puts them back in order.
scan_jobs() {
  printf '%s' "${WORKTREE_REVIEW_JOBS:-$(getconf _NPROCESSORS_ONLN 2>/dev/null || printf 8)}"
}

# Seconds a displayed generation may be stale before the next invocation
# recomputes one in the background. The display itself always reads a whole
# generation, so a refresh never reorders the list under the cursor.
readonly CACHE_TTL_SECONDS="${WORKTREE_REVIEW_CACHE_TTL:-60}"

usage() {
  cat <<'USAGE'
usage: worktree-review.sh <command> [-- review-command [args...]]

  list      print this repository's worktrees, most recently updated first
  pick      choose a worktree and print its path
  resume    review the chosen worktree, choosing one first if none is set
  refresh   recompute the activity ordering

The review command defaults to tuicr and runs with the chosen worktree as its
working directory.
USAGE
}

# Every refusal here is an argument error, so each one names the problem and
# prints the usage next to it, on stderr, and never falls through to help with
# a success status.
die() {
  printf 'worktree-review.sh: %s\n' "$1" >&2
  usage >&2
  exit 2
}

now_epoch() {
  printf '%s' "${WORKTREE_REVIEW_NOW:-$(date +%s)}"
}

# mtime <path...> : one line per path, portable across BSD and GNU stat.
mtime() {
  stat -f '%m' -- "$@" 2>/dev/null || stat -c '%Y' -- "$@" 2>/dev/null || true
}

common_git_dir() {
  git rev-parse --path-format=absolute --git-common-dir ||
    die 'not inside a git repository'
}

# One state directory per repository, named after its common git directory so
# two checkouts of the same repository share one generation and one selection.
state_dir() {
  local slug="${1//\//_}" base
  base="${WORKTREE_REVIEW_STATE_DIR:-${XDG_STATE_HOME:-$HOME/.local/state}/worktree-review}"
  printf '%s/%s' "$base" "${slug#_}"
}

# base_ref : the ref a lane's own commits are counted against. Empty when the
# repository has none of the usual trunk names, which makes every count zero
# rather than failing the listing.
base_ref() {
  local candidate
  for candidate in refs/remotes/origin/HEAD refs/remotes/origin/main \
    refs/heads/main refs/heads/master; do
    if git rev-parse --verify --quiet "$candidate" >/dev/null; then
      printf '%s' "$candidate"
      return 0
    fi
  done
  printf ''
}

# worktree_paths <common git dir> : every checkout that is actually on disk. A
# registration whose directory is gone is dropped, so a checkout somebody
# deleted cannot occupy a row, and so is a bare repository's own entry, whose
# path IS the git directory and which has no working tree to review.
#
# The `bare` marker cannot be that filter. A checkout can carry core.bare =
# true and still have a full working tree, which git then reports as bare: the
# dashboard spec's environment leak wrote exactly that into this repository on
# 2026-09-15, and the marker would have dropped the operator's primary checkout,
# the one row they orchestrate from. The git directory is the honest test and
# stays right after the config is repaired.
worktree_paths() {
  local common="$1" key value path=''
  while IFS=' ' read -r key value; do
    case "$key" in
      worktree) path="$value" ;;
      '')
        emit_worktree "$path" "$common"
        path=''
        ;;
    esac
  done < <(git worktree list --porcelain)
  emit_worktree "$path" "$common"
  return 0
}

emit_worktree() {
  [[ -n $1 && -d $1 && $1 != "$2" ]] && printf '%s\n' "$1"
  return 0
}

# edited_paths <worktree> : one line per non-ignored path the worktree has
# changed, generated output excluded. A rename's origin path is dropped so one
# change counts once.
#
# --work-tree is named explicitly for the same reason: on a checkout carrying
# core.bare = true a plain `git status` refuses with "must be run in a work
# tree", which would report every edit in that checkout as none.
edited_paths() {
  local worktree="$1" entry status path generated skip=0
  while IFS= read -r -d '' entry; do
    if ((skip)); then
      skip=0
      continue
    fi
    status="${entry:0:2}"
    path="${entry:3}"
    [[ $status == R* || $status == C* ]] && skip=1
    for generated in "${GENERATED_PATHS[@]}"; do
      [[ $path == "$generated" ]] && continue 2
    done
    printf '%s\n' "$path"
  done < <(git -C "$worktree" --work-tree="$worktree" --no-optional-locks \
    status --porcelain=v1 -z --untracked-files=normal)
}

# record <worktree> <base ref> : one cache row,
# activity<TAB>branch<TAB>ahead<TAB>edited<TAB>path
record() {
  local worktree="$1" base="$2" branch head_time ahead=0 activity
  local -a edited=() stamped=()
  branch="$(git -C "$worktree" symbolic-ref --quiet --short HEAD 2>/dev/null)" ||
    branch="$(git -C "$worktree" rev-parse --short HEAD 2>/dev/null)" ||
    branch='(no commits)'
  head_time="$(git -C "$worktree" log -1 --format=%ct 2>/dev/null)" || head_time=0
  [[ -n $head_time ]] || head_time=0
  if [[ -n $base ]]; then
    ahead="$(git -C "$worktree" rev-list --count "$base..HEAD" 2>/dev/null)" || ahead=0
  fi

  local path full
  while IFS= read -r path; do
    edited+=("$path")
    full="$worktree/$path"
    # A deleted file has no mtime of its own; its parent directory is what the
    # unlink stamped, so that is the honest timestamp for the deletion.
    if [[ -e $full ]]; then
      stamped+=("$full")
    else
      stamped+=("${full%/*}")
    fi
  done < <(edited_paths "$worktree")

  activity="$head_time"
  if ((${#stamped[@]})); then
    local stamp
    while IFS= read -r stamp; do
      [[ $stamp =~ ^[0-9]+$ ]] || continue
      ((stamp > activity)) && activity="$stamp"
    done < <(mtime "${stamped[@]}")
  fi
  printf '%s\t%s\t%s\t%s\t%s\n' \
    "$activity" "$branch" "$ahead" "${#edited[@]}" "$worktree"
}

# refresh_cache : recompute every row and publish the result with one rename,
# so a concurrent display reads one whole generation and never a torn file.
refresh_cache() {
  local cache="$1" common="$2" base temporary
  base="$(base_ref)"
  temporary="$(mktemp "${cache}.XXXXXX")"
  worktree_paths "$common" |
    xargs -P "$(scan_jobs)" -I '{}' "$0" scan '{}' "$base" |
    sort -rn -k1,1 >"$temporary"
  mv -f "$temporary" "$cache"
}

render() {
  awk -F'\t' -v now="$(now_epoch)" '
    {
      age = now - $1
      if (age < 0) age = 0
      if (age < 60) stamp = "now"
      else if (age < 3600) stamp = int(age / 60) "m"
      else if (age < 86400) stamp = int(age / 3600) "h"
      else if (age < 604800) stamp = int(age / 86400) "d"
      else stamp = int(age / 604800) "w"
      activity = ""
      if ($3 + 0 > 0) activity = sprintf("%s ahead", $3)
      if ($4 + 0 > 0) activity = activity (activity == "" ? "" : "  ") sprintf("%s edited", $4)
      line = sprintf("%4s  %-44s  %s", stamp, $2, activity)
      sub(/ +$/, "", line)
      print line "\t" $5
    }
  ' "$1"
}

# ensure_cache : a first load has to compute; every later load reads the file.
ensure_cache() {
  local cache="$1" common="$2"
  [[ -s $cache ]] || refresh_cache "$cache" "$common"
}

# refresh_in_background : ordering is recomputed off the display path, so the
# rows on screen belong to the generation the caller already read.
refresh_in_background() {
  local cache="$1" stamp age lock
  stamp="$(mtime "$cache")"
  [[ -n $stamp ]] || return 0
  age=$(($(now_epoch) - stamp))
  ((age > CACHE_TTL_SECONDS)) || return 0
  lock="$cache.lock"
  # mkdir is the atomic test-and-set: a second caller inside the same scan
  # window finds it already there and skips its own scan.
  # ponytail: no stale-lock timeout, a refresh killed mid-scan leaves the
  # lock behind; `rmdir` it by hand if that ever happens.
  mkdir "$lock" 2>/dev/null || return 0
  (
    "$0" refresh >/dev/null 2>&1
    rmdir "$lock" 2>/dev/null
  ) &
  return 0
}

choose() {
  local cache="$1" picker chosen
  picker="${WORKTREE_REVIEW_PICKER:-fzf}"
  chosen="$(render "$cache" | "$picker" \
    --delimiter=$'\t' --with-nth=1 \
    --prompt='review > ' --no-sort --no-multi --height=100%)" || return 1
  [[ -n $chosen ]] || return 1
  printf '%s' "${chosen##*$'\t'}"
}

# record_selection remembers the chosen worktree so a later resume can reuse
# it without walking the list again.
#
# It does not record the originating workspace or pane. The wired path
# (`resume`, run by the herdr-process plugin as `spec.program`) never has
# HERDR_WORKSPACE_ID or HERDR_PANE_ID in its environment: the plugin strips
# every HERDR_-prefixed variable before spawning its child (herdr-process
# startup.rs `private_variable`), and it spawns the process once per profile
# rather than once per toggle, so even an unstripped value would belong to
# whichever pane happened to trigger the first spawn, not whoever is looking
# now. The plugin already tracks the live requesting target itself
# (`Session::context_for`); forwarding it to the child needs a channel the
# plugin updates on every toggle, not a spawn-time environment variable, so
# it stays out of this script.
record_selection() {
  printf '%s\n' "$1" >"$2"
}

main() {
  local command="${1:-}"
  [[ -n $command ]] || {
    usage >&2
    exit 2
  }
  shift
  case "$command" in
    --help | -h | help)
      [[ $# -eq 0 ]] || die 'help takes no arguments; see --help'
      usage
      return 0
      ;;
    scan)
      [[ $# -ge 1 && $# -le 2 ]] || die 'scan takes a worktree path and an optional base ref'
      record "$1" "${2:-}"
      return 0
      ;;
    list | pick | refresh)
      [[ $# -eq 0 ]] || die "$command takes no arguments; see --help"
      ;;
    resume)
      [[ ${1:-'--'} == '--' ]] || die 'resume takes the review command after --; see --help'
      [[ $# -eq 0 ]] || shift
      ;;
    *) die "unknown command $command; see --help" ;;
  esac

  local common state cache selection
  common="$(common_git_dir)"
  state="$(state_dir "$common")"
  mkdir -p "$state"
  cache="$state/activity.tsv"
  selection="$state/selection"

  case "$command" in
    refresh)
      refresh_cache "$cache" "$common"
      ;;
    list)
      ensure_cache "$cache" "$common"
      render "$cache"
      refresh_in_background "$cache"
      ;;
    pick)
      ensure_cache "$cache" "$common"
      local chosen
      chosen="$(choose "$cache")" || die 'nothing selected'
      record_selection "$chosen" "$selection"
      printf '%s\n' "$chosen"
      refresh_in_background "$cache"
      ;;
    resume)
      local -a review=("${DEFAULT_REVIEW_COMMAND[@]}")
      (($#)) && review=("$@")
      local target=''
      if [[ -s $selection ]]; then
        target="$(<"$selection")"
        [[ -d $target ]] || target=''
      fi
      if [[ -z $target ]]; then
        ensure_cache "$cache" "$common"
        target="$(choose "$cache")" || die 'nothing selected'
        record_selection "$target" "$selection"
      fi
      cd "$target" || die "cannot enter $target"
      export WORKTREE_REVIEW_TARGET="$target"
      refresh_in_background "$cache"
      exec "${review[@]}"
      ;;
  esac
}

main "$@"
