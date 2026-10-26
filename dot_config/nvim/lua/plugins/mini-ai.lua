-- mini.ai: `a`/`i` text objects that search forward when the cursor is not
-- inside one, the job targets.vim used to do. Its built-in objects cover
-- brackets, quotes, `b` (any bracket), `q` (any quote), `a` (argument), `t`
-- (tag) and any punctuation as a separator (`i,`, `i|`, ...).
--
-- Every key another plugin maps explicitly (treesitter's `ac`, `il`, `as`,
-- nvim-various-textobjs' `in`, `iq`, `ik`, ...) still wins over mini.ai's id of
-- the same letter, so this spec only fills the gaps:
--   * `f` is a function DEFINITION from tree-sitter; nothing else maps `if`/`af`.
--   * `u` is a function call, mini.ai's own `f`.
--   * Next and last are `iN`/`aN` and `iL`/`aL` (`ciN(` changes inside the next
--     parentheses). mini.ai's default `in`/`il` belong to nvim-various-textobjs'
--     number and treesitter's loop, and `iN`/`aN` are free because the
--     notebook-cell object is disabled in plugins/textobjects.lua.
return {
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
}
