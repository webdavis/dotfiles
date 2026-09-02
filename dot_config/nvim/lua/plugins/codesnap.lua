return {
  "mistricky/codesnap.nvim",
  build = "make",
  keys = {
    { "<leader>cs", "<cmd>CodeSnap<cr>", mode = "x", desc = "CodeSnap: save selected code snapshot to clipboard" },
    {
      "<leader>cS",
      "<cmd>CodeSnapSave<cr>",
      mode = "x",
      desc = "CodeSnap: save selected code snapshot to " .. (os.getenv("HOME") .. "/Pictures"),
    },
  },
  opts = {
    save_path = "~/Pictures",
    has_breadcrumbs = true,
    bg_theme = "grape",
    watermark = "",
  },
}
