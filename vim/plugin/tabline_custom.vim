if exists("+showtabline")
    function! MyTabLine()
        let l:s = ''
        let l:t = tabpagenr()
        let l:i = 1
        while i <= tabpagenr('$')
            let l:buflist = tabpagebuflist(l:i)
            let l:winnr = tabpagewinnr(l:i)
            let l:s .= '%' . l:i . 'T'
            let l:s .= (i == l:t ? '%1*' : '%2*')
            let l:s .= (i == l:t ? '%#TabLineSel#' : '%#TabLine#')
            let l:file = bufname(buflist[winnr - 1])
            if l:file == ''
                let l:file = '[No Name]'
            else
                let l:file = fnamemodify(l:file, ':p:t')
            endif
            let l:s .= ' ' . l:file . ' '
            let l:i = l:i + 1
        endwhile
        let l:s .= '%T%#TabLineFill#%='
        let l:s .= (tabpagenr('$') > 1 ? '%999XX' : 'X')
        return l:s
    endfunction
    set stal=2
    set tabline=%!MyTabLine()
endif
