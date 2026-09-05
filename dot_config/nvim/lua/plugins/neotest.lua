-- neotest: one test-runner surface over per-language adapters, under the `<leader>t` group.
-- Adapters ship only once proven against a real project; the PR body records which were proven
-- and which were held back, with the reason.
return {
  "nvim-neotest/neotest",
  commit = "27bf921498043f7ecd821d6db68d05de244bbd02",
  dependencies = {
    "nvim-neotest/nvim-nio",
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
      },
    })
  end,
}
