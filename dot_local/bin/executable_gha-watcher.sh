#!/usr/bin/env bash
# gha-watcher — background daemon that watches GitHub Actions completions
# across all repos owned by the authenticated user, and fires alerter +
# hue-pulse on success/failure.
#
# Invoked periodically by launchd (com.webdavis.gha-watcher). Each invocation:
#   1. Lists owner's repos pushed within the last 30 days.
#   2. For each, polls the most recent completed runs via gh api.
#   3. Filters to runs by the authenticated user with conclusion success|failure.
#   4. For runs newer than the last-seen ID per repo, fires notifications.
#   5. Persists the new last-seen IDs to ~/.cache/gha-watcher/state.json.
#
# First-run behavior: the state file is seeded with current run IDs WITHOUT
# firing notifications, so installation doesn't spam historical completions.

set -euo pipefail

state_dir="${XDG_CACHE_HOME:-$HOME/.cache}/gha-watcher"
state_file="$state_dir/state.json"
log_file="$HOME/.local/log/gha-watcher.log"
mkdir -p "$state_dir" "$(dirname "$log_file")"

log() { printf '[%s] %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*" >>"$log_file"; }

# --- pre-flight ---------------------------------------------------------------

command -v gh &>/dev/null || {
  log "gh not installed; exit"
  exit 0
}
command -v jq &>/dev/null || {
  log "jq not installed; exit"
  exit 0
}

if ! gh auth status &>/dev/null; then
  log "gh not authenticated; exit"
  exit 0
fi

# Authenticated user's login.
me=$(timeout 10 gh api /user --jq .login 2>/dev/null || true)
[[ -z $me ]] && {
  log "could not resolve gh user; exit"
  exit 0
}

# --- repo discovery -----------------------------------------------------------

# All repos this user OWNS, only those pushed in last 30 days. Capped at 1000.
# Use ISO-8601 cutoff so jq filter is straightforward.
cutoff=$(date -u -v-30d +%Y-%m-%dT%H:%M:%SZ 2>/dev/null ||
  date -u -d '30 days ago' +%Y-%m-%dT%H:%M:%SZ)

mapfile -t repos < <(
  timeout 30 gh repo list "$me" \
    --json nameWithOwner,pushedAt -L 1000 2>/dev/null |
    jq -r --arg cutoff "$cutoff" \
      '.[] | select(.pushedAt > $cutoff) | .nameWithOwner' || true
)

if ((${#repos[@]} == 0)); then
  log "no recently-active repos; exit"
  exit 0
fi

# --- state load ---------------------------------------------------------------

first_run=false
if [[ ! -f $state_file ]]; then
  first_run=true
  echo '{"last_seen":{}}' >"$state_file"
  log "first run — seeding state without firing notifications"
fi

# --- per-repo poll ------------------------------------------------------------

# Collect updates as a single jq merge at the end (atomic write).
declare -A new_last_seen=()

notify() {
  # Args: repo, workflow_name, conclusion (success|failure), run_url
  local repo=$1 wf=$2 conc=$3 url=$4
  local exit_code=0
  [[ $conc == "failure" ]] && exit_code=1
  log "fire: $repo / $wf → $conc"
  # Detach via subshell so the daemon doesn't wait for hue-pulse (~5s) or
  # alerter to complete; both become orphaned to launchd, invisible to us.
  (
    alerter --timeout 30 --title "GitHub Actions: $repo" \
      --message "$wf — $conc" --open "$url" \
      --sound default >/dev/null 2>&1 &
  )
  (
    "$HOME/.local/bin/hue-pulse.sh" "$exit_code" >/dev/null 2>&1 &
  )
}

for repo in "${repos[@]}"; do
  # Most recent 20 runs in this repo, filtered to: completed,
  # initiated by $me, conclusion success or failure.
  runs_json=$(
    timeout 20 gh api \
      "/repos/$repo/actions/runs?per_page=20" \
      --jq "
        [.workflow_runs[]
         | select(.status == \"completed\")
         | select(.actor.login == \"$me\")
         | select(.conclusion == \"success\" or .conclusion == \"failure\")
         | {id, name, conclusion, html_url}]
      " 2>/dev/null || true
  )
  [[ -z $runs_json || $runs_json == "null" ]] && continue

  # Read the prior last-seen ID for this repo (default 0 if absent).
  last_id=$(jq -r --arg r "$repo" '.last_seen[$r] // 0' "$state_file")
  # Newest ID we've now seen.
  newest_id=$(printf '%s' "$runs_json" | jq -r '[.[].id] | max // 0')
  new_last_seen[$repo]=$newest_id

  # Don't fire on first run — just seed.
  $first_run && continue

  # Fire for each run with id > last_id, ordered oldest-first so multi-fire
  # notifications arrive in chronological order.
  printf '%s' "$runs_json" |
    jq -r --argjson last "$last_id" \
      '.[] | select(.id > $last) | "\(.id)\t\(.name)\t\(.conclusion)\t\(.html_url)"' |
    sort -k1,1n |
    while IFS=$'\t' read -r _id wf conc url; do
      notify "$repo" "$wf" "$conc" "$url"
    done
done

# --- state save (atomic write) ------------------------------------------------

# Build the state JSON from scratch each run, since we re-discover all active
# repos every time. State file is a flat map of repo → last_id under a single
# "last_seen" object. Repo names are JSON-quoted via jq's @json filter so any
# character (slashes, dots) is safely escaped.
tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT

{
  printf '{"last_seen":{'
  first=true
  for repo in "${!new_last_seen[@]}"; do
    $first || printf ','
    first=false
    printf '%s:%s' \
      "$(printf '%s' "$repo" | jq -Rs .)" \
      "${new_last_seen[$repo]}"
  done
  printf '}}'
} | jq . >"$tmp"

mv "$tmp" "$state_file"
trap - EXIT

log "poll complete; tracked ${#new_last_seen[@]} repos"
