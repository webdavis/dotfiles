return {
  {
    "catppuccin/nvim",
    dependencies = {
      "folke/snacks.nvim",
    },
    name = "catppuccin",
    priority = 1000,
    opts = {
      flavour = "macchiato", -- latte, frappe, macchiato, mocha
      background = { -- :h background
        light = "latte",
        dark = "macchiato",
      },
      transparent_background = false, -- disables setting the background color.
      lsp_styles = {
        underlines = {
          errors = { "undercurl" },
          hints = { "undercurl" },
          warnings = { "undercurl" },
          information = { "undercurl" },
        },
      },
      integrations = {
        aerial = true,
        alpha = true,
        blink_cmp = { style = "bordered" },
        cmp = true,
        dashboard = true,
        flash = true,
        fzf = true,
        gitsigns = true,
        grug_far = true,
        harpoon = true,
        headlines = true,
        illuminate = { enabled = true, lsp = false },
        indent_blankline = { enabled = true },
        leap = true,
        lsp_trouble = true,
        markview = true,
        mason = true,
        mini = true,
        navic = { enabled = true, custom_bg = "lualine" },
        neogit = true,
        neotest = true,
        neotree = true,
        noice = true,
        overseer = true,
        notify = true,
        snacks = { enabled = true, indent_scope_color = "lavender" },
        telescope = { enabled = true },
        treesitter_context = true,
        which_key = true,
      },
      dim_inactive = { enabled = true, shade = "light", percentage = 0.35 },
    },
    config = function(_, opts)
      require("catppuccin").setup(opts)

      vim.cmd.colorscheme("catppuccin")
    end,
  },
  {
    "akinsho/bufferline.nvim",
    version = "*",
    dependencies = "nvim-tree/nvim-web-devicons",
    -- optional = true,
    opts = function(_, opts)
      if (vim.g.colors_name or ""):find("catppuccin") then
        opts.highlights = require("catppuccin.special.bufferline").get_theme()
      end
    end,
  },
  {
    "sontungexpt/witch-line",
    dependencies = {
      "nvim-tree/nvim-web-devicons",
    },
    lazy = false,
    opts = {},
  },
  {
    "Bekaboo/deadcolumn.nvim",
    config = function()
      require("deadcolumn").setup({
        scope = "buffer",
        modes = function(mode)
          return mode:find("^[nictRss\x13]") ~= nil
        end,
        extra = {
          follow_tw = "+1",
        },
      })
    end,
  },
  {
    "OXY2DEV/helpview.nvim",
    lazy = false,
  },
}
