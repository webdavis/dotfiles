-- custom_api.autosave (spec 5.3, item 42): auto-save.nvim keeps its hands off
-- the buffers claudecode.nvim opens for a proposed edit, so a diff is resolved
-- by `<leader>Cy`/`<leader>Cn` and never by a write that fired on a timer.
--
-- `should_save` takes the name and the buftype as arguments rather than reading
-- them off a buffer, so these cases need no buffer, no plugin and no claudecode
-- session: the exclusion rule is the whole behavior under test.

-- Required per case rather than once at the top of the file, so a missing
-- module fails every case by name instead of aborting the run before the first.
local function autosave()
  return require("custom_api.autosave")
end

return {
  ["a proposed-edit buffer is not auto-saved"] = function()
    assert(autosave().should_save("lua/plugins/lsp.lua (proposed)", "") == false)
  end,

  ["a proposed new-file buffer is not auto-saved"] = function()
    assert(autosave().should_save("lua/plugins/new.lua (NEW FILE - proposed)", "") == false)
  end,

  -- claudecode's diff buffers carry buftype "acwrite", which catches a proposed
  -- buffer whose name the plugin ever spells differently.
  ["an acwrite buffer is not auto-saved"] = function()
    assert(autosave().should_save("lua/plugins/lsp.lua", "acwrite") == false)
  end,

  ["an ordinary file buffer is auto-saved"] = function()
    assert(autosave().should_save("lua/plugins/lsp.lua", "") == true)
  end,
}
