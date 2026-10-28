-- `<leader>ad` sits under herdr-nvim's `<leader>a` prefix because annotations land in herdr-nvim's store.
-- It calls `line()` through `custom_api.try` so its errors are reported like every other keymap's.
return {
  "webdavis/herdr-nvim-annotate-extension.nvim",
  commit = "a7713d5e857c8c8c41ea6cd707eda07c2d082c9c",
  dependencies = { "ChmaraX/herdr-nvim" },
  keys = {
    {
      "<leader>ad",
      function()
        require("custom_api.try")(function()
          return require("herdr-nvim-annotate-extension").line()
        end, { label = "herdr-nvim-annotate-extension.line" })
      end,
      desc = "herdr-nvim: annotate line with diagnostic and blame",
    },
  },
}
