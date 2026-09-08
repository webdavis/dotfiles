#!/usr/bin/env bash
# bashunit sources this file. Build once before timing the actual renderer cases.

pns_config_repo_root() {
  (cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
}

set_up_before_script() {
  PNS_CONFIG_REPO="$(pns_config_repo_root)"
  PNS_CONFIG_FIXTURE="$(mktemp -d)"
  cargo build --locked --quiet --manifest-path "$PNS_CONFIG_REPO/dot_local/share/pns/Cargo.toml" \
    --bin pns-config-render
}

set_up() {
  PNS_CONFIG_CASE="$(mktemp -d "$PNS_CONFIG_FIXTURE/case.XXXXXX")"
}

function test_the_binary_over_the_committed_values_file_writes_the_committed_template_exactly() {
  just --justfile "$PNS_CONFIG_REPO/justfile" --working-directory "$PNS_CONFIG_REPO" \
    pns-config-render "$PNS_CONFIG_CASE/rendered.tmpl" >"$PNS_CONFIG_CASE/stdout" 2>"$PNS_CONFIG_CASE/stderr"
  assert_successful_code
  diff -u "$PNS_CONFIG_REPO/dot_config/pns/private_config.toml.tmpl" "$PNS_CONFIG_CASE/rendered.tmpl"
  assert_successful_code
}

function test_the_resolved_configuration_over_the_committed_values_file_matches_its_snapshot() {
  cp "$PNS_CONFIG_REPO/dot_config/pns/config-values.toml" "$PNS_CONFIG_CASE/values.toml"
  cargo run --locked --quiet --manifest-path "$PNS_CONFIG_REPO/dot_local/share/pns/Cargo.toml" \
    --bin pns-config-render -- --check "$PNS_CONFIG_CASE/values.toml" \
    >"$PNS_CONFIG_CASE/stdout" 2>"$PNS_CONFIG_CASE/stderr"
  assert_successful_code
  [[ ! -s $PNS_CONFIG_CASE/stdout ]]
  assert_successful_code
  diff -u "$PNS_CONFIG_REPO/dot_config/pns/config-values.toml" "$PNS_CONFIG_CASE/values.toml"
  assert_successful_code
}
