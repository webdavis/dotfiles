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
  local nvim_mcp_private = tonumber("700", 8)
  -- The canonical record pathname, set only once one has actually been
  -- published. The exit path below deletes through THIS, never through the
  -- configured string.
  local nvim_mcp_published = nil

  -- nvim_mcp_unsafe(dir) -- why <dir> is not a private directory this user
  -- controls all the way up, nil when it is one.
  --
  -- lstat, never stat, so a symlink standing where the registry belongs reads
  -- as what it is rather than as whatever it points at. And every ancestor is
  -- checked too: a directory another account can write, without the sticky bit
  -- that stops them removing what is not theirs, is one where they can replace
  -- the whole subtree between any two of the operations below. An ancestor
  -- owned by root is fine, which is what /, /Users and /var are.
  local function nvim_mcp_unsafe(dir)
    local uid = vim.uv.getuid()
    local info = vim.uv.fs_lstat(dir)
    if not info or info.type ~= "directory" then
      return dir .. " is not a directory"
    end
    if info.uid ~= uid then
      return dir .. " is owned by another account"
    end
    if info.mode % 512 ~= nvim_mcp_private then
      return dir .. " is not mode 0700"
    end
    -- The argument is already canonical, so its ancestors carry no symlink
    -- components. Walking an unresolved path instead would reject every
    -- ordinary macOS temp directory, since /var and /tmp are both symlinks
    -- into /private there.
    local current = dir
    while true do
      local parent = vim.fs.dirname(current)
      if parent == current then
        return nil
      end
      local up = vim.uv.fs_lstat(parent)
      if not up or up.type ~= "directory" then
        return parent .. " is not a directory"
      end
      if up.uid ~= uid and up.uid ~= 0 then
        return parent .. " is owned by another account"
      end
      local perm = up.mode % 4096
      local sticky = math.floor(perm / 512) % 2 == 1
      local shared_write = math.floor(perm / 16) % 2 == 1 or math.floor(perm / 2) % 2 == 1
      if shared_write and not sticky then
        return parent .. " is writable by other accounts and is not sticky"
      end
      current = parent
    end
  end

  vim.api.nvim_create_autocmd("VimEnter", {
    group = augroup("nvim_mcp_registry"),
    callback = function()
      -- Empty when Neovim was started with no socket at all; there is then
      -- nothing for the resolver to connect to.
      if vim.v.servername == "" then
        return
      end
      -- A record is one line of WHITESPACE-SEPARATED fields, so a pathname
      -- carrying a space, a tab or a newline cannot be written down
      -- unambiguously, and a NUL cannot be written down at all. Neovim binds
      -- every one of those happily and answers on it, so a record for one would
      -- leave the resolver reading the pathname only as far as the first space,
      -- probing a name nothing answers on, and DELETING the record of a healthy
      -- instance. Refusing to register, once and out loud, is better than
      -- turning a live editor into stale state.
      if vim.v.servername:find("[%s%z]") then
        vim.notify(
          "nvim-mcp: not registering, the --listen path contains whitespace or NUL: " .. vim.inspect(vim.v.servername),
          vim.log.levels.WARN
        )
        return
      end

      -- NOT race free, and the failure is deliberately ignored: mkdir tests for
      -- each component and then creates it, so two instances starting together
      -- both find the directory missing and the loser raises E739. Whether this
      -- instance created the directory or found it makes no difference, because
      -- the validation below is what decides whether to go on.
      pcall(vim.fn.mkdir, nvim_mcp_dir, "p", nvim_mcp_private)

      -- Resolved ONCE, here, and every operation below goes through the
      -- canonical result. Validating one pathname and then opening, renaming
      -- and deleting through another leaves a window in which any symlink
      -- along the configured path can be swapped for something else, and a
      -- shared directory anywhere above it is enough for another account to do
      -- exactly that. The leaf is checked before resolving, because an alias
      -- standing where the registry belongs is not something to follow.
      local leaf = vim.uv.fs_lstat(nvim_mcp_dir)
      if not leaf or leaf.type ~= "directory" then
        vim.notify("nvim-mcp: not registering, " .. nvim_mcp_dir .. " is not a directory", vim.log.levels.WARN)
        return
      end
      local canonical = vim.uv.fs_realpath(nvim_mcp_dir)
      if not canonical then
        vim.notify("nvim-mcp: not registering, " .. nvim_mcp_dir .. " cannot be resolved", vim.log.levels.WARN)
        return
      end
      -- Validated on EVERY start, not only when the create failed: a registry
      -- left at 0777 by anything at all would otherwise be written into without
      -- a word, and it is a directory any account could then plant a record in.
      local unsafe = nvim_mcp_unsafe(canonical)
      if unsafe then
        vim.notify("nvim-mcp: not registering, " .. unsafe, vim.log.levels.WARN)
        return
      end
      local record = canonical .. "/" .. vim.fn.getpid()
      local temp = record .. ".tmp"

      -- O_EXCL through "wx", never io.open(..., "w"), which FOLLOWS whatever is
      -- at the name: a symlink planted at the temp path redirects the write
      -- into any file this user can write, truncating it and then publishing it
      -- as a record. "wx" fails on an existing name instead. A leftover .tmp
      -- from a crash blocks only that one pid, which changes on restart.
      local fd = vim.uv.fs_open(temp, "wx", tonumber("600", 8))
      if not fd then
        return
      end
      local line = table.concat({ nvim_mcp_pane, vim.fn.getpid(), vim.v.servername, vim.fn.getcwd() }, " ")
      vim.uv.fs_write(fd, line .. "\n")
      vim.uv.fs_close(fd)
      -- Rename rather than write in place, so the resolver never reads a record
      -- that is only half written.
      vim.uv.fs_rename(temp, record)
      nvim_mcp_published = record
    end,
  })

  vim.api.nvim_create_autocmd("VimLeavePre", {
    group = augroup("nvim_mcp_registry_leave"),
    callback = function()
      -- Only what THIS instance actually published, and only through the
      -- canonical pathname it was published at. A refused registration has
      -- nothing to clean up, and deleting a file that merely shares our pid in
      -- a directory we declined to write to would be somebody else's data.
      if nvim_mcp_published then
        os.remove(nvim_mcp_published)
      end
    end,
  })
end
