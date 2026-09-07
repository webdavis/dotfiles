#!/usr/bin/env bash
# The apply-time build script is what puts the posture binary on the machine
# and what publishes the known-good tuple the file-integrity arm vouches for.
# Its behaviors are load-bearing and none is visible anywhere else: install the
# binary WHERE THE AGENTS WILL LOOK, leave the run_onchange trigger retryable
# when the build could not happen, publish the build record and refresh the
# manifests BEFORE the bytes land so the alerter never sees a change the
# manifest predates, restore the record when the refresh fails, and refuse an
# artifact the manifest audit could not hash.
#
# The script resolves cargo, rustc and the manifest runner at fixed paths under
# $HOME and $CHEZMOI_SOURCE_DIR, so a sandboxed HOME with stubs runs the real
# rendered script end to end. Nothing here reaches the live machine.
#
# bashunit SOURCES this file, so it carries no `set -euo pipefail` and no
# executable bit; test/validate-tests.sh pins the shape. assert_same, never
# assert_equals: the latter normalizes control characters away (0.50.1).

repo_root() {
  printf '%s' "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
}

# Rendered ONCE for the whole file: the render is the slow step, and every
# test drives the same script against its own sandbox.
set_up_before_script() {
  render_dir="$(mktemp -d)"
  rendered_builder="$render_dir/build-posture.sh"
  HOME="$render_dir" CI=1 chezmoi --source "$(repo_root)" execute-template --no-tty \
    <"$(repo_root)/.chezmoiscripts/run_onchange_after_58-build-posture.sh.tmpl" \
    >"$rendered_builder" 2>/dev/null
  [[ -s $rendered_builder ]] || {
    echo "the build script rendered empty" >&2
    return 1
  }
  chmod +x "$rendered_builder"
  # Exercise the builder's trigger without rehashing every build input per case.
  retry_template="$render_dir/retry-trigger.tmpl"
  : >"$render_dir/chezmoi.toml"
  sed -n '/^#   retry-marker: /p' \
    "$(repo_root)/.chezmoiscripts/run_onchange_after_58-build-posture.sh.tmpl" >"$retry_template"
  [[ -s $retry_template ]]
}

# Sandboxes stay under the system temporary directory for failure inspection.

# One sandbox per test: a HOME with a stub toolchain and the deployed crate
# sources, and a chezmoi SOURCE dir holding a stub manifest runner. The stubs
# record every invocation so a test asserts order and arguments, not effects.
set_up() {
  sandbox="$(mktemp -d)"
  sandbox_home="$sandbox/home"
  sandbox_source="$sandbox/source"
  installed_binary="$sandbox_home/.local/libexec/posture/posture"
  retry_marker="$sandbox_home/.cache/posture-build/posture.retry"
  build_record="$sandbox_home/.local/state/posture-build-record"
  cargo_args="$sandbox/cargo.args"
  runner_calls="$sandbox/runner.calls"
  runner_status="$sandbox/runner.status"
  builder_stdout="$sandbox/stdout"
  builder_stderr="$sandbox/stderr"
  mkdir -p "$sandbox_home" "$sandbox_source/.chezmoiscripts"
  : >"$cargo_args"
  : >"$runner_calls"
  write_stub_runner
}

tear_down() {
  chmod -R u+w "$sandbox" 2>/dev/null || true
}

install_stub_toolchain() {
  mkdir -p "$sandbox_home/.cargo/bin"
  # Stand-in for the real build: honors --manifest-path, refuses a call without
  # --locked or without --bin posture, and produces the artifact the script
  # installs. The artifact's bytes come from $HOME/.stub-artifact when present,
  # or a zero-filled file of $HOME/.stub-artifact-bytes bytes, so a test can
  # change the bytes or the size between two runs.
  cat >"$sandbox_home/.cargo/bin/cargo" <<STUB
#!/usr/bin/env bash
printf '%s\n' "\$*" >>"$cargo_args"
[[ ! -f "\$HOME/.stub-cargo-failure" ]] || exit 25
manifest=""
locked=0
bin=""
while [[ \$# -gt 0 ]]; do
  [[ \$1 == --manifest-path ]] && manifest="\$2"
  [[ \$1 == --locked ]] && locked=1
  [[ \$1 == --bin ]] && bin="\$2"
  shift
done
[[ \$locked -eq 1 ]] || { echo "cargo was invoked without --locked" >&2; exit 1; }
[[ \$bin == posture ]] || { echo "cargo was invoked without --bin posture" >&2; exit 1; }
crate="\$(dirname "\$manifest")"
mkdir -p "\$crate/target/release"
if [[ -f "\$HOME/.stub-artifact-bytes" ]]; then
  head -c "\$(cat "\$HOME/.stub-artifact-bytes")" /dev/zero >"\$crate/target/release/posture"
elif [[ -f "\$HOME/.stub-artifact" ]]; then
  cp "\$HOME/.stub-artifact" "\$crate/target/release/posture"
else
  printf '#!/usr/bin/env bash\nprintf posture-build-1\n' >"\$crate/target/release/posture"
fi
chmod +x "\$crate/target/release/posture"
STUB
  cat >"$sandbox_home/.cargo/bin/rustc" <<'STUB'
#!/usr/bin/env bash
printf 'rustc 1.92.0-nightly (stub)\nhost: aarch64-apple-darwin\n'
STUB
  chmod +x "$sandbox_home/.cargo/bin/cargo" "$sandbox_home/.cargo/bin/rustc"
}

install_deployed_sources() {
  mkdir -p "$sandbox_home/.local/share/posture" "$sandbox_home/.local/share/pns/crates/pns-protocol"
  printf '[workspace]\n' >"$sandbox_home/.local/share/posture/Cargo.toml"
  printf '[package]\nname = "pns-protocol"\n' >"$sandbox_home/.local/share/pns/crates/pns-protocol/Cargo.toml"
}

# The stub manifest runner records what the world looked like WHEN IT RAN: the
# record's sha256 line (or "no-record") and the installed binary's bytes (or
# "no-binary"), one line per call, and exits with runner.status (default 0).
write_stub_runner() {
  cat >"$sandbox_source/.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh" <<STUB
#!/usr/bin/env bash
set -euo pipefail
[[ \$# -eq 1 && \$1 == --pipeline-only ]] || exit 26
cp "$build_record" "$sandbox/record-at-refresh"
if [[ -f "$installed_binary" ]]; then
  shasum -a 256 "$installed_binary" | awk '{print \$1}' >"$sandbox/binary-at-refresh"
else
  printf absent >"$sandbox/binary-at-refresh"
fi
printf 'refresh\n' >>"$runner_calls"
[[ ! -f "$runner_status" ]] || exit "\$(cat "$runner_status")"
STUB
  chmod +x "$sandbox_source/.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh"
}

run_builder() {
  HOME="$sandbox_home" CHEZMOI_SOURCE_DIR="$sandbox_source" \
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null "$rendered_builder" >"$builder_stdout" 2>"$builder_stderr"
}

# bashunit has no bare `fail`, so the two exit-status expectations are
# assertions of their own: the apply-time contract is "exit 0 or the apply
# fails", and every test states which side it expects.
assert_builder_succeeds() {
  local status=0
  run_builder || status=$?
  assert_same 0 "$status"
}

assert_builder_fails() {
  local status=0
  run_builder || status=$?
  assert_not_same 0 "$status"
}

ready_to_build() {
  install_stub_toolchain
  install_deployed_sources
}

# --- deferral: a missing build input never fails the apply and leaves the ---
# --- trigger retryable ----------------------------------------------------

function test_a_missing_toolchain_defers_the_build_and_leaves_the_trigger_retryable() {
  install_deployed_sources
  assert_builder_succeeds
  assert_file_not_exists "$installed_binary"
  assert_file_exists "$retry_marker"
  local first_attempt
  first_attempt="$(wc -c <"$retry_marker")"
  assert_builder_succeeds
  assert_greater_than "$first_attempt" "$(wc -c <"$retry_marker")"
}

function test_same_second_deferrals_change_the_rendered_retry_trigger() {
  install_stub_toolchain
  local first_trigger second_trigger
  assert_builder_succeeds
  touch -t 202601010000.00 "$retry_marker"
  first_trigger="$(HOME="$sandbox_home" chezmoi --source "$render_dir" --config "$render_dir/chezmoi.toml" \
    execute-template --no-tty <"$retry_template")" || return 1
  assert_builder_succeeds
  touch -t 202601010000.00 "$retry_marker"
  second_trigger="$(HOME="$sandbox_home" chezmoi --source "$render_dir" --config "$render_dir/chezmoi.toml" \
    execute-template --no-tty <"$retry_template")" || return 1
  assert_not_same "$first_trigger" "$second_trigger"
}

function test_a_toolchain_without_the_deployed_crate_defers_the_build() {
  install_stub_toolchain
  mkdir -p "$sandbox_home/.local/share/pns/crates/pns-protocol"
  printf '[package]\nname = "pns-protocol"\n' >"$sandbox_home/.local/share/pns/crates/pns-protocol/Cargo.toml"
  assert_builder_succeeds
  assert_file_not_exists "$installed_binary"
  assert_file_exists "$retry_marker"
}

function test_a_crate_without_the_sibling_pns_protocol_source_defers_the_build() {
  install_stub_toolchain
  mkdir -p "$sandbox_home/.local/share/posture"
  printf '[workspace]\n' >"$sandbox_home/.local/share/posture/Cargo.toml"
  assert_builder_succeeds
  assert_file_not_exists "$installed_binary"
  assert_file_exists "$retry_marker"
}

# --- install: the binary lands where the agents will look, built from the --
# --- committed lock, and the trigger settles --------------------------------

function test_a_successful_build_installs_the_binary_where_the_agents_will_run_it_and_settles_the_trigger() {
  install_deployed_sources
  assert_builder_succeeds
  assert_file_exists "$retry_marker"
  install_stub_toolchain
  assert_builder_succeeds
  assert_file_exists "$installed_binary"
  assert_same posture-build-1 "$("$installed_binary")"
  assert_file_not_exists "$retry_marker"
}

function test_the_build_runs_from_the_committed_lock_and_names_the_one_binary() {
  ready_to_build
  assert_builder_succeeds
  assert_same 1 "$(wc -l <"$cargo_args" | tr -d ' ')"
  assert_contains ' --locked ' " $(cat "$cargo_args") "
  assert_contains ' --release ' " $(cat "$cargo_args") "
  assert_contains ' --bin posture ' " $(cat "$cargo_args") "
  assert_contains "--manifest-path $sandbox_home/.local/share/posture/Cargo.toml" "$(cat "$cargo_args")"
}

function test_a_build_publishes_a_private_record_before_refresh_and_install() {
  ready_to_build
  assert_builder_succeeds
  local artifact="$sandbox_home/.local/share/posture/target/release/posture"
  assert_contains "sha256 $(shasum -a 256 "$artifact" | awk '{print $1}')" "$(cat "$build_record")"
  assert_contains "bytes $(wc -c <"$artifact" | tr -d ' ')" "$(cat "$build_record")"
  assert_contains 'rustc 1.92.0-nightly (stub)' "$(cat "$build_record")"
  assert_same 600 "$(stat -f '%Lp' "$build_record")"
  assert_same "$(cat "$build_record")" "$(cat "$sandbox/record-at-refresh")"
  assert_same absent "$(cat "$sandbox/binary-at-refresh")"
}

function test_a_failed_refresh_restores_the_previous_record_and_binary() {
  ready_to_build
  assert_builder_succeeds
  cp "$build_record" "$sandbox/previous-record"
  printf 'new artifact' >"$sandbox_home/.stub-artifact"
  printf 1 >"$runner_status"
  assert_builder_fails
  assert_same "$(cat "$sandbox/previous-record")" "$(cat "$build_record")"
  assert_same posture-build-1 "$("$installed_binary")"
}

function test_a_failed_first_refresh_restores_the_absent_record() {
  ready_to_build
  printf 1 >"$runner_status"
  assert_builder_fails
  assert_file_not_exists "$build_record"
  assert_file_not_exists "$installed_binary"
}

function test_an_identical_build_retains_the_record_and_skips_refresh() {
  ready_to_build
  assert_builder_succeeds
  cp "$build_record" "$sandbox/previous-record"
  local inode
  inode="$(stat -f '%i' "$build_record")"
  assert_builder_succeeds
  assert_same "$inode" "$(stat -f '%i' "$build_record")"
  assert_same "$(cat "$sandbox/previous-record")" "$(cat "$build_record")"
  assert_same 1 "$(wc -l <"$runner_calls" | tr -d ' ')"
}

function test_an_identical_build_repairs_binary_and_record_permissions() {
  ready_to_build
  assert_builder_succeeds
  chmod 644 "$installed_binary" "$build_record"
  assert_builder_succeeds
  assert_same 755 "$(stat -f '%Lp' "$installed_binary")"
  assert_same 600 "$(stat -f '%Lp' "$build_record")"
  assert_same 1 "$(wc -l <"$runner_calls" | tr -d ' ')"
}

function test_an_artifact_at_the_audit_size_limit_can_be_published() {
  ready_to_build
  printf 8388608 >"$sandbox_home/.stub-artifact-bytes"
  assert_builder_succeeds
  assert_contains 'bytes 8388608' "$(cat "$build_record")"
  assert_same 8388608 "$(wc -c <"$installed_binary" | tr -d ' ')"
}

function test_an_artifact_over_the_audit_size_limit_never_replaces_trusted_state() {
  ready_to_build
  assert_builder_succeeds
  cp "$build_record" "$sandbox/previous-record"
  printf 8388609 >"$sandbox_home/.stub-artifact-bytes"
  assert_builder_fails
  assert_same "$(cat "$sandbox/previous-record")" "$(cat "$build_record")"
  assert_same posture-build-1 "$("$installed_binary")"
  assert_same 1 "$(wc -l <"$runner_calls" | tr -d ' ')"
}

function test_a_failed_build_never_refreshes_or_replaces_trusted_state() {
  ready_to_build
  assert_builder_succeeds
  cp "$build_record" "$sandbox/previous-record"
  touch "$sandbox_home/.stub-cargo-failure"
  assert_builder_fails
  assert_same "$(cat "$sandbox/previous-record")" "$(cat "$build_record")"
  assert_same posture-build-1 "$("$installed_binary")"
  assert_same 1 "$(wc -l <"$runner_calls" | tr -d ' ')"
}

function test_a_failed_install_keeps_the_new_record_for_a_retry() {
  ready_to_build
  assert_builder_succeeds
  printf 'new artifact' >"$sandbox_home/.stub-artifact"
  chmod 500 "$(dirname "$installed_binary")"
  assert_builder_fails
  assert_same posture-build-1 "$("$installed_binary")"
  assert_contains "sha256 $(printf 'new artifact' | shasum -a 256 | awk '{print $1}')" "$(cat "$build_record")"
  chmod 700 "$(dirname "$installed_binary")"
  assert_builder_succeeds
  assert_same 'new artifact' "$(cat "$installed_binary")"
}

function test_an_empty_artifact_never_replaces_trusted_state() {
  ready_to_build
  assert_builder_succeeds
  cp "$build_record" "$sandbox/previous-record"
  cp "$installed_binary" "$sandbox/previous-binary"
  printf 0 >"$sandbox_home/.stub-artifact-bytes"
  assert_builder_fails
  cmp -s "$sandbox/previous-record" "$build_record"
  assert_successful_code
  cmp -s "$sandbox/previous-binary" "$installed_binary"
  assert_successful_code
  assert_same 1 "$(wc -l <"$runner_calls" | tr -d ' ')"
}
