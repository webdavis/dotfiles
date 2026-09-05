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
    lhs,
    function()
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
    silent = true,
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
    -- `:Git` and the four log commands take a plain line range and no `-bar`,
    -- which is exactly what a lazy.nvim placeholder is, so a placeholder parses
    -- them the way fugitive would. Keeping them here is what gives the command
    -- line its completion before fugitive loads.
    -- `Gr` is here for a different reason: it is `-bar`, but leaving it out of
    -- `cmd` leaves the name undefined, and an undefined name that is a unique
    -- prefix of a defined one abbreviates to it, so `:Gr` reaches grug-far's
    -- `GrugFar` and no `CmdUndefined` ever fires. A placeholder is an exact
    -- match and wins, at the cost of the bar on a first-use `:Gr file | cmd`.
    -- It is the only fugitive command any other spec's name abbreviates to.
    cmd = { "G", "GcLog", "Gclog", "Git", "GlLog", "Gllog", "Gr" },
    -- Every other fugitive command is `-bar`, or addresses tabs or windows
    -- rather than lines. A placeholder is created with `range = true` and no
    -- `bar`, so on FIRST use it parses those wrongly: `:2Gtabedit HEAD:file`
    -- checks the 2 against the buffer's line count and dies with E16 without
    -- loading fugitive at all, and `:Gedit file | let x = 1` swallows the bar
    -- into the argument. Loading from `CmdUndefined` leaves the command
    -- undefined until it is typed, so fugitive's own declaration is what parses
    -- the first invocation.
    event = {
      {
        event = "CmdUndefined",
        pattern = {
          "GBrowse",
          "GDelete",
          "GMove",
          "GRemove",
          "GRename",
          "GUnlink",
          "Gcd",
          "Gdiffsplit",
          "Gdrop",
          "Ge",
          "Gedit",
          "Ggrep",
          "Ghdiffsplit",
          "Glcd",
          "Glgrep",
          "Gpedit",
          "Gread",
          "Gsplit",
          "Gtabedit",
          "Gvdiffsplit",
          "Gvsplit",
          "Gw",
          "Gwq",
          "Gwrite",
        },
      },
    },
    keys = function()
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

      local function last_monday()
        local now = os.time()
        local day_of_week = tonumber(os.date("%w", now)) -- 0 = Sunday, 1 = Monday ...
        -- Compute days since Monday
        local days_since_monday = (day_of_week == 0) and 6 or (day_of_week - 1)
        local monday = now - days_since_monday * 24 * 60 * 60
        return os.date("%Y-%m-%d", monday)
      end

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
          "<C-g>o" .. key,
          function()
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
          silent = true,
        }

        return mapping_table
      end

      return {
        -- Init／Create:
        {
          "<C-g>i",
          function()
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
                  vim.notify(
                    "Project creation aborted for project **" .. project .. "**",
                    log_info,
                    notify_github_title
                  )
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
          silent = true,
        },

        -- Status:
        { "<C-g>ss", "<cmd>Git<cr>", desc = "Fugitive: status", silent = true },
        { "<C-g>sn", "<cmd>Git status -sb<cr>", desc = "Fugitive: status (as notification)", silent = true },

        -- Staging:
        { "<C-g>a", "<cmd>Gwrite<cr>", desc = "Fugitive: add file", silent = true },

        -- Stage current file → Amend last commit (no edit) → Force push:
        {
          "<C-g>!",
          "<cmd>Gwrite|Git commit --amend --no-edit|Git push --force<cr>",
          desc = "Fugitive: stage → amend (no edit) → force push",
          silent = true,
        },

        -- Stash push:
        {
          "<C-g>Sd",
          "<cmd>Git stash --include-untracked<cr>",
          desc = "Push: tracked + untracked (default)",
          silent = true,
        },

        {
          "<C-g>Se",
          "<cmd>Git stash --all<cr>",
          desc = "Push: all (tracked + untracked + ignored)",
          silent = true,
        },

        {
          "<C-g>Sw",
          "<cmd>Git stash --keep-index<cr>",
          desc = "Push: working (keep staged changes)",
          silent = true,
        },

        {
          "<C-g>SW",
          "<cmd>Git stash --keep-index --include-untracked<cr>",
          desc = "Push: working + untracked (keep staged changes)",
          silent = true,
        },

        {
          "<C-g>Ss",
          "<cmd>Git stash --staged<cr>",
          desc = "Push: staged",
          silent = true,
        },

        -- Stash pop:
        { "<C-g>Sp", "<cmd>Git stash pop<cr>", desc = "Pop: most recent (default)", silent = true },

        {
          "<C-g>SP",
          function()
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
          silent = true,
        },

        -- Stash apply:
        { "<C-g>Sa", "<cmd>Git stash apply<cr>", desc = "Apply: most recent (default)", silent = true },

        {
          "<C-g>SA",
          function()
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
          silent = true,
        },

        -- Remote:
        copy_url_mapping_helper("<C-g>rh", "origin", "https"),
        copy_url_mapping_helper("<C-g>rH", "upstream", "https"),
        copy_url_mapping_helper("<C-g>rs", "origin", "ssh"),
        copy_url_mapping_helper("<C-g>rS", "upstream", "ssh"),

        -- stylua: ignore start
        -- Checkout:
        {
          "<C-g>Cb",
          function()
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
          silent = true,
        },

        { "<C-g>C-", "<cmd>Git checkout -<cr>", desc = "Fugitive (checkout): previous branch", silent = true },
        { "<C-g>Cm", "<cmd>Git checkout main<cr>", desc = "Fugitive (checkout): main", silent = true },
        -- stylua: ignore end

        -- Branch:
        { "<C-g>bb", "<cmd>Git branch<cr>", desc = "Fugitive: local", silent = true },
        { "<C-g>bV", "<cmd>Git branch -vv<cr>", desc = "Fugitive: local (verbose)", silent = true },
        { "<C-g>bR", "<cmd>Git branch -rv<cr>", desc = "Fugitive: remotes (verbose)", silent = true },
        { "<C-g>bA", "<cmd>Git branch --all -vv<cr>", desc = "Fugitive: local + remote (verbose)", silent = true },

        {
          "<C-g>bc",
          show_current_git_branch,
          desc = "Notify: current + copy hash to clipboard (verbose)",
          silent = true,
        },

        {
          "<C-g>bv",
          show_all_local_branches_with_info,
          desc = "Notify: local + info",
          silent = true,
        },

        -- stylua: ignore start
        -- Commit:
        { "<C-g>cc", "<cmd>Git commit --verbose<cr>", desc = "Fugitive: entire index (all staged changes)", silent = true },
        { "<C-g>cf", "<cmd>Git commit %<cr>", desc = "Fugitive: current file only", silent = true },
        { "<C-g>ca", "<cmd>Git commit --amend --verbose<cr>", desc = "Fugitive: amend latest (edit message)", silent = true },
        { "<C-g>cn", "<cmd>Git commit --amend --no-edit<cr>", desc = "Fugitive: amend latest (don't edit message)", silent = true },
        -- stylua: ignore end

        {
          "<C-g>cp",
          function()
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
          silent = true,
        },

        -- An interactive command to amend the author/email of the latest commit:
        {
          "<C-g>cA",
          function()
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
          silent = true,
        },

        -- Log:
        -- stylua: ignore start
        { "<C-g>lo", "<cmd>Git log --oneline<cr>", desc = "Fugitive: oneline", silent = true },
        { "<C-g>lO", "<cmd>Git log --oneline -- %<cr>", desc = "Fugitive: oneline (current file)", silent = true },
        { "<C-g>lc", "<cmd>Git log --oneline -- %<cr>", desc = "Fugitive: oneline (current file) (alt)", silent = true },

        { "<C-g>ll", "<cmd>Git log<cr>", desc = "Fugitive: default", silent = true },
        { "<C-g>lL", "<cmd>Git log -- %<cr>", desc = "Fugitive: default (current file)", silent = true },
        { "<C-g>lso", "<cmd>Git log --oneline --no-merges origin/main..HEAD<cr>", desc = "Fugitive: oneline", silent = true },
        { "<C-g>lsl", "<cmd>Git log --no-merges origin/main..HEAD<cr>", desc = "Fugitive: default", silent = true },
        -- stylua: ignore end

        {
          "<C-g>lr",
          function()
            overseer.overseer_runner({
              cmds = "git log --pretty=format:'%<(7)%C(yellow)%h%C(reset) %<(15,trunc)%C(cyan)%ar%C(reset) %<(16,trunc)%C(green)%an%C(reset) %<(80,trunc)%s'",
            })
          end,
          desc = "Overseer: pretty (relative time)",
          silent = true,
        },

        {
          "<C-g>lw",
          function()
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
          silent = true,
        },

        {
          "<C-g>lW",
          function()
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
          silent = true,
        },

        {
          "<C-g>lsr",
          function()
            overseer.overseer_runner({
              cmds = "git log --no-merges --pretty=format:'%<(7)%C(yellow)%h%C(reset) %<(15,trunc)%C(cyan)%ar%C(reset) %<(16,trunc)%C(green)%an%C(reset) %<(80,trunc)%s' origin/main..HEAD",
            })
          end,
          desc = "Overseer: pretty (relative time)",
          silent = true,
        },

        {
          "<C-g>lp",
          function()
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
          silent = true,
        },

        -- Diff (HEAD / latest commit):
        {
          "<C-g>dhf",
          "<cmd>vertical Git diff -p --stat --function-context<cr>",
          desc = "Fugitive: with function context (vertical)",
          silent = true,
        },

        {
          "<C-g>dhF",
          "<cmd>Git diff -p --stat --function-context<cr>",
          desc = "Fugitive: with function context (horizontal)",
          silent = true,
        },

        {
          "<C-g>dhw",
          function()
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
          silent = true,
        },

        {
          "<C-g>dhm",
          function()
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
          silent = true,
        },

        -- Diff (index / staging):
        {
          "<C-g>dic",
          "<cmd>Git diff --cached -U0<cr>",
          desc = "Git: no context",
          silent = true,
        },

        {
          "<C-g>diC",
          "<cmd>Git diff --cached -U0 -- %<cr>",
          desc = "Git: no context (current file)",
          silent = true,
        },

        {
          "<C-g>dif",
          "<cmd>vertical Git diff -p --stat --cached --function-context<cr>",
          desc = "Fugitive: function context (vertical)",
          silent = true,
        },

        {
          "<C-g>diF",
          "<cmd>Git diff -p --stat --cached --function-context<cr>",
          desc = "Fugitive: function context (horizontal)",
          silent = true,
        },

        -- Fetch/Pull:
        { "<C-g>Ff", "<cmd>Git fetch<cr>", desc = "Fugitive: fetch", silent = true },
        { "<C-g>Fp", "<cmd>Git pull<cr>", desc = "Fugitive: pull", silent = true },
        { "<C-g>Fr", "<cmd>Git pull --rebase<cr>", desc = "Fugitive: pull --rebase", silent = true },

        -- stylua: ignore start
        -- Push:
        { "<C-g>pp", "<cmd>Git push<cr>", desc = "Fugitive: push", silent = true },
        { "<C-g>pf", "<cmd>Git push --force-with-lease<cr>", desc = "Fugitive: push --force-with-lease", silent = true },
        -- stylua: ignore end

        -- An interactive `git push -u origin <current_branch>` implementation:
        --   ∙ Prompts the user for confirmation before pushing the current branch to GitHub.
        --   ∙ Useful for safely publishing new branches without accidentally pushing unintended
        --     changes.
        {
          "<C-g>pu",
          function()
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
          silent = true,
        },

        -- Whatchanged:
        {
          "<C-g>ww",
          "<cmd>Git whatchanged --i-still-use-this<cr>",
          desc = "Fugitive: whatchanged (workspace)",
          silent = true,
        },

        {
          "<C-g>wb",
          "<cmd>Git whatchanged --i-still-use-this -- %<cr>",
          desc = "Fugitive: whatchanged (buffer)",
          silent = true,
        },

        {
          "<C-g>wc",
          ":Git whatchanged --i-still-use-this --since=",
          desc = "Fugitive: whatchanged --since=<date>",
        },

        -- Browse:
        { "<C-g>of", "<cmd>GBrowse<cr>", desc = "Fugitive: browse (file)", silent = true },
        { "<C-g>ol", "<cmd>.GBrowse<cr>", desc = "Fugitive: browse (line in file)", silent = true },

        open_github_page_mapping({ key = "o" }),
        open_github_page_mapping({ key = "b", branch = true }),
        open_github_page_mapping({ key = "i", url_suffix = "issues" }),
        open_github_page_mapping({ key = "p", url_suffix = "pulls", page_desc = "Pull Requests" }),
        open_github_page_mapping({ key = "a", url_suffix = "actions" }),
        open_github_page_mapping({ key = "P", url_suffix = "projects" }),
        open_github_page_mapping({ key = "w", url_suffix = "wiki" }),
        open_github_page_mapping({ key = "S", url_suffix = "security" }),
        open_github_page_mapping({ key = "I", url_suffix = "pulse", page_desc = "Insights" }),
        open_github_page_mapping({ key = "s", url_suffix = "settings" }),
      }
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
      -- `Octo` is the only command octo defines, and `octo.setup()` in `config`
      -- is what creates it, so it has to be named up front. The rows below are
      -- the `<leader>gh` mappings themselves: lazy.nvim installs the placeholder
      -- at startup and sets the real mapping from the same row once octo loads.
      -- The `<localleader>` groups stay in `config`, registered by its
      -- `FileType octo` autocmd, which is not a keymap the spec can carry.
      cmd = "Octo",
      keys = {
        { "<leader>ghg", "<cmd>Octo gist list<cr>", desc = "Octo: list gists", silent = true },
        { "<leader>ghi", "<cmd>Octo issue list<cr>", desc = "Octo: list issues", silent = true },
        { "<leader>ghI", "<cmd>Octo issue create<cr>", desc = "Octo: create issue", silent = true },
        { "<leader>ghm", "<cmd>Octo pr merge<cr>", desc = "Octo: merge pull request", silent = true },
        { "<leader>ghn", "<cmd>Octo notification<cr>", desc = "Octo: notifications", silent = true },
        { "<leader>ghp", "<cmd>Octo pr list<cr>", desc = "Octo: list pull requests", silent = true },
        { "<leader>ghP", "<cmd>Octo pr create<cr>", desc = "Octo: create pull request", silent = true },
        { "<leader>ghr", "<cmd>Octo repo list<cr>", desc = "Octo: list repos", silent = true },
        { "<leader>ghw", "<cmd>Octo run list<cr>", desc = "Octo: list workflow runs", silent = true },
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
