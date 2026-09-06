return {
  "folke/which-key.nvim",
  event = "VeryLazy",
  opts_extend = { "spec" },
  opts = {
    preset = "helix",
    defaults = {},
    spec = {
      {
        mode = { "n", "v" },
        { "<C-g>", group = "git operations" },
        { "<C-g>b", group = "branch" },
        { "<C-g>B", group = "blame" },
        { "<C-g>c", group = "commit" },
        { "<C-g>C", group = "checkout" },
        { "<C-g>d", group = "diff" },
        { "<C-g>dh", group = "HEAD (latest commit)" },
        { "<C-g>di", group = "index (staging)" },
        { "<C-g>F", group = "fetch／pull" },
        { "<C-g>l", group = "log" },
        { "<C-g>ls", group = "since origin/main" },
        { "<C-g>o", group = "browse" },
        { "<C-g>p", group = "push" },
        { "<C-g>r", group = "remote" },
        { "<C-g>S", group = "stash" },
        { "<C-g>s", group = "status" },
        { "<C-g>w", group = "whatchanged" },
        { "<leader>/", group = "grep" },
        { "<leader>0", group = "quit" },
        { "<leader>a", group = "aerial" },
        { "<leader>A", group = "herdr" },
        { "<leader>c", group = "format／snapshot" },
        { "<leader>C", group = "claude" },
        { "<leader>d", group = "docker" },
        { "<leader>D", group = "debug" },
        { "<leader>Dp", group = "profiler" },
        { "<leader>e", group = "file" },
        { "<leader>f", group = "find" },
        { "<leader>g", group = "git" },
        { "<leader>gh", group = "GitHub" },
        { "<leader>ghf", group = "find (pickers)" },
        { "<leader>gt", group = "atlas" },
        { "<leader>h", group = "harpoon" },
        { "<leader>j", group = "split／join" },
        { "<leader>L", group = "lazy" },
        { "<leader>l", group = "LSP" },
        { "<leader>n", group = "notifications／messages" },
        { "<leader>o", group = "overseer" },
        { "<leader>r", group = "rename／find-and-replace" },
        { "<leader>R", group = "rest (kulala)" },
        { "<leader>s", group = "search" },
        { "<leader>t", group = "test" },
        { "<leader>u", group = "toggle" },
        { "<leader>U", group = "urlview" },
        { "<leader>x", group = "xcode" },
        { "<leader>X", group = "diagnostics／quickfix" },
        { "<leader>y", group = "yank" },
        { "[", group = "prev" },
        { "]", group = "next" },
        { "gc", group = "comments" },
        { "gr", group = "LSP (built-in)" },
        { "gx", desc = "Open with system app" },
        { "z", group = "fold／spelling" },
        {
          "<leader>b",
          group = "buffer",
          expand = function()
            return require("which-key.extras").expand.buf()
          end,
        },
        {
          "<leader>w",
          group = "windows",
          proxy = "<c-w>",
          expand = function()
            return require("which-key.extras").expand.win()
          end,
        },
      },
    },
  },
  keys = {
    {
      "<leader>b?",
      function()
        require("which-key").show({ global = false })
      end,
      desc = "Buffer Local Keymaps (which-key)",
    },
    {
      "<c-w><space>",
      function()
        require("which-key").show({ keys = "<c-w>", loop = true })
      end,
      desc = "Window Hydra Mode (which-key)",
    },
  },
  config = function(_, opts)
    local which_key = require("which-key")
    which_key.setup(opts)
  end,
}
