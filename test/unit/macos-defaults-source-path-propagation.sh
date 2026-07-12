#!/usr/bin/env bash
# macos-defaults-source-path-propagation.sh -- resolve_source_dir (in
# macos-defaults-lib.sh) must PROPAGATE a nonzero `chezmoi source-path` exit, not
# mask it with an unconditional `return 0` (R1-6). The worktree branch ran `chezmoi
# --source=<top> source-path` on its own line then `return 0` regardless of exit
# status, so a chezmoi failure was swallowed; the tool only failed later, and less
# precisely, via a downstream readability guard.
#
# Sources the lib with a git stub (so the worktree branch fires) and a chezmoi stub
# that FAILS `source-path`, then calls resolve_source_dir in a shell WITHOUT `set
# -e` (a caller without set -e is exactly where the `return 0` masks the failure;
# under set -e the mask is incidentally hidden). Asserts nonzero return and a
# precise message. Pure stubs, fast: a unit test.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LIB="$REPO_ROOT/dot_local/bin/macos-defaults-lib.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}
[[ -f $LIB ]] || fail "missing lib: $LIB"

work="$(cd "$(mktemp -d)" && pwd -P)"
trap 'rm -rf "$work"' EXIT

# A fake worktree top carrying the data file, so the worktree branch of
# resolve_source_dir is the branch that runs.
top="$work/top"
mkdir -p "$top/.chezmoidata"
: >"$top/.chezmoidata/macos_defaults.yaml"

stub="$work/bin"
mkdir -p "$stub"
cat >"$stub/git" <<EOF
#!/bin/bash
if [[ "\$1 \$2" == "rev-parse --show-toplevel" ]]; then
  printf '%s\n' "$top"
  exit 0
fi
exit 0
EOF
cat >"$stub/chezmoi" <<'EOF'
#!/bin/bash
# Fail specifically on the source-path call (the masked one); succeed otherwise.
for a in "$@"; do
  [[ $a == source-path ]] && {
    printf 'chezmoi: boom\n' >&2
    exit 1
  }
done
exit 0
EOF
chmod +x "$stub/git" "$stub/chezmoi"

# Deliberately NO `set -e` in the inner shell: that is the caller context in which
# the old `return 0` masks the chezmoi failure.
rc=0
out="$(PATH="$stub:$PATH" LIB="$LIB" MACOS_DEFAULTS_SOURCE_DIR="" bash -c '
  unset MACOS_DEFAULTS_SOURCE_DIR
  source "$LIB"
  resolve_source_dir
' 2>"$work/err")" || rc=$?

[[ $rc -ne 0 ]] ||
  fail "resolve_source_dir must PROPAGATE the chezmoi source-path failure (got rc=0, masked; stdout: '$out')"
grep -qi 'source-path' "$work/err" ||
  fail "expected a precise error mentioning source-path (stderr: $(cat "$work/err"))"

printf 'macos-defaults-source-path-propagation: OK (chezmoi source-path failure propagates with a precise message, not masked)\n'
