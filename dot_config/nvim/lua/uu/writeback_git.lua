local M = {}
M.LOCK = "dot_config/nvim/lazy-lock.json"

function M.run(repo, args, allowed_failure, env)
  local command = { "git", "-C", repo }
  vim.list_extend(command, args)
  local result = vim.system(command, { text = true, env = env }):wait()
  if result.code ~= 0 and not (allowed_failure and result.code == 1) then
    error(table.concat(command, " ") .. ": " .. result.stderr:sub(-2048))
  end
  return result.stdout
end

local function trim(text)
  return (text:gsub("\n$", ""))
end

function M.identity(repo)
  return {
    branch = trim(M.run(repo, { "symbolic-ref", "-q", "HEAD" }, true)),
    head = trim(M.run(repo, { "rev-parse", "HEAD" })),
    porcelain = M.run(repo, { "status", "--porcelain", "--", M.LOCK }),
    committed = M.run(repo, { "show", "HEAD:" .. M.LOCK }),
    index = M.run(repo, { "show", ":" .. M.LOCK }),
  }
end

function M.installed_agree(lock, plugins)
  local pins = vim.json.decode(lock)
  assert(type(pins) == "table", "lock has no plugin pins")
  for name, directory in pairs(plugins()) do
    local pin = pins[name]
    assert(type(pin) == "table" and type(pin.commit) == "string", name .. ": committed lock has no revision")
    local actual = trim(M.run(directory, { "rev-parse", "HEAD" }))
    assert(actual == pin.commit, name .. ": installed revision disagrees with the lock")
  end
end

function M.commit(repo)
  M.run(
    repo,
    { "commit", "--only", "-m", "chore(nvim): update plugin pins", "--", M.LOCK },
    false,
    { SKIP_AI_COMMIT = "1", GRAPHIFY_SKIP_HOOK = "1" }
  )
  return trim(M.run(repo, { "rev-parse", "HEAD" }))
end

return M
