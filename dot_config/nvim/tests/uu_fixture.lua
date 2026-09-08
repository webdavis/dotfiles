local M = {}

function M.write(path, text)
  vim.fn.mkdir(vim.fn.fnamemodify(path, ":h"), "p", 448)
  local file = assert(io.open(path, "wb"))
  assert(file:write(text))
  assert(file:close())
end

function M.read(path)
  local file = assert(io.open(path, "rb"))
  local text = file:read("*a")
  file:close()
  return text
end

-- Reading NO global or system config is one move that covers every setting the
-- operator's own git carries, rather than neutralizing them one at a time. It
-- goes on each command rather than in the process environment: `git_spec`
-- unsets every `GIT_*` name process-wide before any fixture exists, and a
-- per-command table survives that where an exported variable does not.
local NO_USER_CONFIG = { GIT_CONFIG_GLOBAL = "/dev/null", GIT_CONFIG_SYSTEM = "/dev/null" }

function M.git(repo, ...)
  local args = { "git", "-C", repo }
  vim.list_extend(args, { ... })
  local result = vim.system(args, { text = true, env = NO_USER_CONFIG }):wait()
  assert(result.code == 0, table.concat(args, " ") .. ": " .. result.stderr)
  return (result.stdout:gsub("\n$", ""))
end

local function repository(path)
  vim.fn.mkdir(path, "p", 448)
  M.git(path, "init", "-b", "fixture")
  M.git(path, "config", "user.name", "Owned Fixture")
  M.git(path, "config", "user.email", "fixture@example.invalid")
  -- The code under test runs git through its OWN runner, which passes no
  -- environment, so it reads the operator's global config whatever this file
  -- does. Only the repository's own settings outrank that, so the three that
  -- reach into a fixture are pinned here:
  --   hooksPath, or the operator's hooks shadow the ones a spec installs and a
  --     spec asserting its own hook ran fails here while passing in CI
  --   gpgsign,   or every fixture commit is signed with the operator's real key,
  --     which on a passphrase-locked key hangs the run on a headless prompt
  --   fsmonitor, or every throwaway repository leaves a daemon behind it
  M.git(path, "config", "core.hooksPath", path .. "/.git/hooks")
  M.git(path, "config", "commit.gpgsign", "false")
  M.git(path, "config", "core.fsmonitor", "false")
end

function M.new()
  local root = vim.fn.tempname()
  local f = { root = root, repo = root .. "/repo", config = root .. "/config", plugin = root .. "/plugin", updates = 0 }
  repository(f.plugin)
  M.write(f.plugin .. "/owned.txt", "old\n")
  M.git(f.plugin, "add", "owned.txt")
  M.git(f.plugin, "commit", "-m", "test: seed owned plugin")
  f.old = M.git(f.plugin, "rev-parse", "HEAD")
  M.write(f.plugin .. "/owned.txt", "new\n")
  M.git(f.plugin, "commit", "-am", "test: advance owned plugin")
  f.new = M.git(f.plugin, "rev-parse", "HEAD")
  M.git(f.plugin, "checkout", f.old)
  f.original = vim.json.encode({ fixture = { commit = f.old, branch = "fixture" } }) .. "\n"
  f.candidate = vim.json.encode({ fixture = { commit = f.new, branch = "fixture" } }) .. "\n"
  f.source = f.repo .. "/dot_config/nvim/lazy-lock.json"
  f.deployed = f.config .. "/lazy-lock.json"
  repository(f.repo)
  M.write(f.source, f.original)
  M.write(f.deployed, f.original)
  M.write(f.repo .. "/unrelated.txt", "original unrelated\n")
  M.git(f.repo, "add", "dot_config/nvim/lazy-lock.json", "unrelated.txt")
  M.git(f.repo, "commit", "-m", "test: seed source lock")
  f.head = M.git(f.repo, "rev-parse", "HEAD")
  f.options = {
    repo = f.repo,
    config = f.config,
    auto_commit = true,
    recovery = root .. "/state/recovery.json",
    plugins = function()
      return { fixture = f.plugin }
    end,
    update = function()
      f.updates = f.updates + 1
      M.git(f.plugin, "checkout", f.new)
      M.write(f.deployed, f.candidate)
      return {}, 0
    end,
  }
  return f
end

function M.hook(fixture, body)
  local path = fixture.repo .. "/.git/hooks/pre-commit"
  M.write(path, "#!/bin/bash\nset -euo pipefail\n" .. body .. "\n")
  assert(vim.uv.fs_chmod(path, 493))
end

function M.staged_other(fixture)
  M.write(fixture.repo .. "/unrelated.txt", "staged operator change\n")
  M.git(fixture.repo, "add", "unrelated.txt")
  return M.git(fixture.repo, "rev-parse", ":unrelated.txt")
end

function M.module(module, init)
  local root = vim.fn.tempname()
  local report = require("uu.report")
  local source = debug.getinfo(report.plugin_lines, "S").source:sub(2)
  local config = vim.fn.fnamemodify(source, ":h:h:h")
  local path = root .. "/init.lua"
  M.write(path, string.format("package.path = %q .. package.path\n", config .. "/lua/?.lua;") .. init)
  return vim
    .system({ vim.v.progpath, "--headless", "-u", path, "-l", config .. "/lua/uu/" .. module .. ".lua" }, { text = true })
    :wait()
end

return M
