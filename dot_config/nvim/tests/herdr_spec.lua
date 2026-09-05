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
}
