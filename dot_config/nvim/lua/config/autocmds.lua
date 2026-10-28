local function augroup(name)
  return vim.api.nvim_create_augroup("nvim_config_" .. name, { clear = true })
end

-- Reload files changed outside Neovim when focus returns.
vim.api.nvim_create_autocmd({ "FocusGained", "TermClose", "TermLeave" }, {
  group = augroup("checktime"),
  callback = function()
    if vim.o.buftype ~= "nofile" then
      vim.cmd("checktime")
    end
  end,
})

-- Watch each file buffer so a write from another pane reloads it while Neovim sits idle.
local auto_reload = require("custom_api.auto_reload")
-- Both autocmds share one group, because each augroup() call clears it.
local auto_reload_group = augroup("auto_reload")

vim.api.nvim_create_autocmd({ "BufReadPost", "BufWritePost" }, {
  group = auto_reload_group,
  callback = function(event)
    if vim.bo[event.buf].buftype == "" and vim.api.nvim_buf_get_name(event.buf) ~= "" then
      auto_reload.watch(event.buf)
    end
  end,
})

vim.api.nvim_create_autocmd({ "BufDelete", "BufUnload" }, {
  group = auto_reload_group,
  callback = function(event)
    auto_reload.unwatch(event.buf)
  end,
})

vim.api.nvim_create_autocmd("TextYankPost", {
  group = augroup("highlight_yank"),
  callback = function()
    (vim.hl or vim.highlight).on_yank()
  end,
})

vim.api.nvim_create_autocmd({ "VimResized" }, {
  group = augroup("resize_splits"),
  callback = function()
    local current_tab = vim.fn.tabpagenr()
    vim.cmd("tabdo wincmd =")
    vim.cmd("tabnext " .. current_tab)
  end,
})

vim.api.nvim_create_autocmd("BufReadPost", {
  group = augroup("last_loc"),
  callback = function(event)
    local exclude = { "gitcommit" }
    local buf = event.buf
    if vim.tbl_contains(exclude, vim.bo[buf].filetype) or vim.b[buf].nvim_config_last_loc then
      return
    end
    vim.b[buf].nvim_config_last_loc = true
    local mark = vim.api.nvim_buf_get_mark(buf, '"')
    local lcount = vim.api.nvim_buf_line_count(buf)
    if mark[1] > 0 and mark[1] <= lcount then
      pcall(vim.api.nvim_win_set_cursor, 0, mark)
    end
  end,
})

vim.api.nvim_create_autocmd("FileType", {
  group = augroup("close_with_q"),
  pattern = {
    "PlenaryTestPopup",
    "checkhealth",
    "dbout",
    "gitsigns-blame",
    "grug-far",
    "help",
    "lspinfo",
    "neotest-output",
    "neotest-output-panel",
    "neotest-summary",
    "qf",
    "spectre_panel",
    "startuptime",
    "tsplayground",
  },
  callback = function(event)
    vim.bo[event.buf].buflisted = false
    vim.schedule(function()
      vim.keymap.set("n", "q", function()
        vim.cmd("close")
        pcall(vim.api.nvim_buf_delete, event.buf, { force = true })
      end, {
        buffer = event.buf,
        silent = true,
        desc = "Quit buffer",
      })
    end)
  end,
})

-- Unlist man pages so they close easily.
vim.api.nvim_create_autocmd("FileType", {
  group = augroup("man_unlisted"),
  pattern = { "man" },
  callback = function(event)
    vim.bo[event.buf].buflisted = false
  end,
})

-- Sets a two-space indent after the file-type plugins that set four.
-- A project's .editorconfig still wins.
vim.api.nvim_create_autocmd("FileType", {
  group = augroup("indent_two"),
  callback = function(event)
    vim.bo[event.buf].shiftwidth = 2
    vim.bo[event.buf].tabstop = 2
    vim.bo[event.buf].softtabstop = 2
  end,
})

vim.api.nvim_create_autocmd("FileType", {
  group = augroup("wrap_spell"),
  pattern = { "text", "plaintex", "typst", "gitcommit" },
  callback = function()
    vim.opt_local.wrap = true
    vim.opt_local.spell = true
  end,
})

-- Show JSON quotes instead of concealing them.
vim.api.nvim_create_autocmd({ "FileType" }, {
  group = augroup("json_conceal"),
  pattern = { "json", "jsonc", "json5" },
  callback = function()
    vim.opt_local.conceallevel = 0
  end,
})

vim.api.nvim_create_autocmd({ "BufWritePre" }, {
  group = augroup("auto_create_dir"),
  callback = function(event)
    if event.match:match("^%w%w+:[\\/][\\/]") then
      return
    end
    local file = vim.uv.fs_realpath(event.match) or event.match
    vim.fn.mkdir(vim.fn.fnamemodify(file, ":p:h"), "p")
  end,
})

-- Close sidebar windows when only they and floating windows remain.
vim.api.nvim_create_autocmd("QuitPre", {
  group = augroup("close_sidebars_on_quit"),
  callback = function()
    local sidebar_windows = {}
    local floating_windows = {}
    local windows = vim.api.nvim_list_wins()
    for _, w in ipairs(windows) do
      local filetype = vim.api.nvim_get_option_value("filetype", { buf = vim.api.nvim_win_get_buf(w) })
      if filetype:match("snacks_") ~= nil or filetype == "OverseerList" or filetype == "aerial" then
        table.insert(sidebar_windows, w)
      elseif vim.api.nvim_win_get_config(w).relative ~= "" then
        table.insert(floating_windows, w)
      end
    end
    if
      1 == #windows - #floating_windows - #sidebar_windows
      and vim.api.nvim_win_get_config(vim.api.nvim_get_current_win()).relative == ""
    then
      for _, w in ipairs(sidebar_windows) do
        vim.api.nvim_win_close(w, true)
      end
    end
  end,
})

vim.api.nvim_create_autocmd("User", {
  pattern = "AutoSaveWritePost",
  group = augroup("auto_save"),
  callback = function(opts)
    if opts.data.saved_buffer ~= nil then
      local buffer = opts.data.saved_buffer
      local path = vim.api.nvim_buf_get_name(buffer)

      local filename = vim.fn.fnamemodify(path, ":t")
      local time = vim.fn.strftime("%H:%M:%S")

      vim.api.nvim_echo({
        { "AutoSave: saved ", "Comment" },
        { filename, "String" },
        { " at ", "Comment" },
        { time, "Number" },
      }, false, {})
    end
  end,
})
