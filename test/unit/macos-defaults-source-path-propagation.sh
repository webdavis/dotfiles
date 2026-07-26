#!/usr/bin/env bash
# macos-defaults-source-path-propagation.sh -- resolve_source_dir must PROPAGATE a
# failed `chezmoi source-path`, never mask it and never fall through to a different
# checkout.
#
# The masking shape this pins: running `chezmoi --source=<top> source-path` on its
# own line and then `return 0` unconditionally. Under `set -e` the mask is hidden
# by the caller dying anyway, so every case below calls the function from a shell
# WITHOUT `set -e`, which is the only context where a masked failure is observable.
# Falling through to the next resolution rule is the same bug wearing a different
# hat: it would silently retarget another checkout, which is exactly what this
# resolver exists to prevent.
#
# Pure stubs, no real git and no real chezmoi, so this is a unit test. Case 1 is
# the unmutated control: a green harness must be able to report success.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LIB="$REPO_ROOT/dot_local/bin/macos-defaults-lib.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# refute_file_contains <file> <fixed-string> <message> -- explicit negative
# assertion. A bare `! grep` is dead under `set -e`, so negatives go through here.
refute_file_contains() { # <file> <fixed-string> <message>
  if grep -qF -- "$2" "$1"; then
    fail "$3"
  fi
}

[[ -f $LIB ]] || fail "missing lib: $LIB"

work="$(cd "$(mktemp -d)" && pwd -P)"
trap 'rm -rf "$work"' EXIT

# A fake worktree top carrying the data file, so the worktree branch of
# resolve_source_dir is the branch under test.
worktree_top="$work/top"
mkdir -p "$worktree_top/.chezmoidata"
: >"$worktree_top/.chezmoidata/macos_defaults.yaml"

stub_bin="$work/bin"
mkdir -p "$stub_bin"

# write_git_stub <found|missing> -- `git rev-parse --show-toplevel` either reports
# the fake worktree top or fails, selecting which resolution branch runs.
write_git_stub() { # <found|missing>
  if [[ $1 == found ]]; then
    cat >"$stub_bin/git" <<EOF
#!/bin/bash
if [[ "\$1 \$2" == "rev-parse --show-toplevel" ]]; then
  printf '%s\n' "$worktree_top"
  exit 0
fi
exit 0
EOF
  else
    cat >"$stub_bin/git" <<'EOF'
#!/bin/bash
if [[ "$1 $2" == "rev-parse --show-toplevel" ]]; then
  printf 'fatal: not a git repository\n' >&2
  exit 128
fi
exit 0
EOF
  fi
  chmod +x "$stub_bin/git"
}

# write_chezmoi_stub <ok|fail|empty> -- how `chezmoi source-path` behaves.
write_chezmoi_stub() { # <ok|fail|empty>
  case "$1" in
    ok)
      cat >"$stub_bin/chezmoi" <<EOF
#!/bin/bash
printf '%s\n' "$worktree_top"
exit 0
EOF
      ;;
    fail)
      cat >"$stub_bin/chezmoi" <<'EOF'
#!/bin/bash
printf 'chezmoi: source-path is unavailable\n' >&2
exit 1
EOF
      ;;
    empty)
      cat >"$stub_bin/chezmoi" <<'EOF'
#!/bin/bash
exit 0
EOF
      ;;
  esac
  chmod +x "$stub_bin/chezmoi"
}

# call_lib <function> -- run one library function in a shell WITHOUT `set -e`,
# with the stubs on PATH and no MACOS_DEFAULTS_SOURCE_DIR override. Prints the
# function's stdout, writes its stderr to "$work/err", returns its status.
call_lib() { # <function>
  PATH="$stub_bin:$PATH" LIB="$LIB" FUNCTION="$1" bash -c '
    unset MACOS_DEFAULTS_SOURCE_DIR
    source "$LIB"
    "$FUNCTION"
  ' 2>"$work/err"
}

# call_lib_with_override <function> <dir> -- same, with the explicit override set.
call_lib_with_override() { # <function> <dir>
  PATH="$stub_bin:$PATH" LIB="$LIB" FUNCTION="$1" MACOS_DEFAULTS_SOURCE_DIR="$2" bash -c '
    source "$LIB"
    "$FUNCTION"
  ' 2>"$work/err"
}

# ---- case 0: the explicit override wins and short-circuits ------------------
# Both stubs fail hard, so a resolver that consulted git or chezmoi at all cannot
# return the override's value.
write_git_stub missing
write_chezmoi_stub fail
status=0
output="$(call_lib_with_override resolve_source_dir "$work/override")" || status=$?
[[ $status -eq 0 ]] ||
  fail "override: MACOS_DEFAULTS_SOURCE_DIR must win outright (got $status, stderr: $(cat "$work/err"))"
[[ $output == "$work/override" ]] ||
  fail "override: resolve_source_dir must print the override verbatim (got '$output')"

# ---- case 1: control. A working chezmoi resolves cleanly. -------------------
write_git_stub found
write_chezmoi_stub ok
control_status=0
control_output="$(call_lib resolve_source_dir)" || control_status=$?
[[ $control_status -eq 0 ]] ||
  fail "control: resolve_source_dir must succeed when chezmoi succeeds (got $control_status, stderr: $(cat "$work/err"))"
[[ $control_output == "$worktree_top" ]] ||
  fail "control: resolve_source_dir must print the resolved dir (got '$control_output')"

# ---- case 2: worktree branch, chezmoi source-path fails ---------------------
write_git_stub found
write_chezmoi_stub fail
status=0
output="$(call_lib resolve_source_dir)" || status=$?
[[ $status -ne 0 ]] ||
  fail "worktree branch: a failed chezmoi source-path must propagate, not be masked by return 0 (got 0, stdout: '$output')"
grep -qF 'source-path' "$work/err" ||
  fail "worktree branch: the error must name source-path (stderr: $(cat "$work/err"))"
[[ -z $output ]] ||
  fail "worktree branch: a failed resolution must print no directory on stdout (got '$output')"

# ---- case 3: fallback branch, chezmoi source-path fails ---------------------
write_git_stub missing
write_chezmoi_stub fail
status=0
output="$(call_lib resolve_source_dir)" || status=$?
[[ $status -ne 0 ]] ||
  fail "fallback branch: a failed chezmoi source-path must propagate (got 0, stdout: '$output')"
grep -qF 'source-path' "$work/err" ||
  fail "fallback branch: the error must name source-path (stderr: $(cat "$work/err"))"

# ---- case 4: an empty resolution is a failure, not a plausible path ---------
# A chezmoi that exits 0 printing nothing would otherwise compose into
# "/.chezmoidata/macos_defaults.yaml", a real path on the wrong tree.
write_git_stub found
write_chezmoi_stub empty
status=0
output="$(call_lib macos_defaults_data_file)" || status=$?
[[ $status -ne 0 ]] ||
  fail "empty resolution: macos_defaults_data_file must fail rather than compose a rooted path (got 0, stdout: '$output')"
printf '%s' "$output" >"$work/out"
refute_file_contains "$work/out" '/.chezmoidata/macos_defaults.yaml' \
  "empty resolution: macos_defaults_data_file must not emit a composed path when the source dir is empty"

printf 'macos-defaults-source-path-propagation: OK (control resolves; both branches propagate a failed source-path; an empty resolution fails closed)\n'
