return {
  "ThePrimeagen/harpoon",
  branch = "harpoon2",
  opts = {
    menu = {
      width = vim.api.nvim_win_get_width(0) - 4,
    },
    settings = {
      save_on_toggle = false,
    },
  },
  config = function(_, opts)
    local harpoon = require("harpoon")

    harpoon.setup(opts)

    map({
      mode = "n",
      lhs = "<leader>ha",
      rhs = function()
        harpoon:list():add()
      end,
      desc = "Harpoon: mark current file",
    })

    map({
      mode = "n",
      lhs = "<leader>hh",
      rhs = function()
        harpoon.ui:toggle_quick_menu(harpoon:list())
      end,
      desc = "Harpoon: open file menu",
    })

    map({
      mode = "n",
      lhs = { "<C-p>", "<leader>hp" },
      rhs = function()
        harpoon:list():prev()
      end,
      desc = "Harpoon: jump to previous file",
    })

    map({
      mode = "n",
      lhs = { "<C-n>", "<leader>hn" },
      rhs = function()
        harpoon:list():next()
      end,
      desc = "Harpoon: jump to next file",
    })

    for i = 1, 5 do
      map({
        mode = "n",
        lhs = "<leader>" .. i,
        rhs = function()
          harpoon:list():select(i)
        end,
        desc = "Harpoon: jump to file " .. i,
      })
    end
  end,
}
