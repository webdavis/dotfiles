# shellcheck shell=bash

__fzf_pickers_menu() {
  local listing chosen
  listing=$(bindings --group fzf-pickers) || return 1
  chosen=$(
    awk -F '  +' '{ printf "%-16s%s\t%s\n", $1, (NF > 3 ? $3 : ""), $NF }' <<<"$listing" |
      fzf --delimiter=$'\t' --with-nth=1 --no-multi --prompt='picker> '
  ) || return 0
  eval "${chosen#*$'\t'}"
}

pickers() {
  local READLINE_LINE='' READLINE_POINT=0
  __fzf_pickers_menu || return
  [[ -n $READLINE_LINE ]] || return 0
  history -s -- "$READLINE_LINE"
  printf '%s\n' "$READLINE_LINE"
  __fzf_next_line=$READLINE_LINE
}

__fzf_register_next_line_hook() {
  [[ " ${precmd_functions[*]-} " == *" __fzf_offer_next_line "* ]] ||
    precmd_functions+=(__fzf_offer_next_line)
}

__fzf_offer_next_line() {
  [[ -n ${__fzf_next_line-} ]] || return 0
  local line=$__fzf_next_line mode
  unset __fzf_next_line
  [[ $line != *[[:cntrl:]]* ]] || return 0
  line=${line//\\/\\\\}
  line=${line//\"/\\\"}
  for mode in vi-insert vi-command emacs-standard; do
    bind -m "$mode" -x '"\C-x\C-z": __fzf_disarm_next_line'
  done
  bind -m vi-insert "\"\\e[0n\": \"$line\\C-x\\C-z\""
  bind -m vi-command "\"\\e[0n\": \"i$line\\C-x\\C-z\""
  bind -m emacs-standard "\"\\e[0n\": \"$line\\C-x\\C-z\""
  PS1="\[\e[5n\]$PS1"
}

__fzf_disarm_next_line() {
  local mode
  for mode in vi-insert vi-command emacs-standard; do
    bind -m "$mode" '"\e[0n": ""'
  done
}
