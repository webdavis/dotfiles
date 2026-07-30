# shellcheck shell=bash
# Write tiny probe scripts (the good/bad test files the checker and runner scan)
# for the test-system suite. Sourced; no main.

# write_probe_script <path> [body-line...] -- write an executable bash script at
# <path>: a shebang followed by each body line (defaults to `exit 0`). Creates
# parent dirs. Each body line is one physical line, so a two-line body forms a
# backslash continuation when the first line ends in `\`.
write_probe_script() {
  local path="$1"
  shift
  mkdir -p "$(dirname "$path")"
  {
    printf '#!/usr/bin/env bash\n'
    if [[ $# -gt 0 ]]; then
      printf '%s\n' "$@"
    else
      printf 'exit 0\n'
    fi
  } >"$path"
  chmod +x "$path"
}

# write_probe_in_suite <root> <suite> <name> [body-line...] -- write an
# executable probe at <root>/<suite>/<name>.sh and print its path.
write_probe_in_suite() {
  local root="$1" suite="$2" name="$3"
  shift 3
  local path="$root/$suite/$name.sh"
  write_probe_script "$path" "$@"
  printf '%s\n' "$path"
}
