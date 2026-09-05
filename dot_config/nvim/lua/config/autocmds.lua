-- ╭──────────────╮
-- │   Autocmds   │
-- ╰──────────────╯

-- If needed, this is a good place to remove existing autocmds by group name.
-- For example, uncomment the following line to remove the "nvim_config_auto_create_dir" group:
--
-- vim.api.nvim_del_augroup_by_name("nvim_config_auto_create_dir")

local function augroup(name)
  return vim.api.nvim_create_augroup("nvim_config_" .. name, { clear = true })
end

-- Check if we need to reload the file when it changed.
vim.api.nvim_create_autocmd({ "FocusGained", "TermClose", "TermLeave" }, {
  group = augroup("checktime"),
  callback = function()
    if vim.o.buftype ~= "nofile" then
      vim.cmd("checktime")
    end
  end,
})

-- Follow the file when something outside Neovim writes it (spec 5.4). The
-- group above only fires on focus change, which never happens while Neovim
-- sits idle in one herdr pane and an agent writes the file from another, so a
-- normal file buffer also watches its own file.
local auto_reload = require("custom_api.auto_reload")
-- Held in a local and reused below, because `augroup` above passes
-- `clear = true`: calling it a second time for this group would delete the
-- autocmd the first call registered.
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

-- Highlight on yank.
vim.api.nvim_create_autocmd("TextYankPost", {
  group = augroup("highlight_yank"),
  callback = function()
    (vim.hl or vim.highlight).on_yank()
  end,
})

-- Resize splits if window got resized.
vim.api.nvim_create_autocmd({ "VimResized" }, {
  group = augroup("resize_splits"),
  callback = function()
    local current_tab = vim.fn.tabpagenr()
    vim.cmd("tabdo wincmd =")
    vim.cmd("tabnext " .. current_tab)
  end,
})

-- Go to last loc when opening a buffer.
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

-- Close some filetypes with <q>.
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

-- Make it easier to close man-files when opened inline.
vim.api.nvim_create_autocmd("FileType", {
  group = augroup("man_unlisted"),
  pattern = { "man" },
  callback = function(event)
    vim.bo[event.buf].buflisted = false
  end,
})

-- Wrap and check for spell in text filetypes.
vim.api.nvim_create_autocmd("FileType", {
  group = augroup("wrap_spell"),
  pattern = { "text", "plaintex", "typst", "gitcommit" },
  callback = function()
    vim.opt_local.wrap = true
    vim.opt_local.spell = true
  end,
})

-- Fix conceallevel for json files
vim.api.nvim_create_autocmd({ "FileType" }, {
  group = augroup("json_conceal"),
  pattern = { "json", "jsonc", "json5" },
  callback = function()
    vim.opt_local.conceallevel = 0
  end,
})

-- Auto create directory when saving a file, in case some intermediate directory does not exist.
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

-- Close sidebar windows (Snacks Explorer, Overseer) when they are the last non-floating windows.
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

-- Tell the nvim-mcp resolver which Neovim lives in which herdr pane (spec 7.3).
-- ~/.local/libexec/nvim-mcp/nvim-mcp-connect.sh reads this registry to answer
-- "which Neovim does this agent mean" by pane rather than by focus or by
-- current directory, neither of which can tell two Neovim panes in one
-- workspace apart.
--
-- ONE FILE PER INSTANCE, named for its pid, holding
-- "<pane id> <pid> <socket> <cwd>" (cwd last, the only field that can hold
-- spaces). A single shared file loses an update when two instances start at
-- once, and keys a nested Neovim over its parent because both inherit the same
-- pane id; per-pid files make the nested pair two candidates the resolver's
-- picker names instead of one that silently wins.
--
-- Written to a temp name and renamed, which is atomic within the directory, so
-- the resolver never reads a half-written record. Each instance removes only
-- its own file, so no exit can delete another's. A record left by a crash is
-- pruned by the resolver's identity check.
--
-- Written only under herdr: without HERDR_PANE_ID there is no pane to register
-- and the resolver has nothing to match against.
local nvim_mcp_pane = vim.env.HERDR_PANE_ID
if nvim_mcp_pane and nvim_mcp_pane ~= "" then
  local nvim_mcp_dir = (vim.env.XDG_STATE_HOME or (vim.env.HOME .. "/.local/state")) .. "/nvim-mcp/registry"
  local nvim_mcp_record = nvim_mcp_dir .. "/" .. vim.fn.getpid()

  vim.api.nvim_create_autocmd("VimEnter", {
    group = augroup("nvim_mcp_registry"),
    callback = function()
      -- Empty when Neovim was started with no socket at all; there is then
      -- nothing for the resolver to connect to.
      if vim.v.servername == "" then
        return
      end
      -- 0700, because the resolver refuses to read or prune a registry any
      -- other account could have planted a record in.
      --
      -- NOT race free, which is why the failure is caught rather than allowed
      -- to propagate: mkdir tests for each component and then creates it, so
      -- two instances starting together both find the directory missing and
      -- the loser raises E739 out of the create and registers nothing. One
      -- record then survives where there are two live editors, and the
      -- resolver, whose fallback only runs when the registry yields nothing,
      -- selects it silently instead of offering the picker. Losing the race is
      -- fine; what matters is that the winner left the private directory this
      -- design expects.
      local made = pcall(vim.fn.mkdir, nvim_mcp_dir, "p", tonumber("700", 8))
      if not made then
        local info = vim.uv.fs_stat(nvim_mcp_dir)
        if
          not info
          or info.type ~= "directory"
          or info.uid ~= vim.uv.getuid()
          or info.mode % 512 ~= tonumber("700", 8)
        then
          return
        end
      end
      local temp = nvim_mcp_record .. ".tmp"
      local file = io.open(temp, "w")
      if not file then
        return
      end
      file:write(table.concat({ nvim_mcp_pane, vim.fn.getpid(), vim.v.servername, vim.fn.getcwd() }, " "), "\n")
      file:close()
      os.rename(temp, nvim_mcp_record)
    end,
  })

  vim.api.nvim_create_autocmd("VimLeavePre", {
    group = augroup("nvim_mcp_registry_leave"),
    callback = function()
      os.remove(nvim_mcp_record)
    end,
  })
end
