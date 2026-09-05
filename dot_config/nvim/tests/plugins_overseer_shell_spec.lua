-- The `OverseerShell` lazy loader in `lua/plugins/overseer.lua`.
--
-- The subject is the proxy command the spec's `init` creates. `OverseerShell` is
-- the one overseer command that reads the RAW argument string, so it cannot ride
-- lazy.nvim's placeholder: the placeholder replays the command from `fargs`
-- joined with single spaces, which drops the shell escaping. The proxy has to
-- hand the real command the same text the operator typed.
--
-- The runner loads no plugins, so `lazy` is faked and the command overseer would
-- have created is stood up by that fake with overseer's own declaration
-- (`nargs="*"`, `bang`, `complete="shellcmdline"`). What is under test is the
-- proxy's forwarding, which is entirely this repo's code.

local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

local function plugin_spec()
  return dofile(config_root .. "/lua/plugins/overseer.lua")
end

-- Run the plugin spec's `init`, then invoke `OverseerShell` once and report what
-- reached the command overseer would have defined. `loaded` is what the fake
-- `lazy.load` was asked for, so a proxy that forwards without loading is caught.
local function first_invocation(line)
  local received, loaded

  local saved_lazy = package.loaded["lazy"]
  package.loaded["lazy"] = {
    load = function(opts)
      loaded = opts.plugins
      vim.api.nvim_create_user_command("OverseerShell", function(params)
        received = { args = params.args, bang = params.bang, silent = params.smods.silent }
      end, { nargs = "*", bang = true, complete = "shellcmdline" })
    end,
  }

  pcall(vim.api.nvim_del_user_command, "OverseerShell")
  local ok, err = pcall(function()
    plugin_spec().init()
    vim.cmd(line)
  end)

  package.loaded["lazy"] = saved_lazy
  pcall(vim.api.nvim_del_user_command, "OverseerShell")

  assert(ok, err)
  return assert(received, "the command overseer defines was never reached"), loaded
end

return {
  ["the first OverseerShell forwards its arguments exactly as typed"] = function()
    -- The regression: with overseer unloaded the placeholder rebuilt this line
    -- from `fargs`, so the task ran as `ls a b` and `a\ b` stopped being one
    -- filename.
    local received = first_invocation([[OverseerShell! ls a\ b]])
    assert(received.args == [[ls a\ b]], "arguments arrived as [" .. received.args .. "]")
  end,

  ["the first OverseerShell keeps repeated spaces"] = function()
    -- The same collapse, without any escaping to notice it by: joining `fargs`
    -- puts exactly one space between every pair of words.
    local received = first_invocation([[OverseerShell! echo  a   b]])
    assert(received.args == [[echo  a   b]], "arguments arrived as [" .. received.args .. "]")
  end,

  ["the first OverseerShell forwards its bang"] = function()
    -- Bang is "create the task but do not start it", so losing it would run a
    -- command the operator asked only to stage.
    assert(first_invocation([[OverseerShell! ls]]).bang == true, "the bang was dropped")
    assert(first_invocation([[OverseerShell ls]]).bang == false, "a bang appeared from nowhere")
  end,

  ["the first OverseerShell forwards its modifiers"] = function()
    assert(first_invocation([[silent OverseerShell! ls]]).silent == true, "the modifiers were dropped")
  end,

  ["a bare OverseerShell reaches overseer with no argument"] = function()
    -- Overseer prompts for the command when it gets none. `nvim_cmd` refuses an
    -- empty string as an argument, so forwarding one unconditionally turned this
    -- call into an error instead of a prompt.
    assert(first_invocation("OverseerShell").args == "", "a bare call invented an argument")
  end,

  ["the OverseerShell proxy loads overseer before forwarding"] = function()
    local _, loaded = first_invocation([[OverseerShell! ls]])
    assert(vim.tbl_contains(loaded or {}, "overseer.nvim"), "overseer was not loaded first")
  end,

  ["the OverseerShell proxy declares what overseer declares"] = function()
    -- The proxy has to accept the same shape as the real command or the first
    -- invocation fails on syntax before any of the above can matter.
    pcall(vim.api.nvim_del_user_command, "OverseerShell")
    plugin_spec().init()
    local info = assert(vim.api.nvim_get_commands({})["OverseerShell"], "no OverseerShell command")
    assert(info.nargs == "*", "nargs is " .. tostring(info.nargs))
    assert(info.bang == true, "the proxy does not take a bang")
    assert(info.complete == "shellcmdline", "completion is " .. tostring(info.complete))
    -- Built-in completion, so the command line answers without loading overseer.
    assert(#vim.fn.getcompletion("OverseerShell ", "cmdline") > 0, "completion answered nothing")
    pcall(vim.api.nvim_del_user_command, "OverseerShell")
  end,

  ["OverseerShell is not also a lazy cmd trigger"] = function()
    -- lazy installs its placeholders at startup too. Leaving the name in `cmd`
    -- would let the placeholder take it back and undo all of the above.
    assert(
      not vim.tbl_contains(plugin_spec().cmd, "OverseerShell"),
      "OverseerShell is still a cmd trigger, so the placeholder wins the name"
    )
  end,
}
