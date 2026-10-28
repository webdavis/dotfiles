map({
  mode = "n",
  lhs = "<leader>la",
  rhs = function()
    vim.lsp.buf.code_action()
  end,
  desc = "LSP: code action",
})

map({ mode = "n", lhs = "<C-s>", rhs = "write", desc = "Save file" })

map({
  mode = "n",
  lhs = "<leader>ex",
  rhs = function()
    local file = vim.fn.fnamemodify(vim.fn.expand("%"), ":.")
    -- Passes the file name as an argument, so the shell never expands a name like `a$(...)b`.
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

map({ mode = "n", lhs = "<leader>lm", rhs = "Mason", desc = "Mason: open" })

map({ mode = { "n", "x" }, lhs = "j", rhs = "v:count == 0 ? 'gj' : 'j'", expr = true, sequence = true })
map({ mode = { "n", "x" }, lhs = "k", rhs = "v:count == 0 ? 'gk' : 'k'", expr = true, sequence = true })

map({ mode = "n", lhs = "<C-e>", rhs = "<C-e><C-e>", desc = "Scroll down (x2)", sequence = true })
map({ mode = "n", lhs = "<C-y>", rhs = "<C-y><C-y>", desc = "Scroll up (x2)", sequence = true })

-- `n` searches forward and `N` backward, whatever the last search direction.
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

map({ mode = "n", lhs = "<C-Up>", rhs = "resize +2", desc = "Increase window Height" })
map({ mode = "n", lhs = "<C-Down>", rhs = "resize -2", desc = "Decrease window Height" })
map({ mode = "n", lhs = "<C-Left>", rhs = "vertical resize -2", desc = "Decrease window Width" })
map({ mode = "n", lhs = "<C-Right>", rhs = "vertical resize +2", desc = "Increase window Width" })

map({ mode = "v", lhs = "<", rhs = "<gv", desc = "Shift line left (←)", sequence = true })
map({ mode = "v", lhs = ">", rhs = ">gv", desc = "Shift line right (→)", sequence = true })

-- Each of these marks starts a new undo step, so one undo removes less text.
map({ mode = "i", lhs = ",", rhs = ",<c-g>u", sequence = true })
map({ mode = "i", lhs = ".", rhs = ".<c-g>u", sequence = true })
map({ mode = "i", lhs = ";", rhs = ";<c-g>u", sequence = true })

map({
  mode = "n",
  lhs = "<leader>K",
  rhs = function()
    vim.cmd("normal! K")
  end,
  desc = "Keywordprg",
})

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

map({
  mode = "n",
  lhs = "Y",
  rhs = function()
    require("custom_api.clipboard").yank_to_line_end()
  end,
  desc = "Yank to the end-of-line (without line-ending)",
})

map({
  mode = "n",
  lhs = "<leader>yp",
  rhs = '"+yap',
  desc = "Yank current paragraph to clipboard",
  sequence = true,
})

-- `x` rather than `v`, so typing over a snippet placeholder in select mode still replaces it.
map({
  mode = "x",
  lhs = "<leader>y",
  rhs = function()
    require("custom_api.clipboard").yank_selection()
  end,
  desc = "Yank the selection to clipboard, exactly as selected",
})

-- Built-in visual `Y` yanks to the unnamed register only.
map({
  mode = "x",
  lhs = "Y",
  rhs = function()
    require("custom_api.clipboard").yank_selected_lines()
  end,
  desc = "Yank the selected lines to clipboard, whole",
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
    if filename == "" then
      vim.notify("Nothing to yank: this buffer has no filename", vim.log.levels.WARN)
      return
    end
    vim.fn.setreg("+", filename)
    vim.notify("Filename (*" .. filename .. "*) yanked to clipboard", vim.log.levels.INFO)
  end,
  desc = "Yank filename to clipboard",
})

-- Both keys of a pair select the same text object.
for _, pair in ipairs({
  { keys = { "(", ")" }, delimiter = "(", label = "( ) pair" },
  { keys = { "{", "}" }, delimiter = "{", label = "{ } pair" },
  { keys = { "[", "]" }, delimiter = "[", label = "[ ] pair" },
  { keys = { '"' }, delimiter = '"', label = 'pair of " quotes' },
  { keys = { "'" }, delimiter = "'", label = "pair of ' quotes" },
}) do
  for _, key in ipairs(pair.keys) do
    map({
      mode = "n",
      lhs = "<leader>y" .. key,
      rhs = function()
        require("custom_api.clipboard").yank_inside(pair.delimiter, pair.label)
      end,
      desc = "Yank inside nearest " .. pair.label .. " to clipboard",
    })
  end
end

-- The script reads the path from awk's FILENAME, because `-v` would turn its backslashes into escapes.
-- Setting `all` to 1 keeps the closed rows.
local ledger_awk = [==[
function trim(s) { gsub(/^[ \t]+|[ \t]+$/, "", s); return s }
function run_len(s, i, len,   n) {
  n = 0
  while (i + n <= len && substr(s, i + n, 1) == "`") n++
  return n
}
# Index of the last backtick of the run of n that closes the span opened at i, or 0.
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
# Split s on delims, keeping each closed code span whole.
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
# Returns the first `<path>:<line>` token whose path has a slash or a letter extension,
# so `v1.2.3:45` is skipped.
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
  -- `complete = "file"` already expanded the argument, so a second `expand` would unescape it twice.
  local register = opts.args
  if register == "" then
    -- `glob` expands the tilde itself. Running `expand` first would also expand the wildcard.
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
