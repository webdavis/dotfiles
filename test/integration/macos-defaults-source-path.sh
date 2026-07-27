#!/usr/bin/env bash
# macos-defaults-source-path.sh -- all three macos-defaults tools must resolve
# .chezmoidata/macos_defaults.yaml for the CURRENT context, never a hardcoded
# primary checkout.
#
# The bug this pins: apply, capture and drift each hardcoded
# "${HOME}/workspaces/Ivy/webdavis/dotfiles/.chezmoidata/macos_defaults.yaml", so a
# run from a SECONDARY git worktree read (or wrote) the PRIMARY tree instead of the
# worktree the operator was standing in. The shared library now resolves the source
# directory from the current git worktree, routed through chezmoi, and falls back to
# chezmoi's configured source directory.
#
# Everything lives under one sandbox temp dir. HOME points there and a throwaway
# chezmoi config points sourceDir at a sandbox "primary", so NEITHER the operator's
# real checkout NOR the buggy hardcoded ${HOME}/workspaces/... path can be touched:
# a red run mutates only the sandbox.
#
# Two decoy primaries are seeded, one per wrong answer the old code could give, and
# each of the three tools is asserted to have consulted the WORKTREE and not either
# decoy. That is a completeness check on the set of consumers: a tool that kept a
# private copy of the hardcoded path fails here rather than passing by association.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
APPLY="$REPO_ROOT/dot_local/bin/executable_macos-defaults-apply.sh"
CAPTURE="$REPO_ROOT/dot_local/bin/executable_macos-defaults-capture.sh"
DRIFT="$REPO_ROOT/dot_local/bin/executable_macos-defaults-drift.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# refute_file_contains <file> <fixed-string> <message> -- the explicit negative
# assertion. A bare `! grep` is dead under `set -e` unless it happens to be the
# last statement, so every negative below goes through this helper.
refute_file_contains() { # <file> <fixed-string> <message>
  if grep -qF -- "$2" "$1"; then
    fail "$3"
  fi
}

assert_file_contains() { # <file> <fixed-string> <message>
  grep -qF -- "$2" "$1" || fail "$3"
}

# Host-tool guard: a suite *.sh runs with host tools. The de-homebrewed
# CI-faithful run has no chezmoi/yq/git on PATH, and this test cannot exercise
# source-directory resolution without them.
for tool in chezmoi yq git; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'SKIP: %s not on PATH; cannot exercise source-directory resolution\n' "$tool"
    exit 0
  }
done
for script in "$APPLY" "$CAPTURE" "$DRIFT"; do
  [[ -f $script ]] || fail "missing script: $script"
done

# Canonicalize away macOS's /var -> /private/var symlink so these paths match what
# `git rev-parse --show-toplevel` (used by the resolver under test) reports.
sandbox="$(cd "$(mktemp -d)" && pwd -P)"
trap 'chmod -R u+rwX "$sandbox" 2>/dev/null; rm -rf "$sandbox"' EXIT

WORKTREE_DOMAIN="com.example.s10worktree"
PRIMARY_DOMAIN="com.example.s10primary"
CAPTURED_DOMAIN="com.example.s10captured"

# seed_data_file <path> <domain> -- one tracked bool record under <domain>.
seed_data_file() { # <path> <domain>
  mkdir -p "$(dirname "$1")"
  cat >"$1" <<EOF
macos:
  defaults:
    - domain: $2
      key: s10flag
      type: bool
      value: true
  killall: []
EOF
}

# Decoy 1: chezmoi's configured source directory.
mkdir -p "$sandbox/.config/chezmoi"
printf 'sourceDir = "%s/primary-src"\n' "$sandbox" >"$sandbox/.config/chezmoi/chezmoi.toml"
configured_primary="$sandbox/primary-src/.chezmoidata/macos_defaults.yaml"
seed_data_file "$configured_primary" "$PRIMARY_DOMAIN"

# Decoy 2: the path the buggy tools hardcoded. With HOME=sandbox it lands here, so
# a red run has a readable target and still mutates only the sandbox.
hardcoded_primary="$sandbox/workspaces/Ivy/webdavis/dotfiles/.chezmoidata/macos_defaults.yaml"
seed_data_file "$hardcoded_primary" "$PRIMARY_DOMAIN"

# The secondary worktree the operator is standing in: a real git repo carrying its
# own macos_defaults.yaml and the .chezmoiversion marker that identifies a chezmoi
# source tree. The marker, not the data file, is what the resolver keys on, so the
# unreadable-file cases below still resolve THIS tree rather than falling through.
worktree="$sandbox/wt"
worktree_data_file="$worktree/.chezmoidata/macos_defaults.yaml"
seed_data_file "$worktree_data_file" "$WORKTREE_DOMAIN"
printf '2.62.3\n' >"$worktree/.chezmoiversion"
git -C "$worktree" init -q
# Pre-flight: the worktree branch of resolution fires only when git reports this
# directory as its own top level. Assert it, so a green pass cannot hide a silent
# fallback to a primary.
[[ "$(git -C "$worktree" rev-parse --show-toplevel)" == "$worktree" ]] ||
  fail "test setup: $worktree is not its own git top level"

# Stubs. `defaults` logs every invocation so the apply assertions can read which
# record was written; osascript and killall are stubbed so apply cannot reach the
# operator's real System Settings or processes.
stub_bin="$sandbox/bin"
defaults_log="$sandbox/defaults.log"
: >"$defaults_log"
mkdir -p "$stub_bin"
cat >"$stub_bin/defaults" <<STUB
#!/bin/bash
printf '%s\n' "\$*" >>"$defaults_log"
for arg in "\$@"; do
  case "\$arg" in
    read-type)
      printf 'Type is boolean\n'
      exit 0
      ;;
    read)
      printf '1\n'
      exit 0
      ;;
  esac
done
exit 0
STUB
printf '#!/bin/bash\nexit 0\n' >"$stub_bin/osascript"
printf '#!/bin/bash\nexit 0\n' >"$stub_bin/killall"
chmod +x "$stub_bin/defaults" "$stub_bin/osascript" "$stub_bin/killall"

# run_from_worktree <script> [args...] -- run a tool with the operator standing in
# the worktree, against the sandbox HOME and the stub PATH.
# The caller's own environment is scrubbed. This suite runs on developer machines
# where the very override the library invites, and git's context variables, may
# already be exported; inheriting any of them resolves a different tree and fails
# the run for a reason that has nothing to do with the code. That is a false RED,
# so the sandbox is made explicit rather than assumed.
run_from_worktree() { # <script> [args...]
  (
    cd "$worktree" || exit 1
    unset MACOS_DEFAULTS_SOURCE_DIR GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE
    HOME="$sandbox" XDG_CONFIG_HOME="$sandbox/.config" PATH="$stub_bin:$PATH" bash "$@"
  )
}

# ---- apply (read path, whole record) ---------------------------------------
# apply must write the WORKTREE's record and never the decoys' record.
run_from_worktree "$APPLY" || fail "apply exited non-zero from the worktree"

assert_file_contains "$defaults_log" "$WORKTREE_DOMAIN" \
  "apply did not write the worktree's record; it read a primary instead of the worktree"
refute_file_contains "$defaults_log" "$PRIMARY_DOMAIN" \
  "apply wrote a primary checkout's record instead of the worktree's"

# ---- capture (write path, the strongest proof) -------------------------------
run_from_worktree "$CAPTURE" "$CAPTURED_DOMAIN" s10flag ||
  fail "capture exited non-zero from the worktree"

assert_file_contains "$worktree_data_file" "$CAPTURED_DOMAIN" \
  "capture did not write the worktree yaml; the record went to a primary"
refute_file_contains "$hardcoded_primary" "$CAPTURED_DOMAIN" \
  "capture wrote the hardcoded ~/workspaces/... primary instead of the worktree"
refute_file_contains "$configured_primary" "$CAPTURED_DOMAIN" \
  "capture wrote the chezmoi-configured primary instead of the worktree"

# ---- drift (read path, unreadable data file) ---------------------------------
# An unreadable WORKTREE yaml must exit 2 naming that file. Both decoys stay
# readable, so a tool still reading a primary exits 0 or 1 here and fails.
chmod 000 "$worktree_data_file"
drift_stderr="$sandbox/drift.err"
drift_status=0
run_from_worktree "$DRIFT" 2>"$drift_stderr" || drift_status=$?
chmod u+rw "$worktree_data_file"

[[ $drift_status -eq 2 ]] ||
  fail "drift from the worktree with an unreadable yaml must exit 2 (got $drift_status); it read a primary, not the worktree"
assert_file_contains "$drift_stderr" "$worktree_data_file" \
  "drift's exit-2 message must name the WORKTREE yaml, proving it resolved the worktree (stderr: $(cat "$drift_stderr"))"

# ---- apply and capture, unreadable data file --------------------------------
# require_readable_data_file RETURNS its status rather than exiting, so each call
# site has to propagate it. Only drift's site was exercised above; a site that
# dropped the propagation would degrade a clean exit 2 into yq's own failure with
# a different status and message, and nothing would have noticed. Every tool gets
# the same assertion so this stays a property of the set, not of one member.
assert_unreadable_exits_two() { # <label> <script> [args...]
  local label="$1" script="$2"
  shift 2
  local err="$sandbox/$label.err" status=0
  chmod 000 "$worktree_data_file"
  run_from_worktree "$script" "$@" 2>"$err" || status=$?
  chmod u+rw "$worktree_data_file"

  [[ $status -eq 2 ]] ||
    fail "$label with an unreadable worktree yaml must exit 2, the documented status (got $status, stderr: $(cat "$err"))"
  assert_file_contains "$err" "$worktree_data_file" \
    "$label's exit-2 message must name the WORKTREE yaml (stderr: $(cat "$err"))"
}

assert_unreadable_exits_two apply "$APPLY"
assert_unreadable_exits_two capture "$CAPTURE" "$CAPTURED_DOMAIN" s10flag

printf 'macos-defaults-source-path: OK (apply, capture and drift all resolve the worktree; both decoy primaries untouched; all three exit 2 naming the worktree yaml when it is unreadable)\n'
