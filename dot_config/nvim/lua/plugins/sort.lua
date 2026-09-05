return {
  "sQVe/sort.nvim",
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
