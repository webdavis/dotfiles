-- The range family is on `<leader>S`, because `<leader>s` is the snacks picker group.
-- `on_substitute` sends substituted text to yanky, which owns `y` and `p` here.
return {
  "gbprod/substitute.nvim",
  dependencies = {
    "gbprod/yanky.nvim",
  },
  opts = function()
    return {
      on_substitute = require("yanky.integration").substitute(),
    }
  end,
  keys = {
    -- stylua: ignore start
    { "s",          function() require("substitute").operator() end,        desc = "Substitute: operator" },
    { "ss",         function() require("substitute").line() end,            desc = "Substitute: line" },
    { "S",          function() require("substitute").eol() end,             desc = "Substitute: to end of line" },
    { "s",          function() require("substitute").visual() end,          mode = "x", desc = "Substitute: selection" },
    { "<leader>S",  function() require("substitute.range").operator() end,  desc = "Substitute: over range (operator)" },
    { "<leader>S",  function() require("substitute.range").visual() end,    mode = "x", desc = "Substitute: over range (selection)" },
    { "<leader>SS", function() require("substitute.range").word() end,      desc = "Substitute: over range (word)" },
    { "sx",         function() require("substitute.exchange").operator() end, desc = "Substitute: exchange operator" },
    { "sxx",        function() require("substitute.exchange").line() end,   desc = "Substitute: exchange line" },
    { "sxc",        function() require("substitute.exchange").cancel() end, desc = "Substitute: cancel exchange" },
    { "X",          function() require("substitute.exchange").visual() end, mode = "x", desc = "Substitute: exchange selection" },
    -- stylua: ignore end
  },
}
