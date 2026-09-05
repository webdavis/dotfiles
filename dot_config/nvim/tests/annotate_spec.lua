-- custom_api.annotate, the line annotator's pure core (spec 7.7).
--
-- Only the pure parts are pinned here. `annotate.line()` reads the cursor, the
-- diagnostic store, a treesitter tree and git, and hands its text to
-- `herdr-nvim`; none of that is ours to assert, and a fake herdr would be a
-- test of a plugin we did not write. What IS ours is the composer, the
-- node-type filter and the blame formatter, and each takes plain data.

local annotate = require("custom_api.annotate")
local git = require("custom_api.git")

-- `annotate.line()` reaches four things. Two of them, the diagnostic store and
-- treesitter, are core and real here: nvim ships a Lua parser
-- (`lib/nvim/parser/lua.so`), which attaches under `--clean`. The other two are
-- substituted because they are not what these cases are about: git would shell
-- out, and `herdr-nvim` is a plugin no `--clean` run has on its runtimepath.
--
-- Returns the text the annotator handed to the store.
local function annotated(lines, cursor)
  local real_runner = git.runner
  local real_comments = package.loaded["herdr-nvim.comments"]
  local real_ui = package.loaded["herdr-nvim.ui"]

  local stored
  git.runner = function()
    return 128, "fatal: not a git repository"
  end
  package.loaded["herdr-nvim.comments"] = {
    add = function(_, _, _, text)
      stored = text
      return 1
    end,
  }
  package.loaded["herdr-nvim.ui"] = { decorate = function() end }

  local previous = vim.api.nvim_get_current_buf()
  -- NOT a scratch buffer: `nvim_create_buf(_, true)` sets `buftype = nofile`,
  -- which the annotator refuses. A name is required too (an unnamed buffer is
  -- refused) and has to be a real path, because the annotator asks
  -- `util.file_dir` for its directory.
  local bufnr = vim.api.nvim_create_buf(false, false)
  vim.api.nvim_buf_set_name(bufnr, vim.fn.tempname() .. ".lua")
  vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, lines)
  vim.bo[bufnr].filetype = "lua"
  vim.api.nvim_set_current_buf(bufnr)
  vim.treesitter.get_parser(bufnr, "lua"):parse()
  vim.api.nvim_win_set_cursor(0, cursor)

  local ok, result, reason = pcall(annotate.line)

  vim.api.nvim_set_current_buf(previous)
  vim.api.nvim_buf_delete(bufnr, { force = true })
  git.runner = real_runner
  package.loaded["herdr-nvim.comments"] = real_comments
  package.loaded["herdr-nvim.ui"] = real_ui

  assert(ok, result)
  assert(result, "the annotator refused the buffer: " .. tostring(reason))
  return stored
end

-- Row 2 is `  local function inner()`; column 18 is inside `inner`, column 0 is
-- outside it and inside `outer`.
local NESTED = {
  "function outer()",
  "  local function inner()",
  "    return 1",
  "  end",
  "end",
}

-- A treesitter node stands in for the real one through the two methods the
-- filter calls. The chain is built innermost-first, so `chain({"a","b"})`
-- returns the "a" node whose parent is the "b" node.
local function chain(types)
  local node
  for index = #types, 1, -1 do
    local parent = node
    node = {
      _type = types[index],
      type = function(self)
        return self._type
      end,
      parent = function()
        return parent
      end,
    }
  end
  return node
end

return {
  ["joins every part one per line"] = function()
    local text = annotate.compose_text({
      mention = "@lua/custom_api/annotate.lua:42",
      diagnostic = "ERROR: undefined variable",
      func = "function compose_text",
      blame = "blame a1b2c3d add the annotator",
    })
    assert(
      text
        == "@lua/custom_api/annotate.lua:42\n"
          .. "ERROR: undefined variable\n"
          .. "function compose_text\n"
          .. "blame a1b2c3d add the annotator",
      "composed " .. vim.inspect(text)
    )
  end,

  ["returns just the mention when it is the only part"] = function()
    local text = annotate.compose_text({ mention = "@init.lua:1" })
    assert(text == "@init.lua:1", "composed " .. vim.inspect(text))
  end,

  ["leaves no blank line where a part is nil"] = function()
    -- The diagnostic is the missing middle part: a composer that emitted "" for
    -- it would still hold four lines, and the annotation would carry a gap.
    local text = annotate.compose_text({
      mention = "@init.lua:1",
      func = "function setup",
      blame = "blame a1b2c3d",
    })
    assert(text == "@init.lua:1\nfunction setup\nblame a1b2c3d", "composed " .. vim.inspect(text))
    assert(not text:find("\n\n", 1, true), "blank line in " .. vim.inspect(text))
  end,

  ["orders the parts mention, diagnostic, function, blame"] = function()
    -- Each value names its own role, so a composer that walked the table with
    -- `pairs` (unordered) or reversed the sequence reports which order it used.
    local text = annotate.compose_text({
      blame = "fourth",
      func = "third",
      diagnostic = "second",
      mention = "first",
    })
    assert(text == "first\nsecond\nthird\nfourth", "composed " .. vim.inspect(text))
  end,

  ["joins with the separator it was given"] = function()
    -- The stored text is single-line, because herdr-nvim's comment listing
    -- cannot render a newline; the pure join still takes whichever separator
    -- the caller wants, so it reverts by changing one argument.
    local text = annotate.compose_text({
      mention = "@init.lua:1",
      diagnostic = "ERROR: undefined variable",
      func = "function setup",
      blame = "blame a1b2c3d",
    }, " | ")
    assert(
      text == "@init.lua:1 | ERROR: undefined variable | function setup | blame a1b2c3d",
      "composed " .. vim.inspect(text)
    )
  end,

  ["stores its parts on one line, separated by a pipe"] = function()
    -- The stored shape is an operator ruling (2026-09-05), not an
    -- implementation detail: herdr-nvim's listing cannot render a newline, and
    -- the parts still have to be told apart by whoever reads the annotation.
    assert(annotate.PART_SEPARATOR == " | ", "separator is " .. vim.inspect(annotate.PART_SEPARATOR))
    local text = annotate.compose_text({ mention = "@init.lua:1", blame = "blame a1b2c3d" }, annotate.PART_SEPARATOR)
    assert(not text:find("\n", 1, true), "stored text holds a newline: " .. vim.inspect(text))
    assert(text == "@init.lua:1 | blame a1b2c3d", "composed " .. vim.inspect(text))
  end,

  ["separates by newline when it was given no separator"] = function()
    local text = annotate.compose_text({ mention = "@init.lua:1", blame = "blame a1b2c3d" })
    assert(text == "@init.lua:1\nblame a1b2c3d", "composed " .. vim.inspect(text))
  end,

  ["never doubles the separator around a missing part"] = function()
    local text = annotate.compose_text({ mention = "@init.lua:1", blame = "blame a1b2c3d" }, " | ")
    assert(text == "@init.lua:1 | blame a1b2c3d", "composed " .. vim.inspect(text))
  end,

  ["treats an empty part as no part"] = function()
    -- `git.latest_commit` returns a normalized summary, but a diagnostic
    -- message trimmed to nothing arrives as "" rather than nil.
    local text = annotate.compose_text({ mention = "@init.lua:1", diagnostic = "", blame = "blame a1b2c3d" })
    assert(text == "@init.lua:1\nblame a1b2c3d", "composed " .. vim.inspect(text))
  end,

  ["composes nothing from no parts"] = function()
    assert(annotate.compose_text({}) == "", "composed " .. vim.inspect(annotate.compose_text({})))
  end,

  ["walks up to the enclosing function node"] = function()
    local node = chain({ "identifier", "arguments", "function_declaration", "chunk" })
    local found = annotate.enclosing_function(node)
    assert(found and found:type() == "function_declaration", "found " .. tostring(found and found:type()))
  end,

  ["reads the cursor node itself as the enclosing function"] = function()
    local found = annotate.enclosing_function(chain({ "function_definition", "chunk" }))
    assert(found and found:type() == "function_definition", "found " .. tostring(found and found:type()))
  end,

  ["stops at the innermost function-shaped ancestor"] = function()
    local node = chain({ "identifier", "method_definition", "class_definition", "function_declaration" })
    local found = annotate.enclosing_function(node)
    assert(found and found:type() == "method_definition", "found " .. tostring(found and found:type()))
  end,

  ["finds no function outside one"] = function()
    assert(annotate.enclosing_function(chain({ "identifier", "table_constructor", "chunk" })) == nil)
    assert(annotate.enclosing_function(nil) == nil)
  end,

  ["does not read a merely function-adjacent node as a function"] = function()
    -- The rule is a SUFFIX match, so `function_call` and `parameters` are not
    -- functions; a plain `find` on "function" would take the first of these.
    local node = chain({ "identifier", "function_call", "parameters", "function_definition" })
    local found = annotate.enclosing_function(node)
    assert(found and found:type() == "function_definition", "found " .. tostring(found and found:type()))
  end,

  ["covers the function node of every language this config edits"] = function()
    -- Measured against the installed grammars, one sample file each: Rust
    -- names its functions `function_item`, Go methods `method_declaration`,
    -- and a named TypeScript function expression `function_expression`. None
    -- of the three ends in a Lua or Python spelling.
    for _, node_type in ipairs({
      "function_definition",
      "function_declaration",
      "method_definition",
      "function_item",
      "method_declaration",
      "function_expression",
    }) do
      local found = annotate.enclosing_function(chain({ "identifier", node_type }))
      assert(found and found:type() == node_type, node_type .. " is not read as a function")
    end
  end,

  ["reads a suffix only at the end of the node type"] = function()
    -- `function_definition_call` CONTAINS a listed suffix without ending in
    -- one. A literal substring search would stop here and name the call site
    -- as the enclosing function.
    local node = chain({ "identifier", "function_definition_call", "function_declaration" })
    local found = annotate.enclosing_function(node)
    assert(found and found:type() == "function_declaration", "found " .. tostring(found and found:type()))
  end,

  ["annotates a named file buffer"] = function()
    assert(annotate.annotatable("lua/custom_api/annotate.lua", "") == true)
  end,

  ["annotates a named file that has never been written"] = function()
    -- A new file the operator has typed into is a real path with real lines,
    -- and the mention it produces is one an agent can open.
    assert(annotate.annotatable("notes.md", "") == true)
  end,

  ["refuses a buffer with no file name"] = function()
    -- The mention would be `@:1`, which names nothing.
    local ok, reason = annotate.annotatable("", "")
    assert(ok == false, "annotatable returned " .. tostring(ok))
    assert(type(reason) == "string" and reason ~= "", "reason " .. vim.inspect(reason))
  end,

  ["refuses a buffer that is not a file"] = function()
    -- Every non-empty `buftype` is something other than a file on disk: a
    -- scratch buffer, a terminal, a quickfix list, a help window.
    for _, buftype in ipairs({ "nofile", "nowrite", "acwrite", "terminal", "quickfix", "help", "prompt" }) do
      local ok, reason = annotate.annotatable("/tmp/x.lua", buftype)
      assert(ok == false, buftype .. " was accepted")
      assert(type(reason) == "string" and reason ~= "", buftype .. " gave no reason")
    end
  end,

  ["refuses a buffer whose name is a URI rather than a path"] = function()
    -- Measured: an Oil directory buffer and a Fugitive revision buffer both
    -- carry an EMPTY `buftype`, so the buftype rule above does not reach them.
    -- Their names are what gives them away.
    for _, name in ipairs({
      "oil:///Users/stephen/src/",
      "fugitive://./.git//5ef4e631b3f43a2ad5bbbdac634bdfad7a432706/nested.lua",
      "term://~//12345:bash",
      "octo://webdavis/dotfiles/pull/350",
    }) do
      local ok, reason = annotate.annotatable(name, "")
      assert(ok == false, name .. " was accepted")
      assert(type(reason) == "string" and reason ~= "", name .. " gave no reason")
    end
  end,

  ["annotates a path that merely contains a colon"] = function()
    -- The scheme is anchored at the start, so a legal filename holding `://`
    -- further along is still a path.
    assert(annotate.annotatable("/tmp/a:b/x.lua", "") == true)
    assert(annotate.annotatable("notes/http://example.md", "") == true)
  end,

  ["reads the function at the cursor's column, not at the start of its line"] = function()
    -- Column zero of a nested declaration line sits outside the function being
    -- declared, so this reported the function AROUND it.
    local text = annotated(NESTED, { 2, 18 })
    assert(text:find("function inner", 1, true), "annotated with " .. vim.inspect(text))
  end,

  ["names the severity beside the diagnostic message"] = function()
    local line = annotate.diagnostic_line({ severity = vim.diagnostic.severity.WARN, message = "unused local" })
    assert(line == "WARN: unused local", "diagnostic line " .. vim.inspect(line))
  end,

  ["collapses a multi-line diagnostic message onto one line"] = function()
    -- Every part is one line by contract, and a language server is free to
    -- send a message spanning several.
    local line = annotate.diagnostic_line({
      severity = vim.diagnostic.severity.ERROR,
      message = "expected type\n  found string",
    })
    assert(line == "ERROR: expected type found string", "diagnostic line " .. vim.inspect(line))
  end,

  ["has no diagnostic part when the message is only whitespace"] = function()
    -- A linter that reports a blank message would otherwise contribute the
    -- bare severity label, `ERROR: `, as a part of its own.
    assert(annotate.diagnostic_line({ severity = vim.diagnostic.severity.ERROR, message = "   " }) == nil)
    assert(annotate.diagnostic_line({ severity = vim.diagnostic.severity.ERROR, message = "" }) == nil)
  end,

  ["has no diagnostic part without a diagnostic"] = function()
    assert(annotate.diagnostic_line(nil) == nil)
  end,

  ["names the blame commit when the line was last touched by HEAD"] = function()
    local line = annotate.blame_line("a1b2c3d4e5f6", { hash = "a1b2c3d", summary = "add the annotator" })
    assert(line == "blame a1b2c3d add the annotator", "blame line " .. vim.inspect(line))
  end,

  ["gives the blame sha alone when HEAD is a different commit"] = function()
    -- `git.latest_commit` only ever describes HEAD, so attaching its summary to
    -- an older blame sha would caption the line with the wrong commit message.
    local line = annotate.blame_line("a1b2c3d4e5f6", { hash = "9999999", summary = "unrelated work" })
    assert(line == "blame a1b2c3d", "blame line " .. vim.inspect(line))
  end,

  ["gives the blame sha alone when HEAD could not be read"] = function()
    assert(annotate.blame_line("a1b2c3d4e5f6", nil) == "blame a1b2c3d")
    assert(annotate.blame_line("a1b2c3d4e5f6", { hash = "a1b2c3d" }) == "blame a1b2c3d")
  end,

  ["has no blame part without a sha"] = function()
    assert(annotate.blame_line(nil, { hash = "a1b2c3d", summary = "add the annotator" }) == nil)
  end,
}
