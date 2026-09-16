---@class markdown_plus_keymap
---@field mode string
---@field lhs string
---@field rhs string The `<Plug>` target markdown-plus registers globally.

---Every mapping this configuration adds on top of markdown-plus.nvim, applied
---buffer-locally to each markdown buffer by the `FileType` autocmd in `config`
---below. Buffer-local is not a preference: ten of these shadow a Vim built-in
---(`gd`, `]]`, `[[`, `o`, `O` and insert-mode `<CR>`, `<Tab>`, `<S-Tab>`, `<BS>`
---and `<C-t>`), so a global copy replaced those motions in every other buffer
---for the rest of the session once a single markdown file had been opened. The
---`<Plug>` targets are registered globally by the plugin, which is what lets a
---buffer-local left-hand side reach them.
---@type markdown_plus_keymap[]
local markdown_plus_keymaps = {
  -- Text Formatting
  -----------------------
  -- Normal mode:
  { mode = "n", lhs = "<localleader>mb", rhs = "<Plug>(MarkdownPlusBold)" },
  { mode = "n", lhs = "<localleader>mi", rhs = "<Plug>(MarkdownPlusItalic)" },
  { mode = "n", lhs = "<localleader>ms", rhs = "<Plug>(MarkdownPlusStrikethrough)" },
  { mode = "n", lhs = "<localleader>mc", rhs = "<Plug>(MarkdownPlusCode)" },
  { mode = "n", lhs = "<localleader>mw", rhs = "<Plug>(MarkdownPlusCodeBlock)" },
  { mode = "n", lhs = "<localleader>mC", rhs = "<Plug>(MarkdownPlusClearFormatting)" },

  -- Visual mode:
  { mode = "x", lhs = "<localleader>mb", rhs = "<Plug>(MarkdownPlusBold)" },
  { mode = "x", lhs = "<localleader>mi", rhs = "<Plug>(MarkdownPlusItalic)" },
  { mode = "x", lhs = "<localleader>ms", rhs = "<Plug>(MarkdownPlusStrikethrough)" },
  { mode = "x", lhs = "<localleader>mc", rhs = "<Plug>(MarkdownPlusCode)" },
  { mode = "x", lhs = "<localleader>mw", rhs = "<Plug>(MarkdownPlusCodeBlock)" },
  { mode = "x", lhs = "<localleader>mC", rhs = "<Plug>(MarkdownPlusClearFormatting)" },

  -- Headers
  -----------------------
  { mode = "n", lhs = "]]", rhs = "<Plug>(MarkdownPlusNextHeader)" },
  { mode = "n", lhs = "[[", rhs = "<Plug>(MarkdownPlusPrevHeader)" },
  { mode = "n", lhs = "<localleader>h+", rhs = "<Plug>(MarkdownPlusPromoteHeader)" },
  { mode = "n", lhs = "<localleader>h-", rhs = "<Plug>(MarkdownPlusDemoteHeader)" },
  { mode = "n", lhs = "<localleader>hT", rhs = "<Plug>(MarkdownPlusOpenTocWindow)" },
  { mode = "n", lhs = "gd", rhs = "<Plug>(MarkdownPlusFollowLink)" },

  -- Table of Contents
  -----------------------
  { mode = "n", lhs = "<localleader>ht", rhs = "<Plug>(MarkdownPlusGenerateTOC)" },
  { mode = "n", lhs = "<localleader>hu", rhs = "<Plug>(MarkdownPlusUpdateTOC)" },

  -- Links & References
  -----------------------
  { mode = "n", lhs = "<localleader>l", rhs = "<Plug>(MarkdownPlusInsertLink)" },
  { mode = "v", lhs = "<localleader>l", rhs = "<Plug>(MarkdownPlusSelectionToLink)" },
  { mode = "n", lhs = "<localleader>e", rhs = "<Plug>(MarkdownPlusEditLink)" },
  { mode = "n", lhs = "<localleader>a", rhs = "<Plug>(MarkdownPlusAutoLinkURL)" },
  { mode = "n", lhs = "<localleader>R", rhs = "<Plug>(MarkdownPlusConvertToReference)" },
  { mode = "n", lhs = "<localleader>I", rhs = "<Plug>(MarkdownPlusConvertToInline)" },

  -- Image Links
  -----------------------
  { mode = "n", lhs = "<localleader>L", rhs = "<Plug>(MarkdownPlusInsertImage)" },
  { mode = "v", lhs = "<localleader>L", rhs = "<Plug>(MarkdownPlusSelectionToImage)" },
  { mode = "n", lhs = "<localleader>E", rhs = "<Plug>(MarkdownPlusEditImage)" },
  { mode = "n", lhs = "<localleader>A", rhs = "<Plug>(MarkdownPlusToggleImageLink)" },

  -- List Management
  -----------------------
  -- Insert mode:
  { mode = "i", lhs = "<CR>", rhs = "<Plug>(MarkdownPlusListEnter)" },
  { mode = "i", lhs = "<A-CR>", rhs = "<Plug>(MarkdownPlusListShiftEnter)" },
  { mode = "i", lhs = "<Tab>", rhs = "<Plug>(MarkdownPlusListIndent)" },
  { mode = "i", lhs = "<S-Tab>", rhs = "<Plug>(MarkdownPlusListOutdent)" },
  { mode = "i", lhs = "<BS>", rhs = "<Plug>(MarkdownPlusListBackspace)" },
  { mode = "i", lhs = "<C-t>", rhs = "<Plug>(MarkdownPlusToggleCheckbox)" },

  -- Normal mode:
  { mode = "n", lhs = "o", rhs = "<Plug>(MarkdownPlusNewListItemBelow)" },
  { mode = "n", lhs = "O", rhs = "<Plug>(MarkdownPlusNewListItemAbove)" },
  { mode = "n", lhs = "<localleader>r", rhs = "<Plug>(MarkdownPlusRenumberLists)" },
  { mode = "n", lhs = "<localleader>d", rhs = "<Plug>(MarkdownPlusDebugLists)" },
  { mode = "n", lhs = "<localleader>X", rhs = "<Plug>(MarkdownPlusToggleCheckbox)" },

  -- Visual mode:
  { mode = "x", lhs = "<localleader>mx", rhs = "<Plug>(MarkdownPlusToggleCheckbox)" },

  -- Quotes Management
  -----------------------
  { mode = "n", lhs = "<localleader>mq", rhs = "<Plug>(MarkdownPlusToggleQuote)" },
  { mode = "x", lhs = "<localleader>mq", rhs = "<Plug>(MarkdownPlusToggleQuote)" },

  -- Callouts
  -----------------------
  { mode = "n", lhs = "<localleader>mQi", rhs = "<Plug>(MarkdownPlusInsertCallout)" },
  { mode = "x", lhs = "<localleader>mQi", rhs = "<Plug>(MarkdownPlusInsertCallout)" },
  { mode = "n", lhs = "<localleader>mQt", rhs = "<Plug>(MarkdownPlusToggleCalloutType)" },
  { mode = "n", lhs = "<localleader>mQc", rhs = "<Plug>(MarkdownPlusConvertToCallout)" },
  { mode = "n", lhs = "<localleader>mQb", rhs = "<Plug>(MarkdownPlusConvertToBlockquote)" },

  -- Footnotes
  -----------------------
  { mode = "n", lhs = "<localleader>fi", rhs = "<Plug>(MarkdownPlusFootnoteInsert)" },
  { mode = "n", lhs = "<localleader>fe", rhs = "<Plug>(MarkdownPlusFootnoteEdit)" },
  { mode = "n", lhs = "<localleader>fd", rhs = "<Plug>(MarkdownPlusFootnoteDelete)" },
  { mode = "n", lhs = "<localleader>fg", rhs = "<Plug>(MarkdownPlusFootnoteGotoDefinition)" },
  { mode = "n", lhs = "<localleader>fr", rhs = "<Plug>(MarkdownPlusFootnoteGotoReference)" },
  { mode = "n", lhs = "<localleader>fn", rhs = "<Plug>(MarkdownPlusFootnoteNext)" },
  { mode = "n", lhs = "<localleader>fp", rhs = "<Plug>(MarkdownPlusFootnotePrev)" },
  { mode = "n", lhs = "<localleader>fl", rhs = "<Plug>(MarkdownPlusFootnoteList)" },

  -- Tables
  -----------------------
  { mode = "n", lhs = "<localleader>tc", rhs = "<Plug>(markdown-plus-table-create)" },
  { mode = "n", lhs = "<localleader>tf", rhs = "<Plug>(markdown-plus-table-format)" },
  { mode = "n", lhs = "<localleader>tn", rhs = "<Plug>(markdown-plus-table-normalize)" },

  -- Row operations.
  { mode = "n", lhs = "<localleader>tir", rhs = "<Plug>(markdown-plus-table-insert-row-below)" },
  { mode = "n", lhs = "<localleader>tiR", rhs = "<Plug>(markdown-plus-table-insert-row-above)" },
  { mode = "n", lhs = "<localleader>tdr", rhs = "<Plug>(markdown-plus-table-delete-row)" },
  { mode = "n", lhs = "<localleader>tyr", rhs = "<Plug>(markdown-plus-table-duplicate-row)" },
  { mode = "n", lhs = "<localleader>tk", rhs = "<Plug>(markdown-plus-table-move-row-up)" },
  { mode = "n", lhs = "<localleader>tj", rhs = "<Plug>(markdown-plus-table-move-row-down)" },

  -- Column operations.
  { mode = "n", lhs = "<localleader>tic", rhs = "<Plug>(markdown-plus-table-insert-column-right)" },
  { mode = "n", lhs = "<localleader>tiC", rhs = "<Plug>(markdown-plus-table-insert-column-left)" },
  { mode = "n", lhs = "<localleader>tdc", rhs = "<Plug>(markdown-plus-table-delete-column)" },
  { mode = "n", lhs = "<localleader>tyc", rhs = "<Plug>(markdown-plus-table-duplicate-column)" },
  { mode = "n", lhs = "<localleader>tmh", rhs = "<Plug>(markdown-plus-table-move-column-left)" },
  { mode = "n", lhs = "<localleader>tml", rhs = "<Plug>(markdown-plus-table-move-column-right)" },

  -- Cell operations.
  { mode = "n", lhs = "<localleader>ta", rhs = "<Plug>(markdown-plus-table-toggle-cell-alignment)" },
  { mode = "n", lhs = "<localleader>tx", rhs = "<Plug>(markdown-plus-table-clear-cell)" },

  -- Sort operations.
  { mode = "n", lhs = "<localleader>tt", rhs = "<Plug>(markdown-plus-table-transpose)" },
  { mode = "n", lhs = "<localleader>tsa", rhs = "<Plug>(markdown-plus-table-sort-ascending)" },
  { mode = "n", lhs = "<localleader>tsd", rhs = "<Plug>(markdown-plus-table-sort-descending)" },

  -- CSV <--> Table:
  { mode = "n", lhs = "<localleader>tvx", rhs = "<Plug>(markdown-plus-table-to-csv)" },
  { mode = "n", lhs = "<localleader>tvi", rhs = "<Plug>(markdown-plus-table-from-csv)" },
}

-- Header levels 1-6.
for level = 1, 6 do
  table.insert(markdown_plus_keymaps, {
    mode = "n",
    lhs = "<localleader>h" .. level,
    rhs = "<Plug>(MarkdownPlusHeader" .. level .. ")",
  })
end

---Apply every mapping above to the current buffer.
---@return nil
local function set_markdown_plus_keymaps()
  for _, keymap in ipairs(markdown_plus_keymaps) do
    vim.keymap.set(keymap.mode, keymap.lhs, keymap.rhs, { buffer = true })
  end
end

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

      -- Registered after the plugin's own `MarkdownPlus` group, so these keys
      -- win over its defaults on the ones where this configuration chose a
      -- different left-hand side. lazy.nvim re-fires `FileType` after loading an
      -- `ft` spec, so the buffer that triggered the load is covered too.
      vim.api.nvim_create_autocmd("FileType", {
        group = vim.api.nvim_create_augroup("MarkdownPlusUserKeymaps", { clear = true }),
        pattern = "markdown",
        callback = set_markdown_plus_keymaps,
      })
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
      -- Reachable from the rest of the network as <this host>.$NVIM_MKDP_HOST; loopback when unset.
      vim.g.mkdp_open_ip = require("custom_api.preview_host").resolve(vim.fn.hostname(), vim.env.NVIM_MKDP_HOST)
      vim.g.mkdp_port = "8366"
    end,
  },
}
