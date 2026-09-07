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
      -- Turning it back off means every integration this roster wants has to be
      -- named here, and five were not: measured against auto-detection, 92
      -- highlight groups were missing, all of them `BlinkCmp*`, `Dap*`,
      -- `Octo*` and `NvimSurround*`. A table value counts as ON only when it
      -- carries `enabled = true` (catppuccin's lib/mapper.lua), so `blink_cmp`
      -- styled but never enabled was contributing nothing at all.
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

      -- Same theme under two names across a pin bump: the old pin only ships
      -- `catppuccin`, so the new name fails there. Editing lazy-lock.json installs
      -- nothing until `:Lazy restore`, so the window between an apply and that
      -- restore is real, and lazy reports the failure through `vim.notify`, which
      -- makes it silent at startup.
      --
      -- Which name exists is settled BEFORE loading rather than by trying one and
      -- falling back when it raises. A fallback fires on ANY error, so an error
      -- raised once from a ColorScheme callback ran the whole loader a second
      -- time, and a persistent one was reported as "neither loaded" with both
      -- real errors thrown away. `getcompletion` answers off the `colors/` files
      -- on the runtimepath, which is the question actually being asked.
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
    "akinsho/bufferline.nvim",
    lazy = false,
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
      --
      -- Both callbacks reach `custom_api.overseer_status` by name and capture
      -- NOTHING. witch-line's cache serializes them as bytecode without their
      -- upvalues, so a callback closing over a local helper came back from a
      -- populated cache as `attempt to call upvalue 'counts' (a nil value)`:
      -- the counter worked on a cold start and broke on every start after one.
      -- `require` is a global lookup, which survives that roundtrip.
      local overseer_component = {
        id = "overseer.tasks",
        padding = { left = 1, right = 1 },
        -- Task status changes on its own schedule, with no event to hook, so
        -- this polls rather than waiting to be told.
        timing = 1000,
        hidden = function()
          return require("custom_api.overseer_status").is_idle()
        end,
        update = function()
          return require("custom_api.overseer_status").render()
        end,
      }

      local components = vim.deepcopy(require("witch-line.constant.default"))
      table.insert(components, overseer_component)
      return { statusline = { global = components } }
    end,
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
