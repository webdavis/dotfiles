#!/usr/bin/env bash
# Real chezmoi at the formatter boundary; validators capture only rendered data.
# shellcheck disable=SC2016

set_up() {
  RENDER_REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  RENDER_FIXTURE="$(mktemp -d)"
  RENDER_SOURCE="$RENDER_FIXTURE/source with 'quote'"
  RENDER_TMPDIR="$RENDER_FIXTURE/tmp"
  mkdir -p "$RENDER_SOURCE/.chezmoidata/nested" "$RENDER_SOURCE/.chezmoitemplates" \
    "$RENDER_SOURCE/inputs" "$RENDER_FIXTURE/bin" "$RENDER_TMPDIR"
  printf 'value: retained\n' >"$RENDER_SOURCE/.chezmoidata/nested/value.yaml"
  printf '{"extra": "root data"}\n' >"$RENDER_SOURCE/.chezmoidata.json"
  printf '{{ .value }}' >"$RENDER_SOURCE/.chezmoitemplates/partial"
  printf abc >"$RENDER_SOURCE/inputs/file"
  cat >"$RENDER_SOURCE/body.tmpl" <<'EOF'
{{ .value }}|{{ .extra }}|{{ includeTemplate "partial" . }}
{{ include "inputs/file" | sha256sum }}
{{ include (joinPath .chezmoi.sourceDir "inputs/file") | sha256sum }}
{{ range glob (joinPath .chezmoi.sourceDir "inputs/*") }}{{ include . | sha256sum }}{{ end }}
{{ .chezmoi.sourceDir }}
{{ env "CI" }}
EOF
  {
    printf '#!/bin/bash\n'
    cat "$RENDER_SOURCE/body.tmpl"
  } >"$RENDER_SOURCE/shell.tmpl"
  # jq is also used to prepare the context, so keep it real. Only lint stdin is
  # captured; invalid-body coverage below uses the real validators instead.
  cat >"$RENDER_FIXTURE/bin/shellcheck" <<'EOF'
#!/usr/bin/env bash
cat >"$RENDER_CAPTURE"
EOF
  cp "$RENDER_FIXTURE/bin/shellcheck" "$RENDER_FIXTURE/bin/yq"
  chmod +x "$RENDER_FIXTURE/bin/shellcheck" "$RENDER_FIXTURE/bin/yq"
  RENDER_CAPTURE="$RENDER_FIXTURE/capture"
  export RENDER_CAPTURE
}

tear_down() {
  # The build-output fixture below drops a mode to 000, and rm cannot recurse into
  # that, so the owner's bits go back on before the fixture goes.
  [[ -n ${RENDER_FIXTURE:-} ]] || return 0
  chmod -R u+rwX "$RENDER_FIXTURE" 2>/dev/null
  rm -rf "$RENDER_FIXTURE"
}

# TMPDIR is inside the fixture so a formatter's own scratch is torn down with it
# and the leak assertion below has a directory it owns to count.
run_formatter() (
  cd "$RENDER_SOURCE" || exit 1
  HOME="$RENDER_FIXTURE" TMPDIR="$RENDER_TMPDIR" PATH="$RENDER_FIXTURE/bin:$PATH" \
    "$RENDER_REPO/scripts/treefmt/$1" "$2"
)

count_render_tmpdir_entries() {
  find "$RENDER_TMPDIR" -mindepth 1 -maxdepth 1 | wc -l | tr -d ' '
}

function test_render_context_is_created_under_the_callers_tmpdir_and_removed_on_exit() {
  printf '#!/bin/bash\n# {{ env "HOME" }}\n' >"$RENDER_SOURCE/home.tmpl"
  assert_same 0 "$(count_render_tmpdir_entries)"
  run_formatter shellcheck-rendered-template.sh home.tmpl
  assert_successful_code
  assert_contains "$RENDER_TMPDIR/" "$(cat "$RENDER_CAPTURE")"
  assert_same 0 "$(count_render_tmpdir_entries)"
}

# A vault template is never rendered, so the hostile fixtures only ever MENTION
# keepassxc, in a comment, and carry a `fail` action that reddens the assertion
# below if the classifier stops refusing them. Nothing here can reach the vault.
function test_shell_formatter_refuses_to_render_a_template_that_names_keepassxc() {
  local output
  printf '#!/bin/bash\n# the password comes from keepassxc\n%s\n' \
    '{{ fail "rendered a template that needs the vault" }}' >"$RENDER_SOURCE/vault.tmpl"
  output="$(run_formatter shellcheck-rendered-template.sh vault.tmpl 2>&1)"
  assert_successful_code
  assert_empty "$output"
  assert_file_not_exists "$RENDER_CAPTURE"
}

function test_shell_formatter_refuses_to_render_a_template_whose_partial_names_keepassxc() {
  local output
  printf '# the password comes from keepassxc\n%s\n' \
    '{{ fail "rendered a partial that needs the vault" }}' \
    >"$RENDER_SOURCE/.chezmoitemplates/vault-partial"
  printf '#!/bin/bash\n%s\n' '{{ includeTemplate "vault-partial" . }}' \
    >"$RENDER_SOURCE/vault-caller.tmpl"
  output="$(run_formatter shellcheck-rendered-template.sh vault-caller.tmpl 2>&1)"
  assert_successful_code
  assert_empty "$output"
  assert_file_not_exists "$RENDER_CAPTURE"
}

# Two fixtures for the two ways unrelated output in the checkout has broken a render:
# a cargo build's vanishing dep entry, and the post-commit graphify rebuild churning
# its AST cache under a push, which died on `lstat graphify-out/cache/ast/<name>.tmp:
# no such file or directory`. A 000 directory stands in for that churn because a walk
# which descends cannot open it on any run, where a vanishing file races.
function test_shell_render_ignores_unrelated_build_output_and_keeps_source_hashes() {
  mkdir -p "$RENDER_SOURCE/pns/target/debug/deps"
  ln -s "$RENDER_FIXTURE/absent-rmeta" "$RENDER_SOURCE/pns/target/debug/deps/rmeta-gone"
  mkdir -p "$RENDER_SOURCE/graphify-out/cache/ast"
  chmod 000 "$RENDER_SOURCE/graphify-out/cache/ast"
  run_formatter shellcheck-rendered-template.sh shell.tmpl
  assert_successful_code
  assert_same "#!/bin/bash
retained|root data|retained
ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
$RENDER_SOURCE
1" "$(cat "$RENDER_CAPTURE" 2>/dev/null)"
}

function test_osquery_render_uses_own_data_despite_nested_worktree_data() {
  local output
  mkdir -p "$RENDER_SOURCE/.worktrees/stale/.chezmoidata"
  printf 'value: wrong\n' >"$RENDER_SOURCE/.worktrees/stale/.chezmoidata/value.yaml"
  printf '{{ if ne .value "retained" }}{{ fail "stale data" }}{{ end }}{"value":{{ .value | toJson }}}' \
    >"$RENDER_SOURCE/config.conf"
  output="$(run_formatter osquery-config-render.sh config.conf 2>&1)"
  assert_successful_code
  assert_empty "$output"
}

function test_espanso_render_keeps_data_partials_and_ci_guard() {
  run_formatter espanso-match-render.sh body.tmpl
  assert_successful_code
  assert_contains 'retained|root data|retained' "$(cat "$RENDER_CAPTURE")"
  assert_contains "$RENDER_SOURCE" "$(cat "$RENDER_CAPTURE")"
  assert_same 1 "$(tail -1 "$RENDER_CAPTURE")"
}

function test_all_formatters_propagate_real_template_errors() {
  local formatter output
  printf '#!/bin/bash\n{{ fail "intentional render failure" }}\n' >"$RENDER_SOURCE/error.tmpl"
  for formatter in shellcheck-rendered-template osquery-config-render espanso-match-render; do
    output="$(run_formatter "$formatter.sh" error.tmpl 2>&1)"
    assert_general_error
    assert_contains 'intentional render failure' "$output"
  done
}

function test_all_formatters_propagate_invalid_rendered_bodies() {
  local formatter output
  printf '#!/bin/bash\n"unterminated\n' >"$RENDER_SOURCE/invalid.tmpl"
  for formatter in shellcheck-rendered-template osquery-config-render espanso-match-render; do
    output="$(cd "$RENDER_SOURCE" && HOME="$RENDER_FIXTURE" TMPDIR="$RENDER_TMPDIR" \
      "$RENDER_REPO/scripts/treefmt/$formatter.sh" invalid.tmpl 2>&1)"
    assert_general_error
    assert_not_empty "$output"
  done
}
