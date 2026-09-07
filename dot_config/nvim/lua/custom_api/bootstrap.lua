local M = {}

local function await(start)
  local done, success, result = false, false, nil
  start(function(ok, value)
    success, result, done = ok, value, true
  end)
  while not done do
    local _, reason = vim.wait(10000, function()
      return done
    end)
    assert(reason ~= -2, "bootstrap interrupted")
  end
  return success, result
end

function M.enabled_lock(source)
  local lock = {}
  for name, plugin in pairs(require("lazy.core.config").plugins) do
    if source[name] then
      lock[name] = source[name]
    elseif not plugin._.is_local then
      error("enabled plugin has no source pin: " .. name)
    end
  end
  assert(next(lock), "enabled plugin inventory is empty")
  return lock
end

local function check_tasks(plugins, require_build)
  for name, plugin in pairs(plugins) do
    local built = false
    for _, task in ipairs(plugin._.tasks or {}) do
      assert(not task:running(), name .. " task still running: " .. task.name)
      assert(not task:has_errors(), name .. " " .. task.name .. ": " .. task:output())
      built = built or task.name == "build"
    end
    assert(not require_build or built, name .. " has no completed build task")
  end
end

function M.restore_plugins()
  require("lazy").restore({ wait = true, show = false })
  check_tasks(require("lazy.core.config").plugins, false)
end

local function shell_build(command, dir)
  local result = vim.system({ vim.env.SHELL or vim.o.shell, "-c", command }, { cwd = dir, text = true }):wait()
  io.write(result.stdout or "", result.stderr or "")
  assert(result.code == 0 and result.signal == 0, "build failed (exit " .. result.code .. "): " .. command)
end

local function install_core_parsers(plugin)
  local treesitter = require("nvim-treesitter")
  -- A second install can wait for the first one to stop without preserving its
  -- failure. Keep the actual LazyDone result before checking installed files.
  if plugin._.core_parser_install then
    assert(plugin._.core_parser_install:wait(), "core parser installation failed")
  end
  local declared = assert(require("lazy.core.plugin").values(plugin, "opts", false).ensure_installed)
  assert(#declared > 0, "core parser inventory is empty")
  local parsers = require("nvim-treesitter.parsers")
  for _, language in ipairs(declared) do
    assert(parsers[language], "unknown core parser: " .. language)
  end
  local required = require("nvim-treesitter.config").norm_languages(vim.deepcopy(declared))
  local function missing()
    local installed = { parsers = treesitter.get_installed("parsers"), queries = treesitter.get_installed("queries") }
    return vim.tbl_filter(function(language)
      return not vim.list_contains(installed.queries, language)
        or (parsers[language].install_info and not vim.list_contains(installed.parsers, language))
    end, required)
  end
  local absent = missing()
  if #absent > 0 then
    -- Ordinary install accepts either artifact. Force only incomplete
    -- languages so a leftover parser or query directory can recover.
    assert(treesitter.install(absent, { force = true }):wait(), "core parser installation failed")
  end
  absent = missing()
  assert(#absent == 0, "missing core parsers: " .. table.concat(absent, ", "))
end

function M.build_plugins()
  local plugins = require("lazy.core.config").plugins
  local managed = {}
  for name, plugin in pairs(plugins) do
    local build = plugin.build
    -- Lazy's shell task drops the subprocess status in headless mode. Run our
    -- shell build declarations with Neovim's process API and check that status.
    if type(build) == "string" and build:sub(1, 1) ~= ":" and not build:match("%.lua$") and build ~= "rockspec" then
      shell_build(build, plugin.dir)
    elseif name == "nvim-treesitter" and build == ":TSUpdate" then
      -- This command starts background work; its public task API waits and
      -- returns false when a parser could not be built.
      assert(require("nvim-treesitter").update():wait(), "parser update failed")
      install_core_parsers(plugin)
    else
      managed[name] = plugin
    end
  end
  -- Force builds even at the right commit: a previous failed build is not
  -- recorded across Neovim processes, and restore then sees nothing to do.
  if next(managed) then
    require("lazy").build({ plugins = vim.tbl_values(managed), wait = true, show = false })
    check_tasks(managed, true)
  end
end

function M.install_mason()
  local registry = require("mason-registry")
  -- The pinned registry discards errors without a callback. Do not enter the
  -- tool installer's completion loop after a registry transport failure.
  local ready, reason = await(registry.refresh)
  assert(ready, "Mason registry refresh failed: " .. vim.inspect(reason))
  local plugins = require("lazy.core.config").plugins
  local values = require("lazy.core.plugin").values
  local mapping = require("mason-lspconfig.mappings").get_mason_map().lspconfig_to_package
  local names, seen = {}, {}
  for _, plugin_name in ipairs({ "mason-lspconfig.nvim", "mason-tool-installer.nvim" }) do
    local spec = assert(plugins[plugin_name], plugin_name .. " is not enabled")
    local declared = assert(values(spec, "opts", false).ensure_installed, plugin_name .. " has no tool list")
    for _, declared_name in ipairs(declared) do
      assert(type(declared_name) == "string" and declared_name:match("%S"), "invalid Mason tool name")
      local name = declared_name
      if plugin_name == "mason-lspconfig.nvim" then
        name = assert(mapping[declared_name], "unmapped Mason server: " .. declared_name)
      end
      if not seen[name] then
        seen[name], names[#names + 1] = true, name
      end
    end
  end
  assert(#names > 0, "Mason tool inventory is empty")
  table.sort(names)
  -- Resolve every package before starting any install, so an invalid name
  -- fails without leaving a partial queue waiting on an impossible callback.
  local packages = {}
  for _, name in ipairs(names) do
    packages[name] = registry.get_package(name)
  end
  for _, name in ipairs(names) do
    local package = packages[name]
    if not package:is_installed() then
      local ok, err = await(function(callback)
        local handle = package:install({}, callback)
        for _, event in ipairs({ "stdout", "stderr" }) do
          handle:on(event, function(chunk)
            io.write(chunk)
          end)
        end
      end)
      assert(ok, "Mason package " .. name .. " failed: " .. vim.inspect(err))
    end
  end
  return names
end

return M
