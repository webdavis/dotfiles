return {
  "kevinhwang91/nvim-hlslens",
  lazy = false,
  config = function()
    require("hlslens").setup()

    -- These two rows are hlslens's own documented composition (its README's
    -- minimal configuration) with one substitution: the literal `n` and `N`
    -- become the vim-galore direction-stable selection `config/keymaps.lua`
    -- already uses, plus the `zv` that opens a fold around the landing line.
    -- `n` is then always forward and `N` always backward, whichever way the
    -- search was started, and the lens still starts.
    --
    -- The substitution is what makes the two files agree. hlslens re-maps
    -- NORMAL mode only, and it loads after `config/keymaps.lua` has run, so a
    -- literal `n` here left `n` reversed after a `?` search while the visual and
    -- operator-pending `n` that keymaps.lua owns stayed forward: `n` and `dn`
    -- went opposite ways.
    --
    -- `'Nn'[v:searchforward]` is a subscript, which binds tighter than the `.`
    -- concatenation around it, so it is the character that is concatenated
    -- rather than the whole joined string being indexed.
    -- stylua: ignore start
    map({ mode = "n", lhs = "n", rhs = [[<Cmd>execute('normal! ' . v:count1 . 'Nn'[v:searchforward] . 'zv')<CR><Cmd>lua require('hlslens').start()<CR>]], desc = "Next Search Result (hlslens)", sequence = true })
    map({ mode = "n", lhs = "N", rhs = [[<Cmd>execute('normal! ' . v:count1 . 'nN'[v:searchforward] . 'zv')<CR><Cmd>lua require('hlslens').start()<CR>]], desc = "Prev Search Result (hlslens)", sequence = true })
    map({ mode = "n", lhs = "*", rhs = [[*<Cmd>lua require('hlslens').start()<CR>]], desc = "Search word under cursor (hlslens)", sequence = true })
    map({ mode = "n", lhs = "#", rhs = [[#<Cmd>lua require('hlslens').start()<CR>]], desc = "Search word under cursor backwards (hlslens)", sequence = true })
    map({ mode = "n", lhs = "g*", rhs = [[g*<Cmd>lua require('hlslens').start()<CR>]], desc = "Search word under cursor (partial, hlslens)", sequence = true })
    map({ mode = "n", lhs = "g#", rhs = [[g#<Cmd>lua require('hlslens').start()<CR>]], desc = "Search word under cursor backwards (partial, hlslens)", sequence = true })
  end,
}
