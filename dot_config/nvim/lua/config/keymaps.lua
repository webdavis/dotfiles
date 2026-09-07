-- Keymaps

-- ┏━━━━━━━━━━━┓
-- ┃    LSP    ┃
-- ┗━━━━━━━━━━━┛

map({
  mode = "n",
  lhs = "<leader>la",
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
  lhs = "<leader>ex",
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
  desc = "File: make the current file executable (chmod +x)",
})

map({
  mode = "n",
  lhs = "<leader>bw",
  rhs = function()
    vim.cmd("wall")
    vim.notify("Saved all files", vim.log.levels.INFO)
  end,
  desc = "Write all modified buffers",
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

-- ┏━━━━━━━━━━━━━━━━━━━━━━━┓
-- ┃    Review Ledger      ┃
-- ┗━━━━━━━━━━━━━━━━━━━━━━━┛

-- One awk program, no Lua parser. The pipeline's findings registers are not
-- tracked by this repository, so a reader written in Lua would be a second thing
-- to keep in step with the table format. `f` is the ledger path used when a row
-- carries no `path:line` token of its own, and it comes from awk's own
-- `FILENAME` rather than a `-v` variable, because `-v` runs escape processing
-- over the value and would turn a backslash in the path into a control
-- character. `all` of 1 keeps the closed rows.
-- The skip is a PREFIX match, because `FIXED-NOTEST` is a closed finding too:
-- the register grammar puts a commit sha on every one of them.
local ledger_awk = [==[
function trim(s) { gsub(/^[ \t]+|[ \t]+$/, "", s); return s }
# How many backticks run on from position i.
function run_len(s, i, len,   n) {
  n = 0
  while (i + n <= len && substr(s, i + n, 1) == "`") n++
  return n
}
# Where the span opened at i by a run of n backticks closes, as the index of the
# last backtick of the CLOSING run, or 0 when nothing closes it. A run of N
# closes only on a run of N, which is what makes a doubled delimiter one span
# rather than two toggles that cancel.
function span_end(s, i, len, n,   j, r) {
  j = i + n
  while (j <= len) {
    if (substr(s, j, 1) != "`") { j++; continue }
    r = run_len(s, j, len)
    if (r == n) return j + n - 1
    j += r
  }
  return 0
}
# Split `s` on any character of `delims`, with a closed code span kept WHOLE: a
# pipe inside one belongs to the cell rather than to the row structure, and a
# space inside one belongs to the path rather than separating two candidates.
# Splitting on the raw character instead let a summary masquerade as a
# disposition, and cut a delimited path in half. An unmatched run is ordinary
# text, and a backslash escapes only OUTSIDE a span, which is where markdown
# puts escapes too.
function tokenize(s, delims, out,   i, c, n, cell, len, r, e) {
  split("", out); n = 0; cell = ""; len = length(s); i = 1
  while (i <= len) {
    c = substr(s, i, 1)
    if (c == "`") {
      r = run_len(s, i, len)
      e = span_end(s, i, len, r)
      if (e > 0) { cell = cell substr(s, i, e - i + 1); i = e + 1; continue }
      cell = cell substr(s, i, r); i += r; continue
    }
    if (c == "\\" && i < len) { cell = cell c substr(s, i + 1, 1); i += 2; continue }
    if (index(delims, c) > 0) { out[++n] = cell; cell = ""; i++; continue }
    cell = cell c; i++
  }
  out[++n] = cell
  return n
}
# A location is a PATH-SHAPED token, not any word carrying a colon and digits:
# a version (v1.2.3:45), a URL with a port, and a neighbour joined by a comma all
# read as one otherwise. Each token is cut back to its leading `<path>:<line>`,
# which is what drops enclosing brackets, a trailing possessive, and the second
# half of a line RANGE, and the first qualifying token wins. Path-shaped means a
# slash or an extension whose first character is a letter, which is what tells
# `a/b.lua` from `v1.2.3`. The location stops at the FIRST colon-and-digits, so
# an editor column in `a/b.c:4:5` is dropped rather than joining the filename,
# and a Windows drive colon is taken off first so it is not mistaken for one.
# Ceiling: a bare `host.com:8080` would still qualify.
function locate(s,   parts, n, i, t, path, drive) {
  n = tokenize(s, " \t,", parts)
  for (i = 1; i <= n; i++) {
    t = parts[i]
    if (t == "" || t ~ /:\/\//) continue
    sub(/^[(\[{`<'"]+/, "", t)
    drive = ""
    if (t ~ /^[A-Za-z]:[\\\/]/) { drive = substr(t, 1, 2); t = substr(t, 3) }
    if (!match(t, /:[0-9]+/)) continue
    path = drive substr(t, 1, RSTART - 1)
    if (path ~ /\// || path ~ /\.[A-Za-z][A-Za-z0-9_]*$/) return path ":" substr(t, RSTART + 1, RLENGTH - 1)
  }
  return ""
}
/^[ \t]*\|[ \t]*F[0-9]+[ \t]*\|/ {
  tokenize($0, "|", cell)
  id = trim(cell[2]); severity = trim(cell[4]); summary = trim(cell[5]); disposition = trim(cell[6])
  if (disposition ~ /^FIXED/ && all != 1) next
  location = locate(summary)
  where = (location == "") ? FILENAME ":" FNR : location
  printf "%s: %s %s %s: %s\n", where, id, severity, disposition, summary
}
]==]

vim.api.nvim_create_user_command("ReviewLedger", function(opts)
  -- Not `expand`: `complete = "file"` already expanded and unescaped this for
  -- us, the way `:edit` does. A second pass unescapes what the first pass
  -- produced, so a name carrying a backslash or a leading `%` is looked for
  -- under a name that is not its own.
  local register = opts.args
  if register == "" then
    -- `glob` expands the tilde itself, and `expand` must NOT run first: it
    -- expands the WILDCARD too, so `glob` would be handed every match joined by
    -- newlines and would match nothing (measured 2026-09-05).
    local found = vim.fn.glob("~/.claude/pipeline/slices/findings-*.md", false, true)
    table.sort(found, function(a, b)
      return vim.fn.getftime(a) > vim.fn.getftime(b)
    end)
    register = found[1]
  end
  if not register or register == "" then
    vim.notify("ReviewLedger: no findings register found", vim.log.levels.WARN)
    return
  end
  local lines = vim.fn.systemlist(
    ("awk -v all=%d %s %s"):format(opts.bang and 1 or 0, vim.fn.shellescape(ledger_awk), vim.fn.shellescape(register))
  )
  vim.fn.setqflist({}, " ", { title = "ReviewLedger " .. register, lines = lines, efm = "%f:%l: %m" })
  vim.cmd("copen")
end, {
  nargs = "?",
  bang = true,
  complete = "file",
  desc = "Load a findings register into the quickfix list (! keeps the closed rows)",
})

return { ledger_awk = ledger_awk }
