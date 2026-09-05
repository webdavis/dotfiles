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

local rows = table.concat({
  "| id | step | severity | summary | disposition | evidence |",
  "|----|------|----------|---------|-------------|----------|",
  "| F1 | 6v | HIGH | the guard at `lua/config/keymaps.lua:42` never runs | ACCEPTED | rationale |",
  "| F2 | 4b | MEDIUM | no path token anywhere in this summary | ACCEPTED | rationale |",
  "| F3 | 6v | LOW | closed, and it carries `lua/config/options.lua:7` | FIXED | abc1234 |",
  "| F4 | 6v | LOW | closed with no path token | FIXED | def5678 |",
  "| F5 | 6v | LOW | closed, and untestable | FIXED-NOTEST | 9abcdef, no testable surface |",
  "",
}, "\n")

local function awk_program()
  _G.map = _G.map or function() end
  local program = require("config.keymaps").ledger_awk
  assert(type(program) == "string", "config.keymaps exports no ledger_awk")
  return program
end

local function run(all)
  local result = vim
    .system({
      "awk",
      "-v",
      "f=ledger.md",
      "-v",
      "all=" .. all,
      awk_program(),
    }, { stdin = rows })
    :wait()
  assert(result.code == 0, "awk exited " .. result.code .. ": " .. tostring(result.stderr))
  return vim.split(vim.trim(result.stdout), "\n")
end

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
    local expected = "ledger.md:4: F2 MEDIUM ACCEPTED: no path token anywhere in this summary"
    assert(lines[2] == expected, "got " .. tostring(lines[2]))
  end,

  ["lists the closed rows too when banged"] = function()
    local lines = run(1)
    assert(#lines == 5, "got " .. #lines .. " lines: " .. table.concat(lines, " / "))
    assert(lines[3]:find("^lua/config/options%.lua:7: F3 LOW FIXED: "), "got " .. tostring(lines[3]))
    assert(lines[4]:find("^ledger%.md:6: F4 LOW FIXED: "), "got " .. tostring(lines[4]))
  end,

  ["reads FIXED-NOTEST as closed as well, so the skip is a prefix match"] = function()
    local open = table.concat(run(0), "\n")
    assert(not open:find("F5", 1, true), "F5 listed unbanged: " .. open)
    local all = run(1)
    assert(all[5] == "ledger.md:7: F5 LOW FIXED-NOTEST: closed, and untestable", "got " .. tostring(all[5]))
  end,
}
