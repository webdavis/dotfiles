" Maintainer:   Stephen A. Davis <stephen@webdavis.io>
" License:      Unlicense
" Repository:   https://github.com/webdavis/dotfiles.git
" Description:  Vim/Neovim configuration. (Mappings are in ~/.vim/plugin/mappings.vim.)

" Options {{{1

" Use Vim settings, rather than Vi settings. This should be first.
if &compatible && !has('nvim') | set nocompatible | endif

" When started as 'evim', evim.vim provisions the settings.
if v:progname =~? 'evim' | finish | endif

" if has('vms') | set nobackup | else | set backup | endif

set signcolumn=yes
set cmdheight=2

" Turn on mouse for all modes, and enable during prompts such as Shell command output.
if has('mouse') | set mouse=ar | endif

" Set the default textwidth and indicator.
let &textwidth=90 | let &colorcolumn=&textwidth + 1

" Allow backspace over everything in insert mode.
set backspace=indent,eol,start

" Enable clipboard.
set clipboard=unnamed

" Do not hide concealed text defined by the `Conceal` hl-group.
set conceallevel=0

" Use the number column for wrapped lines.
set cpoptions+=n

" Set the fileformat so that Vim will be written without CR and ^M characters.
set fileformat=unix

" See :help fo-table
set formatoptions+=tn
set formatoptions-=o
if v:version > 703 || v:version == 703 && has('patch541') | set formatoptions+=j | endif

set tags+=tags;/

set encoding=utf-8

" Search behavior.
set ignorecase incsearch

" Prevent screen from redrawing while executing macros.
set lazyredraw

" Set visual selection with cursor over the area to be manipulated.
set selection=inclusive

set completeopt=longest,menuone,noinsert,noselect
set omnifunc=syntaxcomplete#Complete
setglobal completefunc=syntaxcomplete#Complete

" Specify offset from the window border when scrolling.
set scrolloff=3

" If fish shell then propagate to bash shell.
if &shell =~# 'fish$' && (v:version < 704 || v:version == 704 && !has('patch276'))
    set shell=/bin/bash
endif

" Window split behavior.
set splitright

" Set the characters used to separate windows.
set fillchars+=vert:│,fold:―

" Display ↪ at the beginning of a wrapped line, and → when nowrap is set.
let &showbreak = ' +'
setglobal listchars=tab:>\ ,trail:-,extends:→,precedes:<

" Highlight the row and columns where the cursor is.
set cursorline cursorcolumn

" Set the textwidth and the indicator at (textwidth + 1).
set textwidth=90 colorcolumn=+1

" Set {{{ }}} as the opening/closing markers.
set foldmethod=marker
set diffopt+=vertical

set sessionoptions+=unix,slash

" Override ignorecase option if search pat contains uppercase letters.
set smartcase

" Change the amount of indentation for line continuation.
let g:vim_indent_cont = &shiftwidth

" Automatically set indent using C-style standards.
set autoindent
filetype plugin indent on
" See https://stackoverflow.com/questions/2063175/ (fixes annoying start-of-line commenting behavior).
inoremap # X#

" Do not wrap lines by default.
" Wrapped lines will be indented the same amount as the beginning of that line.
" Wrap long lines at a character in 'breakat' rather than the last character that fits on the screen.
set nowrap breakindent linebreak

" Turn on search wrapping.
set wrapscan

" Tabs settings:
" - tabstop adjusts tab size.
" - softtabstop adjusts tab size for editing tasks.
" - expandtab expands tabs to spaces.
" - smarttab inserts blanks according to shiftwidth at beginning of line.
set tabstop=8 shiftwidth=4 softtabstop=4
set expandtab smarttab

" Round to nearest multiple of 4.
set shiftround

" Show matching enclosure for 3 seconds: [],(),{},"",'',<>.
set showmatch matchtime=3

" Sets the mapped key timeout to 3 seconds, and the key code timeout to essentially 0.
set ttimeout timeoutlen=3000 ttimeoutlen=1

" Reload files that have been changed outside of the current session.
setglobal autoread

" Autowrite the file as it's modified.
setglobal autowrite

" Set the backup file location, creating the location if it doesn't exist.
set backupdir=~/.vim/tmp/backup
if !isdirectory(expand(&backupdir))
    call mkdir(expand(&backupdir), 'p')
endif

" Set the swap file location.
set directory=~/.vim/tmp/swap
if !isdirectory(expand(&directory))
    call mkdir(expand(&directory), 'p')
endif

" Set the undo file location.
if exists('+undofile')
    set undofile
    set undodir=~/.vim/tmp/undo
    if !isdirectory(expand(&undodir))
        call mkdir(expand(&undodir), 'p')
    endif
endif

" List of flags specifying which commands wrap text.
set whichwrap=b,s,<,>,[,]

" Show a highlighted menu of possible command mode completions.
set wildmenu

" Shows possible completions above command line using auto completion.
set showcmd

" list:longest - when more than one match, list all matches and complete until longest common string.
" list:full - when more than one match, list all matches and complete first match.
set wildmode=full

" Show the cursor position all the time.
set ruler

" The number of modelines that are checked for set commands.
setlocal modelines=1

" When a buffer is hidden it means that it is not attached to a window. This will allow
" you to close windows without saving them, while the buffer is still open within the Vim address space.
set hidden

" Decrease updatetime for CursorHold and CursorHoldI autocommands.
set updatetime=1000

" Number lines relative from cursor position.
set number relativenumber

" Set the language used to check against spelling.
set spelllang=en_us

" The number of commands and search patterns that are remembered.
set history=2000

" Enable project specific vimrc
set exrc
" This prevents :autocmd, shell, and write commands from being run inside project-specific
" .vimrc files unless they're owned by me.
set secure

" See :help usr_06.txt for syntax setup.
if &t_Co > 1 || has('gui_running')
    syntax enable vim
    set hlsearch
endif

" Add support for Git to the statusline.
set statusline=\ %{fugitive#statusline()}\ %<%f\ \ \|\sw=%{&sw}\ ts=%{&ts}%h%m%r%=%-2.(%l,%c%V%)\ \|\ %P\

" Autocommands {{{1

" If only 2 windows left, NERDTree and Tag_List, close vim or current tab
function! s:CloseAddons()
    for w in range(1, winnr('$'))
        if bufname(winbufnr(w)) !~? 'Tagbar\|NERD_tree_\|coc-explorer' && getbufvar(winbufnr(w), "&buftype") !=? "quickfix"
            return
        endif
    endfor

    if tabpagenr('$') ==? 1
        execute 'quitall'
    else
        execute 'tabclose'
    endif
endfunction

augroup tagbar_custom
    autocmd!
    autocmd WinEnter * silent! call <SID>CloseAddons()
augroup end

augroup vimrc
    autocmd!
    " Return to last edit position if when opening files.
    autocmd BufReadPost * if line("'\"") > 1 && line("'\"") <= line("$") | exe "normal! g`\"" | endif
augroup END

augroup filetypes
    autocmd!
    autocmd BufNewFile,BufRead *.doc setlocal filetype=text
	autocmd FileType json syntax match Comment +\/\/.\+$+
augroup END

augroup vagrant
    autocmd!
    " Highlight Vagrantfile as ruby code.
    autocmd BufRead,BufNewFile Vagrantfile set filetype=ruby
augroup END

" View man pages using :Man command when in Vim.
if !exists('tnoremap') | runtime! ftplugin/man.vim | endif

" Install Plug if it's not installed.
if empty(glob('~/.vim/autoload/plug.vim'))
    silent !curl -fLo ~/.vim/autoload/plug.vim --create-dirs
            \ https://raw.githubusercontent.com/junegunn/vim-plug/master/plug.vim
    autocmd VimEnter * PlugInstall --sync | source $MYVIMRC
endif

" Plugin instantiations. {{{1
call plug#begin('~/.vim/plugged')
    Plug 'kshenoy/vim-signature'                           " Displays marks before line number.
    Plug 'henrik/vim-indexed-search'                       " Displays 'Match #N out of K /regex/' below command line.
    Plug 'szw/vim-maximizer'                               " Provides a more fluent way to toggle maximizing a Vim window.
    Plug 'mbbill/undotree'                                 " Provides a window for visualizing your undo tree.
    Plug 'mhinz/vim-signify'                               " Provides indicators for changes in code managed by a version control system.
    Plug 'christoomey/vim-tmux-navigator'                  " Provides joint Vim and Tmux navigation keys.
    Plug 'unblevable/quick-scope'                          " Provides intelligent target highlighting for the 'f', 'F', 't', and 'T' keys.
    Plug 'junegunn/fzf', { 'dir': '~/workspaces/tools/fzf', 'do': './install --all' } | Plug 'junegunn/fzf.vim' " Install a fuzzy finder. fzf works as a command line tool as well.
    Plug 'scrooloose/nerdtree'                             " Installs a file browser.
    Plug 'Yggdroot/indentLine'                             " Provides thin vertical lines when indented with spaces.
    Plug 'ntpeters/vim-better-whitespace'                  " Provides trailing whitespace highlighting.
    Plug 'tpope/vim-scriptease'                            " Provides useful commands for editing Vimscript.
    Plug 'tpope/vim-repeat'                                " Provides a way for plugins to tap into repeat with . (dot).
    Plug 'tpope/vim-abolish'                               " Provides an abbreviation engine.
    Plug 'tpope/vim-rsi'                                   " Provides readline-like mappings for Insert mode.
    Plug 'tpope/vim-endwise'                               " Autocloses statements: if, for, while, et cetera.
    Plug 'tpope/vim-obsession'                             " Autosaves vim sessions.
    " Plug 'tpope/vim-sleuth'                                " Provides and engine that heuristically adjusts 'shiftwidth' and 'expandtab'.
    Plug 'tpope/vim-ragtag'                                " Provides HTML (and it's many variants) mappings.
    Plug 'mattn/emmet-vim'                                 " Provides HTML/CSS workflow improvements.
    Plug 'tpope/vim-characterize'                          " Press 'ga' to print the unicode value of the character under the cursor.
    Plug 'tpope/vim-tbone'                                 " Provides access to Tmux from Vim-cli.
    Plug 'tpope/vim-fugitive'                              " Provides Git support.
    Plug 'oguzbilgic/vim-gdiff'
    Plug 'junegunn/gv.vim'                                 " Provides the :GV command for opening all Git commits in a new tab.
    Plug 'KabbAmine/zeavim.vim'                            " Zeal is a desktop application that provides access to many different code libraries; use <Leader>z to search within Zeal.
    Plug 'LnL7/vim-nix'                                    " Nix syntax and filetype detection.
    Plug 'tpope/vim-eunuch'                                " Provides UNIX Shell commands such as :Mkdir, Delete, Move, etc.
    Plug 'tpope/vim-rhubarb'                               " Provides GitHub support using `hub`. Requires `curl`.
    Plug 'tpope/vim-surround'                              " Provides enclosure mappings such as ci)] and ysiw<!-- <strong> -->.
    Plug 'tpope/vim-unimpaired'                            " Provides some useful mappings for things like option toggling with yo[character].
    Plug 'tpope/vim-projectionist'                         " Provides granular project configuration.
    Plug 'tpope/vim-dispatch'                              " https://vimeo.com/63116209 ...this is how all software should be introduced.
    Plug 'janko/vim-test'                                  " Provides strategies for program execution.
    Plug 'w0rp/ale'                                        " Enables an asynchronous linting engine.
    Plug 'mhinz/vim-grepper'                               " Provides help with Vim's grep, and adds support for most grep-like tools.
    " Plug 'lervag/vimtex'                                   " Provides enhanced LaTeX features like continuous mode for auto compile.
    Plug 'lfilho/cosco.vim'                                " Vim colon and semicolon insertion bliss.
    Plug 'othree/xml.vim'                                  " Adds some useful stuff for editing .xml files.
    Plug 'godlygeek/tabular'                               " Required by plasticboy/vim-markdown.
    Plug 'plasticboy/vim-markdown', { 'for': 'markdown'}   " Tabular provides a mapping to align text. vim-markdown provides better markdown editing.
    Plug 'iamcco/markdown-preview.nvim', { 'do': { -> mkdp#util#install() } } " Markdown live preview.
    Plug 'Vimjas/vim-python-pep8-indent'                   " Provides PEP8 compliant indentation.
    Plug 'heavenshell/vim-pydocstring'                     " Generates Python docstrings: put the cursor on a method and run Pydocstring.
    Plug 'webdavis/vim-xapprentice'                        " A super version of the Apprentice colorscheme.
    Plug 'webdavis/vim-setswitch'                          " Provides user extensible buffer local capabilities for all Vim options.
    Plug 'AndrewRadev/splitjoin.vim'                       " Provides the gS and gJ mappings for splitting or joining code constructs.
    Plug 'wellle/targets.vim'                              " Provides advanced text object selection.
    Plug 'dhruvasagar/vim-table-mode'                      " Provides an automatic table creator.
    Plug 'SirVer/ultisnips'                                " Snippet engine.
    Plug 'honza/vim-snippets'                              " Snippet databases.
    Plug 'tomtom/tcomment_vim'                             " Enables easy comment toggling with gcc mappings.
    Plug 'Shougo/neco-vim'                                 " Adds VimL completion support.
    Plug 'neoclide/coc-neco'                               " Access neco-vim using coc.nvim.
    if has('nvim') | Plug 'neoclide/coc.nvim', {'do': 'yarn install --frozen-lockfile'} | endif
    Plug 'ludovicchabant/vim-gutentags' | Plug 'skywind3000/gutentags_plus' " Automated tag generation and syntax highlighting in Vim.
    Plug 'majutsushi/tagbar'                               " Provides ctags and psuedo tags for code.
call plug#end()
if !has('nvim') | runtime macros/matchit.vim | endif       " Provides mapping for jumping between enclosure characters (builtin to Neovim).


" Plugins Settings {{{1

" christoomey/vim-tmux-navigator {{{2
" Disable tmux navigator when zooming the Vim pane.
let g:tmux_navigator_disable_when_zoomed = 1


" Shougo/neosnippet.vim {{{2
" Load snipMat snippets by default.
let g:neosnippet#enable_snipmate_compatibility = 1


" Gutentags {{{2

" Enable gtags module.
let g:gutentags_modules = ['ctags', 'gtags_cscope']

" By default Gutentags will identify project roots by VCS directories such as .git and
" .svn. If you aren't using either of those then create an empty .root file in your
" desired project root.
let g:gutentags_project_root = ['.root']

" Generate databases in the cache directory, preventing gtags files polluting the project.
let g:gutentags_cache_dir = expand('~/.cache/tags')

" change focus to quickfix window after search (optional).
let g:gutentags_plus_switch = 1


" w0rp/ale {{{2

" Check for errors when the buffer is saved.
let g:ale_lint_on_text_changed = 'always'

" Disable ALE history.
let g:ale_history_enabled = 0

" Keep the sign gutter open at all times to prevent the buffer from moving while during
" typing.
let g:ale_sign_column_always = 1
let g:ale_sign_error = 'E'
let g:ale_sign_warning = 'W'
let b:ale_warn_about_trailing_whitespace = 1
let b:ale_warn_about_trailing_blank_lines = 1
let g:ale_python_auto_pipenv = 1
let g:ale_linters = {'python': ['flake8']}
let g:ale_fixers = {'python': ['black']}
let g:ale_python_pycodestyle_options = '--max-line-length=90'
let g:ale_python_flake8_options = '--max-line-length=90'

" Turn ALE off for the following filetypes.
let g:ale_pattern_options = {
\   '.*\.java$': {'ale_enabled': 0},
\   '.*\.py$': {'ale_enabled': 0},
\}

" mhinz/vim-signify {{{2

let g:signify_vcs_list = ['git', 'svn', 'perforce',]
let g:signify_skip_filetype = { 'man': 1, }
let g:signify_sign_add = ''
let g:signify_sign_delete = '-'
let g:signify_sign_delete_first_line = '‾'
let g:signify_sign_change = ''
execute "let g:signify_sign_changedelete = '" g:signify_sign_change . g:signify_sign_delete . "'"
let g:signify_sign_show_count = 1


" mbbill/undotree {{{2

" Saves space.
let g:undotree_ShortIndicators = 1

let g:undotree_SetFocusWhenToggle = 1


" szw/vim-maximizer {{{2

" Turn off default mapping for vim-maximizer.
let g:maximizer_set_default_mapping = 0
let g:maximizer_restore_on_winleave = 1


" Yggdroot/indentLine {{{2

let g:indentLine_fileTypeExclude = ['man', 'help', 'markdown', 'md', 'json']
let g:indentLine_setConceal = 1
let g:indentLine_color_gui = '#626262'
let g:indentLine_bgcolor_gui = 'NONE'
let g:indentLine_color_term = 240
let g:indentLine_bgcolor_term = 'NONE'
" let g:indentLine_color_tty_light = 0
" let g:indentLine_color_dark = 2
let g:indentLine_conceallevel = 1
let g:indentLine_concealcursor = 'icnv'
let g:indentLine_char = '⋮'
let g:indentLine_showFirstIndentLevel = 0


" ntpeters/vim-better-whitespace {{{2

" vim-better-whitespace: blacklist certain filetypes from showing lines ending with whitespace.
let g:better_whitespace_filetypes_blacklist = ['diff', 'unite', 'qf', 'help', 'vim']

" augroup vimrc_better_whitespace
"     autocmd!
"     autocmd BufWritePre * StripWhitespace
" augroup END
let g:strip_whitespace_confirm = 0
let g:strip_whitespace_on_save = 1
let g:strip_whitelines_at_eof = 1

" webdavis/vim-setswitch {{{2

" Toggle these settings upon entering and exiting a window.
let g:setswitch = {
        \ 'all': ['colorcolumn=+1', 'cursorline', 'cursorcolumn', 'relativenumber'],
        \ 'netrw': ['colorcolumn=', 'nocursorline', 'nocursorcolumn'],
        \ 'tagbar': ['colorcolumn=', 'nocursorline', 'nocursorcolumn'],
        \ 'nerdtree': ['colorcolumn=', 'nocursorline', 'nocursorcolumn', 'relativenumber'],
        \ 'man': ['colorcolumn=', 'cursorline', 'cursorcolumn'],
        \ 'help': ['colorcolumn=', 'cursorline', 'cursorcolumn', 'relativenumber'],
        \ 'gitcommit': ['colorcolumn=+1,51', 'nocursorline', 'nocursorcolumn', 'relativenumber'],
        \ }

" Toggle these settings upon entering and exiting insert mode.
let g:setswitch_insert = { 'all': ['cursorline', 'cursorcolumn', 'relativenumber'], }

" Listen for these options to be set.
let g:setswitch_hooks = ['cursorline', 'cursorcolumn', 'relativenumber', 'wrap', 'hlsearch', 'colorcolumn']


" janko/vim-test {{{2

" Make test commands execute using tpope's vim-dispatch.
let test#strategy = 'dispatch_background'

" Automatically run tests when the buffer is written.
augroup vim_test_custom
    autocmd!
    autocmd BufWrite * if test#exists() | TestNearest | endif
augroup END


" plasticboy/vim-markdown {{{2

" Turn off concealing which makes Yggdroot/indentLine usable.
let g:vim_markdown_conceal = 0
let g:tex_conceal = ""
let g:vim_markdown_math = 1
" Disable vim-markdown's folding.
let g:vim_markdown_folding_disabled = 1
" Auto-shrink Toc window if possible.
let g:vim_markdown_toc_autofit = 1
" Turn off automatically inserting new list items.
let g:vim_markdown_auto_insert_bullets = 0
let g:vim_markdown_new_list_item_indent = 0

" Open links in horizontal splits.
let g:vim_markdown_edit_url_in = 'hsplit'


" iamcco/markdown-preview {{{2

" Prevent the preview from closing when a BufEnter event is fired off.
let g:mkdp_auto_close = 0

let g:mkdp_preview_options = {
        \ 'mkit': {},
        \ 'katex': {},
        \ 'uml': {},
        \ 'maid': {},
        \ 'disable_sync_scroll': 0,
        \ 'sync_scroll_type': 'middle',
        \ 'hide_yaml_meta': 1
        \ }

" Forces synchronized scroll have minimal delay. Warning: this will also affect the rate
" at which swap files are written.
set updatetime=100


" neoclide/coc.nvim {{{2


" tpope/vim-projectionist {{{2
let g:projectionist_heuristics = {
        \ "src/main/java/|src/*.java": {
        \       "src/main/java/*.java": {
        \           "type": "source",
        \           "alternate": "src/test/java/{}Test.java"
        \       },
        \       "src/test/java/*Test.java": {
        \           "type": "test",
        \           "alternate": "src/main/java/{}.java"
        \       },
        \       "src/*.java": {
        \           "type": "source",
        \           "alternate": "test/{}Test.java"
        \       },
        \       "test/*Test.java": {
        \           "type": "test",
        \           "alternate": "src/{}.java"
        \       },
        \       "*.java": {"dispatch": "mvn exec:java",
        \           "make": "mvn clean package",
        \           "console": "jshell",
        \           "start": "mvn verify"
        \       }
        \ },
        \ "*.java": {
        \       "*.java": {"dispatch": "java {basename}",
        \           "make": "javac {file}",
        \           "console": "jshell",
        \           "start": "/bin/bash"
        \       }
        \ },
        \ "*.py": {
        \       "*.py": {"dispatch": "python {file}",
        \           "make": "python {file}",
        \           "console": "python",
        \           "start": "/bin/bash"
        \       }
        \ },
        \ "*.js": {
        \       "*": {"dispatch": "node {file}",
        \           "make": "node {file}",
        \           "console": "node",
        \           "start": "/bin/bash"
        \       }
        \ },
        \ "*.mjs": {
        \       "*": {"dispatch": "node {file}",
        \           "make": "nsx {file}",
        \           "console": "node",
        \           "start": "/bin/bash"
        \       }
        \ },
\ }


" KabbAmine/zeavim.vim {{{2
let g:zv_file_types = {
    \ 'scss': 'sass',
	\ 'sh': 'bash',
	\ 'tex': 'latex',
	\ 'py': 'python_3',
	\ }


" vi:fdm=marker fdl=0 tw=90 sw=4 ts=4 sts=4:
