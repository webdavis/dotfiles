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
      -- vitest and jest both claim `*.test.js` in a project that declares both, and neotest
      -- walks its adapter map with `pairs`, so leaving both eligible makes the adapter that runs
      -- a file vary between runs. jest stands down wherever vitest claims: a repository carrying
      -- both runners is a migration TO vitest, and its config is the one new test files are
      -- written against. jest is asked through ITS OWN default predicate rather than a second,
      -- weaker copy of its dependency detection here, because the pinned adapters also consult
      -- the working directory and the git root and a local reimplementation disagrees with them.
      local vitest = require("neotest-vitest")
      local jest_claims = require("neotest-jest.jest-util").defaultIsTestFile
      local jest = require("neotest-jest")({
        isTestFile = function(file_path)
          -- No nil guard of its own: neotest types `file_path` as optional and both pinned
          -- predicates answer false for nil rather than raising, so the composed one answers
          -- too.
          return jest_claims(file_path) and not vitest.is_test_file(file_path)
        end,
      })

      -- Construction audit at these pins. Only neotest-golang REQUIRES the call: its
      -- `M.Adapter.options` is assigned inside `__call` alone (init.lua:241) and read by
      -- `filter_dir` (init.lua:49), so the bare module raises on any Go module with a
      -- subdirectory. The rest carry their options without it: neotest-python IS a constructed
      -- adapter at load (init.lua:70), vitest and jest assign their defaults at load and let
      -- `__call` override only what the caller supplies, busted's config module starts at its
      -- own defaults (config.lua:17), and the Swift adapter's `__call` only sets a log level.
      require("neotest").setup({
        adapters = {
          require("neotest-python"),
          require("neotest-golang")({}),
          vitest,
          jest,
          require("neotest-busted"),
          require("neotest-swift-testing"),
        },
      })
    end,
  },
  { "marilari88/neotest-vitest", commit = "c3c69715da4b158069fd4262083e7219a5c14cfb", ft = javascript_filetypes },
  { "nvim-neotest/neotest-jest", commit = "0e7979d51301dfae5ef839d771bd28cf593fde3f", ft = javascript_filetypes },
  { "MisanthropicBit/neotest-busted", commit = "9efddcee53d255cef8937541808eccd464772f80", ft = "lua" },
  {
    -- Codeberg, not GitHub: the GitHub repository has been an archived redirect since 2026-04-28.
    url = "https://codeberg.org/mmllr/neotest-swift-testing",
    commit = "5b2d7efea43cb0d66d97de65b9ebc7b1db4659fd",
    ft = "swift",
  },
}
