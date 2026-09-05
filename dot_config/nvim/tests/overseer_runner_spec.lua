-- `custom_api.overseer.overseer_runner`: what it hands to overseer.
--
-- The pure part is the decision between the two task shapes. One command has to
-- stay a shell string, because every one-command caller in `plugins/git.lua`
-- passes git format specifiers in single quotes that only a shell reads. Two or
-- more have to become an orchestrator, because `;` runs the next command whether
-- or not the last one worked.

local runner = require("custom_api.overseer")

---Run `overseer_runner` with `overseer` faked, and return the task definition it
---asked for. Nothing is started; the fake's `start` only records that it was.
---@param opts table
---@return table definition
---@return boolean started
local function definition_for(opts)
  local captured, started
  local fake = {
    new_task = function(defn)
      captured = defn
      return {
        start = function()
          started = true
        end,
      }
    end,
  }
  local saved = package.loaded["overseer"]
  package.loaded["overseer"] = fake
  local ok, err = pcall(runner.overseer_runner, opts)
  package.loaded["overseer"] = saved
  assert(ok, err)
  return assert(captured, "overseer.new_task was never called"), started == true
end

---@param opts table
---@return string message
local function refusal_for(opts)
  local saved = package.loaded["overseer"]
  package.loaded["overseer"] = {
    new_task = function()
      error("new_task should not be reached")
    end,
  }
  local ok, err = pcall(runner.overseer_runner, opts)
  package.loaded["overseer"] = saved
  assert(not ok, "expected a refusal, got a task")
  return tostring(err)
end

return {
  ["one command runs as a shell string, not argv"] = function()
    local defn, started = definition_for({ cmds = "git diff --color-words" })
    assert(type(defn.cmd) == "string", "cmd was a " .. type(defn.cmd) .. ", so the shell would not read it")
    assert(defn.cmd == "git diff --color-words", "cmd was " .. tostring(defn.cmd))
    assert(defn.strategy == nil, "a single command should not need a strategy")
    assert(started, "the task was never started")
  end,

  ["a lone command in a table is still one shell string"] = function()
    local defn = definition_for({ cmds = { "git status" } })
    assert(defn.cmd == "git status", "cmd was " .. tostring(defn.cmd))
    assert(defn.strategy == nil, "a single command should not need a strategy")
  end,

  ["single quotes in a git format specifier survive untouched"] = function()
    local pretty = "git log --pretty=format:'%<(7)%C(yellow)%h%C(reset)'"
    local defn = definition_for({ cmds = pretty })
    assert(defn.cmd == pretty, "the format specifier was altered: " .. tostring(defn.cmd))
  end,

  ["two commands become an orchestrator, never a joined string"] = function()
    local defn, started = definition_for({ cmds = { "git init", "gh repo create --public x" } })
    assert(defn.cmd == nil, "a joined cmd was set alongside the strategy: " .. tostring(defn.cmd))
    assert(type(defn.strategy) == "table", "strategy was " .. type(defn.strategy))
    assert(defn.strategy[1] == "orchestrator", "strategy was " .. tostring(defn.strategy[1]))
    assert(started, "the task was never started")
  end,

  ["each command becomes its own orchestrator step, in order"] = function()
    local defn = definition_for({ cmds = { "one", "two", "three" } })
    local steps = defn.strategy.tasks
    assert(#steps == 3, "got " .. #steps .. " steps")
    for index, want in ipairs({ "one", "two", "three" }) do
      assert(steps[index].cmd == want, ("step %d was %s"):format(index, tostring(steps[index].cmd)))
    end
  end,

  ["no command is ever joined with a semicolon"] = function()
    -- The defect this replaced: `;` runs the next command whether or not the
    -- last one worked, so a failed `git init` still created the repository.
    local defn = definition_for({ cmds = { "git init", "gh repo create" } })
    local encoded = vim.inspect(defn)
    assert(not encoded:match("git init%s*;"), "a semicolon-joined command survived: " .. encoded)
  end,

  ["the notify component is asked for before the default alias"] = function()
    -- Components are a no-op if already present, so the customised
    -- on_complete_notify has to come before `default`, which also adds one.
    local defn = definition_for({ cmds = "git status" })
    assert(type(defn.components[1]) == "table", "the first component is not the customised one")
    assert(defn.components[1][1] == "on_complete_notify", "first component was " .. vim.inspect(defn.components[1]))
    assert(defn.components[2] == "default", "second component was " .. tostring(defn.components[2]))
  end,

  ["a non-string, non-table cmds is refused"] = function()
    assert(refusal_for({ cmds = 7 }):match("must be a string or a table"), "wrong message")
  end,

  ["a non-string command inside the list is refused by index"] = function()
    assert(refusal_for({ cmds = { "git status", 7 } }):match("index 2"), "wrong message")
  end,

  ["an empty command list is refused rather than run"] = function()
    assert(refusal_for({ cmds = {} }):match("must not be empty"), "wrong message")
  end,
}
