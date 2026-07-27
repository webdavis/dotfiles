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

# A fake worktree top carrying the .chezmoiversion marker, so the worktree branch
# of resolve_source_dir is the branch under test. The marker, NOT the data file,
# is what identifies a chezmoi source tree: case 6 pins that a source tree whose
# data file is absent still resolves to ITSELF.
worktree_top="$work/top"
mkdir -p "$worktree_top/.chezmoidata"
: >"$worktree_top/.chezmoiversion"
: >"$worktree_top/.chezmoidata/macos_defaults.yaml"

# A second source tree, marked but with NO data file, for case 6.
markerless_data="$work/marked-no-data"
mkdir -p "$markerless_data"
: >"$markerless_data/.chezmoiversion"

stub_bin="$work/bin"
mkdir -p "$stub_bin"

# write_git_stub <found|missing> [top] -- `git rev-parse --show-toplevel` either
# reports a worktree top or fails, selecting which resolution branch runs. [top]
# defaults to the marked fake worktree.
#
# The found stub HONORS $GIT_WORK_TREE exactly as real git does, printing it in
# preference to the physical top. That is what makes the scrub observable: if the
# library ever stops unsetting the git context variables, this stub reports the
# hijacked directory and case 5 fails.
write_git_stub() { # <found|missing> [top]
  local top="${2:-$worktree_top}"
  if [[ $1 == found ]]; then
    cat >"$stub_bin/git" <<EOF
#!/bin/bash
if [[ "\$1 \$2" == "rev-parse --show-toplevel" ]]; then
  if [[ -n \${GIT_WORK_TREE:-} ]]; then
    printf '%s\n' "\$GIT_WORK_TREE"
  else
    printf '%s\n' "$top"
  fi
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
      # Echoes back whatever --source it was handed, as real chezmoi's
      # `--source=<dir> source-path` does, so a test can tell WHICH tree the
      # library asked about. Falls back to the marked fake worktree.
      cat >"$stub_bin/chezmoi" <<EOF
#!/bin/bash
for arg in "\$@"; do
  case "\$arg" in
    --source=*)
      printf '%s\n' "\${arg#--source=}"
      exit 0
      ;;
  esac
done
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
# Assert on text only the LIBRARY emits. The chezmoi stub writes its own
# "source-path is unavailable" to the same stderr, so grepping for 'source-path'
# passed even with the library's whole diagnostic deleted.
grep -qF 'refusing to fall back' "$work/err" ||
  fail "worktree branch: the library must say it refuses to fall back (stderr: $(cat "$work/err"))"
[[ -z $output ]] ||
  fail "worktree branch: a failed resolution must print no directory on stdout (got '$output')"

# ---- case 3: fallback branch, chezmoi source-path fails ---------------------
write_git_stub missing
write_chezmoi_stub fail
status=0
output="$(call_lib resolve_source_dir)" || status=$?
[[ $status -ne 0 ]] ||
  fail "fallback branch: a failed chezmoi source-path must propagate (got 0, stdout: '$output')"
# Library-unique text again, for the same reason as case 2.
grep -qF 'source directory is unknown' "$work/err" ||
  fail "fallback branch: the library must say the source directory is unknown (stderr: $(cat "$work/err"))"

# ---- case 4: an empty resolution is a failure, not a plausible path ---------
# A chezmoi that exits 0 printing nothing would otherwise compose into
# "/.chezmoidata/macos_defaults.yaml", a real path on the wrong tree.
write_git_stub found
write_chezmoi_stub empty
status=0
output="$(call_lib macos_defaults_data_file)" || status=$?
[[ $status -ne 0 ]] ||
  fail "empty resolution: macos_defaults_data_file must fail rather than compose a rooted path (got 0, stdout: '$output')"
# The EXACT status matters. The tools document 2 as "data file missing or
# unreadable" and capture distinguishes 2 from 3, so asserting merely nonzero
# left the documented contract free to drift to any other code.
[[ $status -eq 2 ]] ||
  fail "empty resolution: macos_defaults_data_file must return the documented status 2, not just nonzero (got $status)"
printf '%s' "$output" >"$work/out"
refute_file_contains "$work/out" '/.chezmoidata/macos_defaults.yaml' \
  "empty resolution: macos_defaults_data_file must not emit a composed path when the source dir is empty"

# ---- case 5: an inherited git context must NOT retarget another checkout -----
# `git rev-parse` honors $GIT_WORK_TREE and $GIT_DIR, so a value exported by a git
# hook or a wrapper used to make the resolver describe a checkout the caller was
# not standing in. The git stub honors GIT_WORK_TREE exactly as real git does, so
# if the library stops scrubbing, the hijacked path surfaces here.
write_git_stub found
write_chezmoi_stub ok
hijack="$work/hijacked-checkout"
mkdir -p "$hijack"
# The hijack target must itself look like a source tree. Without the marker, a
# resolver that FAILED to scrub would reject the hijacked top, fall through to the
# configured-source rule, and land back on the right answer by accident, so the
# case would pass while pinning nothing. Verified: dropping the scrub in the
# library fails this case only once the marker is here.
: >"$hijack/.chezmoiversion"
status=0
output="$(
  PATH="$stub_bin:$PATH" LIB="$LIB" GIT_WORK_TREE="$hijack" GIT_DIR="$hijack/.git" bash -c '
    unset MACOS_DEFAULTS_SOURCE_DIR
    source "$LIB"
    resolve_source_dir
  ' 2>"$work/err"
)" || status=$?
[[ $status -eq 0 ]] ||
  fail "git context: resolution must still succeed with GIT_WORK_TREE set (got $status, stderr: $(cat "$work/err"))"
[[ $output == "$worktree_top" ]] ||
  fail "git context: an inherited GIT_WORK_TREE must not retarget the resolver (got '$output', wanted '$worktree_top')"

# ---- case 6: a source tree with NO data file resolves to ITSELF -------------
# Identity and data-file presence are different questions. Keying identity on the
# data file made an absent file look like "some unrelated directory", so the
# resolver silently fell through to whichever other checkout did have one. The
# tree must resolve to itself and let the readable-file guard report the miss.
write_git_stub found "$markerless_data"
write_chezmoi_stub ok
status=0
output="$(call_lib resolve_source_dir)" || status=$?
[[ $status -eq 0 ]] ||
  fail "marked tree without data: resolution must succeed (got $status, stderr: $(cat "$work/err"))"
[[ $output == "$markerless_data" ]] ||
  fail "marked tree without data: must resolve to ITSELF, not fall through to another checkout (got '$output', wanted '$markerless_data')"

# ---- case 7: an override that is SET BUT EMPTY is rejected ------------------
# Treating set-but-empty as unset silently resolved a different checkout, which is
# a fall-through the resolver's contract forbids.
write_git_stub found
write_chezmoi_stub ok
status=0
output="$(call_lib_with_override resolve_source_dir '')" || status=$?
[[ $status -ne 0 ]] ||
  fail "empty override: a set-but-empty MACOS_DEFAULTS_SOURCE_DIR must be rejected, not skipped (got 0, stdout: '$output')"
grep -qF 'set but empty' "$work/err" ||
  fail "empty override: the library must name the empty override (stderr: $(cat "$work/err"))"
[[ -z $output ]] ||
  fail "empty override: a rejected override must print no directory (got '$output')"

printf 'macos-defaults-source-path-propagation: OK (control resolves; both branches propagate a failed source-path; an empty resolution fails closed with status 2; an inherited git context cannot retarget; a marked tree without data resolves to itself; a set-but-empty override is rejected)\n'
