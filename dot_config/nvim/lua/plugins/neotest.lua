-- neotest: one test-runner surface over per-language adapters, under the `<leader>t` group.
-- Adapters ship only once proven against a real project; the PR body records which were proven
-- and which were held back, with the reason.
--
-- The per-language adapters below load on their own filetype rather than at startup, and neotest
-- requires whichever are still unloaded when it first runs.
local javascript_filetypes = {
  "javascript",
  "javascriptreact",
  "typescript",
  "typescriptreact",
}

--- Whether the package.json nearest `file_path` declares `name` as a dependency.
---@param file_path string
---@param name string
---@return boolean
local function declares_dependency(file_path, name)
  local root = vim.fs.root(file_path, "package.json")
  if not root then
    return false
  end
  -- Plain `io` rather than `vim.fn.readfile`: neotest calls `is_test_file` from its own async
  -- contexts, where a Vimscript call raises E5560 and would silently answer "no dependency".
  local handle = io.open(root .. "/package.json", "r")
  if not handle then
    return false
  end
  local contents = handle:read("*a")
  handle:close()
  local ok, package_json = pcall(vim.json.decode, contents)
  if not ok or type(package_json) ~= "table" then
    return false
  end
  for _, field in ipairs({ "dependencies", "devDependencies" }) do
    if type(package_json[field]) == "table" and package_json[field][name] ~= nil then
      return true
    end
  end
  return false
end

return {
  {
    "nvim-neotest/neotest",
    commit = "27bf921498043f7ecd821d6db68d05de244bbd02",
    dependencies = {
      -- The busted and Swift adapters ship a rockspec listing nvim-nio, and lazy.nvim turns a
      -- rockspec dependency into a top-level spec, which would make nvim-nio load at startup.
      { "nvim-neotest/nvim-nio", lazy = true },
      { "nvim-neotest/neotest-python", commit = "1b56ca4ba51c6014f986d6548ee629bdc95589d1" },
      { "fredrikaverpil/neotest-golang", commit = "65b2be63c3de00e6f15c05388ebdc8d603dbd727" },
    },
    -- stylua: ignore start
    keys = {
      { "<leader>tt", function() require("neotest").run.run() end, desc = "Neotest: run nearest test" },
      { "<leader>tf", function() require("neotest").run.run(vim.fn.expand("%")) end, desc = "Neotest: run file" },
      { "<leader>ta", function() require("neotest").run.run(vim.uv.cwd()) end, desc = "Neotest: run all tests" },
      { "<leader>ts", function() require("neotest").summary.toggle() end, desc = "Neotest: toggle summary" },
      { "<leader>to", function() require("neotest").output.open({ enter = true }) end, desc = "Neotest: open output" },
      { "<leader>tS", function() require("neotest").run.stop() end, desc = "Neotest: stop run" },
    },
    -- stylua: ignore end
    config = function()
      require("neotest").setup({
        adapters = {
          require("neotest-python"),
          require("neotest-golang"),
          -- vitest and jest each claim a file only when that runner is a project dependency,
          -- while neotest-nodejs claims every `*.test.js` it sees. Left at its default the three
          -- race for the same file and the adapter that wins a run varies between runs (measured:
          -- 1 run in 4 ran the vitest project under node:test). node:test is the fallback, so it
          -- stands down wherever one of the other two owns the project.
          require("neotest-vitest"),
          require("neotest-jest"),
          require("neotest-nodejs")({
            isTestFile = function(file_path)
              return require("neotest-nodejs.node-util").defaultIsTestFile(file_path)
                and not declares_dependency(file_path, "vitest")
                and not declares_dependency(file_path, "jest")
            end,
          }),
          require("neotest-busted"),
          require("neotest-swift-testing"),
        },
      })
    end,
  },
  { "marilari88/neotest-vitest", commit = "c3c69715da4b158069fd4262083e7219a5c14cfb", ft = javascript_filetypes },
  { "nvim-neotest/neotest-jest", commit = "0e7979d51301dfae5ef839d771bd28cf593fde3f", ft = javascript_filetypes },
  { "AkisArou/neotest-nodejs", commit = "68e558dff61f7ac630f55ab63a092c1767965386", ft = javascript_filetypes },
  { "MisanthropicBit/neotest-busted", commit = "9efddcee53d255cef8937541808eccd464772f80", ft = "lua" },
  {
    -- Codeberg, not GitHub: the GitHub repository has been an archived redirect since 2026-04-28.
    url = "https://codeberg.org/mmllr/neotest-swift-testing",
    commit = "5b2d7efea43cb0d66d97de65b9ebc7b1db4659fd",
    ft = "swift",
  },
}
