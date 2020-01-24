" Leader Keys. {{{1

" Hack for setting spacebar as the Leader key.
unlet! mapleader
let g:mapleader = "<Up>"
map <Space> <Leader>

" LocalLeader key.
let maplocalleader = '\'

" Misc. {{{1

" Disable 'j' and 'k' unless they used with a count.
" Note: 'gj' and 'gk' still work.
" nmap <expr> j v:count ==? 0 ? '' : 'j'
" nmap <expr> k v:count ==? 0 ? '' : 'k'
" vmap <expr> j v:count ==? 0 ? '' : 'j'
" vmap <expr> k v:count ==? 0 ? '' : 'k'

" Removes all whitespace from the end of lines.
nnoremap <Leader><C-w> :<C-u>StripWhitespace<CR>

" Reinitialize Visual mode after line shifting.
nnoremap << <<
nnoremap >> >>
xnoremap < <<gv
xnoremap > >>gv

" Toggle tab to 2 spaces
" function! s:ToggleTab() abort
" 	if &sw == 2 && &sts == 2
" 		execute 'setlocal shiftwidth=4 softtabstop=4'
" 	else
" 		execute 'setlocal shiftwidth=2 softtabstop=2'
" 	endif
" endfunction
" nnoremap yoe :<C-u>call <SID>ToggleTab()<CR>

" When the popup menu is showing, force the <Enter> key to close it and insert a new line.
inoremap <expr> <CR> pumvisible() ? "\<c-y>\<cr>" : "\<CR>"

" Auto close preview window when completion is done.
autocmd! CompleteDone * if pumvisible() == 0 | pclose | endif

" Shift line in insert mode.
inoremap <M-<> <C-o><<
inoremap <M->> <C-o>>>

" Paste the current file name.
inoremap <C-x>f <C-r>%

" Paste the last command.
inoremap <C-x>: <C-o>":p

" Delete the entire line.
inoremap <C-x><C-g> <C-o>0<C-o>C

" Insert a line above and below.
inoremap <M-o> <C-o>o
inoremap <M-O> <C-o>O

" Add the line under the cursor to the command-line.
cnoremap <C-r><C-l> <C-r>=substitute(getline('.'), '^\s*', '', '')<CR>

" Open a new tab.
nnoremap <silent> <Leader>C :<c-u>tabedit<cr>

" Move tabs.
nnoremap <silent> <M-PageUp> :execute 'silent! tabmove ' . (tabpagenr()-2)<CR>
nnoremap <silent> <M-PageDown> :execute 'silent! tabmove ' . (tabpagenr()+1)<CR>

" Pipe "diffsplit " to the command line.
nnoremap <Leader>;ds :execute "let a = ' '"<CR>:diffsplit<C-r>=a<CR>
" Pipe "vert diffsplit " to the command line.
nnoremap <Leader>;dv :execute "let a = ' '"<CR>:vert diffsplit<C-r>=a<CR>

" These change the working directory and print it out.
nnoremap <Leader>cl :<C-u>lcd %:p:h<CR>:pwd<CR>
nnoremap <Leader>cp :<C-u>cd %:p:h<CR>:pwd<CR>
nnoremap <Leader>;cp :<C-u>execute "let a = expand('%:p:h')"<CR>:cd <C-r>=a<CR>
nnoremap <Leader>cr :<C-u>execute 'cd '.fnamemodify(resolve(expand("%:p")), ":h")<CR>:pwd<CR>
nnoremap <Leader>;cr :<C-u>execute 'let a = fnamemodify(resolve(expand("%:p")), ":h")'<CR>:cd <C-r>=a<CR>

" Makes a session file.
nnoremap <Leader>S :<C-u>wall<BAR>execute "mksession! " . v:this_session<CR>

" Repeat last operation on the next search match.
nnoremap <Leader>n :<C-u>normal! n.<CR>
nnoremap <Leader>N :<C-u>normal! N.<CR>

" Builds command to substitute the word under the cursor, buffer-wise.
nnoremap <Leader>s% :%s/\<<C-r><C-w>\>//g<Left><Left>
vnoremap <Leader>s% y:%s/<C-r>"//g<Left><Left>
" Builds command to substitute the word under the cursor, line-wise.
nnoremap <Leader>sl :s/\<<C-r><C-w>\>//g<Left><Left>
" Builds command to drop into standard substitute, line-wise.
nnoremap <Leader>su :s///g<Left><Left><Left>
" Builds command to drop into standard substitute, buffer-wise.
nnoremap <Leader>sb :%s///g<Left><Left><Left>

" Fix spelling mistake under cursor (easier to remember than the original mapping).
nnoremap <Leader>sf :<C-u>execute 'normal! 1z='<CR>

" Fix last spelling error.
nnoremap <Leader>sp :<C-u>execute 'normal! mc[s1z=`cmc'<CR>

" Fix next spelling error.
nnoremap <Leader>sn :<C-u>execute 'normal! mc]s1z=`cmc'<CR>

" Case sensitive search.
nnoremap <Leader>/ /\C
vnoremap <Leader>/ y/<C-r>"<CR>

" Search for double spacing, excluding spacing at the beggining of the line.
nnoremap <Leader>? /\(^\s*\)\@<!\s\s\w<CR>

" Print the highlight group of the word under the cursor.
function! s:SynStack() abort
    if !exists("*synstack") | return | endif
    echo map(synstack(line('.'), col('.')), 'synIDattr(v:val, "name")')
endfunction
nnoremap <special> <Leader>sy :<C-u>call <SID>SynStack()<CR>

" Sources vimrc and resets Vim.
" Credit: tpope
nmap <Leader><C-r>  :<C-u>execute 'update<BAR>source ~/.vimrc<BAR>filetype detect<BAR>doau VimEnter -'<CR>
autocmd! VimEnter - set nohlsearch

" Press . (dot) in visual mode to repeat.
xnoremap . :<C-u>norm .<CR>


" FIXME this doesn't really work yet... fix it when I get some time.
function! DiffAgainstFileOnDisk()
    :w! /tmp/working_copy
    execute "!diff /tmp/working_copy %"
endfunction

command! DiffAgainstFileOnDisk call DiffAgainstFileOnDisk()


" Grep word under cursor, opening the output in a new tab.
function! s:GrepUnderCursor(word) abort
    execute 'tabnew'
    setlocal buftype=nofile bufhidden=hide noswapfile
    execute 'read !grep -Hnr "'.a:word.'"'
endfunction
nnoremap <Leader>tg :<C-u>call <SID>GrepUnderCursor(expand("<cword>"))<CR>


if has('mouse')
    function! s:MouseToggle()
        if &mouse ==? ''
            execute 'set mouse=ar'
            echom 'Mouse on'
        else
            execute 'set mouse='
            echom 'Mouse off'
        endif
    endfunction

    nnoremap <silent> <Leader>M :<C-u>call <SID>MouseToggle()<CR>
endif

function! s:ToggleAutoSave() abort
    if exists('#AutoSave#CursorHold')
        echom 'Unset autosave'
        augroup AutoSave
            autocmd!
        augroup END
    else
        echom 'Set autosave'
        augroup AutoSave
            autocmd!
            autocmd CursorHold * silent! execute 'update'
        augroup END
    endif
endfunction
nnoremap yoz :<C-u>call <SID>ToggleAutoSave()<CR>

" Visual-mode mappings {{{1

" Sort visually selected lines. {{{2
xnoremap <Leader>Sq :!sort<CR>
xnoremap <Leader>Su :!sort -u<CR>

" Searches for visually selected text. {{{2
function! s:getSelectedText()
  let l:old_reg = getreg('"')
  let l:old_regtype = getregtype('"')
  norm gvy
  let l:ret = getreg('"')
  call setreg('"', l:old_reg, l:old_regtype)
  exe "norm \<Esc>"
  return l:ret
endfunction

xnoremap <silent> * :call setreg("/",
    \ substitute(<SID>getSelectedText(),
    \ '\_s\+',
    \ '\\_s\\+', 'g')
    \ )<Cr>n

xnoremap <silent> # :call setreg("?",
    \ substitute(<SID>getSelectedText(),
    \ '\_s\+',
    \ '\\_s\\+', 'g')
    \ )<Cr>n

" Use / in insert mode to paste the last searched text. {{{1
function! Del_word_delims()
   let reg = getreg('/')
   " After *                i^r/ will give me pattern instead of \<pattern\>
   let res = substitute(reg, '^\\<\(.*\)\\>$', '\1', '' )
   if res != reg
      return res
   endif
   " After * on a selection i^r/ will give me pattern instead of \Vpattern
   let res = substitute(reg, '^\\V'          , ''  , '' )
   let res = substitute(res, '\\\\'          , '\\', 'g')
   let res = substitute(res, '\\n'           , '\n', 'g')
   return res
endfunction
inoremap <silent> <C-r>/ <C-r>=Del_word_delims()<CR>
cnoremap          <C-r>/ <C-r>=Del_word_delims()<CR>


" Delete to the end of the line. {{{1
function! s:StoreCmdBeforeCursor()
    let b:cmdl = matchstr(getcmdline(), getcmdline()[0 : getcmdpos()-2])
endfunction

function! s:PrintCmdBeforeCursor()
    return b:cmdl
endfunction

noremap! <expr> <SID>CmdDeleteAfterCursor
    \ getcmdpos() <= strlen(getcmdline()) ? (getcmdpos() <= 1 ? "\<Lt>End>\<Lt>C-u>" :
        \ <SID>StoreCmdBeforeCursor() . "\<Lt>End>\<Lt>C-u>" . <SID>PrintCmdBeforeCursor()) : ''

cmap <script> <M-K> <SID>CmdDeleteAfterCursor
inoremap <script> <M-K> <SID>CmdDeleteAfterCursor


" Pop and Pull Text. {{{1

" Paste mappings.
nnoremap <Leader>p$ my$i<C-o>"+p<ESC>`y
nnoremap <Leader>pa i<C-o>"+p<ESC>
nnoremap <Leader>pb i<C-o>"+P<ESC>
nnoremap <Leader>pf "%p
nnoremap <Leader>p^ my^i<C-o>"+P<ESC>`y
nnoremap <Leader>p: ":p
nnoremap <Leader>pp "+p^
nnoremap <Leader>PP "+P^

" Copy to end of line.
nnoremap Y yg_
nnoremap gY "+yg_

" Copy entire buffer to Vim register.
nnoremap <Leader>ya mmggyG`mmm

" Copy to system clipboard.
nnoremap gyy "+yy

" Copy entire buffer to the system clipboard.
nnoremap gya mmgg"+yG`mmm

" Copy the visually selected text to system clipboard.
xnoremap gyy "+y

" Delete or change the entire buffer.
nnoremap <Leader>cG ggcG
nnoremap <Leader>dG ggdG

" Removes all text from a line.
nnoremap <Leader>dd 0D

" Visually select the entire file.
nnoremap <Leader>Va gg<s-v>G


" Edit important files. {{{1

nnoremap <silent> <Leader>ev :<C-u>execute 'edit '.resolve(fnamemodify("~/.vimrc", ':p'))<CR>
nnoremap <silent> <Leader>sv :<C-u>execute 'split '.resolve(fnamemodify("~/.vimrc", ':p'))<CR>
nnoremap <silent> <Leader>vv :<C-u>execute 'vsplit '.resolve(fnamemodify("~/.vimrc", ':p'))<CR>
nnoremap <silent> <Leader>tv :<C-u>execute 'tabedit '.resolve(fnamemodify("~/.vimrc", ':p'))<CR>
nnoremap <silent> <Leader>em :<C-u>execute 'edit '.resolve(fnamemodify("~/.vim/plugin/mappings.vim", ':p'))<CR>
nnoremap <silent> <Leader>sm :<C-u>execute 'split '.resolve(fnamemodify("~/.vim/plugin/mappings.vim", ':p'))<CR>
nnoremap <silent> <Leader>vm :<C-u>execute 'vsplit '.resolve(fnamemodify("~/.vim/plugin/mappings.vim", ':p'))<CR>
nnoremap <silent> <Leader>tm :<C-u>execute 'tabedit '.resolve(fnamemodify("~/.vim/plugin/mappings.vim", ':p'))<CR>
nnoremap <silent> <Leader>ei :<C-u>execute 'edit '.resolve(fnamemodify("~/.config/i3/config", ':p'))<CR>
nnoremap <silent> <Leader>si :<C-u>execute 'split '.resolve(fnamemodify("~/.config/i3/config", ':p'))<CR>
nnoremap <silent> <Leader>vi :<C-u>execute 'vsplit '.resolve(fnamemodify("~/.config/i3/config", ':p'))<CR>
nnoremap <silent> <Leader>ti :<C-u>execute 'tabedit '.resolve(fnamemodify("~/.config/i3/config", ':p'))<CR>
nnoremap <silent> <Leader>eb :<C-u>execute 'edit '.resolve(fnamemodify("~/.bashrc", ':p'))<CR>
nnoremap <silent> <Leader>sb :<C-u>execute 'split '.resolve(fnamemodify("~/.bashrc", ':p'))<CR>
nnoremap <silent> <Leader>vb :<C-u>execute 'vsplit '.resolve(fnamemodify("~/.bashrc", ':p'))<CR>
nnoremap <silent> <Leader>tb :<C-u>execute 'tabedit '.resolve(fnamemodify("~/.bashrc", ':p'))<CR>
nnoremap <silent> <Leader>ee :<C-u>execute 'edit '.resolve(fnamemodify("~/.bash_aliases", ':p'))<CR>
nnoremap <silent> <Leader>se :<C-u>execute 'split '.resolve(fnamemodify("~/.bash_aliases", ':p'))<CR>
nnoremap <silent> <Leader>ve :<C-u>execute 'vsplit '.resolve(fnamemodify("~/.bash_aliases", ':p'))<CR>
nnoremap <silent> <Leader>te :<C-u>execute 'tabedit '.resolve(fnamemodify("~/.bash_aliases", ':p'))<CR>
nnoremap <silent> <Leader>et :<C-u>execute 'edit '.resolve(fnamemodify("~/.tmux.conf", ':p'))<CR>
nnoremap <silent> <Leader>st :<C-u>execute 'split '.resolve(fnamemodify("~/.tmux.conf", ':p'))<CR>
nnoremap <silent> <Leader>vt :<C-u>execute 'vsplit '.resolve(fnamemodify("~/.tmux.conf", ':p'))<CR>
nnoremap <silent> <Leader>tt :<C-u>execute 'tabedit '.resolve(fnamemodify("~/.tmux.conf", ':p'))<CR>
nnoremap <silent> <Leader>ec :<C-u>execute 'edit '.resolve(fnamemodify("~/.vim/coc-settings.json", ':p'))<CR>
nnoremap <silent> <Leader>sc :<C-u>execute 'split '.resolve(fnamemodify("~/.vim/coc-settings.json", ':p'))<CR>
nnoremap <silent> <Leader>vc :<C-u>execute 'vsplit '.resolve(fnamemodify("~/.vim/coc-settings.json", ':p'))<CR>
nnoremap <silent> <Leader>tc :<C-u>execute 'tabedit '.resolve(fnamemodify("~/.vim/coc-settings.json", ':p'))<CR>
nnoremap <silent> <Leader>eA :<C-u>execute 'edit '.resolve(fnamemodify("~/.vim/after/plugin/abolish.vim", ':p'))<CR>
nnoremap <silent> <Leader>sA :<C-u>execute 'split '.resolve(fnamemodify("~/.vim/after/plugin/abolish.vim", ':p'))<CR>
nnoremap <silent> <Leader>vA :<C-u>execute 'vsplit '.resolve(fnamemodify("~/.vim/after/plugin/abolish.vim", ':p'))<CR>
nnoremap <silent> <Leader>tA :<C-u>execute 'tabedit '.resolve(fnamemodify("~/.vim/after/plugin/abolish.vim", ':p'))<CR>


" Snippets {{{2

" Opens the snippet file for the current filetype.
nnoremap <silent> <Leader>es :<C-u>execute "edit ~/.config/coc/ultisnips/" . &filetype . ".snippets"<CR>
nnoremap <silent> <Leader>ss :<C-u>execute "split ~/.config/coc/ultisnips/" . &filetype . ".snippets"<CR>
nnoremap <silent> <Leader>vs :<C-u>execute "vsplit ~/.config/coc/ultisnips/" . &filetype . ".snippets"<CR>
nnoremap <silent> <Leader>ts :<C-u>execute "tabedit ~/.config/coc/ultisnips/" . &filetype . ".snippets"<CR>


" Projections {{{2

" Mappings for jumping to alternate files.
" Note: this requires a projections.json file at the root of the project.
nnoremap <silent> <Leader>ea :<C-u>A<CR>
nnoremap <silent> <Leader>sa :<C-u>AS<CR>
nnoremap <silent> <Leader>va :<C-u>AV<CR>
nnoremap <silent> <Leader>ta :<C-u>AT<CR>


" Most recent file {{{2
" TODO: Recursively search for most recently modified file using vimL, ignoring binary
" files.
" function! s:MostRecentlyModifiedFile()
"     return systemlist('find . -type f -exec stat --format "%Y :%n" "{}" \; | sort -nr | cut -d: -f2- | head')[0]
" endfunction

" Edit the most recently modified file.
" nnoremap <silent> <Leader>er :<C-u>execute 'edit ' . <SID>MostRecentlyModifiedFile()<CR>
" nnoremap <silent> <Leader>vr :<C-u>execute 'vsplit ' . <SID>MostRecentlyModifiedFile()<CR>
" nnoremap <silent> <Leader>sr :<C-u>execute 'split ' . <SID>MostRecentlyModifiedFile()<CR>


" Letter Case Toggle. {{{1
function! s:InsertCaseToggle()
    if col('.') ==? col('$')-1
        execute 'normal! mmg~iw`mmm'
    else
        execute 'normal! hmmg~iw`mmml'
    endif
endfunction
inoremap <silent> <M-u> <Esc>:<C-u>call <SID>InsertCaseToggle()<CR>a


" View. {{{1

" Increase scrolling distance in Visual mode.
if exists(':tnoremap')
    xnoremap <End> <C-e><C-e><C-e>
    xnoremap <Home> <C-y><C-y><C-y>
    xnoremap <C-e> <C-e><C-e><C-e>

    nnoremap <End> <C-e><C-e><C-e>
    nnoremap <Home> <C-y><C-y><C-y>
    nnoremap <C-e> <C-e><C-e><C-e>
else
    xnoremap OH <C-y><C-y><C-y>
    xnoremap OF <C-e><C-e><C-e>
    nnoremap OH <C-y><C-y><C-y>
    nnoremap OF <C-e><C-e><C-e>
endif
xnoremap <C-e> <C-e><C-e><C-e>
xnoremap <C-y> <C-y><C-y><C-y>
nnoremap <C-e> <C-e><C-e><C-e>
nnoremap <C-y> <C-y><C-y><C-y>

" Resize the window.
nnoremap <M-J> <C-w>-<C-w>-<C-w>-
nnoremap <M-K> <C-w>+<C-w>+<C-w>+
nnoremap <M-H> <C-w><<C-w><<C-w><<C-w><<C-w><<C-w><<C-w><<C-w><<C-w><<C-w><<C-w><
nnoremap <M-L> <C-w>><C-w>><C-w>><C-w>><C-w>><C-w>><C-w>><C-w>><C-w>><C-w>><C-w>>

" Maximize window.
nnoremap <silent> <C-w><C-m> :<C-u>echohl ErrorMsg<BAR>echo 'Maximizer'<BAR>MaximizerToggle!<CR>
xnoremap <silent> <C-w><C-m> :<C-u>echohl ErrorMsg<BAR>echo 'Maximizer'<BAR>MaximizerToggle!<CR>gv
inoremap <silent> <C-x><C-m> <C-o>:<C-u>echohl ErrorMsg<BAR>echo 'Maximizer'<BAR>MaximizerToggle!<CR>


" Quickfix Window Toggle. {{{1

" This will close the quickfix window whether it is the
" currently focused window or not. Credit:
" https://vim.fandom.com/wiki/Toggle_to_open_or_close_the_quickfix_window
function! s:GetBufferList()
    redir =>buflist
    silent! ls!
    redir END
    return buflist
endfunction

function! s:ToggleList(bufname, pfx)
    let l:buflist = s:GetBufferList()
    for bufnum in map(filter(split(l:buflist, '\n'), 'v:val =~ "'.a:bufname.'"'), 'str2nr(matchstr(v:val, "\\d\\+"))')
        if bufwinnr(bufnum) != -1
            exec(tolower(a:pfx) . 'close')
            return
        endif
    endfor
    if a:pfx == 'l' && len(getloclist(0)) == 0
        echohl ErrorMsg
        echo "Location List is Empty."
        return
    endif
    let winnr = winnr()
	exec(a:pfx . 'open')
    if winnr() != winnr
        wincmd p
    endif
endfunction

nmap <silent> yoL :call <SID>ToggleList("Location List", 'l')<CR>
nmap <silent> yoq :call <SID>ToggleList("Quickfix List", 'c')<CR>

" Press <C-w> + p to jump to the quickfix window, or back to the previous window if
" already in the quickfix window.
nnoremap <silent> <expr><C-w>b &filetype ==# 'qf' ? '<C-w>p' : '<C-w>b'


" Nvim Terminal Mode. {{{1

" Exit terminal mode and change windows.
if has('nvim')
	" Start Terminal in insert mode.
    tnoremap <silent> <C-h> <C-\><C-n>:TmuxNavigateLeft<CR>
    tnoremap <silent> <C-j> <C-\><C-n>:TmuxNavigateDown<CR>
    tnoremap <silent> <C-k> <C-\><C-n>:TmuxNavigateUp<CR>
    tnoremap <silent> <C-l> <C-\><C-n>:TmuxNavigateRight<CR>
	function! s:CloseWindow()
		execute winnr('$') >? 1 ? 'close' : 'quit'
	endfunction
	tnoremap <silent> <C-q> <C-\><C-n>:<C-u>call <SID>CloseWindow()<CR>
	tnoremap <M-[> <Esc>
    tnoremap <Esc> <C-\><C-n>

	" Open terminal.
	" Credit: https://github.com/neovim/neovim/issues/5073
	command! -nargs=* T split | terminal <args>
	command! -nargs=* VT vsplit | terminal <args>
    nnoremap <C-t><C-x> :<C-u>T<CR>
    nnoremap <C-t><C-v> :<C-u>VT<CR>
endif


" ALE. {{{1

" Toggles showing errors detected by ALE.
nmap <silent> yoa <Plug>(ale_toggle_buffer)

" Turns ALE on or off.
nmap <silent> yoA <Plug>(ale_toggle)

" Cycle through errors.
nmap <silent> <M-p> <Plug>(ale_previous_wrap)
nmap <silent> <M-n> <Plug>(ale_next_wrap)
nmap <silent> <M-P> <Plug>(ale_first)
nmap <silent> <M-N> <Plug>(ale_last)


" Tags. {{{1

" Open tags in preview window instead of replacing current window.
nnoremap <C-]> <Esc>:exe "ptjump " . expand("<cword>")<Esc>


" Gundo. {{{1
nnoremap <Leader>U :<C-u>UndotreeToggle<CR>


" Plug. {{{1
nnoremap <Leader>pi :<C-u>PlugInstall<CR>
nnoremap <Leader>pu :<C-u>PlugUpdate<CR>
nnoremap <Leader>pc :<C-u>PlugClean<CR>


" vim-xapprentice. {{{1
nnoremap <silent> <Leader>X :<C-u>XBackground<CR>


" Coc Intellisense Engine. {{{1

" Coc Commands {{{2

nnoremap <silent> <M-d> :<C-u>CocList diagnostics<CR>
nnoremap <silent> <M-c> :<C-u>CocList commands<CR>
nnoremap <silent> <M-/> :<C-u>CocSearch <C-r><C-w><CR>
nnoremap <silent> <M-s> :<C-u>CocList --interactive symbols<CR>
nnoremap <silent> <M-o> :<C-u>CocList --auto-preview outline<CR>

nmap <Leader>ca <Plug>(coc-codeaction)
nmap <Leader>cs <Plug>(coc-codeaction-selected)
xmap <Leader>ca <Plug>(coc-codeaction-selected)
nmap <Leader>gi :<C-u>CocList gitignore<CR>
nmap <Leader>re <Plug>(coc-refactor)
nmap <M-a> <Plug>(coc-codelens-action)
nmap <M-f> <Plug>(coc-fix-current)
nmap <M-O> <Plug>(coc-openlink)
nmap <M-q> :<C-u>call CocAction('format')<CR>
xmap <M-q> <Plug>(coc-format-selected)
nmap <M-r> <Plug>(coc-rename)
nmap <M-y> :<C-u>CocList -A yank<CR>
nmap <silent> ]g <Plug>(coc-diagnostic-next)
nmap <silent> [g <Plug>(coc-diagnostic-prev)
nnoremap <M-g> :<C-u>:CocListResume<CR>
nnoremap <M-t> :<C-u>execute 'CocCommand terminal.Toggle'<CR>

" Multicursors. {{{2
function! s:SelectCurrentWord()
    if !get(g:, 'coc_cursors_activated', 0)
        return "\<Plug>(coc-cursors-word)"
    endif
    return "*\<Plug>(coc-cursors-word):nohlsearch\<CR>"
endfunction
nmap <expr> <silent> <M-w> <SID>SelectCurrentWord()
nmap        <silent> <M-W> <Plug>(coc-cursors-word)
nmap <silent> <M-m> <Plug>(coc-cursors-position)
xmap <silent> <M-m> <Plug>(coc-cursors-range)
" }}}


" Trigger completion.
inoremap <silent><expr> <C-space> coc#refresh()

" Remap keys for gotos.
nmap <silent> <Leader>jd <Plug>(coc-definition)
nmap <silent> <Leader>jt <Plug>(coc-type-definition)
nmap <silent> <Leader>ji <Plug>(coc-implementation)
nmap <silent> <Leader>jr <Plug>(coc-references)
autocmd FileType java nmap <M-i> :<C-u>call :<C-u>CocCommand java.action.organizeImports<CR>
autocmd FileType python nmap <M-i> :<C-u>call :<C-u>CocCommand python.sortImports<CR>

" Use K to show documentation in the preview window.
function! s:ShowDocumentation()
	if index(['vim', 'help'], &filetype) >= 0
		execute 'help ' . scriptease#helptopic()
	else
		call CocAction('doHover')
	endif
endfunction
nnoremap <silent> K :call <SID>ShowDocumentation()<CR>
nmap <silent> <expr> <Esc> pumvisible() ==? 0 ? "<Plug>(coc-float-hide)" : "\<Esc>"

" Scroll the popup created by doHover.
nnoremap <expr><C-f> coc#util#has_float() ? coc#util#float_scroll(1) : "\<C-f>"
nnoremap <expr><C-b> coc#util#has_float() ? coc#util#float_scroll(0) : "\<C-b>"

" Ensure Ctrl+e always moves the cursor to the end of the line.
inoremap <expr> <C-e> pumvisible() ? "\<Lt>End>" : "\<Lt>End>"

" Go to next and previous snippet position.
let g:coc_snippet_next = '<C-l>'
let g:coc_snippet_prev = '<C-h>'
inoremap <silent><expr> <C-l> pumvisible() ? coc#_select_confirm() : "\<C-g>u\<C-l>\<c-r>=coc#on_enter()\<CR>"

function! s:check_back_space() abort
    let col = col('.') - 1
    return !col || getline('.')[col - 1]  =~# '\s'
endfunction

imap <Tab> <Plug>(neosnippet_expand)
smap <Tab> <Plug>(neosnippet_expand)
xmap <Tab> <Plug>(neosnippet_expand)
imap <Tab> <Plug>(coc-snippets-expand)
" let g:UltiSnipsExpandTrigger = '<Tab>'
" let g:UltiSnipsJumpForwardTrigger = '<C-j>'
let g:UltiSnipsJumpBackwardTrigger = '<C-h>'
let g:UltiSnipsSnippetDirectories = [$HOME . "/.config/coc/ultisnips"]

xmap if <Plug>(coc-funcobj-i)
xmap af <Plug>(coc-funcobj-a)
omap if <Plug>(coc-funcobj-i)
omap af <Plug>(coc-funcobj-a)

augroup custom_coc
	autocmd!
	autocmd FileType java,typescript,json setl formatexpr=CocAction('format')
	autocmd CursorHold * silent call CocActionAsync('highlight')
	autocmd User CocJumpPlaceholder call CocActionAsync('showSignatureHelp')
augroup END


" Coc-explorer {{{2

" Toggles explorer on/off.
nnoremap <silent> <C-w><C-n> :CocCommand explorer --toggle<CR>

" Opens explorer to the current buffer.
nnoremap <silent> <C-w><C-f> :CocCommand explorer --reveal --toggle<CR>


" Dispatch. {{{1

" Compiler setttings.

function! s:ShowQuickfix() abort
	if &filetype ==# 'qf'
        execute 'cclose'
    else
        execute 'Copen'
        execute 'MaximizerToggle'
    endif
endfunction

augroup custom_dispatch
    autocmd!
    autocmd BufReadPost quickfix nnoremap <buffer> R :Copen<CR>G
    autocmd QuickFixCmdPost * nnoremap <silent> <F5> :<C-u>call <SID>ShowQuickfix()<CR>
    autocmd QuickFixCmdPost * nmap <silent> <F4> :call <SID>ToggleList("Quickfix List", 'C')<CR>
augroup END

" Force the quickfix window to open whether Make is successful or not.
setlocal errorformat+=%+G%.%#

" Turn Tmux strategy off.
let g:dispatch_no_tmux_dispatch = 1

" Triggers.
nnoremap <C-s>\     :<C-u>write<BAR>Make<CR>
nnoremap <C-s><C-m> :<C-u>write<BAR>Make!<CR>
nnoremap <C-s>m     :<C-u>execute "let a = ' '"<CR>:Make<C-r>=a<CR>
nnoremap <C-s>n     :<C-u>execute "let a = ' '"<CR>:Make!<C-r>=a<CR>

nnoremap <C-s><C-f> :<C-u>write<BAR>Dispatch<CR>
nnoremap <C-s><C-d> :<C-u>write<BAR>Dispatch!<CR>
nnoremap <C-s>;     :<C-u>execute "let a = ' '"<CR>:Dispatch<C-r>=a<CR>
nnoremap <C-s>d     :<C-u>execute "let a = ' '"<CR>:Dispatch!<C-r>=a<CR>

nnoremap <C-s>'     :<C-u>write<BAR>Start<CR>
nnoremap <C-s><C-s> :<C-u>write<BAR>Start!<CR>
nnoremap <C-s>s     :<C-u>execute "let a = ' '"<CR>:Start<C-r>=a<CR>
nnoremap <C-s>t     :<C-u>execute "let a = ' '"<CR>:Start!<C-r>=a<CR>

nnoremap <C-s><C-g> :<C-u>write<BAR>Spawn<CR>
nnoremap <C-s><C-p> :<C-u>write<BAR>Spawn!<CR>
nnoremap <C-s>g     :<C-u>execute "let a = ' '"<CR>:Spawn<C-r>=a<CR>
nnoremap <C-s>p     :<C-u>execute "let a = ' '"<CR>:Spawn!<C-r>=a<CR>

nnoremap <C-s>c     :<C-u>Console<CR>


" vim-test {{{1
nnoremap <silent> t<CR>  :TestNearest<CR>
nnoremap <silent> t<C-f> :TestFile<CR>
nnoremap <silent> t<C-s> :TestSuite<CR>
nnoremap <silent> t<C-l> :TestLast<CR>
nnoremap <silent> t<C-v> :TestVisit<CR>


" Zeavim. {{{1
nmap <Leader>z <Plug>Zeavim
xnoremap <Leader>z <Plug>ZVVisSelection
nnoremap <Leader>Z <Plug>ZVKeyDocset

nnoremap M :<C-u>execute "Man " . expand("<cWORD>")<CR>
nnoremap <Leader>m :execute "let a = ' '"<CR>:Man<C-r>=a<CR>
nnoremap <Leader>k :execute "let a = ' '"<CR>:help<C-r>=a<CR>


" junegunn/fzf {{{1

" Add Fzf to Vim's runtimepath.
set rtp+=~/workspaces/tools/fzf/bin

let $FZF_DEFAULT_COMMAND = 'rg --files --no-ignore --vimgrep --color=never --smart-case --follow --glob "!.git/" 2>/dev/null'

" The size and location of the fzf window.
let g:fzf_layout = { 'down': '~20%' }

" Custom fzf colors.
let g:fzf_colors = {
    \ 'fg':      ['fg', 'Normal'],
    \ 'bg':      ['bg', 'Normal'],
    \ 'hl':      ['fg', 'Comment'],
    \ 'fg+':     ['fg', 'CursorLine', 'CursorColumn', 'Normal'],
    \ 'bg+':     ['bg', 'CursorLine', 'CursorColumn'],
    \ 'hl+':     ['fg', 'Statement'],
    \ 'info':    ['fg', 'PreProc'],
    \ 'border':  ['fg', 'VertSplit'],
    \ 'prompt':  ['fg', 'Conditional'],
    \ 'pointer': ['fg', 'Exception'],
    \ 'marker':  ['fg', 'Keyword'],
    \ 'spinner': ['fg', 'Label'],
    \ 'header':  ['fg', 'Comment']
    \ }

" Enables searching and going to buffers.
let g:fzf_buffers_jump = 1

" Instruct Fzf where to store its usage history. Some fzf.vim's commands spawn files for
" querying.
let g:fzf_history_dir = '~/.local/share/fzf-history'

" If the window exists, jump to it.
let g:fzf_buffers_jump = 1

" The command used to generate tags file.
let g:fzf_tags_command = 'ctags -R'


" File search settings.
command! -bang -nargs=? -complete=dir Files call fzf#vim#files(
    \ <q-args>,
    \ <bang>0 ? fzf#vim#with_preview('up:60%') : fzf#vim#with_preview('right:50%:hidden', '?'),
    \ <bang>0)


" Builds a command for Ag (search tool).
command! -bang -nargs=* Ag call fzf#vim#ag(
    \ <q-args>,
    \ <bang>0 ? \ fzf#vim#with_preview('up:60%') : fzf#vim#with_preview('right:50%:hidden', '?'),
    \ <bang>0)


" Builds a command for Ripgrep (search tool). See https://github.com/BurntSushi/ripgrep.
command! -bang -nargs=* Rg call fzf#vim#grep(
        \ (systemlist('git rev-parse --show-toplevel')[0]) =~? 'fatal' ?
        \ 	"rg --column --line-number --no-heading --color=never --smart-case --hidden " .
        \ 	"--ignore-file ~/.gitignore_global " . shellescape(<q-args>) :
        \	'git grep --no-index --line-number ' . shellescape(<q-args>),
        \ 0,
        \ (systemlist('git rev-parse --show-toplevel')[0]) =~? 'fatal' ?
        \ 	(<bang>0 ? fzf#vim#with_preview('up:60%', '?') : fzf#vim#with_preview('right:50%:hidden', '?')) :
        \ 	(<bang>0 ? fzf#vim#with_preview({ 'dir': systemlist('git rev-parse --show-toplevel')[0] }, 'up:60%', '?') :
        \	fzf#vim#with_preview({ 'dir': systemlist('git rev-parse --show-toplevel')[0] }, 'right:50%:hidden', '?')),
        \ <bang>0)


function! s:GitRootDir(default)
    let l:proot = systemlist('git rev-parse --show-toplevel')[0]
    return l:proot =~? 'fatal' ? a:default : l:proot
endfunction


" Mappings.
nnoremap <C-t><C-t> :<C-u>Files<CR>
nnoremap <C-t><C-n> :<C-u>Files ~<CR>
nnoremap <C-t><C-w> :<C-u>Files ~/workspaces/projects<CR>
nnoremap <C-t><C-p> :<C-u>execute "Files " . <SID>GitRootDir('~/workspaces/projects')<CR>
nnoremap <C-t>0     :<C-u>Files /<CR>
nnoremap <C-t>,     :<C-u>Buffers<CR>
nnoremap <C-t>;     :<C-u>Windows<CR>
nnoremap <C-t><C-u> :<C-u>History<CR>
nnoremap <C-t>l     :<C-u>BLines<CR>
nnoremap <C-t><C-l> :<C-u>Lines<CR>
nnoremap <C-t>]     :<C-u>BTags<CR>
nnoremap <C-t><C-]> :<C-u>Tags<CR>
nnoremap <C-t><C-r> :<C-u>History:<CR>
nnoremap <C-t>/     :<C-u>History/<CR>
nnoremap <C-t>C     :<C-u>Commits<CR>
nnoremap <C-t>c     :<C-u>BCommits<CR>
nnoremap <C-t>g     :<C-u>GFiles?<CR>
nnoremap <C-t><C-g> :<C-u>GFiles<CR>
nnoremap <C-t><C-f> :<C-u>Rg<CR>
nnoremap <C-t><C-m> :<C-u>Marks<CR>
nnoremap <C-t>m     :<C-u>Maps<CR>


" Global line completion with Ripgrep (not just open buffers).
inoremap <expr> <M-l> fzf#vim#complete(fzf#wrap({
    \ 'prefix':  '^.*$',
    \ 'source': 'rg -n ^ --color always',
    \ 'options': '--tiebreak=index --ansi --delimiter : --nth 3.. --tabstop=1',
    \ 'reducer': { lines -> join(split(lines[0], ':\zs')[2:], '') }}))


" Fugitive commands {{{1
nnoremap <C-g>b  :<C-u>Git branch -v<CR>
nnoremap <C-g>cc :<C-u>Gcommit %<CR>
nnoremap <C-g>ca :<C-u>Gcommit --amend %<CR>
nnoremap <C-g>C  :<C-u>Git checkout master<CR>
nnoremap <C-g>;C :<C-u>execute "let a = ' '"<CR>:Git checkout<C-r>=a<CR>
nnoremap <C-g>d  :<C-u>Gdiffsplit<CR>
nnoremap <C-g>;d :<C-u>execute "let a = ' '"<CR>:Gdiffsplit<C-r>=a<CR>
nnoremap <C-g>D  :<C-u>Gdiff master<CR>
nnoremap <C-g>r  :<C-u>Gdelete %<CR>
nnoremap <C-g>;r :<C-u>execute "let a = ' '"<CR>:Gdelete<C-r>=a<CR>
nnoremap <C-g>;g :<C-u>execute "let a = ' '"<CR>:Git<C-r>=a<CR>
nnoremap <C-g>B  :<C-u>Gbrowse<CR>
nnoremap <C-g>h  :<C-u>Git rev-parse --short origin/master<CR>
nnoremap <C-g>l  :<C-u>0Glog<CR>
nnoremap <C-g>L  :<C-u>Glog --<CR>
nnoremap <C-g>v  :<C-u>GV<CR>
nnoremap <C-g>V  :<C-u>GV!<CR>
nnoremap <C-g>;m :<C-u>execute "let a = ' '"<CR>:Gmove<C-r>=a<CR>
nnoremap <C-g>p  :<C-u>Gpush<CR>
nnoremap <C-g>;p :<C-u>execute "let a = ' '"<CR>:Gpush<C-r>=a<CR>
nnoremap <C-g>P  :<C-u>Gpush --set-upstream origin master<CR>
nnoremap <C-g>;R :<C-u>execute "let a = ' '"<CR>:Grebase<C-r>=a<CR>
nnoremap <C-g>s  :<C-u>Gstatus<CR>
nnoremap <C-g>a  :<C-u>Gwrite<CR>

" Remap original Ctrl-g mapping.
nmap <C-g><G-g> <G-g>


" vim-gdiff {{{2
if exists('g:loaded_fugitive')
    nnoremap ]r :cnext<CR>:Gdiffsplit master<CR>
    nnoremap [r :cprevious<CR>:Gdiffsplit master<CR>
    nnoremap ]R :clast<CR>:Gdiffsplit master<CR>
    nnoremap [R :cfirst<CR>:Gdiffsplit master<CR>
endif


" cosco {{{1
autocmd FileType * nmap <silent> <Leader>; <Plug>(cosco-commaOrSemiColon)
autocmd FileType * imap <silent> <C-x>; <c-o><Plug>(cosco-commaOrSemiColon)


" TODO: plugin that prompts you to forcefully quit or save and quit. {{{1
function! s:CloseWindow()
    let l:close = winnr('$') >? 1 ? 'close' : 'quit'
    execute l:close
endfunction
nnoremap <silent> <C-q> :<C-u>call <SID>CloseWindow()<CR>

" TODO: plugin that prompts you for save type based on file owner/permissions.
" Save the buffer.
nnoremap Zu :<C-u>update<CR>
" Save all buffers.
nnoremap Zwa :<C-u>confirm wall<CR>
nnoremap Zwq :<C-u>confirm wqall<CR>
nnoremap Zqa :<C-u>confirm qall<CR>
nnoremap <silent> ZqA :<C-u>quitall!<CR>


" vi:foldmethod=marker foldlevel=0 textwidth=90 shiftwidth=4 tabstop=4 softtabstop=4:
