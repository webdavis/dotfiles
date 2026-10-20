-- Launch-or-attach for `<leader>Cc` (spec 7.2). `herdr-nvim-2` owns agent
-- resolution (`pick_agent`: the workspace gate, this tab's agent, the lone
-- workspace agent, else a picker). This module adds the launch plan and the
-- three herdr CLI calls that carry it out.
--
-- The `require("herdr-nvim-2")` sits INSIDE `launch_or_attach`. The headless
-- spec runner starts `--clean` with only this config's `lua/` on
-- `package.path` (tests/run.lua), so a top-level require would make the whole
-- module unloadable there and take the pure functions down with it.

local M = {}

-- The one seam to the herdr CLI, a field so the spec can replace it.
function M.cli(argv)
  return vim.system(argv, { text = true }):wait()
end

-- Spec 7.2. herdr agent names must match `[a-z][a-z0-9_-]{0,31}` and be unique
-- among LIVE agents, so both transforms are load bearing and a fixed name would
-- fail in the second workspace that launches. Pane ids are never reused and a
-- name is cleared when its agent exits, so the derived name is free unless a
-- case-folded twin is live.
function M.agent_name(pane_id)
  return "claude-" .. pane_id:lower():gsub(":", "-")
end

-- The decision half of `launch_or_attach`, kept apart from the three CLI calls
-- that carry it out so it is answerable without a running herdr.
function M.plan_launch(pane_id, cwd)
  if pane_id then
    return { "prompt", pane_id }
  end
  return { "split", cwd }
end

-- herdr reports a failure as a JSON envelope with an `error` object, on stdout
-- for some verbs and on stderr for others, alongside a non-zero exit (measured
-- on herdr 0.8.2: `agent get` on an unknown pane answers on stdout, `agent
-- start` under a taken name on stderr). Both streams are searched rather than
-- guessed at.
local function failure(result)
  if result.code == 0 then
    return nil
  end
  local said = vim.trim(result.stderr) ~= "" and result.stderr or result.stdout
  return vim.trim(said) ~= "" and vim.trim(said) or ("exit " .. result.code)
end

-- The new pane's id is `.result.pane.pane_id`.
local function split_pane(cwd)
  local result = M.cli({
    "herdr",
    "pane",
    "split",
    "--current",
    "--direction",
    "right",
    "--cwd",
    cwd,
    "--focus",
  })
  local err = failure(result)
  if err then
    return nil, "herdr pane split failed: " .. err
  end

  local ok, decoded = pcall(vim.json.decode, result.stdout)
  if not ok or type(decoded) ~= "table" then
    return nil, "herdr pane split: unparseable JSON"
  end
  local pane_id = ((decoded.result or {}).pane or {}).pane_id
  if not pane_id then
    return nil, "herdr pane split: the reply carried no .result.pane.pane_id"
  end
  return pane_id
end

local function start_agent(name, pane_id)
  local result = M.cli({
    "herdr",
    "agent",
    "start",
    name,
    "--kind",
    "claude",
    "--pane",
    pane_id,
    "--",
    "--ide",
  })
  return failure(result)
end

-- `<leader>Cc`: prompt `/ide` at the Claude agent `pick_agent` resolves, or
-- split a pane beside the editor and start one there with `--ide`.
function M.launch_or_attach()
  require("herdr-nvim-2").pick_agent(function(agent)
    local plan = M.plan_launch(agent and agent.pane_id, vim.fn.getcwd())

    if plan[1] == "prompt" then
      -- `agent prompt` refuses while that session is blocked on an approval,
      -- which is the right behavior: typing `/ide` into a blocked prompt with
      -- `pane run` would ANSWER the approval instead.
      local err = failure(M.cli({ "herdr", "agent", "prompt", plan[2], "/ide" }))
      if err then
        vim.notify("herdr: herdr agent prompt failed: " .. err, vim.log.levels.ERROR)
      end
      return
    end

    local new_pane, split_err = split_pane(plan[2])
    if not new_pane then
      vim.notify("herdr: " .. split_err, vim.log.levels.ERROR)
      return
    end

    local name = M.agent_name(new_pane)
    local err = start_agent(name, new_pane)
    -- The one way the derived name is taken is a live case-folded twin, and
    -- herdr says so by name (`agent_name_taken`, measured), so one retry under a
    -- suffixed name is the whole recovery. Any other failure is reported rather
    -- than retried, which would only double a 30-second readiness timeout.
    if err and err:find("agent_name_taken", 1, true) then
      err = start_agent(name .. "-2", new_pane)
    end
    if err then
      vim.notify("herdr: herdr agent start failed: " .. err, vim.log.levels.ERROR)
    end
  end, { kind = "claude" })
end

return M
