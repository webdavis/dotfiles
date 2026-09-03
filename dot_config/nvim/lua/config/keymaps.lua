-- Keymaps

-- ┏━━━━━━━━━━━━━━━━━┓
-- ┃    Delegates    ┃
-- ┗━━━━━━━━━━━━━━━━━┛
require("custom_api.delegate").setup()

map({
  mode = "n",
  lhs = "<leader>dx",
  rhs = function()
    local file = vim.fn.fnamemodify(vim.fn.expand("%"), ":.")
    -- argv, not `:!`, which hands the path to the shell: a tracked file named
    -- `a$(...)b` runs the substitution before chmod ever sees it.
    local result = vim.system({ "chmod", "+x", file }):wait()
    if result.code ~= 0 then
      vim.notify("Could not chmod +x *" .. file .. "*: " .. (result.stderr or ""), vim.log.levels.ERROR)
      return
    end
    vim.notify("⚡ Made *" .. file .. "* executable", vim.log.levels.INFO)
  end,
  desc = "Action: chmod +x %>",
})

map({
  mode = "n",
  lhs = "<leader>da",
  rhs = function()
    vim.lsp.buf.code_action()
  end,
  desc = "LSP: code action",
})

-- ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
-- ┃    Faster File Manipulation    ┃
-- ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
map({ mode = "n", lhs = "<C-s>", rhs = "write", desc = "Save file" })

map({
  mode = "n",
  lhs = "<leader>wa",
  rhs = function()
    vim.cmd("wall")
    vim.notify("Saved all files", vim.log.levels.INFO)
  end,
  desc = "Write all files",
})

map({ mode = "n", lhs = '<leader>"', rhs = "new", desc = "Create a new file (split)" })
map({ mode = "n", lhs = "<leader>%", rhs = "vnew", desc = "Create a new file (vsplit)" })
map({ mode = "n", lhs = "<C-q>", rhs = "quit", desc = "Quit: file" })
map({ mode = "n", lhs = "<leader>00", rhs = "quit", desc = "Quit: file" })
map({ mode = "n", lhs = "<leader>0a", rhs = "qa", desc = "Quit: all files" })
map({ mode = "n", lhs = "<leader>0A", rhs = "qa!", desc = "Quit: all files (force)" })

-- ┏━━━━━━━━━━━━┓
-- ┃    Lazy    ┃
-- ┗━━━━━━━━━━━━┛
map({ mode = "n", lhs = "<leader>LL", rhs = "Lazy", desc = "Lazy: open dashboard" })
map({ mode = "n", lhs = "<leader>Lh", rhs = "Lazy health", desc = "Lazy: health" })
map({ mode = "n", lhs = "<leader>Li", rhs = "Lazy install", desc = "Lazy: install" })
map({ mode = "n", lhs = "<leader>Lu", rhs = "Lazy update", desc = "Lazy: update" })
map({ mode = "n", lhs = "<leader>Ls", rhs = "Lazy sync", desc = "Lazy: sync" })
map({ mode = "n", lhs = "<leader>Lx", rhs = "Lazy clean", desc = "Lazy: clean" })
map({ mode = "n", lhs = "<leader>Lc", rhs = "Lazy check", desc = "Lazy: check" })
map({ mode = "n", lhs = "<leader>Ll", rhs = "Lazy log", desc = "Lazy: logs" })
map({ mode = "n", lhs = "<leader>Lr", rhs = "Lazy restore", desc = "Lazy: restore" })
map({ mode = "n", lhs = "<leader>Lp", rhs = "Lazy profile", desc = "Lazy: profile" })
map({ mode = "n", lhs = "<leader>Ld", rhs = "Lazy debug", desc = "Lazy: debug" })

-- ┏━━━━━━━━━━━━┓
-- ┃    Mason   ┃
-- ┗━━━━━━━━━━━━┛
map({ mode = "n", lhs = "<leader>lm", rhs = "Mason", desc = "Mason: open" })

-- ┏━━━━━━━━━━━━━━━━┓
-- ┃    Movement    ┃
-- ┗━━━━━━━━━━━━━━━━┛

-- Better up & down keys when line wraps.
map({ mode = { "n", "x" }, lhs = "j", rhs = "v:count == 0 ? 'gj' : 'j'", expr = true, sequence = true })
map({ mode = { "n", "x" }, lhs = "k", rhs = "v:count == 0 ? 'gk' : 'k'", expr = true, sequence = true })

-- Faster scrolling.
map({ mode = "n", lhs = "<C-e>", rhs = "<C-e><C-e>", desc = "Scroll down (x2)", sequence = true })
map({ mode = "n", lhs = "<C-y>", rhs = "<C-y><C-y>", desc = "Scroll up (x2)", sequence = true })

-- https://github.com/mhinz/vim-galore#saner-behavior-of-n-and-n
map({
  mode = "n",
  lhs = "n",
  rhs = "'Nn'[v:searchforward].'zv'",
  expr = true,
  desc = "Next Search Result",
  sequence = true,
})
map({ mode = "x", lhs = "n", rhs = "'Nn'[v:searchforward]", expr = true, desc = "Next Search Result", sequence = true })
map({ mode = "o", lhs = "n", rhs = "'Nn'[v:searchforward]", expr = true, desc = "Next Search Result", sequence = true })
map({
  mode = "n",
  lhs = "N",
  rhs = "'nN'[v:searchforward].'zv'",
  expr = true,
  desc = "Prev Search Result",
  sequence = true,
})
map({ mode = "x", lhs = "N", rhs = "'nN'[v:searchforward]", expr = true, desc = "Prev Search Result", sequence = true })
map({ mode = "o", lhs = "N", rhs = "'nN'[v:searchforward]", expr = true, desc = "Prev Search Result", sequence = true })

-- ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
-- ┃    Window & Line Editing    ┃
-- ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
map({ mode = "n", lhs = "<C-Up>", rhs = "resize +2", desc = "Increase window Height" })
map({ mode = "n", lhs = "<C-Down>", rhs = "resize -2", desc = "Decrease window Height" })
map({ mode = "n", lhs = "<C-Left>", rhs = "vertical resize -2", desc = "Decrease window Width" })
map({ mode = "n", lhs = "<C-Right>", rhs = "vertical resize +2", desc = "Increase window Width" })

-- Better line shifting.
map({ mode = "v", lhs = "<", rhs = "<gv", desc = "Shift line left (←)", sequence = true })
map({ mode = "v", lhs = ">", rhs = ">gv", desc = "Shift line right (→)", sequence = true })

-- Add undo break-points.
map({ mode = "i", lhs = ",", rhs = ",<c-g>u", sequence = true })
map({ mode = "i", lhs = ".", rhs = ".<c-g>u", sequence = true })
map({ mode = "i", lhs = ";", rhs = ";<c-g>u", sequence = true })

-- keywordprg
map({
  mode = "n",
  lhs = "<leader>K",
  rhs = function()
    vim.cmd("normal! K")
  end,
  desc = "Keywordprg",
})

-- Start a comment on the next or previous line.
map({
  mode = "n",
  lhs = "gco",
  rhs = "o<esc>Vcx<esc><cmd>normal gcc<cr>fxa<bs>",
  desc = "Add Comment Below",
  sequence = true,
})

map({
  mode = "n",
  lhs = "gcO",
  rhs = "O<esc>Vcx<esc><cmd>normal gcc<cr>fxa<bs>",
  desc = "Add Comment Above",
  sequence = true,
})

-- ┏━━━━━━━━━━━━━┓
-- ┃    Debug    ┃
-- ┗━━━━━━━━━━━━━┛
map({
  mode = "n",
  lhs = "<leader>Ds",
  rhs = vim.show_pos,
  desc = "Debug: syntax under cursor",
})

map({
  mode = "n",
  lhs = "<leader>Dt",
  rhs = function()
    vim.treesitter.inspect_tree()
    vim.api.nvim_input("I")
  end,
  desc = "Debug: treesitter syntax tree",
})

-- ┏━━━━━━━━━━━━━━━━━━━━━━━━━━┓
-- ┃    Clipboard Mappings    ┃
-- ┗━━━━━━━━━━━━━━━━━━━━━━━━━━┛
map({
  mode = "n",
  lhs = "Y",
  rhs = '"+yg_',
  desc = "Yank to the end-of-line (without line-ending)",
  sequence = true,
})

map({
  mode = "n",
  lhs = "<leader>yp",
  rhs = '"+yap',
  desc = "Yank current paragraph to clipboard",
  sequence = true,
})

map({
  mode = "v",
  lhs = "<leader>y",
  rhs = [[:<C-u>'<,'>y+<CR>]],
  desc = "Yank selected text to clipboard (keep cursor/window)",
  sequence = true,
})

map({
  mode = "n",
  lhs = "<leader>ya",
  rhs = "%y+",
  desc = "Yank entire buffer to system clipboard",
})

map({
  mode = "n",
  lhs = "<leader>yl",
  rhs = [[yank + | echo '1 line yanked into "+']],
  desc = "Yank current line to clipboard",
})

map({
  mode = "n",
  lhs = "<leader>yf",
  rhs = function()
    local filename = vim.fn.expand("%:t")
    vim.fn.setreg("+", filename)
    vim.notify("Filename (*" .. filename .. "*) yanked to clipboard", vim.log.levels.INFO)
  end,
  desc = "Yank filename to clipboard",
})

map({
  mode = { "n" },
  lhs = "<leader>y(",
  rhs = function()
    vim.cmd('normal! "+yi(')
    local yanked = vim.fn.getreg("+")
    vim.notify("Yanked: " .. yanked, vim.log.levels.INFO)
  end,
  desc = "Yank inside nearest ( to clipboard",
})

map({
  mode = { "n" },
  lhs = "<leader>y)",
  rhs = function()
    vim.cmd('normal! "+yi)')
    local yanked = vim.fn.getreg("+")
    vim.notify("Yanked: " .. yanked, vim.log.levels.INFO)
  end,
  desc = "Yank inside nearest ) to clipboard",
})

map({
  mode = { "n" },
  lhs = "<leader>y{",
  rhs = function()
    vim.cmd('normal! "+yi{')
    local yanked = vim.fn.getreg("+")
    vim.notify("Yanked: " .. yanked, vim.log.levels.INFO)
  end,
  desc = "Yank inside nearest {} to clipboard",
})

map({
  mode = { "n" },
  lhs = "<leader>y}",
  rhs = function()
    vim.cmd('normal! "+yi}')
    local yanked = vim.fn.getreg("+")
    vim.notify("Yanked: " .. yanked, vim.log.levels.INFO)
  end,
  desc = "Yank inside nearest } to clipboard",
})

map({
  mode = { "n" },
  lhs = "<leader>y[",
  rhs = function()
    vim.cmd('normal! "+yi[')
    local yanked = vim.fn.getreg("+")
    vim.notify("Yanked: " .. yanked, vim.log.levels.INFO)
  end,
  desc = "Yank inside nearest [] to clipboard",
})

map({
  mode = { "n" },
  lhs = "<leader>y]",
  rhs = function()
    vim.cmd('normal! "+yi]')
    local yanked = vim.fn.getreg("+")
    vim.notify("Yanked: " .. yanked, vim.log.levels.INFO)
  end,
  desc = "Yank inside nearest ] to clipboard",
})

map({
  mode = { "n" },
  lhs = '<leader>y"',
  rhs = function()
    vim.cmd('normal! "+yi"')
    local yanked = vim.fn.getreg("+")
    vim.notify("Yanked: " .. yanked, vim.log.levels.INFO)
  end,
  desc = 'Yank inside nearest "" to clipboard',
})

map({
  mode = { "n" },
  lhs = "<leader>y'",
  rhs = function()
    vim.cmd([[normal! "+yi']])
    local yanked = vim.fn.getreg("+")
    vim.notify("Yanked: " .. yanked, vim.log.levels.INFO)
  end,
  desc = "Yank inside nearest '' to clipboard",
})
