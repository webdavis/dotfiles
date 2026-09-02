return {
  "folke/noice.nvim",
  event = "VeryLazy",
  dependencies = {
    -- if you lazy-load any plugin below, make sure to add proper `module="..."` entries
    "MunifTanjim/nui.nvim",
    -- OPTIONAL:
    --   `nvim-notify` is only needed, if you want to use the notification view.
    --   If not available, we use `mini` as the fallback
    "rcarriga/nvim-notify",
  },
  opts = {
    lsp = {
      override = {
        ["vim.lsp.util.convert_input_to_markdown_lines"] = true,
        ["vim.lsp.util.stylize_markdown"] = true,
        ["cmp.entry.get_documentation"] = true,
      },
    },
    routes = {
      {
        filter = {
          event = "msg_show",
          any = {
            { find = "%d+L, %d+B" },
            { find = "; after #%d+" },
            { find = "; before #%d+" },
          },
        },
        view = "mini",
      },
    },
    presets = {
      bottom_search = false,
      long_message_to_split = true,
      inc_rename = true, -- enables an input dialog for inc-rename.nvim
      lsp_doc_border = true, -- add a border to hover docs and signature help
      command_palette = {
        views = {
          cmdline_popup = {
            size = {
              min_width = 100,
              width = "auto",
              height = "auto",
            },
          },
        },
      },
    },
  },
  keys = {
    -- stylua: ignore start
    { "<S-Enter>", function() require("noice").redirect(vim.fn.getcmdline()) end, mode = "c", desc = "Noice: redirect cmdline" },
    { "<leader>nl", function() require("noice").cmd("last") end, desc = "Noice: last message" },
    { "<leader>nh", function() require("noice").cmd("history") end, desc = "Noice: history" },
    { "<leader>na", function() require("noice").cmd("all") end, desc = "Noice: all" },
    { "<leader>nD", function() require("noice").cmd("dismiss") end, desc = "Noice: dismiss all messages" },
    { "<leader>np", function() require("noice").cmd("pick") end, desc = "Noice: picker" },
    { "<c-f>", function() if not require("noice.lsp").scroll(4) then return "<c-f>" end end, silent = true, expr = true, desc = "Noice: scroll forward", mode = { "n", "s" } },
    { "<c-b>", function() if not require("noice.lsp").scroll(-4) then return "<c-b>" end end, silent = true, expr = true, desc = "Noice: scroll backward", mode = { "n", "s" } },
    -- stylua: ignore end
  },
  config = function(_, opts)
    -- HACK: noice shows messages from before it was enabled,
    -- but this is not ideal when Lazy is installing plugins,
    -- so clear the messages in this case.
    if vim.o.filetype == "lazy" then
      vim.cmd([[messages clear]])
    end
    require("noice").setup(opts)
  end,
}
