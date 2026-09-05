return {
  "saecki/live-rename.nvim",
  -- The `keys` entries carry the mappings themselves. lazy.nvim installs the
  -- placeholder from each entry at startup and the real mapping from the same
  -- entry once the plugin loads, so there is one declaration per key rather
  -- than one here and another in `config`.
  keys = {
    {
      "<leader>rn",
      function()
        require("live-rename").rename()
      end,
      desc = "Live Rename: normal mode",
    },
    {
      "<leader>rN",
      -- `live_rename.map(opts)` returns `function() rename(opts) end`, so this
      -- is that same call without building the closure first.
      function()
        require("live-rename").rename({ text = "", insert = true })
      end,
      desc = "Live Rename: insert mode",
    },
  },
}
