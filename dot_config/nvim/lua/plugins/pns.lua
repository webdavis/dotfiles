return vim.tbl_extend("force", require("custom_api.paired_plugin").spec("pns"), {
  lazy = true,
  opts = {
    binary = "~/.cargo/bin/pns",
    producer = "nvim",
    project = nil,
    pane = nil,
  },
})
