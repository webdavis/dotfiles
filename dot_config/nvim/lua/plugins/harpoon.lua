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
  -- harpoon2 sizes the quick menu from the `toggle_quick_menu` argument, so a `menu` table here has no effect.
  opts = {
    settings = {
      save_on_toggle = false,
    },
  },
  keys = keys,
  config = function(_, opts)
    require("harpoon").setup(opts)
  end,
}
