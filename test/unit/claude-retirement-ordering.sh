#!/usr/bin/env bash
# claude-retirement-ordering.sh: the com.claude.code LaunchAgent retirement
# chezmoiscript must run BEFORE run_after_58-herdr-migration-verify, because
# after_58 removes tmux/sesh (via brew bundle cleanup) and the com.claude.code
# supervisor is tmux-coupled (its plist execs the tmux-driven claude-restart.sh).
# Retiring the supervisor only AFTER its multiplexer is gone is the wrong order.
#
# Chezmoi runs after_ scripts in lexical order of the name that follows the
# run_[once_|onchange_]after_ attribute prefix, so the numeric prefix decides
# ordering. This asserts, from the filenames alone (no render):
#   1. exactly one retirement script exists (*retire-claude-code-launchagent)
#   2. it is a run_after_ script (convergent, re-runnable) -- NOT run_once_
#      (which records success permanently and cannot retry a failed retirement)
#   3. its after-number is strictly less than after_58's, so it precedes the
#      tmux/sesh teardown
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT_DIR="$REPO_ROOT/.chezmoiscripts"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# after_number <basename> : echoes the NN in run_..._after_NN-..., or nothing.
after_number() {
  local base="$1"
  [[ $base =~ _after_([0-9]+)- ]] || return 1
  printf '%s' "${BASH_REMATCH[1]}"
}

# The teardown owner: run_after_58-herdr-migration-verify.
verify_glob=("$SCRIPT_DIR"/run_after_*-herdr-migration-verify.sh.tmpl)
[[ -e ${verify_glob[0]} ]] || fail "cannot find the herdr-migration-verify script (the tmux/sesh teardown owner)"
[[ ${#verify_glob[@]} -eq 1 ]] || fail "expected exactly one herdr-migration-verify script, found ${#verify_glob[@]}"
verify_base="$(basename "${verify_glob[0]}")"
verify_num="$(after_number "$verify_base")" || fail "could not parse an after-number from $verify_base"

# The retirement script, matched by its stable suffix (number-independent).
retire_glob=("$SCRIPT_DIR"/*retire-claude-code-launchagent.sh.tmpl)
[[ -e ${retire_glob[0]} ]] || fail "cannot find the com.claude.code retirement script"
[[ ${#retire_glob[@]} -eq 1 ]] || fail "expected exactly one retirement script, found ${#retire_glob[@]}"
retire_base="$(basename "${retire_glob[0]}")"

# 2) it must be a run_after_ script, not run_once_ (convergence requirement).
[[ $retire_base == run_after_* ]] ||
  fail "retirement script must be a run_after_ (convergent, re-runnable), not '$retire_base' (a run_once_ records success permanently and cannot retry a failed retirement)"

# 3) it must precede after_58's teardown.
retire_num="$(after_number "$retire_base")" || fail "could not parse an after-number from $retire_base"
if [[ $((10#$retire_num)) -ge $((10#$verify_num)) ]]; then
  fail "retirement ($retire_base, after_$retire_num) does not precede the tmux/sesh teardown ($verify_base, after_$verify_num); renumber it below $verify_num"
fi

printf 'PASS: %s is a run_after_ and precedes the tmux/sesh teardown %s (after_%s < after_%s)\n' \
  "$retire_base" "$verify_base" "$retire_num" "$verify_num"
