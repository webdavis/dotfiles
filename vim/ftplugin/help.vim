" Custom textwidth for help files.
setlocal textwidth=78

" Mappings.
nnoremap <buffer> <CR> <C-]>
nnoremap <buffer> <BS> <C-T>
nnoremap <buffer> o /'\l\{2,\}'<CR>
nnoremap <buffer> O ?'\l\{2,\}'<CR>
nnoremap <buffer> s /\|\zs\S\+\ze\|<CR>
nnoremap <buffer> S ?\|\zs\S\+\ze\|<CR>

" Press q to close window.
nnoremap <buffer> q :<C-U>execute 'if winnr("$") ># 1<BAR>close<BAR>else<BAR>quit<BAR>endif'<CR>

" Toggle to text filetype.
nnoremap <LocalLeader>f :<C-U>setlocal filetype=text<CR>
