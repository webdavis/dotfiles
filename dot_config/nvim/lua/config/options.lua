vim.g.mapleader = " "
vim.g.maplocalleader = "\\"

local opt = vim.opt

local textwidth = 95
opt.textwidth = textwidth
opt.colorcolumn = tostring(textwidth)

opt.breakindent = true
opt.showmatch = true
opt.matchtime = 3
opt.softtabstop = 2

opt.backspace = "indent,eol,start"
opt.whichwrap = "b,s,<,>,[,]"
opt.showbreak = "↪ "
opt.listchars = [[tab:>\ ,trail:-,extends:→,precedes:<]]

opt.autowrite = true
opt.completeopt = "menu,menuone,noselect"
opt.conceallevel = 0
opt.confirm = true
opt.cursorline = true
opt.cursorcolumn = true
opt.expandtab = true
opt.fillchars = {
  foldopen = "",
  foldclose = "",
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
opt.formatoptions = "jcroqlnt"
opt.grepformat = "%f:%l:%c:%m"
opt.grepprg = "rg --vimgrep"
opt.ignorecase = true
opt.inccommand = "nosplit"
opt.jumpoptions = "view"
opt.laststatus = 3
opt.linebreak = true
opt.list = true
opt.mouse = "a"
opt.number = true
opt.pumblend = 10
opt.pumheight = 10
opt.relativenumber = true
opt.ruler = false
opt.scrolloff = 4
opt.sessionoptions = { "buffers", "curdir", "tabpages", "winsize", "help", "globals", "skiprtp", "folds" }
opt.shiftround = true
opt.shiftwidth = 2
opt.shortmess:append({ W = true, I = true, c = true, C = true })
opt.sidescrolloff = 8
opt.signcolumn = "yes" -- Keep the sign column so text never shifts
opt.smartcase = true
opt.smartindent = true
opt.smoothscroll = true
opt.spelllang = { "en" }
opt.splitbelow = true
opt.splitkeep = "screen"
opt.splitright = true
opt.showmode = false
opt.tabstop = 2
opt.termguicolors = true
opt.timeoutlen = vim.g.vscode and 1000 or 3000
opt.undofile = true
opt.undolevels = 10000
opt.updatetime = 200
opt.virtualedit = "block"
opt.wildmode = "longest:full,full"
opt.winborder = "rounded" -- Applies to every floating window, so no plugin sets its own border
opt.pumborder = "rounded"
opt.winminwidth = 5
opt.wrap = false
opt.exrc = true

-- Keep the Markdown file-type plugin from forcing 4-space indents.
vim.g.markdown_recommended_style = 0

local custom_formatoptions_group = vim.api.nvim_create_augroup("custom_settings_fo", { clear = true })

vim.api.nvim_create_autocmd("BufEnter", {
  group = custom_formatoptions_group,
  pattern = "*",
  callback = function()
    vim.opt.formatoptions:remove("o")
  end,
})

opt.autoread = true
