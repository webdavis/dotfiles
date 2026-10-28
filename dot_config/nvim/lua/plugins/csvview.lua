return {
  "hat0uma/csvview.nvim",
  ft = { "csv", "tsv" },
  cmd = { "CsvViewEnable", "CsvViewDisable", "CsvViewToggle" },
  ---@module "csvview"
  ---@type CsvView.Options
  opts = {
    parser = { comments = { "#", "//" } },
    view = { display_mode = "border", header_lnum = true },
    keymaps = {
      textobject_field_inner = { "if", mode = { "o", "x" } },
      textobject_field_outer = { "af", mode = { "o", "x" } },
      jump_next_field_end = { "<Tab>", mode = { "n", "v" } },
      jump_prev_field_end = { "<S-Tab>", mode = { "n", "v" } },
      jump_next_row = { "<Enter>", mode = { "n", "v" } },
      jump_prev_row = { "<S-Enter>", mode = { "n", "v" } },
    },
  },
  config = function(_, opts)
    require("csvview").setup(opts)
    -- On by default: every csv and tsv buffer opens already aligned.
    vim.api.nvim_create_autocmd("FileType", {
      group = vim.api.nvim_create_augroup("CsvViewAutoEnable", { clear = true }),
      pattern = { "csv", "tsv" },
      callback = function(event)
        require("csvview").enable(event.buf)
      end,
    })
    -- lazy.nvim re-fires FileType only for the buffer that triggered the load.
    for _, buf in ipairs(vim.api.nvim_list_bufs()) do
      if vim.tbl_contains({ "csv", "tsv" }, vim.bo[buf].filetype) then
        require("csvview").enable(buf)
      end
    end
  end,
}
