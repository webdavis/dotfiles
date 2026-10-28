-- `OverseerShell` reads the raw argument string, so the lazy proxy must forward the exact text typed.
-- `lazy` is faked and stands up the real command with the declaration overseer uses.

local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

local function plugin_spec()
  return dofile(config_root .. "/lua/plugins/overseer.lua")
end

-- Run the plugin `init`, invoke `OverseerShell` once, and report what reached the real command.
-- `loaded` records what the fake `lazy.load` was asked for.
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

-- The same line sent straight to the real command, the result the proxy must match.
local function direct_invocation(line)
  local received
  pcall(vim.api.nvim_del_user_command, "OverseerShell")
  vim.api.nvim_create_user_command("OverseerShell", function(params)
    received = params.args
  end, { nargs = "*", bang = true, complete = "shellcmdline" })
  local ok, err = pcall(vim.cmd, line)
  pcall(vim.api.nvim_del_user_command, "OverseerShell")
  assert(ok, err)
  return received
end

-- `shellcmdline` expands these while parsing, so the proxy must not expand them again.
local EXPANSION_CASES = {
  [[OverseerShell! printf '\%s\n' hello]],
  [[OverseerShell! echo \#]],
  [[OverseerShell! echo \<cword>]],
  [[OverseerShell! cat weird\%name.txt]],
}

-- Run `fn` in a buffer with this name, or an unnamed one, then restore the previous buffer.
local function in_buffer(name, fn)
  local previous = vim.api.nvim_get_current_buf()
  vim.cmd(name and ("edit " .. name) or "enew")
  local ok, err = pcall(fn)
  vim.cmd("bwipeout!")
  if vim.api.nvim_buf_is_valid(previous) then
    vim.api.nvim_set_current_buf(previous)
  end
  assert(ok, err)
end

local function assert_no_second_expansion()
  for _, line in ipairs(EXPANSION_CASES) do
    local want = direct_invocation(line)
    local got = first_invocation(line).args
    assert(got == want, ("%s arrived as [%s], wanted [%s]"):format(line, got, want))
  end
end

return {
  ["the first OverseerShell forwards its arguments exactly as typed"] = function()
    -- An escaped space stays inside one filename.
    local received = first_invocation([[OverseerShell! ls a\ b]])
    assert(received.args == [[ls a\ b]], "arguments arrived as [" .. received.args .. "]")
  end,

  ["the first OverseerShell keeps repeated spaces"] = function()
    -- Runs of spaces survive.
    local received = first_invocation([[OverseerShell! echo  a   b]])
    assert(received.args == [[echo  a   b]], "arguments arrived as [" .. received.args .. "]")
  end,

  ["the first OverseerShell forwards its bang"] = function()
    -- Bang stages the task without starting it.
    assert(first_invocation([[OverseerShell! ls]]).bang == true, "the bang was dropped")
    assert(first_invocation([[OverseerShell ls]]).bang == false, "a bang appeared from nowhere")
  end,

  ["the first OverseerShell forwards its modifiers"] = function()
    assert(first_invocation([[silent OverseerShell! ls]]).silent == true, "the modifiers were dropped")
  end,

  ["a bare OverseerShell reaches overseer with no argument"] = function()
    -- No argument forwards no argument, so overseer prompts for the command.
    assert(first_invocation("OverseerShell").args == "", "a bare call invented an argument")
  end,

  ["the first OverseerShell expands no filename token a second time"] = function()
    -- A second expansion would rewrite what the operator escaped.
    in_buffer("overseer_spec_current.lua", assert_no_second_expansion)
  end,

  ["the first OverseerShell expands nothing twice in an unnamed buffer either"] = function()
    -- With no file name, a second expansion fails the call with E499.
    in_buffer(nil, assert_no_second_expansion)
  end,

  ["the OverseerShell proxy loads overseer before forwarding"] = function()
    local _, loaded = first_invocation([[OverseerShell! ls]])
    assert(vim.tbl_contains(loaded or {}, "overseer.nvim"), "overseer was not loaded first")
  end,

  ["the OverseerShell proxy declares what overseer declares"] = function()
    -- The proxy accepts the same command shape as the real one.
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
    -- A name left in `cmd` would let the lazy placeholder take it back.
    assert(
      not vim.tbl_contains(plugin_spec().cmd, "OverseerShell"),
      "OverseerShell is still a cmd trigger, so the placeholder wins the name"
    )
  end,
}
