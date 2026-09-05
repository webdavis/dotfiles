-- `<leader>as` is a GLOBAL map that happens to be bound from `on_attach`, so on a
-- lazy spec it cannot bootstrap its own plugin: lazy loads aerial and re-feeds
-- the key before anything has attached, and `on_attach` has not run to bind it
-- yet, so the picker never opens (measured). Naming this same function in `keys`
-- gives lazy a real mapping to install at load time; `on_attach` still rebinds
-- the identical function afterwards.
local function snacks_picker()
  require("aerial").snacks_picker({
    layout = {
      -- preset = "dropdown",
      -- preview = false,
    },
  })
end

return {
  "stevearc/aerial.nvim",
  -- Every command aerial registers, so one typed at the command line still
  -- works while the plugin is unloaded.
  cmd = {
    "AerialClose",
    "AerialCloseAll",
    "AerialGo",
    "AerialInfo",
    "AerialNavClose",
    "AerialNavOpen",
    "AerialNavToggle",
    "AerialNext",
    "AerialOpen",
    "AerialOpenAll",
    "AerialPrev",
    "AerialToggle",
  },
  -- `<leader>as` is bound by `on_attach` below rather than beside the others,
  -- so it only ever appears once aerial has attached to a buffer. It still
  -- belongs here: without it the key does nothing at all until something else
  -- loads the plugin. `{` and `}` are buffer-local to an aerial-attached
  -- buffer, which means aerial is already loaded by the time they exist.
  keys = {
    { "<leader>at", desc = "Aerial: toggle sidebar (don't focus)" },
    { "<leader>aa", desc = "Aerial: toggle sidebar (don't focus)" },
    { "<leader>aT", desc = "Aerial: toggle sidebar (and focus)" },
    { "<leader>aA", desc = "Aerial: toggle sidebar (and focus)" },
    { "<leader>ao", desc = "Aerial: open sidebar (don't focus)" },
    { "<leader>aO", desc = "Aerial: open sidebar (and focus)" },
    { "<leader>ac", desc = "Aerial: close sidebar" },
    { "<leader>aC", desc = "Aerial: close all sidebars" },
    { "<leader>as", snacks_picker, desc = "Aerial: Snacks picker" },
  },
  dependencies = {
    "nvim-treesitter/nvim-treesitter",
    "nvim-tree/nvim-web-devicons",
    "folke/snacks.nvim",
  },
  config = function()
    local opts = {
      layout = {
        min_width = 20,
      },
      on_attach = function(bufnr)
        map({ mode = "n", lhs = "{", rhs = "AerialPrev", desc = "Aerial: jump to next code object", buffer = bufnr })
        map({ mode = "n", lhs = "}", rhs = "AerialNext", desc = "Aerial: jump to previous code object", buffer = bufnr })

        map({ mode = "n", lhs = "<leader>as", rhs = snacks_picker, desc = "Aerial: Snacks picker" })
      end,
    }

    require("aerial").setup(opts)

    -- stylua: ignore start
    map({ mode = "n", lhs = { "<leader>at", "<leader>aa" }, rhs = "AerialToggle!", desc = "Aerial: toggle sidebar (don't focus)" })
    map({ mode = "n", lhs = { "<leader>aT", "<leader>aA" }, rhs = "AerialToggle", desc = "Aerial: toggle sidebar (and focus)" })
    map({ mode = "n", lhs = "<leader>ao", rhs = "AerialOpen!", desc = "Aerial: open sidebar (don't focus)" })
    map({ mode = "n", lhs = "<leader>aO", rhs = "AerialOpen", desc = "Aerial: open sidebar (and focus)" })
    map({ mode = "n", lhs = "<leader>ac", rhs = "AerialClose", desc = "Aerial: close sidebar" })
    map({ mode = "n", lhs = "<leader>aC", rhs = "AerialCloseAll", desc = "Aerial: close all sidebars" })
  end,
}
