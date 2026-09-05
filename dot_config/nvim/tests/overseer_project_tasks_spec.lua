-- The project-local task provider in `lua/overseer/template/user/project_tasks.lua`.
--
-- The subject is what it accepts and what it refuses. A task file comes out of
-- someone's repository, so a malformed entry has to be skipped with a reason
-- rather than raising and taking the whole task picker down with it. These cases
-- drive the real generator against real files in a scratch directory.

local provider = require("overseer.template.user.project_tasks")

local scratch = vim.fn.tempname()

---Write a task file and run the generator against its directory.
---@param body string|table The file content, or a table encoded as JSON
---@param filename? string Defaults to `.overseer/tasks.json`
---@return table|string templates The generator's return, or its error string
---@return string[] warnings Everything the provider notified about
local function generate(body, filename)
  filename = filename or ".overseer/tasks.json"
  vim.fn.delete(scratch, "rf")
  local path = vim.fs.joinpath(scratch, filename)
  vim.fn.mkdir(vim.fs.dirname(path), "p")
  local text = type(body) == "string" and body or vim.json.encode(body)
  assert(vim.fn.writefile(vim.split(text, "\n"), path) == 0, "could not write " .. path)

  local warnings = {}
  local real_notify = vim.notify
  vim.notify = function(msg)
    table.insert(warnings, tostring(msg))
  end
  local ok, result = pcall(provider.generator, { dir = scratch })
  vim.notify = real_notify
  vim.fn.delete(scratch, "rf")
  assert(ok, result)
  return result, warnings
end

---@param templates table
---@return string[]
local function names(templates)
  local out = {}
  for _, t in ipairs(templates) do
    table.insert(out, t.name)
  end
  table.sort(out)
  return out
end

return {
  ["a well-formed entry becomes a template"] = function()
    local templates = generate({ tasks = { { name = "build", cmd = { "make" }, desc = "Build it" } } })
    assert(#templates == 1, "got " .. #templates .. " templates")
    assert(templates[1].name == "build", "name was " .. tostring(templates[1].name))
    assert(templates[1].desc == "Build it", "desc was " .. tostring(templates[1].desc))
    assert(vim.deep_equal(templates[1].builder().cmd, { "make" }), "cmd did not survive the builder")
  end,

  ["a relative cwd resolves against the file, not the caller"] = function()
    local templates = generate({ tasks = { { name = "sub", cmd = "pwd", cwd = "server" } } })
    local cwd = templates[1].builder().cwd
    assert(cwd:match("/server$"), "cwd was " .. tostring(cwd))
    assert(not cwd:match("%.overseer"), "cwd was resolved against .overseer rather than the project: " .. cwd)
  end,

  ["an entry with no cmd is skipped, and the rest still load"] = function()
    local templates, warnings = generate({
      tasks = { { name = "good", cmd = { "true" } }, { name = "bad" } },
    })
    assert(vim.deep_equal(names(templates), { "good" }), "got " .. vim.inspect(names(templates)))
    assert(#warnings == 1, "expected one warning, got " .. #warnings)
    assert(warnings[1]:match('"bad" has no cmd'), "warning was " .. warnings[1])
  end,

  ["an entry with no name is skipped by position"] = function()
    local _, warnings = generate({ tasks = { { cmd = { "true" } } } })
    assert(warnings[1]:match("entry 1 has no name"), "warning was " .. tostring(warnings[1]))
  end,

  ["an unknown tag is skipped rather than passed through"] = function()
    local templates, warnings = generate({ tasks = { { name = "t", cmd = { "true" }, tags = { "NOPE" } } } })
    assert(#templates == 0, "the entry was accepted")
    assert(warnings[1]:match("unknown tag"), "warning was " .. tostring(warnings[1]))
  end,

  ["an empty cmd list is refused"] = function()
    local templates = generate({ tasks = { { name = "t", cmd = {} } } })
    assert(#templates == 0, "an empty argv was accepted")
  end,

  ["a non-string word in cmd is refused"] = function()
    local templates = generate({ tasks = { { name = "t", cmd = { "echo", 7 } } } })
    assert(#templates == 0, "a non-string argv word was accepted")
  end,

  ["a file that is not JSON is reported, not raised"] = function()
    local result = generate("this is not json")
    assert(type(result) == "string", "expected an error string, got " .. type(result))
    assert(result:match("tasks list"), "message was " .. result)
  end,

  ["a JSON file with no tasks list is reported"] = function()
    local result = generate({ nottasks = {} })
    assert(type(result) == "string", "expected an error string, got " .. type(result))
  end,

  ["overseer.json is read as well as .overseer/tasks.json"] = function()
    local templates = generate({ tasks = { { name = "flat", cmd = { "true" } } } }, "overseer.json")
    assert(vim.deep_equal(names(templates), { "flat" }), "got " .. vim.inspect(names(templates)))
  end,

  ["a directory with no task file is reported, not raised"] = function()
    vim.fn.delete(scratch, "rf")
    vim.fn.mkdir(scratch, "p")
    local result = provider.generator({ dir = scratch })
    vim.fn.delete(scratch, "rf")
    assert(type(result) == "string", "expected an error string, got " .. type(result))
  end,
}
