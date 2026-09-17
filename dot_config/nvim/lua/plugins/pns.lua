return {
  "webdavis/pns.nvim",
  -- webdavis/pns.nvim#2: prepends "send" for pns 0.2.0's send subcommand.
  commit = "4e52741a170f3207d53980a238730729abbfb58c",
  -- Each producer loads this dependency before its first run.
  lazy = true,
  opts = {
    binary = "~/.cargo/bin/pns",
    minimum_version = "0.2.0",
  },
}
