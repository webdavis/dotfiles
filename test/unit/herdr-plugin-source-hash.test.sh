#!/usr/bin/env bash
set -euo pipefail

function test_build_trigger_covers_changed_and_added_rust_modules() {
  bash "$(dirname "${BASH_SOURCE[0]}")/helpers/herdr-plugin-source-hash.sh" modules
  assert_successful_code
}

function test_build_trigger_covers_workspace_manifests_and_build_scripts() {
  bash "$(dirname "${BASH_SOURCE[0]}")/helpers/herdr-plugin-source-hash.sh" workspace
  assert_successful_code
}

function test_build_trigger_ignores_compiled_artifacts_and_keeps_other_plugins() {
  bash "$(dirname "${BASH_SOURCE[0]}")/helpers/herdr-plugin-source-hash.sh" other
  assert_successful_code
}
