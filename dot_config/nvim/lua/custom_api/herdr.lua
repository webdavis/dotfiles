-- The shared herdr seam (spec 7.4). One module answers "which pane" and "how to
-- send", and every caller in this config goes through it. It is a thin wrapper
-- over the installed `herdr-nvim` plugin, never a second implementation of what
-- that plugin already does: `herdr-nvim` owns the workspace-scoped agent lookup
-- (`agents.list`, `agents.resolve`, `ui.pick_agent`) and the two dispatch verbs
-- (`dispatch.send`), and this module adds only the Claude filter, the interrupt
-- policy and the launch plan.
--
-- Every `require("herdr-nvim.…")` sits INSIDE the function that needs it. The
-- headless spec runner starts `--clean` with only this config's `lua/` on
-- `package.path` (tests/run.lua), so a top-level require would make the whole
-- module unloadable there and take the pure functions down with it.

local M = {}

-- ╭────────────────────╮
-- │ The interrupt policy │
-- ╰────────────────────╯

-- Spec 7.4's table, verbatim. `working` warns and sends because both harnesses
-- queue input that arrives mid-turn; `blocked` refuses because text typed into a
-- pane waiting on an approval ANSWERS that approval. There is no queue, no
-- waiter and no recheck: a refusal is final and the operator presses the key
-- again once the approval is answered.
local VERDICTS = {
  idle = "send",
  done = "send",
  working = "warn",
  blocked = "refuse",
}

-- "send", "warn" (warn and send) or "refuse". `unknown` takes the `working` row,
-- and so does any state herdr grows later: an unlisted state must not become a
-- silent refusal, and must not become a silent send either.
function M.may_send(status)
  return VERDICTS[status] or "warn"
end

-- ╭─────────────╮
-- │ Which pane  │
-- ╰─────────────╯

-- `on_pane` rather than a return value: `ui.pick_agent` goes through
-- `vim.ui.select`, which snacks.nvim replaces with an asynchronous picker, so
-- the ambiguous case cannot answer in the caller's stack frame. It is called
-- with the pane id, or with nil when this workspace runs no Claude agent; a
-- cancelled picker calls it not at all.
function M.agent_pane(on_pane)
  local agents = require("herdr-nvim.agents")
  local all, err = agents.list()
  if not all then
    vim.notify("herdr: " .. err, vim.log.levels.ERROR)
    return
  end

  local claude = vim.tbl_filter(function(a)
    return a.kind == "claude"
  end, all)

  if #claude == 0 then
    return on_pane(nil)
  end

  -- `agents.resolve` narrows to the agent sharing HERDR_TAB_ID, else a lone
  -- agent in the workspace, and answers nil when that is genuinely ambiguous.
  -- Focus is never consulted (spec 7.2): herdr focus is UI-wide, so every agent
  -- of a background workspace reports `focused = false`.
  local one = agents.resolve(claude)
  if one then
    return on_pane(one.pane_id)
  end

  require("herdr-nvim.ui").pick_agent(claude, function(agent)
    on_pane(agent.pane_id)
  end)
end

-- ╭─────────────╮
-- │ How to send │
-- ╰─────────────╯

local function herdr_cli(argv)
  return require("herdr-nvim.exec").default_exec(argv)
end

-- The agent's state at send time, read fresh rather than taken from the listing
-- `agent_pane` resolved on: a picker the operator sat in front of for a while is
-- exactly the case where the cached status has gone stale. Anything unreadable
-- is `unknown`, which the 7.4 table already has a row for.
local function agent_status(pane_id)
  local result = herdr_cli({ "herdr", "agent", "get", pane_id })
  if result.code ~= 0 then
    return "unknown"
  end
  local ok, decoded = pcall(vim.json.decode, result.stdout)
  if not ok or type(decoded) ~= "table" then
    return "unknown"
  end
  local agent = ((decoded.result or {}).agent) or {}
  return agent.agent_status or "unknown"
end

-- `submit` runs `herdr agent prompt`, which presses Enter itself; without it the
-- text goes through `herdr pane send-text` and waits at the prompt. No key name
-- is ever sent either way, so nothing is split on newlines.
function M.send(text, opts)
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

return M
