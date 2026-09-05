-- Mason's bin directory on PATH at startup, from `lua/plugins/lsp.lua`.
--
-- Every spec in that file is lazy, so nothing calls `mason.setup()` during startup
-- any more and nothing does its PATH prepend either. Treesitter builds parsers from
-- `LazyDone` and from a fileless `:TSUpdate`, both before any buffer trigger fires,
-- and `tree-sitter` lives only under Mason's bin on this machine. The `init` on the
-- mason.nvim spec is what closes that window, so this reads the shipped spec table
-- rather than restating the path here.
--
-- `vim.env` is swapped for a plain table around each call: `init` writes through
-- whatever `vim.env` names, so a plain table captures the write and the real process
-- environment is never touched.

local config_root = assert(package.path:match("^(.-)/lua/%?%.lua;"), "config root not on package.path")

local function mason_spec()
  for _, spec in ipairs(dofile(config_root .. "/lua/plugins/lsp.lua")) do
    if spec[1] == "mason-org/mason.nvim" then
      return spec
    end
  end
  error("no mason-org/mason.nvim spec in lua/plugins/lsp.lua")
end

local mason_bin = vim.fn.stdpath("data") .. "/mason/bin"

-- Run the spec's `init` against a stub PATH and hand back what it left behind.
-- `calls` runs it more than once, which is how the duplicate guard is observed.
local function path_after(starting_path, calls)
  local spec = mason_spec()
  local init = assert(spec.init, "the mason.nvim spec has no `init`")
  local saved = vim.env
  vim.env = { PATH = starting_path }
  local ok, err = pcall(function()
    for _ = 1, (calls or 1) do
      init()
    end
  end)
  local result = vim.env.PATH
  vim.env = saved
  assert(ok, err)
  return result
end

local function entries(path)
  return vim.split(path, ":", { plain = true })
end

local function occurrences(path, dir)
  local count = 0
  for _, entry in ipairs(entries(path)) do
    if entry == dir then
      count = count + 1
    end
  end
  return count
end

return {
  ["puts Mason's bin first on PATH at startup"] = function()
    local path = path_after("/usr/bin:/bin")
    assert(entries(path)[1] == mason_bin, ("expected %s first on PATH, got %s"):format(mason_bin, path))
  end,

  ["keeps the rest of PATH, in order, behind it"] = function()
    local path = path_after("/usr/bin:/bin:/opt/homebrew/bin")
    assert(path == mason_bin .. ":/usr/bin:/bin:/opt/homebrew/bin", "PATH tail was not preserved: " .. path)
  end,

  ["adds no second copy when the directory is already on PATH"] = function()
    local path = path_after(mason_bin .. ":/usr/bin")
    assert(
      occurrences(path, mason_bin) == 1,
      ("expected one %s on PATH, got %d in %s"):format(mason_bin, occurrences(path, mason_bin), path)
    )
  end,

  ["is idempotent across repeated calls"] = function()
    local path = path_after("/usr/bin:/bin", 3)
    assert(
      occurrences(path, mason_bin) == 1,
      ("three calls left %d copies of %s: %s"):format(occurrences(path, mason_bin), mason_bin, path)
    )
  end,

  ["sets PATH to the bin directory when the environment carries none"] = function()
    assert(path_after(nil) == mason_bin, "an absent PATH should become exactly the bin directory")
  end,

  ["moves the directory to the front when something already sits ahead of it"] = function()
    -- Present is not the same as first. With Homebrew ahead of it, `shfmt` and `stylua`
    -- resolve to Homebrew's copies rather than Mason's, which is what an eager
    -- `mason.setup()` used to prevent.
    local path = path_after("/opt/homebrew/bin:" .. mason_bin .. ":/usr/bin:/bin")
    assert(
      path == mason_bin .. ":/opt/homebrew/bin:/usr/bin:/bin",
      "the directory should move to the front with the rest of PATH in order, got " .. path
    )
  end,
}
