local M = {}

function M.write(path, text)
  vim.fn.mkdir(vim.fn.fnamemodify(path, ":h"), "p", 448)
  local file = assert(io.open(path, "wb"))
  assert(file:write(text))
  assert(file:close())
end

function M.read(path)
  local file = assert(io.open(path, "rb"))
  local text = file:read("*a")
  file:close()
  return text
end

return M
