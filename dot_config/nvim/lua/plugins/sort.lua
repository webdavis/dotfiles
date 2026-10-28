return {
  "sQVe/sort.nvim",
  cmd = "Sort",
  -- These entries are triggers only, carrying no mapping of their own, unlike
  -- the aerial and live-rename ones: sort.nvim binds all seven itself inside
  -- its own setup, off the `mappings` table in `opts` below, so a mapping here
  -- would be a second declaration of a key this config does not own. They still
  -- have to be listed, because a key left out stops firing until something else
  -- happens to load the plugin. `go` and `gogo` come from the default
  -- `mappings.operator`; the rest are the pair reassigned in `opts`.
  keys = {
    { "go", mode = "n", desc = "Sort operator" },
    { "go", mode = "x", desc = "Sort selection" },
    { "gogo", mode = "n", desc = "Sort current line" },
    { "ir", mode = { "o", "x" }, desc = "Inner sortable region" },
    { "ar", mode = { "o", "x" }, desc = "Around sortable region" },
    { "],", mode = { "n", "x", "o" }, desc = "Next delimiter" },
    { "[,", mode = { "n", "x", "o" }, desc = "Previous delimiter" },
  },
  opts = {
    ignore_case = true,
    -- sort.nvim and nvim-treesitter-textobjects both bound `as`, `]s` and `[s`,
    -- and whichever loaded last won that start, so `]s` meant "next delimiter"
    -- on some starts and "next local scope" on others (9 of 14 measured starts
    -- one way, 5 the other). Treesitter keeps all three: a scope textobject and
    -- scope motions are the conventional meanings, and the rest of that family
    -- (`ac`, `al`, `]c`, `]m`, `]l`, `]k`, `]z`) already reads that way.
    --
    -- sort.nvim's pair moves to keys nothing else claims. `[S` and `]S` are not
    -- among them: those are Vim's own bad-spelling motions and still work.
    mappings = {
      textobject = { inner = "ir", around = "ar" },
      motion = { next_delimiter = "],", prev_delimiter = "[," },
    },
  },
}
