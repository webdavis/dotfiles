return {
  "webdavis/pns.nvim",
  commit = "bfac762784393b77144c3e65019b7caf6ef70c7c",
  -- Each producer loads this dependency before its first run.
  lazy = true,
  opts = {
    binary = "~/.cargo/bin/pns",
    minimum_version = "0.1.0",
  },
}
