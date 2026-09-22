-- The operator keys are upstream's own suggestions. `s`, `S` and visual `s` are
-- free here: nvim-surround takes `ys`, `ds`, `cs` and visual `S`, flash.nvim
-- ships with `char` mode disabled so it never claims `s`, and
-- nvim-various-textobjs maps `iS`/`aS` rather than a bare one.
--
-- The range family is on `<leader>S`, because `<leader>s` is the snacks picker
-- group (23 keys) and an operator there would swallow the whole group.
--
-- `on_substitute` hands the substituted text to yanky, which owns `y`, `p` and
-- `P` in this configuration. Without it a substitute bypasses the yank ring and
-- the history picker on `<leader>p` goes stale.
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
