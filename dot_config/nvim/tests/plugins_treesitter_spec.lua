-- nvim-treesitter builds only grammars it knows, so the hook asks before installing. Plugin scratch filetypes are open-ended.
-- Both nvim-treesitter modules are faked. `lua` is bundled with Neovim, so it has a parser under `--clean`.

local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

-- Made-up names, because other specs put the real parser directory on the runtimepath.
local UNINSTALLED_LANGUAGE = "language_with_no_parser_on_disk"
local REGISTERED_LATER_LANGUAGE = "language_registered_after_startup"
local AVAILABLE = { "lua", UNINSTALLED_LANGUAGE, "vimdoc" }

---Load the treesitter spec against faked modules, run its `config()`, and hand `fn` a recorder.
---`recorder.available` is what the next `get_available()` call answers. The fakes stay in place while `fn` runs.
---@param fn fun(recorder: { installs: string[], available: string[] })
local function with_treesitter(fn, opts)
  local names = { "nvim-treesitter", "nvim-treesitter.config" }
  local saved = {}
  for _, name in ipairs(names) do
    saved[name] = { package.loaded[name] }
  end

  local recorder = { installs = {}, available = vim.deepcopy(AVAILABLE), task = {} }

  package.loaded["nvim-treesitter"] = {
    install = function(languages)
      for _, language in ipairs(type(languages) == "table" and languages or { languages }) do
        table.insert(recorder.installs, language)
      end
      return recorder.task
    end,
  }
  package.loaded["nvim-treesitter.config"] = {
    get_available = function()
      return vim.deepcopy(recorder.available)
    end,
  }

  local ok, err = pcall(function()
    local spec = dofile(config_root .. "/lua/plugins/treesitter.lua")
    assert(spec[1] == "nvim-treesitter/nvim-treesitter", "the nvim-treesitter spec moved out of plugins/treesitter.lua")
    spec._ = {}
    spec.config(spec, opts or spec.opts)
    fn(recorder, spec)
  end)

  for _, name in ipairs(names) do
    package.loaded[name] = saved[name][1]
  end
  assert(ok, err)
end

---Give a fresh scratch buffer a filetype, which is what fires the hook.
---@param filetype string
---@return integer
local function open(filetype)
  local buffer = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_set_option_value("filetype", filetype, { buf = buffer })
  return buffer
end

---@param installs string[]
---@return string
local function joined(installs)
  return #installs == 0 and "<none>" or table.concat(installs, ",")
end

return {
  ["LazyDone installs the configured core list"] = function()
    with_treesitter(function(recorder, spec)
      vim.api.nvim_exec_autocmds("User", { pattern = "LazyDone" })
      assert(vim.deep_equal(recorder.installs, { "fixture_core" }), joined(recorder.installs))
      assert(spec._.core_parser_install == recorder.task, "lost the original install task")
    end, { ensure_installed = { "fixture_core" } })
  end,
  ["a filetype no grammar exists for is never queued for install"] = function()
    -- snacks.nvim notification buffers.
    with_treesitter(function(recorder)
      open("snacks_notif")
      assert(#recorder.installs == 0, "installed " .. joined(recorder.installs) .. " for snacks_notif")
    end)
  end,

  ["a mixed-case filetype with no grammar is never queued for install"] = function()
    -- Every nvim-treesitter parser name is lowercase, so a capitalized filetype never matches.
    with_treesitter(function(recorder)
      open("OverseerOutput")
      assert(#recorder.installs == 0, "installed " .. joined(recorder.installs) .. " for OverseerOutput")
    end)
  end,

  ["a dotted filetype whose base language has no grammar is never queued"] = function()
    -- `get_lang` reduces `atlas.notes` to `atlas`.
    with_treesitter(function(recorder)
      open("atlas.notes")
      assert(#recorder.installs == 0, "installed " .. joined(recorder.installs) .. " for atlas.notes")
    end)
  end,

  ["checkhealth is left alone even though its language has a parser"] = function()
    -- `get_lang` resolves this to the bundled `vimdoc`, so it stays on the ignore list.
    with_treesitter(function(recorder)
      local buffer = open("checkhealth")
      assert(vim.treesitter.highlighter.active[buffer] == nil, "treesitter started on the checkhealth buffer")
      assert(#recorder.installs == 0, "installed " .. joined(recorder.installs) .. " for checkhealth")
    end)
  end,

  ["a filetype whose parser is already loaded is never queued"] = function()
    with_treesitter(function(recorder)
      local buffer = open("lua")
      assert(vim.treesitter.highlighter.active[buffer] ~= nil, "treesitter never started on the lua buffer")
      assert(#recorder.installs == 0, "installed " .. joined(recorder.installs) .. " for an already-parsed buffer")
    end)
  end,

  ["a parser nvim-treesitter does not list is still used for highlighting"] = function()
    -- `c` is bundled and absent from AVAILABLE. The availability test guards only installing, so it sits below highlighting.
    with_treesitter(function(recorder)
      local buffer = open("c")
      assert(vim.treesitter.highlighter.active[buffer] ~= nil, "treesitter never started on the c buffer")
      assert(#recorder.installs == 0, "installed " .. joined(recorder.installs) .. " for an already-parsed buffer")
    end)
  end,

  ["a language nvim-treesitter has but has not built yet is queued"] = function()
    assert(
      #vim.api.nvim_get_runtime_file("parser/" .. UNINSTALLED_LANGUAGE .. ".*", true) == 0,
      "a parser for " .. UNINSTALLED_LANGUAGE .. " exists, so this case no longer measures the missing-parser path"
    )
    with_treesitter(function(recorder)
      open(UNINSTALLED_LANGUAGE)
      assert(
        #recorder.installs == 1 and recorder.installs[1] == UNINSTALLED_LANGUAGE,
        "queued " .. joined(recorder.installs)
      )
    end)
  end,

  ["a language the registry gains after the first query is queued on the next"] = function()
    -- `:TSUpdate` can make a language installable mid-session, so the hook asks every time.
    assert(
      #vim.api.nvim_get_runtime_file("parser/" .. REGISTERED_LATER_LANGUAGE .. ".*", true) == 0,
      "a parser for " .. REGISTERED_LATER_LANGUAGE .. " exists, so this case no longer measures the missing-parser path"
    )
    with_treesitter(function(recorder)
      open("snacks_notif") -- the first query, answered without the new language
      table.insert(recorder.available, REGISTERED_LATER_LANGUAGE)
      open(REGISTERED_LATER_LANGUAGE)
      assert(
        #recorder.installs == 1 and recorder.installs[1] == REGISTERED_LATER_LANGUAGE,
        "queued " .. joined(recorder.installs)
      )
    end)
  end,
}
