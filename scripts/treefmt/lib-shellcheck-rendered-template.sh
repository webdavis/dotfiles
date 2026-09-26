# shellcheck shell=bash

rendered_text_is_blank() {
  local rendered_text=$1
  [[ -z ${rendered_text//[[:space:]]/} ]]
}

shellcheck_rendered_text() {
  local rendered_text=$1
  printf '%s\n' "$rendered_text" | shellcheck -
}

report_render_failure() {
  local template=$1
  printf 'shellcheck-rendered-template: chezmoi render failed: %s\n' "$template" >&2
}

report_shellcheck_failure() {
  local template=$1
  printf 'shellcheck-rendered-template: rendered template failed shellcheck: %s\n' "$template" >&2
}

shellcheck_rendered_template() {
  local template=$1
  local rendered_text
  if ! rendered_text="$(render_template "$template")"; then
    report_render_failure "$template"
    return 1
  fi
  if rendered_text_is_blank "$rendered_text"; then
    return 0
  fi
  if ! shellcheck_rendered_text "$rendered_text"; then
    report_shellcheck_failure "$template"
    return 1
  fi
}
