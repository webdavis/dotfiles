" Textwidth for regular filetypes.
setlocal textwidth=78

" Vim type text/help files.
if fnamemodify(expand('%'), ':h') ==# 'doc'
    " Custom textwidth for text files.
    setlocal textwidth=78

    " Toggle to help filetype.
    nnoremap <LocalLeader>f :<C-U>setlocal filetype=help<CR>
endif
