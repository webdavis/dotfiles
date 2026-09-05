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
}
