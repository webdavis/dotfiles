-- The FileType hook in `lua/plugins/treesitter.lua`, which auto-installs a
-- parser for every filetype it has not seen.
--
-- nvim-treesitter will only ever build a grammar it knows about. Asked for one
-- it does not have, it logs "skipping unsupported language" to stderr and the
-- hook then polls thirty seconds for a parser that is never coming. Plugins
-- name their own scratch buffers (`snacks_notif`, `atlas.notes`), so that class
-- of filetype is open-ended and the hook has to ask rather than carry a list.
--
-- nvim-treesitter is a plugin, absent under `nvim --clean`, so both of its
-- modules are faked here. `lua` is the one language that really does have a
-- parser under `--clean`: it is bundled with Neovim.

local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

-- What the faked `get_available()` answers at first. The two made-up entries are
-- names no parser file can be called rather than real languages: `neotest_spec`
-- appends the real `stdpath("data")/site` to the runtimepath and never takes it
-- back, so in an aggregate run every genuinely available language already has a
-- parser on this machine and the install cases would invert.
local UNINSTALLED_LANGUAGE = "language_with_no_parser_on_disk"
local REGISTERED_LATER_LANGUAGE = "language_registered_after_startup"
local AVAILABLE = { "lua", UNINSTALLED_LANGUAGE, "vimdoc" }

---Load the treesitter plugin spec against faked nvim-treesitter modules, run
---its `config()`, and hand `fn` a recorder of what the hook asked to install.
---`recorder.available` is the list the faked `get_available()` answers with on
---its next call, so a case can grow the registry mid-run.
---
---The fakes stay in place while `fn` runs: the hook resolves nothing at build
---time, so restoring `package.loaded` before firing a FileType would measure
---the real plugin instead of the doubles.
---@param fn fun(recorder: { installs: string[], available: string[] })
local function with_treesitter(fn)
  local names = { "nvim-treesitter", "nvim-treesitter.config" }
  local saved = {}
  for _, name in ipairs(names) do
    saved[name] = { package.loaded[name] }
  end

  local recorder = { installs = {}, available = vim.deepcopy(AVAILABLE) }

  package.loaded["nvim-treesitter"] = {
    install = function(languages)
      for _, language in ipairs(type(languages) == "table" and languages or { languages }) do
        table.insert(recorder.installs, language)
      end
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
    spec.config()
    fn(recorder)
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
  ["a filetype no grammar exists for is never queued for install"] = function()
    -- snacks.nvim's notification buffers. The measured symptom: one headless
    -- start in about five wrote the warning to stderr.
    with_treesitter(function(recorder)
      open("snacks_notif")
      assert(#recorder.installs == 0, "installed " .. joined(recorder.installs) .. " for snacks_notif")
    end)
  end,

  ["a mixed-case filetype with no grammar is never queued for install"] = function()
    -- The filetype a newer overseer.nvim pin gives its output buffers. Every
    -- parser nvim-treesitter ships is named in lowercase, so a filetype carrying
    -- capitals can never match one. This case is here so that pin can move
    -- without the warning coming back.
    with_treesitter(function(recorder)
      open("OverseerOutput")
      assert(#recorder.installs == 0, "installed " .. joined(recorder.installs) .. " for OverseerOutput")
    end)
  end,

  ["a dotted filetype whose base language has no grammar is never queued"] = function()
    -- `get_lang` reduces `atlas.notes` to `atlas`, so one availability test on
    -- the language covers every atlas.nvim window without a prefix match.
    with_treesitter(function(recorder)
      open("atlas.notes")
      assert(#recorder.installs == 0, "installed " .. joined(recorder.installs) .. " for atlas.notes")
    end)
  end,

  ["checkhealth is left alone even though its language has a parser"] = function()
    -- The one filetype the ignore list still earns: `get_lang` resolves it to
    -- `vimdoc`, which is bundled, so dropping it from the list as "covered by
    -- the availability test" would start highlighting health reports as help.
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
    -- `c` is bundled with Neovim and deliberately absent from AVAILABLE above.
    -- The availability test is asked only about INSTALLING, so it must sit below
    -- the highlighting attempt: hoisting it above would silently stop
    -- highlighting every parser that came from somewhere other than
    -- nvim-treesitter's own table.
    with_treesitter(function(recorder)
      local buffer = open("c")
      assert(vim.treesitter.highlighter.active[buffer] ~= nil, "treesitter never started on the c buffer")
      assert(#recorder.installs == 0, "installed " .. joined(recorder.installs) .. " for an already-parsed buffer")
    end)
  end,

  ["a language nvim-treesitter has but has not built yet is queued"] = function()
    -- The case the hook exists for. Without it the availability test would just
    -- be a way of never installing anything.
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
    -- `:TSUpdate` and `:TSInstall` reload nvim-treesitter's parser table and
    -- re-run every `User TSUpdate` registration, so a plugin update can make a
    -- language installable in the middle of a session. A set kept from the first
    -- answer rejected such a language until restart; the hook asks every time.
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
