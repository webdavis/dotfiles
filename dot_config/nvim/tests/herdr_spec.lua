-- custom_api.herdr (spec 7.4): the interrupt policy is the one piece of this
-- seam that is neither a wrapper over `herdr-nvim` nor a wrapper over the herdr
-- CLI, so it is the one piece with a unit test. `agent_pane`, `send` and
-- `send_selection_or_paragraph` are glue over a third-party API, over the CLI
-- and over `getpos`; they are checked live in the pull request body instead.

local herdr = require("custom_api.herdr")

-- Every state herdr's own `agent_status` field can hold (`herdr agent prompt
-- --until`, measured on herdr 0.8.2), so a case cannot pass by testing a state
-- that does not exist.
local STATES = { "idle", "working", "blocked", "done", "unknown" }

-- `agent_pane` is the one function here that is not pure and not glue: the ORDER
-- of its gate, its listing and its continuation is the behavior under test. The
-- two herdr-nvim modules it reaches for are replaced through `package.loaded`,
-- which is the same seam idea as `github.runner`, and both are restored after
-- the case so no other spec in this process sees them. `exec` answers only
-- `herdr pane current --current`; any other command is an error rather than a
-- fallthrough, so a call that asked for something else cannot pass quietly.
local function with_stubbed_herdr(scenario, on_pane)
  local calls = { list = 0, continuation = 0 }
  local saved = {
    agents = package.loaded["herdr-nvim.agents"],
    exec = package.loaded["herdr-nvim.exec"],
    herdr_env = vim.env.HERDR_ENV,
    workspace_id = vim.env.HERDR_WORKSPACE_ID,
    -- A refusal notifies, and under `--clean` that lands on the runner's own
    -- stdout and merges into the next case's report line.
    notify = vim.notify,
  }

  package.loaded["herdr-nvim.agents"] = {
    list = function()
      calls.list = calls.list + 1
      return { { kind = "claude", workspace_id = scenario.workspace_id, pane_id = "wW:p3K" } }
    end,
    resolve = function(list)
      return list[1]
    end,
    display = function(agent)
      return agent.kind
    end,
  }
  package.loaded["herdr-nvim.exec"] = {
    default_exec = function(argv)
      local command = table.concat(argv, " ")
      assert(command == "herdr pane current --current", "unexpected command: " .. command)
      if not scenario.live_workspace_id then
        return { code = 1, stdout = "", stderr = "no pane" }
      end
      return {
        code = 0,
        stderr = "",
        stdout = vim.json.encode({
          result = {
            pane = { pane_id = scenario.live_workspace_id .. ":p9", workspace_id = scenario.live_workspace_id },
          },
        }),
      }
    end,
  }
  vim.env.HERDR_ENV = "1"
  vim.env.HERDR_WORKSPACE_ID = scenario.workspace_id
  vim.notify = function() end

  local ok, err = pcall(herdr.agent_pane, function(pane_id)
    calls.continuation = calls.continuation + 1
    if on_pane then
      on_pane(pane_id)
    end
  end)

  package.loaded["herdr-nvim.agents"] = saved.agents
  package.loaded["herdr-nvim.exec"] = saved.exec
  vim.env.HERDR_ENV = saved.herdr_env
  vim.env.HERDR_WORKSPACE_ID = saved.workspace_id
  vim.notify = saved.notify
  assert(ok, err)
  return calls
end

return {
  ["may_send sends on idle"] = function()
    assert(herdr.may_send("idle") == "send", herdr.may_send("idle"))
  end,

  ["may_send sends on done"] = function()
    assert(herdr.may_send("done") == "send", herdr.may_send("done"))
  end,

  ["may_send warns and sends on working"] = function()
    assert(herdr.may_send("working") == "warn", herdr.may_send("working"))
  end,

  ["may_send warns rather than refusing on unknown"] = function()
    assert(herdr.may_send("unknown") == "warn", herdr.may_send("unknown"))
  end,

  ["may_send refuses on blocked"] = function()
    assert(herdr.may_send("blocked") == "refuse", herdr.may_send("blocked"))
  end,

  ["blocked is the only refusal"] = function()
    for _, state in ipairs(STATES) do
      local verdict = herdr.may_send(state)
      if state == "blocked" then
        assert(verdict == "refuse", state .. " -> " .. verdict)
      else
        assert(verdict ~= "refuse", state .. " -> " .. verdict)
      end
    end
  end,

  -- A state herdr grows later must not turn into a silent refusal, and must not
  -- turn into a silent send either: it takes the `unknown` row of the 7.4 table.
  ["a state the table does not list is treated as unknown"] = function()
    assert(herdr.may_send("compacting") == "warn", herdr.may_send("compacting"))
    assert(herdr.may_send(nil) == "warn", tostring(herdr.may_send(nil)))
  end,

  -- Spec 7.2: the derived name has to match `[a-z][a-z0-9_-]{0,31}`, and herdr's
  -- pane ids are mixed case with a colon in them, so both transforms are load
  -- bearing. Pane ids are never reused, so the name is free unless a
  -- case-folded twin is live.
  ["agent_name lowercases the pane id and replaces its colon"] = function()
    assert(herdr.agent_name("wW:p3K") == "claude-ww-p3k", herdr.agent_name("wW:p3K"))
  end,

  ["plan_launch prompts the pane that was found"] = function()
    local plan = herdr.plan_launch("wW:p3K", "/somewhere", "/tmp/nvim.sock")
    assert(plan[1] == "prompt", plan[1])
    assert(plan[2] == "wW:p3K", tostring(plan[2]))
    assert(plan[3] == nil, tostring(plan[3]))
  end,

  ["plan_launch splits when no pane was found"] = function()
    local plan = herdr.plan_launch(nil, "/somewhere", "/tmp/nvim.sock")
    assert(plan[1] == "split", plan[1])
    assert(plan[2] == "/somewhere", tostring(plan[2]))
    assert(plan[3] == "/tmp/nvim.sock", tostring(plan[3]))
  end,

  -- Finding 1. `agents.list()` is workspace scoped only when HERDR_WORKSPACE_ID
  -- is set: with the variable unset it answers with every workspace's agents
  -- (measured, `wW` and `wX` both came back), so a lone Claude anywhere on the
  -- machine would be resolved silently and sent the buffer.
  ["the lookup is refused when herdr did not start this editor"] = function()
    assert(herdr.workspace_refusal(nil, "wW"), "no herdr env at all was allowed")
    assert(herdr.workspace_refusal("0", "wW"), "HERDR_ENV=0 was allowed")
  end,

  ["the lookup is refused without a workspace to scope it to"] = function()
    assert(herdr.workspace_refusal("1", nil), "an unset HERDR_WORKSPACE_ID was allowed")
    assert(herdr.workspace_refusal("1", ""), "an empty HERDR_WORKSPACE_ID was allowed")
  end,

  ["the lookup runs inside a herdr pane"] = function()
    local refusal = herdr.workspace_refusal("1", "wW")
    assert(refusal == nil, tostring(refusal))
  end,

  -- Finding 2. `agents.display` renders kind, status and cwd basename, which two
  -- Claude agents in two tabs of one repository share exactly.
  ["the picker row names the pane, so two identical agents differ"] = function()
    local display = "claude · idle · dotfiles"
    local first = herdr.picker_label(display, "wW:p3K")
    local second = herdr.picker_label(display, "wW:p8K")
    assert(first ~= second, first .. " == " .. second)
    assert(first:find("wW:p3K", 1, true), first)
    assert(second:find("wW:p8K", 1, true), second)
  end,

  -- Finding 3. Native Neovim behavior, so the case runs against a real buffer in
  -- real blockwise Visual mode rather than a stub. `<C-v>j$` from `b` over
  -- `abc123456` / `xyz` yanks `bc123456` / `yz`; reading the marks after leaving
  -- Visual mode loses the to-end-of-line intent and truncated it to `bc1` / `yz`.
  ["a blockwise selection to the end of the line is not truncated"] = function()
    local buffer = vim.api.nvim_create_buf(false, true)
    vim.api.nvim_set_current_buf(buffer)
    vim.api.nvim_buf_set_lines(buffer, 0, -1, false, { "abc123456", "xyz" })
    vim.api.nvim_win_set_cursor(0, { 1, 1 })
    vim.api.nvim_feedkeys(vim.api.nvim_replace_termcodes("<C-v>j$", true, false, true), "x", false)
    assert(vim.fn.mode() == "\22", "not in blockwise Visual mode: " .. vim.fn.mode())
    local lines = herdr.visual_lines()
    assert(#lines == 2, vim.inspect(lines))
    assert(lines[1] == "bc123456", vim.inspect(lines))
    assert(lines[2] == "yz", vim.inspect(lines))
  end,

  ["an ordinary blockwise selection keeps its width"] = function()
    local buffer = vim.api.nvim_create_buf(false, true)
    vim.api.nvim_set_current_buf(buffer)
    vim.api.nvim_buf_set_lines(buffer, 0, -1, false, { "abc123456", "xyz789" })
    vim.api.nvim_win_set_cursor(0, { 1, 1 })
    vim.api.nvim_feedkeys(vim.api.nvim_replace_termcodes("<C-v>jl", true, false, true), "x", false)
    local lines = herdr.visual_lines()
    assert(lines[1] == "bc", vim.inspect(lines))
    assert(lines[2] == "yz", vim.inspect(lines))
  end,

  ["a charwise selection sends exactly what is highlighted"] = function()
    local buffer = vim.api.nvim_create_buf(false, true)
    vim.api.nvim_set_current_buf(buffer)
    vim.api.nvim_buf_set_lines(buffer, 0, -1, false, { "abc123456" })
    vim.api.nvim_win_set_cursor(0, { 1, 1 })
    vim.api.nvim_feedkeys(vim.api.nvim_replace_termcodes("vll", true, false, true), "x", false)
    -- One call only: `visual_lines` leaves Visual mode, so a second call would
    -- read normal mode and throw E475 rather than report the first one's answer.
    local lines = herdr.visual_lines()
    assert(table.concat(lines, "\n") == "bc1", vim.inspect(lines))
  end,

  -- Finding 4. `herdr agent prompt` refuses an agent that is already blocked;
  -- `herdr pane send-text` does not, so the non-submitting path could lose the
  -- race between the status read and the dispatch and type into the approval
  -- dialog. A second status check does not close it, so the path is gone.
  ["a non-submitting send is refused by name"] = function()
    for _, opts in ipairs({ { submit = false }, {} }) do
      local ok, err = pcall(herdr.send, "some text", opts)
      assert(not ok, "a non-submitting send was accepted")
      assert(tostring(err):find("submit", 1, true), tostring(err))
    end
    local ok, err = pcall(herdr.send, "some text")
    assert(not ok, "a send with no options at all was accepted")
    assert(tostring(err):find("submit", 1, true), tostring(err))
  end,

  -- Finding 1, the second half. The gate makes `agents.list()` scope itself, so
  -- this filter is a second lock on the same door; it is pinned so that a change
  -- to either one cannot quietly leave the door open.
  ["the filter keeps only this workspace's Claude agents"] = function()
    local all = {
      { kind = "claude", workspace_id = "wW", pane_id = "wW:p3K" },
      { kind = "claude", workspace_id = "wX", pane_id = "wX:p1" },
      { kind = "codex", workspace_id = "wW", pane_id = "wW:p8K" },
    }
    local here = herdr.claude_agents_here(all, "wW")
    assert(#here == 1, vim.inspect(here))
    assert(here[1].pane_id == "wW:p3K", vim.inspect(here))
  end,

  ["the filter keeps nothing when the workspace is unknown"] = function()
    local all = { { kind = "claude", workspace_id = "wW", pane_id = "wW:p3K" } }
    assert(#herdr.claude_agents_here(all, nil) == 0, "a nil workspace matched an agent")
    assert(#herdr.claude_agents_here(all, "wX") == 0, "another workspace's agent matched")
  end,

  -- Round 2. HERDR_WORKSPACE_ID and HERDR_PANE_ID are stamped in when the
  -- TERMINAL launches and are never updated, but a cross-workspace pane move
  -- keeps the terminal and changes both. Measured on herdr 0.8.2: a pane moved
  -- from w18:p2 to w19:p2 kept terminal term_65ab65225b56329 while its shell
  -- still reported HERDR_WORKSPACE_ID=w18, and `herdr pane current --current`
  -- answered w19:p2 in w19. An editor moved out from under its agent would
  -- otherwise keep sending to the workspace it was started in.
  ["a pane that has moved workspaces is refused"] = function()
    local moved = herdr.stale_workspace_refusal("wW", "wX")
    assert(moved, "a moved pane was allowed")
    assert(moved:find("wX", 1, true) and moved:find("wW", 1, true), moved)
  end,

  -- A herdr that cannot answer is its own refusal, with its own words: reusing
  -- the moved-pane message would tell the operator their editor moved to a
  -- workspace called nil, which sends them looking for a move that never
  -- happened.
  ["a pane herdr cannot place is refused without claiming it moved"] = function()
    local unplaced = herdr.stale_workspace_refusal("wW", nil)
    assert(unplaced, "an unresolvable live workspace was allowed")
    assert(unplaced:find("could not say", 1, true), unplaced)
    assert(not unplaced:find("moved", 1, true), unplaced)
  end,

  ["a pane that has not moved is allowed"] = function()
    local refusal = herdr.stale_workspace_refusal("wW", "wW")
    assert(refusal == nil, tostring(refusal))
  end,

  -- The refusal has to happen BEFORE the listing and instead of the
  -- continuation: a lookup that ran would have read the old workspace's agents,
  -- and a continuation that ran would have let `launch_or_attach` split a pane.
  ["a stale workspace lists no agents and runs no continuation"] = function()
    local calls = with_stubbed_herdr({ workspace_id = "wW", live_workspace_id = "wX" })
    assert(calls.list == 0, "agents.list was called " .. calls.list .. " times")
    assert(calls.continuation == 0, "the continuation ran " .. calls.continuation .. " times")
  end,

  ["a pane still in its own workspace lists agents and runs the continuation"] = function()
    local calls = with_stubbed_herdr({ workspace_id = "wW", live_workspace_id = "wW" }, function(pane_id)
      assert(pane_id == "wW:p3K", tostring(pane_id))
    end)
    assert(calls.list == 1, "agents.list was called " .. calls.list .. " times")
    assert(calls.continuation == 1, "the continuation ran " .. calls.continuation .. " times")
  end,
}
