#!/usr/bin/env bash
# The builder refreshes only its governing manifest. The legacy two-manifest
# refresh can publish the first and then fail on the second.

set_up_before_script() {
  local root
  root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  render_dir="$(mktemp -d)"
  rendered_builder="$render_dir/build-posture.sh"
  HOME="$render_dir" CI=1 chezmoi --source "$root" execute-template --no-tty \
    <"$root/.chezmoiscripts/run_onchange_after_58-build-posture.sh.tmpl" \
    >"$rendered_builder" 2>/dev/null
  [[ -s $rendered_builder ]] || return 1
  # The monorepo move bakes the real checkout's absolute path into crate_dir at
  # render time. The builder reads its artifact from under that directory, so an
  # unredirected run would look in the working tree instead of the sandbox the
  # test staged. Point the rendered copy at a per-sandbox directory under HOME.
  sed -i '' 's|^crate_dir=.*|crate_dir="$HOME/crate"|' "$rendered_builder"
  grep -q '^crate_dir="\$HOME/crate"$' "$rendered_builder"
}

set_up() {
  sandbox="$(mktemp -d)"
  stubbin="$sandbox/bin"
  sandbox_home="$sandbox/home with spaces"
  build_record="$sandbox_home/.local/state/posture-build-record"
  binary="$sandbox_home/.local/libexec/posture/posture"
  artifact="$sandbox_home/crate/target/release/posture"
  mkdir -p "$stubbin" "$(dirname "$build_record")" "$(dirname "$binary")" "$(dirname "$artifact")"
  pipeline_manifest="$sandbox/pipeline"
  bin_manifest="$sandbox/managed-bin"
  printf old-pipeline >"$pipeline_manifest"
  printf old-bin >"$bin_manifest"
  cat >"$stubbin/chezmoi" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
while [[ $# -gt 0 ]]; do
  case "$1" in
    managed)
      printf '%s\n' "$HOME/.local/libexec/osquery/example.sh" "$HOME/.local/bin/example"
      exit 0 ;;
    dump)
      printf '{".local/libexec/osquery/example.sh":{"perm":493},".local/bin/example":{"perm":493}}'
      exit 0 ;;
    cat)
      shift
      if [[ "$1" == "$HOME/.local/bin/example" && -f "$TEST_ROOT/fail-bin" ]]; then exit 23; fi
      printf 'intended bytes\n'
      exit 0 ;;
  esac
  shift
done
exit 2
STUB
  cat >"$stubbin/sudo" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
[[ "$1" == install ]] || exit 99
[[ ! -f "$TEST_ROOT/fail-install" ]] || exit 24
source_file="${@: -2:1}"
destination="${@: -1}"
[[ "$destination" == "$TEST_ROOT/pipeline" || "$destination" == "$TEST_ROOT/managed-bin" ]] || exit 98
if [[ "$destination" == "$TEST_ROOT/pipeline" && -f "$HOME/.local/libexec/posture/posture" ]]; then
  cp "$HOME/.local/libexec/posture/posture" "$TEST_ROOT/binary-at-publication"
fi
cp "$source_file" "$destination"
STUB
  chmod +x "$stubbin/chezmoi" "$stubbin/sudo"
}

run_refresh() {
  local root
  root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  run_in_sandbox bash "$root/.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh" "$@"
}

run_in_sandbox() {
  HOME="$sandbox_home" CHEZMOI_SOURCE_DIR="$sandbox/source" TEST_ROOT="$sandbox" \
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null \
    OSQUERY_PIPELINE_MANIFEST="$pipeline_manifest" OSQUERY_MANAGED_BIN_MANIFEST="$bin_manifest" \
    PATH="$stubbin:$PATH" "$@" >"$sandbox/stdout" 2>"$sandbox/stderr"
}

function test_pipeline_only_refresh_never_fails_after_publishing_due_to_the_other_manifest() {
  touch "$sandbox/fail-bin"
  local status=0
  run_refresh --pipeline-only || status=$?
  assert_same 0 "$status"
  assert_contains '0755' "$(cat "$pipeline_manifest")"
  assert_same old-bin "$(cat "$bin_manifest")"
}

function test_a_failed_pipeline_install_preserves_the_previous_tuple() {
  touch "$sandbox/fail-install"
  local status=0
  run_refresh --pipeline-only || status=$?
  assert_not_same 0 "$status"
  assert_same old-pipeline "$(cat "$pipeline_manifest")"
  assert_same old-bin "$(cat "$bin_manifest")"
}

function test_the_default_refresh_still_updates_both_manifests() {
  local status=0
  run_refresh || status=$?
  assert_same 0 "$status"
  assert_contains '0755' "$(cat "$pipeline_manifest")"
  assert_contains '0755' "$(cat "$bin_manifest")"
}

function test_an_unknown_refresh_scope_refuses_to_publish() {
  local status=0
  run_refresh --unrecognized || status=$?
  assert_same 2 "$status"
  assert_same old-pipeline "$(cat "$pipeline_manifest")"
  assert_same old-bin "$(cat "$bin_manifest")"
}

write_record() {
  printf 'sha256 %s\nbytes %s\nrustc 1.92.0-nightly (stub)\nhost: aarch64-apple-darwin\n' \
    "${1:-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa}" "${2:-3}" >"$build_record"
  chmod 600 "$build_record"
}

function test_the_binary_tuple_comes_from_the_build_record_without_a_deployed_binary() {
  write_record
  printf 'out-of-band build' >"$artifact"
  run_refresh --pipeline-only
  assert_successful_code
  assert_contains "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 0755 $(id -u) $binary" \
    "$(cat "$pipeline_manifest")"
  assert_file_not_exists "$binary"
}

function test_refresh_without_a_build_retains_the_tuple_despite_tampered_live_and_target_bytes() {
  write_record
  run_refresh --pipeline-only
  local previous
  previous="$(cat "$pipeline_manifest")"
  printf 'tampered binary' >"$binary"
  printf 'unauthorized build' >"$artifact"
  run_refresh --pipeline-only
  assert_successful_code
  assert_contains "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 0755 $(id -u) $binary" "$previous"
  assert_same "$previous" "$(cat "$pipeline_manifest")"
}

function test_a_missing_record_enumerates_an_unbuilt_binary_without_adopting_any_bytes() {
  printf 'unrecorded binary' >"$binary"
  printf 'unrecorded artifact' >"$artifact"
  run_refresh --pipeline-only
  assert_successful_code
  assert_contains "unbuilt 0755 $(id -u) $binary" "$(cat "$pipeline_manifest")"
}

function test_a_malformed_record_digest_preserves_both_manifests() {
  local digest status
  for digest in not-a-digest \
    aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa \
    aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa; do
    write_record "$digest"
    status=0
    run_refresh || status=$?
    assert_not_same 0 "$status"
    assert_same old-pipeline "$(cat "$pipeline_manifest")"
    assert_same old-bin "$(cat "$bin_manifest")"
  done
}

function test_a_record_with_an_invalid_artifact_size_refuses_pipeline_publication() {
  local bytes status
  for bytes in 0 8388609 invalid; do
    write_record aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa "$bytes"
    status=0
    run_refresh --pipeline-only || status=$?
    assert_not_same 0 "$status"
    assert_same old-pipeline "$(cat "$pipeline_manifest")"
  done
}

function test_a_truncated_record_is_refused_instead_of_treated_as_unbuilt() {
  printf 'sha256 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n' >"$build_record"
  local status=0
  run_refresh --pipeline-only || status=$?
  assert_not_same 0 "$status"
  assert_same old-pipeline "$(cat "$pipeline_manifest")"
}

function test_a_nonregular_record_is_refused_without_opening_it() {
  mkfifo "$build_record"
  local status=0
  run_refresh --pipeline-only || status=$?
  assert_not_same 0 "$status"
  assert_same old-pipeline "$(cat "$pipeline_manifest")"
}

run_consumer() {
  local root
  root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  HOME="$sandbox_home" OSQUERY_PIPELINE_MANIFEST="$pipeline_manifest" \
    OSQUERY_MANAGED_BIN_MANIFEST="$bin_manifest" OSQUERY_PIPELINE_SETTLE_SECONDS=0 \
    bash -c 'set -euo pipefail; source "$1/results-alerter/pipeline-verdict.sh";
      source "$1/executable_pipeline-audit.sh"; shift; "$@"' _ \
    "$root/dot_local/libexec/osquery" "$@"
}

write_unbuilt_tuple() {
  printf 'unbuilt 0755 %s %s\n' "$(id -u)" "$binary" >"$pipeline_manifest"
}

assert_audit_finding() {
  local report status=0
  report="$(run_consumer _pipeline_audit_scan_manifest "$pipeline_manifest" "$((EPOCHSECONDS + 60))" 500 8388608)" || status=$?
  assert_same 0 "$status"
  assert_same "$1 $binary" "$report"
}

function test_the_audit_reports_a_missing_unbuilt_binary() {
  write_unbuilt_tuple
  assert_audit_finding missing
}

function test_the_audit_refuses_an_empty_unbuilt_binary_with_matching_mode_and_owner() {
  write_unbuilt_tuple
  : >"$binary"
  chmod 755 "$binary"
  assert_audit_finding content
}

function test_the_audit_refuses_nonempty_unbuilt_content() {
  write_unbuilt_tuple
  printf abc >"$binary"
  chmod 755 "$binary"
  assert_audit_finding content
}

function test_an_unbuilt_symlink_keeps_the_irregular_file_refusal() {
  write_unbuilt_tuple
  printf abc >"$artifact"
  ln -s "$artifact" "$binary"
  assert_audit_finding irregular
}

function test_a_forged_unbuilt_digest_never_matches_the_tuple() {
  binary="$sandbox_home/required-binary"
  write_unbuilt_tuple
  local status=0
  run_consumer _pipeline_manifest_has_tuple "$binary" unbuilt 0755 "$(id -u)" || status=$?
  assert_same 1 "$status"
}

function test_an_ordinary_digest_tuple_still_vouches_for_its_exact_content() {
  binary="$sandbox_home/known-good"
  printf abc >"$binary"
  chmod 755 "$binary"
  printf 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad 0755 %s %s\n' \
    "$(id -u)" "$binary" >"$pipeline_manifest"
  local report status=0
  report="$(run_consumer _pipeline_audit_scan_manifest "$pipeline_manifest" "$((EPOCHSECONDS + 60))" 500 8388608)" || status=$?
  assert_same 0 "$status"
  assert_empty "$report"
  run_consumer _pipeline_deployed_state_is_known_good "$binary"
  assert_successful_code
}

prepare_builder() {
  local root
  root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  mkdir -p "$sandbox/source/.chezmoiscripts" "$sandbox_home/.cargo/bin"
  cp "$root/.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh" "$sandbox/source/.chezmoiscripts/"
  : >"$sandbox_home/crate/Cargo.toml"
  cat >"$sandbox_home/.cargo/bin/cargo" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
# The test supplied the build artifact; no compiler or network is reached.
exit 0
STUB
  cat >"$sandbox_home/.cargo/bin/rustc" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
printf 'rustc 1.92.0-nightly (stub)\nhost: aarch64-apple-darwin\n'
STUB
  chmod +x "$sandbox_home/.cargo/bin/cargo" "$sandbox_home/.cargo/bin/rustc"
  printf abc >"$artifact"
}

function test_the_builder_publishes_the_record_tuple_before_installing_the_binary() {
  prepare_builder
  printf 'previous binary' >"$binary"
  run_in_sandbox bash "$rendered_builder"
  assert_successful_code
  assert_same 'previous binary' "$(cat "$sandbox/binary-at-publication")"
  assert_contains "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad 0755 $(id -u) $binary" \
    "$(cat "$pipeline_manifest")"
  assert_same abc "$(cat "$binary")"
  run_consumer _pipeline_deployed_state_is_known_good "$binary"
  assert_successful_code
}

function test_a_failed_builder_refresh_preserves_the_prior_record_tuple_and_binary() {
  prepare_builder
  run_in_sandbox bash "$rendered_builder"
  local previous_record previous_manifest status=0
  previous_record="$(cat "$build_record")"
  previous_manifest="$(cat "$pipeline_manifest")"
  printf def >"$artifact"
  touch "$sandbox/fail-install"
  run_in_sandbox bash "$rendered_builder" || status=$?
  assert_not_same 0 "$status"
  assert_same "$previous_record" "$(cat "$build_record")"
  assert_same "$previous_manifest" "$(cat "$pipeline_manifest")"
  assert_same abc "$(cat "$binary")"
  run_consumer _pipeline_deployed_state_is_known_good "$binary"
  assert_successful_code
}
