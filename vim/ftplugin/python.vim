" Interferes with language server managed by coc.nvim.
set nobackup
set nowritebackup

" Intellisense mappings.
nnoremap <LocalLeader>R :<C-u>execute 'CocCommand python.startREPL'<CR>
nnoremap <LocalLeader>r :<C-u>execute 'CocCommand python.execInTerminal'<CR>
xnoremap <LocalLeader>r :<C-u>execute 'CocCommand python.execSelectionInTerminal'<CR>
xnoremap <LocalLeader>d :<C-u>execute 'CocCommand python.execSelectionInDjangoShell'<CR>
