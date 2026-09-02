local chars = {
  ";",
  ",",
  ".",
  ":",
}

local keys = {}

for _, c in ipairs(chars) do
  table.insert(keys, {
    "<leader>" .. c,
    function()
      require("blink.chartoggle").toggle_char_eol(c)
    end,
    mode = { "n", "v" },
    desc = "Toggle " .. c .. " at end-of-line",
  })
end

return {
  "saghen/blink.nvim",
  build = "cargo build --release",
  lazy = false,
  opts = {
    chartoggle = { enabled = true },
    tree = { enabled = false },
  },
  keys = keys,
}
