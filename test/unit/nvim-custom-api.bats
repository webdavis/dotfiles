#!/usr/bin/env bats
# The headless Lua specs for the nvim config's custom_api modules (spec 6.3),
# one @test per spec file.
#
# The process boundary is the behavior here: what these pin is our Lua running
# under a real headless Neovim, which is the case the bats ruling allows a spawn
# for. `--clean` keeps the whole plugin tree out, so each spawn costs about
# 30 ms; `dot_config/nvim/tests/run.lua` puts the SOURCE tree's `lua/` on
# `package.path`, so nothing here reads the deployed config in $HOME.

setup() {
  REPO_ROOT="$(cd "$BATS_TEST_DIRNAME/../.." && pwd)"
  RUNNER="$REPO_ROOT/dot_config/nvim/tests/run.lua"
}

# run_spec <name>_spec: one spec file, green, with every case reported.
run_spec() {
  run nvim --headless --clean -l "$RUNNER" "$1"
  [ "$status" -eq 0 ]
  [[ "$output" != *FAIL* ]]
  [[ "$output" == *"ok $1:"* ]]
}

@test "custom_api auto_reload_spec passes" {
  run_spec auto_reload_spec
}

@test "custom_api bootstrap_spec passes" {
  run_spec bootstrap_spec
}

@test "custom_api util_spec passes" {
  run_spec util_spec
}

@test "custom_api try_spec passes" {
  run_spec try_spec
}

@test "custom_api git_spec passes" {
  run_spec git_spec
}

@test "custom_api github_spec passes" {
  run_spec github_spec
}

@test "the runner exits non-zero on a failing case" {
  # A runner that reported failure only in its output would be a gate that
  # cannot fail, so the exit code is pinned against a spec built to fail.
  local sandbox="$BATS_TEST_TMPDIR/tests"
  mkdir -p "$sandbox"
  cp "$RUNNER" "$sandbox/run.lua"
  printf 'return {\n  ["deliberately fails"] = function()\n    assert(false, "as designed")\n  end,\n}\n' \
    >"$sandbox/failing_spec.lua"

  run nvim --headless --clean -l "$sandbox/run.lua" --config "$REPO_ROOT/dot_config/nvim" failing_spec
  [ "$status" -eq 1 ]
  [[ "$output" == *"FAIL failing_spec: deliberately fails"* ]]
}

@test "the runner refuses a run that matched no spec files" {
  # An empty run would otherwise exit 0 and read as a pass.
  local sandbox="$BATS_TEST_TMPDIR/empty"
  mkdir -p "$sandbox"
  cp "$RUNNER" "$sandbox/run.lua"

  run nvim --headless --clean -l "$sandbox/run.lua" --config "$REPO_ROOT/dot_config/nvim"
  [ "$status" -ne 0 ]
  [[ "$output" == *"no spec files matched"* ]]
}
