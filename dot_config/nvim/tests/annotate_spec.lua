-- custom_api.annotate, the line annotator's pure core (spec 7.7).
--
-- Only the pure parts are pinned here. `annotate.line()` reads the cursor, the
-- diagnostic store, a treesitter tree and git, and hands its text to
-- `herdr-nvim`; none of that is ours to assert, and a fake herdr would be a
-- test of a plugin we did not write. What IS ours is the composer, the
-- node-type filter and the blame formatter, and each takes plain data.

local annotate = require("custom_api.annotate")

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

  ["reads a suffix only at the end of the node type"] = function()
    -- `function_definition_call` CONTAINS a listed suffix without ending in
    -- one. A literal substring search would stop here and name the call site
    -- as the enclosing function.
    local node = chain({ "identifier", "function_definition_call", "function_declaration" })
    local found = annotate.enclosing_function(node)
    assert(found and found:type() == "function_declaration", "found " .. tostring(found and found:type()))
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
