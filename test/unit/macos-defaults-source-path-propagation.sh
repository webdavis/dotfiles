#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LIB="$REPO_ROOT/scripts/macos-defaults/helpers/defaults-records.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

refute_file_contains() {
  if grep -qF -- "$2" "$1"; then
    fail "$3"
  fi
}

[[ -f $LIB ]] || fail "missing lib: $LIB"

work="$(cd "$(mktemp -d)" && pwd -P)"
trap 'rm -rf "$work"' EXIT

worktree_top="$work/top"
mkdir -p "$worktree_top/.chezmoidata"
: >"$worktree_top/.chezmoiversion"
: >"$worktree_top/.chezmoidata/macos_defaults.yaml"

markerless_data="$work/marked-no-data"
mkdir -p "$markerless_data"
: >"$markerless_data/.chezmoiversion"

stub_bin="$work/bin"
mkdir -p "$stub_bin"

write_git_stub() {
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

write_chezmoi_stub() {
  case "$1" in
    ok)
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

call_lib() {
  PATH="$stub_bin:$PATH" LIB="$LIB" FUNCTION="$1" bash -c '
    unset MACOS_DEFAULTS_SOURCE_DIR
    source "$LIB"
    "$FUNCTION"
  ' 2>"$work/err"
}

call_lib_with_override() {
  PATH="$stub_bin:$PATH" LIB="$LIB" FUNCTION="$1" MACOS_DEFAULTS_SOURCE_DIR="$2" bash -c '
    source "$LIB"
    "$FUNCTION"
  ' 2>"$work/err"
}

write_git_stub missing
write_chezmoi_stub fail
status=0
output="$(call_lib_with_override resolve_source_dir "$work/override")" || status=$?
[[ $status -eq 0 ]] ||
  fail "override: MACOS_DEFAULTS_SOURCE_DIR must win outright (got $status, stderr: $(cat "$work/err"))"
[[ $output == "$work/override" ]] ||
  fail "override: resolve_source_dir must print the override verbatim (got '$output')"

write_git_stub found
write_chezmoi_stub ok
control_status=0
control_output="$(call_lib resolve_source_dir)" || control_status=$?
[[ $control_status -eq 0 ]] ||
  fail "control: resolve_source_dir must succeed when chezmoi succeeds (got $control_status, stderr: $(cat "$work/err"))"
[[ $control_output == "$worktree_top" ]] ||
  fail "control: resolve_source_dir must print the resolved dir (got '$control_output')"

write_git_stub found
write_chezmoi_stub fail
status=0
output="$(call_lib resolve_source_dir)" || status=$?
[[ $status -ne 0 ]] ||
  fail "worktree branch: a failed chezmoi source-path must propagate, not be masked by return 0 (got 0, stdout: '$output')"
grep -qF 'refusing to fall back' "$work/err" ||
  fail "worktree branch: the library must say it refuses to fall back (stderr: $(cat "$work/err"))"
[[ -z $output ]] ||
  fail "worktree branch: a failed resolution must print no directory on stdout (got '$output')"

write_git_stub missing
write_chezmoi_stub fail
status=0
output="$(call_lib resolve_source_dir)" || status=$?
[[ $status -ne 0 ]] ||
  fail "fallback branch: a failed chezmoi source-path must propagate (got 0, stdout: '$output')"
grep -qF 'source directory is unknown' "$work/err" ||
  fail "fallback branch: the library must say the source directory is unknown (stderr: $(cat "$work/err"))"

write_git_stub found
write_chezmoi_stub empty
status=0
output="$(call_lib macos_defaults_data_file)" || status=$?
[[ $status -ne 0 ]] ||
  fail "empty resolution: macos_defaults_data_file must fail rather than compose a rooted path (got 0, stdout: '$output')"
[[ $status -eq 2 ]] ||
  fail "empty resolution: macos_defaults_data_file must return the documented status 2, not just nonzero (got $status)"
printf '%s' "$output" >"$work/out"
refute_file_contains "$work/out" '/.chezmoidata/macos_defaults.yaml' \
  "empty resolution: macos_defaults_data_file must not emit a composed path when the source dir is empty"

write_git_stub found
write_chezmoi_stub ok
hijack="$work/hijacked-checkout"
mkdir -p "$hijack"
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

write_git_stub found "$markerless_data"
write_chezmoi_stub ok
status=0
output="$(call_lib resolve_source_dir)" || status=$?
[[ $status -eq 0 ]] ||
  fail "marked tree without data: resolution must succeed (got $status, stderr: $(cat "$work/err"))"
[[ $output == "$markerless_data" ]] ||
  fail "marked tree without data: must resolve to ITSELF, not fall through to another checkout (got '$output', wanted '$markerless_data')"

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
