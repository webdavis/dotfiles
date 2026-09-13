#!/usr/bin/env bash
# Exercise the whole rendered package installer with private tool doubles.
function set_up_before_script() {
  SEED_REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  SEED_SOURCE="$(mktemp -d)"
  mkdir -p "$SEED_SOURCE/.chezmoitemplates"
  cp "$SEED_REPO/.chezmoitemplates/cli-print-style-lib.sh.tmpl" \
    "$SEED_REPO/.chezmoitemplates/brew-bundle-cleanup-guard.sh.tmpl" "$SEED_SOURCE/.chezmoitemplates/"
}

function set_up() {
  SEED_CASE="$(mktemp -d)"
  SEED_HOME="$SEED_CASE/home with spaces"
  SEED_CONFIG="$SEED_HOME/.local/share/graphify/claude"
  SEED_BUNDLE="$SEED_CONFIG/skills/graphify"
  SEED_OS=darwin
  SEED_PACKAGES='["graphifyy"]'
  SEED_SKIP=0
  SEED_PACKAGE_EXIT=0
  SEED_INSTALL_EXIT=0
  SEED_TIMEOUT_EXIT=0
  mkdir -p "$SEED_HOME/.local/bin" "$SEED_HOME/.claude/skills/graphify/references" \
    "$SEED_CASE/brew/bin" "$SEED_CASE/cache" "$SEED_CASE/tmp"
  printf 'managed instructions\n' >"$SEED_HOME/.claude/CLAUDE.md"
  printf 'legacy custom skill\n' >"$SEED_HOME/.claude/skills/graphify/SKILL.md"
  printf 'legacy custom reference\n' >"$SEED_HOME/.claude/skills/graphify/references/custom.md"
  SEED_LEGACY="$(find "$SEED_HOME/.claude" -type f -exec cksum {} \; | sort)"
  : >"$SEED_CASE/chezmoi.toml"
  cat >"$SEED_CASE/brew/bin/brew" <<'STUB'
#!/bin/bash
set -euo pipefail
printf '%s\n' "$*" >>"$HOME/brew-calls"
case "$1" in
  list) exit 1 ;;
  bundle) exit 0 ;;
  *) exit 99 ;;
esac
STUB
  cat >"$SEED_CASE/brew/bin/uv" <<'STUB'
#!/bin/bash
set -euo pipefail
printf '%s\n' "$@" >"$HOME/uv-argv"
[[ $# == 3 && $1 == tool && $2 == install && $3 == graphifyy ]] || exit 99
exit "$SEED_PACKAGE_EXIT"
STUB
  cat >"$SEED_CASE/brew/bin/gtimeout" <<'STUB'
#!/bin/bash
set -euo pipefail
printf '%s\n' "$1" >"$HOME/deadline"
[[ $SEED_TIMEOUT_EXIT == 0 ]] || exit "$SEED_TIMEOUT_EXIT"
shift
exec "$@"
STUB
  cat >"$SEED_HOME/.local/bin/graphify" <<'STUB'
#!/bin/bash
set -euo pipefail
printf '%s\n' "$@" >"$HOME/graphify-argv"
printf '%s\n' "$CLAUDE_CONFIG_DIR" >"$HOME/graphify-config"
mkdir -p "$CLAUDE_CONFIG_DIR/skills/graphify/references"
printf 'seeded skill\n' >"$CLAUDE_CONFIG_DIR/skills/graphify/SKILL.md"
printf 'app registration\n' >"$CLAUDE_CONFIG_DIR/CLAUDE.md"
[[ $SEED_INSTALL_EXIT == 0 ]] || exit "$SEED_INSTALL_EXIT"
printf 'fixture-version\n' >"$CLAUDE_CONFIG_DIR/skills/graphify/.graphify_version"
STUB
  chmod 755 "$SEED_CASE/brew/bin/"* "$SEED_HOME/.local/bin/graphify"
}

seed_render() {
  jq -n --arg os "$SEED_OS" --arg home "$SEED_HOME" --argjson uv "$SEED_PACKAGES" \
    '{chezmoi: {os: $os, homeDir: $home}, packages: {macos: {
      homebrew: {trusted_taps: [], taps: [], formulae: ["coreutils", "uv"], casks: [], mas: []},
      uv: $uv, fnm: []}}}' >"$SEED_CASE/data.json"
  HOME="$SEED_HOME" SKIP_SYSTEM_PACKAGES="$SEED_SKIP" chezmoi \
    --config "$SEED_CASE/chezmoi.toml" --source "$SEED_SOURCE" \
    --destination "$SEED_HOME" --cache "$SEED_CASE/cache" \
    --persistent-state "$SEED_CASE/state.boltdb" --override-data-file "$SEED_CASE/data.json" \
    execute-template --no-tty \
    <"$SEED_REPO/.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl" \
    >"$SEED_CASE/subject.sh" 2>"$SEED_CASE/render.stderr"
}

seed_invoke() {
  seed_render || return 1
  SEED_EXIT=0
  env -i HOME="$SEED_HOME" PATH=/usr/bin:/bin TMPDIR="$SEED_CASE/tmp" \
    HOMEBREW_PREFIX="$SEED_CASE/brew" NO_COLOR=1 REPORT_LIB_PLAIN=1 \
    CLAUDE_CONFIG_DIR="$SEED_HOME/.claude" SEED_PACKAGE_EXIT="$SEED_PACKAGE_EXIT" \
    SEED_INSTALL_EXIT="$SEED_INSTALL_EXIT" SEED_TIMEOUT_EXIT="$SEED_TIMEOUT_EXIT" \
    /bin/bash "$SEED_CASE/subject.sh" >"$SEED_CASE/stdout" 2>"$SEED_CASE/stderr" || SEED_EXIT=$?
  local legacy_after=''
  if [[ -d $SEED_HOME/.claude ]]; then
    legacy_after="$(find "$SEED_HOME/.claude" -type f -exec cksum {} \; | sort)"
  fi
  assert_same "$SEED_LEGACY" "$legacy_after"
  assert_is_not_symlink "$SEED_HOME/.claude/skills/graphify"
}

seed_existing_bundle() {
  mkdir -p "$SEED_BUNDLE/references"
  printf 'custom app skill\n' >"$SEED_BUNDLE/SKILL.md"
  printf 'old-version\n' >"$SEED_BUNDLE/.graphify_version"
  printf 'custom app reference\n' >"$SEED_BUNDLE/references/custom.md"
  printf 'custom app registration\n' >"$SEED_CONFIG/CLAUDE.md"
}

function test_a_fresh_bundle_is_seeded_without_changing_legacy_data_or_global_instructions() {
  seed_invoke
  assert_same 0 "$SEED_EXIT"
  assert_same $'tool\ninstall\ngraphifyy' "$(cat "$SEED_HOME/uv-argv")"
  assert_same $'install\n--platform\nclaude' "$(cat "$SEED_HOME/graphify-argv" 2>/dev/null)"
  assert_same "$SEED_CONFIG" "$(cat "$SEED_HOME/graphify-config" 2>/dev/null)"
  assert_same 60 "$(cat "$SEED_HOME/deadline" 2>/dev/null)"
  assert_same 'seeded skill' "$(cat "$SEED_BUNDLE/SKILL.md" 2>/dev/null)"
  assert_same 'fixture-version' "$(cat "$SEED_BUNDLE/.graphify_version" 2>/dev/null)"
}

function test_an_existing_complete_bundle_and_its_customizations_are_left_alone() {
  seed_existing_bundle
  local before
  before="$(find "$SEED_CONFIG" -type f -exec cksum {} \; | sort)"
  seed_invoke
  assert_same 0 "$SEED_EXIT"
  assert_file_not_exists "$SEED_HOME/graphify-argv"
  assert_same "$before" "$(find "$SEED_CONFIG" -type f -exec cksum {} \; | sort)"
}

function test_a_home_without_any_claude_directory_gets_only_the_app_owned_seed() {
  mv "$SEED_HOME/.claude" "$SEED_CASE/retained-fixture"
  SEED_LEGACY=''
  seed_invoke
  assert_same 0 "$SEED_EXIT"
  assert_file_exists "$SEED_BUNDLE/SKILL.md"
  assert_directory_not_exists "$SEED_HOME/.claude"
}

function test_an_incomplete_destination_is_refused_without_overwriting_its_data() {
  mkdir -p "$SEED_CONFIG"
  printf 'keep this\n' >"$SEED_CONFIG/custom.txt"
  seed_invoke
  assert_same 1 "$SEED_EXIT"
  assert_contains 'incomplete' "$(cat "$SEED_CASE/stderr")"
  assert_same 'keep this' "$(cat "$SEED_CONFIG/custom.txt")"
  assert_file_not_exists "$SEED_HOME/graphify-argv"
}

function test_a_file_at_the_destination_is_refused_without_overwriting_it() {
  mkdir -p "${SEED_CONFIG%/*}"
  printf 'keep this\n' >"$SEED_CONFIG"
  seed_invoke
  assert_same 1 "$SEED_EXIT"
  assert_same 'keep this' "$(cat "$SEED_CONFIG")"
  assert_file_not_exists "$SEED_HOME/graphify-argv"
}

function test_a_destination_symlink_to_the_legacy_directory_is_refused() {
  mkdir -p "${SEED_CONFIG%/*}"
  ln -s "$SEED_HOME/.claude/skills/graphify" "$SEED_CONFIG"
  seed_invoke
  assert_same 1 "$SEED_EXIT"
  assert_contains 'symlink' "$(cat "$SEED_CASE/stderr")"
  assert_is_symlink "$SEED_CONFIG"
  assert_file_not_exists "$SEED_HOME/graphify-argv"
}

function test_a_dangling_destination_symlink_is_refused() {
  mkdir -p "${SEED_CONFIG%/*}"
  ln -s "$SEED_CASE/missing" "$SEED_CONFIG"
  seed_invoke
  assert_same 1 "$SEED_EXIT"
  assert_is_symlink "$SEED_CONFIG"
  assert_file_not_exists "$SEED_HOME/graphify-argv"
}

function test_a_symlinked_parent_cannot_redirect_seeding_into_legacy_data() {
  mkdir -p "$SEED_HOME/.local/share"
  ln -s "$SEED_HOME/.claude/skills/graphify" "$SEED_HOME/.local/share/graphify"
  seed_invoke
  assert_same 1 "$SEED_EXIT"
  assert_contains 'symlink' "$(cat "$SEED_CASE/stderr")"
  assert_file_not_exists "$SEED_HOME/graphify-argv"
}

function test_a_symlink_inside_the_bundle_destination_is_refused() {
  mkdir -p "$SEED_CONFIG/skills"
  ln -s "$SEED_HOME/.claude/skills/graphify" "$SEED_BUNDLE"
  seed_invoke
  assert_same 1 "$SEED_EXIT"
  assert_contains 'symlink' "$(cat "$SEED_CASE/stderr")"
  assert_is_symlink "$SEED_BUNDLE"
  assert_file_not_exists "$SEED_HOME/graphify-argv"
}

function test_seeding_is_absent_when_the_package_is_not_declared() {
  SEED_PACKAGES='[]'
  seed_invoke
  assert_same 0 "$SEED_EXIT"
  assert_file_not_exists "$SEED_HOME/uv-argv"
  assert_file_not_exists "$SEED_HOME/graphify-argv"
}

function test_the_platform_guard_prevents_seeding_on_linux() {
  SEED_OS=linux
  seed_invoke
  assert_same 0 "$SEED_EXIT"
  assert_file_not_exists "$SEED_HOME/brew-calls"
  assert_file_not_exists "$SEED_HOME/graphify-argv"
}

function test_the_existing_system_package_skip_also_skips_seeding() {
  SEED_SKIP=1
  seed_invoke
  assert_same 0 "$SEED_EXIT"
  assert_file_not_exists "$SEED_HOME/brew-calls"
  assert_file_not_exists "$SEED_HOME/graphify-argv"
}

function test_a_failed_package_install_never_reaches_skill_seeding() {
  SEED_PACKAGE_EXIT=23
  seed_invoke
  assert_same 23 "$SEED_EXIT"
  assert_file_not_exists "$SEED_HOME/graphify-argv"
}

function test_a_failed_skill_install_propagates_without_claiming_a_complete_bundle() {
  SEED_INSTALL_EXIT=7
  seed_invoke
  assert_same 7 "$SEED_EXIT"
  assert_same 'seeded skill' "$(cat "$SEED_BUNDLE/SKILL.md" 2>/dev/null)"
  assert_file_not_exists "$SEED_BUNDLE/.graphify_version"
}

function test_a_bundle_left_without_a_completion_marker_is_preserved_for_manual_repair() {
  mkdir -p "$SEED_BUNDLE/references"
  printf 'partial skill\n' >"$SEED_BUNDLE/SKILL.md"
  seed_invoke
  assert_same 1 "$SEED_EXIT"
  assert_same 'partial skill' "$(cat "$SEED_BUNDLE/SKILL.md")"
  assert_file_not_exists "$SEED_HOME/graphify-argv"
  assert_file_not_exists "$SEED_BUNDLE/.graphify_version"
}

function test_a_directory_cannot_stand_in_for_the_skill_file() {
  mkdir -p "$SEED_BUNDLE/references" "$SEED_BUNDLE/SKILL.md"
  printf 'fixture-version\n' >"$SEED_BUNDLE/.graphify_version"
  seed_invoke
  assert_same 1 "$SEED_EXIT"
  assert_directory_exists "$SEED_BUNDLE/SKILL.md"
  assert_file_not_exists "$SEED_HOME/graphify-argv"
}

function test_a_directory_cannot_stand_in_for_the_completion_marker() {
  mkdir -p "$SEED_BUNDLE/references" "$SEED_BUNDLE/.graphify_version"
  printf 'partial skill\n' >"$SEED_BUNDLE/SKILL.md"
  seed_invoke
  assert_same 1 "$SEED_EXIT"
  assert_directory_exists "$SEED_BUNDLE/.graphify_version"
  assert_file_not_exists "$SEED_HOME/graphify-argv"
}

function test_a_seed_deadline_failure_propagates_without_running_the_installer() {
  SEED_TIMEOUT_EXIT=124
  seed_invoke
  assert_same 124 "$SEED_EXIT"
  assert_file_not_exists "$SEED_HOME/graphify-argv"
}
