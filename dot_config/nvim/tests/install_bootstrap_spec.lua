local function with_modules(modules, run)
  local saved = {}
  for name, value in pairs(modules) do
    saved[name] = package.loaded[name]
    package.loaded[name] = value
  end
  local ok, err = pcall(run, require("custom_api.bootstrap"))
  for name in pairs(modules) do
    package.loaded[name] = saved[name]
  end
  assert(ok, err)
end

local function task(failed, running)
  return {
    name = "build",
    has_errors = function()
      return failed
    end,
    running = function()
      return running
    end,
    output = function()
      return "fixture compiler exited 42"
    end,
  }
end

local function mason_modules(refresh_ok, install_ok)
  local installed, looked_up = {}, {}
  local modules = {
    ["lazy.core.config"] = {
      plugins = {
        ["mason-lspconfig.nvim"] = { opts = { ensure_installed = { "bashls", "lua_ls" } } },
        ["mason-tool-installer.nvim"] = { opts = { ensure_installed = { "stylua", "lua-language-server" } } },
      },
    },
    ["lazy.core.plugin"] = {
      values = function(plugin)
        return plugin.opts
      end,
    },
    ["mason-lspconfig.mappings"] = {
      get_mason_map = function()
        return { lspconfig_to_package = { bashls = "bash-language-server", lua_ls = "lua-language-server" } }
      end,
    },
    ["mason-registry"] = {
      refresh = function(callback)
        vim.schedule(function()
          callback(refresh_ok, "fixture registry transport failed")
        end)
      end,
      get_package = function(name)
        looked_up[#looked_up + 1] = name
        return {
          is_installed = function()
            return name == "stylua"
          end,
          install = function(_, _, callback)
            vim.schedule(function()
              installed[#installed + 1] = name
              callback(install_ok, "fixture package download failed")
            end)
            return { on = function() end }
          end,
        }
      end,
    },
  }
  return modules, installed, looked_up
end

local function parser_modules(success, artifacts)
  local state = { finished = false, requested = {}, force = false }
  local modules = {
    ["lazy.core.config"] = {
      plugins = { ["nvim-treesitter"] = { build = ":TSUpdate", opts = { ensure_installed = { "lua" } }, _ = {} } },
    },
    ["lazy.core.plugin"] = {
      values = function(plugin)
        return plugin.opts
      end,
    },
    ["nvim-treesitter.config"] = {
      norm_languages = function(languages)
        assert(vim.deep_equal(languages, { "lua" }), "did not use the configured core list")
        return { "lua", "ecma" }
      end,
    },
    ["nvim-treesitter.parsers"] = { lua = { install_info = {} }, ecma = {} },
    ["nvim-treesitter"] = {
      update = function()
        return {
          wait = function()
            return true
          end,
        }
      end,
      get_installed = function(kind)
        if state.finished then
          return artifacts[kind]
        end
        -- A leftover query directory must not count as a compiled parser.
        return kind == "queries" and { "lua" } or {}
      end,
      install = function(languages, options)
        state.requested, state.force = languages, options and options.force
        vim.defer_fn(function()
          state.finished = true
        end, 20)
        return {
          wait = function()
            assert(
              vim.wait(500, function()
                return state.finished
              end),
              "fixture installation never completed"
            )
            return success
          end,
        }
      end,
    },
  }
  return modules, state
end

return {
  ["installs the server and tool union and waits for asynchronous completion"] = function()
    local modules, installed = mason_modules(true, true)
    with_modules(modules, function(subject)
      local names = subject.install_mason()
      assert(vim.deep_equal(names, { "bash-language-server", "lua-language-server", "stylua" }), vim.inspect(names))
      assert(vim.deep_equal(installed, { "bash-language-server", "lua-language-server" }), vim.inspect(installed))
    end)
  end,
  ["a registry failure returns its diagnostic before package lookup"] = function()
    local modules, installed, looked_up = mason_modules(false, true)
    with_modules(modules, function(subject)
      local ok, err = pcall(subject.install_mason)
      assert(not ok and tostring(err):find("fixture registry transport failed", 1, true), tostring(err))
      assert(#looked_up == 0 and #installed == 0, "entered the installer after registry failure")
    end)
  end,
  ["a failed package callback fails the bootstrap with the package name"] = function()
    with_modules(mason_modules(true, false), function(subject)
      local ok, err = pcall(subject.install_mason)
      assert(not ok and tostring(err):find("bash-language-server", 1, true), tostring(err))
      assert(tostring(err):find("fixture package download failed", 1, true), tostring(err))
    end)
  end,
  ["empty resolved tool lists cannot pass"] = function()
    local modules = mason_modules(true, true)
    for _, plugin in pairs(modules["lazy.core.config"].plugins) do
      plugin.opts.ensure_installed = {}
    end
    with_modules(modules, function(subject)
      local ok, err = pcall(subject.install_mason)
      assert(not ok and tostring(err):find("empty", 1, true), tostring(err))
    end)
  end,
  ["verification drops a platform disabled plugin while preserving enabled pins"] = function()
    with_modules({ ["lazy.core.config"] = { plugins = { active = { _ = {} } } } }, function(subject)
      local pin = { commit = string.rep("a", 40) }
      local lock = subject.enabled_lock({ active = pin, ["xcodebuild.nvim"] = { commit = string.rep("b", 40) } })
      assert(vim.deep_equal(lock, { active = pin }), vim.inspect(lock))
    end)
  end,
  ["an enabled remote plugin without a source pin is refused"] = function()
    with_modules({ ["lazy.core.config"] = { plugins = { absent = { _ = {} } } } }, function(subject)
      local ok, err = pcall(subject.enabled_lock, {})
      assert(not ok and tostring(err):find("absent", 1, true), tostring(err))
    end)
  end,
  ["an empty enabled plugin inventory is refused"] = function()
    with_modules({ ["lazy.core.config"] = { plugins = {} } }, function(subject)
      local ok, err = pcall(subject.enabled_lock, {})
      assert(not ok and tostring(err):find("empty", 1, true), tostring(err))
    end)
  end,
  ["restore waits and reports a failed Lazy task"] = function()
    local waited = false
    local plugin = { name = "fixture", _ = { tasks = {} } }
    with_modules({
      ["lazy.core.config"] = { plugins = { fixture = plugin } },
      ["lazy"] = {
        restore = function(opts)
          waited = opts.wait == true
          plugin._.tasks = { task(true, false) }
        end,
      },
    }, function(subject)
      local ok, err = pcall(subject.restore_plugins)
      assert(waited, "restore did not wait")
      assert(not ok and tostring(err):find("fixture compiler exited 42", 1, true), tostring(err))
    end)
  end,
  ["builds already pinned plugins and waits for their tasks"] = function()
    local plugin = { name = "fixture", build = ":FixtureBuild", _ = { tasks = {} } }
    with_modules({
      ["lazy.core.config"] = { plugins = { fixture = plugin } },
      ["lazy"] = {
        build = function(opts)
          assert(opts.wait and opts.plugins[1] == plugin, "build did not select and wait for the pinned plugin")
          plugin._.tasks = { task(false, false) }
        end,
      },
    }, function(subject)
      subject.build_plugins()
      assert(#plugin._.tasks == 1, "build was skipped")
    end)
  end,
  ["an unsuccessful build fails even when the checkout is pinned"] = function()
    local plugin = { name = "fixture", _ = { tasks = {} } }
    with_modules({
      ["lazy.core.config"] = { plugins = { fixture = plugin } },
      ["lazy"] = {
        build = function()
          plugin._.tasks = { task(true, false) }
        end,
      },
    }, function(subject)
      local ok, err = pcall(subject.build_plugins)
      assert(not ok and tostring(err):find("fixture compiler exited 42", 1, true), tostring(err))
    end)
  end,
  ["a still running build cannot be mistaken for success"] = function()
    local plugin = { name = "fixture", _ = { tasks = {} } }
    with_modules({
      ["lazy.core.config"] = { plugins = { fixture = plugin } },
      ["lazy"] = {
        build = function()
          plugin._.tasks = { task(false, true) }
        end,
      },
    }, function(subject)
      local ok, err = pcall(subject.build_plugins)
      assert(not ok and tostring(err):find("running", 1, true), tostring(err))
    end)
  end,
  ["a missing build task cannot be mistaken for success"] = function()
    with_modules({
      ["lazy.core.config"] = {
        plugins = { fixture = { name = "fixture", build = ":FixtureBuild", _ = { tasks = {} } } },
      },
      ["lazy"] = { build = function() end },
    }, function(subject)
      local ok, err = pcall(subject.build_plugins)
      assert(not ok and tostring(err):find("build", 1, true), tostring(err))
    end)
  end,
  ["waits for parser updates and rejects an unsuccessful result"] = function()
    local waited = false
    local plugin = { name = "nvim-treesitter", build = ":TSUpdate", _ = { tasks = { task(false, false) } } }
    with_modules({
      ["lazy.core.config"] = { plugins = { ["nvim-treesitter"] = plugin } },
      ["lazy"] = { build = function() end },
      ["nvim-treesitter"] = {
        update = function()
          return {
            wait = function()
              waited = true
              return false
            end,
          }
        end,
      },
    }, function(subject)
      local ok, err = pcall(subject.build_plugins)
      assert(waited, "parser update did not finish")
      assert(not ok and tostring(err):find("parser", 1, true), tostring(err))
    end)
  end,
  ["waits for a delayed core install failure even when update found nothing"] = function()
    local modules, state = parser_modules(false, { parsers = {}, queries = {} })
    with_modules(modules, function(subject)
      local ok, err = pcall(subject.build_plugins)
      assert(state.finished, "returned while core installation was pending")
      assert(not ok and tostring(err):find("core parser installation failed", 1, true), tostring(err))
    end)
  end,
  ["awaits the original LazyDone task and preserves its delayed failure"] = function()
    local modules, state = parser_modules(true, { parsers = { "lua" }, queries = { "ecma" } })
    modules["lazy.core.config"].plugins["nvim-treesitter"]._ = {
      core_parser_install = {
        wait = function()
          vim.defer_fn(function()
            state.finished = true
          end, 20)
          assert(
            vim.wait(500, function()
              return state.finished
            end),
            "fixture installation never completed"
          )
          return false
        end,
      },
    }
    with_modules(modules, function(subject)
      local ok, err = pcall(subject.build_plugins)
      assert(state.finished, "did not wait for the original installation")
      assert(not ok and tostring(err):find("core parser installation failed", 1, true), tostring(err))
      assert(#state.requested == 0, "replaced the failed task with another installation")
    end)
  end,
  ["waits for core parsers and query dependencies to finish successfully"] = function()
    local modules, state = parser_modules(true, { parsers = { "lua" }, queries = { "ecma" } })
    with_modules(modules, function(subject)
      subject.build_plugins()
      assert(state.finished, "returned while core installation was pending")
      assert(vim.deep_equal(state.requested, { "lua", "ecma" }), vim.inspect(state.requested))
      assert(state.force, "leftover queries can make an ordinary install skip the missing parser")
    end)
  end,
  ["a joined install reporting success still requires the compiled parser"] = function()
    local modules = parser_modules(true, { parsers = {}, queries = { "lua", "ecma" } })
    with_modules(modules, function(subject)
      local ok, err = pcall(subject.build_plugins)
      assert(not ok and tostring(err):find("missing core parsers: lua", 1, true), tostring(err))
    end)
  end,
  ["a joined install cannot omit a required query-only language"] = function()
    local modules = parser_modules(true, { parsers = { "lua" }, queries = {} })
    with_modules(modules, function(subject)
      local ok, err = pcall(subject.build_plugins)
      assert(not ok and tostring(err):find("missing core parsers: ecma", 1, true), tostring(err))
    end)
  end,
  ["already installed core languages need no forced installation"] = function()
    local modules, state = parser_modules(true, { parsers = { "lua" }, queries = { "ecma" } })
    state.finished = true
    with_modules(modules, function(subject)
      subject.build_plugins()
      assert(#state.requested == 0, "rebuilt an already installed core language")
    end)
  end,
  ["a silent shell build failure reports its exit status"] = function()
    with_modules({
      ["lazy.core.config"] = {
        plugins = {
          fixture = { build = "exit 42", dir = vim.fn.getcwd(), _ = {} },
        },
      },
    }, function(subject)
      local ok, err = pcall(subject.build_plugins)
      assert(not ok and tostring(err):find("42", 1, true), tostring(err))
    end)
  end,
  ["a successful shell build completes before returning"] = function()
    local out = vim.fn.tempname()
    with_modules({
      ["lazy.core.config"] = {
        plugins = {
          fixture = {
            build = "sleep 0.02; printf complete > " .. vim.fn.shellescape(out),
            dir = vim.fn.getcwd(),
            _ = {},
          },
        },
      },
    }, function(subject)
      subject.build_plugins()
      assert(vim.fn.readfile(out)[1] == "complete", "returned before the build wrote its result")
    end)
  end,
}
