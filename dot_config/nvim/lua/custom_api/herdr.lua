-- Sends selections to this workspace's Claude agent, and launches one when none runs.
-- Require herdr-nvim inside each function, so the test runner can load this module without it.

local M = {}

-- `working` warns and sends, because the agents queue input sent mid-turn.
-- `blocked` refuses, because typed text would answer the pending approval.
local VERDICTS = {
  idle = "send",
  done = "send",
  working = "warn",
  blocked = "refuse",
}

-- Any other state, `unknown` included, warns and sends.
function M.may_send(status)
  return VERDICTS[status] or "warn"
end

local function herdr_cli(argv)
  return require("herdr-nvim.exec").default_exec(argv)
end

-- The pane this editor is in now. The HERDR_* variables keep the launch-time pane after a move.
-- Returns nil when herdr cannot say.
local function live_pane()
  local result = herdr_cli({ "herdr", "pane", "current", "--current" })
  if result.code ~= 0 then
    return nil
  end
  local ok, decoded = pcall(vim.json.decode, result.stdout)
  if not ok or type(decoded) ~= "table" then
    return nil
  end
  local pane = type(decoded.result) == "table" and decoded.result.pane
  if type(pane) ~= "table" or type(pane.tab_id) ~= "string" or pane.tab_id == "" then
    return nil
  end
  return pane
end

-- `agents.list()` spans every workspace when HERDR_WORKSPACE_ID is unset, so refuse first.
-- Returns the reason, or nil.
function M.workspace_refusal(herdr_env, workspace_id)
  if herdr_env ~= "1" then
    return "not running inside herdr, so an agent lookup would cross every workspace"
  end
  if not workspace_id or workspace_id == "" then
    return "herdr set no HERDR_WORKSPACE_ID for this pane, so a lookup cannot be scoped"
  end
  return nil
end

-- `agents.list` scopes by the launch-time workspace, so after a move the editor must restart.
function M.stale_workspace_refusal(env_workspace_id, live_workspace_id)
  if not live_workspace_id then
    return "herdr could not say which pane this editor is in, so a lookup cannot be scoped"
  end
  if live_workspace_id ~= env_workspace_id then
    return ("this editor pane has moved to %s since it started in %s, so its herdr environment is stale; restart it in its pane"):format(
      live_workspace_id,
      env_workspace_id
    )
  end
  return nil
end

-- Filter by workspace here too, because scoping the send is this module's promise.
function M.claude_agents_here(agents, workspace_id)
  return vim.tbl_filter(function(agent)
    return agent.kind == "claude" and agent.workspace_id == workspace_id
  end, agents)
end

-- The pane id tells apart two agents whose display text matches.
function M.picker_label(display, pane_id)
  return display .. " [" .. pane_id .. "]"
end

-- Takes a callback, because the picker is asynchronous.
-- The callback gets the pane id, or nil when no Claude agent runs in this workspace.
-- A refusal or a cancelled picker never calls it.
function M.agent_pane(on_pane)
  local refusal = M.workspace_refusal(vim.env.HERDR_ENV, vim.env.HERDR_WORKSPACE_ID)
  if refusal then
    vim.notify("herdr: " .. refusal, vim.log.levels.WARN)
    return
  end

  -- Check the live pane before listing, so the listing never reads a workspace the editor has left.
  local pane = live_pane()
  refusal = M.stale_workspace_refusal(vim.env.HERDR_WORKSPACE_ID, pane and pane.workspace_id)
  if refusal then
    vim.notify("herdr: " .. refusal, vim.log.levels.WARN)
    return
  end

  local agents = require("herdr-nvim.agents")
  local all, err = agents.list()
  if not all then
    vim.notify("herdr: " .. err, vim.log.levels.ERROR)
    return
  end

  local claude = M.claude_agents_here(all, pane.workspace_id)

  if #claude == 0 then
    return on_pane(nil)
  end

  -- Match the live tab, because the plugin's `agents.resolve` reads the launch-time HERDR_TAB_ID.
  -- Fall back to the lone agent in the workspace, never to UI focus.
  local in_tab = vim.tbl_filter(function(agent)
    return agent.tab_id == pane.tab_id
  end, claude)
  local one = #in_tab == 1 and in_tab[1] or (#claude == 1 and claude[1])
  if one then
    return on_pane(one.pane_id)
  end

  -- Uses its own picker, because `ui.pick_agent` cannot tell apart two rows with the same text.
  vim.ui.select(claude, {
    prompt = "Send to Claude agent",
    format_item = function(agent)
      return M.picker_label(agents.display(agent), agent.pane_id)
    end,
  }, function(agent)
    if agent then
      on_pane(agent.pane_id)
    end
  end)
end

-- Read the status fresh, because a picker can stay open long enough for the listing to go stale.
local function agent_status(pane_id)
  local result = herdr_cli({ "herdr", "agent", "get", pane_id })
  if result.code ~= 0 then
    return "unknown"
  end
  local ok, decoded = pcall(vim.json.decode, result.stdout)
  if not ok or type(decoded) ~= "table" then
    return "unknown"
  end
  local agent = (decoded.result or {}).agent or {}
  return agent.agent_status or "unknown"
end

-- `submit` is required, because `herdr agent prompt` refuses a blocked agent
-- and a send that does not submit could type into an approval dialog.
function M.send(text, opts)
  if not (opts and opts.submit) then
    error("custom_api.herdr.send: submit = true is required; herdr has no guarded non-submitting send")
  end

  M.agent_pane(function(pane_id)
    if not pane_id then
      vim.notify("herdr: no claude agent in this workspace", vim.log.levels.WARN)
      return
    end

    local status = agent_status(pane_id)
    local verdict = M.may_send(status)
    if verdict == "refuse" then
      vim.notify(("herdr: agent is %s, not sending"):format(status), vim.log.levels.WARN)
      return
    end
    if verdict == "warn" then
      vim.notify(("herdr: agent is %s, sending anyway"):format(status), vim.log.levels.WARN)
    end

    local sent, err = require("herdr-nvim.dispatch").send(pane_id, text, opts)
    if not sent then
      vim.notify("herdr: " .. err, vim.log.levels.ERROR)
    end
  end)
end

-- The run of non-blank lines around the cursor, or nil on a blank line.
local function paragraph_range()
  local buffer = vim.api.nvim_get_current_buf()
  local cursor = vim.api.nvim_win_get_cursor(0)[1]
  local last = vim.api.nvim_buf_line_count(buffer)

  local function blank(line)
    return vim.fn.getline(line):match("^%s*$") ~= nil
  end

  if blank(cursor) then
    return nil
  end

  local first, final = cursor, cursor
  while first > 1 and not blank(first - 1) do
    first = first - 1
  end
  while final < last and not blank(final + 1) do
    final = final + 1
  end
  return first, final
end

-- Call while still in Visual mode, because `mode()` and `curswant` describe only the live selection.
-- `curswant` at `v:maxcol` marks a blockwise selection taken to each line end.
function M.visual_lines()
  local visual = vim.fn.mode()
  local to_end_of_line = vim.fn.getcurpos()[5] == vim.v.maxcol
  vim.cmd([[execute "normal! \<esc>"]])
  local kind = (visual == "\22" and to_end_of_line) and ("\22" .. vim.v.maxcol) or visual
  return vim.fn.getregion(vim.fn.getpos("'<"), vim.fn.getpos("'>"), { type = kind })
end

function M.send_selection_or_paragraph()
  local lines

  if vim.fn.mode():match("^[vV\22]") then
    lines = M.visual_lines()
  else
    local first, final = paragraph_range()
    if first then
      lines = vim.api.nvim_buf_get_lines(0, first - 1, final, false)
    end
  end

  local text = table.concat(lines or {}, "\n")
  if vim.trim(text) == "" then
    vim.notify("herdr: nothing under the cursor to send", vim.log.levels.WARN)
    return
  end

  M.send(text, { submit = true })
end

-- herdr names must match `[a-z][a-z0-9_-]{0,31}` and be unique among live agents,
-- so the name comes from the pane id.
function M.agent_name(pane_id)
  return "claude-" .. pane_id:lower():gsub(":", "-")
end

-- The decision half of `launch_or_attach`, pure so tests need no running herdr.
function M.plan_launch(pane_id, cwd)
  if pane_id then
    return { "prompt", pane_id }
  end
  return { "split", cwd }
end

-- herdr reports a failure on stdout or stderr depending on the verb, so read both.
local function failure(result)
  if result.code == 0 then
    return nil
  end
  local said = vim.trim(result.stderr) ~= "" and result.stderr or result.stdout
  return vim.trim(said) ~= "" and vim.trim(said) or ("exit " .. result.code)
end

local function split_pane(cwd)
  local result = herdr_cli({
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
  local result = herdr_cli({
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

-- Prompt `/ide` at this workspace's Claude agent, or split a pane and start one with `--ide`.
function M.launch_or_attach()
  M.agent_pane(function(pane_id)
    local plan = M.plan_launch(pane_id, vim.fn.getcwd())

    if plan[1] == "prompt" then
      -- `agent prompt` refuses a blocked session, so `/ide` never answers an approval.
      local err = failure(herdr_cli({ "herdr", "agent", "prompt", plan[2], "/ide" }))
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
    -- The name can clash only with a live agent whose name differs just in letter case,
    -- so retry once with a suffix.
    if err and err:find("agent_name_taken", 1, true) then
      err = start_agent(name .. "-2", new_pane)
    end
    if err then
      vim.notify("herdr: herdr agent start failed: " .. err, vim.log.levels.ERROR)
    end
  end)
end

return M
