return {
  "folke/trouble.nvim",
  cmd = { "Trouble" },
  optional = false,
  opts = {
    modes = {
      lsp = {
        win = { position = "right" },
      },
    },
  },
  keys = {
    { "<leader>xx", "<cmd>Trouble diagnostics toggle<cr>", desc = "Trouble: diagnostics" },
    { "<leader>xX", "<cmd>Trouble diagnostics toggle filter.buf=0<cr>", desc = "Trouble: buffer diagnostics" },
    { "<leader>xL", "<cmd>Trouble loclist toggle<cr>", desc = "Trouble: Location List" },
    { "<leader>xq", "<cmd>Trouble qflist toggle<cr>", desc = "Trouble: Quickfix List" },

    -- LSP
    -- stylua: ignore start
    { "<leader>lx", "<cmd>Trouble lsp toggle focus=false win.position=right<cr>", desc = "Trouble (LSP): definitions／references／etc" },
    { "<leader>lX", "<cmd>Trouble symbols toggle focus=false<cr>", desc = "Trouble (LSP): symbols (don't focus)" },
    { "<leader>lS", "<cmd>Trouble symbols toggle focus=true<cr>", desc = "Trouble (LSP): symbols (and focus)" },
    -- stylua: ignore end
    {
      "[q",
      function()
        local trouble = require("trouble")
        if trouble.is_open() then
          trouble.prev({ skip_groups = true, jump = true })
        else
          local ok, err = pcall(vim.cmd.cprev)
          if not ok then
            vim.notify(err, vim.log.levels.ERROR)
          end
        end
      end,
      desc = "Trouble／Quickfix: jump to previous item",
    },
    {
      "]q",
      function()
        local trouble = require("trouble")
        if trouble.is_open() then
          trouble.next({ skip_groups = true, jump = true })
        else
          local ok, err = pcall(vim.cmd.cnext)
          if not ok then
            vim.notify(err, vim.log.levels.ERROR)
          end
        end
      end,
      desc = "Trouble／Quickfix: jump to next item",
    },
  },
}
