-- A second GitHub client beside octo, on `<leader>gt`.
-- Pinned, because upstream warns of breaking changes.
-- Read the new tag's README before moving the pin.
return {
  "emrearmagan/atlas.nvim",
  commit = "5aa47cd33400a137bf59df84e088a954e1add3b0",
  dependencies = { "nvim-tree/nvim-web-devicons" },
  cmd = { "Atlas", "AtlasDiff" },
  keys = {
    { "<leader>gtt", "<cmd>Atlas<cr>", desc = "Atlas: pick a command" },
    { "<leader>gtp", "<cmd>Atlas pulls github<cr>", desc = "Atlas: list pull requests" },
    { "<leader>gtP", "<cmd>Atlas create pr<cr>", desc = "Atlas: create pull request" },
    { "<leader>gti", "<cmd>Atlas issues github<cr>", desc = "Atlas: list issues" },
    { "<leader>gtI", "<cmd>Atlas create issue<cr>", desc = "Atlas: create issue" },
    { "<leader>gtr", "<cmd>Atlas review<cr>", desc = "Atlas: review pull request" },
    { "<leader>gtn", "<cmd>Atlas notes<cr>", desc = "Atlas: local review notes" },
    { "<leader>gts", "<cmd>Atlas search<cr>", desc = "Atlas: search providers" },
  },
  ---@type AtlasConfig
  opts = {
    ui = {
      picker = "snacks",
    },
    -- Atlas offers only the providers listed here, and reads the GitHub credential from the `gh` CLI.
    providers = {
      github = {},
    },
    pulls = {
      -- ~/.gitconfig rewrites GitHub https URLs to ssh.
      git_transport = "ssh",
      -- Keep local review notes after an approve or a merge.
      delete_notes = false,
      diff = {
        -- The codediff and diffview integrations reach into atlas internals
        -- and break on upstream changes.
        open_cmd = "AtlasDiff",
      },
      -- Atlas stays read and review only. Setting `repo_config.paths` would let the dashboard's `gc`
      -- switch a branch with one unconfirmed key.
    },
  },
}
