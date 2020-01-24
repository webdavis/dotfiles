set runtimepath^=~/.vim
set runtimepath+=~/.vim/after
let &packpath = &runtimepath
source ~/.vimrc
set nosecure
let g:vimtex_compiler_progname = 'nvr'
