-- atlas.nvim: a second GitHub client, installed BESIDE octo rather than in place of it. The evaluation
-- at docs/research/2026-09-atlas-nvim-evaluation.md returned DO NOT ADOPT for a swap; the operator
-- overruled that for a side-by-side trial on 2026-09-05. Octo keeps `<leader>gh` exactly as it is and
-- atlas takes `<leader>gt` (git -> aTlas: `a`, `A`, `l` and `s` are all taken under `<leader>g`).
--
-- Pinned to tag 0.7.3, released 2026-08-26. The README opens with "Still in early development, will
-- have breaking changes!" and the tag history runs 0.6.6 to 0.7.3 inside three days, so everything
-- below is written against 0.7.3 in particular. Re-read that tag's README before moving the pin.
return {
  "emrearmagan/atlas.nvim",
  commit = "5aa47cd33400a137bf59df84e088a954e1add3b0",
  -- Devicons is the one dependency taken. render-markdown.nvim is the README's other recommendation
  -- and is skipped on purpose: markview.nvim already renders markdown in this config and a second
  -- renderer would fight it. codediff.nvim and diffview.nvim are skipped with the diff decision below.
  dependencies = { "nvim-tree/nvim-web-devicons" },
  -- Nothing loads at startup. `Atlas` and `AtlasDiff` are the only commands the plugin registers, and
  -- it registers them from `setup()`, so lazy's stubs are what stand in until one of these fires.
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
      -- "auto" resolves to snacks here anyway; naming it drops the detection step, as octo does.
      picker = "snacks",
    },
    -- GitHub and nothing else. Atlas reads its GitHub credential from the `gh` CLI, so no token is
    -- stored here or pulled from the vault. A provider is offered to `:Atlas` only when it has an entry
    -- in this table, so leaving GitLab, Bitbucket and Jira out is what keeps them off the dashboards;
    -- each of the three also hard-requires a token, and no account for any of them was found on this
    -- machine (evaluation 4.6).
    providers = {
      github = {},
    },
    pulls = {
      -- Every remote here is ssh and ~/.gitconfig rewrites https://github.com/ to git@github.com:, so
      -- the "https" default would be rewritten on the way out. Say ssh once instead.
      git_transport = "ssh",
      -- Local review notes outlive an approve or a merge. They never reach the forge: `<leader>n` in a
      -- diff writes one, `<leader>gtn` lists them, and bin/atlas-notes drives the same store from a
      -- script. This is the one atlas review path that posts nothing to GitHub.
      delete_notes = false,
      diff = {
        -- Native AtlasDiff, chosen rather than defaulted into. The alternatives are codediff.nvim and
        -- diffview.nvim, neither installed here, and the README records that both integrations reach
        -- into atlas internals and break on upstream changes. Native is the only one that adds no
        -- dependency and no breakage path at once.
        open_cmd = "AtlasDiff",
      },
      repo_config = {
        -- Atlas will not check a pull request branch out (`gc`) without this mapping, and reads it for
        -- diffs against a repository other than the current one. The workspace half of a key is literal
        -- and the `*` count has to match on both sides of the arrow. One line covers every clone under
        -- ~/workspaces/Ivy/webdavis. A repository kept somewhere else gets its own exact key,
        -- ["webdavis/name"] = "/path/to/that/clone", which beats the wildcard.
        --
        -- The worktrees under ~/.herdr/worktrees/dotfiles are deliberately not mapped. That directory
        -- is a plain container rather than a git repository, and each directory inside it already has
        -- a branch checked out, so neither is somewhere atlas can put a pull request branch.
        paths = {
          ["webdavis/*"] = "~/workspaces/Ivy/webdavis/*",
        },
      },
    },
  },
}
