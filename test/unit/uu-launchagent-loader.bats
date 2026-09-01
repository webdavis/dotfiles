#!/usr/bin/env bats
# The uu LaunchAgent loader, and the one precondition it has to check: is the
# binary the job would run actually there?
#
# The builder beside it DEFERS on a machine with no toolchain and exits 0 so
# the apply carries on, so this loader can run with nothing installed to run.
# Booting the current job out and bootstrapping a plist that points at nothing
# leaves the machine with NO updater at all, which is strictly worse than the
# job that was already loaded.
#
# The rendered script runs whole, against a stub launchctl on PATH and a
# sandbox HOME; nothing here touches this machine's launchd.

setup_file() {
  REPO_ROOT="$(cd "$BATS_TEST_DIRNAME/../.." && pwd)"
  export SCRIPT="$BATS_FILE_TMPDIR/load-uu.sh"
  CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
    <"$REPO_ROOT/.chezmoiscripts/run_onchange_after_71-load-uu-launchagent.sh.tmpl" \
    >"$SCRIPT" 2>/dev/null
  [[ -s $SCRIPT ]] || {
    echo "the loader rendered empty" >&2
    return 1
  }
  chmod +x "$SCRIPT"
}

setup() {
  SANDBOX="$BATS_TEST_TMPDIR"
  CALLS="$SANDBOX/launchctl-calls"
  BIN="$SANDBOX/bin"
  mkdir -p "$BIN"
  printf '#!/bin/bash\nprintf "%%s\\n" "$*" >>"%s"\nexit 0\n' "$CALLS" >"$BIN/launchctl"
  chmod +x "$BIN/launchctl"
}

load_agent() {
  HOME="$SANDBOX" PATH="$BIN:$PATH" "$SCRIPT"
}

# install_uu <mode>: the built binary, at the path the plist names.
install_uu() {
  mkdir -p "$SANDBOX/.local/libexec/uu"
  printf '#!/bin/sh\nexit 0\n' >"$SANDBOX/.local/libexec/uu/uu"
  chmod "$1" "$SANDBOX/.local/libexec/uu/uu"
}

@test "a binary the build has not produced yet leaves the loaded job alone" {
  run load_agent
  [ "$status" -eq 0 ]
  [ ! -e "$CALLS" ]
  # The SENTENCE, not just the silence: a refusal nobody can read looks
  # identical to a loader that decided there was nothing to do.
  [[ $output == *"uu"* ]]
  [[ $output == *"$SANDBOX/.local/libexec/uu/uu"* ]]
}

@test "a binary that is there but not executable is refused the same way" {
  install_uu 644
  run load_agent
  [ "$status" -eq 0 ]
  [ ! -e "$CALLS" ]
  [[ $output == *"$SANDBOX/.local/libexec/uu/uu"* ]]
}

@test "a built binary is booted out and bootstrapped" {
  install_uu 755
  run load_agent
  [ "$status" -eq 0 ]
  [[ "$(cat "$CALLS")" == *bootout* ]]
  [[ "$(cat "$CALLS")" == *bootstrap* ]]
}
