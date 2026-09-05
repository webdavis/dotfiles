-- The `:ReviewLedger` awk program (spec 7.7).
--
-- There is no Lua parser to test: the command shells out to ONE awk program, so
-- these cases drive that same program rather than a copy of it. `config.keymaps`
-- exports it for exactly this reason; `map()` is stubbed because the file is a
-- side-effecting config file, not a module.
--
-- Five rows, not the three the format needs, so the banged case asserts a count
-- the default case cannot reach by accident, and so both closed dispositions
-- (`FIXED` and `FIXED-NOTEST`) are represented.

local rows = {
  "| id | step | severity | summary | disposition | evidence |",
  "|----|------|----------|---------|-------------|----------|",
  "| F1 | 6v | HIGH | the guard at `lua/config/keymaps.lua:42` never runs | ACCEPTED | rationale |",
  "| F2 | 4b | MEDIUM | no path token anywhere in this summary | ACCEPTED | rationale |",
  "| F3 | 6v | LOW | closed, and it carries `lua/config/options.lua:7` | FIXED | abc1234 |",
  "| F4 | 6v | LOW | closed with no path token | FIXED | def5678 |",
  "| F5 | 6v | LOW | closed, and untestable | FIXED-NOTEST | 9abcdef, no testable surface |",
}

-- A real file, not stdin: the fallback location comes from awk's own `FILENAME`
-- now, so the expectations stay path-shaped and the cases feed the program the
-- way the command feeds it.
local fixture_path

local function write_register(lines)
  local dir = vim.fn.tempname()
  vim.fn.mkdir(dir, "p")
  local path = dir .. "/findings-fixture.md"
  vim.fn.writefile(lines, path)
  return path
end

local function fixture()
  fixture_path = fixture_path or write_register(rows)
  return fixture_path
end

local function awk_program()
  _G.map = _G.map or function() end
  local program = require("config.keymaps").ledger_awk
  assert(type(program) == "string", "config.keymaps exports no ledger_awk")
  return program
end

local function awk_over(register, all)
  local result = vim.system({ "awk", "-v", "all=" .. all, awk_program(), register }):wait()
  assert(result.code == 0, "awk exited " .. result.code .. ": " .. tostring(result.stderr))
  return vim.split(vim.trim(result.stdout), "\n")
end

local function run(all)
  return awk_over(fixture(), all)
end

-- A markdown table cell may hold a pipe two ways: escaped, and inside a code
-- span. Splitting on the raw character shifts every column after it.
local location_rows = {
  "| F1 | 6v | LOW | a version bump to v1.2.3:45 landed | ACCEPTED | rationale |",
  "| F2 | 6v | LOW | see https://x.y/z:80 for the report | ACCEPTED | rationale |",
  "| F3 | 6v | LOW | the guard (lua/a.lua:4) never runs | ACCEPTED | rationale |",
  "| F4 | 6v | LOW | both src/first.lua:12,src/second.lua:34 moved | ACCEPTED | rationale |",
  "| F5 | 6v | LOW | hooks.rs:2757's cost was the stub's sleep | ACCEPTED | rationale |",
}

local piped_rows = {
  "| F1 | 6v | HIGH | a code span holding `left|FIXED` inside it | ACCEPTED | rationale |",
  "| F2 | 6v | LOW | an escaped pipe \\| sitting in prose | ACCEPTED | rationale |",
}

return {
  ["skips the closed rows and keeps the open ones in order"] = function()
    local lines = run(0)
    assert(#lines == 2, "got " .. #lines .. " lines: " .. table.concat(lines, " / "))
  end,

  ["takes the location from a path:line token in the summary"] = function()
    local lines = run(0)
    local expected = "lua/config/keymaps.lua:42: F1 HIGH ACCEPTED: "
      .. "the guard at `lua/config/keymaps.lua:42` never runs"
    assert(lines[1] == expected, "got " .. tostring(lines[1]))
  end,

  ["falls back to the ledger file and the row's own line number"] = function()
    local lines = run(0)
    local expected = fixture() .. ":4: F2 MEDIUM ACCEPTED: no path token anywhere in this summary"
    assert(lines[2] == expected, "got " .. tostring(lines[2]))
  end,

  ["lists the closed rows too when banged"] = function()
    local lines = run(1)
    assert(#lines == 5, "got " .. #lines .. " lines: " .. table.concat(lines, " / "))
    assert(lines[3]:find("^lua/config/options%.lua:7: F3 LOW FIXED: "), "got " .. tostring(lines[3]))
    assert(lines[4] == fixture() .. ":6: F4 LOW FIXED: closed with no path token", "got " .. tostring(lines[4]))
  end,

  -- `complete = "file"` means Neovim expands the argument BEFORE the callback
  -- sees it, exactly as `:edit` does. Expanding it a second time unescapes what
  -- that first pass already unescaped, so a register whose name carries a
  -- backslash is looked for under the wrong name and never read.
  -- A location is a path-shaped token, not any word carrying a colon and digits.
  ["reads a version number as prose, not as a location"] = function()
    local register = write_register(location_rows)
    local lines = awk_over(register, 0)
    assert(lines[1] == register .. ":1: F1 LOW ACCEPTED: a version bump to v1.2.3:45 landed", lines[1])
  end,

  ["reads a URL with a port as prose, not as a location"] = function()
    local register = write_register(location_rows)
    local lines = awk_over(register, 0)
    assert(lines[2] == register .. ":2: F2 LOW ACCEPTED: see https://x.y/z:80 for the report", lines[2])
  end,

  ["strips the punctuation enclosing a location"] = function()
    local lines = awk_over(write_register(location_rows), 0)
    assert(lines[3]:find("^lua/a%.lua:4: F3 LOW ACCEPTED: "), lines[3])
  end,

  ["takes the first of two locations joined by a comma"] = function()
    local lines = awk_over(write_register(location_rows), 0)
    assert(lines[4]:find("^src/first%.lua:12: F4 LOW ACCEPTED: "), lines[4])
  end,

  -- A real register carries this shape: the possessive follows the location.
  ["strips a possessive that trails a location"] = function()
    local lines = awk_over(write_register(location_rows), 0)
    assert(lines[5]:find("^hooks%.rs:2757: F5 LOW ACCEPTED: "), lines[5])
  end,

  ["keeps a row whose code span holds a pipe and the word FIXED"] = function()
    local lines = awk_over(write_register(piped_rows), 0)
    assert(#lines == 2, "got " .. #lines .. " lines: " .. table.concat(lines, " / "))
    assert(lines[1]:find("F1 HIGH ACCEPTED: a code span holding `left|FIXED` inside it", 1, true), lines[1])
  end,

  ["reads the disposition past an escaped pipe in the summary"] = function()
    local lines = awk_over(write_register(piped_rows), 0)
    assert(lines[2]:find("F2 LOW ACCEPTED: an escaped pipe \\| sitting in prose", 1, true), lines[2])
  end,

  ["passes the command argument through without a second expansion"] = function()
    local dir = vim.fn.tempname()
    vim.fn.mkdir(dir, "p")
    local register = dir .. "/back\\slash.md"
    vim.fn.writefile({ "| F1 | 6v | HIGH | a plain summary | ACCEPTED | rationale |" }, register)
    awk_program()
    vim.cmd("ReviewLedger " .. vim.fn.fnameescape(register))
    local info = vim.fn.getqflist({ title = 1, items = 1 })
    vim.cmd("cclose")
    assert(info.title == "ReviewLedger " .. register, "title: " .. info.title)
    assert(#info.items == 1, "entries: " .. #info.items)
  end,

  -- awk's `-v` runs escape processing over the value, so a register path holding
  -- a literal backslash-n arrived at the program as a real newline and split the
  -- record it was supposed to prefix.
  ["keeps a backslash in the register path out of awk's escape processing"] = function()
    local dir = vim.fn.tempname()
    vim.fn.mkdir(dir, "p")
    local register = dir .. "/led\\ngm.md"
    vim.fn.writefile({ "| F1 | 6v | HIGH | a summary with no path token | ACCEPTED | rationale |" }, register)
    awk_program()
    vim.cmd("ReviewLedger " .. vim.fn.fnameescape(register))
    local items = vim.fn.getqflist({ items = 1 }).items
    vim.cmd("cclose")
    assert(#items == 1, "entries: " .. #items)
    -- macOS resolves the temp root through a symlink, so compare resolved paths.
    local listed = vim.fn.bufname(items[1].bufnr)
    assert(listed == vim.fn.resolve(register), "filename: " .. listed)
  end,

  ["reads FIXED-NOTEST as closed as well, so the skip is a prefix match"] = function()
    local open = table.concat(run(0), "\n")
    assert(not open:find("F5", 1, true), "F5 listed unbanged: " .. open)
    local all = run(1)
    assert(all[5] == fixture() .. ":7: F5 LOW FIXED-NOTEST: closed, and untestable", "got " .. tostring(all[5]))
  end,
}
