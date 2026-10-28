---@class markdown_plus_keymap
---@field mode string
---@field lhs string
---@field rhs string The `<Plug>` target markdown-plus registers globally.

---Buffer-local, because many of these shadow Vim built-ins such as `gd`, `]]`, `o` and insert-mode `<CR>`.
---@type markdown_plus_keymap[]
local markdown_plus_keymaps = {
  { mode = "n", lhs = "<localleader>mb", rhs = "<Plug>(MarkdownPlusBold)" },
  { mode = "n", lhs = "<localleader>mi", rhs = "<Plug>(MarkdownPlusItalic)" },
  { mode = "n", lhs = "<localleader>ms", rhs = "<Plug>(MarkdownPlusStrikethrough)" },
  { mode = "n", lhs = "<localleader>mc", rhs = "<Plug>(MarkdownPlusCode)" },
  { mode = "n", lhs = "<localleader>mw", rhs = "<Plug>(MarkdownPlusCodeBlock)" },
  { mode = "n", lhs = "<localleader>mC", rhs = "<Plug>(MarkdownPlusClearFormatting)" },

  { mode = "x", lhs = "<localleader>mb", rhs = "<Plug>(MarkdownPlusBold)" },
  { mode = "x", lhs = "<localleader>mi", rhs = "<Plug>(MarkdownPlusItalic)" },
  { mode = "x", lhs = "<localleader>ms", rhs = "<Plug>(MarkdownPlusStrikethrough)" },
  { mode = "x", lhs = "<localleader>mc", rhs = "<Plug>(MarkdownPlusCode)" },
  { mode = "x", lhs = "<localleader>mw", rhs = "<Plug>(MarkdownPlusCodeBlock)" },
  { mode = "x", lhs = "<localleader>mC", rhs = "<Plug>(MarkdownPlusClearFormatting)" },

  { mode = "n", lhs = "]]", rhs = "<Plug>(MarkdownPlusNextHeader)" },
  { mode = "n", lhs = "[[", rhs = "<Plug>(MarkdownPlusPrevHeader)" },
  { mode = "n", lhs = "<localleader>h+", rhs = "<Plug>(MarkdownPlusPromoteHeader)" },
  { mode = "n", lhs = "<localleader>h-", rhs = "<Plug>(MarkdownPlusDemoteHeader)" },
  { mode = "n", lhs = "<localleader>hT", rhs = "<Plug>(MarkdownPlusOpenTocWindow)" },
  { mode = "n", lhs = "gd", rhs = "<Plug>(MarkdownPlusFollowLink)" },

  { mode = "n", lhs = "<localleader>ht", rhs = "<Plug>(MarkdownPlusGenerateTOC)" },
  { mode = "n", lhs = "<localleader>hu", rhs = "<Plug>(MarkdownPlusUpdateTOC)" },

  { mode = "n", lhs = "<localleader>l", rhs = "<Plug>(MarkdownPlusInsertLink)" },
  { mode = "v", lhs = "<localleader>l", rhs = "<Plug>(MarkdownPlusSelectionToLink)" },
  { mode = "n", lhs = "<localleader>e", rhs = "<Plug>(MarkdownPlusEditLink)" },
  { mode = "n", lhs = "<localleader>a", rhs = "<Plug>(MarkdownPlusAutoLinkURL)" },
  { mode = "n", lhs = "<localleader>R", rhs = "<Plug>(MarkdownPlusConvertToReference)" },
  { mode = "n", lhs = "<localleader>I", rhs = "<Plug>(MarkdownPlusConvertToInline)" },

  { mode = "n", lhs = "<localleader>L", rhs = "<Plug>(MarkdownPlusInsertImage)" },
  { mode = "v", lhs = "<localleader>L", rhs = "<Plug>(MarkdownPlusSelectionToImage)" },
  { mode = "n", lhs = "<localleader>E", rhs = "<Plug>(MarkdownPlusEditImage)" },
  { mode = "n", lhs = "<localleader>A", rhs = "<Plug>(MarkdownPlusToggleImageLink)" },

  { mode = "i", lhs = "<CR>", rhs = "<Plug>(MarkdownPlusListEnter)" },
  { mode = "i", lhs = "<A-CR>", rhs = "<Plug>(MarkdownPlusListShiftEnter)" },
  { mode = "i", lhs = "<Tab>", rhs = "<Plug>(MarkdownPlusListIndent)" },
  { mode = "i", lhs = "<S-Tab>", rhs = "<Plug>(MarkdownPlusListOutdent)" },
  { mode = "i", lhs = "<BS>", rhs = "<Plug>(MarkdownPlusListBackspace)" },
  { mode = "i", lhs = "<C-t>", rhs = "<Plug>(MarkdownPlusToggleCheckbox)" },

  { mode = "n", lhs = "o", rhs = "<Plug>(MarkdownPlusNewListItemBelow)" },
  { mode = "n", lhs = "O", rhs = "<Plug>(MarkdownPlusNewListItemAbove)" },
  { mode = "n", lhs = "<localleader>r", rhs = "<Plug>(MarkdownPlusRenumberLists)" },
  { mode = "n", lhs = "<localleader>d", rhs = "<Plug>(MarkdownPlusDebugLists)" },
  { mode = "n", lhs = "<localleader>X", rhs = "<Plug>(MarkdownPlusToggleCheckbox)" },

  { mode = "x", lhs = "<localleader>mx", rhs = "<Plug>(MarkdownPlusToggleCheckbox)" },

  { mode = "n", lhs = "<localleader>mq", rhs = "<Plug>(MarkdownPlusToggleQuote)" },
  { mode = "x", lhs = "<localleader>mq", rhs = "<Plug>(MarkdownPlusToggleQuote)" },

  { mode = "n", lhs = "<localleader>mQi", rhs = "<Plug>(MarkdownPlusInsertCallout)" },
  { mode = "x", lhs = "<localleader>mQi", rhs = "<Plug>(MarkdownPlusInsertCallout)" },
  { mode = "n", lhs = "<localleader>mQt", rhs = "<Plug>(MarkdownPlusToggleCalloutType)" },
  { mode = "n", lhs = "<localleader>mQc", rhs = "<Plug>(MarkdownPlusConvertToCallout)" },
  { mode = "n", lhs = "<localleader>mQb", rhs = "<Plug>(MarkdownPlusConvertToBlockquote)" },

  { mode = "n", lhs = "<localleader>fi", rhs = "<Plug>(MarkdownPlusFootnoteInsert)" },
  { mode = "n", lhs = "<localleader>fe", rhs = "<Plug>(MarkdownPlusFootnoteEdit)" },
  { mode = "n", lhs = "<localleader>fd", rhs = "<Plug>(MarkdownPlusFootnoteDelete)" },
  { mode = "n", lhs = "<localleader>fg", rhs = "<Plug>(MarkdownPlusFootnoteGotoDefinition)" },
  { mode = "n", lhs = "<localleader>fr", rhs = "<Plug>(MarkdownPlusFootnoteGotoReference)" },
  { mode = "n", lhs = "<localleader>fn", rhs = "<Plug>(MarkdownPlusFootnoteNext)" },
  { mode = "n", lhs = "<localleader>fp", rhs = "<Plug>(MarkdownPlusFootnotePrev)" },
  { mode = "n", lhs = "<localleader>fl", rhs = "<Plug>(MarkdownPlusFootnoteList)" },
}

for level = 1, 6 do
  table.insert(markdown_plus_keymaps, {
    mode = "n",
    lhs = "<localleader>h" .. level,
    rhs = "<Plug>(MarkdownPlusHeader" .. level .. ")",
  })
end

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

      -- Created after the plugin's own group, so these keys win over its defaults.
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

          local start_row = row

          local end_row = row
          while end_row < #lines and lines[end_row + 1]:match("^%s") do
            end_row = end_row + 1
          end

          local first_line = lines[start_row]

          local unchecked = first_line:match("^%s*%- %[ %]") ~= nil
          local checked = first_line:match("^%s*%- %[[xX]%]") ~= nil

          local new_checked
          if unchecked then
            lines[start_row] = first_line:gsub("^(%s*%-+%s*)%[ %]", "%1[x]")
            new_checked = true
          elseif checked then
            lines[start_row] = first_line:gsub("^(%s*%-+%s*)%[[xX]%]", "%1[ ]")
            new_checked = false
          else
            vim.notify("Not a valid Markdown task", vim.log.levels.WARN, { title = "Markview" })
            return
          end

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
      vim.g.mkdp_open_to_the_world = true
      vim.g.vmt_auto_update_on_save = true
      vim.g.mkdp_echo_preview_url = true
      -- Other machines reach the preview at this host's name under $NVIM_MKDP_HOST.
      -- Without that variable, only this machine can open it.
      vim.g.mkdp_open_ip = require("custom_api.preview_host").resolve(vim.fn.hostname(), vim.env.NVIM_MKDP_HOST)
      vim.g.mkdp_port = "8366"
    end,
  },
}
