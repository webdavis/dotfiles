return {
  -- Quickfix preview, fzf filter and sign-based filtering over the native quickfix window.
  "kevinhwang91/nvim-bqf",
  ft = "qf",
  config = function()
    require("bqf").setup()
  end,
}
