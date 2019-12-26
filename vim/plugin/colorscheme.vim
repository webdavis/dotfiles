" Colorscheme settings.

" Dark colorscheme {{{1
let g:xapprentice_dark_bold_group = [
        \'CursorLineNr',
	\'Directory',
	\'EndOfBuffer',
        \'Function',
        \'IncSearch',
        \'ModeMsg',
        \'MoreMsg',
        \'NonText',
        \'SignifySignDelete',
	\'StatusLine',
        \'Title',
        \'Todo',
	\]
let g:xapprentice_dark_italic_group = ['Comment',]
let g:xapprentice_dark_underline = 1
let g:xapprentice_dark_underline_group = ['VisualNOS']
let g:xapprentice_dark_standout_group = [
        \'Cursor',
        \'iCursor',
        \'lCursor',
        \]
let g:xapprentice_dark_reverse_group = ['ColorColumn']

" Light colorscheme {{{1
let g:xapprentice_light_bold_group = [
	\'CursorLineNr',
        \'Directory',
        \'EndOfBuffer',
        \'Function',
        \'IncSearch',
        \'ModeMsg',
        \'MoreMsg',
        \'NonText',
	\'SignifySignDelete',
	\'StatusLine',
        \'StatusLine',
        \'TablineSel',
        \'Title',
        \'Todo',
	\]
let g:xapprentice_light_italic_group = ['Comment',]
let g:xapprentice_light_underline = 1
let g:xapprentice_light_underline_group = ['VisualNOS']
let g:xapprentice_light_standout_group = [
        \'Cursor',
        \'iCursor',
        \'lCursor',
        \]
let g:xapprentice_light_reverse_group = ['ColorColumn']

" Plugins {{{1
" Activate xapprentice's support for signify.
let g:xapprentice_signify = 1

" Activate xapprentice's support for ALE.
let g:xapprentice_ale = 1
" }}}

" Set Vim-specific sequences for RGB colors. See https://github.com/vim/vim/issues/993.
let &t_8f = "\<Esc>[38;2;%lu;%lu;%lum"
let &t_8b = "\<Esc>[48;2;%lu;%lu;%lum"

" If TRUECOLOR is available then enable it.
if has('nvim') && has("termguicolors") | set termguicolors | endif

" Set xapprentice as the colorscheme.
colorscheme xapprentice_dark
