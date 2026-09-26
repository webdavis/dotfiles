# shellcheck shell=bash

readonly render_context_temp_template="${TMPDIR:-/tmp}/chezmoi-render.XXXXXXXX"

create_render_home() {
  render_home="$(mktemp -d "$render_context_temp_template")" || return 1
  trap 'rm -rf "$render_home"' EXIT
  HOME="$render_home"
  export HOME
}

link_repository_top_level_into() {
  local shallow_source=$1
  local entry
  for entry in "$PWD"/* "$PWD"/.[!.]* "$PWD"/..?*; do
    [[ -e $entry || -L $entry ]] || continue
    ln -s "$entry" "$shallow_source/${entry##*/}" || return 1
  done
}

write_render_config_naming_the_real_source() {
  local config_file=$1
  jq -n --arg source "$PWD" '{data: {chezmoi: {sourceDir: $source}}}' >"$config_file"
}

init_render_context() {
  create_render_home || return 1
  render_source="$render_home/source"
  mkdir "$render_source" || return 1
  link_repository_top_level_into "$render_source" || return 1
  render_config="$render_home/chezmoi.json"
  write_render_config_naming_the_real_source "$render_config" || return 1
}

render_template() {
  local template_file=$1
  CI=1 chezmoi --config "$render_config" --source "$render_source" \
    execute-template --no-tty <"$template_file"
}
