#!/usr/bin/env bash
# Git merge driver for lazy.nvim's lock file. It merges plugin by plugin, so two sides that changed different plugins never conflict.
# When both sides changed the same plugin, a side that only repeats what this checkout already published gives way,
# and otherwise the incoming side wins; under a rebasing pull that side is the local change.

set -euo pipefail

is_usable_lock() {
  local lock=$1
  jq -e 'type == "object" and length > 0' "$lock" >/dev/null 2>&1
}

# The lock this checkout last published, or nothing when it has published none.
published_lock() {
  local marker blob
  marker="$(git rev-parse --git-dir 2>/dev/null)/nvim-lock-published" || return 0
  [[ -f $marker ]] || return 0
  blob="$(<"$marker")"
  git cat-file -p "$blob" 2>/dev/null || true
}

merged_lock() {
  local base=$1 current=$2 incoming=$3 published=$4
  jq -rn --slurpfile base "$base" --slurpfile current "$current" --slurpfile incoming "$incoming" \
    --slurpfile published "$published" '
    $base[0] as $base | $current[0] as $current | $incoming[0] as $incoming | ($published[0] // null) as $published
    | reduce (($base + $current + $incoming) | keys[]) as $plugin ({};
        $base[$plugin] as $was | $current[$plugin] as $theirs | $incoming[$plugin] as $ours
        | (if $ours == $was then $theirs
           elif $theirs == $was then $ours
           elif $published != null and $ours == $published[$plugin] then $theirs
           else $ours end) as $entry
        | if $entry == null then . else .[$plugin] = $entry end)
    | to_entries | sort_by(.key)
    | map("  \(.key | @json): { \"branch\": \(.value.branch | @json), \"commit\": \(.value.commit | @json) }")
    | "{\n" + join(",\n") + "\n}"'
}

main() {
  local base=$1 current=$2 incoming=$3 merged published
  if ! is_usable_lock "$incoming"; then
    return 0
  fi
  if ! is_usable_lock "$current"; then
    cat "$incoming" >"$current"
    return 0
  fi
  published="$(published_lock)"
  if ! jq -e 'type == "object"' <<<"$published" >/dev/null 2>&1; then
    published=null
  fi
  if is_usable_lock "$base"; then
    merged="$(merged_lock "$base" "$current" "$incoming" <(printf '%s' "$published"))"
  else
    merged="$(merged_lock <(printf '{}') "$current" "$incoming" <(printf '%s' "$published"))"
  fi
  printf '%s\n' "$merged" >"$current"
}

main "$@"
