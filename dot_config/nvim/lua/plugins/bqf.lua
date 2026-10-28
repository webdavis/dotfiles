return {
  -- Adds a preview, an fzf filter and sign-based filtering to the quickfix window.
  "kevinhwang91/nvim-bqf",
  ft = "qf",
  config = function()
    require("bqf").setup()
  end,
}
