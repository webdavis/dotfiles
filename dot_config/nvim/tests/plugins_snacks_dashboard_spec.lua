-- The jq programs the dashboard's GitHub panes hand `gh`.
--
-- Those panes are pty terminals `dashboard.width` minus the section's
-- `indent` columns wide, so a wider row wraps mid-word and a pane that prints
-- nothing at all renders as Neovim's `[Process exited 0]` line. Both are
-- properties of the jq programs, not of snacks, so they are pinned here: each
-- program is pulled out of the command the spec actually declares and run over
-- a fixture, once empty and once with rows far past what fits.
--
-- jq slices strings by codepoint while the pane counts display columns, which
-- is why every program strips non-ASCII first; the fixtures carry an emoji, a
-- tab and a newline to keep that strip honest.

local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

---@type number|nil
local dashboard_width

---The dashboard's terminal sections, keyed by title, with `Snacks` faked: the
---section generator asks it whether the cwd is a repository.
---@return table<string, table>
local function terminal_sections()
  local saved = _G.Snacks
  _G.Snacks = { git = {
    get_root = function()
      return config_root
    end,
  } }
  local ok, items = pcall(function()
    local spec = dofile(config_root .. "/lua/plugins/snacks.lua")
    for _, plugin in ipairs(spec) do
      if plugin[1] == "folke/snacks.nvim" then
        dashboard_width = plugin.opts.dashboard.width
        for _, section in ipairs(plugin.opts.dashboard.sections) do
          if type(section) == "function" then
            return section()
          end
        end
      end
    end
  end)
  _G.Snacks = saved
  assert(ok, items)

  local by_title = {}
  for _, item in ipairs(assert(items, "the dashboard declares no terminal section generator")) do
    by_title[item.title] = item
  end
  return by_title
end

local sections = terminal_sections()

-- No pane row may be wider: the dashboard's own width, less a pane's indent,
-- both read off the config rather than pinned here a second time.
local pane_width = assert(dashboard_width, "the dashboard declares no width")
  - assert(sections["Notifications"], "the dashboard declares no Notifications pane").indent

---The jq program the named pane hands `gh`.
---@param title string
---@return string
local function jq_program(title)
  local item = assert(sections[title], title .. " is not a dashboard section")
  return assert(item.cmd:match("%-%-jq '(.*)'"), title .. " hands gh no --jq program")
end

---Run `program` over `json`, and answer the lines it printed.
---@param program string
---@param json string
---@return string[]
local function jq_lines(program, json)
  local path = vim.fn.tempname()
  vim.fn.writefile({ json }, path)
  local lines = vim.fn.systemlist({ "jq", "-r", program, path })
  local code = vim.v.shell_error
  vim.fn.delete(path)
  assert(code == 0, ("jq exited %d: %s"):format(code, table.concat(lines, "\n")))
  return lines
end

---The widest line in the pane's display columns, with the SGR escapes the
---terminal consumes rather than prints removed.
---@param lines string[]
---@return number
local function widest(lines)
  local max = 0
  for _, line in ipairs(lines) do
    max = math.max(max, vim.fn.strdisplaywidth((line:gsub("\27%[[%d;]*m", ""))))
  end
  return max
end

-- Long enough that an unbounded row would run several times past the pane.
local long_title = ("supercalifragilistic "):rep(20)

local notifications_fixture = vim.json.encode({
  {
    unread = true,
    repository = {
      name = "a-repository-name-far-past-twenty-columns",
      full_name = "webdavis/a-repository-name-far-past-twenty-columns",
    },
    subject = { title = "🚀 " .. long_title .. "\tand\na wrapped tail" },
  },
  {
    unread = false,
    repository = { name = "short", full_name = "webdavis/short" },
    subject = { title = long_title },
  },
})

local number_title_fixture = vim.json.encode({
  { number = 12345, title = "🚀 " .. long_title },
  { number = 1, title = "short enough" },
})

return {
  ["notifications answer inbox zero rather than nothing"] = function()
    local lines = jq_lines(jq_program("Notifications"), "[]")
    assert(#lines == 1 and lines[1] == "inbox zero", vim.inspect(lines))
  end,

  ["notifications rows fit the pane"] = function()
    local lines = jq_lines(jq_program("Notifications"), notifications_fixture)
    assert(#lines == 2, ("one row per notification, got %d: %s"):format(#lines, vim.inspect(lines)))
    assert(widest(lines) <= pane_width, ("widest row is %d columns: %s"):format(widest(lines), vim.inspect(lines)))
    assert(
      not lines[1]:find("🚀", 1, true),
      "an emoji survived, so jq's codepoint slicing no longer matches the pane"
    )
    assert(lines[1]:sub(-2) == "..", "a cut title is not marked as cut: " .. lines[1])
  end,

  ["notifications name the repository without its owner"] = function()
    local lines = jq_lines(jq_program("Notifications"), notifications_fixture)
    -- The owner is half the width of a full_name and never varies, which is
    -- what made the old row too wide to read.
    assert(not lines[1]:find("webdavis/", 1, true), "the row still carries the owner: " .. lines[1])
    assert(lines[2]:find("short", 1, true), "the row lost the repository name: " .. lines[2])
  end,

  ["notifications mark the unread ones"] = function()
    local lines = jq_lines(jq_program("Notifications"), notifications_fixture)
    assert(lines[1]:sub(1, 1) == "*", "unread row is unmarked: " .. lines[1])
    assert(lines[2]:sub(1, 1) == " ", "read row is marked: " .. lines[2])
  end,

  ["issue and pull request rows answer none rather than nothing"] = function()
    local lines = jq_lines(jq_program("Open Issues"), "[]")
    assert(#lines == 1 and lines[1] == "none", vim.inspect(lines))
  end,

  ["issue and pull request rows fit the pane, numbered"] = function()
    local lines = jq_lines(jq_program("Open Issues"), number_title_fixture)
    assert(#lines == 2, ("one row per item, got %d: %s"):format(#lines, vim.inspect(lines)))
    assert(widest(lines) <= pane_width, ("widest row is %d columns: %s"):format(widest(lines), vim.inspect(lines)))
    assert(lines[1]:sub(1, 6) == "#12345", "the row lost its number: " .. lines[1])
    assert(lines[1]:sub(-2) == "..", "a cut title is not marked as cut: " .. lines[1])
    assert(lines[2] == "#1 short enough", "a row that fits was changed: " .. lines[2])
  end,

  ["both GitHub list panes run the same program"] = function()
    -- One program, so a fix to the row format reaches issues and pull requests
    -- together rather than drifting apart.
    assert(jq_program("Open Issues") == jq_program("Open PRs"), "the two list panes format their rows differently")
  end,
}
