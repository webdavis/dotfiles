-- The shared herdr seam (spec 7.4). One module answers "which pane" and "how to
-- send", and every caller in this config goes through it. It is a thin wrapper
-- over the installed `herdr-nvim` plugin, never a second implementation of what
-- that plugin already does: `herdr-nvim` owns the agent lookup (`agents.list`,
-- `agents.resolve`, `agents.display`) and the dispatch verb (`dispatch.send`),
-- and this module adds only the workspace gate, the Claude filter, the picker
-- row, the interrupt policy and the launch plan.
--
-- Every `require("herdr-nvim.…")` sits INSIDE the function that needs it. The
-- headless spec runner starts `--clean` with only this config's `lua/` on
-- `package.path` (tests/run.lua), so a top-level require would make the whole
-- module unloadable there and take the pure functions down with it.

local M = {}

-- ╭──────────────────────╮
-- │ The interrupt policy │
-- ╰──────────────────────╯

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

-- ╭───────────────────╮
-- │ Talking to herdr  │
-- ╰───────────────────╯

local function herdr_cli(argv)
  return require("herdr-nvim.exec").default_exec(argv)
end

-- Where this editor's pane IS, as opposed to where it was started. herdr stamps
-- HERDR_WORKSPACE_ID, HERDR_TAB_ID and HERDR_PANE_ID into the environment when
-- the TERMINAL launches and never updates them, but a cross-workspace pane move
-- keeps that terminal and changes all three. Measured on herdr 0.8.2: a pane
-- moved from w18:p2 to w19:p2 kept terminal term_65ab65225b56329 while its shell
-- still reported HERDR_WORKSPACE_ID=w18.
--
-- `pane current --current` is the one place this seam asks that question, and it
-- is the right one to ask here: the command answers the CALLER's pane, which is
-- a trap for anything wanting the FOCUSED pane and is exactly what is wanted for
-- "which pane am I". It resolves by terminal rather than by the environment, so
-- it answered w19:p2 for the moved pane above. Returns nil when it cannot say,
-- and the caller refuses on nil rather than guessing.
local function live_pane()
  local result = herdr_cli({ "herdr", "pane", "current", "--current" })
  if result.code ~= 0 then
    return nil
  end
  local ok, decoded = pcall(vim.json.decode, result.stdout)
  if not ok or type(decoded) ~= "table" then
    return nil
  end
  return (decoded.result or {}).pane
end

-- ╭────────────╮
-- │ Which pane │
-- ╰────────────╯

-- Why the editor must prove it is inside herdr before anything is looked up:
-- `herdr agent list` is machine-wide and `agents.list()` narrows it to
-- HERDR_WORKSPACE_ID only when that variable is SET. Started outside a herdr
-- pane it therefore answers with every workspace's agents (measured: `wW` and
-- `wX` both came back), and a lone Claude anywhere on the machine would be
-- resolved silently and sent this buffer. Returns the reason to refuse, or nil.
function M.workspace_refusal(herdr_env, workspace_id)
  if herdr_env ~= "1" then
    return "not running inside herdr, so an agent lookup would cross every workspace"
  end
  if not workspace_id or workspace_id == "" then
    return "herdr set no HERDR_WORKSPACE_ID for this pane, so a lookup cannot be scoped"
  end
  return nil
end

-- The second half of the gate, once the live pane is known. Refusing rather than
-- adapting to the live workspace is deliberate: `agents.list` and
-- `agents.resolve` inside `herdr-nvim` read the same stale environment, so an
-- editor that has been moved cannot be served correctly by any amount of work on
-- this side. The operator restarts it in its new pane, which costs one command
-- and cannot deliver a buffer to the workspace they walked away from.
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

-- The Claude agents of ONE workspace. The workspace test is repeated here
-- rather than left to `agents.list()`, whose own filter is a third-party
-- plugin's implementation detail: scoping the send is this seam's promise, so
-- this seam is where it is kept and where it is pinned.
function M.claude_agents_here(agents, workspace_id)
  return vim.tbl_filter(function(agent)
    return agent.kind == "claude" and agent.workspace_id == workspace_id
  end, agents)
end

-- The picker row. `agents.display` renders kind, status and cwd basename, which
-- two Claude agents in two tabs of one repository share exactly; the pane id is
-- the field that tells them apart before the text goes anywhere.
function M.picker_label(display, pane_id)
  return display .. " [" .. pane_id .. "]"
end

-- `on_pane` rather than a return value: the picker goes through
-- `vim.ui.select`, which snacks.nvim replaces with an asynchronous picker, so
-- the ambiguous case cannot answer in the caller's stack frame. It is called
-- with the pane id, or with nil when this workspace runs no Claude agent. A
-- refusal and a cancelled picker call it NOT AT ALL, which is what keeps
-- `launch_or_attach` from splitting a pane on the strength of a lookup that
-- never happened.
function M.agent_pane(on_pane)
  local refusal = M.workspace_refusal(vim.env.HERDR_ENV, vim.env.HERDR_WORKSPACE_ID)
  if refusal then
    vim.notify("herdr: " .. refusal, vim.log.levels.WARN)
    return
  end

  -- Before the listing, not after: a listing that ran would already have read
  -- the agents of the workspace this editor merely used to be in.
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

  -- The live workspace, not the environment's copy of it: the two are equal by
  -- the gate above, and reading the live one keeps that true if the gate moves.
  local claude = M.claude_agents_here(all, pane.workspace_id)

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

  -- Not `ui.pick_agent`: its rows are `agents.display` alone, which collides.
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

-- ╭─────────────╮
-- │ How to send │
-- ╰─────────────╯

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
  local agent = (decoded.result or {}).agent or {}
  return agent.agent_status or "unknown"
end

-- Submitting is the only mode this seam offers, and it is required rather than
-- defaulted so a caller cannot ask for the unsafe one by omission. `herdr agent
-- prompt` presses Enter itself and REFUSES an agent that is already blocked;
-- `herdr pane send-text` does neither, so a non-submitting send can lose the
-- race between the status read below and the dispatch and type into an approval
-- dialog. A second status check does not close that race, only a guarded
-- non-submitting verb would, and herdr 0.8.2 has none. No key name is ever sent,
-- so nothing is split on newlines.
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

-- ╭──────────────╮
-- │ What to send │
-- ╰──────────────╯

-- The paragraph under the cursor: the run of non-blank lines around it. A blank
-- line is its own paragraph, which sends nothing and is caught by the caller.
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

-- The visual selection, or the paragraph under the cursor in normal mode, sent
-- with `submit = true`: the run-this-there behavior vim-slime was carried for.
-- There is no target-picking keymap and no `<leader>CP` (spec 7.4):
-- `agent_pane` resolves the target and prompts only when it is genuinely
-- ambiguous, so a stored target would be a second answer to a settled question.
-- The lines of the CURRENT Visual selection. Called while the mode is still
-- visual, because both of the things it has to read are gone once it is not:
-- `mode()` answers "v", "V" or CTRL-V, the exact spelling `getregion` wants for
-- its `type` (`visualmode()` would answer for the PREVIOUS selection), and
-- `curswant` at `v:maxcol` is the only record that a blockwise selection was
-- taken to the END of each line. Without that second read, `<C-v>j$` from `b`
-- over `abc123456` / `xyz` returned `bc1` / `yz` where Neovim yanks `bc123456` /
-- `yz`. Leaving Visual mode is what materializes `'<` and `'>`, so it happens
-- after both reads and before the marks are used.
function M.visual_lines()
  local visual = vim.fn.mode()
  local to_end_of_line = vim.fn.getcurpos()[5] == vim.v.maxcol
  vim.cmd([[execute "normal! \<esc>"]])
  local kind = (visual == "\22" and to_end_of_line) and ("\22" .. vim.v.maxcol) or visual
  -- Not the line range: `getregion` reads charwise, linewise and blockwise
  -- exactly, so half a line selected sends half a line.
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

-- ╭──────────────────╮
-- │ Launch or attach │
-- ╰──────────────────╯

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
function M.plan_launch(pane_id, cwd, servername)
  if pane_id then
    return { "prompt", pane_id }
  end
  return { "split", cwd, servername }
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

-- The new pane's id is `.result.pane.pane_id`. `--env` pins the socket: the MCP
-- server the CLI starts in that pane inherits it and the resolver connects with
-- no discovery.
local function split_pane(cwd, servername)
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
    "--env",
    "NVIM_MCP_SOCKET=" .. servername,
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

-- `<leader>Cc`: prompt `/ide` at the Claude agent already in this workspace, or
-- split a pane beside the editor and start one there with `--ide`.
function M.launch_or_attach()
  M.agent_pane(function(pane_id)
    local plan = M.plan_launch(pane_id, vim.fn.getcwd(), vim.v.servername)

    if plan[1] == "prompt" then
      -- `agent prompt` refuses while that session is blocked on an approval,
      -- which is the right behavior: typing `/ide` into a blocked prompt with
      -- `pane run` would ANSWER the approval instead.
      local err = failure(herdr_cli({ "herdr", "agent", "prompt", plan[2], "/ide" }))
      if err then
        vim.notify("herdr: herdr agent prompt failed: " .. err, vim.log.levels.ERROR)
      end
      return
    end

    local new_pane, split_err = split_pane(plan[2], plan[3])
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
  end)
end

return M
