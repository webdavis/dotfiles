" Improved nameescape() function.
function! s:Fnameescape(file)
    if exists('*fnameescape')
        return fnameescape(a:file)
    else
        return escape(a:file, " \t\n*?[{`$\\%#'\"|!<")
    endif
endfunction

function! nerdtree_extension#AttemptSelectLastFile()
    let l:previous = s:Fnameescape(fnamemodify(expand('#'), ':t'))
    if l:previous !=? ''
        call search('\v<' . l:previous . '>')
    endif
endfunction
