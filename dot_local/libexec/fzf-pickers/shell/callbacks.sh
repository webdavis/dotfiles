# shellcheck shell=bash
# vi: set filetype=sh:

__fzf_insert_arguments() {
  local text='' arg quoted before after
  before=${READLINE_LINE:0:READLINE_POINT}
  after=${READLINE_LINE:READLINE_POINT}
  for arg in "$@"; do
    printf -v quoted '%q' "$arg"
    text+="${text:+ }${quoted}"
  done
  [[ -n $text ]] || return 0
  [[ -z $before || $before == *[[:space:]] ]] || text=" $text"
  [[ -z $after || $after == [[:space:]]* ]] || text+=' '
  READLINE_LINE="${before}${text}${after}"
  READLINE_POINT=$((${#before} + ${#text}))
}

__fzf_run_picker() {
  python3 "$FZF_PICKERS_HELPER" run "$@"
}

__fzf_pick() {
  local result key value
  result=$(mktemp "${TMPDIR:-/tmp}/fzf-result.XXXXXXXX") || return 0
  if __fzf_run_picker "$@" >"$result"; then
    local -a fields=()
    mapfile -d '' -t fields <"$result"
    key=${fields[0]-}
    case $key in
      insert) __fzf_insert_arguments "${fields[@]:1}" ;;
      command)
        value=${fields[1]-}
        if [[ -n $value ]]; then
          READLINE_LINE=$value
          READLINE_POINT=${#READLINE_LINE}
        fi
        ;;
    esac
  fi
  # The picker removes its private result file after the shell has read it.
  python3 "$FZF_PICKERS_HELPER" cleanup "$result"
  return 0
}

__fzf_network_menu() { __fzf_pick network; }

__fzf_provenance() {
  local records name
  records=$(mktemp "${TMPDIR:-/tmp}/fzf-provenance.XXXXXXXX") || return 0
  while IFS= read -r name; do
    printf '%s\0' "$name"
    type -t -- "$name"
    printf '\0'
    type -a -- "$name"
    printf '\0'
  done < <(compgen -c | LC_ALL=C sort -u) >"$records"
  __fzf_pick provenance --provenance "$records"
  python3 "$FZF_PICKERS_HELPER" cleanup "$records"
}

__fzf_bookmarks() { __fzf_pick bookmarks --paths "${DIRSTACK[@]:1}"; }
__fzf_bookmark() {
  local directory
  for directory in "${DIRSTACK[@]:1}"; do
    [[ $directory != "$PWD" ]] || return 0
  done
  printf -v READLINE_LINE 'pushd -n -- %q' "$PWD"
  READLINE_POINT=${#READLINE_LINE}
}
