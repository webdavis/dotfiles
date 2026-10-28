-- stylua: ignore start
local cmd_map = {
  go = function(file) return { "go", "run", file } end,
  lua = function(file) return { "lua", file } end,
  python = function(file) return { "python3", file } end,
  sh = function(file) return { "bash", file } end,
  swift = "set -eo pipefail; swift build",
}
-- stylua: ignore end

local default_components = {
  { "on_output_quickfix", open = false, set_diagnostics = true },
  "on_result_diagnostics",
  "default",
}

return {
  name = "run script",
  builder = function()
    local ft = vim.bo.filetype
    local cmd_entry = cmd_map[ft]

    if not cmd_entry then
      return nil
    end

    local file = vim.fn.expand("%:p")
    if file == "" and type(cmd_entry) == "function" then
      return nil
    end

    local cmd
    if type(cmd_entry) == "function" then
      cmd = cmd_entry(file)
    else
      cmd = cmd_entry
    end

    return {
      cmd = cmd,
      components = default_components,
    }
  end,
  condition = {
    filetype = vim.tbl_keys(cmd_map),
  },
}
