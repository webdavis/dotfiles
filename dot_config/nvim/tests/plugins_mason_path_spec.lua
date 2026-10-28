-- Treesitter needs `tree-sitter` from the Mason bin before any buffer trigger, so the mason.nvim `init` puts it on PATH.
-- `vim.env` is a plain table around each call, so the real environment stays untouched.

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
-- `calls` runs it more than once, to check the duplicate guard.
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
    -- The Mason bin goes first, so its `shfmt` and `stylua` beat the Homebrew copies.
    local path = path_after("/opt/homebrew/bin:" .. mason_bin .. ":/usr/bin:/bin")
    assert(
      path == mason_bin .. ":/opt/homebrew/bin:/usr/bin:/bin",
      "the directory should move to the front with the rest of PATH in order, got " .. path
    )
  end,

  ["keeps an empty component, which means the working directory, in place"] = function()
    -- An empty PATH entry means the working directory and must survive.
    local path = path_after("/usr/bin::/bin")
    assert(path == mason_bin .. ":/usr/bin::/bin", "the empty component was dropped or moved: " .. path)
  end,

  ["keeps a leading empty component"] = function()
    local path = path_after(":/usr/bin")
    assert(path == mason_bin .. "::/usr/bin", "the leading empty component was dropped: " .. path)
  end,

  ["keeps a trailing empty component"] = function()
    local path = path_after("/usr/bin:")
    assert(path == mason_bin .. ":/usr/bin:", "the trailing empty component was dropped: " .. path)
  end,
}
