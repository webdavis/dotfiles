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
      -- Upstream 058e83d turned auto-detection on by default; its vim.pack.get()
      -- call leaves an empty site/pack/core that :checkhealth lazy then flags.
      auto_integrations = false,
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
        treesitter_context = true,
        which_key = true,
      },
      dim_inactive = { enabled = true, shade = "light", percentage = 0.35 },
    },
    config = function(_, opts)
      require("catppuccin").setup(opts)

      -- Same theme under two names across a pin bump: the old pin only ships
      -- `catppuccin`, so the new name fails there. Editing lazy-lock.json installs
      -- nothing until `:Lazy restore`, so the window between an apply and that
      -- restore is real, and lazy reports the failure through `vim.notify`, which
      -- makes it silent at startup.
      if not pcall(vim.cmd.colorscheme, "catppuccin-nvim") and not pcall(vim.cmd.colorscheme, "catppuccin") then
        vim.notify("catppuccin: neither `catppuccin-nvim` nor `catppuccin` loaded", vim.log.levels.WARN)
      end
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
    -- A function, not a table: the default component list is a witch-line module,
    -- so it can only be required once the plugin is on the runtimepath.
    opts = function()
      -- Overseer task counts, appended to witch-line's own default components
      -- rather than replacing them. Overseer's third-party doc ships recipes for
      -- lualine and heirline only, so this is the same idea in witch-line's
      -- component shape: one count per status, hidden when there is nothing to
      -- report.
      local statuses = { "RUNNING", "FAILURE", "SUCCESS", "CANCELED" }
      local icons = { RUNNING = "󰑮", FAILURE = "󰅚", SUCCESS = "󰄴", CANCELED = "" }

      ---@return table<string, integer>
      local function counts()
        -- `package.loaded` rather than require: overseer lazy-loads itself, and
        -- asking for it here would drag it in on every statusline redraw.
        if not package.loaded["overseer"] then
          return {}
        end
        local by_status = {}
        -- unique collapses repeat runs of one task; wrapped background jobs stay
        -- out, which is the same set the task list shows.
        for _, task in ipairs(require("overseer.task_list").list_tasks({ unique = true })) do
          by_status[task.status] = (by_status[task.status] or 0) + 1
        end
        return by_status
      end

      local overseer_component = {
        id = "overseer.tasks",
        padding = { left = 1, right = 1 },
        -- Task status changes on its own schedule, with no event to hook, so
        -- this polls rather than waiting to be told.
        timing = 1000,
        hidden = function()
          return vim.tbl_isempty(counts())
        end,
        update = function()
          local by_status = counts()
          local parts = {}
          for _, status in ipairs(statuses) do
            if by_status[status] then
              table.insert(parts, ("%s %d"):format(icons[status], by_status[status]))
            end
          end
          return table.concat(parts, " ")
        end,
      }

      local components = vim.deepcopy(require("witch-line.constant.default"))
      table.insert(components, overseer_component)
      return { statusline = { global = components } }
    end,
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
