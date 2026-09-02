return {
  {
    "folke/snacks.nvim",
    priority = 1000,
    lazy = false,
    ---@type snacks.Config
    opts = {
      bigfile = { enabled = true },
      dashboard = {
        enabled = true,
        sections = {
          { section = "header" },
          { section = "keys", gap = 1, padding = 2 },
          { icon = " ", title = "Recent Files", section = "recent_files", indent = 2, padding = 2 },
          { icon = " ", title = "Projects", section = "projects", indent = 2, padding = 2 },
          { section = "startup" },
          {
            pane = 2,
            title = [[
              ██████╗ ██╗████████╗
              ██╔════╝ ██║╚══██╔══╝
              ██║ ████╗██║   ██║
              ██║   ██║██║   ██║
              ╚██████╔╝██║   ██║
              ╚═════╝ ╚═╝   ╚═╝]],
            height = 0,
            padding = 1,
          },
          {
            pane = 2,
            icon = " ",
            desc = "Browse Repo",
            padding = 1,
            key = "b",
            -- stylua: ignore
            action = function() Snacks.gitbrowse() end,
          },
          function()
            local in_git = Snacks.git.get_root() ~= nil
            -- stylua: ignore start
            local cmds = {
                { icon = " ", title = "Git Status", cmd = "git --no-pager diff --stat -B -M -C", height = 10 },
                { icon = " ", title = "Git Log", cmd = "git log --oneline", height = 10 },
                {
                  title = "Notifications",
                  cmd = "gh notify -s -a -n5",
                  action = function() vim.ui.open( "https://github.com/notifications") end,
                  key = "n",
                  icon = " ",
                  height = 5,
                  enabled = true,
                },
                {
                  title = "Open Issues",
                  cmd = "gh issue list -L 3",
                  key = "i",
                  action = function() vim.fn.jobstart( "gh issue list --web", { detach = true, }) end,
                  icon = " ",
                  height = 5,
                },
                {
                  icon = " ",
                  title = "Open PRs",
                  cmd = "gh pr list -L 3",
                  key = "P",
                  action = function() vim.fn.jobstart( "gh pr list --web", { detach = true, }) end,
                  height = 5,
                },
            }
            -- stylua: ignore end
            return vim.tbl_map(function(cmd)
              return vim.tbl_extend("force", {
                pane = 2,
                section = "terminal",
                enabled = in_git,
                padding = 1,
                ttl = 5 * 60,
                indent = 3,
              }, cmd)
            end, cmds)
          end,
        },
      },
      explorer = {
        enabled = true,
        replace_netrw = false, -- Don't replace netrw with the snacks explorer.
        trash = true,
      },
      indent = { enabled = false },
      input = { enabled = true },
      notifier = {
        enabled = true,
        timeout = 3000,
        width = { min = 40, max = 0.9 },
        style = "compact",
      },
      picker = {
        enabled = true,
        layout = {
          preset = "default",
        },
        sources = {
          explorer = {
            diagnostics_open = true,
            git_status_open = true,
            hidden = true,
            ignored = true,
          },
          files = {
            show_empty = true,
            hidden = true,
            ignored = true,
          },
          git_branches = {
            layout = {
              preview = false,
            },
          },
          git_log = {
            layout = {
              preview = false,
            },
          },
          git_log_file = {
            layout = {
              preview = false,
            },
          },
          -- Wrap text in  Snacks.picker.notifications() previews.
          -- Without this it's almost impossible to read notifications using Snacks.
          -- Ref: https://www.reddit.com/r/neovim/comments/1mvlp86/lazyvim_snacks_picker_how_to_turn_on_preview/
          notifications = {
            win = {
              preview = {
                wo = {
                  wrap = true,
                },
              },
            },
            actions = {
              -- Variant 1
              --  Yank the current item message
              yank_msg = function(_, item)
                vim.fn.setreg("+", item.item.msg)
              end,
              -- Variant 2
              --  Yank the current item message, or
              --  if multiple items are selected (with <Tab>), yank them concatenated with ' '
              yank_many_msg = function(picker)
                local selected = picker:selected({ fallback = true })
                local messages = vim.tbl_map(function(s)
                  return s.item.msg
                end, selected)
                vim.fn.setreg("+", table.concat(messages, " "))
              end,
            },
            confirm = { "yank_msg", "close" },
          },
        },
      },
      quickfile = { enabled = true },
      scope = { enabled = true },
      scroll = { enabled = false },
      statuscolumn = { enabled = true },
      words = { enabled = true },
      styles = {
        notification = {
          -- Wrap text in vim.notify messages. Note: doesn't affect Snacks.picker.notifications()
          wo = { wrap = true },
        },
        input = {
          width = 100,
        },
      },
    },
    keys = {
      -- stylua: ignore start

      -- Top Pickers:
      { "<leader>fs", function() Snacks.picker.smart() end, desc = "Snacks: smart find files" },
      { "<leader>s:", function() Snacks.picker.command_history() end, desc = "Snacks: command history" },
      { "<C-g><C-f>", function() Snacks.picker.git_files() end, desc = "Snacks (Git): find files" },
      { "<leader>g,", function() Snacks.picker.git_files() end, desc = "Snacks (Git): find files (alt)" },
      { "<C-g>.", function() Snacks.picker.git_grep() end, desc = "Snacks (Git): grep git files" },
      { "<leader>g.", function() Snacks.picker.git_grep() end, desc = "Snacks (Git): grep git files (alt)" },
      { "<leader>ee", function() Snacks.explorer() end, desc = "File Explorer (Snacks)" },

      -- Notifications:
      { "<leader>nn", function() Snacks.picker.notifications() end, desc = "Snacks (Notifications): history (picker)" },
      { "<leader>nb", function() Snacks.notifier.show_history() end, desc = "Snacks (Notifications): history (buffer)" },
      { "<leader>nd", function() Snacks.notifier.hide() end, desc = "Snacks (Notifications): dismiss all" },

      -- Find:
      { "<leader>fb", function() Snacks.picker.buffers() end, desc = "Snacks: open buffers" },
      { "<leader>fB", function() Snacks.picker.buffers({ hidden = true, nofile = true }) end, desc = "Snacks: buffers (all)" },
      { "<leader>fc", function() Snacks.picker.files({ cwd = vim.fn.stdpath("config") }) end, desc = "Snacks: find config file" },
      { "<leader>ff", function() Snacks.picker.files() end, desc = "Snacks: find files" },
      { "<leader>fp", function() Snacks.picker.projects() end, desc = "Snacks: projects" },
      { "<leader>fr", function() Snacks.picker.recent() end, desc = "Snacks: recent files" },
      { "<leader>fR", function() Snacks.picker.recent({ filter = { cwd = true }}) end, desc = "Snacks: recent files (cwd)" },

      -- Grep:
      { "<leader>/g", function() Snacks.picker.git_grep() end, desc = "Snacks (Git): grep git files" },
      { "<leader>/p", function() Snacks.picker.grep() end, desc = "Grep: entire project" },
      { "<leader>/e", function() Snacks.picker.grep() end, desc = "Grep: entire project (alt)" },
      { "<leader>//", function() Snacks.picker.lines() end, desc = "Grep: current buffer" },
      { "<leader>/B", function() Snacks.picker.lines() end, desc = "Grep: current buffer (alt)" },
      { "<leader>/w", function() Snacks.picker.grep_word() end, desc = "Grep: <cword> / visual selection", mode = { "n", "x" } },
      { "<leader>/b", function() Snacks.picker.grep_buffers() end, desc = "Grep: available buffers" },

      -- Git:
      { "<leader>go", function() Snacks.gitbrowse() end, desc = "Snacks (Git): browse (opens file on GitHub)", mode = { "n", "v" } },
      { "<leader>gb", function() Snacks.picker.git_branches() end, desc = "Snacks (Git): branches" },
      { "<leader>gd", function() Snacks.picker.git_diff() end, desc = "Snacks (Git): diff hunks" },
      { "<leader>gf", function() Snacks.picker.git_log_file() end, desc = "Snacks (Git): log - current file" },
      { "<leader>gl", function() Snacks.picker.git_log() end, desc = "Snacks (Git): log" },
      { "<leader>gL", function() Snacks.picker.git_log_line() end, desc = "Snacks (Git): log - current line in file" },
      { "<leader>gs", function() Snacks.picker.git_status() end, desc = "Snacks (Git): status" },
      { "<leader>gS", function() Snacks.picker.git_stash() end, desc = "Snacks (Git): stash" },

      -- gh:
      { "<leader>Gi", function() Snacks.picker.gh_issue() end, desc = "GitHub Issues (open)" },
      { "<leader>GI", function() Snacks.picker.gh_issue({ state = "all" }) end, desc = "GitHub Issues (all)" },
      { "<leader>Gp", function() Snacks.picker.gh_pr() end, desc = "GitHub Pull Requests (open)" },
      { "<leader>GP", function() Snacks.picker.gh_pr({ state = "all" }) end, desc = "GitHub Pull Requests (all)" },

      -- Search:
      { "<leader>s/", function() Snacks.picker.search_history() end, desc = "Snacks: search history" },
      { "<leader>sa", function() Snacks.picker.autocmds() end, desc = "Snacks: autocmds" },
      { "<leader>sb", function() Snacks.picker.lines() end, desc = "Snacks: buffer lines" },
      { "<leader>sc", function() Snacks.picker.commands() end, desc = "Snacks: commands" },
      { "<leader>sD", function() Snacks.picker.diagnostics() end, desc = "Snacks: diagnostics" },
      { "<leader>sd", function() Snacks.picker.diagnostics_buffer() end, desc = "Snacks: buffer diagnostics" },
      { "<leader>sh", function() Snacks.picker.help() end, desc = "Snacks: help pages" },
      { "<leader>sH", function() Snacks.picker.highlights() end, desc = "Snacks: highlights" },
      { "<leader>si", function() Snacks.picker.icons() end, desc = "Snacks: icons" },
      { "<leader>sj", function() Snacks.picker.jumps() end, desc = "Snacks: jumps" },
      { "<leader>sk", function() Snacks.picker.keymaps() end, desc = "Snacks: keymaps" },
      { "<leader>sl", function() Snacks.picker.loclist() end, desc = "Snacks: location list" },
      { "<leader>sM", function() Snacks.picker.man() end, desc = "Snacks: man pages" },
      { "<leader>sm", function() Snacks.picker.marks() end, desc = "Snacks: marks" },
      { "<leader>sp", function() Snacks.picker.lazy() end, desc = "Snacks: search plugin specs" },
      { "<leader>sq", function() Snacks.picker.qflist() end, desc = "Snacks: Quickfix List" },
      { "<leader>sR", function() Snacks.picker.resume() end, desc = "Snacks: Resume Picker" },
      { "<leader>sC", function() Snacks.picker.colorschemes() end, desc = "Snacks: colorschemes" },
      { "<leader>su", function() Snacks.picker.undo() end, desc = "Snacks: undo history" },
      { '<leader>s"', function() Snacks.picker.registers() end, desc = "Snacks: registers" },

      -- LSP:
      { "<leader>ld", function() Snacks.picker.lsp_definitions() end, desc = "Snacks (LSP): go-to definition" },
      { "<leader>lD", function() Snacks.picker.lsp_declarations() end, desc = "Snacks (LSP): go-to declaration" },
      { "<leader>li", function() Snacks.picker.lsp_implementations() end, desc = "Snacks (LSP): go-to implementation" },
      { "<leader>lr", function() Snacks.picker.lsp_references() end, nowait = true, desc = "Snacks (LSP): references" },
      { "<leader>ls", function() Snacks.picker.lsp_symbols() end, desc = "Snacks (LSP): symbols" },
      { "<leader>lw", function() Snacks.picker.lsp_workspace_symbols() end, desc = "Snacks (LSP): workspace symbols" },
      { "<leader>lt", function() Snacks.picker.lsp_type_definitions() end, desc = "Snacks (LSP): go-to type definition" },

      -- Debug:
      { "<leader>Dps", function() Snacks.profiler.scratch() end, desc = "Snacks (Debug): profiler scratch buffer" },

      -- Scratch Buffer:
      { "<leader>s.", function() Snacks.scratch() end, desc = "Snacks: toggle scratch buffer" },
      { "<leader>s>", function() Snacks.scratch.select() end, desc = "Snacks: select scratch buffer" },

      -- Misc:
      { "<c-/>", function() Snacks.terminal() end, desc = "Snacks: toggle terminal" },
      { "<c-_>", function() Snacks.terminal() end, desc = "Snacks: which_key_ignore" },
      { "<leader>bd", function() Snacks.bufdelete() end, desc = "Snacks: delete buffer" },
      { "<leader>rf", function() Snacks.rename.rename_file() end, desc = "Snacks (Rename): file" },
      { "<M-n>",         function() Snacks.words.jump(vim.v.count1, true) end, desc = "Next Reference", mode = { "n", "t" } },
      { "<M-p>",         function() Snacks.words.jump(-vim.v.count1, true) end, desc = "Prev Reference", mode = { "n", "t" } },

      -- stylua: ignore end
      {
        "<leader>N",
        desc = "News: Neovim",
        function()
          Snacks.win({
            file = vim.api.nvim_get_runtime_file("doc/news.txt", false)[1],
            width = 0.6,
            height = 0.6,
            wo = {
              spell = false,
              wrap = false,
              signcolumn = "yes",
              statuscolumn = " ",
              conceallevel = 3,
            },
          })
        end,
      },
    },
    init = function()
      vim.api.nvim_create_autocmd("User", {
        pattern = "VeryLazy",
        callback = function()
          -- Setup some globals for debugging (lazy-loaded)
          _G.dd = function(...)
            Snacks.debug.inspect(...)
          end
          _G.bt = function()
            Snacks.debug.backtrace()
          end
          vim.print = _G.dd -- Override print to use snacks for `:=` command

          -- Toggle Options
          -- ——————————————————————————————————————————————————————————————————
          Snacks.toggle.animate():map("<leader>ua")
          Snacks.toggle
            .option("showtabline", { off = 0, on = vim.o.showtabline > 0 and vim.o.showtabline or 2, name = "Tabline" })
            :map("<leader>uA")
          Snacks.toggle.option("background", { off = "light", on = "dark", name = "Dark Background" }):map("<leader>ub")
          Snacks.toggle
            .option(
              "conceallevel",
              { off = 0, on = vim.o.conceallevel > 0 and vim.o.conceallevel or 2, name = "Conceal Level" }
            )
            :map("<leader>uc")
          Snacks.toggle.diagnostics():map("<leader>ud")
          Snacks.toggle.dim():map("<leader>uD")
          Snacks.toggle.words():map("<leader>ui")
          Snacks.toggle.indent():map("<leader>uI")

          Snacks.toggle({
            name = "Git Signs",
            get = function()
              return require("gitsigns.config").config.signcolumn
            end,
            set = function(state)
              require("gitsigns").toggle_signs(state)
            end,
          }):map("<leader>uG")

          Snacks.toggle.option("hlsearch", { off = false, on = true, name = "Search Highlight" }):map("<leader>uh")
          if vim.lsp.inlay_hint then
            Snacks.toggle.inlay_hints():map("<leader>uH")
          end

          Snacks.toggle.option("relativenumber", { name = "Relative Number" }):map("<leader>ul")
          Snacks.toggle.line_number():map("<leader>uL")
          Snacks.toggle.option("spell", { name = "Spelling" }):map("<leader>us")
          Snacks.toggle.scroll():map("<leader>uS")
          Snacks.toggle
            .option("textwidth", { off = 999, on = vim.opt.textwidth:get(), name = "Textwidth Limit" })
            :map("<leader>ut")
          Snacks.toggle.treesitter():map("<leader>uT")
          Snacks.toggle.option("wrap", { name = "Wrap" }):map("<leader>uw")

          -- Toggle the cursorline and cursorcolumn simultaneously.
          Snacks.toggle
            .new({
              id = "cursor_guides",
              name = "Cursor Guides",
              get = function()
                return vim.wo.cursorline and vim.wo.cursorcolumn
              end,
              set = function(state)
                vim.wo.cursorline = state
                vim.wo.cursorcolumn = state
              end,
            })
            :map("<leader>uX")

          Snacks.toggle.zoom():map("<leader>uz")
          Snacks.toggle.zen():map("<leader>uZ")

          Snacks.toggle({
            name = "QuickFix List",
            get = function()
              for _, win in pairs(vim.fn.getwininfo()) do
                if win["quickfix"] == 1 then
                  return true
                end
              end
              return false
            end,
            set = function(open)
              if open then
                vim.cmd("copen")
                vim.api.nvim_feedkeys([['"]], "im", false)
              else
                vim.cmd("cclose")
              end
            end,
          }):map("<leader>uq")

          Snacks.toggle.profiler():map("<leader>Dpp")
          Snacks.toggle.profiler_highlights():map("<leader>Dph")

          local function pick_cmd_result(picker_opts)
            local git_root = Snacks.git.get_root()
            local function finder(opts, ctx)
              return require("snacks.picker.source.proc").proc({
                opts,
                {
                  cmd = picker_opts.cmd,
                  args = picker_opts.args,
                  transform = function(item)
                    item.cwd = picker_opts.cwd or git_root
                    item.file = item.text
                  end,
                },
              }, ctx)
            end

            Snacks.picker.pick({
              source = picker_opts.name,
              finder = finder,
              preview = picker_opts.preview,
              title = picker_opts.title,
            })
          end

          local custom_pickers = {}

          function custom_pickers.git_show()
            pick_cmd_result({
              cmd = "git",
              args = { "diff-tree", "--no-commit-id", "--name-only", "--diff-filter=d", "HEAD", "-r" },
              name = "git_show",
              title = "Git Last Commit",
              preview = "git_show",
            })
          end

          function custom_pickers.git_diff_upstream()
            pick_cmd_result({
              cmd = "git",
              args = { "diff-tree", "--no-commit-id", "--name-only", "--diff-filter=d", "HEAD@{u}..HEAD", "-r" },
              name = "git_diff_upstream",
              title = "Git Branch Changed Files",
              preview = "file",
            })
          end

          map({
            mode = "n",
            lhs = "<leader>g<",
            rhs = custom_pickers.git_show,
            desc = "Snacks (Git): show last commit changes",
          })

          map({
            mode = "n",
            lhs = "<leader>gu",
            rhs = custom_pickers.git_diff_upstream,
            desc = "Snacks (Git): diff - current vs upstream",
          })
        end,
      })
    end,
  },
}
