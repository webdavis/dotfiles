-- Everything neotest-bashunit knows about bashunit's shapes, as pure functions
-- over strings and tables. No file system, no jobs, no neotest: the adapter in
-- init.lua owns those, so every rule below is testable by `tests/run.lua` under
-- a bare `nvim --headless --clean -l`, with no plugin installed.
--
-- Verified against bashunit 0.50.1, which is the version the repository pins in
-- Brewfile.dev, the CI toolchain step and the machine package set.

local M = {}

M.suffix = ".test.sh"

---A bashunit test file, by the repository's naming rule.
---@param path string
---@return boolean
function M.is_test_file(path)
  return #path > #M.suffix and path:sub(-#M.suffix) == M.suffix
end

---bashunit's own title for a test function, mirrored from
---`bashunit::helper::normalize_test_function_name_to_slot`: strip a leading
---`test_`, or a bare leading `test` when there was no underscore, turn every
---remaining underscore into a space, and upcase the first character when it is
---a lowercase letter.
---
---This matters more than it looks. Every structured report bashunit writes
---names its tests by this title and never by the function, so the title is the
---ONLY handle a report row carries back to the function that produced it.
---@param function_name string
---@return string
function M.humanize(function_name)
  local title, stripped = function_name:gsub("^test_", "")
  if stripped == 0 then
    title = function_name:gsub("^test", "")
  end
  title = title:gsub("_", " ")
  local first = title:sub(1, 1)
  if first:match("%l") then
    title = first:upper() .. title:sub(2)
  end
  return title
end

-- bashunit's own discovery, mirrored from the grep at bin/bashunit:8732 in
-- 0.50.1:
--
--   ^[[:space:]]*(function[[:space:]]+)?test[a-zA-Z_][a-zA-Z0-9_]*[[:space:]]*\(\)
--
-- Both spellings are tests, `function name()` and the bare `name()`, the name
-- is `test` plus at least one more character, and the parentheses hold nothing,
-- not even a space. A position this scan offers that bashunit would not run is
-- a test neotest can never turn green, so the two patterns stay in step with
-- that line rather than with what looks reasonable.
local WITH_KEYWORD = "^%s*function%s+(test[%a_][%w_]*)%s*%(%)"
local BARE = "^%s*(test[%a_][%w_]*)%s*%(%)"

---Every test function in a file, in definition order, with its 1-based line.
---@param lines string[]
---@return { name: string, line: integer }[]
function M.test_functions(lines)
  local found = {}
  for number, line in ipairs(lines) do
    local name = line:match(WITH_KEYWORD) or line:match(BARE)
    if name then
      found[#found + 1] = { name = name, line = number }
    end
  end
  return found
end

---The nested list `neotest.Tree.from_list` parses: the file, then one child per
---test function. Ranges are 0-based. A test's range runs to the line before the
---next test so that "run nearest" from anywhere inside a body finds the test it
---is inside rather than the one above it.
---@param path string absolute path
---@param lines string[]
---@return table[]
function M.positions(path, lines)
  local functions = M.test_functions(lines)
  local list = {
    {
      id = path,
      type = "file",
      name = path:match("[^/]+$") or path,
      path = path,
      range = { 0, 0, #lines, 0 },
    },
  }
  for index, found in ipairs(functions) do
    local following = functions[index + 1]
    local last_line = following and following.line - 1 or #lines
    list[#list + 1] = {
      id = path .. "::" .. found.name,
      type = "test",
      name = M.humanize(found.name),
      path = path,
      range = { found.line - 1, 0, last_line, 0 },
    }
  end
  return list
end

-- bashunit reports "incomplete" for a test that declared itself unfinished.
-- neotest has three statuses, and an unfinished test did not run, so it lands
-- with the skipped ones rather than counting as a pass.
local STATUS = {
  passed = "passed",
  failed = "failed",
  skipped = "skipped",
  incomplete = "skipped",
}

---The rows of a `--report-json` document: one per test that ran, each with the
---file it came from, its humanized title, a neotest status and bashunit's own
---message. Returns nil plus a reason when the document cannot be read, because
---an unreadable report and an empty one mean opposite things.
---@param report_text string|nil
---@return { file: string, name: string, status: string, message: string }[]|nil
---@return string|nil error
function M.report_rows(report_text)
  if not report_text or report_text:match("^%s*$") then
    return nil, "bashunit wrote no JSON report"
  end
  local ok, document = pcall(vim.json.decode, report_text)
  if not ok or type(document) ~= "table" or type(document.tests) ~= "table" then
    return nil, "bashunit's JSON report did not parse"
  end
  local rows = {}
  for _, test in ipairs(document.tests) do
    rows[#rows + 1] = {
      file = test.file or "",
      name = test.name or "",
      status = STATUS[test.status] or "failed",
      message = test.message or "",
    }
  end
  return rows, nil
end

---The line each failure actually failed on, keyed by the humanized title.
---
---Only the text summary carries it. Every structured report (json, junit, tap)
---reports the test's DEFINITION line in its `at <file>:<line>` instead, which
---is where the test starts and not where the assertion blew up, so the jump
---target has to come out of the run's own output. Carriage returns are stripped
---because neotest runs its command under a pty.
---@param output_text string|nil
---@return table<string, integer>
function M.failing_lines(output_text)
  local lines = {}
  if not output_text then
    return lines
  end
  local current, awaiting_source = nil, false
  for line in (output_text .. "\n"):gmatch("([^\n]*)\n") do
    line = line:gsub("\r", "")
    local name = line:match("^|?%s*✗ Failed: (.+)$")
    if name then
      current, awaiting_source = name, false
    elseif line:match("Source:%s*$") then
      awaiting_source = true
    elseif awaiting_source then
      local number = line:match("^|?%s*(%d+):")
      if number and current then
        lines[current] = tonumber(number)
      end
      awaiting_source = false
    end
  end
  return lines
end

---The `at <file>:<line>` a report row's message ends with: the test's own
---definition line, and the fallback jump target when the text summary did not
---survive.
---@param message string
---@return integer|nil
function M.message_line(message)
  local number = message:match("at [^\n]-:(%d+)%s*$")
  return number and tonumber(number) or nil
end

---Match report rows onto position ids.
---
---A row names its file and its humanized title; a position carries the id
---neotest wants back. The pair is matched on both, then on the title alone,
---because the two paths can name the same file in different shapes (a
---symlinked checkout, /tmp against /private/tmp). A title that is ambiguous
---inside the run is never matched by the fallback: a wrong result is worse
---than a missing one.
---@param rows { file: string, name: string }[]
---@param positions { id: string, name: string, path: string }[]
---@return table<string, table> row keyed by position id
function M.match_rows(rows, positions)
  local by_path_and_name, by_name = {}, {}
  for _, position in ipairs(positions) do
    by_path_and_name[position.path .. "\0" .. position.name] = position.id
    if by_name[position.name] == nil then
      by_name[position.name] = position.id
    else
      by_name[position.name] = false
    end
  end

  local matched = {}
  for _, row in ipairs(rows) do
    local id = by_path_and_name[row.file .. "\0" .. row.name]
    if not id and by_name[row.name] then
      id = by_name[row.name]
    end
    if id then
      matched[id] = row
    end
  end
  return matched
end

return M
