local Snacks = require("snacks")

local todo_keywords = {
  "BUG",
  "ERROR",
  "FAILED",
  "FIX",
  "FIXIT",
  "FIXME",
  "HACK",
  "INFO",
  "ISSUE",
  "MARK",
  "NOTE",
  "OPTIM",
  "OPTIMIZE",
  "REFACTOR",
  "PASSED",
  "PERF",
  "PERFORMANCE",
  "TEST",
  "TESTING",
  "TODO",
  "WARN",
  "WARNING",
  "XXX",
}

return {
  "folke/todo-comments.nvim",
  dependencies = {
    "nvim-lua/plenary.nvim",
    "folke/trouble.nvim",
    "folke/snacks.nvim",
  },
  opts = {},
  keys = {
    {
      "<leader>xt",
      function()
        vim.cmd("Trouble todo")
      end,
      desc = "Trouble: open project todo-comments",
    },
    {
      "<leader>s#",
      function()
        Snacks.picker.todo_comments({ keywords = { "TODO" } })
      end,
      desc = "Snacks: search TODO comments",
    },
    {
      "<leader>sK",
      function()
        Snacks.picker.todo_comments({ keywords = todo_keywords })
      end,
      desc = "Snacks: search all todo-comments keywords",
    },
    {
      "[T",
      function()
        require("todo-comments").jump_prev({ keywords = todo_keywords })
      end,
      desc = "Jump to previous todo-comments keyword",
    },
    {
      "]T",
      function()
        require("todo-comments").jump_next({ keywords = todo_keywords })
      end,
      desc = "Jump to next todo-comments keyword",
    },
  },
  config = function()
    local todo_comments = require("todo-comments")
    todo_comments.setup({
      keywords = {
        MARK = {
          icon = "🅜",
          color = "test",
          alt = {},
        },
        REFACTOR = {
          icon = "🔧",
          color = "default",
          alt = {},
        },
      },
      search = {
        command = "rg",
        args = {
          "--color=never",
          "--no-heading",
          "--with-filename",
          "--line-number",
          "--column",
          "--no-ignore-vcs",
        },
      },
    })

    -- map({
    --   mode = "n",
    --   lhs = "]k",
    --   rhs = function()
    --     todo_comments.jump_next({ keywords = todo_keywords })
    --   end,
    --   desc = "Jump to next todo-comments.keyword",
    --   expr = true,
    -- })
    --
    -- map({
    --   mode = "n",
    --   lhs = "[k",
    --   rhs = function()
    --     todo_comments.jump_prev({ keywords = todo_keywords })
    --   end,
    --   desc = "Jump to previous todo-comments.keyword",
    --   expr = true,
    -- })
  end,
}
