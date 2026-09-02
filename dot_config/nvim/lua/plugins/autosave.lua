-- Some recommended exclusions. You can use `:lua print(vim.bo.filetype)` to
-- get the filetype string for the current buffer.
local excluded_filetypes = {
  "gitcommit",
  "NvimTree",
  "Outline",
  "TelescopePrompt",
  "alpha",
  "dashboard",
  "lazygit",
  "neo-tree",
  "oil",
  "prompt",
  "toggleterm",
  "harpoon",
}

local excluded_filenames = {
  "do-not-autosave-me.lua",
}

local function save_condition(buf)
  if
    vim.tbl_contains(excluded_filetypes, vim.fn.getbufvar(buf, "&filetype"))
    or vim.tbl_contains(excluded_filenames, vim.fn.expand("%:t"))
  then
    return false
  end
  return true
end

return {
  "okuuva/auto-save.nvim",
  dependencies = {
    "folke/snacks.nvim",
  },
  version = "^1.0.0",
  opts = {
    enabled = false,
    trigger_events = {
      immediate_save = { "BufLeave", "FocusLost", "QuitPre", "VimSuspend" },
      defer_save = { "InsertLeave", "TextChanged" },
      cancel_deferred_save = { "InsertEnter" },
    },
    condition = save_condition,
    write_all_buffers = false,
    noautocmd = false,
    lockmarks = false,
    debounce_delay = 1000,
    debug = false,
  },
  config = function(_, opts)
    local autosave = require("auto-save")
    autosave.setup(opts)

    require("snacks")
      .toggle({
        name = "Autosave",
        get = function()
          return opts.enabled
        end,
        set = function(on)
          opts.enabled = on

          if on then
            autosave.on()
          else
            autosave.off()
          end
        end,
      })
      :map("<leader>uv")
  end,
}
