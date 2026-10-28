return {
  {
    "catppuccin/nvim",
    lazy = false,
    dependencies = {
      "folke/snacks.nvim",
    },
    name = "catppuccin",
    priority = 1000,
    opts = {
      flavour = "macchiato",
      background = {
        light = "latte",
        dark = "macchiato",
      },
      transparent_background = false,
      lsp_styles = {
        underlines = {
          errors = { "undercurl" },
          hints = { "undercurl" },
          warnings = { "undercurl" },
          information = { "undercurl" },
        },
      },
      -- Auto-detection stays off, because it creates an empty folder that `:checkhealth lazy` flags.
      -- Each integration is listed by hand instead, and a table entry needs `enabled = true` to count.
      auto_integrations = false,
      integrations = {
        aerial = true,
        alpha = true,
        blink_cmp = { enabled = true, style = "bordered" },
        cmp = true,
        dap = true,
        dap_ui = true,
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
        nvim_surround = true,
        octo = true,
        overseer = true,
        notify = true,
        snacks = { enabled = true, indent_scope_color = "lavender" },
        treesitter_context = true,
        which_key = true,
      },
      dim_inactive = { enabled = true, shade = "light", percentage = 0.35 },
    },
    config = function(_, opts)
      require("catppuccin").setup(opts)

      -- Older catppuccin ships only `catppuccin`, so load whichever name the runtimepath has.
      local installed = vim.fn.getcompletion("catppuccin", "color")
      local name
      for _, candidate in ipairs({ "catppuccin-nvim", "catppuccin" }) do
        if vim.tbl_contains(installed, candidate) then
          name = candidate
          break
        end
      end

      if not name then
        vim.notify("catppuccin: neither `catppuccin-nvim` nor `catppuccin` is installed", vim.log.levels.WARN)
        return
      end

      local ok, err = pcall(vim.cmd.colorscheme, name)
      if not ok then
        vim.notify(("catppuccin: `%s` failed to load: %s"):format(name, err), vim.log.levels.WARN)
      end
    end,
  },
  {
    "nvim-lualine/lualine.nvim",
    lazy = false,
    dependencies = { "nvim-tree/nvim-web-devicons" },
    opts = {
      options = {
        theme = "auto",
        globalstatus = true,
        component_separators = { left = "", right = "" },
        section_separators = { left = "", right = "" },
        disabled_filetypes = { statusline = { "snacks_dashboard" } },
      },
      sections = {
        lualine_a = { "mode" },
        lualine_b = { "branch", "diff" },
        lualine_c = {
          {
            "filename",
            path = 1,
            newfile_status = true,
            symbols = { modified = "●", readonly = "", unnamed = "[no name]" },
          },
          { "diagnostics", sources = { "nvim_diagnostic" } },
        },
        lualine_x = {
          {
            function()
              return require("custom_api.overseer_status").render()
            end,
            cond = function()
              return not require("custom_api.overseer_status").is_idle()
            end,
          },
          {
            function()
              return require("noice").api.status.mode.get()
            end,
            cond = function()
              return package.loaded["noice"] and require("noice").api.status.mode.has()
            end,
          },
          {
            function()
              local names = {}
              for _, client in ipairs(vim.lsp.get_clients({ bufnr = 0 })) do
                table.insert(names, client.name)
              end
              return table.concat(names, " ")
            end,
            icon = "",
            cond = function()
              return #vim.lsp.get_clients({ bufnr = 0 }) > 0
            end,
          },
          "searchcount",
          "filetype",
        },
        lualine_y = { "progress" },
        lualine_z = { "location" },
      },
      tabline = {
        lualine_a = { { "buffers", mode = 2, use_mode_colors = true } },
        lualine_z = {
          {
            "tabs",
            mode = 1,
            cond = function()
              return #vim.api.nvim_list_tabpages() > 1
            end,
          },
        },
      },
      extensions = {
        "aerial",
        "fugitive",
        "lazy",
        "man",
        "mason",
        "nvim-dap-ui",
        "oil",
        "overseer",
        "quickfix",
        "trouble",
      },
    },
  },
  {
    "Bekaboo/deadcolumn.nvim",
    lazy = false,
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
