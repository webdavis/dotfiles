-- ╭─────────────────────╮
-- │   Default Options   │
-- ╰─────────────────────╯

vim.g.mapleader = " "
vim.g.maplocalleader = "\\"

-- if the completion engine supports the AI source,
-- use that instead of inline suggestions
-- vim.g.ai_cmp = true

-- Set LSP servers to be ignored when used with `util.root.detectors.lsp`
-- for detecting the LSP root
-- vim.g.root_lsp_ignore = { "copilot" }

-- Hide deprecation warnings
vim.g.deprecation_warnings = false

local opt = vim.opt

local textwidth = 95
opt.textwidth = textwidth
opt.colorcolumn = tostring(textwidth)

opt.breakindent = true
opt.showmatch = true
opt.matchtime = 3
opt.softtabstop = 4

opt.backspace = "indent,eol,start"
opt.whichwrap = "b,s,<,>,[,]"
opt.showbreak = "↪ "
opt.listchars = [[tab:>\ ,trail:-,extends:→,precedes:<]]

opt.autowrite = true -- Enable auto write
-- only set clipboard if not in ssh, to make sure the OSC 52
-- integration works automatically.
-- opt.clipboard = vim.env.SSH_TTY and "" or "unnamedplus" -- Sync with system clipboard
opt.completeopt = "menu,menuone,noselect"
opt.conceallevel = 0 -- full visibility
opt.confirm = true -- Confirm to save changes before exiting modified buffer
opt.cursorline = true -- Enable highlighting of the current line
opt.cursorcolumn = true -- Enable highlighting of the current line
opt.expandtab = true -- Use spaces instead of tabs
opt.fillchars = {
  foldopen = "",
  foldclose = "",
  -- fold = " ",
  fold = "—",
  foldsep = " ",
  diff = "╱",
  eob = " ",
  vert = "│",
}
opt.directory = "."
opt.foldlevel = 99
opt.foldmethod = "indent"
opt.foldtext = ""
opt.formatoptions = "jcroqlnt" -- tcqj
opt.grepformat = "%f:%l:%c:%m"
opt.grepprg = "rg --vimgrep"
opt.ignorecase = true -- Ignore case
opt.inccommand = "nosplit" -- preview incremental substitute
opt.jumpoptions = "view"
opt.laststatus = 3 -- global statusline
opt.linebreak = true -- Wrap lines at convenient points
opt.list = true -- Show some invisible characters (tabs...
opt.mouse = "a" -- Enable mouse mode
opt.number = true -- Print line number
opt.pumblend = 10 -- Popup blend
opt.pumheight = 10 -- Maximum number of entries in a popup
opt.relativenumber = true -- Relative line numbers
opt.ruler = false -- Disable the default ruler
opt.scrolloff = 4 -- Lines of context
opt.sessionoptions = { "buffers", "curdir", "tabpages", "winsize", "help", "globals", "skiprtp", "folds" }
opt.shiftround = true -- Round indent
opt.shiftwidth = 2 -- Size of an indent
opt.shortmess:append({ W = true, I = true, c = true, C = true })
opt.sidescrolloff = 8 -- Columns of context
opt.signcolumn = "yes" -- Always show the signcolumn, otherwise it would shift the text each time
opt.smartcase = true -- Don't ignore case with capitals
opt.smartindent = true -- Insert indents automatically
opt.smoothscroll = true
opt.spelllang = { "en" }
opt.splitbelow = true -- Put new windows below current
opt.splitkeep = "screen"
opt.splitright = true -- Put new windows right of current
opt.showmode = false -- Dont show mode since we have a statusline
opt.tabstop = 2 -- Number of spaces tabs count for
opt.termguicolors = true -- True color support
opt.timeoutlen = vim.g.vscode and 1000 or 3000 -- Lower the default (1000) to quickly trigger which-key
opt.undofile = true
opt.undolevels = 10000
opt.updatetime = 200 -- Save swap file and trigger CursorHold
opt.virtualedit = "block" -- Allow cursor to move where there is no text in visual block mode
opt.wildmode = "longest:full,full" -- Command-line completion mode
opt.winminwidth = 5 -- Minimum window width
opt.wrap = false -- Disable line wrap
opt.exrc = true -- Turn on directory-specific Neovim settings.

-- Fix markdown indentation settings
vim.g.markdown_recommended_style = 0

-- See :help fo-table
local custom_formatoptions_group = vim.api.nvim_create_augroup("custom_settings_fo", { clear = true })

vim.api.nvim_create_autocmd("BufEnter", {
  group = custom_formatoptions_group,
  pattern = "*",
  callback = function()
    vim.opt.formatoptions:remove("o")
  end,
})

opt.autoread = true
vim.api.nvim_create_autocmd({ "FocusGained", "BufEnter", "CursorHold" }, {
  command = "checktime",
})
