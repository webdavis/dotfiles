local function report()
  return require("uu.report")
end

return {
  ["a plugin with updates is listed by name and the run is pending"] = function()
    local lines, status = report().plugin_lines({ { name = "finder", updates = true } })
    assert(
      status == report().PENDING
        and vim.deep_equal(lines, { "finder: updates available", "plugins: 0 current, 1 pending, 0 failed" })
    )
  end,
  ["a plugin with errors fails the run even when others are current"] = function()
    local lines, status = report().plugin_lines({ { name = "broken", errors = true }, { name = "current" } })
    assert(
      status == report().FAILED
        and lines[1] == "broken: plugin check failed"
        and lines[2] == "plugins: 1 current, 0 pending, 1 failed"
    )
  end,
  ["a current plugin is counted and not named"] = function()
    local lines, status = report().plugin_lines({ { name = "quiet" } })
    assert(status == report().OK and vim.deep_equal(lines, { "plugins: 1 current, 0 pending, 0 failed" }))
  end,
  ["no other socket means no notice"] = function()
    assert(report().restart_notice(report().other_instances({ "/private/run/nvim.42.0" }, 42)) == nil)
  end,
  ["two other sockets read as two instances and never count our own"] = function()
    local count = report().other_instances({ "/r/one/nvim.41.0", "/r/two/nvim.42.0", "/r/three/nvim.43.0" }, 42)
    assert(
      count == 2
        and report().restart_notice(count)
          == "2 Neovim instance(s) were running during this update; restart them to load the new versions"
    )
  end,
  ["a mapping whose rhs changed is neither added nor removed"] = function()
    local before = report().keymap_rows({
      n = { { lhs = "x", rhs = "old", desc = "old description" }, { lhs = "gone", rhs = "old" } },
    })
    local after = report().keymap_rows({
      n = { { lhs = "x", rhs = "new", desc = "new description" }, { lhs = "new", rhs = "new" } },
    })
    local added, removed = report().keymap_diff(before, after)
    assert(vim.deep_equal(added, { "n\tnew\tnew\t" }) and vim.deep_equal(removed, { "n\tgone\told\t" }))
  end,
  ["a health report's ERROR and WARNING lines are counted by severity"] = function()
    local errors, warnings = report().health_counts({
      "- ERROR failure",
      "- WARNING slow",
      "- OK all fine",
      "ERROR second failure",
      "ordinary prose about ERROR",
    })
    assert(errors == 2 and warnings == 1)
  end,
  ["a dirty lock file refuses the commit and names the reason"] = function()
    local allowed, why = report().commit_allowed(" M dot_config/nvim/lazy-lock.json\n", "refs/heads/main")
    assert(not allowed and why:find("lock", 1, true) and why:find("dirty", 1, true))
  end,
  ["a detached HEAD refuses the commit"] = function()
    local allowed, why = report().commit_allowed("", "")
    assert(not allowed and why:find("detached", 1, true))
  end,
  ["a clean lock on a branch allows it"] = function()
    local allowed, why = report().commit_allowed("", "refs/heads/topic\n")
    assert(allowed and why == nil)
  end,
  ["the run directory's parent is the per-user socket root"] = function()
    local stdpath, glob = vim.fn.stdpath, vim.fn.glob
    local pattern
    vim.fn.stdpath = function(kind)
      assert(kind == "run")
      return "/private/fixture/nvim.user/one-instance"
    end
    vim.fn.glob = function(value)
      pattern = value
      return { "/private/fixture/nvim.user/other/nvim.42.0" }
    end
    local ok, sockets = pcall(function()
      return report().running_sockets()
    end)
    vim.fn.stdpath, vim.fn.glob = stdpath, glob
    assert(ok, sockets)
    assert(pattern == "/private/fixture/nvim.user/*/nvim.*.0" and #sockets == 1)
  end,
  ["a pending plugin report closes its owned Neovim server socket"] = function()
    local fixture = dofile(vim.fn.fnamemodify(debug.getinfo(1, "S").source:sub(2), ":h") .. "/uu_fixture.lua")
    local root = vim.fn.tempname()
    local module = debug.getinfo(report().plugin_lines, "S").source:sub(2)
    local config = vim.fn.fnamemodify(module, ":h:h:h")
    local marker, init = root .. "/socket", root .. "/init.lua"
    fixture.write(init, (string.format("package.path = %q .. package.path\n", config .. "/lua/?.lua;")) .. [[
package.loaded["lazy.core.config"] = { plugins = { fixture = { _ = { updates = true } } } }
package.loaded["lazy.core.plugin"] = { has_errors = function() return false end }
package.loaded["lazy.manage"] = { check = function(options) assert(options.wait and not options.show) end }
package.loaded["uu.writeback"] = { run = function() return { kind = "check", lines = {} } end }
]] .. string.format("local f = assert(io.open(%q, 'w')); f:write(vim.v.servername); f:close()\n", marker))
    local child = vim
      .system({ vim.v.progpath, "--headless", "-u", init, "-l", config .. "/lua/uu/plugins.lua" }, { text = true })
      :wait()
    local socket = fixture.read(marker)
    assert(child.code == 100 and child.stdout:find("fixture: updates available", 1, true), child.stdout .. child.stderr)
    assert(socket ~= "" and vim.uv.fs_stat(socket) == nil, "completed plugin report left its owned server socket")
  end,
}
