local function act(fn)
  return function()
    fn(require("harpoon"))
  end
end

local add = act(function(h)
  h:list():add()
end)
local menu = act(function(h)
  h.ui:toggle_quick_menu(h:list())
end)
local prev = act(function(h)
  h:list():prev()
end)
local next_file = act(function(h)
  h:list():next()
end)

local keys = {
  { "<leader>ha", add, desc = "Harpoon: mark current file", silent = true },
  { "<leader>hh", menu, desc = "Harpoon: open file menu", silent = true },
  { "<C-p>", prev, desc = "Harpoon: jump to previous file", silent = true },
  { "<leader>hp", prev, desc = "Harpoon: jump to previous file", silent = true },
  { "<C-n>", next_file, desc = "Harpoon: jump to next file", silent = true },
  { "<leader>hn", next_file, desc = "Harpoon: jump to next file", silent = true },
}
for i = 1, 5 do
  keys[#keys + 1] = {
    "<leader>" .. i,
    act(function(h)
      h:list():select(i)
    end),
    desc = "Harpoon: jump to file " .. i,
    silent = true,
  }
end

return {
  "ThePrimeagen/harpoon",
  branch = "harpoon2",
  -- No `menu` table here: that spelling is harpoon v1. On harpoon2 the quick
  -- menu is sized inside ui.lua from the toggle_quick_menu argument
  -- (`ui_width_ratio`, default 0.62569 of the editor), and merge_config files an
  -- unrecognized top-level key as a per-list entry, so a `menu.width` in setup
  -- reaches harpoon.config and never the window (measured: 5 and 200 both open
  -- the same menu).
  opts = {
    settings = {
      save_on_toggle = false,
    },
  },
  -- Each row IS the mapping: lazy sets a placeholder at startup and installs this
  -- same rhs when the plugin loads, so there is no second copy in `config`.
  -- Nothing else loads harpoon at startup: the only other mentions of it are
  -- catppuccin's highlight-integration flag (ui.lua) and an auto-save filetype
  -- exclusion, neither of which requires the module.
  keys = keys,
  config = function(_, opts)
    require("harpoon").setup(opts)
  end,
}
