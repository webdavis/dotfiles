return {
  {
    "stevearc/oil.nvim",
    ---@module 'oil'
    ---@type oil.SetupOpts
    dependencies = {
      "nvim-mini/mini.icons", -- optional
    },
    -- dependencies = { "nvim-tree/nvim-web-devicons" }, -- use if you prefer nvim-web-devicons
    -- Lazy loading is not recommended because it is very tricky to make it work correctly in all situations.
    lazy = false,
    config = function()
      local oil = require("oil")

      map({ mode = "n", lhs = { "-", "<leader>ef" }, rhs = "Oil", desc = "Oil: open parent directory" })

      -- Declare a global function to retrieve the current directory
      function _G.get_oil_winbar()
        local bufnr = vim.api.nvim_win_get_buf(vim.g.statusline_winid)
        local dir = oil.get_current_dir(bufnr)
        if dir then
          return vim.fn.fnamemodify(dir, ":~")
        else
          -- If there is no current directory (e.g. over ssh), just show the buffer name
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
    dependencies = { "stevearc/oil.nvim" },
    -- No opts or config needed! Works automatically
  },
}
