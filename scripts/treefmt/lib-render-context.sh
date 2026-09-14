# shellcheck shell=bash
# execute-template walks unrelated build output before evaluating its input.
# A shallow source view lets chezmoi read the root data and templates without
# descending into build directories or nested worktrees. Relative includes still
# follow these links; sourceDir-based includes and hashes use the real checkout.
init_render_context() {
  local entry
  # The template is spelled out because a bare `mktemp -d` ignores TMPDIR on
  # macOS, which puts the context somewhere the caller did not ask for.
  render_home="$(mktemp -d "${TMPDIR:-/tmp}/chezmoi-render.XXXXXXXX")" || return 1
  trap 'rm -rf "$render_home"' EXIT
  HOME="$render_home"
  export HOME
  render_source="$render_home/source"
  mkdir "$render_source" || return 1
  for entry in "$PWD"/* "$PWD"/.[!.]* "$PWD"/..?*; do
    [[ -e $entry || -L $entry ]] || continue
    ln -s "$entry" "$render_source/${entry##*/}" || return 1
  done
  render_config="$render_home/chezmoi.json"
  jq -n --arg source "$PWD" '{data: {chezmoi: {sourceDir: $source}}}' >"$render_config" || return 1
}

render_template() {
  CI=1 chezmoi --config "$render_config" --source "$render_source" --working-tree "$PWD" \
    --destination "$HOME" --cache "$HOME/cache" --persistent-state "$HOME/state.boltdb" \
    execute-template --no-tty <"$1"
}
