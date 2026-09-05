-- The runner's own two self-checks (spec 6.3). A runner that reported failure
-- only in its output would be a gate that cannot fail, and an empty run would
-- otherwise exit 0 and read as a pass, so both are pinned against a scratch
-- copy of run.lua whose directory holds exactly the spec files the case needs
-- (run.lua reads its specs from its own directory).

local runner = arg[0]

local function scratch_runner(spec_source)
  local dir = vim.fn.tempname()
  assert(vim.fn.mkdir(dir, "p") == 1, "mkdir " .. dir)
  assert(vim.uv.fs_copyfile(runner, dir .. "/run.lua"), "copy " .. runner)
  if spec_source then
    assert(vim.fn.writefile({ spec_source }, dir .. "/failing_spec.lua") == 0, "write failing_spec")
  end
  return dir .. "/run.lua"
end

local function run(...)
  local result = vim.system({ vim.v.progpath, "--headless", "--clean", "-l", ... }, { text = true }):wait()
  return result.code, result.stdout .. result.stderr
end

return {
  ["a failing case exits non-zero"] = function()
    local copy = scratch_runner('return { ["deliberately fails"] = function() assert(false, "as designed") end }')
    local code, output = run(copy, "failing_spec")
    assert(code == 1, "exit code " .. code .. ": " .. output)
    assert(output:find("FAIL failing_spec: deliberately fails", 1, true), output)
  end,

  ["a run that matched no spec files is refused"] = function()
    local code, output = run(scratch_runner())
    assert(code ~= 0, "exit code " .. code .. ": " .. output)
    assert(output:find("no spec files matched", 1, true), output)
  end,
}
