-- Everything neotest-bashunit knows about bashunit's shapes, as pure functions
-- over strings and tables. No file system, no jobs, no neotest: the adapter in
-- init.lua owns those, so every rule below is testable by `tests/run.lua` under
-- a bare `nvim --headless --clean -l`, with no plugin installed.
--
-- Every rule and fixture here was measured against ONE bashunit release, named
-- by `M.verified_version` below and checked by this project's own gate. See
-- that field for why the check exists rather than a version pin.

local M = {}

M.suffix = ".test.sh"

---The bashunit release every rule in this file was measured against.
---
---Homebrew has no declarative version pin, so all three declarations in this
---repository (Brewfile.dev, the CI toolchain step, the machine package set)
---install whatever is current, and `brew info bashunit` reports the formula as
---unpinned and auto-bumped. A release that changes an output shape would leave
---this adapter's frozen fixtures green while it silently misreported real runs,
---so the gate refuses to certify fixtures captured from a different release and
---names both versions. That is the same trade the repository takes on stylua
---and for the same reason: a visible failure on an untouched file beats a
---silent behavior change.
---
---Moving this means re-measuring, not just editing: every fixture in
---tests/parse_spec.lua is transcribed from a run of this exact version.
M.verified_version = "0.50.1"

---The release number out of `bashunit --version`, which prints it wrapped in
---ANSI escapes even under NO_COLOR.
---@param version_output string|nil
---@return string|nil
function M.version_of(version_output)
  return version_output and version_output:match("(%d+%.%d+%.%d+)") or nil
end

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

-- bashunit's RUNTIME selection, mirrored from
-- `bashunit::helper::get_functions_to_run` in 0.50.1:
--
--   case "$fn" in ${prefix}_*${filter}*)      # prefix is the literal "test"
--
-- It runs over `compgen -A function` after sourcing the file, so the rule is
-- exactly `test_` plus at least one more character, and what follows may be any
-- byte bash accepts in a function name.
--
-- The tempting thing to mirror is bashunit's line-lookup grep at bin/bashunit:8732
-- (`test[a-zA-Z_][a-zA-Z0-9_]*[[:space:]]*\(\)`), and it is WRONG in both
-- directions, measured on 0.50.1: it matches `testCamel` and `testable`, which
-- bashunit never runs, and it misses `test_éclair`, which bashunit runs and
-- titles "éclair". A position bashunit will not run can never go green, and one
-- it runs but we never offered is a test neotest cannot see.
--
-- The definition line itself is bash's, not bashunit's: `test_x ( )` is defined
-- by bash, enumerated by compgen and run (measured), so the parentheses may
-- hold and be surrounded by whitespace.
local WITH_KEYWORD = "^%s*function%s+(test_[^%s()]+)%s*%(%s*%)"
local BARE = "^%s*(test_[^%s()]+)%s*%(%s*%)"

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

---Positions whose title another position in the SAME FILE also carries, mapped
---to the message that names the collision.
---
---`test_dupe` and `test_Dupe` both humanize to `Dupe`, and bashunit runs both
---and reports two rows under that one name (measured on 0.50.1, one failing and
---one passing, process exit 1). Since the title is the only handle a row
---carries, neither row can be attributed, and taking the last one silently
---reported the pass and hid the failure. Both sides are refused instead.
---
---A shared title across two DIFFERENT files is not ambiguous: a row names its
---file too.
---@param positions { id: string, name: string, path: string }[]
---@return table<string, string> position id -> collision message
function M.ambiguous_positions(positions)
  local groups = {}
  for _, position in ipairs(positions) do
    local key = position.path .. "\0" .. position.name
    groups[key] = groups[key] or {}
    table.insert(groups[key], position)
  end

  local ambiguous = {}
  for _, group in pairs(groups) do
    if #group > 1 then
      local function_names = {}
      for _, position in ipairs(group) do
        function_names[#function_names + 1] = position.id:match("::(.*)$") or position.id
      end
      table.sort(function_names)
      local message = ("neotest-bashunit: %d tests in this file share the title %q (%s). bashunit names a result by its title alone, so no report row can be attributed to either one. Rename one of them."):format(
        #group,
        group[1].name,
        table.concat(function_names, ", ")
      )
      for _, position in ipairs(group) do
        ambiguous[position.id] = message
      end
    end
  end
  return ambiguous
end

---The sibling function names a `--filter` on `selected` would also run.
---
---bashunit's filter is `case "$fn" in test_*${needle}*`, where the needle is the
---given name with `test_` removed, so it is a substring match anchored only at
---the prefix: `--filter test_alpha` runs `test_alpha_extended` too, and
---`test_beta_alpha` as well. Feeding these back as `--exclude-filter` entries is
---what makes running one test run one test.
---@param selected string
---@param function_names string[]
---@return string[]
function M.exclude_filters(selected, function_names)
  local needle = selected:gsub("^test_", "", 1)
  local excludes = {}
  for _, name in ipairs(function_names) do
    if name ~= selected and name:find(needle, 1, true) then
      excludes[#excludes + 1] = name
    end
  end
  return excludes
end

---The nested list `neotest.Tree.from_list` parses: the file, then one child per
---test function. Ranges are 0-based. A test's range runs to the line before the
---next test so that "run nearest" from anywhere inside a body finds the test it
---is inside rather than the one above it.
---
---Each test also carries the ambiguity verdict, computed here because this is
---the only place that sees every position in the file at once; `results` reads
---it back off the position rather than recomputing from a tree that may hold
---one node.
---@param path string absolute path
---@param lines string[]
---@return table[]
function M.positions(path, lines)
  local functions = M.test_functions(lines)
  local tests = {}
  for index, found in ipairs(functions) do
    local following = functions[index + 1]
    tests[#tests + 1] = {
      id = path .. "::" .. found.name,
      type = "test",
      name = M.humanize(found.name),
      path = path,
      range = { found.line - 1, 0, following and following.line - 1 or #lines, 0 },
    }
  end

  local ambiguous = M.ambiguous_positions(tests)
  local list = {
    {
      id = path,
      type = "file",
      name = path:match("[^/]+$") or path,
      path = path,
      range = { 0, 0, #lines, 0 },
    },
  }
  for _, test in ipairs(tests) do
    test.ambiguous = ambiguous[test.id]
    list[#list + 1] = test
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

---Every line bashunit listed under `Source:` for each failure, keyed by the
---failure's file and humanized title.
---
---Two things this deliberately does not do. It does not pick a line: bashunit
---lists EVERY textual assertion in the failing function, not the one that
---failed (measured on 0.50.1: a passing assertion on line 16 followed by a
---failure on 17 lists both), so only a lone candidate identifies anything and
---the caller decides what to do with more. And it does not key by title alone:
---the same title in two files failing on lines 2 and 100 would collapse onto
---one entry and send both jumps to the wrong file, so the file comes off the
---numbered failure header that opens each block.
---
---The text summary is the only place any of this appears. Every structured
---report (json, junit, tap, re-checked at 0.50.1) carries the test's DEFINITION
---line in `at <file>:<line>` and no assertion line at all. Carriage returns are
---stripped because neotest runs its command under a pty.
---@param output_text string|nil
---@return table<string, integer[]> "<file>\0<title>" -> candidate lines, in order
function M.failing_lines(output_text)
  local blocks = {}
  if not output_text then
    return blocks
  end

  local file, title, collecting = nil, nil, false
  for line in (output_text .. "\n"):gmatch("([^\n]*)\n") do
    line = line:gsub("\r", "")
    local header_file = line:match("^|?%s*%d+%)%s*(.+):%d+%s*$")
    local failed_title = line:match("^|?%s*✗ Failed: (.+)$")
    if header_file then
      file, title, collecting = header_file, nil, false
    elseif failed_title then
      title, collecting = failed_title, false
    elseif line:match("Source:%s*$") then
      collecting = file ~= nil and title ~= nil
      if collecting then
        blocks[file .. "\0" .. title] = blocks[file .. "\0" .. title] or {}
      end
    elseif collecting then
      local number = line:match("^|?%s*(%d+):")
      if number then
        table.insert(blocks[file .. "\0" .. title], tonumber(number))
      else
        collecting = false
      end
    end
  end
  return blocks
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
    -- A position discovery marked ambiguous shares its title with another test
    -- in the same file, so no row can be shown to belong to it. It is refused
    -- here and answered explicitly by `results` instead.
    if not position.ambiguous then
      by_path_and_name[position.path .. "\0" .. position.name] = position.id
      if by_name[position.name] == nil then
        by_name[position.name] = position.id
      else
        by_name[position.name] = false
      end
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
