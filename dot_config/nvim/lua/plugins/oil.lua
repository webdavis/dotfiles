return {
  {
    "stevearc/oil.nvim",
    ---@module 'oil'
    ---@type oil.SetupOpts
    dependencies = {
      "nvim-mini/mini.icons",
    },
    -- Loaded at startup, because oil's lazy loading breaks in some situations.
    lazy = false,
    config = function()
      local oil = require("oil")

      map({ mode = "n", lhs = { "-", "<leader>ef" }, rhs = "Oil", desc = "Oil: open parent directory" })

      function _G.get_oil_winbar()
        local bufnr = vim.api.nvim_win_get_buf(vim.g.statusline_winid)
        local dir = oil.get_current_dir(bufnr)
        if dir then
          return vim.fn.fnamemodify(dir, ":~")
        else
          -- Over ssh there is no current directory, so show the buffer name.
          return vim.api.nvim_buf_get_name(0)
        end
      end

      local detail = false

      oil.setup({
        default_file_explorer = true,
        win_options = {
          winbar = "%!v:lua.get_oil_winbar()",
          signcolumn = "yes:3",
        },
        keymaps = {
          ["gd"] = {
            desc = "Toggle file detail view",
            callback = function()
              detail = not detail
              if detail then
                require("oil").set_columns({ "icon", "permissions" })
              else
                require("oil").set_columns({ "icon" })
              end
            end,
          },
        },
      })
    end,
  },
  {
    "benomahony/oil-git.nvim",
    -- Its whole surface is an oil ftplugin, so the oil filetype loads it.
    ft = "oil",
    dependencies = { "stevearc/oil.nvim" },
  },
}
