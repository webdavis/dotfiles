#!/usr/bin/env bash
# A pin in the herdr plugin roster is a decision, so the apply-time installer
# compares each roster revision with the one herdr recorded and reinstalls the
# plugins that drifted. herdr v1 has no `plugin update`, so a reinstall is the
# only way to move one.
#
# The rendered script is driven with its own roster loop removed, so each test
# calls install_plugin with exactly the entry whose behavior it pins.

set_up_before_script() {
  local root
  root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  render_dir="$(mktemp -d)"
  rendered_installer="$render_dir/install-herdr-plugins.sh"
  HOME="$render_dir" CI=1 chezmoi --source "$root" execute-template --no-tty \
    <"$root/.chezmoiscripts/run_after_53-install-herdr-third-party-plugins.sh.tmpl" \
    >"$rendered_installer" 2>/dev/null
  [[ -s $rendered_installer ]] || return 1
  # The render bakes this machine's own roster in as install_plugin call lines.
  # Drop them so the calls under test are the only ones that run.
  sed -i '' '/^install_plugin /d' "$rendered_installer"
  ! grep -q '^install_plugin ' "$rendered_installer"
}

set_up() {
  sandbox="$(mktemp -d)"
  stubbin="$sandbox/bin"
  mkdir -p "$stubbin"
  installs="$sandbox/installs"
  : >"$installs"
  printf '[]\n' >"$sandbox/installed.json"
  # The stub answers `plugin list` out of installed.json in herdr's own envelope
  # shape, and records an install while upserting the plugin at the revision the
  # install asked for, which is what the post-install verification reads back.
  # The plugin id is the repository's last path segment, which the roster entries
  # these tests use follow.
  cat >"$stubbin/herdr" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
[[ ${1:-} == plugin ]] || exit 99
subcommand=$2
shift 2
case "$subcommand" in
  list)
    plugin=""
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --plugin)
          plugin=$2
          shift 2
          ;;
        *) shift ;;
      esac
    done
    jq --arg id "$plugin" \
      '{id: "cli:plugin", result: {plugins: [.[] | select(.plugin_id == $id)], type: "plugin_list"}}' \
      "$TEST_ROOT/installed.json"
    ;;
  install)
    printf '%s\n' "$*" >>"$TEST_ROOT/installs"
    source_repo=$1
    requested=""
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --ref)
          requested=$2
          shift 2
          ;;
        *) shift ;;
      esac
    done
    plugin_id=${source_repo##*/}
    jq --arg id "$plugin_id" --arg requested "$requested" \
      '[.[] | select(.plugin_id != $id)] + [{plugin_id: $id, source: {requested_ref: $requested, resolved_commit: $requested}}]' \
      "$TEST_ROOT/installed.json" >"$TEST_ROOT/installed.next"
    mv "$TEST_ROOT/installed.next" "$TEST_ROOT/installed.json"
    ;;
  *) exit 98 ;;
esac
STUB
  # Bounding every herdr call is the script's own precondition, and the real
  # timeout binary would leave the test depending on which one is on PATH.
  cat >"$stubbin/timeout" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
shift
exec "$@"
STUB
  chmod +x "$stubbin/herdr" "$stubbin/timeout"
}

# Record one plugin as installed at a requested revision and a resolved commit.
record_installed() {
  jq --arg id "$1" --arg requested "$2" --arg commit "$3" \
    '. + [{plugin_id: $id, source: {requested_ref: $requested, resolved_commit: $commit}}]' \
    "$sandbox/installed.json" >"$sandbox/installed.next"
  mv "$sandbox/installed.next" "$sandbox/installed.json"
}

# Run the rendered installer with one roster entry appended as its only call.
run_installer() {
  local driver="$sandbox/driver.sh"
  cp "$rendered_installer" "$driver"
  printf 'install_plugin %s %s %s\n' "$1" "$2" "'${3:-}'" >>"$driver"
  HOME="$sandbox" TEST_ROOT="$sandbox" PATH="$stubbin:$PATH" \
    bash "$driver" >"$sandbox/stdout" 2>"$sandbox/stderr"
}

function test_a_plugin_already_at_its_pinned_revision_is_not_reinstalled() {
  record_installed herdr-smart-nav b82c2cf b82c2cf
  run_installer herdr-smart-nav webdavis/herdr-smart-nav b82c2cf
  assert_successful_code
  assert_empty "$(cat "$installs")"
  assert_empty "$(cat "$sandbox/stdout")"
}

function test_a_moved_pin_reinstalls_the_plugin_at_the_pinned_revision() {
  record_installed herdr-smart-nav b82c2cf b82c2cf
  run_installer herdr-smart-nav webdavis/herdr-smart-nav 9f10abc
  assert_successful_code
  assert_same 'webdavis/herdr-smart-nav --ref 9f10abc --yes' "$(cat "$installs")"
  assert_contains 'herdr plugin herdr-smart-nav moved to 9f10abc' "$(cat "$sandbox/stdout")"
}

function test_a_roster_plugin_no_listing_reports_is_installed() {
  run_installer herdr-smart-nav webdavis/herdr-smart-nav b82c2cf
  assert_successful_code
  assert_same 'webdavis/herdr-smart-nav --ref b82c2cf --yes' "$(cat "$installs")"
  assert_contains 'herdr plugin herdr-smart-nav installed from' "$(cat "$sandbox/stdout")"
}

function test_an_unpinned_roster_entry_is_left_at_whatever_is_installed() {
  record_installed worktrunk "" 4be9bbbaab1dfbecc81b298d30624052d0c432d1
  run_installer worktrunk devashish2203/herdr-worktrunk
  assert_successful_code
  assert_empty "$(cat "$installs")"
}

# A plugin uu's weekly run reinstalled at tip records no requested revision, so
# a pin naming the commit it landed on is held rather than reinstalled.
function test_a_pin_naming_the_recorded_commit_is_held() {
  record_installed worktrunk "" 4be9bbbaab1dfbecc81b298d30624052d0c432d1
  run_installer worktrunk devashish2203/herdr-worktrunk 4be9bbbaab1d
  assert_successful_code
  assert_empty "$(cat "$installs")"
}
