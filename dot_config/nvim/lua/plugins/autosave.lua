-- Some recommended exclusions. You can use `:lua print(vim.bo.filetype)` to
-- get the filetype string for the current buffer.
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
  -- claudecode.nvim's proposed-edit buffers, which a timed write would accept.
  return require("custom_api.autosave").should_save(vim.api.nvim_buf_get_name(buf), vim.fn.getbufvar(buf, "&buftype"))
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

    -- An automatic write must not reformat the buffer under the operator's
    -- cursor, so it announces itself and lsp-format's BufWritePre handler
    -- (plugins/lsp.lua) stands down while the flag is set. An explicit `:w`
    -- never sets it and keeps formatting.
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
