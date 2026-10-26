-- mini.ai: `a`/`i` text objects that search forward when the cursor is not
-- inside one.
--
-- Keys other plugins map win over mini.ai's, so this spec fills only the gaps.
-- `f` is a function definition from tree-sitter, and `u` is mini.ai's own
-- function call. Next and last sit on `N` and `L`, because `n` and `l` belong to
-- Neovim's grow/shrink selection and treesitter's loop.

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
