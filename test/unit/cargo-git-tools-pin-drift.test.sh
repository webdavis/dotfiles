#!/usr/bin/env bash
# The pin-drift check in run_onchange_after_57-install-cargo-git-tools.sh.tmpl
# is the whole feature: a pattern that never matches reinstalls (and
# re-fetches) on every apply, one that always matches means a `ref` bump never
# lands and dam stays stale forever, and a stale record surviving a removed
# binary means it is never reinstalled either. All three failures are
# invisible in an apply, so already_installed's own contract is pinned here
# directly against .crates.toml and the binary, no cargo spawn needed.
#
# The template is rendered once with HOME pointed at a fixture directory
# (install_root bakes .chezmoi.homeDir at RENDER time, the same convention
# every sibling Rust builder uses), then trimmed to everything up to and
# including the already_installed function, so sourcing it never reaches the
# roster-driven install loop.

set_up_before_script() {
  local root full
  root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  fixture_home="$(mktemp -d)"
  # Satisfies the cargo-present guard the trimmed snippet still carries.
  mkdir -p "$fixture_home/.cargo/bin"
  touch "$fixture_home/.cargo/bin/cargo"
  chmod +x "$fixture_home/.cargo/bin/cargo"
  full="$fixture_home/full.sh"
  HOME="$fixture_home" CI=1 chezmoi --source "$root" execute-template --no-tty \
    <"$root/.chezmoiscripts/run_onchange_after_57-install-cargo-git-tools.sh.tmpl" \
    >"$full" 2>/dev/null
  [[ -s $full ]] || return 1
  snippet="$fixture_home/snippet.sh"
  awk '
    /^already_installed\(\)/ { in_fn = 1 }
    { print }
    in_fn && /^}$/ { exit }
  ' "$full" >"$snippet"
  grep -q '^already_installed() {$' "$snippet"
  # SC2016: the literal grep pattern, not a shell expansion.
  # shellcheck disable=SC2016
  ! grep -q '"$cargo_bin" install' "$snippet"
}

# Runs already_installed <repo> <rev> <name> as a fresh process (never
# `source`, so a `set -e` in the snippet cannot terminate the test runner) and
# returns its exit code as this function's own.
run_already_installed() {
  local driver="$fixture_home/driver.sh"
  cp "$snippet" "$driver"
  printf 'already_installed %q %q %q\n' "$1" "$2" "$3" >>"$driver"
  HOME="$fixture_home" bash "$driver"
}

record_pinned_crates_toml() {
  cat >"$fixture_home/.cargo/.crates.toml" <<'EOF'
[v1]
"damnit 0.2.0 (git+https://github.com/webdavis/damnit?rev=dfe31427f7bc41a289aa0b48f9c3a75eb7689a0a#dfe31427f7bc41a289aa0b48f9c3a75eb7689a0a)" = ["dam"]
EOF
}

function test_a_crates_toml_line_at_the_pinned_rev_with_the_binary_present_is_already_installed() {
  touch "$fixture_home/.cargo/bin/dam"
  chmod +x "$fixture_home/.cargo/bin/dam"
  record_pinned_crates_toml
  run_already_installed "webdavis/damnit" "dfe31427f7bc41a289aa0b48f9c3a75eb7689a0a" "dam"
  assert_successful_code
}

function test_the_same_repo_at_a_different_rev_is_not_already_installed() {
  touch "$fixture_home/.cargo/bin/dam"
  chmod +x "$fixture_home/.cargo/bin/dam"
  record_pinned_crates_toml
  run_already_installed "webdavis/damnit" "0000000000000000000000000000000000000000" "dam"
  assert_general_error
}

function test_a_missing_crates_toml_is_not_already_installed() {
  touch "$fixture_home/.cargo/bin/dam"
  chmod +x "$fixture_home/.cargo/bin/dam"
  rm -f "$fixture_home/.cargo/.crates.toml"
  run_already_installed "webdavis/damnit" "dfe31427f7bc41a289aa0b48f9c3a75eb7689a0a" "dam"
  assert_general_error
}

# A hand rm or a partial restore can remove the binary while the record
# survives; without this the next apply would silently never reinstall it.
function test_a_matching_record_with_the_binary_missing_is_not_already_installed() {
  rm -f "$fixture_home/.cargo/bin/dam"
  record_pinned_crates_toml
  run_already_installed "webdavis/damnit" "dfe31427f7bc41a289aa0b48f9c3a75eb7689a0a" "dam"
  assert_general_error
}
