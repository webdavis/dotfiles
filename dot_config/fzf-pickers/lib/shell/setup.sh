# shellcheck shell=bash

__fzf_setup() {
  local root=${1%/shell} saved_command=${FZF_CTRL_T_COMMAND-} command_set=${FZF_CTRL_T_COMMAND+x}
  if [[ -f $root/executable_picker.py ]]; then
    FZF_PICKERS_HELPER=$root/executable_picker.py
  else
    FZF_PICKERS_HELPER=$root/picker.py
  fi
  export FZF_PICKERS_HELPER
  export FZF_DEFAULT_OPTS='--height=60% --layout=reverse --cycle --multi --border=rounded --list-border=rounded --input-border=rounded --preview-border=rounded --preview-window="right,50%,border-rounded,<50(down,50%)" --color=bg+:#313244,bg:#1E1E2E,fg:#CDD6F4,fg+:#CDD6F4,hl:#F38BA8,hl+:#F38BA8,header:#A6ADC8,prompt:#CBA6F7,pointer:#CBA6F7,border:#6C7086 --bind=ctrl-n:down,ctrl-p:up,ctrl-alt-n:page-down,ctrl-alt-p:page-up,alt-p:toggle-preview,alt-w:toggle-preview-wrap,alt-W:toggle-wrap,ctrl-r:toggle-sort'
  export FZF_CTRL_T_OPTS='--height=60% --preview="bat --color=always --style=numbers -- {}" --header="Ctrl-/ Alt-/: help | Alt-P: preview | Alt-W: preview wrap | Alt-Shift-W: results wrap | Alt-Y: copy | Ctrl-R: order" --bind="start:hide-header,ctrl-/:toggle-header,ctrl-_:toggle-header,alt-/:toggle-header,alt-y:execute-silent(cat {+f} | pbcopy),alt-r:ignore"'
  export FZF_CTRL_R_OPTS='--height=60% --bind="start:hide-header,ctrl-/:toggle-header,ctrl-_:toggle-header,alt-/:toggle-header,alt-r:ignore,alt-R:toggle-raw,alt-y:execute-silent(cat {+f2..} | pbcopy)" --header="Ctrl-/ Alt-/: help | Alt-Shift-R: raw entries | Ctrl-R: order | Ctrl-Y: query yank"'
  FZF_CTRL_T_COMMAND=''
  local binding_file
  for binding_file in /opt/homebrew/opt/fzf/shell/key-bindings.bash /usr/share/fzf/key-bindings.bash /usr/share/doc/fzf/examples/key-bindings.bash; do
    if [[ -f $binding_file ]]; then
      # shellcheck disable=SC1090
      source "$binding_file"
      break
    fi
  done
  if [[ -n $command_set ]]; then
    FZF_CTRL_T_COMMAND=$saved_command
  else
    unset FZF_CTRL_T_COMMAND
  fi
}
