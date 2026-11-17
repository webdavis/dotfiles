#!/usr/bin/env bash
# Succeeds when lazy.nvim's lock file holds a full commit for every plugin lazy.nvim installs from a remote.
# Otherwise it says what is wrong on stderr and fails.

set -euo pipefail

readonly name_marker='remote-plugin '

lock_parses_with_full_commits() {
  local lock=$1
  jq -e 'type == "object" and length > 0 and all(.[]; (.commit // "") | test("^[0-9a-f]{40}$"))' "$lock" >/dev/null 2>&1
}

# Each name is printed behind a marker, because Neovim shares its output with lazy.nvim's install progress.
remote_plugin_names() {
  nvim --headless \
    "+lua for _, plugin in pairs(require('lazy.core.config').plugins) do if not plugin._.is_local then io.stdout:write('\n$name_marker' .. plugin.name .. '\n') end end" \
    +qa 2>/dev/null | sed -n "s/^$name_marker//p" | sort -u
}

locked_plugin_names() {
  local lock=$1
  jq -r 'keys[]' "$lock" | sort -u
}

main() {
  local lock=$1 wanted unlocked
  if ! lock_parses_with_full_commits "$lock"; then
    printf 'the lock file is empty, unreadable, or holds something other than a full commit\n' >&2
    return 1
  fi
  wanted="$(remote_plugin_names || true)"
  if [[ -z $wanted ]]; then
    printf 'Neovim did not say which plugins it installs, so the lock file could not be checked\n' >&2
    return 1
  fi
  unlocked="$(comm -23 <(printf '%s\n' "$wanted") <(locked_plugin_names "$lock") | tr '\n' ' ')"
  if [[ -n $unlocked ]]; then
    printf 'the lock file has no line for: %s\n' "$unlocked" >&2
    return 1
  fi
}

main "$@"
