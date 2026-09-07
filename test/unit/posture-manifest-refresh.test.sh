#!/usr/bin/env bash
# The builder refreshes only its governing manifest. The legacy two-manifest
# refresh can publish the first and then fail on the second.

set_up() {
  sandbox="$(mktemp -d)"
  stubbin="$sandbox/bin"
  mkdir -p "$stubbin" "$sandbox/home"
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
cp "$source_file" "$destination"
STUB
  chmod +x "$stubbin/chezmoi" "$stubbin/sudo"
}

run_refresh() {
  local root
  root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  HOME="$sandbox/home" CHEZMOI_SOURCE_DIR="$sandbox/source" TEST_ROOT="$sandbox" \
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null \
    OSQUERY_PIPELINE_MANIFEST="$pipeline_manifest" OSQUERY_MANAGED_BIN_MANIFEST="$bin_manifest" \
    PATH="$stubbin:$PATH" bash "$root/.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh" \
    "$@" >"$sandbox/stdout" 2>"$sandbox/stderr"
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
