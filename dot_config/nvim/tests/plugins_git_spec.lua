-- Two subjects in `lua/plugins/git.lua`, sharing one load of the file.
--
-- The `<C-g>` guards in `lua/plugins/git.lua` (B87). `github.repo` and
-- `github.account` answer `nil, message` when `gh` cannot, and every keymap that
-- reads a field off either used to raise E5108 on the index.
--
-- The Fugitive spec's `config` is run with every `custom_api` module replaced by
-- a fake, `map` captured instead of installed, and each sink counted, so the
-- twelve affected keys can be invoked here with no `gh`, no repository and no
-- user interface. The fakes mirror the real modules' argument contracts (a
-- missing `repo_name` raises, as `git.latest_commit` does), so a dropped guard
-- fails a case rather than passing quietly.
--
-- And `<leader>gM`, whose callback decides between focusing the blame window
-- this file already has open and opening a new one. Real windows and real
-- buffer names carry that decision, so the cases below name buffers the way
-- gitsigns and Fugitive name theirs and read back which window ended up
-- current.

-- The runner prepends `<config_root>/lua/?.lua` to package.path, and it invokes
-- a spec with `dofile`, which passes no arguments at all. A vararg here would
-- therefore always fall through to the source beside this file, ignore
-- `--config`, and pass while pointed at a config with none of the guards in it.
local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

-- What each fake was asked to do. Every counter must stay at zero for a guarded
-- key: reaching any of them means the callback ran past its guard.
local calls = {}

local function reset_calls()
  calls.notifications = {}
  calls.prompts = 0
  calls.commands = 0
  calls.clipboard = 0
  calls.overseer = 0
end

reset_calls()

-- The scenario the fake `github` is answering for. Mutated between cases; the
-- captured callbacks index this table at call time, so one load covers all three.
local github_fake = {}
local git_fake = {}
local util_fake = {}
local overseer_fake = {}

-- gitsigns, down to the two functions `lua/plugins/git.lua` reaches: `setup` at
-- configure time, and `blame`, which is what `<leader>gM` calls when it decides
-- no blame window is open.
local blame_calls = 0
local gitsigns_fake = {
  setup = function() end,
  blame = function()
    blame_calls = blame_calls + 1
  end,
}

local function set_scenario(scenario)
  if scenario == "account_fails" then
    github_fake.account = function()
      return nil, "no gitconfig user.name and not logged into GitHub CLI"
    end
    github_fake.repo = function()
      return { nameWithOwner = "owner/name", owner = "owner", name = "name" }
    end
  elseif scenario == "repo_fails" then
    github_fake.account = function()
      return { fullname = "Full Name", username = "username" }
    end
    github_fake.repo = function()
      return nil, "failed to get GitHub repository info"
    end
  elseif scenario == "repo_malformed" then
    github_fake.account = function()
      return { fullname = "Full Name", username = "username" }
    end
    -- A successful `gh` whose answer carried no slash: `repo` hands back a table
    -- with a nil `name`, which `git.latest_commit` and `git.url` both raise on.
    github_fake.repo = function()
      return { nameWithOwner = "noslash", owner = nil, name = nil }
    end
  else
    error("unknown scenario " .. tostring(scenario))
  end
end

git_fake.initialized = function()
  return true
end
git_fake.current_branch = function()
  return { name = "main", hash = "abc1234" }
end
git_fake.default_branch = function()
  return "main"
end
git_fake.all_branches = function()
  return {}
end
git_fake.blame_sha = function()
  return "abc1234"
end
git_fake.latest_commit = function(opts)
  opts = opts or {}
  if not opts.repo_name then
    error("Missing required argument `repo_name`")
  end
  return { hash = "abc1234", summary = "a commit", body = "" }
end
git_fake.url = function(opts)
  opts = opts or {}
  if not opts.remote then
    error("Missing required argument `remote`")
  end
  if not opts.account_name then
    error("Missing required argument `account_name`")
  end
  if not opts.repo_name then
    error("Missing required argument `repo_name`")
  end
  return "git@github.com:owner/name.git"
end
git_fake.copy_url_to_clipboard = function()
  calls.clipboard = calls.clipboard + 1
  return "copied"
end

util_fake.copy_to_system_clipboard = function()
  calls.clipboard = calls.clipboard + 1
end
util_fake.trim = function(value)
  return (tostring(value or ""):gsub("^%s+", ""):gsub("%s+$", ""))
end
util_fake.sanitize_input = function(value)
  return util_fake.trim(value):lower()
end
util_fake.normalize = function(value)
  local trimmed = util_fake.trim(value)
  return trimmed ~= "" and trimmed or nil
end
util_fake.get_cwd_basename = function()
  return "project"
end
util_fake.run_shell_command = function()
  return 0, ""
end

overseer_fake.overseer_runner = function()
  calls.overseer = calls.overseer + 1
end

-- `map` is a global installed by init.lua, which this runner never loads.
local captured = {}
local function capture_map(spec)
  local lhs_list = type(spec.lhs) == "table" and spec.lhs or { spec.lhs }
  for _, lhs in ipairs(lhs_list) do
    captured[lhs] = spec
  end
end

-- The runner loads every spec into ONE process, so nothing global may outlive a
-- call here. An earlier version left the fakes in `package.loaded` and broke
-- `util_spec`, which then required this file's `custom_api.util` instead of the
-- real one. Everything is saved and put back.
local MODULES = {
  ["custom_api.github"] = github_fake,
  ["custom_api.git"] = git_fake,
  ["custom_api.util"] = util_fake,
  ["custom_api.overseer"] = overseer_fake,
  ["gitsigns"] = gitsigns_fake,
}

local function with_fake_modules(fn)
  local saved = {}
  for name, fake in pairs(MODULES) do
    saved[name] = { package.loaded[name] }
    package.loaded[name] = fake
  end

  local ok, err = pcall(fn)

  for name, value in pairs(saved) do
    package.loaded[name] = value[1]
  end

  assert(ok, err)
end

-- The sinks a callback past its guard would reach, in place only while one runs.
local function with_fake_sinks(fn)
  local real = {
    notify = vim.notify,
    input = vim.ui.input,
    cmd = vim.cmd,
    setreg = vim.fn.setreg,
    fn_input = vim.fn.input,
    map = _G.map,
  }

  vim.notify = function(message)
    table.insert(calls.notifications, tostring(message))
  end
  vim.ui.input = function(_, on_confirm)
    calls.prompts = calls.prompts + 1
    if on_confirm then
      on_confirm(nil)
    end
  end
  vim.cmd = function()
    calls.commands = calls.commands + 1
  end
  vim.fn.setreg = function()
    calls.clipboard = calls.clipboard + 1
  end
  vim.fn.input = function()
    calls.prompts = calls.prompts + 1
    return ""
  end
  _G.map = capture_map

  local ok, err = pcall(fn)

  vim.notify = real.notify
  vim.ui.input = real.input
  vim.cmd = real.cmd
  vim.fn.setreg = real.setreg
  vim.fn.input = real.fn_input
  _G.map = real.map

  return ok, err
end

set_scenario("repo_fails")

-- Loaded with the fakes in place: `lua/plugins/git.lua` binds them to its own
-- module-level locals, which the captured callbacks then close over, so the
-- fakes keep answering after `package.loaded` is put back.
local specs
with_fake_modules(function()
  specs = dofile(config_root .. "/lua/plugins/git.lua")
end)

local fugitive
for _, spec in ipairs(specs) do
  if spec[1] == "tpope/vim-fugitive" then
    fugitive = spec
  end
end
assert(fugitive, "no vim-fugitive spec in lua/plugins/git.lua")

local configured, config_error = with_fake_sinks(function()
  fugitive.config()
end)
assert(configured, "the fugitive spec's config raised: " .. tostring(config_error))

local gitsigns_spec
for _, spec in ipairs(specs) do
  if spec[1] == "lewis6991/gitsigns.nvim" then
    gitsigns_spec = spec
  end
end
assert(gitsigns_spec, "no gitsigns.nvim spec in lua/plugins/git.lua")

-- The fakes are back in place for this one, because the gitsigns spec requires
-- the plugin inside its `config` rather than at file scope.
with_fake_modules(function()
  local ok, err = with_fake_sinks(function()
    gitsigns_spec.config()
  end)
  assert(ok, "the gitsigns spec's config raised: " .. tostring(err))
end)

-- Keys whose callback reads a field off `github.account`, and keys that read one
-- off `github.repo`. The four `<C-g>r` keys read both, account first.
local ACCOUNT_KEYS = { "<C-g>rh", "<C-g>rH", "<C-g>rs", "<C-g>rS", "<C-g>lw", "<C-g>lW" }
local REPO_KEYS = {
  "<C-g>rh",
  "<C-g>rH",
  "<C-g>rs",
  "<C-g>rS",
  "<C-g>Cb",
  "<C-g>bc",
  "<C-g>cp",
  "<C-g>cA",
  "<C-g>dhw",
  "<C-g>dhm",
}

local SCENARIOS = {
  { scenario = "account_fails", keys = ACCOUNT_KEYS },
  { scenario = "repo_fails", keys = REPO_KEYS },
  { scenario = "repo_malformed", keys = REPO_KEYS },
}

local function drive(scenario, lhs)
  set_scenario(scenario)
  reset_calls()
  local spec = captured[lhs] or error("no keymap captured for " .. lhs)
  return with_fake_sinks(spec.rhs)
end

local function assert_guarded(scenario, lhs)
  local ok, err = drive(scenario, lhs)
  local where = ("%s under %s"):format(lhs, scenario)
  assert(ok, where .. " raised: " .. tostring(err))
  assert(
    #calls.notifications == 1,
    ("%s produced %d notifications, expected 1: %s"):format(
      where,
      #calls.notifications,
      table.concat(calls.notifications, " | ")
    )
  )
  assert(calls.prompts == 0, ("%s opened %d prompts"):format(where, calls.prompts))
  assert(calls.commands == 0, ("%s ran %d ex commands"):format(where, calls.commands))
  assert(calls.clipboard == 0, ("%s wrote the clipboard %d times"):format(where, calls.clipboard))
  assert(calls.overseer == 0, ("%s started %d Overseer runs"):format(where, calls.overseer))
end

local cases = {}

for _, row in ipairs(SCENARIOS) do
  for _, lhs in ipairs(row.keys) do
    cases[("%s notifies and stops under %s"):format(lhs, row.scenario)] = function()
      assert_guarded(row.scenario, lhs)
    end
  end
end

-- A key dropped from either list above would silently stop being covered, and a
-- key renamed in git.lua would stop being found. Both are pinned here.
cases["the two guarded key lists cover exactly the twelve affected keys"] = function()
  local union = {}
  for _, list in ipairs({ ACCOUNT_KEYS, REPO_KEYS }) do
    for _, lhs in ipairs(list) do
      union[lhs] = true
    end
  end

  local names = {}
  for lhs in pairs(union) do
    assert(captured[lhs], lhs .. " is not mapped by the fugitive spec any more")
    table.insert(names, lhs)
  end
  table.sort(names)
  assert(#names == 12, ("%d keys covered, expected 12: %s"):format(#names, table.concat(names, " ")))
end

-- Paths that exist nowhere: every name below is set on a scratch buffer, and
-- nothing here reads or writes the filesystem.
local ROOT = "/private/tmp/plugins-git-spec/repo"
local GITDIR = ROOT .. "/.git"
local BARE = "/private/tmp/plugins-git-spec/bare.git"

---One `<leader>gM` press against a window layout built for the case. Each name
---in `blames` becomes a window, the source buffer gets the last one, and the
---answer names the window left current and counts the blames opened.
---@param case table
---@return table
local function press_blame_walk(case)
  vim.cmd("tabnew")

  local buffers = {}
  local blame_windows = {}
  for index, name in ipairs(case.blames) do
    local buffer = vim.api.nvim_create_buf(false, true)
    vim.api.nvim_buf_set_name(buffer, name)
    table.insert(buffers, buffer)
    vim.cmd("split")
    vim.api.nvim_win_set_buf(0, buffer)
    blame_windows[vim.api.nvim_get_current_win()] = index
  end

  local source = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_name(source, case.source)
  table.insert(buffers, source)
  vim.cmd("split")
  local source_window = vim.api.nvim_get_current_win()
  vim.api.nvim_win_set_buf(source_window, source)
  vim.b[source].gitsigns_status_dict = case.status

  blame_calls = 0
  local ok, err = pcall(captured["<leader>gM"].rhs)
  local focused = vim.api.nvim_get_current_win()

  vim.cmd("tabclose")
  for _, buffer in ipairs(buffers) do
    pcall(vim.api.nvim_buf_delete, buffer, { force = true })
  end

  assert(ok, "<leader>gM raised: " .. tostring(err))
  return {
    focused = focused == source_window and "source" or blame_windows[focused] or "elsewhere",
    blames_opened = blame_calls,
  }
end

---@param case table
---@param expected table
local function assert_blame_walk(case, expected)
  local answer = press_blame_walk(case)
  assert(
    answer.focused == expected.focused,
    ("%s focused %s, expected %s"):format(case.what, tostring(answer.focused), tostring(expected.focused))
  )
  assert(
    answer.blames_opened == expected.blames_opened,
    ("%s opened %d blames, expected %d"):format(case.what, answer.blames_opened, expected.blames_opened)
  )
end

cases["`<leader>gM` focuses the blame window open on this file"] = function()
  assert_blame_walk({
    what = "the blame for this file",
    source = ROOT .. "/x.lua",
    status = { gitdir = GITDIR, root = ROOT },
    blames = { ("gitsigns-blame://%s//:0:x.lua"):format(GITDIR) },
  }, { focused = 1, blames_opened = 0 })
end

-- The blame a reader already walked with `R` sits at another revision and is
-- still the window to focus rather than to duplicate, so only the path is
-- compared. This case also drives the other revision shape: gitsigns names the
-- index `:0`, which carries a colon of its own, and a walked blame does not.
cases["`<leader>gM` focuses a blame already walked to another revision"] = function()
  assert_blame_walk({
    what = "the blame walked to a parent commit",
    source = ROOT .. "/x.lua",
    status = { gitdir = GITDIR, root = ROOT },
    blames = { ("gitsigns-blame://%s//abc1234^:x.lua"):format(GITDIR) },
  }, { focused = 1, blames_opened = 0 })
end

-- A colon is legal in a filename, so the blame name for `a:x.lua` ends with the
-- blame name for `x.lua`. Matching the tail focuses the wrong window and leaves
-- this file unblamed.
cases["`<leader>gM` does not focus a blame whose path merely ends with this one"] = function()
  assert_blame_walk({
    what = "the blame for a differently named file",
    source = ROOT .. "/x.lua",
    status = { gitdir = GITDIR, root = ROOT },
    blames = { ("gitsigns-blame://%s//:0:a:x.lua"):format(GITDIR) },
  }, { focused = "source", blames_opened = 1 })
end

-- A bare repository has no working tree, so a Fugitive revision buffer's
-- `fugitive://` name cannot start with one. Fugitive parses its own URL back
-- into `<rev>:<path>`, which is the only place that path is available; without
-- it the second press opens a second blame and fails naming its buffer.
cases["`<leader>gM` focuses the blame of a Fugitive buffer in a bare repository"] = function()
  local real_parse = vim.fn.FugitiveParse
  vim.fn.FugitiveParse = function()
    return { "abc1234:x.lua", BARE }
  end

  local ok, err = pcall(assert_blame_walk, {
    what = "the blame of a bare repository's revision buffer",
    source = ("fugitive://%s//abc1234/x.lua"):format(BARE),
    status = { gitdir = BARE, root = BARE },
    blames = { ("gitsigns-blame://%s//abc1234:x.lua"):format(BARE) },
  }, { focused = 1, blames_opened = 0 })

  vim.fn.FugitiveParse = real_parse
  assert(ok, err)
end

-- A repository rooted at `/` already ends in the separator, so appending one
-- gives `//` and no absolute path starts with that.
cases["`<leader>gM` focuses the blame in a repository rooted at the filesystem root"] = function()
  assert_blame_walk({
    what = "the blame in a repository rooted at `/`",
    source = "/x.lua",
    status = { gitdir = "/.git", root = "/" },
    blames = { "gitsigns-blame:///.git//:0:x.lua" },
  }, { focused = 1, blames_opened = 0 })
end

return cases
