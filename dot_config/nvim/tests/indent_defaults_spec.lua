-- config.autocmds: every file type indents 2 by default, overriding the
-- built-in file-type plugins that switch to 4, and a project's .editorconfig
-- still wins over that default.
--
-- Real files under Neovim's own temp tree, opened with `:edit`, because the
-- behavior under test is the ORDER in which Neovim's own FileType plugins,
-- this autocmd and its bundled EditorConfig run on a real read.

local function write(path, text)
  local handle = assert(io.open(path, "w"), "could not write " .. path)
  handle:write(text)
  handle:close()
end

local function temp_dir()
  local dir = vim.fn.tempname()
  assert(vim.fn.mkdir(dir, "p") == 1, "could not create " .. dir)
  return dir
end

local function indent_of(path)
  vim.cmd.edit(path)
  local indent = { sw = vim.bo.shiftwidth, sts = vim.bo.softtabstop, ts = vim.bo.tabstop }
  vim.cmd("bwipeout!")
  return indent
end

require("config.autocmds")

return {
  ["a file type whose built-in plugin wants 4 indents 2"] = function()
    for _, name in ipairs({ "a.py", "a.md", "a.rs" }) do
      local path = temp_dir() .. "/" .. name
      write(path, "x\n")
      local indent = indent_of(path)
      assert(
        indent.sw == 2 and indent.sts == 2 and indent.ts == 2,
        ("%s: sw=%d sts=%d ts=%d"):format(name, indent.sw, indent.sts, indent.ts)
      )
    end
  end,

  ["a project's .editorconfig wins where it covers the file"] = function()
    local dir = temp_dir()
    write(dir .. "/.editorconfig", "root = true\n[*.py]\nindent_style = space\nindent_size = 4\n")
    write(dir .. "/a.py", "x\n")
    write(dir .. "/a.sh", "x\n")
    local python = indent_of(dir .. "/a.py")
    local shell = indent_of(dir .. "/a.sh")
    assert(python.sw == 4, "python under a 4-space .editorconfig has sw=" .. python.sw)
    assert(shell.sw == 2, "shell, which the .editorconfig does not cover, has sw=" .. shell.sw)
  end,
}
