-- custom_api.clipboard, the yanks that land in the system clipboard.
--
-- Every case here asserts what the yank PUT IN THE REGISTER, because the
-- reported bug was a mapping whose description and whose effect disagreed:
-- one word selected, the whole line copied.
--
-- Two pieces of scaffolding earn their place. The clipboard provider is
-- replaced with an in-memory one, so a test run neither shells out to pbcopy
-- (which would put the run's fixtures in the operator's own clipboard) nor
-- depends on a provider existing at all, and the register TYPE it records is
-- what proves a blockwise yank stayed blockwise. And each selection case runs
-- the function through a real Visual-mode mapping, because that is the context
-- the defect lived in: the callback of a Visual-mode mapping runs while the
-- mode is STILL visual, with `'<` and `'>` not yet set.

local clipboard = require("custom_api.clipboard")

local stored = { lines = { "" }, regtype = "v" }

vim.g.clipboard = {
  name = "in-memory",
  copy = {
    ["+"] = function(lines, regtype)
      stored = { lines = lines, regtype = regtype }
    end,
  },
  paste = {
    ["+"] = function()
      return stored.lines, stored.regtype
    end,
  },
  cache_enabled = 0,
}

---Put `text` in the clipboard, as the register the yanks overwrite.
local function seed_clipboard(text)
  stored = { lines = vim.split(text, "\n", { plain = true }), regtype = "v" }
end

---The clipboard's contents as one string.
local function clipboard_text()
  return table.concat(stored.lines, "\n")
end

---Run `action` from a Visual-mode mapping, with `keys` making the selection.
local function from_visual(lines, cursor, keys, action)
  vim.api.nvim_buf_set_lines(0, 0, -1, false, lines)
  vim.api.nvim_win_set_cursor(0, cursor)
  vim.keymap.set("x", "<F13>", action)
  local sequence = vim.api.nvim_replace_termcodes(keys .. "<F13>", true, false, true)
  vim.api.nvim_feedkeys(sequence, "x", false)
end

---Collect the notifications one `action` raises.
local function notifications_from(action)
  local seen = {}
  local real = vim.notify
  vim.notify = function(message, level)
    table.insert(seen, { message = message, level = level })
  end
  local ok, err = pcall(action)
  vim.notify = real
  assert(ok, err)
  return seen
end

return {
  ["a charwise selection yanks the selection, not its whole line"] = function()
    seed_clipboard("PREVIOUS")
    from_visual({ "alpha beta gamma", "second line here" }, { 1, 6 }, "vee", clipboard.yank_selection)
    assert(clipboard_text() == "beta gamma", ("clipboard held %q"):format(clipboard_text()))
    assert(stored.regtype == "v", ("register type was %q"):format(stored.regtype))
    -- The cursor is put back exactly where the selection left it (the end of
    -- "gamma"), not moved to the start of the yank the way a plain `y` would.
    assert(vim.deep_equal(vim.api.nvim_win_get_cursor(0), { 1, 15 }), vim.inspect(vim.api.nvim_win_get_cursor(0)))
  end,

  ["a linewise selection yanks the whole selected lines"] = function()
    seed_clipboard("PREVIOUS")
    from_visual({ "alpha beta gamma", "second line here", "third" }, { 1, 6 }, "Vj", clipboard.yank_selection)
    assert(clipboard_text() == "alpha beta gamma\nsecond line here\n", ("clipboard held %q"):format(clipboard_text()))
    assert(stored.regtype == "V", ("register type was %q"):format(stored.regtype))
  end,

  ["a blockwise selection yanks the block, not the lines it crosses"] = function()
    seed_clipboard("PREVIOUS")
    from_visual({ "alpha beta gamma", "second line here" }, { 1, 6 }, "<C-v>jll", clipboard.yank_selection)
    assert(clipboard_text() == "bet\n li\n", ("clipboard held %q"):format(clipboard_text()))
    -- A provider is handed "b" for a blockwise yank, where a register holds
    -- CTRL-V plus the block width.
    assert(stored.regtype == "b", ("register type was %q"):format(stored.regtype))
  end,

  ["a blockwise selection taken to end-of-line keeps each line whole"] = function()
    -- `<C-v>j$` is the case a hand-rolled range reader gets wrong: the ragged
    -- right edge is only recorded in `curswant`. Vim's own yank knows it.
    seed_clipboard("PREVIOUS")
    from_visual({ "abc123456", "xyz" }, { 1, 1 }, "<C-v>j$", clipboard.yank_selection)
    assert(clipboard_text() == "bc123456\nyz\n", ("clipboard held %q"):format(clipboard_text()))
  end,

  ["the linewise yank takes the lines a charwise selection sits in"] = function()
    seed_clipboard("PREVIOUS")
    from_visual({ "alpha beta gamma", "second line here" }, { 1, 6 }, "vee", clipboard.yank_selected_lines)
    assert(clipboard_text() == "alpha beta gamma\n", ("clipboard held %q"):format(clipboard_text()))
    assert(stored.regtype == "V", ("register type was %q"):format(stored.regtype))
  end,

  ["a missing text object restores the clipboard and warns"] = function()
    seed_clipboard("PREVIOUS")
    vim.api.nvim_buf_set_lines(0, 0, -1, false, { "no parentheses on this line" })
    vim.api.nvim_win_set_cursor(0, { 1, 0 })
    local seen = notifications_from(function()
      assert(clipboard.yank_inside("(", "( ) pair") == false, "reported a yank that did not happen")
    end)
    assert(clipboard_text() == "PREVIOUS", ("clipboard held %q"):format(clipboard_text()))
    assert(#seen == 1 and seen[1].level == vim.log.levels.WARN, vim.inspect(seen))
    assert(seen[1].message:match("no %( %) pair"), seen[1].message)
  end,

  ["a present text object is yanked and reported"] = function()
    seed_clipboard("PREVIOUS")
    vim.api.nvim_buf_set_lines(0, 0, -1, false, { "call(inner text)" })
    vim.api.nvim_win_set_cursor(0, { 1, 0 })
    local seen = notifications_from(function()
      assert(clipboard.yank_inside("(", "( ) pair") == true, "reported a miss")
    end)
    assert(clipboard_text() == "inner text", ("clipboard held %q"):format(clipboard_text()))
    assert(#seen == 1 and seen[1].message == "Yanked: inner text", vim.inspect(seen))
  end,

  ["a blank line is refused rather than beeped at"] = function()
    seed_clipboard("PREVIOUS")
    vim.api.nvim_buf_set_lines(0, 0, -1, false, { "   " })
    vim.api.nvim_win_set_cursor(0, { 1, 0 })
    local seen = notifications_from(clipboard.yank_to_line_end)
    assert(clipboard_text() == "PREVIOUS", ("clipboard held %q"):format(clipboard_text()))
    assert(#seen == 1 and seen[1].level == vim.log.levels.WARN, vim.inspect(seen))
  end,

  ["yanking to end-of-line stops at the last non-blank character"] = function()
    seed_clipboard("PREVIOUS")
    vim.api.nvim_buf_set_lines(0, 0, -1, false, { "  indented tail   " })
    vim.api.nvim_win_set_cursor(0, { 1, 2 })
    clipboard.yank_to_line_end()
    assert(clipboard_text() == "indented tail", ("clipboard held %q"):format(clipboard_text()))
  end,

  ["a short single-line preview is the line itself"] = function()
    assert(clipboard.summarize("short line") == "short line", clipboard.summarize("short line"))
  end,

  ["a long line is cut short in the preview"] = function()
    local preview = clipboard.summarize(("x"):rep(200))
    assert(preview == ("x"):rep(60) .. "...", preview)
  end,

  ["a multi-line yank is counted rather than printed"] = function()
    local preview = clipboard.summarize("first\nsecond\nthird")
    assert(preview == "first (3 lines)", preview)
  end,

  ["a long line is cut on a character boundary"] = function()
    -- Byte truncation would leave half a character behind and print a
    -- replacement glyph in the notification.
    local preview = clipboard.summarize(("é"):rep(200))
    assert(preview == ("é"):rep(60) .. "...", preview)
  end,
}
