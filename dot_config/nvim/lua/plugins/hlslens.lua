return {
  "kevinhwang91/nvim-hlslens",
  lazy = false,
  config = function()
    require("hlslens").setup()

    -- `n` always searches forward and `N` backward, like the visual and operator-pending `n` in
    -- `config/keymaps.lua`. `zv` opens a fold at the match.
    -- stylua: ignore start
    map({ mode = "n", lhs = "n", rhs = [[<Cmd>execute('normal! ' . v:count1 . 'Nn'[v:searchforward] . 'zv')<CR><Cmd>lua require('hlslens').start()<CR>]], desc = "Next Search Result (hlslens)", sequence = true })
    map({ mode = "n", lhs = "N", rhs = [[<Cmd>execute('normal! ' . v:count1 . 'nN'[v:searchforward] . 'zv')<CR><Cmd>lua require('hlslens').start()<CR>]], desc = "Prev Search Result (hlslens)", sequence = true })
    map({ mode = "n", lhs = "*", rhs = [[*<Cmd>lua require('hlslens').start()<CR>]], desc = "Search word under cursor (hlslens)", sequence = true })
    map({ mode = "n", lhs = "#", rhs = [[#<Cmd>lua require('hlslens').start()<CR>]], desc = "Search word under cursor backwards (hlslens)", sequence = true })
    map({ mode = "n", lhs = "g*", rhs = [[g*<Cmd>lua require('hlslens').start()<CR>]], desc = "Search word under cursor (partial, hlslens)", sequence = true })
    map({ mode = "n", lhs = "g#", rhs = [[g#<Cmd>lua require('hlslens').start()<CR>]], desc = "Search word under cursor backwards (partial, hlslens)", sequence = true })
  end,
}
