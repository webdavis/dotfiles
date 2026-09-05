-- Which directories neotest-bashunit claims as its own.
--
-- A marker alone is not enough. `.git` sits at the top of every repository in
-- existence, so claiming on the marker made this adapter attach to all of them,
-- and neotest hands a directory run to the one non-JavaScript adapter that
-- attached. A repository with no bash tests in it would then run its "all
-- tests" through bashunit and find nothing.
--
-- Real directories under `vim.fn.tempname()` rather than a stubbed file system:
-- the rule is a question about what is on disk, and a fake would only prove the
-- fake agrees with itself.

local adapter = require("neotest-bashunit")

local function tree(layout)
  local root = assert(vim.uv.fs_realpath(vim.fn.tempname()) or vim.fn.tempname())
  vim.fn.mkdir(root, "p")
  root = vim.uv.fs_realpath(root) or root
  for relative, contents in pairs(layout) do
    local path = root .. "/" .. relative
    vim.fn.mkdir(vim.fn.fnamemodify(path, ":h"), "p")
    if contents ~= false then
      local handle = assert(io.open(path, "w"), "could not write " .. path)
      handle:write(contents)
      handle:close()
    end
  end
  return root
end

return {
  ["a repository holding a bashunit test is claimed"] = function()
    local root = tree({ [".git/HEAD"] = "ref: refs/heads/main\n", ["test/unit/thing.test.sh"] = "" })
    assert(adapter.root(root) == root, "expected the repository root, got " .. tostring(adapter.root(root)))
  end,

  ["a repository holding no bashunit test is not claimed"] = function()
    local root = tree({ [".git/HEAD"] = "ref: refs/heads/main\n", ["src/index.js"] = "", ["package.json"] = "{}" })
    assert(adapter.root(root) == nil, "expected nil, got " .. tostring(adapter.root(root)))
  end,

  ["a .bashunitrc claims the directory with no test file present"] = function()
    -- The explicit opt-in. Someone who wrote the configuration file means it,
    -- and a project can carry one before its first test is written.
    local root = tree({ [".bashunitrc"] = "", ["src/index.js"] = "" })
    assert(adapter.root(root) == root, "expected the repository root, got " .. tostring(adapter.root(root)))
  end,

  ["a test file only inside a pruned directory does not claim the repository"] = function()
    -- `filter_dir` already refuses these during discovery, so a test neotest
    -- would never run must not be what attaches the adapter either.
    local root = tree({
      [".git/HEAD"] = "ref: refs/heads/main\n",
      ["node_modules/pkg/vendored.test.sh"] = "",
      ["target/debug/build.test.sh"] = "",
    })
    assert(adapter.root(root) == nil, "expected nil, got " .. tostring(adapter.root(root)))
  end,

  ["a file merely ending in .sh does not claim the repository"] = function()
    local root = tree({ [".git/HEAD"] = "ref: refs/heads/main\n", ["scripts/deploy.sh"] = "" })
    assert(adapter.root(root) == nil, "expected nil, got " .. tostring(adapter.root(root)))
  end,

  ["a directory under no marker at all is claimed by nobody"] = function()
    local root = tree({ ["thing.test.sh"] = "" })
    assert(adapter.root(root) == nil, "expected nil, got " .. tostring(adapter.root(root)))
  end,
}
