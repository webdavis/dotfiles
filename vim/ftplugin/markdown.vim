nnoremap <LocalLeader>f :TableFormat<CR>
nnoremap <LocalLeader>t :TableModeToggle<CR>
xnoremap <LocalLeader>n :s/^/\=printf("%d. ", line(".") - line("'<") + 1)<CR>
nnoremap <LocalLeader>d :<C-U>execute 'let @a = system("date ''+%F, %A''")'<CR><Bar>$a<C-R>a<BS><Esc>
