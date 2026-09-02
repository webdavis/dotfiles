return {
  {
    "yousefhadder/markdown-plus.nvim",
    ft = "markdown",
    config = function()
      require("markdown-plus").setup({
        enabled = true,
        toc = {
          initial_depth = 6,
        },
      })

      -- Text Formatting
      -----------------------
      -- Normal mode:
      vim.keymap.set("n", "<localleader>mb", "<Plug>(MarkdownPlusBold)")
      vim.keymap.set("n", "<localleader>mi", "<Plug>(MarkdownPlusItalic)")
      vim.keymap.set("n", "<localleader>ms", "<Plug>(MarkdownPlusStrikethrough)")
      vim.keymap.set("n", "<localleader>mc", "<Plug>(MarkdownPlusCode)")
      vim.keymap.set("n", "<localleader>mw", "<Plug>(MarkdownPlusCodeBlock)")
      vim.keymap.set("n", "<localleader>mC", "<Plug>(MarkdownPlusClearFormatting)")

      -- Visual mode:
      vim.keymap.set("x", "<localleader>mb", "<Plug>(MarkdownPlusBold)")
      vim.keymap.set("x", "<localleader>mi", "<Plug>(MarkdownPlusItalic)")
      vim.keymap.set("x", "<localleader>ms", "<Plug>(MarkdownPlusStrikethrough)")
      vim.keymap.set("x", "<localleader>mc", "<Plug>(MarkdownPlusCode)")
      vim.keymap.set("x", "<localleader>mw", "<Plug>(MarkdownPlusCodeBlock)")
      vim.keymap.set("x", "<localleader>mC", "<Plug>(MarkdownPlusClearFormatting)")

      -- Headers
      -----------------------
      vim.keymap.set("n", "]]", "<Plug>(MarkdownPlusNextHeader)")
      vim.keymap.set("n", "[[", "<Plug>(MarkdownPlusPrevHeader)")
      vim.keymap.set("n", "<localleader>h+", "<Plug>(MarkdownPlusPromoteHeader)")
      vim.keymap.set("n", "<localleader>h-", "<Plug>(MarkdownPlusDemoteHeader)")
      vim.keymap.set("n", "<localleader>hT", "<Plug>(MarkdownPlusOpenTocWindow)")
      vim.keymap.set("n", "gd", "<Plug>(MarkdownPlusFollowLink)")

      -- Header levels 1-6.
      for i = 1, 6 do
        vim.keymap.set("n", "<localleader>h" .. i, "<Plug>(MarkdownPlusHeader" .. i .. ")")
      end

      -- Table of Contents
      -----------------------
      vim.keymap.set("n", "<localleader>ht", "<Plug>(MarkdownPlusGenerateTOC)")
      vim.keymap.set("n", "<localleader>hu", "<Plug>(MarkdownPlusUpdateTOC)")

      -- Links & References
      -----------------------
      vim.keymap.set("n", "<localleader>l", "<Plug>(MarkdownPlusInsertLink)")
      vim.keymap.set("v", "<localleader>l", "<Plug>(MarkdownPlusSelectionToLink)")
      vim.keymap.set("n", "<localleader>e", "<Plug>(MarkdownPlusEditLink)")
      vim.keymap.set("n", "<localleader>a", "<Plug>(MarkdownPlusAutoLinkURL)")
      vim.keymap.set("n", "<localleader>R", "<Plug>(MarkdownPlusConvertToReference)")
      vim.keymap.set("n", "<localleader>I", "<Plug>(MarkdownPlusConvertToInline)")

      -- Image Links
      -----------------------
      vim.keymap.set("n", "<localleader>L", "<Plug>(MarkdownPlusInsertImage)")
      vim.keymap.set("v", "<localleader>L", "<Plug>(MarkdownPlusSelectionToImage)")
      vim.keymap.set("n", "<localleader>E", "<Plug>(MarkdownPlusEditImage)")
      vim.keymap.set("n", "<localleader>A", "<Plug>(MarkdownPlusToggleImageLink)")

      -- List Management
      -----------------------
      -- Insert mode:
      vim.keymap.set("i", "<CR>", "<Plug>(MarkdownPlusListEnter)")
      vim.keymap.set("i", "<A-CR>", "<Plug>(MarkdownPlusListShiftEnter)")
      vim.keymap.set("i", "<Tab>", "<Plug>(MarkdownPlusListIndent)")
      vim.keymap.set("i", "<S-Tab>", "<Plug>(MarkdownPlusListOutdent)")
      vim.keymap.set("i", "<BS>", "<Plug>(MarkdownPlusListBackspace)")
      vim.keymap.set("i", "<C-t>", "<Plug>(MarkdownPlusToggleCheckbox)")

      -- Normal mode:
      vim.keymap.set("n", "o", "<Plug>(MarkdownPlusNewListItemBelow)")
      vim.keymap.set("n", "O", "<Plug>(MarkdownPlusNewListItemAbove)")
      vim.keymap.set("n", "<localleader>r", "<Plug>(MarkdownPlusRenumberLists)")
      vim.keymap.set("n", "<localleader>d", "<Plug>(MarkdownPlusDebugLists)")
      vim.keymap.set("n", "<localleader>X", "<Plug>(MarkdownPlusToggleCheckbox)")

      -- Visual mode:
      vim.keymap.set("x", "<localleader>mx", "<Plug>(MarkdownPlusToggleCheckbox)")

      -- Quotes Management
      -----------------------
      vim.keymap.set("n", "<localleader>mq", "<Plug>(MarkdownPlusToggleQuote)")
      vim.keymap.set("x", "<localleader>mq", "<Plug>(MarkdownPlusToggleQuote)")

      -- Callouts
      -----------------------
      vim.keymap.set("n", "<localleader>mQi", "<Plug>(MarkdownPlusInsertCallout)")
      vim.keymap.set("x", "<localleader>mQi", "<Plug>(MarkdownPlusInsertCallout)")
      vim.keymap.set("n", "<localleader>mQt", "<Plug>(MarkdownPlusToggleCalloutType)")
      vim.keymap.set("n", "<localleader>mQc", "<Plug>(MarkdownPlusConvertToCallout)")
      vim.keymap.set("n", "<localleader>mQb", "<Plug>(MarkdownPlusConvertToBlockquote)")

      -- Footnotes
      -----------------------
      vim.keymap.set("n", "<localleader>fi", "<Plug>(MarkdownPlusFootnoteInsert)")
      vim.keymap.set("n", "<localleader>fe", "<Plug>(MarkdownPlusFootnoteEdit)")
      vim.keymap.set("n", "<localleader>fd", "<Plug>(MarkdownPlusFootnoteDelete)")
      vim.keymap.set("n", "<localleader>fg", "<Plug>(MarkdownPlusFootnoteGotoDefinition)")
      vim.keymap.set("n", "<localleader>fr", "<Plug>(MarkdownPlusFootnoteGotoReference)")
      vim.keymap.set("n", "<localleader>fn", "<Plug>(MarkdownPlusFootnoteNext)")
      vim.keymap.set("n", "<localleader>fp", "<Plug>(MarkdownPlusFootnotePrev)")
      vim.keymap.set("n", "<localleader>fl", "<Plug>(MarkdownPlusFootnoteList)")

      -- Tables
      -----------------------
      vim.keymap.set("n", "<localleader>tc", "<Plug>(markdown-plus-table-create)", { buffer = false })
      vim.keymap.set("n", "<localleader>tf", "<Plug>(markdown-plus-table-format)", { buffer = false })
      vim.keymap.set("n", "<localleader>tn", "<Plug>(markdown-plus-table-normalize)", { buffer = false })

      -- Row operations.
      vim.keymap.set("n", "<localleader>tir", "<Plug>(markdown-plus-table-insert-row-below)", { buffer = false })
      vim.keymap.set("n", "<localleader>tiR", "<Plug>(markdown-plus-table-insert-row-above)", { buffer = false })
      vim.keymap.set("n", "<localleader>tdr", "<Plug>(markdown-plus-table-delete-row)", { buffer = false })
      vim.keymap.set("n", "<localleader>tyr", "<Plug>(markdown-plus-table-duplicate-row)", { buffer = false })
      vim.keymap.set("n", "<localleader>tk", "<Plug>(markdown-plus-table-move-row-up)", { buffer = false })
      vim.keymap.set("n", "<localleader>tj", "<Plug>(markdown-plus-table-move-row-down)", { buffer = false })

      -- Column operations.
      vim.keymap.set("n", "<localleader>tic", "<Plug>(markdown-plus-table-insert-column-right)", { buffer = false })
      vim.keymap.set("n", "<localleader>tiC", "<Plug>(markdown-plus-table-insert-column-left)", { buffer = false })
      vim.keymap.set("n", "<localleader>tdc", "<Plug>(markdown-plus-table-delete-column)", { buffer = false })
      vim.keymap.set("n", "<localleader>tyc", "<Plug>(markdown-plus-table-duplicate-column)", { buffer = false })
      vim.keymap.set("n", "<localleader>tmh", "<Plug>(markdown-plus-table-move-column-left)", { buffer = false })
      vim.keymap.set("n", "<localleader>tml", "<Plug>(markdown-plus-table-move-column-right)", { buffer = false })

      -- Cell operations.
      vim.keymap.set("n", "<localleader>ta", "<Plug>(markdown-plus-table-toggle-cell-alignment)", { buffer = true })
      vim.keymap.set("n", "<localleader>tx", "<Plug>(markdown-plus-table-clear-cell)", { buffer = false })

      -- Sort operations.
      vim.keymap.set("n", "<localleader>tt", "<Plug>(markdown-plus-table-transpose)", { buffer = false })
      vim.keymap.set("n", "<localleader>tsa", "<Plug>(markdown-plus-table-sort-ascending)", { buffer = false })
      vim.keymap.set("n", "<localleader>tsd", "<Plug>(markdown-plus-table-sort-descending)", { buffer = false })

      -- CSV <--> Table:
      vim.keymap.set("n", "<localleader>tvx", "<Plug>(markdown-plus-table-to-csv)", { buffer = false })
      vim.keymap.set("n", "<localleader>tvi", "<Plug>(markdown-plus-table-from-csv)", { buffer = false })
    end,
  },
  {
    "OXY2DEV/markview.nvim",
    dependencies = {
      "folke/snacks.nvim",
    },
    lazy = false,
    config = function()
      local markview = require("markview")

      -- Integrate advanced code editor feature.
      require("markview.extras.editor").setup()

      local snacks = require("snacks")

      local markview_global_toggle = snacks.toggle({
        name = "Markview (global)",
        get = function()
          local state = require("markview.state")
          local attached = state.get_attached_buffers()
          if #attached == 0 then
            return state.get_buffer_state(-1, true).enable
          else
            return state.get_buffer_state(attached[1], false).enable
          end
        end,
        set = function(state)
          if state then
            markview.commands.Enable()
          else
            markview.commands.Disable()
          end
        end,
      })
      markview_global_toggle:map("<leader>uu")

      local markview_buffer_toggle = snacks.toggle({
        name = "Markview (buffer)",
        get = function()
          local bufnr = vim.api.nvim_get_current_buf()
          local buf_state = require("markview.state").vars.buffer_states[bufnr]
          return buf_state and buf_state.enable == true
        end,
        set = function(state)
          if state then
            markview.commands.enable()
          else
            markview.commands.disable()
          end
        end,
      })
      markview_buffer_toggle:map("<leader>uU")

      map({
        mode = "n",
        lhs = "<localleader>x",
        rhs = function()
          local bufnr = vim.api.nvim_get_current_buf()
          local row, _ = unpack(vim.api.nvim_win_get_cursor(0)) -- 1-based row
          local lines = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)

          -- 1. Start row is always the current line (checkbox line)
          local start_row = row

          -- 2. End row expands downward while lines are indented
          local end_row = row
          while end_row < #lines and lines[end_row + 1]:match("^%s") do
            end_row = end_row + 1
          end

          local first_line = lines[start_row]

          -- 3. Detect checkbox state on first line of task.
          local unchecked = first_line:match("^%s*%- %[ %]") ~= nil
          local checked = first_line:match("^%s*%- %[[xX]%]") ~= nil

          -- 4. Toggle the checkbox.
          local new_checked
          if unchecked then
            lines[start_row] = first_line:gsub("^(%s*%-+%s*)%[ %]", "%1[x]") -- check
            new_checked = true
          elseif checked then
            lines[start_row] = first_line:gsub("^(%s*%-+%s*)%[[xX]%]", "%1[ ]") -- uncheck
            new_checked = false
          else
            vim.notify("Not a valid Markdown task", vim.log.levels.WARN, { title = "Markview" })
            return
          end

          -- 5. Handle the completion comment on the last line.
          local last_line = lines[end_row]
          local comment_pattern = "%s*<!%-%- completed: %d%d%d%d%-%d%d%-%d%d %-%->"
          local comment = " <!-- completed: " .. os.date("%Y-%m-%d") .. " -->"

          if new_checked then
            if not last_line:match(comment_pattern) then
              lines[end_row] = last_line .. comment
            end
          else
            lines[end_row] = last_line:gsub(comment_pattern, "")
          end

          -- 6. Write back updated lines to buffer.
          -- stylua: ignore
          vim.api.nvim_buf_set_lines(
            bufnr,
            start_row - 1,
            end_row,
            false,
            vim.list_slice(lines, start_row, end_row)
          )
        end,
        desc = "Markdown: toggle checkbox (postfix completion date)",
      })
    end,
  },
  {
    "iamcco/markdown-preview.nvim",
    cmd = { "MarkdownPreviewToggle", "MarkdownPreview", "MarkdownPreviewStop" },
    ft = { "markdown" },
    build = "cd ~/.local/share/nvim/lazy/markdown-preview.nvim/app/ && yarn install",
    keys = {
      {
        "<localleader>s",
        function()
          local fname = vim.fn.expand("%:t")
          vim.cmd("MarkdownPreview")
          vim.notify("Starting Markdown Preview for " .. fname)
        end,
        desc = "Markdown Preview: Start",
        ft = "markdown",
      },
      {
        "<localleader>q",
        function()
          local fname = vim.fn.expand("%:t")
          vim.cmd("MarkdownPreviewStop")
          vim.notify("Stopping Markdown Preview for " .. fname)
        end,
        desc = "Markdown Preview: Stop",
        ft = "markdown",
      },
      {
        "<localleader><space>",
        function()
          local fname = vim.fn.expand("%:t")
          -- Track state in buffer variable
          vim.b.markdown_preview_running = vim.b.markdown_preview_running or false

          if vim.b.markdown_preview_running then
            vim.cmd("MarkdownPreviewStop")
            vim.notify("Stopping Markdown Preview for " .. fname)
            vim.b.markdown_preview_running = false
          else
            vim.cmd("MarkdownPreview")
            vim.notify("Starting Markdown Preview for " .. fname)
            vim.b.markdown_preview_running = true
          end
        end,
        desc = "Markdown Preview: toggle",
        ft = "markdown",
      },
    },
    config = function()
      --
      -- Make the preview server is available to others on my network. (default: 127.0.0.1)
      vim.g.mkdp_open_to_the_world = true
      vim.g.vmt_auto_update_on_save = true
      vim.g.mkdp_echo_preview_url = true
      vim.g.mkdp_open_ip = "dresden.home.webdavis.io"
      vim.g.mkdp_port = "8366"
    end,
  },
}
