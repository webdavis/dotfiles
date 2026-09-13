#!/usr/bin/env bash
# shellcheck disable=SC2016

set_up_before_script() {
  local root
  root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  render_dir="$(mktemp -d)"
  builder="$render_dir/builder.sh"
  HOME="$render_dir" CI=1 chezmoi --source "$root" execute-template --no-tty \
    <"$root/.chezmoiscripts/run_onchange_after_58-build-pns-engine.sh.tmpl" >"$builder" 2>/dev/null
  [[ -s $builder ]] || return 1
  sed -i '' 's|^crate_dir=.*|crate_dir="$HOME/crate"|' "$builder"
  sed -i '' 's|^install_dir=.*|install_dir="$HOME/.cargo/bin"|' "$builder"
  grep -q '^crate_dir="\$HOME/crate"$' "$builder" || return 1
  grep -q '^install_dir="\$HOME/.cargo/bin"$' "$builder"
}
set_up() {
  sandbox="$(mktemp -d)"
  fixture_home="$sandbox/home"
  source_dir="$sandbox/source"
  record="$fixture_home/.local/state/pns-build-record"
  binary="$fixture_home/.cargo/bin/pns"
  mkdir -p "$fixture_home/.cargo/bin" "$fixture_home/crate/target/release" "$source_dir/.chezmoiscripts" "$sandbox/bin"
  : >"$sandbox/calls"
  printf old >"$binary"
  printf new >"$fixture_home/crate/target/release/pns"
  cat >"$fixture_home/.cargo/bin/cargo" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
[[ $* == 'build --release --locked --quiet --bin pns' ]]
STUB
  cat >"$fixture_home/.cargo/bin/rustc" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
[[ $* == '--version --verbose' ]]
printf 'rustc fixture\nhost: fixture\n'
STUB
  cat >"$source_dir/.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
[[ $# == 1 && $1 == --pipeline-only ]]
cp "$HOME/.local/state/pns-build-record" "$TEST_ROOT/record-at-refresh"
cp "$HOME/.cargo/bin/pns" "$TEST_ROOT/binary-at-refresh"
printf 'refresh\n' >>"$TEST_ROOT/calls"
[[ ! -f "$TEST_ROOT/fail-refresh" ]] || exit 25
cp "$HOME/.local/state/pns-build-record" "$TEST_ROOT/manifest-record"
STUB
  cat >"$sandbox/bin/launchctl" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
[[ $# == 3 && $1 == kickstart && $2 == -k && $3 == "gui/$(id -u)/com.webdavis.pns-daemon" ]]
printf 'restart\n' >>"$TEST_ROOT/calls"
STUB
  chmod +x "$fixture_home/.cargo/bin/cargo" "$fixture_home/.cargo/bin/rustc" "$sandbox/bin/launchctl"
}
run_builder() {
  HOME="$fixture_home" CHEZMOI_SOURCE_DIR="$source_dir" TEST_ROOT="$sandbox" PATH="$sandbox/bin:$PATH" \
    bash "$builder" >"$sandbox/stdout" 2>"$sandbox/stderr"
}
seed_record() {
  mkdir -p "$(dirname "$record")"
  printf 'sha256 %s\nbytes 3\nrustc fixture\nhost: fixture\n' \
    "$(printf '%s' "$1" | shasum -a 256 | awk '{print $1}')" >"$record"
  chmod 600 "$record"
}
function test_authorized_pns_record_and_manifest_precede_binary_installation() {
  local status=0
  run_builder || status=$?
  assert_same 0 "$status"
  assert_file_exists "$record"
  assert_same old "$(cat "$sandbox/binary-at-refresh" 2>/dev/null)"
  assert_same new "$(cat "$binary")"
  assert_contains "sha256 $(printf new | shasum -a 256 | awk '{print $1}')" "$(cat "$record" 2>/dev/null)"
  assert_contains 'bytes 3' "$(cat "$record" 2>/dev/null)"
  assert_same 600 "$(stat -f '%Lp' "$record" 2>/dev/null)"
  assert_same $'refresh\nrestart' "$(cat "$sandbox/calls")"
}
function test_failed_pns_refresh_preserves_prior_record_and_binary() {
  seed_record old
  local previous
  previous="$(cat "$record")"
  touch "$sandbox/fail-refresh"
  local status=0
  run_builder || status=$?
  assert_not_same 0 "$status"
  assert_same "$previous" "$(cat "$record" 2>/dev/null)"
  assert_same old "$(cat "$binary")"
}
function test_failed_first_pns_refresh_restores_absent_record() {
  touch "$sandbox/fail-refresh"
  local status=0
  run_builder || status=$?
  assert_not_same 0 "$status"
  assert_file_not_exists "$record"
  assert_same old "$(cat "$binary")"
}
function test_identical_pns_build_keeps_the_record_and_skips_refresh_and_restart() {
  seed_record new
  printf new >"$binary"
  local inode
  inode="$(stat -f '%i' "$record" 2>/dev/null)"
  run_builder
  assert_same "$inode" "$(stat -f '%i' "$record" 2>/dev/null)"
  assert_same '' "$(cat "$sandbox/calls")"
}
function test_invalid_pns_artifact_size_cannot_publish_trusted_state() {
  local size status
  for size in 0 8388609; do
    head -c "$size" /dev/zero >"$fixture_home/crate/target/release/pns"
    status=0
    run_builder || status=$?
    assert_not_same 0 "$status"
    assert_file_not_exists "$record"
    assert_same old "$(cat "$binary")"
  done
}
function test_failed_pns_install_retains_the_published_record() {
  chmod 500 "$fixture_home/.cargo/bin"
  local status=0
  run_builder || status=$?
  chmod 700 "$fixture_home/.cargo/bin"
  assert_not_same 0 "$status"
  assert_same old "$(cat "$binary")"
  assert_contains "sha256 $(printf new | shasum -a 256 | awk '{print $1}')" "$(cat "$record" 2>/dev/null)"
}

function test_pns_install_retries_after_publication_left_the_old_binary() {
  seed_record new
  cp "$record" "$sandbox/manifest-record"
  mkdir -p "$fixture_home/.cache/pns-build"
  touch "$fixture_home/.cache/pns-build/restart-pending"
  local status=0
  run_builder || status=$?
  assert_same 0 "$status"
  assert_same new "$(cat "$binary")"
  assert_same $'refresh\nrestart' "$(cat "$sandbox/calls")"
}

function test_symlinked_pns_build_record_is_refused_before_reading_or_publication() {
  mkdir -p "$(dirname "$record")"
  printf retained >"$sandbox/referent"
  ln -s "$sandbox/referent" "$record"
  local status=0
  run_builder || status=$?
  assert_not_same 0 "$status"
  assert_same retained "$(cat "$sandbox/referent")"
  assert_same old "$(cat "$binary")"
  assert_file_not_exists "$sandbox/record-at-refresh"
}
