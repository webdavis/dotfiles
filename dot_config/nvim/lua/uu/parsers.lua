local Report = require("uu.report")

local function run()
  local installed = require("nvim-treesitter.config").get_installed()
  local ran, result = pcall(function()
    return require("nvim-treesitter.install").update(nil, { summary = true }):wait()
  end)
  local log = vim.api.nvim_exec2('lua require("nvim-treesitter.log").show()', { output = true }).output
  local updated, failed, errors = {}, {}, {}
  local collecting
  for _, line in ipairs(vim.split(log, "\n", { plain = true })) do
    local name = line:match("^info%(install/(.-)%): Language installed$")
    if name then
      updated[name] = true
    end
    local error_name = line:match("^error%(install/(.-)%)")
    if error_name or line:match("^error:") then
      if error_name then
        failed[error_name] = true
      end
      errors[#errors + 1] = line
      collecting = #errors
    elseif line:match("^%a+[(:]") then
      collecting = nil
    elseif collecting then
      errors[collecting] = (errors[collecting] .. "\n" .. line):sub(-2048)
    end
  end
  if not ran then
    errors[#errors + 1] = "parser update failed: " .. tostring(result)
  end
  local changed, current = {}, {}
  for name in pairs(updated) do
    changed[#changed + 1] = name
  end
  for _, name in ipairs(installed) do
    if not updated[name] and not failed[name] then
      current[#current + 1] = name
    end
  end
  table.sort(changed)
  table.sort(current)
  local lines, status =
    Report.parser_lines({ ok = ran and result == true, updated = changed, current = current, errors = errors })
  if #changed > 0 then
    local notice = Report.restart_notice(Report.other_instances(Report.running_sockets(), vim.uv.os_getpid()))
    if notice then
      lines[#lines + 1] = notice
    end
  end
  return lines, status
end

local ok, lines, status = pcall(run)
if not ok then
  lines, status = { "parser update failed: " .. tostring(lines) }, Report.FAILED
end
for _, line in ipairs(lines) do
  io.write(line, "\n")
end
io.flush()
vim.cmd("cquit " .. status)
