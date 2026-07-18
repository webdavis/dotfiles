#!/usr/bin/env bats
# Regression suite for test/run-test-suite.sh, the one test runner. Exercises the
# shuffle and timing options against scratch suites. Also the repo's first bats
# suite, so `just test` exercises the bats + GNU parallel path in CI (bats --jobs
# requires `parallel`, provided by the flake).

setup() {
  # shellcheck source=test/test-system/helpers/find-repo-root.sh
  source "$(dirname "$BATS_TEST_FILENAME")/helpers/find-repo-root.sh"
  REPO_ROOT="$(find_repo_root)"
  RUNNER="$REPO_ROOT/test/run-test-suite.sh"
  scratch="$(mktemp -d)"
  # The runner takes an explicit suite dir, so point it at a scratch one.
  SUITE="$scratch/test/unit"
  mkdir -p "$SUITE"
}

teardown() {
  rm -rf "$scratch"
}

mk_test() { # <name> <exit-code>
  printf '#!/usr/bin/env bash\nexit %s\n' "$2" > "$SUITE/$1.sh"
  chmod +x "$SUITE/$1.sh"
}

# A fixture that records its own name in execution order to $ORDER_LOG, so a
# test can compare the ORDER SEQUENCE across runs (the runner prints no passing
# names, and comparing stdout would only see the seed banner). Exits 0.
mk_order_test() { # <name>
  {
    printf '#!/usr/bin/env bash\n'
    printf 'printf '\''%%s\\n'\'' %q >> "$ORDER_LOG"\n' "$1"
    printf 'exit 0\n'
  } > "$SUITE/$1.sh"
  chmod +x "$SUITE/$1.sh"
}

@test "all-pass suite exits 0" {
  mk_test a 0
  mk_test b 0
  run "$RUNNER" "$SUITE"
  [ "$status" -eq 0 ]
}

@test "a failing test fails the runner" {
  mk_test a 0
  mk_test zz-fail 1
  run "$RUNNER" "$SUITE"
  [ "$status" -eq 1 ]
  [[ "$output" == *"FAIL: "*zz-fail* ]]
}

@test "empty suite is a green no-op" {
  run "$RUNNER" "$SUITE"
  [ "$status" -eq 0 ]
  [[ "$output" == *"no tests found"* ]]
}

@test "an unknown flag is a usage error" {
  mk_test a 0
  run "$RUNNER" --bogus "$SUITE"
  [ "$status" -eq 2 ]
  [[ "$output" == *"usage:"* ]]
}

# The warn threshold feeds a bash arithmetic comparison, so a non-numeric value
# is an injection channel: --warn-slow-ms=status=0 once flipped a FAILING suite
# to exit 0. Values must be unsigned decimal integers, checked before any test
# runs.
@test "--warn-slow-ms rejects an arithmetic expression before running any test" {
  mk_test zz-fail 1
  run "$RUNNER" --warn-slow-ms=status=0 "$SUITE"
  [ "$status" -eq 2 ]
  [[ "$output" == *"usage:"* ]]
  [[ "$output" != *"zz-fail"* ]]
}

@test "--warn-slow-ms rejects an empty value" {
  mk_test a 0
  run "$RUNNER" --warn-slow-ms= "$SUITE"
  [ "$status" -eq 2 ]
  [[ "$output" == *"usage:"* ]]
}

@test "--warn-slow-ms rejects a following flag as its value" {
  mk_test a 0
  run "$RUNNER" --warn-slow-ms --shuffle "$SUITE"
  [ "$status" -eq 2 ]
  [[ "$output" == *"usage:"* ]]
}

@test "--warn-slow-ms normalizes a leading-zero value as base 10" {
  mk_test a 0
  run "$RUNNER" --warn-slow-ms=08 "$SUITE"
  [ "$status" -eq 0 ]
}

# A shuffler that exits 0 with bad output must not replace the validated
# discovery list: an empty list once turned a FAILING suite into a green
# "no tests found", and a truncated list would silently skip tests. The runner
# must verify the shuffled list holds exactly the discovered paths and fail
# the gate on any mismatch.
@test "a shuffler emitting an empty list fails the gate instead of green-gating" {
  mk_test zz-fail 1
  mkdir -p "$scratch/bin"
  printf '#!/usr/bin/env bash\nexit 0\n' > "$scratch/bin/gshuf"
  cp "$scratch/bin/gshuf" "$scratch/bin/shuf"
  chmod +x "$scratch/bin/gshuf" "$scratch/bin/shuf"
  PATH="$scratch/bin:$PATH" run "$RUNNER" --shuffle=1 "$SUITE"
  [ "$status" -eq 1 ]
  [[ "$output" == *"shuffle"* ]]
  [[ "$output" != *"no tests found"* ]]
}

@test "a shuffler truncating the list fails the gate" {
  mk_test aa 0
  mk_test zz-fail 1
  mkdir -p "$scratch/bin"
  # Emit only the first discovered entry (NUL-delimited), dropping the rest.
  cat > "$scratch/bin/gshuf" <<'SHIM'
#!/usr/bin/env bash
tr '\0' '\n' | head -n 1 | tr '\n' '\0'
SHIM
  cp "$scratch/bin/gshuf" "$scratch/bin/shuf"
  chmod +x "$scratch/bin/gshuf" "$scratch/bin/shuf"
  PATH="$scratch/bin:$PATH" run "$RUNNER" --shuffle=1 "$SUITE"
  [ "$status" -eq 1 ]
  [[ "$output" == *"shuffle"* ]]
}

# A NUL-delimited stream must end with a NUL byte. A shuffler emitting the
# final record without its terminator slips past a sorted comparison (sort adds
# the terminator back) while the read loop drops the unterminated record, so a
# failing suite once came back green as "no tests found".
@test "a shuffler emitting an unterminated final record fails the gate" {
  mk_test fail 1
  mkdir -p "$scratch/bin"
  cat > "$scratch/bin/gshuf" <<SHIM
#!/usr/bin/env bash
printf '%s' "$SUITE/fail.sh"
SHIM
  cp "$scratch/bin/gshuf" "$scratch/bin/shuf"
  chmod +x "$scratch/bin/gshuf" "$scratch/bin/shuf"
  PATH="$scratch/bin:$PATH" run "$RUNNER" --shuffle=1 "$SUITE"
  [ "$status" -eq 1 ]
  [[ "$output" == *"shuffle"* ]]
  [[ "$output" != *"no tests found"* ]]
}

@test "--shuffle rejects a non-numeric seed" {
  mk_test a 0
  run "$RUNNER" --shuffle=abc "$SUITE"
  [ "$status" -eq 2 ]
  [[ "$output" == *"usage:"* ]]
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
  PATH="$scratch/bin:$PATH" run "$RUNNER" "$SUITE"
  [ "$status" -eq 1 ]
  [[ "$output" == *"discovery failed"* ]]
}

@test "sort failure refuses to green-gate" {
  mk_test a 0
  # Shim `sort` to exit nonzero, modeling a sort that fails mid-discovery; the
  # discovery pipeline (pipefail on) must fail rather than trust the list.
  mkdir -p "$scratch/bin"
  cat > "$scratch/bin/sort" <<SHIM
#!/usr/bin/env bash
/usr/bin/sort "\$@"
exit 7
SHIM
  chmod +x "$scratch/bin/sort"
  PATH="$scratch/bin:$PATH" run "$RUNNER" "$SUITE"
  [ "$status" -eq 1 ]
  [[ "$output" == *"discovery failed"* ]]
}

@test "--warn-slow-ms emits a performance warning naming the slow test" {
  { printf '#!/usr/bin/env bash\n'; printf 'sleep 0.05\n'; } > "$SUITE/slow.sh"
  chmod +x "$SUITE/slow.sh"
  run "$RUNNER" --warn-slow-ms 10 "$SUITE"
  [ "$status" -eq 0 ]
  [[ "$output" == *"PERFORMANCE WARNING"* ]]
  [[ "$output" == *"slow.sh"* ]]
}

# The runner must not rewrite the locale of the tests it runs: a child test
# has to see the caller's LC_ALL, not a leaked LC_ALL=C from the runner's own
# timing internals.
@test "child tests inherit the caller's locale" {
  {
    printf '#!/usr/bin/env bash\n'
    printf 'printf "%%s" "${LC_ALL:-unset}" > "$LOCALE_LOG"\n'
  } > "$SUITE/locale-probe.sh"
  chmod +x "$SUITE/locale-probe.sh"
  export LOCALE_LOG="$scratch/locale.log"
  LC_ALL=de_DE.UTF-8 run "$RUNNER" "$SUITE"
  [ "$status" -eq 0 ]
  [ "$(cat "$LOCALE_LOG")" = "de_DE.UTF-8" ]
}

# The seed test must have TEETH: it has to go RED if the shuffle is removed or
# TEST_SEED is ignored. It records execution order via mk_order_test fixtures
# (the runner prints no passing names) and asserts BOTH:
#   (1) same seed -> identical order  (fails if TEST_SEED is ignored / random)
#   (2) some seed -> order != plain sorted order  (fails if shuffle is disabled,
#       since a disabled shuffle always yields the sorted order)
@test "TEST_SEED drives a real, reproducible shuffle (not just sorted order)" {
  local names=(00 01 02 03 04 05 06 07 08 09)
  local n
  for n in "${names[@]}"; do mk_order_test "$n"; done
  local sorted
  sorted="$(printf '%s\n' "${names[@]}" | LC_ALL=C sort)"

  # (1) reproducibility under a fixed seed, and the seed line is printed
  export ORDER_LOG="$scratch/order.a1"; : > "$ORDER_LOG"
  TEST_SEED=1 run "$RUNNER" "$SUITE"
  [ "$status" -eq 0 ]
  [[ "$output" == *"seed=1"* ]]
  local a1; a1="$(cat "$ORDER_LOG")"
  export ORDER_LOG="$scratch/order.a2"; : > "$ORDER_LOG"
  TEST_SEED=1 run "$RUNNER" "$SUITE"
  [ "$status" -eq 0 ]
  local a2; a2="$(cat "$ORDER_LOG")"
  [ "$a1" = "$a2" ]

  # (2) the shuffle actually reorders relative to sorted, for at least one seed
  local s reordered=0 order
  for s in 1 2 3 4 5; do
    export ORDER_LOG="$scratch/order.s$s"; : > "$ORDER_LOG"
    TEST_SEED="$s" run "$RUNNER" "$SUITE"
    [ "$status" -eq 0 ]
    order="$(cat "$ORDER_LOG")"
    [ "$order" != "$sorted" ] && reordered=1
  done
  [ "$reordered" -eq 1 ]
}
