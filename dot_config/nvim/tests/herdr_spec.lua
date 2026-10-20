-- custom_api.herdr (spec 7.2): the launch plan and the launch-or-attach flow.

local herdr = require("custom_api.herdr")

-- `launch_or_attach` is glue whose ORDER of CLI calls is the behavior under
-- test. `herdr-nvim-2` is replaced through `package.loaded` with a
-- `pick_agent` that answers `agent` at once, and `herdr.cli` with a recorder
-- that answers from `replies` by command and errors on any command it does not
-- know, so a call that asked for something else cannot pass quietly. Both are
-- restored after the case so no other spec in this process sees them.
local function launch(agent, replies)
  local calls = { commands = {} }
  local saved = { plugin = package.loaded["herdr-nvim-2"], cli = herdr.cli, notify = vim.notify }

  package.loaded["herdr-nvim-2"] = {
    pick_agent = function(on_agent, opts)
      calls.pick_opts = opts
      on_agent(agent)
    end,
  }
  herdr.cli = function(argv)
    local command = table.concat(argv, " ")
    table.insert(calls.commands, command)
    for prefix, reply in pairs(replies) do
      if command:sub(1, #prefix) == prefix then
        return vim.tbl_extend(
          "keep",
          type(reply) == "function" and reply(command) or reply,
          { stdout = "", stderr = "" }
        )
      end
    end
    error("unexpected command: " .. command)
  end
  -- A failure notifies, and under `--clean` that lands on the runner's own
  -- stdout and merges into the next case's report line.
  vim.notify = function(message)
    calls.notified = message
  end

  local ok, err = pcall(herdr.launch_or_attach)

  package.loaded["herdr-nvim-2"] = saved.plugin
  herdr.cli = saved.cli
  vim.notify = saved.notify
  assert(ok, err)
  return calls
end

local SPLIT_REPLY = { code = 0, stdout = vim.json.encode({ result = { pane = { pane_id = "wW:p9K" } } }) }

return {
  -- Spec 7.2: the derived name has to match `[a-z][a-z0-9_-]{0,31}`, and herdr's
  -- pane ids are mixed case with a colon in them, so both transforms are load
  -- bearing. Pane ids are never reused, so the name is free unless a
  -- case-folded twin is live.
  ["agent_name lowercases the pane id and replaces its colon"] = function()
    assert(herdr.agent_name("wW:p3K") == "claude-ww-p3k", herdr.agent_name("wW:p3K"))
  end,

  ["plan_launch prompts the pane that was found"] = function()
    local plan = herdr.plan_launch("wW:p3K", "/somewhere")
    assert(plan[1] == "prompt", plan[1])
    assert(plan[2] == "wW:p3K", tostring(plan[2]))
    assert(plan[3] == nil, tostring(plan[3]))
  end,

  ["plan_launch splits when no pane was found"] = function()
    local plan = herdr.plan_launch(nil, "/somewhere")
    assert(plan[1] == "split", plan[1])
    assert(plan[2] == "/somewhere", tostring(plan[2]))
    assert(plan[3] == nil, tostring(plan[3]))
  end,

  ["launch_or_attach asks pick_agent for a claude agent"] = function()
    local calls = launch({ pane_id = "wW:p3K", kind = "claude" }, { ["herdr agent prompt"] = { code = 0 } })
    assert(vim.deep_equal(calls.pick_opts, { kind = "claude" }), vim.inspect(calls.pick_opts))
  end,

  ["launch_or_attach prompts /ide at the agent pick_agent found"] = function()
    local calls = launch({ pane_id = "wW:p3K", kind = "claude" }, { ["herdr agent prompt"] = { code = 0 } })
    assert(vim.deep_equal(calls.commands, { "herdr agent prompt wW:p3K /ide" }), vim.inspect(calls.commands))
    assert(calls.notified == nil, tostring(calls.notified))
  end,

  ["launch_or_attach reports a refused prompt"] = function()
    local calls = launch({ pane_id = "wW:p3K" }, { ["herdr agent prompt"] = { code = 1, stderr = "agent is blocked" } })
    assert(calls.notified == "herdr: herdr agent prompt failed: agent is blocked", tostring(calls.notified))
  end,

  ["launch_or_attach splits a pane and starts claude when there is no agent"] = function()
    local calls = launch(nil, { ["herdr pane split"] = SPLIT_REPLY, ["herdr agent start"] = { code = 0 } })
    assert(#calls.commands == 2, vim.inspect(calls.commands))
    assert(calls.commands[1]:find("^herdr pane split %-%-current %-%-direction right %-%-cwd "), calls.commands[1])
    assert(
      calls.commands[2] == "herdr agent start claude-ww-p9k --kind claude --pane wW:p9K -- --ide",
      calls.commands[2]
    )
    assert(calls.notified == nil, tostring(calls.notified))
  end,

  ["launch_or_attach retries once under a suffixed name when the name is taken"] = function()
    local calls = launch(nil, {
      ["herdr pane split"] = SPLIT_REPLY,
      ["herdr agent start"] = function(command)
        if command:find("claude-ww-p9k-2", 1, true) then
          return { code = 0 }
        end
        return { code = 1, stderr = '{"error":{"code":"agent_name_taken"}}' }
      end,
    })
    assert(#calls.commands == 3, vim.inspect(calls.commands))
    assert(
      calls.commands[3] == "herdr agent start claude-ww-p9k-2 --kind claude --pane wW:p9K -- --ide",
      calls.commands[3]
    )
    assert(calls.notified == nil, tostring(calls.notified))
  end,

  ["launch_or_attach does not retry a start that failed for another reason"] = function()
    local calls = launch(nil, {
      ["herdr pane split"] = SPLIT_REPLY,
      ["herdr agent start"] = { code = 1, stderr = "readiness timeout" },
    })
    assert(#calls.commands == 2, vim.inspect(calls.commands))
    assert(calls.notified == "herdr: herdr agent start failed: readiness timeout", tostring(calls.notified))
  end,

  ["launch_or_attach starts nothing when the split fails"] = function()
    local calls = launch(nil, { ["herdr pane split"] = { code = 1, stderr = "no pane" } })
    assert(#calls.commands == 1, vim.inspect(calls.commands))
    assert(calls.notified == "herdr: herdr pane split failed: no pane", tostring(calls.notified))
  end,
}
