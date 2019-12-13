" vim-max

if exists('g:loaded_vim_maximizer') || &cp || v:version < 700
    finish
endif

let g:loaded_vim_maximizer = 1

if !exists('g:maximizer_set_default_mapping')
    let g:maximizer_set_default_mapping = 1
endif

if !exists('g:maximizer_set_mapping_with_bang')
    let g:maximizer_set_mapping_with_bang = 0
endif

if !exists('g:maximizer_restore_on_winleave')
    let g:maximizer_restore_on_winleave = 0
endif

if !exists('g:maximizer_default_mapping_key')
    let g:maximizer_default_mapping_key = '<F3>'
endif

if g:maximizer_set_default_mapping
    let command = ':MaximizerToggle'

    if g:maximizer_set_mapping_with_bang
        let command .= '!'
    endif

    silent! exe 'nnoremap <silent>' . g:maximizer_default_mapping_key . ' ' . command . '<CR>'
    silent! exe 'vnoremap <silent>' . g:maximizer_default_mapping_key . ' ' . command . '<CR>gv'
    silent! exe 'inoremap <silent>' . g:maximizer_default_mapping_key . ' <C-o>' . command . '<CR>'
endif

function! s:Maximize()
    let t:maximizer_sizes = { 'before': winrestcmd() }
    vert resize | resize
    let t:maximizer_sizes.after = winrestcmd()
    normal! ze
endfunction

function! s:Restore()
    if exists('t:maximizer_sizes')
        silent! exe t:maximizer_sizes.before
        if t:maximizer_sizes.before != winrestcmd()
            wincmd =
        endif
        unlet t:maximizer_sizes
        normal! ze
    end
endfunction

function! s:Toggle(force)
    if exists('t:maximizer_sizes') && (a:force || (t:maximizer_sizes.after == winrestcmd()))
        call s:Restore()
    elseif winnr('$') > 1
        call s:Maximize()
    endif
endfunction

if g:maximizer_restore_on_winleave
    augroup maximizer
        autocmd!
        autocmd WinLeave * call s:Restore()
    augroup END
endif

command! -bang -nargs=0 -range MaximizerToggle :call s:Toggle(<bang>0)
