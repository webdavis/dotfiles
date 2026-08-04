#!/usr/bin/env bash
# install-password-manager-hook.sh -- .install-password-manager.sh is wired in
# .chezmoi.toml.tmpl as chezmoi's `hooks.read-source-state.pre` command, so it
# runs before EVERY chezmoi command that reads the source state, including the
# read-only ones (`execute-template`, `managed`, `status`). Its exit status is
# the whole command's exit status: a hook that fails aborts the chezmoi call
# that triggered it.
#
# That makes the hook best-effort by construction. It may install KeePassXC when
# it can, but a machine it cannot install on must still be able to RUN chezmoi.
# The cases, all with the hook's own PATH under test control:
#
#   keepassxc-cli present            -> exit 0, no package manager touched
#   absent, brew present             -> exit 0, `brew install --cask keepassxc`
#   absent, brew install fails       -> exit 0, names the failure on stderr
#   absent, brew absent              -> exit 0, names KeePassXC and Homebrew
#   unsupported OS                   -> exit 1 (deliberately still fatal: no
#                                       branch exists to install anything, and
#                                       the message is the only signal)
#
# The brew-absent case is the one that coupled the whole render-test layer to
# Homebrew: the hook exited 127 there, so every test that rendered a template
# failed on a host without brew on PATH.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HOOK="$REPO_ROOT/.install-password-manager.sh"

fail() {
  printf 'install-password-manager-hook: FAIL -- %s\n' "$*" >&2
  exit 1
}

[[ -x $HOOK ]] || fail "missing or non-executable hook: $HOOK"

work="$(mktemp -d)"
# `trash` is the operator's rule for interactive removals; a committed test has
# to run on a bare CI runner, where only coreutils exist.
trap 'rm -rf "$work"' EXIT

# The hook needs `uname` and a shell, and must find NOTHING else unless this
# test puts it there. /usr/bin:/bin carries neither brew nor keepassxc-cli on a
# stock macOS or Linux host; assert that instead of assuming it, or a stray
# system-wide install would silently turn a case into a different case.
BASE_PATH="/usr/bin:/bin"
for absent in brew keepassxc-cli; do
  if PATH="$BASE_PATH" command -v "$absent" >/dev/null 2>&1; then
    fail "$absent resolves under PATH=$BASE_PATH, so the cases below cannot control whether the hook sees it"
  fi
done

failures=0
report() { # <ok|bad> <message>
  if [[ $1 == ok ]]; then
    printf '  ok   %s\n' "$2"
  else
    printf '  FAIL %s\n' "$2"
    failures=$((failures + 1))
  fi
}

# run_case <name> <stub-spec...> -- build a stub bin dir from the specs, run the
# hook with PATH = that dir plus the base path, and publish RC, ERR and the
# recorded stub calls in CALLS.
#
# A stub spec is "<name>:<exit-status>[:<stdout>]". Every stub appends its own
# argv to the case's call log, so a case can prove a command was NOT run, not
# just that the hook exited a particular way.
run_case() {
  local name="$1"
  shift
  local dir="$work/$name"
  mkdir -p "$dir/bin"
  local calls="$dir/calls"
  : >"$calls"

  local spec stub_name stub_status stub_stdout
  for spec in "$@"; do
    IFS=: read -r stub_name stub_status stub_stdout <<<"$spec"
    {
      printf '#!/bin/sh\n'
      printf 'printf "%%s %%s\\n" "%s" "$*" >>"%s"\n' "$stub_name" "$calls"
      [[ -n $stub_stdout ]] && printf 'printf "%%s\\n" "%s"\n' "$stub_stdout"
      printf 'exit %s\n' "$stub_status"
    } >"$dir/bin/$stub_name"
    chmod +x "$dir/bin/$stub_name"
  done

  # $(<file) and the pattern tests below are builtins: this is a unit test, and
  # a fork per assertion is most of what it would otherwise cost.
  RC=0
  PATH="$dir/bin:$BASE_PATH" "$HOOK" >"$dir/out" 2>"$dir/err" || RC=$?
  ERR="$(<"$dir/err")"
  CALLS="$(<"$calls")"
}

assert_status() { # <case> <expected-status>
  if [[ $RC -eq $2 ]]; then
    report ok "$1: exits $2"
  else
    report bad "$1: expected exit $2, got $RC (stderr: $ERR)"
  fi
}

assert_called() { # <case> <fragment>
  if [[ $CALLS == *"$2"* ]]; then
    report ok "$1: ran '$2'"
  else
    report bad "$1: expected a call matching '$2', recorded calls: ${CALLS:-<none>}"
  fi
}

assert_not_called() { # <case> <fragment>
  if [[ $CALLS == *"$2"* ]]; then
    report bad "$1: must NOT run '$2', recorded calls: $CALLS"
  else
    report ok "$1: did not run '$2'"
  fi
}

assert_stderr_mentions() { # <case> <fragment>
  if [[ $ERR == *"$2"* ]]; then
    report ok "$1: stderr names '$2'"
  else
    report bad "$1: stderr must name '$2' (stderr: ${ERR:-<empty>})"
  fi
}

assert_stderr_silent() { # <case>
  if [[ -z $ERR ]]; then
    report ok "$1: silent"
  else
    report bad "$1: expected no stderr, got: $ERR"
  fi
}

printf 'install-password-manager-hook cases:\n'

# The password manager is already there: nothing to do, and nothing installed.
run_case already-installed keepassxc-cli:0 brew:0
assert_status already-installed 0
assert_not_called already-installed brew
assert_stderr_silent already-installed

# The hook's real job on a fresh Mac that has Homebrew.
run_case installs-with-brew brew:0
assert_status installs-with-brew 0
assert_called installs-with-brew 'brew install --cask keepassxc'

# Homebrew is there but the install does not work (offline, a cask rename, a
# broken tap). The hook reports it and still lets chezmoi run.
run_case brew-install-fails brew:1
assert_status brew-install-fails 0
assert_called brew-install-fails 'brew install --cask keepassxc'
assert_stderr_mentions brew-install-fails KeePassXC

# Neither the password manager nor the package manager that would install it.
# The hook has nothing it can do, so it says so and gets out of chezmoi's way.
run_case no-package-manager
assert_status no-package-manager 0
assert_stderr_mentions no-package-manager KeePassXC
assert_stderr_mentions no-package-manager Homebrew

# An OS with no install branch at all stays fatal: there is no bootstrap to
# protect on a platform this repo does not target, and the error is the signal.
run_case unsupported-os uname:0:Plan9
assert_status unsupported-os 1
assert_stderr_mentions unsupported-os Plan9

if [[ $failures -gt 0 ]]; then
  printf 'install-password-manager-hook: %d assertion(s) FAILED\n' "$failures" >&2
  exit 1
fi
printf 'install-password-manager-hook: OK (present, installable, install failed, no package manager, unsupported OS)\n'
