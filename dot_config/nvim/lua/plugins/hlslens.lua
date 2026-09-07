return {
  "kevinhwang91/nvim-hlslens",
  lazy = false,
  config = function()
    require("hlslens").setup()

    -- stylua: ignore start
    map({ mode = "n", lhs = "n", rhs = [[<Cmd>execute('normal! ' . v:count1 . 'n')<CR><Cmd>lua require('hlslens').start()<CR>]], desc = "Next Search Result (hlslens)", sequence = true })
    map({ mode = "n", lhs = "N", rhs = [[<Cmd>execute('normal! ' . v:count1 . 'N')<CR><Cmd>lua require('hlslens').start()<CR>]], desc = "Prev Search Result (hlslens)", sequence = true })
    map({ mode = "n", lhs = "*", rhs = [[*<Cmd>lua require('hlslens').start()<CR>]], desc = "Search word under cursor (hlslens)", sequence = true })
    map({ mode = "n", lhs = "#", rhs = [[#<Cmd>lua require('hlslens').start()<CR>]], desc = "Search word under cursor backwards (hlslens)", sequence = true })
    map({ mode = "n", lhs = "g*", rhs = [[g*<Cmd>lua require('hlslens').start()<CR>]], desc = "Search word under cursor (partial, hlslens)", sequence = true })
    map({ mode = "n", lhs = "g#", rhs = [[g#<Cmd>lua require('hlslens').start()<CR>]], desc = "Search word under cursor backwards (partial, hlslens)", sequence = true })
  end,
}
