local excluded_filetypes = {
  "gitcommit",
  "NvimTree",
  "Outline",
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
  -- Skips claudecode.nvim's proposed-edit buffers, because a timed write would accept the edit.
  return require("custom_api.autosave").should_save(buf)
end

return {
  "okuuva/auto-save.nvim",
  dependencies = {
    "folke/snacks.nvim",
  },
  version = "^1.0.0",
  -- Auto-save starts disabled, so the toggle key is the only load trigger.
  keys = { { "<leader>uv", desc = "Toggle Autosave" } },
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

    -- conform skips format-on-save while this flag is set, so a timed write leaves the cursor alone.
    local write_flag_group = vim.api.nvim_create_augroup("AutoSaveWriteFlag", { clear = true })

    vim.api.nvim_create_autocmd("User", {
      group = write_flag_group,
      pattern = "AutoSaveWritePre",
      callback = function(args)
        require("custom_api.autosave").mark_write(args.data.saved_buffer)
      end,
    })

    vim.api.nvim_create_autocmd("User", {
      group = write_flag_group,
      pattern = "AutoSaveWritePost",
      callback = function(args)
        require("custom_api.autosave").clear_write(args.data.saved_buffer)
      end,
    })

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
