return {
  "unblevable/quick-scope",
  dependencies = {
    "folke/snacks.nvim",
  },
  config = function()
    vim.g.qs_filetype_blacklist = { "gitcommit", "help" }

    require("snacks")
      .toggle({
        name = "Quickscope",
        get = function()
          return vim.g.qs_enable == 1
        end,
        set = function(state)
          if state then
            vim.g.qs_enable = 1
          else
            vim.g.qs_enable = 0
          end
          -- force Quickscope to update
          vim.cmd("doautocmd CursorMoved")
        end,
      })
      :map("<leader>uQ")

    vim.api.nvim_set_hl(0, "QuickScopePrimary", { fg = "#f5ca5e", bold = false, underline = true })
    vim.api.nvim_set_hl(0, "QuickScopeSecondary", { fg = "#fc0094", bold = false, underline = true })
  end,
}
