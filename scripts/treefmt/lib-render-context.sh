# shellcheck shell=bash

readonly repository_root="$PWD"
readonly render_home_path_pattern="${TMPDIR:-/tmp}/chezmoi-render.XXXXXXXX"

create_render_home() {
  mktemp -d "$render_home_path_pattern"
}

remove_render_home_on_exit() {
  trap 'rm -rf "$render_home"' EXIT
}

keep_chezmoi_away_from_the_real_home() {
  export HOME="$render_home"
}

tell_templates_to_skip_keepassxc() {
  export CI=1
}

directory_entry_exists() {
  local entry=$1
  [[ -e $entry || -L $entry ]]
}

link_entry_into() {
  local entry=$1 directory=$2
  ln -s "$entry" "$directory/${entry##*/}"
}

link_repository_top_level_into() {
  local directory=$1
  local entry
  for entry in "$repository_root"/* "$repository_root"/.[!.]* "$repository_root"/..?*; do
    if directory_entry_exists "$entry"; then
      link_entry_into "$entry" "$directory" || return 1
    fi
  done
}

create_shallow_source() {
  local shallow_source=$1
  mkdir "$shallow_source" || return 1
  link_repository_top_level_into "$shallow_source"
}

write_render_config_naming_the_real_source() {
  local config_file=$1
  jq -n --arg source "$repository_root" '{data: {chezmoi: {sourceDir: $source}}}' >"$config_file"
}

create_render_context() {
  render_home="$(create_render_home)" || return 1
  remove_render_home_on_exit
  keep_chezmoi_away_from_the_real_home
  tell_templates_to_skip_keepassxc
  render_source="$render_home/source"
  create_shallow_source "$render_source" || return 1
  render_config="$render_home/chezmoi.json"
  write_render_config_naming_the_real_source "$render_config" || return 1
}

render_template() {
  local template_file=$1
  chezmoi --config "$render_config" --source "$render_source" \
    execute-template --no-tty <"$template_file"
}
