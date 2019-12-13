if !exists('loaded_nerd_tree') || exists('g:loaded_nerdree_live_preview') 
    finish
endif
let g:loaded_nerdree_live_preview = 1

" Save the users compatible-options so that they may be restored later.
let s:save_cpoptions = &g:cpoptions
set cpoptions&vim

call NERDTreeAddKeyMap({ 'key': '[p', 'callback': 'NERDTreeLivePreview', 'quickhelpText': 'preview' })

function! NERDTreeLivePreview()
    " Get the path of the item under the cursor if possible:
    let l:file = g:NERDTreeFileNode.GetSelected()

    if l:file == {}
        return
    else
        execute 'pedit ' . file.path.str()
    endif
endfunction

let &cpoptions = s:save_cpoptions
unlet! s:save_cpoptions
