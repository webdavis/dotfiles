" Note: overrides vim-unimpaired mapping.
" Creates a new blank line before or after current line.
nnoremap <silent> ]<Space> :<C-U>execute 'setlocal paste formatoptions-=o<BAR>
        \ execute "normal! mmo"<BAR>
        \ normal! `m'<BAR>
        \ setlocal nopaste formatoptions+=o<CR>
nnoremap <silent> [<Space> :<C-U>execute 'setlocal paste formatoptions-=o<BAR>
        \ execute "normal! mmO"<BAR>
        \ normal! `m'<BAR>
        \ setlocal nopaste formatoptions+=o<CR>
