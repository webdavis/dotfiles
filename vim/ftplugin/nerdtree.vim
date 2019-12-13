" Disable showing listchar attributes.
setlocal nolist

" Disable folding.
if has('folding')
    setlocal nofoldenable
endif

" Move up a directory using '-' like in vim-vinegar. This is set to 'u' by default.
nmap <buffer> <expr> - g:NERDTreeMapUpdir
