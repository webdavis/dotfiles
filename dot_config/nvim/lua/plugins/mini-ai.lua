-- mini.ai: `a`/`i` text objects that search forward when the cursor is not
-- inside one, the job targets.vim used to do. Its built-in objects cover
-- brackets, quotes, `b` (any bracket), `q` (any quote), `a` (argument), `t`
-- (tag) and any punctuation as a separator (`i,`, `i|`, ...).
--
-- Every key another plugin maps explicitly (treesitter's `ac`, `il`, `as`,
-- nvim-various-textobjs' `id`, `ik`, ...) still wins over mini.ai's id of the
-- same letter, so this spec only fills the gaps:
--   * `f` is a function DEFINITION from tree-sitter; nothing else maps `if`/`af`.
--   * `u` is a function call, mini.ai's own `f`.
--   * Next and last are `iN`/`aN` and `iL`/`aL` (`ciN(` changes inside the next
--     parentheses). mini.ai's default `in`/`il` belong to Neovim's grow/shrink
--     selection and treesitter's loop, and `iN`/`aN` are free because the
--     notebook-cell object is disabled in plugins/textobjects.lua.

-- which-key lists mini.ai's ids after `i` and `a`; mini.ai reads the id itself,
-- so these rows are labels only. Its own text-object preset already labels the
-- built-in ids (`i(`, `i"`, `it`, ...) right after `i` and `a`.
local mini_only = {
  { "b", "any bracket" },
  { "q", "any quote" },
  { "a", "argument" },
  { "f", "function" },
  { "u", "function call" },
  { "?", "ask for left and right edges" },
}
local builtin = {
  { "(", "parentheses" },
  { "[", "brackets" },
  { "{", "braces" },
  { "<", "angle brackets" },
  { '"', "double quotes" },
  { "'", "single quotes" },
  { "`", "backticks" },
  { "t", "tag" },
}

local function which_key_spec()
  local spec = { mode = { "o", "x" } }
  for _, prefix in ipairs({ "i", "a" }) do
    for _, label in ipairs(mini_only) do
      table.insert(spec, { prefix .. label[1], desc = label[2] })
    end
  end
  for _, prefix in ipairs({
    { "iN", "inside next" },
    { "aN", "around next" },
    { "iL", "inside last" },
    { "aL", "around last" },
  }) do
    table.insert(spec, { prefix[1], group = prefix[2] })
    for _, list in ipairs({ mini_only, builtin }) do
      for _, label in ipairs(list) do
        table.insert(spec, { prefix[1] .. label[1], desc = label[2] })
      end
    end
  end
  return spec
end

return {
  {
    "folke/which-key.nvim",
    optional = true,
    opts = { spec = { which_key_spec() } },
  },
  {
    "nvim-mini/mini.ai",
    event = "VeryLazy",
    opts = function()
      local ai = require("mini.ai")
      local function_definition = ai.gen_spec.treesitter({ a = "@function.outer", i = "@function.inner" })
      return {
        n_lines = 500,
        custom_textobjects = {
          -- `af` is linewise, so `daf` takes the function's lines with it instead
          -- of leaving a blank one behind.
          f = function(ai_type, id, opts)
            local regions = function_definition(ai_type, id, opts)
            if ai_type == "a" then
              for _, region in ipairs(regions) do
                region.vis_mode = "V"
              end
            end
            return regions
          end,
          u = ai.gen_spec.function_call(),
        },
        mappings = {
          around_next = "aN",
          inside_next = "iN",
          around_last = "aL",
          inside_last = "iL",
        },
      }
    end,
  },
}
