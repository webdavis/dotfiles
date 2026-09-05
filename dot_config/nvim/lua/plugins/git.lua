-- ╭─────────────╮
-- │   Modules   │
-- ╰─────────────╯
local git = require("custom_api.git")
local github = require("custom_api.github")
local overseer = require("custom_api.overseer")
local try = require("custom_api.try")
local util = require("custom_api.util")

-- ╭─────────────╮
-- │   Helpers   │
-- ╰─────────────╯
local log_info = vim.log.levels.INFO
local log_warning = vim.log.levels.WARN
local log_error = vim.log.levels.ERROR
local notify_fugitive_title = { title = "Fugitive" }
local notify_github_title = { title = "GitHub" }

-- `github.account` reports a `gh` that cannot answer as `nil, message`
-- (spec 6.1), so reading a field straight off the call raises on the index.
-- `try` is the one place that message becomes a notification.
local function account_or_notify()
  return try(function()
    return github.account()
  end, { label = "github.account" })
end

-- The same for `github.repo`, plus the field every caller here wants. `name` is
-- nil when the answer carried no slash, and `git.latest_commit` raises on a
-- missing `repo_name`, so that case is answered once rather than at every site.
local function repo_name_or_notify()
  local repository = try(function()
    return github.repo()
  end, { label = "github.repo" })
  if not repository then
    return
  end

  if not repository.name then
    local message = ("No repository name in the answer `gh` gave: *%s*"):format(repository.nameWithOwner)
    vim.notify(message, log_warning, notify_github_title)
    return
  end

  return repository.name
end

local function copy_url_mapping_helper(lhs, remote, protocol)
  local mapping_table = {
    mode = "n",
    lhs = lhs,
    rhs = function()
      if git.initialized() then
        local account = account_or_notify()
        if not account then
          return
        end

        local repo_name = repo_name_or_notify()
        if not repo_name then
          return
        end

        local url = git.url({
          remote = remote,
          account_name = account.username,
          repo_name = repo_name,
        })

        if not url then
          vim.notify("Warning: Nothing copied to clipboard!", log_warning, { title = "git" })
          return
        end

        local message = git.copy_url_to_clipboard({
          url = url,
          remote = remote,
          protocol = protocol,
        })

        if message then
          vim.notify(message, log_info, { title = "git" })
        end
      end
    end,
    desc = "Git (remote): copy " .. protocol:upper() .. " URL (" .. remote .. ")",
  }

  return mapping_table
end

-- ╭──────────────────────────╮
-- │   Fugitive lazy trigger  │
-- ╰──────────────────────────╯
-- Every fugitive mapping is created inside the spec's `config`, which lazy.nvim
-- only runs once the plugin loads, so the trigger has to name each `<C-g>` key
-- up front or the first press of one does nothing at all. `desc` mirrors the
-- matching `map()` call below so the which-key popup reads the same before and
-- after fugitive loads. Sorted by `lhs`; the four other `<C-g>` keys belong to
-- the snacks and gitlinker specs and are not fugitive's to trigger.
local fugitive_keys = {
  { "<C-g>!", desc = "Fugitive: stage → amend (no edit) → force push" },
  { "<C-g>C-", desc = "Fugitive (checkout): previous branch" },
  { "<C-g>Cb", desc = "Git (checkout): create new <branch>" },
  { "<C-g>Cm", desc = "Fugitive (checkout): main" },
  { "<C-g>Ff", desc = "Fugitive: fetch" },
  { "<C-g>Fp", desc = "Fugitive: pull" },
  { "<C-g>Fr", desc = "Fugitive: pull --rebase" },
  { "<C-g>SA", desc = "Apply: by index <#>" },
  { "<C-g>SP", desc = "Pop: by index <#>" },
  { "<C-g>SW", desc = "Push: working + untracked (keep staged changes)" },
  { "<C-g>Sa", desc = "Apply: most recent (default)" },
  { "<C-g>Sd", desc = "Push: tracked + untracked (default)" },
  { "<C-g>Se", desc = "Push: all (tracked + untracked + ignored)" },
  { "<C-g>Sp", desc = "Pop: most recent (default)" },
  { "<C-g>Ss", desc = "Push: staged" },
  { "<C-g>Sw", desc = "Push: working (keep staged changes)" },
  { "<C-g>a", desc = "Fugitive: add file" },
  { "<C-g>bA", desc = "Fugitive: local + remote (verbose)" },
  { "<C-g>bR", desc = "Fugitive: remotes (verbose)" },
  { "<C-g>bV", desc = "Fugitive: local (verbose)" },
  { "<C-g>bb", desc = "Fugitive: local" },
  { "<C-g>bc", desc = "Notify: current + copy hash to clipboard (verbose)" },
  { "<C-g>bv", desc = "Notify: local + info" },
  { "<C-g>cA", desc = "Fugitive: amend latest (author only)" },
  { "<C-g>ca", desc = "Fugitive: amend latest (edit message)" },
  { "<C-g>cc", desc = "Fugitive: entire index (all staged changes)" },
  { "<C-g>cf", desc = "Fugitive: current file only" },
  { "<C-g>cn", desc = "Fugitive: amend latest (don't edit message)" },
  { "<C-g>cp", desc = "Fugitive: paste latest message into buffer" },
  { "<C-g>dhF", desc = "Fugitive: with function context (horizontal)" },
  { "<C-g>dhf", desc = "Fugitive: with function context (vertical)" },
  { "<C-g>dhm", desc = "Overseer: emphasize moved lines" },
  { "<C-g>dhw", desc = "Overseer: emphasize changed words" },
  { "<C-g>diC", desc = "Git: no context (current file)" },
  { "<C-g>diF", desc = "Fugitive: function context (horizontal)" },
  { "<C-g>dic", desc = "Git: no context" },
  { "<C-g>dif", desc = "Fugitive: function context (vertical)" },
  { "<C-g>i", desc = "Git (Overseer): initialize & create GitHub repo" },
  { "<C-g>lL", desc = "Fugitive: default (current file)" },
  { "<C-g>lO", desc = "Fugitive: oneline (current file)" },
  { "<C-g>lW", desc = "Overseer: my contributions this-week (color)" },
  { "<C-g>lc", desc = "Fugitive: oneline (current file) (alt)" },
  { "<C-g>ll", desc = "Fugitive: default" },
  { "<C-g>lo", desc = "Fugitive: oneline" },
  { "<C-g>lp", desc = "Fugitive: pretty (enter number of commits)" },
  { "<C-g>lr", desc = "Overseer: pretty (relative time)" },
  { "<C-g>lsl", desc = "Fugitive: default" },
  { "<C-g>lso", desc = "Fugitive: oneline" },
  { "<C-g>lsr", desc = "Overseer: pretty (relative time)" },
  { "<C-g>lw", desc = "Fugitive: my contributions this-week (no color)" },
  { "<C-g>oI", desc = "Open GitHub: Insights" },
  { "<C-g>oP", desc = "Open GitHub: Projects" },
  { "<C-g>oS", desc = "Open GitHub: Security" },
  { "<C-g>oa", desc = "Open GitHub: Actions" },
  { "<C-g>ob", desc = "Open GitHub: Current Branch (Homepage)" },
  { "<C-g>of", desc = "Fugitive: browse (file)" },
  { "<C-g>oi", desc = "Open GitHub: Issues" },
  { "<C-g>ol", desc = "Fugitive: browse (line in file)" },
  { "<C-g>oo", desc = "Open GitHub: Homepage" },
  { "<C-g>op", desc = "Open GitHub: Pull Requests" },
  { "<C-g>os", desc = "Open GitHub: Settings" },
  { "<C-g>ow", desc = "Open GitHub: Wiki" },
  { "<C-g>pf", desc = "Fugitive: push --force-with-lease" },
  { "<C-g>pp", desc = "Fugitive: push" },
  { "<C-g>pu", desc = "Fugitive: push -u origin <branch>" },
  { "<C-g>rH", desc = "Git (remote): copy HTTPS URL (upstream)" },
  { "<C-g>rS", desc = "Git (remote): copy SSH URL (upstream)" },
  { "<C-g>rh", desc = "Git (remote): copy HTTPS URL (origin)" },
  { "<C-g>rs", desc = "Git (remote): copy SSH URL (origin)" },
  { "<C-g>sn", desc = "Fugitive: status (as notification)" },
  { "<C-g>ss", desc = "Fugitive: status" },
  { "<C-g>wb", desc = "Fugitive: whatchanged (buffer)" },
  { "<C-g>wc", desc = "Fugitive: whatchanged --since=<date>" },
  { "<C-g>ww", desc = "Fugitive: whatchanged (workspace)" },
}

-- ╭─────────────╮
-- │   Plugins   │
-- ╰─────────────╯
return {
  {
    "lewis6991/gitsigns.nvim",
    config = function()
      local gitsigns = require("gitsigns")

      local opts = {
        signs = {
          add = { text = "▎" },
          change = { text = "▎" },
          delete = { text = "" },
          topdelete = { text = "" },
          changedelete = { text = "▎" },
          untracked = { text = "▎" },
        },
        signs_staged = {
          add = { text = "▎" },
          change = { text = "▎" },
          delete = { text = "" },
          topdelete = { text = "" },
          changedelete = { text = "▎" },
        },
        signs_staged_enable = true,
        signcolumn = true,
        numhl = true,
        current_line_blame = true,
        on_attach = function(bufnr)
          -- `bufnr` comes from `on_attach` and ensures the mapping only works in this buffer.

          -- Navigation mappings: move between Git hunks.
          -- ——————————————————————————————————————————————
          map({
            mode = "n",
            lhs = "]g",
            rhs = function()
              if vim.wo.diff then
                vim.cmd.normal({ "]c", bang = true })
              else
                ---@diagnostic disable-next-line: param-type-mismatch
                gitsigns.nav_hunk("next", { target = "all" })
              end
            end,
            desc = "Gitsigns: go to next hunk",
            buffer = bufnr,
          })

          map({
            mode = "n",
            lhs = "[g",
            rhs = function()
              if vim.wo.diff then
                vim.cmd.normal({ "[c", bang = true })
              else
                ---@diagnostic disable-next-line: param-type-mismatch
                gitsigns.nav_hunk("prev", { target = "all" })
              end
            end,
            desc = "Gitsigns: go to previous hunk",
            buffer = bufnr,
          })

          -- Action mappings: stage, reset, undo, preview, diff, blame, and show commit.
          -- —————————————————————————————————————————————————————————————————————————————
          map({
            mode = "n",
            lhs = "<leader>ga",
            rhs = gitsigns.stage_hunk,
            desc = "Gitsigns: Stage Hunk",
            buffer = bufnr,
          })

          map({
            mode = "v",
            lhs = "<leader>ga",
            rhs = function()
              gitsigns.stage_hunk({ vim.fn.line("."), vim.fn.line("v") })
            end,
            desc = "Gitsigns: Stage Hunk",
            buffer = bufnr,
          })

          map({
            mode = "n",
            lhs = "<leader>gA",
            rhs = gitsigns.stage_buffer,
            desc = "Gitsigns: Stage Entire Buffer",
            buffer = bufnr,
          })

          map({
            mode = "n",
            lhs = "<leader>gr",
            rhs = gitsigns.reset_hunk,
            desc = "Gitsigns: Reset Hunk",
            buffer = bufnr,
          })

          map({
            mode = "v",
            lhs = "<leader>gr",
            rhs = function()
              gitsigns.reset_hunk({ vim.fn.line("."), vim.fn.line("v") })
            end,
            desc = "Gitsigns: Reset Hunk",
            buffer = bufnr,
          })

          map({
            mode = "n",
            lhs = "<leader>gR",
            rhs = gitsigns.reset_buffer,
            desc = "Gitsigns: Reset Buffer",
            buffer = bufnr,
          })

          -- undo_stage_hunk is depcrated.
          -- Ref: https://github.com/lewis6991/gitsigns.nvim/issues/1180
          map({
            mode = "n",
            lhs = "<leader>gu",
            ---@diagnostic disable-next-line: deprecated
            rhs = gitsigns.undo_stage_hunk,
            desc = "Gitsigns: Undo Staged Hunk",
            buffer = bufnr,
          })

          map({
            mode = "n",
            lhs = "<leader>gp",
            rhs = gitsigns.preview_hunk,
            desc = "Gitsigns: Preview Hunk",
            buffer = bufnr,
          })

          map({
            mode = "n",
            lhs = "<leader>gi",
            rhs = gitsigns.preview_hunk_inline,
            desc = "Gitsigns: Preview Hunk (inline)",
            buffer = bufnr,
          })

          map({
            mode = "n",
            lhs = "<leader>gB",
            rhs = function()
              gitsigns.blame_line({ full = true })
            end,
            desc = "Gitsigns: blame line (full)",
            buffer = bufnr,
          })

          map({
            mode = "n",
            lhs = "<leader>gc",
            rhs = function()
              gitsigns.show_commit()
            end,
            desc = "Gitsigns: Show Commit",
            buffer = bufnr,
          })

          -- Diff (HEAD / latest commit):
          map({
            mode = "n",
            lhs = "<C-g>dhd",
            rhs = function()
              ---@diagnostic disable-next-line: param-type-mismatch
              gitsigns.diffthis("~1")
            end,
            desc = "Gitsigns: side-by-side",
            buffer = bufnr,
          })

          -- Diff (index / staging):
          map({
            mode = "n",
            lhs = "<C-g>did",
            rhs = gitsigns.diffthis,
            desc = "Gitsigns: side-by-side",
            buffer = bufnr,
          })

          -- Blame mappings: the `<C-g>B` group (spec 5.2). Buffer-local so they
          -- exist only where gitsigns attached, which is what the old git-blame
          -- TODO asked for.
          -- ————————————————————————————————————————————————————————————————————
          local function blame_sha_at_cursor()
            return try(function()
              return git.blame_sha({ file = vim.api.nvim_buf_get_name(bufnr), line = vim.fn.line(".") })
            end, { label = "git.blame_sha" })
          end

          local function commit_url_at_cursor()
            local sha = blame_sha_at_cursor()
            if not sha then
              return
            end

            return try(function()
              return github.commit_url(sha)
            end, { label = "github.commit_url" })
          end

          map({
            mode = "n",
            lhs = "<C-g>Bt",
            rhs = gitsigns.toggle_current_line_blame,
            desc = "Gitsigns: toggle current line blame",
            buffer = bufnr,
          })

          map({
            mode = "n",
            lhs = "<C-g>By",
            rhs = function()
              local sha = blame_sha_at_cursor()
              if not sha then
                return
              end

              util.copy_to_system_clipboard(sha)
              vim.notify(("Copied commit SHA to clipboard: `%s`"):format(sha), log_info, { title = "Git Blame" })
            end,
            desc = "Git Blame: copy commit SHA",
            buffer = bufnr,
          })

          map({
            mode = "n",
            lhs = "<C-g>Bo",
            rhs = function()
              local url = commit_url_at_cursor()
              if not url then
                return
              end

              vim.ui.open(url)
            end,
            desc = "Git Blame: open commit URL",
            buffer = bufnr,
          })

          map({
            mode = "n",
            lhs = "<C-g>BO",
            rhs = function()
              local url = commit_url_at_cursor()
              if not url then
                return
              end

              util.copy_to_system_clipboard(url)
              vim.notify(("Copied commit URL to clipboard: `%s`"):format(url), log_info, { title = "Git Blame" })
            end,
            desc = "Git Blame: copy commit URL",
            buffer = bufnr,
          })

          -- Text-object mapping: select hunk.
          -- ———————————————————————————————————
          map({
            mode = { "o", "x" },
            lhs = "ih",
            rhs = gitsigns.select_hunk,
            desc = "Gitsigns: Change in Hunk (text object)",
            buffer = bufnr,
          })
        end,
      }

      require("gitsigns").setup(opts)

      -- `<leader>gm` outlived git-messenger.vim and keeps its reach: global, not
      -- buffer-local like the `<leader>gB` blame above.
      map({
        mode = "n",
        lhs = "<leader>gm",
        rhs = function()
          gitsigns.blame_line({ full = true })
        end,
        desc = "Gitsigns: blame line (full)",
      })

      -- The `<rev>:<path>` half of a blame name, of which only the path is
      -- wanted. The revision may carry a colon of its own, since gitsigns names
      -- the index `:0`, and so may the path, so the format is pinned rather than
      -- searched: an optional leading colon, a revision with no colon in it, the
      -- colon that closes it, and the whole of the rest.
      local function path_after_revision(text)
        return text:match("^:?[^:]+:(.*)")
      end

      -- The pinned gitsigns opens the split BEFORE it names the blame buffer, so
      -- a second press while this file's blame is still open leaves the new split
      -- behind and then fails on `nvim_buf_set_name` with E95, taking the window
      -- count from two to three. The blame buffer is named
      -- `gitsigns-blame://<gitdir>//<rev>:<path relative to the repo root>`, and
      -- BOTH ends of that identify the file: the gitdir, because two repositories
      -- holding the same relative path produce two different buffers, and the
      -- whole relative path behind its `:`, compared for EQUALITY. A tail match
      -- reads a blame of `a:x.lua` as one of `x.lua`, because a colon is legal in
      -- a filename and the blame name for the one ends with the blame name for
      -- the other. Gitsigns publishes both halves on the source buffer as
      -- `b:gitsigns_status_dict`.
      --
      -- The revision between them is deliberately NOT matched: a blame the reader
      -- already walked with `R` sits at another revision and is still the window
      -- to focus rather than to duplicate.
      --
      -- EVERY window, not just this tab's. A buffer name is global, so a blame
      -- open in another tab collides just the same, and searching one tab missed
      -- it and left the split plus the E95 behind. `nvim_set_current_win` switches
      -- tabs, so focusing it from here works.
      -- The current buffer's path relative to the repository root, which is what
      -- the blame name carries. A Fugitive revision buffer is named
      -- `fugitive://<gitdir>//<rev>/<path>`, so it never begins with the working
      -- tree, and a BARE repository has no working tree for it to begin with:
      -- gitsigns still attaches and still names its blame after the same
      -- relative path, so the root test alone answered nothing there and every
      -- press opened another blame. Fugitive's own `FugitiveParse` is the
      -- inverse of the URL scheme it wrote, and only Fugitive produces such a
      -- name, so nothing else reaches it.
      local function relative_path_for_current_buffer(root)
        local file = vim.api.nvim_buf_get_name(0)

        if vim.startswith(file, "fugitive://") then
          return path_after_revision(vim.fn.FugitiveParse(file)[1])
        end

        -- A repository rooted at `/` already ends in the separator, and `//` is
        -- a prefix of no absolute path at all.
        local prefix = (root:gsub("/+$", "")) .. "/"
        if vim.startswith(file, prefix) then
          return file:sub(#prefix + 1)
        end
      end

      local function blame_window_for_current_buffer()
        local status = vim.b.gitsigns_status_dict
        if not (status and status.gitdir and status.root) then
          return
        end

        local relative_path = relative_path_for_current_buffer(status.root)
        if not relative_path then
          return
        end

        local prefix = ("gitsigns-blame://%s//"):format(status.gitdir)

        for _, window in ipairs(vim.api.nvim_list_wins()) do
          local name = vim.api.nvim_buf_get_name(vim.api.nvim_win_get_buf(window))
          if vim.startswith(name, prefix) and path_after_revision(name:sub(#prefix + 1)) == relative_path then
            return window
          end
        end
      end

      -- git-messenger's `o`/`O` walked back through a line's older commits from
      -- inside its popup, which is why it set `into_popup_after_show`. A
      -- `blame_line` float is static, so the walk lives here instead: a
      -- scroll-bound blame split whose `R` reblames at the PARENT of the commit
      -- under the cursor, which is the same traversal without the popup. `r`
      -- reblames at the commit itself, so it steps in once and then stops:
      -- gitsigns drops a reblame whose sha equals the active revision, and only
      -- the `R` arm appends the `^` that reaches the parent (`actions/blame.lua`
      -- at the pinned 130beacf).
      map({
        mode = "n",
        lhs = "<leader>gM",
        rhs = function()
          local blame_window = blame_window_for_current_buffer()
          if blame_window then
            vim.api.nvim_set_current_win(blame_window)
            return
          end

          gitsigns.blame()
        end,
        desc = "Gitsigns: blame file (walk commits)",
      })
    end,
  },
  {
    "tpope/vim-fugitive",
    dependencies = {
      "tpope/vim-rhubarb",
      "stevearc/overseer.nvim",
      "folke/snacks.nvim",
    },
    -- Every global command `plugin/fugitive.vim` defines, so a lazy fugitive
    -- still answers the ones typed by hand as well as the ones the mappings
    -- below run (`Git`, `Gwrite`, `GBrowse`). The legacy spellings (`Gstatus`,
    -- `Gcommit`, `Gbrowse` and the rest) are omitted: without
    -- `g:fugitive_legacy_commands` they are error stubs, not commands.
    cmd = {
      "G",
      "GBrowse",
      "GDelete",
      "GMove",
      "GRemove",
      "GRename",
      "GUnlink",
      "Gcd",
      "Gclog",
      "GcLog",
      "Gdiffsplit",
      "Gdrop",
      "Ge",
      "Gedit",
      "Ggrep",
      "Ghdiffsplit",
      "Git",
      "Glcd",
      "Glgrep",
      "Gllog",
      "GlLog",
      "Gpedit",
      "Gr",
      "Gread",
      "Gsplit",
      "Gtabedit",
      "Gvdiffsplit",
      "Gvsplit",
      "Gw",
      "Gwq",
      "Gwrite",
    },
    keys = fugitive_keys,
    config = function()
      -- Init／Create:
      map({
        mode = "n",
        lhs = "<C-g>i",
        rhs = function()
          local is_initialized = git.initialized({ quiet = true })

          -- `github.username()` never existed on either side, so this errored
          -- every time the mapping was pressed (item 1). The username is one
          -- field of the account, and `try` is what turns a `gh` that is not
          -- logged in into a notification instead of a raised error.
          local account = try(function()
            return github.account()
          end, { label = "github.account" })
          if not account then
            return
          end
          local user = account.username

          local directory = util.get_cwd_basename()

          local github_project_prompt = "What's the name of your GitHub project (default: " .. directory .. ")? "
          vim.ui.input({ prompt = github_project_prompt }, function(project_name_input)
            local project = util.trim(project_name_input)
            if project == "" then
              project = directory
            end

            local confirmation_prompt = "Create project '" .. project .. "' on GitHub? [y]es／[n]o／[q]uit: "
            vim.ui.input({ prompt = confirmation_prompt }, function(answer)
              local confirm_creation = util.trim(answer):lower()
              local yes_values = { y = true, ye = true, yes = true, yep = true, ok = true }

              if not yes_values[confirm_creation] then
                vim.notify("Project creation aborted for project **" .. project .. "**", log_info, notify_github_title)
                return
              end

              local gh_exit, _ = util.run_shell_command({ cmd = { "gh", "repo", "view", project } })

              if gh_exit == 0 then
                local message = {
                  "Git: project creation cancelled. GitHub project **" .. project .. "** already exists.",
                  "Run `gh repo clone " .. user .. "/" .. project .. "` to download it",
                }
                vim.notify(table.concat(message, "\n\n"), log_info, notify_github_title)
                return
              end

              local cmds = {}
              if not is_initialized then
                table.insert(cmds, "git init")
              end
              -- overseer_runner joins its commands into one shell line, so the
              -- name typed at the prompt is quoted for that shell here.
              table.insert(cmds, "gh repo create --public " .. vim.fn.shellescape(project))

              overseer.overseer_runner({ cmds = cmds })
            end)
          end)
        end,
        desc = "Git (Overseer): initialize & create GitHub repo",
      })

      -- Status:
      map({ mode = "n", lhs = "<C-g>ss", rhs = "Git", desc = "Fugitive: status" })
      map({ mode = "n", lhs = "<C-g>sn", rhs = "Git status -sb", desc = "Fugitive: status (as notification)" })

      -- Staging:
      map({ mode = "n", lhs = "<C-g>a", rhs = "Gwrite", desc = "Fugitive: add file" })

      -- Stage current file → Amend last commit (no edit) → Force push:
      map({
        mode = "n",
        lhs = "<C-g>!",
        rhs = "Gwrite|Git commit --amend --no-edit|Git push --force",
        desc = "Fugitive: stage → amend (no edit) → force push",
      })

      -- Stash push:
      map({
        mode = "n",
        lhs = "<C-g>Sd",
        rhs = "Git stash --include-untracked",
        desc = "Push: tracked + untracked (default)",
      })

      map({
        mode = "n",
        lhs = "<C-g>Se",
        rhs = "Git stash --all",
        desc = "Push: all (tracked + untracked + ignored)",
      })

      map({
        mode = "n",
        lhs = "<C-g>Sw",
        rhs = "Git stash --keep-index",
        desc = "Push: working (keep staged changes)",
      })

      map({
        mode = "n",
        lhs = "<C-g>SW",
        rhs = "Git stash --keep-index --include-untracked",
        desc = "Push: working + untracked (keep staged changes)",
      })

      map({
        mode = "n",
        lhs = "<C-g>Ss",
        rhs = "Git stash --staged",
        desc = "Push: staged",
      })

      -- Stash pop:
      map({ mode = "n", lhs = "<C-g>Sp", rhs = "Git stash pop", desc = "Pop: most recent (default)" })

      map({
        mode = "n",
        lhs = "<C-g>SP",
        rhs = function()
          local index = util.trim(vim.fn.input("Stash index to pop: "))
          -- A stash index is a number, so it is checked as one before it is
          -- spliced into an ex command line. Anything else typed here would be
          -- read as Vim syntax rather than as an index.
          if index:match("^%d+$") then
            vim.cmd("Git stash pop " .. index)
          elseif index ~= "" then
            vim.notify("Not a stash index: *" .. index .. "*", log_warning, notify_fugitive_title)
          end
        end,
        desc = "Pop: by index <#>",
      })

      -- Stash apply:
      map({ mode = "n", lhs = "<C-g>Sa", rhs = "Git stash apply", desc = "Apply: most recent (default)" })

      map({
        mode = "n",
        lhs = "<C-g>SA",
        rhs = function()
          local index = util.trim(vim.fn.input("Stash index to pop: "))
          -- A stash index is a number, so it is checked as one before it is
          -- spliced into an ex command line. Anything else typed here would be
          -- read as Vim syntax rather than as an index.
          if index:match("^%d+$") then
            vim.cmd("Git stash apply " .. index)
          elseif index ~= "" then
            vim.notify("Not a stash index: *" .. index .. "*", log_warning, notify_fugitive_title)
          end
        end,
        desc = "Apply: by index <#>",
      })

      -- Remote:
      map(copy_url_mapping_helper("<C-g>rh", "origin", "https"))
      map(copy_url_mapping_helper("<C-g>rH", "upstream", "https"))
      map(copy_url_mapping_helper("<C-g>rs", "origin", "ssh"))
      map(copy_url_mapping_helper("<C-g>rS", "upstream", "ssh"))

      -- stylua: ignore start
      -- Checkout:
      map({
        mode = "n",
        lhs = "<C-g>Cb",
        rhs = function()
          local repo = repo_name_or_notify()
          if not repo then
            return
          end

          local branch = git.current_branch()

          local prompt
          if branch.name and branch.hash then
            prompt = ("%s: checkout new branch from '%s' (commit: %s): "):format(
              repo,
              branch.name,
              branch.hash
            )
          elseif branch.name then
            prompt = ("%s: checkout new branch from '%s': "):format(
              repo,
              branch.name
            )
          else
            prompt = ("%s: checkout new branch (no commit detected): "):format(repo)
          end

          vim.ui.input({ prompt = prompt }, function(new_branch)
            if not new_branch or new_branch:match("^%s*$") then
              local cancel_message = string.format(
                "Branch creation and checkout cancelled - *no branch name provided*"
                .. "\n\n---------------------------------------"
                .. "\n**Repository:** %s"
                .. "\n**Active Branch:** `%s`",
                repo,
                branch.name
              )
              vim.notify(cancel_message, log_warning, { title = "Git", timeout = 10000 })
              return
            end
            new_branch = util.trim(new_branch)

            -- Fugitive's own argv entry point. Splicing the name into a `:Git`
            -- command line instead lets it be re-read as Vim syntax: a newline
            -- runs a second ex command, `%` expands to the current file, and a
            -- leading dash becomes another git flag. FugitiveExecute passes the
            -- word through untouched, and DidChange refreshes the summary the
            -- `:Git` form would have refreshed.
            local result = vim.fn.FugitiveExecute({ "checkout", "-b", new_branch })
            vim.fn.FugitiveDidChange()

            if result.exit_status ~= 0 then
              vim.notify(
                "Checkout failed for branch *" .. new_branch .. "*\n\n" .. table.concat(result.stderr or {}, "\n"),
                log_warning,
                notify_fugitive_title
              )
            end
          end)

        end,
        desc = "Git (checkout): create new <branch>",
      })

      map({ mode = "n", lhs = "<C-g>C-", rhs = "Git checkout -", desc = "Fugitive (checkout): previous branch" })
      map({ mode = "n", lhs = "<C-g>Cm", rhs = "Git checkout main", desc = "Fugitive (checkout): main" })
      -- stylua: ignore end

      -- Branch:
      map({ mode = "n", lhs = "<C-g>bb", rhs = "Git branch", desc = "Fugitive: local" })
      map({ mode = "n", lhs = "<C-g>bV", rhs = "Git branch -vv", desc = "Fugitive: local (verbose)" })
      map({ mode = "n", lhs = "<C-g>bR", rhs = "Git branch -rv", desc = "Fugitive: remotes (verbose)" })
      map({ mode = "n", lhs = "<C-g>bA", rhs = "Git branch --all -vv", desc = "Fugitive: local + remote (verbose)" })

      local function format_section(label, text, metatext)
        if text and text:match("%S") then
          if metatext and metatext:match("%S") then
            return label and string.format("*%s* [%s] (%s)", label, text, metatext) or text
          else
            return label and string.format("*%s* [%s]", label, text) or text
          end
        elseif label then
          return label
        end
      end

      local function build_sections(branch, commit_hash, summary, body)
        local sections = {}

        if summary then
          table.insert(sections, { summary })
          if body then
            table.insert(sections, { "\n" .. body })
          end
          table.insert(sections, { "\n---------------------------------------" })
        end

        table.insert(sections, { "Branch:", "**" .. branch.name .. "**" })

        if commit_hash then
          table.insert(sections, { "Commit:", commit_hash, "copied to clipboard" })
          util.copy_to_system_clipboard(commit_hash)
        end

        if branch.upstream then
          table.insert(sections, { "Upstream:", branch.upstream })
        end

        return sections
      end

      local function sections_to_message(sections)
        local lines = {}
        for _, s in ipairs(sections) do
          local line = format_section(s[1], s[2], s[3])
          if line then
            table.insert(lines, line)
          end
        end
        return table.concat(lines, "\n")
      end

      local function show_current_git_branch()
        local branch = git.current_branch()
        if not branch then
          return
        end

        local repo_name = repo_name_or_notify()
        if not repo_name then
          return
        end

        local commit = git.latest_commit({ repo_name = repo_name }) or {}
        local sections = build_sections(branch, commit.hash, commit.summary, commit.body)
        local message = sections_to_message(sections)

        vim.notify(message, vim.log.levels.INFO, { title = "Active Git Branch & Latest Commit", timeout = 0 })
      end

      map({
        mode = "n",
        lhs = "<C-g>bc",
        rhs = show_current_git_branch,
        desc = "Notify: current + copy hash to clipboard (verbose)",
      })

      -- Helper functions for formatting:
      local function bold(text)
        return ("**%s**"):format(text)
      end
      local function italicize(text)
        return ("*%s*"):format(text)
      end
      local function inline_code(text)
        return ("`%s`"):format(text)
      end

      local function format_branch_field(label, value, opts)
        if not value then
          return nil
        end

        if label == "Hash" then
          value = inline_code(value)
        elseif label == "Branch" then
          value = opts.bold_label and bold(value) or italicize(value)
        end

        if opts.bold_label then
          label = italicize(label)
        end

        return ("%s: %s"):format(label, value)
      end

      local function format_branch(branch)
        local active = branch.status == "active"
        local opts = { bold_label = active }

        local fields = {
          { "Branch", branch.name },
          { "Hash", branch.hash },
          { "Upstream", branch.upstream },
          { "Message", branch.message },
        }

        local lines = {}
        for _, f in ipairs(fields) do
          local formatted_field = format_branch_field(f[1], f[2], opts)
          if formatted_field then
            table.insert(lines, formatted_field)
          end
        end

        return table.concat(lines, "\n")
      end

      local function group_branches(branches)
        local active, inactive = {}, {}

        for _, branch in ipairs(branches) do
          local formatted = format_branch(branch)
          if branch.status == "active" then
            table.insert(active, formatted)
          else
            table.insert(inactive, formatted)
          end
        end

        return active, inactive
      end

      local function build_notification(active, inactive)
        local lines = {}

        for _, b in ipairs(active) do
          table.insert(lines, b)
        end

        if #inactive > 0 then
          inactive[1] = "Inactive\n------------------------" .. "\n" .. inactive[1]
          for _, b in ipairs(inactive) do
            table.insert(lines, b)
          end
        end

        return table.concat(lines, "\n\n")
      end

      local function show_all_local_branches_with_info()
        local all_branches = git.all_branches()
        if not all_branches or #all_branches == 0 then
          vim.notify("No Git branches available!", log_info)
          return
        end

        local active, inactive = group_branches(all_branches)
        local text = build_notification(active, inactive)
        vim.notify(text, log_info, { title = "All Git Branches", timeout = 0 })
      end

      map({
        mode = "n",
        lhs = "<C-g>bv",
        rhs = show_all_local_branches_with_info,
        desc = "Notify: local + info",
      })

      -- stylua: ignore start
      -- Commit:
      map({ mode = "n", lhs = "<C-g>cc", rhs = "Git commit --verbose", desc = "Fugitive: entire index (all staged changes)" })
      map({ mode = "n", lhs = "<C-g>cf", rhs = "Git commit %", desc = "Fugitive: current file only" })
      map({ mode = "n", lhs = "<C-g>ca", rhs = "Git commit --amend --verbose", desc = "Fugitive: amend latest (edit message)" })
      map({ mode = "n", lhs = "<C-g>cn", rhs = "Git commit --amend --no-edit", desc = "Fugitive: amend latest (don't edit message)" })
      -- stylua: ignore end

      map({
        mode = "n",
        lhs = "<C-g>cp",
        rhs = function()
          local repo_name = repo_name_or_notify()
          if not repo_name then
            return
          end

          local commit = git.latest_commit({ repo_name = repo_name }) or {}
          if not commit.summary then
            return
          end

          vim.fn.setreg('"', commit.summary .. "\n\n" .. (commit.body or ""))
          vim.cmd("normal! ]p")
        end,
        desc = "Fugitive: paste latest message into buffer",
      })

      -- An interactive command to amend the author/email of the latest commit:
      map({
        mode = "n",
        lhs = "<C-g>cA",
        rhs = function()
          local repo_name = repo_name_or_notify()
          if not repo_name then
            return
          end

          local commit = git.latest_commit({ repo_name = repo_name }) or {}
          if not commit.hash then
            return nil
          end

          local function message_helper(subject)
            return "No " .. subject .. " entered - author update cancelled for commit `" .. commit.hash .. "`"
          end

          vim.ui.input({ prompt = "Author for latest commit (" .. commit.hash .. "): " }, function(author)
            if not author or author:match("^%s*$") then
              vim.notify(message_helper("author"), log_warning, notify_fugitive_title)
              return
            end
            author = util.trim(author)

            vim.ui.input({ prompt = "Email for " .. author .. ": " }, function(email)
              if not email or email:match("^%s*$") then
                vim.notify(message_helper("email"), log_warning, notify_fugitive_title)
                return
              end

              email = util.trim(email)
              -- One argv word, so the double quotes that used to wrap it are
              -- gone with the shell they were quoting for. An author typed with
              -- a `"` in it used to close them early and turn the rest of the
              -- name into further `git commit` flags.
              local result = vim.fn.FugitiveExecute({
                "commit",
                "-C",
                "HEAD",
                "--amend",
                "--author=" .. author .. " <" .. email .. ">",
              })
              vim.fn.FugitiveDidChange()

              if result.exit_status ~= 0 then
                vim.notify(
                  "Amend failed\n\n" .. table.concat(result.stderr or {}, "\n"),
                  log_warning,
                  notify_fugitive_title
                )
              end
            end)
          end)
        end,
        desc = "Fugitive: amend latest (author only)",
      })

      -- Log:
      -- stylua: ignore start
      map({ mode = "n", lhs = "<C-g>lo", rhs = "Git log --oneline", desc = "Fugitive: oneline" })
      map({ mode = "n", lhs = "<C-g>lO", rhs = "Git log --oneline -- %", desc = "Fugitive: oneline (current file)" })
      map({ mode = "n", lhs = "<C-g>lc", rhs = "Git log --oneline -- %", desc = "Fugitive: oneline (current file) (alt)" })

      map({ mode = "n", lhs = "<C-g>ll", rhs = "Git log", desc = "Fugitive: default" })
      map({ mode = "n", lhs = "<C-g>lL", rhs = "Git log -- %", desc = "Fugitive: default (current file)" })
      map({ mode = "n", lhs = "<C-g>lso", rhs = "Git log --oneline --no-merges origin/main..HEAD", desc = "Fugitive: oneline" })
      map({ mode = "n", lhs = "<C-g>lsl", rhs = "Git log --no-merges origin/main..HEAD", desc = "Fugitive: default" })
      -- stylua: ignore end

      map({
        mode = "n",
        lhs = "<C-g>lr",
        rhs = function()
          overseer.overseer_runner({
            cmds = "git log --pretty=format:'%<(7)%C(yellow)%h%C(reset) %<(15,trunc)%C(cyan)%ar%C(reset) %<(16,trunc)%C(green)%an%C(reset) %<(80,trunc)%s'",
          })
        end,
        desc = "Overseer: pretty (relative time)",
      })

      local function last_monday()
        local now = os.time()
        local day_of_week = tonumber(os.date("%w", now)) -- 0 = Sunday, 1 = Monday ...
        -- Compute days since Monday
        local days_since_monday = (day_of_week == 0) and 6 or (day_of_week - 1)
        local monday = now - days_since_monday * 24 * 60 * 60
        return os.date("%Y-%m-%d", monday)
      end

      map({
        mode = "n",
        lhs = "<C-g>lw",
        rhs = function()
          local account = account_or_notify()
          if not account then
            return
          end

          local author = account.fullname
          if not author then
            return 1
          end

          local monday = last_monday()
          local args = {
            "Git",
            "log",
            ("--since='%s'"):format(monday),
            ("--author='%s'"):format(author),
            "--date=format-local:'%a, %Y-%m-%d %H:%M'",
            "--pretty=format:'%<(8)%C(yellow)%h%C(reset)  %>>(20)%C(magenta)%ad%C(reset)  %s'",
          }
          local git_cmd = table.concat(args, " ")

          vim.cmd(git_cmd)
        end,
        desc = "Fugitive: my contributions this-week (no color)",
      })

      map({
        mode = "n",
        lhs = "<C-g>lW",
        rhs = function()
          local account = account_or_notify()
          if not account then
            return
          end

          local author = account.fullname
          if not author then
            return 1
          end

          local monday = last_monday()
          local args = {
            "git",
            "log",
            ("--since='%s'"):format(monday),
            ("--author='%s'"):format(author),
            "--date=format-local:'%a, %Y-%m-%d %H:%M'",
            "--pretty=format:'%<(8)%C(yellow)%h%C(reset)  %>>(20)%C(magenta)%ad%C(reset)  %<(80,trunc)%s'",
          }
          local git_cmd = table.concat(args, " ")

          overseer.overseer_runner({ cmds = git_cmd })
        end,
        desc = "Overseer: my contributions this-week (color)",
      })

      map({
        mode = "n",
        lhs = "<C-g>lsr",
        rhs = function()
          overseer.overseer_runner({
            cmds = "git log --no-merges --pretty=format:'%<(7)%C(yellow)%h%C(reset) %<(15,trunc)%C(cyan)%ar%C(reset) %<(16,trunc)%C(green)%an%C(reset) %<(80,trunc)%s' origin/main..HEAD",
          })
        end,
        desc = "Overseer: pretty (relative time)",
      })

      map({
        mode = "n",
        lhs = "<C-g>lp",
        rhs = function()
          local branch = git.current_branch().name
          local default_commits = 20
          vim.ui.input(
            { prompt = ("(Branch: %s) Number of commits [default %d, q to quit]: "):format(branch, default_commits) },
            function(input)
              local sanitized = util.sanitize_input(input or "")

              if sanitized:lower() == "q" then
                vim.notify(("Cancelled Git log for branch `%s`"):format(branch), log_info, notify_fugitive_title)
                return
              end

              if sanitized == "" then
                sanitized = tostring(default_commits)
              end

              local number_commits = tonumber(sanitized)
              if not number_commits or number_commits <= 0 then
                vim.notify(("Invalid number entered: `%s`"):format(sanitized), log_error, notify_fugitive_title)
                return
              end

              vim.notify(
                ("Showing the **%s** most recent commits on branch `%s`"):format(sanitized, branch),
                log_info,
                notify_fugitive_title
              )
              vim.cmd(("Git log --pretty=oneline -n %d --graph --abbrev-commit"):format(number_commits))
            end
          )
        end,
        desc = "Fugitive: pretty (enter number of commits)",
      })

      -- Diff (HEAD / latest commit):
      map({
        mode = "n",
        lhs = "<C-g>dhf",
        rhs = "vertical Git diff -p --stat --function-context",
        desc = "Fugitive: with function context (vertical)",
      })

      map({
        mode = "n",
        lhs = "<C-g>dhF",
        rhs = "Git diff -p --stat --function-context",
        desc = "Fugitive: with function context (horizontal)",
      })

      map({
        mode = "n",
        lhs = "<C-g>dhw",
        rhs = function()
          local repo_name = repo_name_or_notify()
          if not repo_name then
            return
          end

          local commit = git.latest_commit({ repo_name = repo_name }) or {}
          if not commit.hash then
            return
          end
          overseer.overseer_runner({ cmds = "git diff --color-words" })
        end,
        desc = "Overseer: emphasize changed words",
      })

      map({
        mode = "n",
        lhs = "<C-g>dhm",
        rhs = function()
          local repo_name = repo_name_or_notify()
          if not repo_name then
            return
          end

          local commit = git.latest_commit({ repo_name = repo_name }) or {}
          if not commit.hash then
            return
          end
          overseer.overseer_runner({ cmds = "git diff --color-moved" })
        end,
        desc = "Overseer: emphasize moved lines",
      })

      -- Diff (index / staging):
      map({
        mode = "n",
        lhs = "<C-g>dic",
        rhs = "Git diff --cached -U0",
        desc = "Git: no context",
      })

      map({
        mode = "n",
        lhs = "<C-g>diC",
        rhs = "Git diff --cached -U0 -- %",
        desc = "Git: no context (current file)",
      })

      map({
        mode = "n",
        lhs = "<C-g>dif",
        rhs = "vertical Git diff -p --stat --cached --function-context",
        desc = "Fugitive: function context (vertical)",
      })

      map({
        mode = "n",
        lhs = "<C-g>diF",
        rhs = "Git diff -p --stat --cached --function-context",
        desc = "Fugitive: function context (horizontal)",
      })

      -- Fetch/Pull:
      map({ mode = "n", lhs = "<C-g>Ff", rhs = "Git fetch", desc = "Fugitive: fetch" })
      map({ mode = "n", lhs = "<C-g>Fp", rhs = "Git pull", desc = "Fugitive: pull" })
      map({ mode = "n", lhs = "<C-g>Fr", rhs = "Git pull --rebase", desc = "Fugitive: pull --rebase" })

      -- stylua: ignore start
      -- Push:
      map({ mode = "n", lhs = "<C-g>pp", rhs = "Git push", desc = "Fugitive: push" })
      map({ mode = "n", lhs = "<C-g>pf", rhs = "Git push --force-with-lease", desc = "Fugitive: push --force-with-lease" })
      -- stylua: ignore end

      -- An interactive `git push -u origin <current_branch>` implementation:
      --   ∙ Prompts the user for confirmation before pushing the current branch to GitHub.
      --   ∙ Useful for safely publishing new branches without accidentally pushing unintended
      --     changes.
      map({
        mode = "n",
        lhs = "<C-g>pu",
        rhs = function()
          local branch = git.current_branch().name
          if not branch then
            return
          end

          vim.ui.input({ prompt = "Push " .. branch .. " to origin? [y]es／[n]o／[q]uit: " }, function(input)
            local confirm_push = util.sanitize_input(input)
            local yes_values = { y = true, ye = true, yes = true, yep = true, ok = true }

            if not yes_values[confirm_push] then
              vim.notify("Push cancelled for branch *" .. branch .. "*", log_info, notify_fugitive_title)
              return
            end

            -- HEAD is the branch this already resolved, so nothing has to be
            -- spliced in. The single quotes it replaces were not a defence: a
            -- branch name may legally contain one, and closing them early made
            -- the remainder into further `git push` flags.
            vim.cmd("Git push -u origin HEAD")
          end)
        end,
        desc = "Fugitive: push -u origin <branch>",
      })

      -- Whatchanged:
      map({
        mode = "n",
        lhs = "<C-g>ww",
        rhs = "Git whatchanged --i-still-use-this",
        desc = "Fugitive: whatchanged (workspace)",
      })

      map({
        mode = "n",
        lhs = "<C-g>wb",
        rhs = "Git whatchanged --i-still-use-this -- %",
        desc = "Fugitive: whatchanged (buffer)",
      })

      map({
        mode = "n",
        lhs = "<C-g>wc",
        rhs = ":Git whatchanged --i-still-use-this --since=",
        desc = "Fugitive: whatchanged --since=<date>",
      })

      -- Browse:
      map({ mode = "n", lhs = "<C-g>of", rhs = "GBrowse", desc = "Fugitive: browse (file)" })
      map({ mode = "n", lhs = "<C-g>ol", rhs = ".GBrowse", desc = "Fugitive: browse (line in file)" })

      local function get_default_browser()
        -- Detect default browser on macOS
        local detect_default_browser_cmd = table.concat({
          "python3 -c '",
          "import plistlib, os; ",
          'pl=plistlib.load(open(os.path.expanduser("~/Library/Preferences/com.apple.LaunchServices/com.apple.launchservices.secure.plist"),"rb")); ',
          'print(next(item["LSHandlerRoleAll"] for item in pl["LSHandlers"] if item.get("LSHandlerURLScheme")=="http"))\'',
          " | xargs -I{} osascript -e 'name of application id \"{}\"'",
        }, "")

        -- A string on purpose: this is a real pipeline, and every word of it is
        -- a literal written above, with nothing interpolated into it.
        local ok, default_browser = util.run_shell_command({ cmd = detect_default_browser_cmd })

        if not ok or not default_browser then
          default_browser = "your default browser"
        else
          default_browser = util.trim(default_browser)
        end

        return default_browser
      end

      local function build_remote_repo_info(url_suffix, use_current_branch)
        if not git.initialized() then
          return
        end

        -- `github.repo` reports a `gh` that cannot answer as `nil, message`, so
        -- reading a field straight off the call raises on the index instead.
        local repo_info = try(function()
          return github.repo()
        end, { label = "github.repo" })
        if not repo_info then
          return
        end
        local repo = repo_info.nameWithOwner

        -- `git.default_branch` only knows this checkout's remote-tracking refs;
        -- a clone with neither `origin/main` nor `origin/master` falls through
        -- to GitHub, which is the one place that answer can come from. That call
        -- reports a `gh` failure as `nil, message` and raises on a repository
        -- whose answer carried no slash, and `try` is what keeps both out of the
        -- mapping and in a notification.
        local branch_name = use_current_branch and git.current_branch().name
          or git.default_branch()
          or try(function()
            return github.default_branch({ owner = repo_info.owner, name = repo_info.name })
          end, { label = "github.default_branch" })

        local url = "https://github.com/" .. repo
        if url_suffix ~= "" then
          url = url .. "/" .. url_suffix
        end

        if branch_name ~= "" and use_current_branch then
          url = url .. "/tree/" .. branch_name
        end

        return {
          repo = repo,
          branch_name = branch_name,
          url = url,
        }
      end

      local function open_github_page_mapping(opts)
        local key = opts.key
        local url_suffix = opts.url_suffix or ""
        local page_desc = opts.page_desc
        local use_current_branch = opts.branch or false

        if not page_desc then
          if url_suffix ~= "" then
            page_desc = string.gsub(" " .. url_suffix, "%W%l", string.upper):sub(2)
          else
            page_desc = "Homepage"
          end
        end

        local mapping_desc = page_desc
        if url_suffix == "" and use_current_branch then
          mapping_desc = ("Current Branch (%s)"):format(mapping_desc)
        end

        local mapping_table = {
          mode = "n",
          lhs = "<C-g>o" .. key,
          rhs = function()
            local remote_repo_info = build_remote_repo_info(url_suffix, use_current_branch)
            if not remote_repo_info then
              return
            end

            vim.notify(
              (
                "🌐 Opening GitHub Page"
                .. "\n-------------------------------"
                .. "\nPage: **%s [[%s]]**"
                .. "\nRepo: `%s`"
                .. "\nURL: `%s`"
                .. "\nBrowser: `%s`"
              ):format(
                page_desc,
                remote_repo_info.branch_name,
                remote_repo_info.repo,
                remote_repo_info.url,
                get_default_browser()
              ),
              log_info,
              {
                timeout = 0,
                title = "Open in Browser",
              }
            )
            util.run_shell_command({ cmd = { "open", remote_repo_info.url } })
          end,
          desc = ("Open GitHub: %s"):format(mapping_desc),
        }

        return mapping_table
      end

      map(open_github_page_mapping({ key = "o" }))
      map(open_github_page_mapping({ key = "b", branch = true }))
      map(open_github_page_mapping({ key = "i", url_suffix = "issues" }))
      map(open_github_page_mapping({ key = "p", url_suffix = "pulls", page_desc = "Pull Requests" }))
      map(open_github_page_mapping({ key = "a", url_suffix = "actions" }))
      map(open_github_page_mapping({ key = "P", url_suffix = "projects" }))
      map(open_github_page_mapping({ key = "w", url_suffix = "wiki" }))
      map(open_github_page_mapping({ key = "S", url_suffix = "security" }))
      map(open_github_page_mapping({ key = "I", url_suffix = "pulse", page_desc = "Insights" }))
      map(open_github_page_mapping({ key = "s", url_suffix = "settings" }))
    end,
  },
  {
    "linrongbin16/gitlinker.nvim",
    cmd = "GitLink",
    opts = {},
    keys = {
      { "<C-g>y", "<cmd>GitLink<cr>", mode = { "n", "v" }, desc = "Yank git link" },
      { "<C-g>oL", "<cmd>GitLink!<cr>", mode = { "n", "v" }, desc = "Git Linker: browse (line in file)" },
    },
  },
  {
    {
      "pwntester/octo.nvim",
      dependencies = {
        "nvim-lua/plenary.nvim",
        "folke/snacks.nvim",
        "nvim-tree/nvim-web-devicons",
      },
      config = function()
        require("octo").setup({
          suppress_missing_scope = {
            projects_v2 = true,
          },
          default_merge_method = "merge",
          picker = "snacks",
          enable_builtin = true,
          picker_config = {
            use_emojis = false, -- Only used by "fzf-lua" picker for now.
            mappings = { -- mappings for the pickers
              open_in_browser = { lhs = "<C-b>", desc = "Octo: open issue in browser" },
              copy_url = { lhs = "<C-y>", desc = "Octo: copy url to system clipboard" },
              copy_sha = { lhs = "<C-e>", desc = "Octo: copy commit SHA to system clipboard" },
              checkout_pr = { lhs = "<C-o>", desc = "Octo: checkout pull request" },
              merge_pr = { lhs = "<C-r>", desc = "Octo: merge pull request" },
            },
          },
        })

        map({ mode = "n", lhs = "<leader>ghg", rhs = "Octo gist list", desc = "Octo: list gists" })
        map({ mode = "n", lhs = "<leader>ghi", rhs = "Octo issue list", desc = "Octo: list issues" })
        map({ mode = "n", lhs = "<leader>ghI", rhs = "Octo issue create", desc = "Octo: create issue" })
        map({ mode = "n", lhs = "<leader>ghm", rhs = "Octo pr merge", desc = "Octo: merge pull request" })
        map({ mode = "n", lhs = "<leader>ghn", rhs = "Octo notification", desc = "Octo: notifications" })
        map({ mode = "n", lhs = "<leader>ghp", rhs = "Octo pr list", desc = "Octo: list pull requests" })
        map({ mode = "n", lhs = "<leader>ghP", rhs = "Octo pr create", desc = "Octo: create pull request" })
        map({ mode = "n", lhs = "<leader>ghr", rhs = "Octo repo list", desc = "Octo: list repos" })
        map({ mode = "n", lhs = "<leader>ghw", rhs = "Octo run list", desc = "Octo: list workflow runs" })

        local opts = {
          prefix = "<localleader>",
          buffer = 0, -- Target the current buffer
          mode = "n", -- Normal mode
        }

        vim.api.nvim_create_autocmd("FileType", {
          pattern = "octo",
          callback = function()
            local wk = require("which-key")
            wk.add({
              { "<localleader>a", group = "Assignee" },
              { "<localleader>c", group = "Comment" },
              { "<localleader>i", group = "Issue" },
              { "<localleader>g", group = "Navigate" },
              { "<localleader>l", group = "Label" },
              { "<localleader>p", group = "PR" },
              { "<localleader>r", group = "React" },
              { "<localleader>v", group = "Review" },
            }, opts)
          end,
        })
      end,
    },
  },
}
