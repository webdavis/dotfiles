local paired_plugin = require("custom_api.paired_plugin")

local function with_data_home(prepare)
  local data_home = vim.fn.tempname()
  assert(vim.fn.mkdir(data_home .. "/nvim/paired", "p") == 1, "could not create " .. data_home)
  local before = vim.env.XDG_DATA_HOME
  vim.env.XDG_DATA_HOME = data_home
  prepare(data_home .. "/nvim/paired")
  local present = paired_plugin.is_present("tool")
  local spec = paired_plugin.spec("tool")
  vim.env.XDG_DATA_HOME = before
  return present, spec, data_home .. "/nvim/paired/tool"
end

return {
  ["a checkout that holds Lua is present"] = function()
    local present = with_data_home(function(paired)
      assert(vim.fn.mkdir(paired .. "/tool/lua", "p") == 1)
    end)
    assert(present == true)
  end,

  ["a missing checkout is absent"] = function()
    local present = with_data_home(function() end)
    assert(present == false)
  end,

  ["a checkout whose commit was never fetched is absent"] = function()
    local present = with_data_home(function(paired)
      assert(vim.fn.mkdir(paired .. "/tool/.git", "p") == 1)
    end)
    assert(present == false)
  end,

  ["a plain file where the checkout belongs is absent"] = function()
    local present = with_data_home(function(paired)
      assert(vim.fn.writefile({ "" }, paired .. "/tool") == 0)
    end)
    assert(present == false)
  end,

  ["the spec loads the plugin from its checkout under its own name"] = function()
    local _, spec, checkout = with_data_home(function() end)
    assert(spec.dir == checkout, spec.dir)
    assert(spec.name == "tool")
  end,

  ["the spec switches the plugin off while its checkout is absent"] = function()
    local _, spec = with_data_home(function() end)
    assert(spec.enabled() == false)
  end,
}
