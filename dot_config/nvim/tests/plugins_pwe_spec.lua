local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

return {
  ["navigation keys apply in normal mode only"] = function()
    local modes = dofile(config_root .. "/lua/plugins/pwe.lua").opts.modes
    assert(vim.deep_equal(modes, { "n" }), "modes are " .. vim.inspect(modes))
  end,
}
