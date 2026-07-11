#!/usr/bin/env bats
# Regression suite for scripts/run-unit-tests.sh (the commit gate's runner).
# Lives in the integration camp: it exercises the runner against scratch camps.
# Also the repo's first bats suite, so `just test` exercises the bats + GNU
# parallel path in CI (bats --jobs requires `parallel`, provided by the flake).

setup() {
  REPO_ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../.." && pwd)"
  scratch="$(mktemp -d)"
  # A minimal repo skeleton the runner can cd into: the runner anchors on its
  # own script path, so copy it plus a scratch test/unit camp.
  mkdir -p "$scratch/scripts" "$scratch/test/unit"
  cp "$REPO_ROOT/scripts/run-unit-tests.sh" "$scratch/scripts/"
}

teardown() {
  rm -rf "$scratch"
}

mk_test() { # <name> <exit-code>
  printf '#!/usr/bin/env bash\nexit %s\n' "$2" > "$scratch/test/unit/$1.sh"
  chmod +x "$scratch/test/unit/$1.sh"
}

@test "all-pass camp exits 0" {
  mk_test a 0
  mk_test b 0
  run "$scratch/scripts/run-unit-tests.sh"
  [ "$status" -eq 0 ]
}

@test "a failing unit test fails the runner" {
  mk_test a 0
  mk_test zz-fail 1
  run "$scratch/scripts/run-unit-tests.sh"
  [ "$status" -eq 1 ]
  [[ "$output" == *"FAIL: "*zz-fail* ]]
}

@test "empty camp is a green no-op" {
  run "$scratch/scripts/run-unit-tests.sh"
  [ "$status" -eq 0 ]
  [[ "$output" == *"no unit tests found"* ]]
}

@test "discovery failure refuses to green-gate" {
  mk_test a 0
  # Shim `find` to emit a partial listing then fail, modeling a truncated
  # discovery; the runner must fail rather than run the partial list.
  mkdir -p "$scratch/bin"
  cat > "$scratch/bin/find" <<SHIM
#!/usr/bin/env bash
/usr/bin/find "\$@"
exit 7
SHIM
  chmod +x "$scratch/bin/find"
  PATH="$scratch/bin:$PATH" run "$scratch/scripts/run-unit-tests.sh"
  [ "$status" -eq 1 ]
  [[ "$output" == *"discovery failed"* ]]
}

@test "same TEST_SEED reproduces the same order" {
  mk_test a 0
  mk_test b 0
  mk_test c 0
  TEST_SEED=42 run "$scratch/scripts/run-unit-tests.sh"
  first="$output"
  TEST_SEED=42 run "$scratch/scripts/run-unit-tests.sh"
  [ "$first" = "$output" ]
}
