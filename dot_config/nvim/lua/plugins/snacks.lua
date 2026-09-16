-- The dashboard is a doorway, not a room (operator 2026-09-15). Neovim gets
-- opened to work on a particular file, so the whole dashboard is a ranked list
-- of files plus one row of verbs, and every panel that once sat beside it was
-- removed for failing that test: Git Status duplicated the file list's own
-- change markers, and Git Log, Notifications, Open Issues, Open PRs and Browse
-- Repo are all news or browser work rather than the file at hand.
local dashboard_width = 60

---The action keys, all `hidden`: the compact verb row in the sections below is
---the rendering, and these exist only to bind the keys. Without `hidden` the
---dashboard drew the same seven lines twice.
-- stylua: ignore
local action_keys = {
  { icon = " ", key = "f", desc = "Find File",    action = ":lua Snacks.dashboard.pick('files')", hidden = true },
  { icon = " ", key = "r", desc = "Recent Files", action = ":lua Snacks.dashboard.pick('oldfiles')", hidden = true },
  { icon = " ", key = "g", desc = "Find Text",    action = ":lua Snacks.dashboard.pick('live_grep')", hidden = true },
  { icon = " ", key = "n", desc = "New File",     action = ":ene | startinsert", hidden = true },
  { icon = " ", key = "c", desc = "Config",       action = ":lua Snacks.dashboard.pick('files', {cwd = vim.fn.stdpath('config')})", hidden = true },
  { icon = "\u{f00ab} ", key = "L", desc = "Lazy", action = ":Lazy", enabled = package.loaded.lazy ~= nil, hidden = true },
  { icon = " ", key = "q", desc = "Quit",         action = ":qa", hidden = true },
}

-- A key claimed twice is not broken, it is quietly wrong: the dashboard draws
-- both rows and one of them does the other's work. `n` was claimed by New File
-- AND by a Notifications pane for exactly that reason. Refuse at load instead.
do
  local claimed = {}
  for _, item in ipairs(action_keys) do
    if claimed[item.key] then
      error(("dashboard key %q is claimed by both %s and %s"):format(item.key, claimed[item.key], item.desc))
    end
    claimed[item.key] = item.desc
  end
end

-- What the rows around the file list cost: the label above it, the verb row,
-- and a margin so the list never runs to the last line of the window.
local rows_around_the_file_list = 8

---How many file rows this window has room for, between a floor that keeps the
---list useful and a ceiling set by how many single-keystroke rows are worth
---offering at all. Read on every render, because the dashboard redraws on
---`VimResized`.
---@return number
local function file_rows()
  return math.max(3, math.min(9, vim.o.lines - rows_around_the_file_list))
end

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
        width = dashboard_width,
        preset = { keys = action_keys },
        sections = {
          -- FILES FIRST. The dashboard is a doorway, not a room (operator
          -- 2026-09-15): Neovim is opened to work on a specific file, so the
          -- ranked file list takes the top rows and the verbs move under it.
          -- The banner went with them, because it spent the most valuable rows
          -- on screen naming the program that had just been launched.
          {
            padding = 1,
            -- stylua: ignore
            text = { { "  where you left off", hl = "dir" } },
          },
          function()
            return require("custom_api.dashboard_files").section(file_rows(), dashboard_width)
          end,
          -- One row, not eight. Every verb here is also on the leader map, so
          -- the dashboard only has to remind rather than teach.
          {
            padding = 1,
            -- stylua: ignore
            text = {
              { "  " },
              { "f", hl = "key" }, { " find   ", hl = "desc" },
              { "g", hl = "key" }, { " grep   ", hl = "desc" },
              { "n", hl = "key" }, { " new   ", hl = "desc" },
              { "c", hl = "key" }, { " config   ", hl = "desc" },
              { "L", hl = "key" }, { " lazy   ", hl = "desc" },
              { "q", hl = "key" }, { " quit", hl = "desc" },
            },
          },
          -- The keys themselves, declared but not drawn. `hidden` belongs on
          -- each key rather than here: on this wrapper it does not reach the
          -- rows the section generates, and snacks drew a second copy of all
          -- seven lines under the compact row above.
          { section = "keys" },
        },
      },
      explorer = {
        enabled = true,
        replace_netrw = false, -- Don't replace netrw with the snacks explorer.
        trash = true,
      },
      -- The gate on xcodebuild.nvim's SwiftUI, UIKit and AppKit previews and on its
      -- snapshot-test diffs: both refuse to render unless `snacks.image.config.enabled`
      -- is true (`core/previews.lua:78`). Both produce PNG, which Ghostty draws through
      -- the Kitty graphics protocol, and `magick` is installed for the other formats.
      image = { enabled = true },
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
      { "<leader>ghfi", function() Snacks.picker.gh_issue() end, desc = "GitHub Issues (open)" },
      { "<leader>ghfI", function() Snacks.picker.gh_issue({ state = "all" }) end, desc = "GitHub Issues (all)" },
      { "<leader>ghfp", function() Snacks.picker.gh_pr() end, desc = "GitHub Pull Requests (open)" },
      { "<leader>ghfP", function() Snacks.picker.gh_pr({ state = "all" }) end, desc = "GitHub Pull Requests (all)" },

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
              -- Read the module only if something else already loaded it. A bare
              -- `require` here would pull gitsigns off its own trigger through
              -- lazy.nvim's loader, and with no file open it is not loaded at
              -- all, so there are no signs to report.
              local gitsigns_config = package.loaded["gitsigns.config"]
              return gitsigns_config ~= nil and gitsigns_config.config.signcolumn or false
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
