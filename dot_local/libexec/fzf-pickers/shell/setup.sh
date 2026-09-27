# shellcheck shell=bash

__fzf_setup() {
  local root=${1%/shell} saved_command=${FZF_CTRL_T_COMMAND-} command_set=${FZF_CTRL_T_COMMAND+x}
  if [[ -f $root/executable_picker.py ]]; then
    FZF_PICKERS_HELPER=$root/executable_picker.py
  else
    FZF_PICKERS_HELPER=$HOME/.local/libexec/fzf-pickers/picker.py
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
  local mode binding function
  for mode in vi-insert vi-command emacs-standard; do
    bind -m "$mode" -x '"\C-t\C-t": fzf-file-widget'
    while IFS='|' read -r binding function; do
      bind -m "$mode" -x "\"${binding}\": ${function}"
    done <<'BINDINGS'
\C-tf|__fzf_pick files
\C-tF|__fzf_pick files --hidden
\C-th|__fzf_pick files --root home
\C-t/|__fzf_pick files --root /
\C-tdd|__fzf_pick directories
\C-tdg|__fzf_pick directories --root git
\C-td/|__fzf_pick directories --root /
\C-tdp|__fzf_pick parents
\C-tb|__fzf_bookmarks
\C-t\C-b|__fzf_bookmark
\C-g/|__fzf_pick contents
\C-g\C-f|__fzf_pick tracked
\C-gff|__fzf_pick git-files
\C-gfc|__fzf_pick changed
\C-gfh|__fzf_pick historical
\C-gch|__fzf_pick commits
\C-gbs|__fzf_pick refs
\C-gkk|__fzf_pick refs --checkout
\C-gkc|__fzf_pick commits --checkout
\C-gS|__fzf_pick stashes
\C-glr|__fzf_pick reflog
\C-gls|__fzf_pick log-search
\C-gw|__fzf_pick worktrees
\C-ge|__fzf_pick ignored
\C-gx|__fzf_pick conflicts
\C-gV|__fzf_pick git-config
\C-tj|__fzf_pick just
\C-tp|__fzf_pick processes
\C-tc|__fzf_pick chezmoi
\C-to|__fzf_provenance
\C-ta|__fzf_pick agents
\C-tC|__fzf_pick contacts
\C-tm|__fzf_pick messages
\C-tt|__fzf_pick tests
\C-tn|__fzf_pick devices
\C-te|__fzf_pick diagnostics
\C-tr|__fzf_pick recent
\C-tw|__fzf_pick tabs
\C-ts|__fzf_pick shortcuts
\C-tl|__fzf_pick logs
\C-tv|__fzf_pick captures
\C-tu|__fzf_pick url
\C-tq|__fzf_pick sqlite
\C-ti|__fzf_pick json
\C-tk|__fzf_pick services
\C-tx|__fzf_pick quickfix
BINDINGS
  done
}
