-- The line annotator (spec 7.7). It writes down what the operator would
-- otherwise retype at an agent: where the line is, what the language server
-- says about it, which function it sits in, and which commit last touched it.
--
-- Delivery is not this module's job. The text goes into `herdr-nvim`'s own
-- annotation store, which already ships the keys that paste (`<leader>As`) or
-- send (`<leader>AS`) pending comments, so nothing here types into an agent and
-- there is no state to race.

local M = {}

local git = require("custom_api.git")
local util = require("custom_api.util")

-- Every grammar spells its function node differently, so the rule is a SUFFIX
-- match over the six spellings that cover the languages this config edits.
-- A `find` on "function" would match `function_call` and `parameters` and walk
-- no further, reporting the call around the cursor as its enclosing function.
-- The match is anchored at the END for the same reason: `function_definition_call`
-- contains a listed spelling without being one.
--
-- Measured 2026-09-05 by walking a sample file per language under the installed
-- grammars. `(name)` is what `function_part` reads; a node with no name field
-- contributes no function part at all.
--
--   lua         function_declaration (name), function_definition (no name)
--   python      function_definition (name)
--   bash        function_definition (name)
--   rust        function_item (name)
--   go          function_declaration (name), method_declaration (name)
--   swift       function_declaration (name)
--   typescript  function_declaration, function_expression, method_definition (all name),
--               arrow_function (no name, and no suffix here)
local FUNCTION_NODE_SUFFIXES = {
  "function_definition",
  "function_declaration",
  "method_definition",
  "function_item",
  "method_declaration",
  "function_expression",
}

-- The order the parts appear in, stated once. `pairs` over the parts table
-- would order them by whatever the hash walk returned that run.
local PART_ORDER = { "mention", "diagnostic", "func", "blame" }

-- ╭──────╮
-- │  API │
-- ╰──────╯

---Join the parts into one annotation, one part per line, in a fixed order.
---
---A missing part contributes no line at all: an annotation whose diagnostic was
---absent must not carry the blank line where it would have been.
---@param parts { mention: string?, diagnostic: string?, func: string?, blame: string? }
---@return string
function M.compose_text(parts)
  parts = parts or {}

  local lines = {}
  for _, key in ipairs(PART_ORDER) do
    local part = parts[key]
    -- An empty string is a missing part too: a diagnostic message that trimmed
    -- to nothing arrives as "" rather than nil.
    if part and part ~= "" then
      lines[#lines + 1] = part
    end
  end

  return table.concat(lines, "\n")
end

---The innermost function-shaped node at or above `node`, or nil outside one.
---@param node TSNode? the node under the cursor
---@return TSNode?
function M.enclosing_function(node)
  while node do
    local node_type = node:type()
    for _, suffix in ipairs(FUNCTION_NODE_SUFFIXES) do
      if vim.endswith(node_type, suffix) then
        return node
      end
    end
    node = node:parent()
  end

  return nil
end

---The blame part: the line's commit, named when this repository can name it.
---
---`git.latest_commit` describes HEAD and nothing else, so its summary belongs
---to the blamed line only when the blame SHA *is* HEAD. Attached any other
---time it would caption the line with an unrelated commit's message.
---@param sha string? the blame SHA, or nil when the line has none
---@param commit { hash: string, summary: string? }? HEAD, as `git.latest_commit` returns it
---@return string?
function M.blame_line(sha, commit)
  if not sha then
    return nil
  end

  local short = sha:sub(1, 7)

  if commit and commit.summary and commit.hash and vim.startswith(sha, commit.hash) then
    return ("blame %s %s"):format(short, commit.summary)
  end

  return "blame " .. short
end

-- A buffer name that opens with a URI scheme belongs to a plugin, not to the
-- filesystem. MEASURED: an Oil directory buffer and a Fugitive revision buffer
-- both carry an EMPTY `buftype`, so the buftype test below does not reach
-- them; their names are what gives them away. The scheme is anchored at the
-- start, so a legal path holding `://` further along stays a path.
local URI_NAME = "^%a[%w+.-]*://"

---Whether this buffer can produce a mention an agent could act on.
---
---A named file that has never been written is fine: it is a real path with
---real lines. The three refusals are a buffer with no name, which would be
---mentioned as `@:1`; a buffer with any `buftype`, which is a scratch buffer,
---a terminal, a quickfix list or a help window rather than a file; and a
---buffer named by a URI, whose name is not a path an agent can open.
---@param name string the buffer name, as `nvim_buf_get_name` returns it
---@param buftype string the buffer's `buftype`
---@return boolean annotatable
---@return string? reason why not, when it is not
function M.annotatable(name, buftype)
  if name == "" then
    return false, "this buffer has no file name"
  end

  if buftype ~= "" then
    return false, ("this is a %s buffer, not a file"):format(buftype)
  end

  if name:match(URI_NAME) then
    return false, ("this buffer is named by a URI, not a path: %s"):format(name)
  end

  return true
end

---The diagnostic part: the severity and the message, on one line.
---
---A message that trims to nothing yields no part at all. Formatting it anyway
---would contribute the bare severity label, `ERROR: `, which `compose_text`
---reads as a real part because it is not empty.
---@param diagnostic { severity: integer, message: string }? as `vim.diagnostic.get` returns it
---@return string?
function M.diagnostic_line(diagnostic)
  if not diagnostic then
    return nil
  end

  -- A language server is free to send a multi-line message, and every part is
  -- one line by contract, so the message collapses before it is composed.
  local message = util.trim((diagnostic.message:gsub("%s+", " ")))
  if message == "" then
    return nil
  end

  return ("%s: %s"):format(vim.diagnostic.severity[diagnostic.severity], message)
end

-- ╭──────────────────╮
-- │  The editor edge │
-- ╰──────────────────╯

local function diagnostic_part(bufnr, line)
  return M.diagnostic_line(vim.diagnostic.get(bufnr, { lnum = line - 1 })[1])
end

local function function_part(bufnr, line, column)
  -- The CURSOR's column, not zero. Column zero of a nested declaration line
  -- sits outside the function being declared, so it named the function around
  -- it instead; column zero of an indented `def` or `func` line is leading
  -- whitespace, which named no function at all.
  --
  -- `get_node` returns nil rather than raising when no parser is attached
  -- (verified in the 0.12 runtime: `get_parser` reports a message, it does not
  -- error), so an unparsed buffer simply contributes no function part.
  local node = M.enclosing_function(vim.treesitter.get_node({ bufnr = bufnr, pos = { line - 1, column } }))
  if not node then
    return nil
  end

  -- An anonymous function has no `name` field, and its node type alone says
  -- nothing the reader cannot already see, so it contributes no part either.
  local name = node:field("name")[1]
  if not name then
    return nil
  end

  return "function " .. vim.treesitter.get_node_text(name, bufnr)
end

local function blame_part(file, line)
  -- Both calls return `nil, message` on failure (an unnamed buffer, a file
  -- outside a repository); the message is the keymap layer's to report, and
  -- here a line the annotator cannot blame is simply a part it does not write.
  local sha = git.blame_sha({ file = file, line = line })
  if not sha then
    return nil
  end

  return M.blame_line(sha, git.latest_commit({ repo_name = util.get_cwd_basename() }))
end

---Annotate the cursor's line in `herdr-nvim`'s annotation store.
---@return integer? id the new comment's id, or nil when the buffer cannot be annotated
---@return string? reason why not
function M.line()
  local bufnr = vim.api.nvim_get_current_buf()
  local cursor = vim.api.nvim_win_get_cursor(0)
  local line, column = cursor[1], cursor[2]
  local name = vim.api.nvim_buf_get_name(bufnr)

  local ok, reason = M.annotatable(name, vim.bo[bufnr].buftype)
  if not ok then
    return nil, reason
  end

  local file = vim.fn.fnamemodify(name, ":.")

  local text = M.compose_text({
    mention = ("@%s:%d"):format(file, line),
    diagnostic = diagnostic_part(bufnr, line),
    func = function_part(bufnr, line, column),
    blame = blame_part(file, line),
  })

  local id = require("herdr-nvim.comments").add(bufnr, line, line, text)
  require("herdr-nvim.ui").decorate(id)

  return id
end

return M
