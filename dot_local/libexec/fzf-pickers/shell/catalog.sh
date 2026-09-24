# shellcheck shell=bash

# Prepare a command from the generated binding catalog.
__bash_bindings_list_bash_bindings() {
  local records="$HOME/.config/chord/bindings-menu.tsv"
  local source_records
  source_records="$(dirname -- "${BASH_SOURCE[0]}")/../../../../dot_config/chord/bindings-menu.tsv"
  [[ -r $records || ! -r $source_records ]] || records=$source_records
  if [[ ! -r $records ]]; then
    # The command hint is literal.
    # shellcheck disable=SC2016
    printf 'no binding records at %s; run `just chord-render` and apply\n' "$records" >&2
    return 1
  fi

  # key, group, description, action, in that order, for both paths below. A
  # key is never truncated, because the operator needs the whole chord; a
  # description is, because it is prose of no fixed length.
  local display='%-14s %-18s %-46.46s %s'

  # Without fzf there is nothing to pick with, so print the surface instead.
  if ! command -v fzf &>/dev/null; then
    awk -F'\t' -v display="$display" '!/^#/ { printf display "\n", $1, $2, $5, $4 }' "$records"
    return 0
  fi

  # Field 1 is the padded display; fzf searches and shows it alone, and
  # fields 2 and 3 carry the kind and the action back out of the selection.
  # Awk consumes its own field references.
  # shellcheck disable=SC2016
  local program='BEGIN { OFS = "\t" }
    !/^#/ { print sprintf(display, $1, $2, $5, $4), $3, $4, $5, $1 }'
  local reload selection
  printf -v reload '%q ' awk -F $'\t' -v "display=$display" "$program" "$records"
  selection="$(
    awk -F $'\t' -v "display=$display" "$program" "$records" |
      fzf --delimiter=$'\t' --with-nth=1 --id-nth=5 --track --no-multi \
        --prompt='binding> ' \
        --preview="printf '%s\n\n%s\n' {3} {4}" \
        --header=$'Enter     Prepare command\nAlt-Y     Copy command\nAlt-R     Refresh\nAlt-P     Preview\nAlt-W     Preview wrap\nAlt-Shift-W Results wrap' \
        --bind='start:hide-header,ctrl-/:toggle-header,ctrl-_:toggle-header,alt-/:toggle-header' \
        --bind="alt-r:reload-sync($reload)" \
        --bind="alt-y:execute-silent(printf '%s\n' {3} | pbcopy)" \
        --footer='Ctrl-/ Alt-/: help | Enter: prepare | Esc: cancel'
  )" || return 0
  [[ -n $selection ]] || return 0

  local -a fields=()
  IFS=$'\t' read -r -a fields <<<"$selection"
  local kind="${fields[1]}" action="${fields[2]}"
  case "$kind" in
    run | function | insert)
      READLINE_LINE=$action
      READLINE_POINT=${#READLINE_LINE}
      ;;
    command | macro)
      printf 'Press %s to use this Readline action\n' "${fields[4]}" >&2
      ;;
  esac
}
