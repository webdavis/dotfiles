-- Covers the `<C-g>` guards, the gitsigns mappings and `<leader>gM` in `lua/plugins/git.lua`.
-- The fakes mirror the real argument contracts, so a dropped guard fails a case.

-- From package.path, because `dofile` passes no arguments and a vararg would ignore `--config`.
local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

-- What each fake was asked to do. A guarded key leaves every counter at zero.
local calls = {}

local function reset_calls()
  calls.notifications = {}
  calls.prompts = 0
  calls.commands = 0
  calls.clipboard = 0
  calls.overseer = 0
end

reset_calls()

-- The scenario the fake `github` answers. Callbacks read it at call time, so one load covers every case.
local github_fake = {}
local git_fake = {}
local util_fake = {}
local overseer_fake = {}

-- gitsigns faked as one recorder per function, so a wrong target or argument fails a case.
local gitsigns_calls = {}

local function reset_gitsigns_calls()
  for index = #gitsigns_calls, 1, -1 do
    gitsigns_calls[index] = nil
  end
end

local function count_gitsigns_calls(name)
  local count = 0
  for _, call in ipairs(gitsigns_calls) do
    if call.name == name then
      count = count + 1
    end
  end
  return count
end

-- `setup` keeps its options, because the buffer-local mappings live in `on_attach`.
local setup_opts
local gitsigns_fake = {
  setup = function(cfg)
    setup_opts = cfg
  end,
}

for _, name in ipairs({
  "blame",
  "blame_line",
  "diffthis",
  "nav_hunk",
  "preview_hunk",
  "preview_hunk_inline",
  "reset_buffer",
  "reset_buffer_index",
  "reset_hunk",
  "select_hunk",
  "setqflist",
  "show",
  "show_commit",
  "stage_buffer",
  "stage_hunk",
  "toggle_current_line_blame",
  "toggle_linehl",
  "toggle_word_diff",
}) do
  gitsigns_fake[name] = function(...)
    table.insert(gitsigns_calls, { name = name, args = { ... } })
  end
end

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
    -- `gh` answered with no slash, so `repo` returns a nil `name`.
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

-- Keyed by mode and lhs, because two gitsigns keys are declared in both normal and visual mode.
local captured_by_mode = {}

local function capture_map(spec)
  local lhs_list = type(spec.lhs) == "table" and spec.lhs or { spec.lhs }
  local mode_list = type(spec.mode) == "table" and spec.mode or { spec.mode }
  for _, lhs in ipairs(lhs_list) do
    captured[lhs] = spec
    for _, mode in ipairs(mode_list) do
      captured_by_mode[mode .. " " .. lhs] = spec
    end
  end
end

-- Every spec runs in one process, so all globals are saved and restored.
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

-- Loaded with the fakes in place, so the callbacks close over them.
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

-- The fugitive mappings are the spec `keys` rows, built by calling `keys` as lazy.nvim does.
local built, keys_error = with_fake_sinks(function()
  for _, row in ipairs(fugitive.keys()) do
    captured[row[1]] = { lhs = row[1], rhs = row[2], desc = row.desc }
  end
end)
assert(built, "the fugitive spec's keys raised: " .. tostring(keys_error))

local gitsigns_spec
for _, spec in ipairs(specs) do
  if spec[1] == "lewis6991/gitsigns.nvim" then
    gitsigns_spec = spec
  end
end
assert(gitsigns_spec, "no gitsigns.nvim spec in lua/plugins/git.lua")

-- The gitsigns spec requires the plugin inside `config`, so the fakes go back in place.
with_fake_modules(function()
  local ok, err = with_fake_sinks(function()
    gitsigns_spec.config()
  end)
  assert(ok, "the gitsigns spec's config raised: " .. tostring(err))
end)

assert(setup_opts, "the gitsigns spec's config did not call setup")
assert(type(setup_opts.on_attach) == "function", "the gitsigns setup options carry no on_attach")

-- The buffer-local mappings, declared once against a scratch buffer the callbacks close over.
local attach_buffer = vim.api.nvim_create_buf(false, true)
with_fake_modules(function()
  local ok, err = with_fake_sinks(function()
    setup_opts.on_attach(attach_buffer)
  end)
  assert(ok, "the gitsigns spec's on_attach raised: " .. tostring(err))
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

-- Pins both lists against the keys in git.lua.
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

-- `<C-g>!` runs through `FugitiveExecute`, so fake it, run `vim.schedule` callbacks at once, and record `vim.cmd`.
local function press_amend_and_push(exit_status, stderr)
  local real_cmd = vim.cmd
  local real_notify = vim.notify
  local real_schedule = vim.schedule
  local real_execute = vim.fn.FugitiveExecute
  local real_did_change = vim.fn.FugitiveDidChange

  local cmds = {}
  local notifications = {}
  local execute_args

  vim.cmd = function(command)
    table.insert(cmds, command)
  end
  vim.notify = function(message, level)
    table.insert(notifications, { message = message, level = level })
  end
  vim.schedule = function(fn)
    fn()
  end
  local did_change_calls = 0
  vim.fn.FugitiveDidChange = function()
    did_change_calls = did_change_calls + 1
  end
  vim.fn.FugitiveExecute = function(args, callback)
    execute_args = args
    callback({ exit_status = exit_status, stderr = stderr })
  end

  local ok, err = pcall(captured["<C-g>!"].rhs)

  vim.cmd = real_cmd
  vim.notify = real_notify
  vim.schedule = real_schedule
  vim.fn.FugitiveExecute = real_execute
  vim.fn.FugitiveDidChange = real_did_change

  assert(ok, "<C-g>! raised: " .. tostring(err))
  return {
    cmds = cmds,
    notifications = notifications,
    execute_args = execute_args,
    did_change_calls = did_change_calls,
  }
end

cases["<C-g>! stages, amends, and force-pushes in order on success"] = function()
  local answer = press_amend_and_push(0, {})
  assert(
    #answer.cmds == 2 and answer.cmds[1] == "Gwrite" and answer.cmds[2] == "Git! push --force",
    "expected Gwrite then Git! push --force, got: " .. table.concat(answer.cmds, " | ")
  )
  assert(
    answer.execute_args[1] == "commit" and answer.execute_args[2] == "--amend" and answer.execute_args[3] == "--no-edit",
    "FugitiveExecute did not receive the amend args"
  )
  assert(answer.did_change_calls == 1, "expected one FugitiveDidChange call")
  assert(#answer.notifications == 0, "expected no notifications on success")
end

-- The amend hook can run for seconds, so the push targets the buffer current at the keypress.
cases["<C-g>! pushes against the buffer captured before the amend, not one switched to while it ran"] = function()
  local original_buf = vim.api.nvim_get_current_buf()
  local other_buf = vim.api.nvim_create_buf(false, true)

  local real_cmd = vim.cmd
  local real_schedule = vim.schedule
  local real_execute = vim.fn.FugitiveExecute
  local real_did_change = vim.fn.FugitiveDidChange

  local push_buf
  vim.cmd = function(command)
    if command == "Git! push --force" then
      push_buf = vim.api.nvim_get_current_buf()
    end
  end
  vim.schedule = function(fn)
    fn()
  end
  vim.fn.FugitiveDidChange = function() end
  vim.fn.FugitiveExecute = function(_, callback)
    vim.api.nvim_set_current_buf(other_buf)
    callback({ exit_status = 0, stderr = {} })
  end

  local ok, err = pcall(captured["<C-g>!"].rhs)

  vim.cmd = real_cmd
  vim.schedule = real_schedule
  vim.fn.FugitiveExecute = real_execute
  vim.fn.FugitiveDidChange = real_did_change
  vim.api.nvim_set_current_buf(original_buf)
  vim.api.nvim_buf_delete(other_buf, { force = true })

  assert(ok, "<C-g>! raised: " .. tostring(err))
  assert(
    push_buf == original_buf,
    ("push ran against buffer %s, expected the buffer active before the amend (%s)"):format(
      tostring(push_buf),
      tostring(original_buf)
    )
  )
end

cases["<C-g>! stages then warns and stops when the amend fails"] = function()
  local answer = press_amend_and_push(1, { "error: nothing to commit" })
  assert(
    #answer.cmds == 1 and answer.cmds[1] == "Gwrite",
    "expected only Gwrite to run, got: " .. table.concat(answer.cmds, " | ")
  )
  assert(#answer.notifications == 1, ("expected one warning, got %d"):format(#answer.notifications))
  assert(
    answer.notifications[1].message:find("error: nothing to commit", 1, true),
    "warning did not carry the amend's stderr"
  )
  assert(answer.notifications[1].level == vim.log.levels.WARN, "expected a WARN-level notification")
end

-- Paths that exist nowhere, set only as buffer names.
local ROOT = "/private/tmp/plugins-git-spec/repo"
local GITDIR = ROOT .. "/.git"
local BARE = "/private/tmp/plugins-git-spec/bare.git"

---One `<leader>gM` press against a layout with one window per name in `blames`.
---Returns the window left current and the count of blames opened.
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

  reset_gitsigns_calls()
  local ok, err = pcall(captured["<leader>gM"].rhs)
  local focused = vim.api.nvim_get_current_win()

  vim.cmd("tabclose")
  for _, buffer in ipairs(buffers) do
    pcall(vim.api.nvim_buf_delete, buffer, { force = true })
  end

  assert(ok, "<leader>gM raised: " .. tostring(err))
  return {
    focused = focused == source_window and "source" or blame_windows[focused] or "elsewhere",
    blames_opened = count_gitsigns_calls("blame"),
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

-- A walked blame sits at another revision and still gets focus, so compare only the path.
cases["`<leader>gM` focuses a blame already walked to another revision"] = function()
  assert_blame_walk({
    what = "the blame walked to a parent commit",
    source = ROOT .. "/x.lua",
    status = { gitdir = GITDIR, root = ROOT },
    blames = { ("gitsigns-blame://%s//abc1234^:x.lua"):format(GITDIR) },
  }, { focused = 1, blames_opened = 0 })
end

-- A colon is legal in a filename, so match the whole blame name, not its tail.
cases["`<leader>gM` does not focus a blame whose path merely ends with this one"] = function()
  assert_blame_walk({
    what = "the blame for a differently named file",
    source = ROOT .. "/x.lua",
    status = { gitdir = GITDIR, root = ROOT },
    blames = { ("gitsigns-blame://%s//:0:a:x.lua"):format(GITDIR) },
  }, { focused = "source", blames_opened = 1 })
end

-- A bare repository has no working tree, so read the path from the Fugitive URL parse.
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

-- A root of `/` already ends in the separator.
cases["`<leader>gM` focuses the blame in a repository rooted at the filesystem root"] = function()
  assert_blame_walk({
    what = "the blame in a repository rooted at `/`",
    source = "/x.lua",
    status = { gitdir = "/.git", root = "/" },
    blames = { "gitsigns-blame:///.git//:0:x.lua" },
  }, { focused = 1, blames_opened = 0 })
end

-- Every gitsigns mapping with the function it must reach. `args` pins constant arguments, `range` marks the two visual-mode mappings.
local GITSIGNS_KEYS = {
  { lhs = "]g", mode = "n", fn = "nav_hunk", args = { "next", { target = "all" } } },
  { lhs = "[g", mode = "n", fn = "nav_hunk", args = { "prev", { target = "all" } } },
  { lhs = "<leader>ga", mode = "n", fn = "stage_hunk", args = {} },
  { lhs = "<leader>ga", mode = "v", fn = "stage_hunk", range = true },
  { lhs = "<leader>gA", mode = "n", fn = "stage_buffer", args = {} },
  { lhs = "<leader>gr", mode = "n", fn = "reset_hunk", args = {} },
  { lhs = "<leader>gr", mode = "v", fn = "reset_hunk", range = true },
  { lhs = "<leader>gR", mode = "n", fn = "reset_buffer", args = {} },
  { lhs = "<leader>gU", mode = "n", fn = "reset_buffer_index", args = {} },
  { lhs = "<leader>gu", mode = "n", fn = "stage_hunk", args = {} },
  { lhs = "<leader>gp", mode = "n", fn = "preview_hunk", args = {} },
  { lhs = "<leader>gi", mode = "n", fn = "preview_hunk_inline", args = {} },
  { lhs = "<leader>gB", mode = "n", fn = "blame_line", args = { { full = true } } },
  { lhs = "<leader>gc", mode = "n", fn = "show_commit", args = {} },
  { lhs = "<leader>gq", mode = "n", fn = "setqflist", args = {} },
  { lhs = "<leader>gQ", mode = "n", fn = "setqflist", args = { "all" } },
  { lhs = "<leader>gm", mode = "n", fn = "blame_line", args = { { full = true } } },
  { lhs = "<leader>gM", mode = "n", fn = "blame", args = {} },
  { lhs = "<C-g>dhd", mode = "n", fn = "diffthis", args = { "~1", { vertical = true } } },
  { lhs = "<C-g>did", mode = "n", fn = "diffthis", args = { nil, { vertical = true } } },
  { lhs = "<C-g>dis", mode = "n", fn = "show", args = {} },
  { lhs = "<C-g>dw", mode = "n", fn = "toggle_word_diff", args = {} },
  { lhs = "<C-g>dl", mode = "n", fn = "toggle_linehl", args = {} },
  { lhs = "<C-g>Bt", mode = "n", fn = "toggle_current_line_blame", args = {} },
  { lhs = "ih", mode = "o", fn = "select_hunk", args = {} },
  { lhs = "ih", mode = "x", fn = "select_hunk", args = {} },
}

---One press of a gitsigns mapping in its own scratch buffer, returning the calls it made.
---@param row table
---@return table
local function press_gitsigns(row)
  local key = row.mode .. " " .. row.lhs
  local spec = captured_by_mode[key] or error("no keymap captured for " .. key)
  assert(type(spec.rhs) == "function", key .. " has no callable right hand side")

  reset_gitsigns_calls()
  local ok, err = pcall(spec.rhs)
  assert(ok, key .. " raised: " .. tostring(err))

  return { calls = gitsigns_calls, desc = spec.desc }
end

for _, row in ipairs(GITSIGNS_KEYS) do
  cases[("`%s` in %s mode calls gitsigns.%s"):format(row.lhs, row.mode, row.fn)] = function()
    local answer = press_gitsigns(row)
    local where = row.mode .. " " .. row.lhs

    assert(#answer.calls == 1, ("%s made %d gitsigns calls, expected 1"):format(where, #answer.calls))
    assert(
      answer.calls[1].name == row.fn,
      ("%s called gitsigns.%s, expected gitsigns.%s"):format(where, answer.calls[1].name, row.fn)
    )

    local args = answer.calls[1].args
    if row.range then
      assert(#args == 1 and type(args[1]) == "table" and #args[1] == 2, where .. " did not pass a two-line range")
    else
      assert(
        vim.deep_equal(args, row.args),
        ("%s passed %s, expected %s"):format(where, vim.inspect(args), vim.inspect(row.args))
      )
    end

    assert(
      type(answer.desc) == "string" and answer.desc:match("^Gitsigns: "),
      where .. " carries no `Gitsigns: ` description for which-key"
    )
  end
end

-- Finds mappings by their `Gitsigns: ` description, so a new one without a row fails.
cases["the gitsigns table covers every `Gitsigns: ` mapping the spec declares"] = function()
  local covered = {}
  for _, row in ipairs(GITSIGNS_KEYS) do
    local key = row.mode .. " " .. row.lhs
    assert(not covered[key], key .. " appears twice in the gitsigns table")
    covered[key] = true
  end

  local uncovered = {}
  for key, spec in pairs(captured_by_mode) do
    if type(spec.desc) == "string" and spec.desc:match("^Gitsigns: ") and not covered[key] then
      table.insert(uncovered, key)
    end
  end
  table.sort(uncovered)

  assert(
    #uncovered == 0,
    ("%d gitsigns mappings are not in the table: %s"):format(#uncovered, table.concat(uncovered, ", "))
  )
end

return cases
