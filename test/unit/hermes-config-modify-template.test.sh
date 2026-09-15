#!/usr/bin/env bash
set -euo pipefail

# private_dot_hermes/modify_private_config.yaml.tmpl owns two things inside
# ~/.hermes/config.yaml and hands every other line back to hermes, so the two
# behaviors worth pinning are the ones a reader cannot see by inspection: that a
# no-op render reproduces its stdin BYTE FOR BYTE (which is what keeps a quiet
# apply quiet, and what preserves hermes's comments and key order), and that the
# routes map is written WHOLE, so a route this repository does not declare is
# removed while a sibling key under tts survives.
#
# Every keepassxc lookup is stubbed out of a scratch copy of the template: the
# real ones need an interactive vault unlock, which no test host has.

function hermes_config_render() {
  local repo fixture stub
  repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  fixture="$(mktemp -d)"
  mkdir -p "$fixture/source"
  stub="$fixture/modify_config.yaml.tmpl"
  # SC2016 is the point: `$name` is a Go TEMPLATE variable that has to reach
  # chezmoi unexpanded, on both sides of the substitution.
  # shellcheck disable=SC2016
  sed \
    -e 's|(keepassxc (printf "Hermes :: Webhook Secret (#%s)" $name)).Password|(printf "stub-secret-%s" $name)|' \
    -e 's|(keepassxc (printf "Discord (Uriel) :: Channel ID (#%s)" $name)).Password|"12345678901234567"|' \
    -e 's|(keepassxc "ElevenLabs :: Voice ID").Password|"stub-voice-id"|' \
    "$repo/private_dot_hermes/modify_private_config.yaml.tmpl" >"$stub"
  HOME="$fixture" CI=1 chezmoi --config /dev/null --config-format toml --source "$fixture/source" \
    execute-template --no-tty --with-stdin --file "$stub"
}

function test_a_no_op_render_emits_its_stdin_byte_for_byte() {
  local dir
  dir="$(mktemp -d)"
  # The declared-fields-only render is, by construction, a file whose two
  # overlays already match. Wrapping it in a comment and an undeclared key is
  # what proves the pass-through: both are hermes's to own.
  hermes_config_render </dev/null >"$dir/declared.yaml"
  {
    printf '# a hermes comment chezmoi must not touch\n'
    cat "$dir/declared.yaml"
    printf 'hermes_own_key: 42\n'
  } >"$dir/live.yaml"
  hermes_config_render <"$dir/live.yaml" >"$dir/rendered.yaml"
  assert_same identical "$(cmp -s "$dir/live.yaml" "$dir/rendered.yaml" && echo identical || echo differs)"
}

function test_an_undeclared_route_is_removed_and_a_tts_sibling_survives() {
  local rendered route
  rendered="$(
    hermes_config_render <<'LIVE'
platforms:
  webhook:
    extra:
      routes:
        osquery:
          secret: superseded-by-posture
          deliver: discord
          deliver_only: true
tts:
  elevenlabs:
    model_id: keep-me
LIVE
  )"
  assert_not_contains 'osquery' "$rendered"
  assert_contains 'model_id: keep-me' "$rendered"
  for route in general pns pns-recap posture priority uu; do
    assert_contains "        ${route}:" "$rendered"
  done
}
