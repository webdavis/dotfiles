#!/usr/bin/env bash
# Starts a review in one of this repository's worktrees, leaving the caller where it is.

set -euo pipefail

readonly DEFAULT_REVIEW_COMMAND=(tuicr)

# Fits current branch names. A longer name only shifts its own row.
readonly BRANCH_COLUMN_WIDTH=44

# Parallel scans each write one line under PIPE_BUF, so lines stay whole for the sort.
scan_jobs() {
  printf '%s' "${WORKTREE_REVIEW_JOBS:-$(getconf _NPROCESSORS_ONLN 2>/dev/null || printf 8)}"
}

# Once the cache is older than this, the next run refreshes it in the background.
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

die() {
  printf 'worktree-review.sh: %s\n' "$1" >&2
  usage >&2
  exit 2
}

# A runtime refusal skips the usage, so a cancelled fzf leaves the pane clean.
fail() {
  printf 'worktree-review.sh: %s\n' "$1" >&2
  exit 1
}

now_epoch() {
  printf '%s' "${WORKTREE_REVIEW_NOW:-$(date +%s)}"
}

# Tries BSD stat, then GNU stat.
mtime() {
  stat -f '%m' -- "$@" 2>/dev/null || stat -c '%Y' -- "$@" 2>/dev/null || true
}

common_git_dir() {
  git rev-parse --path-format=absolute --git-common-dir ||
    die 'not inside a git repository'
}

# Named for the common git dir, so checkouts of one repository share state.
state_dir() {
  local slug="${1//\//_}" base
  base="${WORKTREE_REVIEW_STATE_DIR:-${XDG_STATE_HOME:-$HOME/.local/state}/worktree-review}"
  printf '%s/%s' "$base" "${slug#_}"
}

# Empty when no trunk exists, which makes every count zero.
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

# Compares paths, since a checkout with core.bare = true still has a working tree.
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

# A rename's origin is skipped so each change counts once.
# --work-tree lets status run on a checkout with core.bare = true.
edited_paths() {
  local worktree="$1" entry status path skip=0
  while IFS= read -r -d '' entry; do
    if ((skip)); then
      skip=0
      continue
    fi
    status="${entry:0:2}"
    path="${entry:3}"
    [[ $status == R* || $status == C* ]] && skip=1
    printf '%s\n' "$path"
  done < <(git -C "$worktree" --work-tree="$worktree" --no-optional-locks \
    status --porcelain=v1 -z --untracked-files=normal)
}

# One tab-separated row: activity, branch, ahead, edited, path.
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
    # A deleted file's parent directory carries the deletion's mtime.
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

# One rename publishes the result, so a reader never sees a torn file.
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
  awk -F'\t' -v now="$(now_epoch)" -v branchw="$BRANCH_COLUMN_WIDTH" '
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
      line = sprintf("%4s  %-" branchw "s  %s", stamp, $2, activity)
      sub(/ +$/, "", line)
      print line "\t" $5
    }
  ' "$1"
}

ensure_cache() {
  local cache="$1" common="$2"
  [[ -s $cache ]] || refresh_cache "$cache" "$common"
}

# Refreshes off the display path, so the rows on screen stay as read.
refresh_in_background() {
  local cache="$1" stamp age lock
  stamp="$(mtime "$cache")"
  [[ -n $stamp ]] || return 0
  age=$(($(now_epoch) - stamp))
  ((age > CACHE_TTL_SECONDS)) || return 0
  lock="$cache.lock"
  # mkdir is an atomic lock, so one caller scans at a time.
  # A refresh killed mid-scan leaves the lock behind; rmdir it by hand.
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
    --preview='printf %s {2}' --preview-window='bottom,1,border-none' \
    --prompt='review > ' --no-sort --no-multi --height=100%)" || return 1
  [[ -n $chosen ]] || return 1
  printf '%s' "${chosen##*$'\t'}"
}

# Remembers the chosen worktree so a later resume reuses it.
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
      chosen="$(choose "$cache")" || fail 'nothing selected'
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
        target="$(choose "$cache")" || fail 'nothing selected'
        record_selection "$target" "$selection"
      fi
      cd "$target" || fail "cannot enter $target"
      export WORKTREE_REVIEW_TARGET="$target"
      refresh_in_background "$cache"
      exec "${review[@]}"
      ;;
  esac
}

main "$@"
