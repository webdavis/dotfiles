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

  -- Normal mode:
  { mode = "n", lhs = "o", rhs = "<Plug>(MarkdownPlusNewListItemBelow)" },
  { mode = "n", lhs = "O", rhs = "<Plug>(MarkdownPlusNewListItemAbove)" },
  { mode = "n", lhs = "<localleader>r", rhs = "<Plug>(MarkdownPlusRenumberLists)" },
  { mode = "n", lhs = "<localleader>d", rhs = "<Plug>(MarkdownPlusDebugLists)" },

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

  -- custom_api.markdown_task is the only task toggle, so the plugin's own
  -- defaults for it go: `<C-t>` belongs to another tool.
  for _, default in ipairs({ { "i", "<C-t>" }, { "n", "<localleader>mx" }, { "x", "<localleader>mx" } }) do
    pcall(vim.keymap.del, default[1], default[2], { buffer = true })
  end
  vim.keymap.set({ "n", "i" }, "<C-CR>", function()
    require("custom_api.markdown_task").apply()
  end, { buffer = true, desc = "Markdown: toggle task" })
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
    end,
  },
}
