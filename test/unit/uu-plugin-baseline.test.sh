#!/usr/bin/env bash
set -euo pipefail

function test_plugin_baseline_seed_is_nonfatal_and_calls_only_uu_bootstrap() {
  local repo scratch mode seed_output seed_status
  repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  scratch="$(mktemp -d "${TMPDIR:-/tmp}/uu-seed-test.XXXXXX")"
  mkdir -p "$scratch/h/.cargo/bin" "$scratch/bin" "$scratch/c" "$scratch/d" "$scratch/s" "$scratch/k" "$scratch/t" "$scratch/claude"
  cat >"$scratch/bin/launchctl" <<'STUB'
#!/bin/bash
printf called >>"$HOME/launchctl-calls"
STUB
  chmod +x "$scratch/bin/launchctl"
  env HOME="$scratch/h" XDG_CONFIG_HOME="$scratch/c" XDG_DATA_HOME="$scratch/d" XDG_STATE_HOME="$scratch/s" XDG_CACHE_HOME="$scratch/k" TMPDIR="$scratch/t" CLAUDE_CONFIG_DIR="$scratch/claude" CI=1 \
    chezmoi --source "$repo" execute-template --no-tty <"$repo/.chezmoiscripts/run_onchange_after_69-seed-claude-plugins-baseline.sh.tmpl" >"$scratch/seed.sh"
  for mode in 0 1 75; do
    cat >"$scratch/h/.cargo/bin/uu" <<'STUB'
#!/bin/bash
printf '%s\n' "$@" >"$HOME/uu-args"
STUB
    printf 'exit %s\n' "$mode" >>"$scratch/h/.cargo/bin/uu"
    chmod +x "$scratch/h/.cargo/bin/uu"
    seed_status=0
    seed_output="$(env HOME="$scratch/h" PATH="$scratch/bin:$PATH" XDG_CONFIG_HOME="$scratch/c" XDG_DATA_HOME="$scratch/d" XDG_STATE_HOME="$scratch/s" XDG_CACHE_HOME="$scratch/k" TMPDIR="$scratch/t" CLAUDE_CONFIG_DIR="$scratch/claude" GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null bash "$scratch/seed.sh" 2>&1)" || seed_status=$?
    assert_same 0 "$seed_status"
    assert_file_exists "$scratch/h/uu-args"
    if [[ -f "$scratch/h/uu-args" ]]; then
      assert_same $'bootstrap\nclaude-plugins' "$(cat "$scratch/h/uu-args")"
    fi
    assert_file_not_exists "$scratch/h/launchctl-calls"
    if [[ $mode != 0 ]]; then assert_contains 'could not be seeded' "$seed_output"; fi
  done
}
