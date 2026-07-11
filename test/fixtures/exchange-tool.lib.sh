# shellcheck shell=bash
# exchange-tool.lib.sh: shared test helper that resolves the generation-exchange
# tool the same way the updater does at run time. The host userland differs by
# environment: a macOS host carries GNU mv as Homebrew's gmv (plain mv is BSD),
# while the Nix devshell (what CI runs) provides GNU coreutils mv as plain mv
# and has no /opt/homebrew. A candidate is accepted only when --version says
# GNU coreutils AND a functional probe performs a real --exchange --no-copy -T
# swap in a private temp dir. Prints the accepted tool name; returns 1 when no
# capable tool exists on PATH.
resolve_exchange_tool() {
  local candidate probe
  for candidate in ${UPDATE_SKILLS_GMV:+"$UPDATE_SKILLS_GMV"} gmv mv; do
    command -v "$candidate" >/dev/null 2>&1 || continue
    "$candidate" --version 2>/dev/null | head -1 | grep -q 'GNU coreutils' || continue
    probe="$(mktemp -d)" || return 1
    mkdir -p "$probe/a" "$probe/b"
    printf 'a' >"$probe/a/marker"
    printf 'b' >"$probe/b/marker"
    if "$candidate" --exchange --no-copy -T "$probe/a" "$probe/b" 2>/dev/null &&
      [[ "$(cat "$probe/a/marker" 2>/dev/null)" == "b" ]]; then
      rm -rf "$probe"
      printf '%s\n' "$candidate"
      return 0
    fi
    rm -rf "$probe"
  done
  return 1
}
