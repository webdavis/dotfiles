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

-- The tail of a run's text output, which is the ONLY place the failing
-- assertion's own line appears. \r is what a pty leaves behind, and neotest
-- runs its command under one.
--
-- Three failures on purpose. The first carries ONE Source candidate, which is
-- the only shape that identifies an assertion. The second is the same title in
-- a DIFFERENT file, failing on a different line. The third carries TWO
-- candidates, because bashunit lists every textual assertion in the function
-- rather than the one that failed.
local OUTPUT = table.concat({
  "There were 3 failures:\r",
  "\r",
  "|1) /w/lines.test.sh:11\r",
  "|\226\156\151 Failed: Second fails here\r",
  "|    Expected '9'\r",
  "|    but got  '2'\r",
  "|    at /w/lines.test.sh:11\r",
  "|    Source:\r",
  '|    13: assert_same 9 "$x"\r',
  "|2) /w/other.test.sh:98\r",
  "|\226\156\151 Failed: Second fails here\r",
  "|    at /w/other.test.sh:98\r",
  "|    Source:\r",
  "|    100: assert_same 1 2\r",
  "|3) /w/lines.test.sh:15\r",
  "|\226\156\151 Failed: Two assertions\r",
  "|    at /w/lines.test.sh:15\r",
  "|    Source:\r",
  "|    16: assert_same 1 1\r",
  "|    17: assert_same 2 3\r",
  "\r",
  "Tests:      2 passed, 3 failed, 5 total\r",
}, "\n")

local function lines_of(text)
  local lines = {}
  for line in (text .. "\n"):gmatch("([^\n]*)\n") do
    lines[#lines + 1] = line
  end
  return lines
end

return {
  ["the installed bashunit is the release these fixtures were captured from"] = function()
    -- Homebrew cannot pin declaratively, so this gate is the pin: a bashunit
    -- release that changes an output shape must fail here rather than leave the
    -- frozen fixtures below green while real runs are misreported.
    -- pcall: vim.fn.system throws on a missing executable rather than setting
    -- shell_error, and the raw E475 does not say where bashunit comes from.
    local ran, output = pcall(vim.fn.system, { "bashunit", "--version" })
    assert(
      ran and vim.v.shell_error == 0,
      "bashunit did not run; it is declared in Brewfile.dev, the CI toolchain step and the machine package set"
    )
    local installed = parse.version_of(output)
    assert(
      installed == parse.verified_version,
      ("bashunit %s is installed, but every rule and fixture in this project was measured against %s. Re-measure them against %s and move parse.verified_version, or install %s."):format(
        tostring(installed),
        parse.verified_version,
        tostring(installed),
        parse.verified_version
      )
    )
  end,

  ["version_of reads the release out of bashunit's own banner"] = function()
    assert(parse.version_of("\27[1m\27[32mbashunit\27[0m - 0.50.1\n") == "0.50.1")
    assert(parse.version_of("bashunit - 1.0.10") == "1.0.10")
    assert(parse.version_of("command not found") == nil)
    assert(parse.version_of(nil) == nil)
  end,

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

  ["test_functions takes a non-ASCII name bashunit runs"] = function()
    -- bashunit selects a test with `case $fn in test_*` over `compgen -A
    -- function`, so the name after `test_` is whatever bash allows. Measured on
    -- 0.50.1: test_éclair runs and is titled "éclair".
    local found = parse.test_functions({ "function test_\195\169clair() {" })
    assert(#found == 1, "expected test_éclair, got " .. #found)
    assert(found[1].name == "test_\195\169clair")
    assert(parse.humanize(found[1].name) == "\195\169clair", "bashunit upcases a-z only")
  end,

  ["test_functions refuses a name without the runtime test_ prefix"] = function()
    -- The other half of the same rule. Measured on 0.50.1: neither testCamel
    -- nor testable is run, so offering either is a position that can never go
    -- green. bashunit's line-lookup grep does match them, which is why that
    -- grep is the wrong thing to mirror.
    local found = parse.test_functions({
      "function testCamel() {",
      "testable() {",
      "test() {",
      "function test_() {",
    })
    assert(#found == 0, "expected nothing runnable, got " .. #found)
  end,

  ["test_functions takes the spaced parentheses bash accepts"] = function()
    -- Measured on 0.50.1: `test_spaced ( )` is defined by bash, enumerated by
    -- compgen and run. bashunit's own line grep would miss it.
    local found = parse.test_functions({ "test_spaced ( ) {", "function test_kw ( ) {" })
    assert(#found == 2, "expected both spaced definitions, got " .. #found)
  end,

  ["test_functions finds both spellings bashunit runs"] = function()
    local found = parse.test_functions(lines_of(table.concat({
      "#!/usr/bin/env bash",
      "function test_with_keyword() {",
      "  assert_same 1 1",
      "}",
      "test_bare_style() {",
      "  assert_same 2 2",
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
      "test() {", -- `test` alone: bashunit needs the underscore and one more character
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
        "  assert_same 1 1",
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
        "  assert_same 1 1",
        "}",
        "",
        "function test_second() {", -- line 5
        "  assert_same 2 2",
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

  ["failing_lines takes a lone Source candidate as the assertion's line"] = function()
    local candidates = parse.failing_lines(OUTPUT)["/w/lines.test.sh\0Second fails here"]
    assert(candidates and #candidates == 1 and candidates[1] == 13, vim.inspect(candidates))
  end,

  ["failing_lines keys a failure by its file, not by its title alone"] = function()
    -- Two files, one title, two different failing lines. Keyed by title alone
    -- the later block silently overwrites the earlier one and both tests jump
    -- to the wrong file's line.
    local lines = parse.failing_lines(OUTPUT)
    assert(lines["/w/lines.test.sh\0Second fails here"][1] == 13)
    assert(lines["/w/other.test.sh\0Second fails here"][1] == 100)
  end,

  ["failing_lines reports every Source candidate, not just the first"] = function()
    -- bashunit lists every textual assertion in the function, so a block with
    -- more than one candidate does not identify the assertion that failed. The
    -- caller needs to see that rather than be handed the first line.
    local candidates = parse.failing_lines(OUTPUT)["/w/lines.test.sh\0Two assertions"]
    assert(#candidates == 2, "expected 2 candidates, got " .. #candidates)
    assert(candidates[1] == 16 and candidates[2] == 17)
  end,

  ["failing_lines reports nothing when nothing failed"] = function()
    local lines = parse.failing_lines("Tests:      3 passed, 3 total\r\n")
    assert(next(lines) == nil)
    assert(next(parse.failing_lines(nil)) == nil)
  end,

  ["ambiguous_positions names both sides of a collided title"] = function()
    -- test_dupe and test_Dupe both become "Dupe" (measured: bashunit runs both
    -- and reports two rows under that one name), so no report row can be
    -- attributed to either. Refusing both is the only honest answer.
    local ambiguous = parse.ambiguous_positions({
      { id = "/w/a.test.sh::test_dupe", name = "Dupe", path = "/w/a.test.sh" },
      { id = "/w/a.test.sh::test_Dupe", name = "Dupe", path = "/w/a.test.sh" },
      { id = "/w/a.test.sh::test_solo", name = "Solo", path = "/w/a.test.sh" },
    })
    assert(ambiguous["/w/a.test.sh::test_dupe"], "the first side must be refused")
    assert(ambiguous["/w/a.test.sh::test_Dupe"], "the second side must be refused too")
    assert(ambiguous["/w/a.test.sh::test_solo"] == nil)
    local message = ambiguous["/w/a.test.sh::test_dupe"]
    assert(message:find("test_dupe", 1, true), message)
    assert(message:find("test_Dupe", 1, true), message)
    assert(message:find("Dupe", 1, true), message)
  end,

  ["the same title in two different files is not ambiguous"] = function()
    local ambiguous = parse.ambiguous_positions({
      { id = "/w/a.test.sh::test_same", name = "Same", path = "/w/a.test.sh" },
      { id = "/w/b.test.sh::test_same", name = "Same", path = "/w/b.test.sh" },
    })
    assert(next(ambiguous) == nil, "a report row names its file, so these are told apart")
  end,

  ["positions carry the ambiguity verdict discovery computed"] = function()
    local list = parse.positions("/w/a.test.sh", { "test_dupe() {", "}", "test_Dupe() {", "}" })
    assert(list[2].ambiguous and list[3].ambiguous, "both sides carry it")
  end,

  ["exclude_filters names every sibling the substring filter would drag in"] = function()
    -- --filter is `case $fn in test_*<needle>*`, so selecting test_alpha also
    -- runs test_alpha_extended (measured). Excluding the siblings by full name
    -- is what makes a single-test run single.
    local excludes = parse.exclude_filters("test_alpha", {
      "test_alpha",
      "test_alpha_extended",
      "test_alpha_more",
      "test_beta_alpha", -- the needle is `alpha`, not the whole name: this matches too
      "test_beta",
    })
    table.sort(excludes)
    assert(#excludes == 3, "expected 3 excludes, got " .. #excludes)
    assert(excludes[1] == "test_alpha_extended")
    assert(excludes[2] == "test_alpha_more")
    assert(excludes[3] == "test_beta_alpha")
  end,

  ["exclude_filters stays empty when no sibling contains the name"] = function()
    -- An exclude that matched the selected test would run nothing at all.
    assert(#parse.exclude_filters("test_alpha_extended", { "test_alpha", "test_alpha_extended" }) == 0)
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

  ["match_rows refuses a position discovery marked ambiguous"] = function()
    local positions = parse.positions("/w/a.test.sh", { "test_dupe() {", "}", "test_Dupe() {", "}" })
    local matched = parse.match_rows({
      { file = "/w/a.test.sh", name = "Dupe", status = "failed" },
      { file = "/w/a.test.sh", name = "Dupe", status = "passed" },
    }, { positions[2], positions[3] })
    assert(next(matched) == nil, "neither side may take a row it cannot be shown to own")
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
