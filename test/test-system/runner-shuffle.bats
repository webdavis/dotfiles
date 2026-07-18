#!/usr/bin/env bats
# Regression suite for test/run-unit-tests.sh (the commit gate's runner).
# Lives in the test-system suite: it exercises the runner against scratch camps.
# Also the repo's first bats suite, so `just test` exercises the bats + GNU
# parallel path in CI (bats --jobs requires `parallel`, provided by the flake).

setup() {
  REPO_ROOT="$(cd "$(dirname "$BATS_TEST_FILENAME")/../.." && pwd)"
  scratch="$(mktemp -d)"
  # A minimal repo skeleton the runner can cd into: the runner anchors on its
  # own script path, so copy it plus a scratch test/unit camp.
  mkdir -p "$scratch/test/unit"
  cp "$REPO_ROOT/test/run-unit-tests.sh" "$scratch/test/"
}

teardown() {
  rm -rf "$scratch"
}

mk_test() { # <name> <exit-code>
  printf '#!/usr/bin/env bash\nexit %s\n' "$2" > "$scratch/test/unit/$1.sh"
  chmod +x "$scratch/test/unit/$1.sh"
}

# A fixture that records its own name in execution order to $ORDER_LOG, so a
# test can compare the ORDER SEQUENCE across runs (the runner prints no passing
# names, and comparing stdout would only see the seed banner). Exits 0.
mk_order_test() { # <name>
  {
    printf '#!/usr/bin/env bash\n'
    printf 'printf '\''%%s\\n'\'' %q >> "$ORDER_LOG"\n' "$1"
    printf 'exit 0\n'
  } > "$scratch/test/unit/$1.sh"
  chmod +x "$scratch/test/unit/$1.sh"
}

@test "all-pass camp exits 0" {
  mk_test a 0
  mk_test b 0
  run "$scratch/test/run-unit-tests.sh"
  [ "$status" -eq 0 ]
}

@test "a failing unit test fails the runner" {
  mk_test a 0
  mk_test zz-fail 1
  run "$scratch/test/run-unit-tests.sh"
  [ "$status" -eq 1 ]
  [[ "$output" == *"FAIL: "*zz-fail* ]]
}

@test "empty camp is a green no-op" {
  run "$scratch/test/run-unit-tests.sh"
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
  PATH="$scratch/bin:$PATH" run "$scratch/test/run-unit-tests.sh"
  [ "$status" -eq 1 ]
  [[ "$output" == *"discovery failed"* ]]
}

@test "sort failure refuses to green-gate" {
  mk_test a 0
  # Shim `sort` to exit nonzero, modeling a sort that fails mid-discovery; the
  # runner must refuse rather than trust the list.
  mkdir -p "$scratch/bin"
  cat > "$scratch/bin/sort" <<SHIM
#!/usr/bin/env bash
/usr/bin/sort "\$@"
exit 7
SHIM
  chmod +x "$scratch/bin/sort"
  PATH="$scratch/bin:$PATH" run "$scratch/test/run-unit-tests.sh"
  [ "$status" -eq 1 ]
  [[ "$output" == *"sort failed"* ]]
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

  # (1) reproducibility under a fixed seed
  export ORDER_LOG="$scratch/order.a1"; : > "$ORDER_LOG"
  TEST_SEED=1 run "$scratch/test/run-unit-tests.sh"
  [ "$status" -eq 0 ]
  local a1; a1="$(cat "$ORDER_LOG")"
  export ORDER_LOG="$scratch/order.a2"; : > "$ORDER_LOG"
  TEST_SEED=1 run "$scratch/test/run-unit-tests.sh"
  [ "$status" -eq 0 ]
  local a2; a2="$(cat "$ORDER_LOG")"
  [ "$a1" = "$a2" ]

  # (2) the shuffle actually reorders relative to sorted, for at least one seed
  local s reordered=0 order
  for s in 1 2 3 4 5; do
    export ORDER_LOG="$scratch/order.s$s"; : > "$ORDER_LOG"
    TEST_SEED="$s" run "$scratch/test/run-unit-tests.sh"
    [ "$status" -eq 0 ]
    order="$(cat "$ORDER_LOG")"
    [ "$order" != "$sorted" ] && reordered=1
  done
  [ "$reordered" -eq 1 ]
}
