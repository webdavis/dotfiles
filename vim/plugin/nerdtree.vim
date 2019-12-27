" Automatically show hidden files in NERDTree.
let g:NERDTreeShowHidden = 1

" Tells the NERD tree whether to display line numbers in the tree window.
let g:NERDTreeShowLineNumbers = 1

" Increase the default buffer width.
let g:NERDTreeWinSize = 40

" Set the Default to the right side the screen.
let g:NERDTreeWinPos = 'left'

" Disables the display of '?' text and 'Bookmarks' label.
let g:NERDTreeMinimalUI = 1

" Enables returning from the NERDTree window with '^#'.
let g:NERDTreeCreatePrefix = 'silent keepalt keepjumps'

" Autodelete buffers that have been deleted with the context menu.
let g:NERDTreeAutoDeleteBuffer = 1

" Change where bookmarks are stored. The default location is ~/.NERDTreeBookmarks.
let NERDTreeBookmarksFile = "~/.vim/NERDTreeBookmarks"

" If a NERDTree buffer is already open then share it with one in the current buffer.
autocmd BufEnter nerdtree NERDTreeMirror | ReadBookmarks

" Attempt to land on last edited file when opening NERDTree in the current buffer.
" Credit: Wincent.
if has('autocmd')
    augroup NERDTree_Extension
        autocmd!
        autocmd User NERDTreeInit call nerdtree_extension#AttemptSelectLastFile()
    augroup END
endif


" Mappings. {{{1

" Toggles NERDTree view.
" nnoremap <silent> <C-w><C-n> :NERDTreeToggle<CR>

" Moves the cursor to NERDTree window.
" nnoremap <silent> <C-w><C-j> :NERDTreeFocus<CR>

" Finds the current file in NERDTree.
" nnoremap <silent> <C-w><C-f> :NERDTreeFind<CR>

" Improved nameescape() function.
function! s:Fnameescape(file)
    if exists('*fnameescape')
        return fnameescape(a:file)
    else
        return escape(a:file, " \t\n*?[{`$\\%#'\"|!<")
    endif
endfunction

" Browse the file directory using '_' like in vim-vinegar.
nnoremap <silent> - :<C-u>silent edit <C-r>=empty(<SID>Fnameescape(expand('%'))) ? '.' : <SID>Fnameescape(fnamemodify(expand('%'), ':p:h'))<CR><CR>
