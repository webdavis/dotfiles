#!/usr/bin/env bats
# homebrew-weekly-upgrade.sh, the OSQUERY CONVERGE STEP only: what the run
# records when the converge tool it calls is not deployed.
#
# That step exists because this job is the only thing on the machine that
# upgrades the osquery CASK, and the cask reinstall wipes our config, packs and
# flags out of /var/osquery. So a week in which the converge did not run is a
# week the root daemon may have spent on the vendor default configuration, and
# the record has to say so rather than read as clean.
#
# The whole script runs here, against stubs: brew, mas and the relay are
# fixtures, nothing upgrades, nothing is posted, and HOME is a sandbox.

setup() {
  SCRIPT="$(cd "$BATS_TEST_DIRNAME/../.." && pwd)/dot_local/libexec/unattended-upgrades/executable_homebrew-weekly-upgrade.sh"
  SANDBOX="$BATS_TEST_TMPDIR"
  STATE_DIR="$SANDBOX/state"
  BIN="$SANDBOX/bin"
  mkdir -p "$STATE_DIR" "$BIN"

  # brew and mas: every subcommand succeeds and prints nothing worth reading.
  printf '#!/bin/bash\nexit 0\n' >"$BIN/brew"
  printf '#!/bin/bash\nexit 0\n' >"$BIN/mas"
  printf '#!/bin/bash\nexit 0\n' >"$BIN/converge"
  chmod +x "$BIN/brew" "$BIN/mas" "$BIN/converge"
}

# weekly <converge-path>: one full run of the job with the converge step pointed
# wherever the test wants it.
weekly() {
  HOME="$SANDBOX" \
    HOMEBREW_WEEKLY_BREW="$BIN/brew" \
    HOMEBREW_WEEKLY_MAS="$BIN/mas" \
    HOMEBREW_WEEKLY_TAILSCALED="$SANDBOX/no-tailscaled" \
    HOMEBREW_WEEKLY_RELAY="$SANDBOX/no-relay" \
    HOMEBREW_WEEKLY_OSQUERY_CONVERGE="$1" \
    HOMEBREW_WEEKLY_LOCKFILE="$SANDBOX/weekly.lock" \
    HOMEBREW_WEEKLY_STATE_DIR="$STATE_DIR" \
    bash "$SCRIPT"
}

@test "a converge tool that is not deployed FAILS the run rather than recording ok" {
  run weekly "$SANDBOX/definitely-not-deployed.sh"
  [ "$status" -ne 0 ]
  [[ $output == *"osquery config converge"* ]]
  [[ $output == *FAILED* ]]
  # The DIAGNOSTIC, not just the failure. Deleting the executability guard leaves
  # the step failing anyway, because bash cannot exec a path that is not there, so
  # a test that only asserts the failure passes with the guard gone and pins
  # nothing. What the guard is for is the sentence: the record has to name the
  # missing tool and what to do, not read "No such file or directory".
  [[ $output == *"$SANDBOX/definitely-not-deployed.sh is not executable"* ]]
  [[ $output == *"may be the vendor default"* ]]
  [[ $output == *"run chezmoi apply"* ]]
}

@test "a converge tool that is not deployed does not advance the last-success marker" {
  # The marker is what every weekly record's gap figure is measured from, so
  # advancing it over a week that never converged would make the record claim a
  # clean run for exactly the week nobody repaired the monitor.
  run weekly "$SANDBOX/definitely-not-deployed.sh"
  [ "$status" -ne 0 ]
  [ ! -e "$STATE_DIR/last-success-at" ]
}

@test "a deployed converge tool leaves the run clean and marks the success" {
  run weekly "$BIN/converge"
  [ "$status" -eq 0 ]
  [ -e "$STATE_DIR/last-success-at" ]
}
