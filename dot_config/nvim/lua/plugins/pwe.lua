return vim.tbl_extend("force", require("custom_api.paired_plugin").spec("pwe"), {
  lazy = false,
  opts = {
    modes = { "n", "i" },
  },
})
