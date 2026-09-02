-- ╭─────────────╮
-- │   Modules   │
-- ╰─────────────╯
local git = require("custom_api.git")
local github = require("custom_api.github")
local util = require("custom_api.util")

-- ╭─────────────╮
-- │   Helpers   │
-- ╰─────────────╯
local log_info = vim.log.levels.INFO
local log_warning = vim.log.levels.WARN
local log_error = vim.log.levels.ERROR
local notify_fugitive_title = { title = "Fugitive" }
local notify_github_title = { title = "GitHub" }

local function copy_url_mapping_helper(lhs, remote, protocol)
  local mapping_table = {
    mode = "n",
    lhs = lhs,
    rhs = function()
      if git.initialized() then
        local url = git.url({
          remote = remote,
          account_name = github.account().username,
          repo_name = github.repo().name,
        })

        if not url then
          vim.notify("Warning: Nothing copied to clipboard!", log_warning, { title = "git" })
          return
        end

        local message = git.copy_URL_to_clipboard({
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
    end,
  },
  {
    "tpope/vim-fugitive",
    dependencies = {
      "tpope/vim-rhubarb",
      "junegunn/gv.vim",
      "stevearc/overseer.nvim",
      "folke/snacks.nvim",
    },
    config = function()
      -- Init／Create:
      map({
        mode = "n",
        lhs = "<C-g>i",
        rhs = function()
          local is_initialized = git.initialized({ quiet = true })

          local user = github.username()
          if not user then
            return
          end

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

              local gh_exit, _ = util.run_shell_command({ cmd = "gh repo view '" .. project .. "'" })

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
              table.insert(cmds, 'gh repo create --public "' .. project .. '"')

              util.overseer_runner({ cmds = cmds })
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
          if index ~= "" then
            vim.cmd("Git stash pop " .. index)
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
          if index ~= "" then
            vim.cmd("Git stash apply " .. index)
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
          local repo = github.repo().name
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

            local cmd = "Git checkout -b " .. new_branch
            vim.cmd(cmd)
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

      map({
        mode = "n",
        lhs = "<C-g>bc",
        rhs = function()
          local branch = git.current_branch().name
          if not branch then
            return
          end
          vim.notify("**Current Branch:** `" .. branch .. "`", log_info, { title = "Active Git Branch" })
        end,
        desc = "Notify: current",
      })

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

        local hash, summary, body = git.latest_commit({ repo_name = github.repo().name })
        local sections = build_sections(branch, hash, summary, body)
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
          local _, summary, body = git.latest_commit({ repo_name = github.repo().name })
          if not summary then
            return
          end

          vim.fn.setreg('"', summary .. "\n\n" .. (body or ""))
          vim.cmd("normal! ]p")
        end,
        desc = "Fugitive: paste latest message into buffer",
      })

      -- An interactive command to amend the author/email of the latest commit:
      map({
        mode = "n",
        lhs = "<C-g>cA",
        rhs = function()
          local hash, _, _ = git.latest_commit({ repo_name = github.repo().name })
          if not hash then
            return nil
          end

          local function message_helper(subject)
            return "No " .. subject .. " entered - author update cancelled for commit `" .. hash .. "`"
          end

          vim.ui.input({ prompt = "Author for latest commit (" .. hash .. "): " }, function(author)
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
              vim.cmd('Git commit -C HEAD --amend --author="' .. author .. " <" .. email .. '>"')
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
          util.overseer_runner({
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
          local author = github.account().fullname
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
          local author = github.account().fullname
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

          util.overseer_runner({ cmds = git_cmd })
        end,
        desc = "Overseer: my contributions this-week (color)",
      })

      map({
        mode = "n",
        lhs = "<C-g>lsr",
        rhs = function()
          util.overseer_runner({
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
          local hash, _, _ = git.latest_commit({ repo_name = github.repo().name })
          if not hash then
            return
          end
          util.overseer_runner({ cmds = "git diff --color-words" })
        end,
        desc = "Overseer: emphasize changed words",
      })

      map({
        mode = "n",
        lhs = "<C-g>dhm",
        rhs = function()
          local hash, _, _ = git.latest_commit({ repo_name = github.repo().name })
          if not hash then
            return
          end
          util.overseer_runner({ cmds = "git diff --color-moved" })
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

            vim.cmd("Git push -u origin '" .. branch .. "'")
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

        local repo = github.repo().nameWithOwner
        local branch_name = use_current_branch and git.current_branch().name or git.default_branch(repo)

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
            util.run_shell_command({ cmd = "open " .. remote_repo_info.url })
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
    "rhysd/git-messenger.vim",
    config = function()
      -- Ensure the cursor always moves to the popup window after `:GitMessenger` is run.
      vim.g.git_messenger_into_popup_after_show = 1
      vim.g.git_messenger_always_into_popup = 1

      local function git_messenger()
        local bufname = vim.api.nvim_buf_get_name(0)

        if not git.initialized() then
          return
        end

        local exit_code, _ =
          util.run_shell_command({ cmd = "git ls-files --error-unmatch " .. vim.fn.fnameescape(bufname) })

        if exit_code ~= 0 then
          vim.notify(
            "File `" .. bufname .. "` is not tracked by *" .. github.repo().name .. "*",
            log_warning,
            { title = "Git Messenger" }
          )
          return
        end

        vim.cmd("GitMessenger")
      end

      map({
        mode = "n",
        lhs = "<leader>gm",
        rhs = git_messenger,
        desc = "Git Messenger: toggle",
      })
    end,
  },
  {
    -- TODO: git-blame: put this somewhere so that it's only available when attached to a git project. (E.g. on_attach)
    "f-person/git-blame.nvim",
    config = function()
      map({ mode = "n", lhs = "<C-g>Bt", rhs = "GitBlameToggle", desc = "Git Blame: toggle" })
      map({ mode = "n", lhs = "<C-g>By", rhs = "GitBlameCopySHA", desc = "Git Blame: copy commit SHA" })
      map({ mode = "n", lhs = "<C-g>Bo", rhs = "GitBlameOpenCommitURL", desc = "Git Blame: open commit URL" })
      map({ mode = "n", lhs = "<C-g>BO", rhs = "GitBlameCopyCommitURL", desc = "Git Blame: copy commit URL" })
    end,
  },
  {
    {
      "nvim-telescope/telescope.nvim",
      config = function()
        require("telescope").setup({
          defaults = {
            layout_config = {
              preview_width = 0.5,
              width = 0.9,
              height = 0.9,
            },
          },
        })
      end,
    },
    {
      "pwntester/octo.nvim",
      dependencies = {
        "nvim-lua/plenary.nvim",
        "folke/snacks.nvim",
        "nvim-telescope/telescope.nvim",
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
