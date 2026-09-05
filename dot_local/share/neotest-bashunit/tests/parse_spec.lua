-- Every rule neotest-bashunit holds about bashunit 0.50.1's shapes. The
-- fixtures below are transcribed from real runs of that version, not written to
-- match the code: a bashunit release that changes any of them should turn these
-- red rather than change the adapter's behavior quietly.

local parse = require("neotest-bashunit.parse")

-- One passing and one failing test, as bashunit's --report-json writes them.
local REPORT = [[
{
  "summary": { "total": 2, "passed": 1, "failed": 1, "skipped": 0, "incomplete": 0, "flaky": 0, "duration_ms": 10 },
  "tests": [
    { "file": "/w/lines.test.sh", "name": "First", "status": "passed", "duration_ms": 2, "retries": 0, "message": "" },
    { "file": "/w/lines.test.sh", "name": "Second fails here", "status": "failed", "duration_ms": 8, "retries": 0,
      "message": "✗ Failed: Second fails here\n    Expected '9'\n    but got  '2'\n    at /w/lines.test.sh:11" }
  ]
}
]]

-- The tail of the same run's text output, which is the ONLY place the failing
-- assertion's own line appears. \r is what a pty leaves behind, and neotest
-- runs its command under one.
local OUTPUT = table.concat({
  "There was 1 failure:\r",
  "\r",
  "|1) /w/lines.test.sh:11\r",
  "|\226\156\151 Failed: Second fails here\r",
  "|    Expected '9'\r",
  "|    but got  '2'\r",
  "|    at /w/lines.test.sh:11\r",
  "|    Source:\r",
  '|    13: assert_equals 9 "$x"\r',
  "\r",
  "Tests:      2 passed, 1 failed, 3 total\r",
}, "\n")

local function lines_of(text)
  local lines = {}
  for line in (text .. "\n"):gmatch("([^\n]*)\n") do
    lines[#lines + 1] = line
  end
  return lines
end

return {
  ["is_test_file takes the .test.sh suffix and nothing near it"] = function()
    assert(parse.is_test_file("/w/osquery.test.sh"))
    assert(not parse.is_test_file("/w/osquery.sh"))
    assert(not parse.is_test_file("/w/osquery.test.bash"))
    assert(not parse.is_test_file("/w/osquery_test.sh"))
    -- A file called nothing but the suffix has no name to show in the summary.
    assert(not parse.is_test_file(".test.sh"))
  end,

  ["humanize reproduces bashunit's own title for a test function"] = function()
    assert(parse.humanize("test_second_fails_here") == "Second fails here")
    assert(parse.humanize("test_a_directory_at_0755_root_wheel") == "A directory at 0755 root wheel")
  end,

  ["humanize strips a bare test prefix when there is no underscore"] = function()
    -- bashunit falls back to `${name#test}` when `${name#test_}` changed
    -- nothing, so a camel-cased test keeps its own capital.
    assert(parse.humanize("testCamelCased") == "CamelCased")
  end,

  ["test_functions finds both spellings bashunit runs"] = function()
    local found = parse.test_functions(lines_of(table.concat({
      "#!/usr/bin/env bash",
      "function test_with_keyword() {",
      "  assert_equals 1 1",
      "}",
      "test_bare_style() {",
      "  assert_equals 2 2",
      "}",
      "  function test_indented() {",
      "}",
    }, "\n")))
    assert(#found == 3, "expected 3 test functions, got " .. #found)
    assert(found[1].name == "test_with_keyword" and found[1].line == 2)
    assert(found[2].name == "test_bare_style" and found[2].line == 5)
    assert(found[3].name == "test_indented" and found[3].line == 8)
  end,

  ["test_functions offers nothing bashunit would refuse to run"] = function()
    local found = parse.test_functions(lines_of(table.concat({
      "helper_function() {", -- not a test
      "test() {", -- `test` alone: bashunit needs one more character
      "test_spaced( ) {", -- bashunit's pattern allows nothing between the parens
      "  local x=test_not_a_definition",
      "}",
    }, "\n")))
    assert(#found == 0, "expected no test functions, got " .. #found)
  end,

  ["positions put the file first and one test after it"] = function()
    local list = parse.positions(
      "/w/lines.test.sh",
      lines_of(table.concat({
        "#!/usr/bin/env bash",
        "function test_first() {",
        "  assert_equals 1 1",
        "}",
      }, "\n"))
    )
    assert(#list == 2)
    assert(list[1].type == "file" and list[1].id == "/w/lines.test.sh" and list[1].name == "lines.test.sh")
    assert(list[2].type == "test" and list[2].id == "/w/lines.test.sh::test_first")
    assert(list[2].name == "First", "the summary shows bashunit's own title")
  end,

  ["a test's range reaches the line before the next test"] = function()
    -- What makes "run nearest" from inside a body pick the test it is inside.
    local list = parse.positions(
      "/w/lines.test.sh",
      lines_of(table.concat({
        "function test_first() {", -- line 1
        "  assert_equals 1 1",
        "}",
        "",
        "function test_second() {", -- line 5
        "  assert_equals 2 2",
        "}",
      }, "\n"))
    )
    assert(list[2].range[1] == 0 and list[2].range[3] == 4, "first test should end where the second starts")
    assert(list[3].range[1] == 4 and list[3].range[3] == 7, "last test should reach the end of the file")
  end,

  ["report_rows reads one row per test out of the JSON report"] = function()
    local rows, err = parse.report_rows(REPORT)
    assert(err == nil, tostring(err))
    assert(#rows == 2)
    assert(rows[1].name == "First" and rows[1].status == "passed" and rows[1].message == "")
    assert(rows[2].name == "Second fails here" and rows[2].status == "failed")
    assert(rows[2].file == "/w/lines.test.sh")
    assert(rows[2].message:find("but got  '2'", 1, true), "the row keeps bashunit's own assertion text")
  end,

  ["an incomplete test is skipped, never a pass"] = function()
    local rows = parse.report_rows('{"tests":[{"file":"/w/a.test.sh","name":"A","status":"incomplete"}]}')
    assert(rows[1].status == "skipped")
  end,

  ["an unreadable report is an error, not an empty run"] = function()
    -- The two mean opposite things: no rows is "nothing matched the filter",
    -- no report is "the run never happened".
    local rows, err = parse.report_rows("")
    assert(rows == nil and err ~= nil)
    rows, err = parse.report_rows("bashunit: command not found")
    assert(rows == nil and err ~= nil)
    rows, err = parse.report_rows('{"summary":{}}')
    assert(rows == nil and err ~= nil, "a document with no tests array is not a report")
  end,

  ["failing_lines takes the assertion's line out of the text summary"] = function()
    local lines = parse.failing_lines(OUTPUT)
    assert(lines["Second fails here"] == 13, "expected 13, got " .. tostring(lines["Second fails here"]))
  end,

  ["failing_lines reports nothing when nothing failed"] = function()
    local lines = parse.failing_lines("Tests:      3 passed, 3 total\r\n")
    assert(next(lines) == nil)
    assert(next(parse.failing_lines(nil)) == nil)
  end,

  ["message_line falls back to the definition line the report carries"] = function()
    local rows = parse.report_rows(REPORT)
    assert(parse.message_line(rows[2].message) == 11)
    assert(parse.message_line("no location here") == nil)
  end,

  ["match_rows pairs a row with the position that has its file and title"] = function()
    local rows = parse.report_rows(REPORT)
    local matched = parse.match_rows(rows, {
      { id = "/w/lines.test.sh::test_first", name = "First", path = "/w/lines.test.sh" },
      { id = "/w/lines.test.sh::test_second_fails_here", name = "Second fails here", path = "/w/lines.test.sh" },
    })
    assert(matched["/w/lines.test.sh::test_first"].status == "passed")
    assert(matched["/w/lines.test.sh::test_second_fails_here"].status == "failed")
  end,

  ["match_rows still pairs when the two paths name the file differently"] = function()
    -- /tmp against /private/tmp, or a symlinked checkout: same file, two
    -- spellings, and a run whose results all went missing would be worse.
    local matched = parse.match_rows(
      { { file = "/private/w/lines.test.sh", name = "First", status = "passed" } },
      { { id = "/w/lines.test.sh::test_first", name = "First", path = "/w/lines.test.sh" } }
    )
    assert(matched["/w/lines.test.sh::test_first"] ~= nil)
  end,

  ["match_rows refuses a title two positions share"] = function()
    -- The fallback is only allowed to guess when there is nothing to guess
    -- between: a wrong result is worse than a missing one.
    local matched = parse.match_rows({ { file = "/elsewhere/a.test.sh", name = "First", status = "failed" } }, {
      { id = "/w/a.test.sh::test_first", name = "First", path = "/w/a.test.sh" },
      { id = "/w/b.test.sh::test_first", name = "First", path = "/w/b.test.sh" },
    })
    assert(next(matched) == nil)
  end,

  ["match_rows drops a row no position asked for"] = function()
    -- --filter is a substring match, so a run can carry a sibling test the
    -- spec's tree does not hold.
    local matched = parse.match_rows(
      { { file = "/w/a.test.sh", name = "Alpha extended", status = "passed" } },
      { { id = "/w/a.test.sh::test_alpha", name = "Alpha", path = "/w/a.test.sh" } }
    )
    assert(next(matched) == nil)
  end,
}
