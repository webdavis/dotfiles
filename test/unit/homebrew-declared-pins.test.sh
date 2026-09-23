#!/usr/bin/env bash
# The pin-verify run_after_ script pins each declared formula, driven with a brew double.
function set_up_before_script() {
  PIN_REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  PIN_RENDER="$(mktemp -d)"
  mkdir -p "$PIN_RENDER/source" "$PIN_RENDER/home"
  : >"$PIN_RENDER/chezmoi.toml"
  jq -n --arg home "$PIN_RENDER/home" '{chezmoi: {os: "darwin", homeDir: $home}, packages: {macos: {
      homebrew: {trusted_taps: [], taps: [], formulae: ["rjyo/moshi/moshi-hook"], casks: [], mas: [],
        pinned: {"rjyo/moshi/moshi-hook": "0.3.26"}},
      uv: [], fnm: []}}}' >"$PIN_RENDER/data.json"
  HOME="$PIN_RENDER/home" chezmoi --config "$PIN_RENDER/chezmoi.toml" --source "$PIN_RENDER/source" \
    --destination "$PIN_RENDER/home" --cache "$PIN_RENDER/cache" \
    --persistent-state "$PIN_RENDER/state.boltdb" --override-data-file "$PIN_RENDER/data.json" \
    execute-template --no-tty \
    <"$PIN_REPO/.chezmoiscripts/run_after_11-verify-homebrew-pins.sh.tmpl" >"$PIN_RENDER/subject.sh"
}

function set_up() {
  PIN_CASE="$(mktemp -d)"
  mkdir -p "$PIN_CASE/home" "$PIN_CASE/brew/bin" "$PIN_CASE/tmp"
  cat >"$PIN_CASE/brew/bin/brew" <<'STUB'
#!/bin/bash
set -euo pipefail
printf '%s\n' "$*" >>"$HOME/brew-calls"
case "$*" in
  'list --versions rjyo/moshi/moshi-hook')
    [[ -n $PIN_INSTALLED ]] || exit 1
    printf 'moshi-hook %s\n' "$PIN_INSTALLED" ;;
  'list --pinned rjyo/moshi/moshi-hook')
    if [[ $PIN_PINNED == 1 ]]; then printf 'moshi-hook\n'; else printf 'Warning: moshi-hook not pinned\n' >&2; fi ;;
  'pin rjyo/moshi/moshi-hook' | bundle*) ;;
  list*) exit 1 ;;
  *) exit 99 ;;
esac
STUB
  chmod 755 "$PIN_CASE/brew/bin/brew"
  PIN_INSTALLED=0.3.26
  PIN_PINNED=0
}

function tear_down() {
  rm -rf "$PIN_CASE"
}

function tear_down_after_script() {
  rm -rf "$PIN_RENDER"
}

pin_invoke() {
  PIN_EXIT=0
  env -i HOME="$PIN_CASE/home" PATH=/usr/bin:/bin TMPDIR="$PIN_CASE/tmp" \
    HOMEBREW_PREFIX="$PIN_CASE/brew" NO_COLOR=1 REPORT_LIB_PLAIN=1 \
    PIN_INSTALLED="$PIN_INSTALLED" PIN_PINNED="$PIN_PINNED" \
    /bin/bash "$PIN_RENDER/subject.sh" >"$PIN_CASE/stdout" 2>"$PIN_CASE/stderr" || PIN_EXIT=$?
}

function test_an_unpinned_formula_at_the_declared_version_is_pinned() {
  pin_invoke
  assert_same 0 "$PIN_EXIT"
  assert_contains 'pin rjyo/moshi/moshi-hook' "$(cat "$PIN_CASE/home/brew-calls")"
  assert_contains 'pinned rjyo/moshi/moshi-hook at 0.3.26' "$(cat "$PIN_CASE/stdout")"
  assert_not_contains 'WARNING' "$(cat "$PIN_CASE/stderr")"
}

function test_a_formula_already_pinned_is_left_as_it_is() {
  PIN_PINNED=1
  pin_invoke
  assert_same 0 "$PIN_EXIT"
  assert_not_contains 'pin rjyo/moshi/moshi-hook' "$(cat "$PIN_CASE/home/brew-calls")"
  assert_empty "$(cat "$PIN_CASE/stdout")"
}

function test_a_formula_installed_at_another_version_is_pinned_there_and_reported() {
  PIN_INSTALLED=0.3.27
  pin_invoke
  assert_same 0 "$PIN_EXIT"
  assert_contains 'pin rjyo/moshi/moshi-hook' "$(cat "$PIN_CASE/home/brew-calls")"
  assert_contains 'rjyo/moshi/moshi-hook is pinned at 0.3.27' "$(cat "$PIN_CASE/stderr")"
  assert_contains 'declares 0.3.26' "$(cat "$PIN_CASE/stderr")"
}

function test_a_declared_formula_that_is_not_installed_is_warned_about_and_not_pinned() {
  PIN_INSTALLED=""
  pin_invoke
  assert_same 0 "$PIN_EXIT"
  assert_not_contains 'pin rjyo/moshi/moshi-hook' "$(cat "$PIN_CASE/home/brew-calls")"
  assert_contains 'is declared pinned at 0.3.26 but is not installed' "$(cat "$PIN_CASE/stderr")"
}
