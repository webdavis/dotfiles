return {
  "sQVe/sort.nvim",
  cmd = "Sort",
  -- These entries only load the plugin, because sort.nvim binds the keys itself from `opts.mappings`.
  keys = {
    { "go", mode = "n", desc = "Sort operator" },
    { "go", mode = "x", desc = "Sort selection" },
    { "gogo", mode = "n", desc = "Sort current line" },
    { "ir", mode = { "o", "x" }, desc = "Inner sortable region" },
    { "ar", mode = { "o", "x" }, desc = "Around sortable region" },
    { "]o", mode = { "n", "x", "o" }, desc = "Next delimiter" },
    { "[o", mode = { "n", "x", "o" }, desc = "Previous delimiter" },
  },
  opts = {
    ignore_case = true,
    -- `as`, `]s` and `[s` belong to the treesitter scope textobjects, so sort.nvim's textobject
    -- and motions use free keys.
    mappings = {
      textobject = { inner = "ir", around = "ar" },
      motion = { next_delimiter = "]o", prev_delimiter = "[o" },
    },
  },
}
