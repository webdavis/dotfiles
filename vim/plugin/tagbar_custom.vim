" Prevent cursor from automatically moving to Tagbar.
let g:tagbar_autofocus = 0

" Enable 'relativenumber' in Tagbar.
let g:tagbar_show_linenumbers=2

" Open Tagbar on the left-hand side.
" let g:tagbar_left = 1

" If editing a markdown file call vim-markdown's Toc (table of contents), otherwise call Tagbar.
function! s:ExtendedTagbarToggle() abort
    if &filetype !~? 'markdown\|qf'
        execute "normal! :TagbarToggle\r"
    else
        redir =>l:buflist
        silent! ls!
        redir END
        for bufnum in map(filter(split(l:buflist, '\n'), 'v:val =~ "Location List"'), 'str2nr(matchstr(v:val, "\\d\\+"))')
            if bufwinnr(bufnum) != -1
                exec('lclose')
                return
            endif
        endfor
        execute "normal! :Toc\r"
    endif
endfunction

" Toggle and open the Tagbar.
nnoremap yoT :<C-u>call <SID>ExtendedTagbarToggle()<CR>
nnoremap yot :execute "TagbarOpen fj"<CR>
