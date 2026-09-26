# shellcheck shell=bash

rendered_text_is_blank() {
  local rendered=$1
  [[ -z ${rendered//[[:space:]]/} ]]
}

shellcheck_rendered_text() {
  local rendered=$1
  printf '%s\n' "$rendered" | shellcheck -
}

render_and_shellcheck_one() {
  local file=$1
  local rendered
  if ! rendered="$(render_template "$file")"; then
    printf 'shellcheck-rendered-template: chezmoi render failed: %s\n' "$file" >&2
    return 1
  fi
  if rendered_text_is_blank "$rendered"; then
    return 0
  fi
  if ! shellcheck_rendered_text "$rendered"; then
    printf 'shellcheck-rendered-template: rendered template failed shellcheck: %s\n' "$file" >&2
    return 1
  fi
}
