-- The keymap scope of the `markdown-plus.nvim` block in `lua/plugins/markdown.lua`.
--
-- The spec loads on `ft = "markdown"`, which says WHEN the plugin loads and
-- nothing at all about where its mappings apply: a `vim.keymap.set` with no
-- `buffer` option is global, so one markdown file used to replace `gd`, `]]`,
-- `[[`, `o`, `O` and five insert-mode keys in every buffer for the rest of the
-- session. The keys have to be buffer-local, and the whole set has to survive.
--
-- markdown-plus.nvim is absent under `nvim --clean`, so its module is faked. Its
-- `<Plug>` targets are therefore unmapped here, which is why the markdown arm
-- asserts the scope and target of each key rather than what pressing it does:
-- the real plugin's behavior is not under test, its reachability is. The
-- non-markdown arm does measure behavior, because the keys it cares about are
-- Vim's own motions and `--clean` has all of them.

local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

-- Every key that shadows a Vim built-in, with the target the spec points it at.
-- These ten are the defect: a global copy of any of them breaks an unrelated
-- buffer. The rest of the block is `<localleader>` keys, which are covered by
-- the "nothing global" assertion below rather than one row each.
local SHADOWING_KEYS = {
  { mode = "n", lhs = "gd", rhs = "<Plug>(MarkdownPlusFollowLink)" },
  { mode = "n", lhs = "]]", rhs = "<Plug>(MarkdownPlusNextHeader)" },
  { mode = "n", lhs = "[[", rhs = "<Plug>(MarkdownPlusPrevHeader)" },
  { mode = "n", lhs = "o", rhs = "<Plug>(MarkdownPlusNewListItemBelow)" },
  { mode = "n", lhs = "O", rhs = "<Plug>(MarkdownPlusNewListItemAbove)" },
  { mode = "i", lhs = "<CR>", rhs = "<Plug>(MarkdownPlusListEnter)" },
  { mode = "i", lhs = "<Tab>", rhs = "<Plug>(MarkdownPlusListIndent)" },
  { mode = "i", lhs = "<S-Tab>", rhs = "<Plug>(MarkdownPlusListOutdent)" },
  { mode = "i", lhs = "<BS>", rhs = "<Plug>(MarkdownPlusListBackspace)" },
  { mode = "i", lhs = "<C-t>", rhs = "<Plug>(MarkdownPlusToggleCheckbox)" },
}

-- How many buffer-local mappings onto a markdown-plus `<Plug>` target one
-- markdown buffer must end up with: the spec declares 63, two of which are
-- visual-mode ("v") and so are stored once in visual and once in select. Losing
-- a mapping in a refactor fails here instead of going unnoticed until a hand
-- reaches for the key. Only this configuration's own mappings are counted, so
-- what Neovim's markdown ftplugin adds to the same buffer cannot move it. The
-- table rows are gone from the count: they targeted `<Plug>(markdown-plus-
-- table-*)`, a spelling the plugin never registers (it uses `<Plug>(MarkdownPlus
-- Table*)`), so the plugin's own working defaults now cover those keys.
local MARKDOWN_PLUS_MAPPINGS_PER_BUFFER = 65

local MODES = { "n", "x", "s", "i" }

---Load the markdown-plus spec against a faked plugin module and run its `config`.
---@param fn fun(spec: table)
local function with_markdown_plus(fn)
  local saved = { package.loaded["markdown-plus"] }
  package.loaded["markdown-plus"] = { setup = function() end }

  local ok, err = pcall(function()
    local spec = dofile(config_root .. "/lua/plugins/markdown.lua")
    assert(spec[1][1] == "yousefhadder/markdown-plus.nvim", "the markdown-plus spec moved out of plugins/markdown.lua")
    assert(spec[1].ft == "markdown", "the markdown-plus spec stopped loading on ft=markdown")
    spec[1].config()
    fn(spec[1])
  end)

  package.loaded["markdown-plus"] = saved[1]
  assert(ok, err)
end

---Open a buffer of `filetype` and fire `FileType` for it, which is what lazy.nvim
---does after loading an `ft` spec.
---@param filetype string
---@param lines string[]
---@return integer
local function open(filetype, lines)
  vim.cmd("enew")
  vim.bo.filetype = filetype
  vim.api.nvim_buf_set_lines(0, 0, -1, false, lines)
  vim.api.nvim_exec_autocmds("FileType", { buffer = 0, modeline = false })
  return vim.api.nvim_get_current_buf()
end

local LUA_LINES = {
  "local function outer()",
  "  local alpha = 1",
  "  local beta = 2",
  "  print(alpha)",
  "end",
  "",
  "local function second() end",
}

local function press(keys)
  vim.api.nvim_feedkeys(vim.api.nvim_replace_termcodes(keys, true, false, true), "mx", false)
end

---@param mode string
---@param lhs string
---@return table
local function mapping(mode, lhs)
  local found = vim.fn.maparg(lhs, mode, false, true)
  return type(found) == "table" and found or {}
end

---Whether a right-hand side is one of markdown-plus's `<Plug>` targets. The
---plugin spells them two ways: `<Plug>(MarkdownPlusBold)` for most features and
---`<Plug>(markdown-plus-table-create)` for the table ones.
---@param rhs any
---@return boolean
local function is_markdown_plus_target(rhs)
  if type(rhs) ~= "string" then
    return false
  end
  return rhs:find("MarkdownPlus", 1, true) ~= nil or rhs:find("markdown-plus-table", 1, true) ~= nil
end

---Every mapping whose right-hand side is one of markdown-plus's `<Plug>` targets,
---taken from the global tables of all four modes the spec maps in.
---@return string[]
local function global_plug_mappings()
  local leaked = {}
  for _, mode in ipairs(MODES) do
    for _, found in ipairs(vim.api.nvim_get_keymap(mode)) do
      if is_markdown_plus_target(found.rhs) then
        table.insert(leaked, mode .. " " .. found.lhs)
      end
    end
  end
  return leaked
end

---How many of a buffer's own mappings point at a markdown-plus `<Plug>` target.
---@param bufnr integer
---@return integer
local function markdown_plus_mapping_count(bufnr)
  local total = 0
  for _, mode in ipairs(MODES) do
    for _, found in ipairs(vim.api.nvim_buf_get_keymap(bufnr, mode)) do
      if is_markdown_plus_target(found.rhs) then
        total = total + 1
      end
    end
  end
  return total
end

return {
  ["a markdown buffer carries every mapping the spec owns, buffer-locally"] = function()
    with_markdown_plus(function()
      local bufnr = open("markdown", { "# One", "", "## Two" })
      for _, key in ipairs(SHADOWING_KEYS) do
        local found = mapping(key.mode, key.lhs)
        assert(next(found) ~= nil, ("%s %s is not mapped in a markdown buffer"):format(key.mode, key.lhs))
        assert(found.buffer == 1, ("%s %s is global, not buffer-local"):format(key.mode, key.lhs))
        assert(
          found.rhs == key.rhs,
          ("%s %s points at %s, not %s"):format(key.mode, key.lhs, tostring(found.rhs), key.rhs)
        )
      end
      assert(
        markdown_plus_mapping_count(bufnr) == MARKDOWN_PLUS_MAPPINGS_PER_BUFFER,
        ("the markdown buffer got %d markdown mappings, expected %d"):format(
          markdown_plus_mapping_count(bufnr),
          MARKDOWN_PLUS_MAPPINGS_PER_BUFFER
        )
      )
    end)
  end,

  ["no markdown mapping is installed globally"] = function()
    with_markdown_plus(function()
      open("markdown", { "# One" })
      local leaked = global_plug_mappings()
      assert(#leaked == 0, "global markdown mappings: " .. table.concat(leaked, ", "))
    end)
  end,

  ["a Lua buffer opened after a markdown buffer keeps Vim's own gd, ]] and [["] = function()
    -- The measurement from the audit, inverted into a test. Before the fix all
    -- three stood still because a `<Plug>` target had replaced them globally.
    with_markdown_plus(function()
      open("markdown", { "# One", "", "## Two" })

      open("lua", LUA_LINES)
      vim.api.nvim_win_set_cursor(0, { 4, 8 })
      press("gd")
      assert(vim.api.nvim_win_get_cursor(0)[1] == 2, "gd did not reach the declaration of alpha on line 2")

      open("lua", LUA_LINES)
      vim.api.nvim_win_set_cursor(0, { 1, 0 })
      press("]]")
      assert(vim.api.nvim_win_get_cursor(0)[1] == 7, "]] did not move to the next section")

      open("lua", LUA_LINES)
      vim.api.nvim_win_set_cursor(0, { 7, 0 })
      press("[[")
      assert(vim.api.nvim_win_get_cursor(0)[1] == 1, "[[ did not move to the previous section")
    end)
  end,

  ["a Lua buffer opened after a markdown buffer keeps o, O and the insert keys"] = function()
    with_markdown_plus(function()
      open("markdown", { "# One" })
      local bufnr = open("lua", LUA_LINES)
      for _, key in ipairs(SHADOWING_KEYS) do
        -- Not "unmapped": Neovim itself ships a global insert-mode `<Tab>`
        -- (`vim.snippet.jump if active, otherwise <Tab>`). What must be gone is
        -- the markdown mapping, at which point Vim's own behavior is back.
        local found = mapping(key.mode, key.lhs)
        assert(found.buffer ~= 1, ("%s %s is buffer-local in a Lua buffer"):format(key.mode, key.lhs))
        assert(
          not (type(found.rhs) == "string" and found.rhs:find("MarkdownPlus", 1, true)),
          ("%s %s still reaches %s in a Lua buffer"):format(key.mode, key.lhs, tostring(found.rhs))
        )
      end
      assert(markdown_plus_mapping_count(bufnr) == 0, "the Lua buffer picked up buffer-local markdown mappings")
    end)
  end,

  ["a second markdown buffer gets the mappings too"] = function()
    -- A one-shot `FileType` autocmd, or a set applied only to the buffer that
    -- loaded the plugin, would leave every later markdown file bare.
    with_markdown_plus(function()
      open("markdown", { "# One" })
      open("lua", LUA_LINES)
      local bufnr = open("markdown", { "# Two" })
      assert(
        markdown_plus_mapping_count(bufnr) == MARKDOWN_PLUS_MAPPINGS_PER_BUFFER,
        ("the second markdown buffer got %d markdown mappings, expected %d"):format(
          markdown_plus_mapping_count(bufnr),
          MARKDOWN_PLUS_MAPPINGS_PER_BUFFER
        )
      )
    end)
  end,
}
