local tests = vim.fn.fnamemodify(debug.getinfo(1, "S").source:sub(2), ":h")
local fixture = dofile(tests .. "/uu_fixture.lua")
local function run()
  return require("uu.auto_commit").run
end
local function failed(result, f)
  assert(result.kind == "finished" and result.status == 1, vim.inspect(result))
  assert(table.concat(result.lines, "\n"):find(f.options.recovery, 1, true))
end
local function retained(f)
  assert(vim.uv.fs_stat(f.options.recovery), "recovery was lost")
end

return {
  ["a_failed_recovery_record_write_prevents_any_plugin_update"] = function()
    local execute, f = run(), fixture.new()
    fixture.write(f.root .. "/blocked", "a file\n")
    f.options.recovery = f.root .. "/blocked/recovery.json"
    failed(execute(f.options), f)
    assert(f.updates == 0 and fixture.read(f.source) == f.original)
  end,
  ["a_preflight_deployed_lock_disagreement_falls_back_to_report_only"] = function()
    local execute = run()
    local f = fixture.new()
    fixture.write(f.deployed, f.candidate)
    local result = execute(f.options)
    assert(result.kind == "check" and #result.lines > 0, vim.inspect(result))
    assert(f.updates == 0 and not vim.uv.fs_stat(f.options.recovery))
  end,
  ["a_preflight_installed_revision_disagreement_falls_back_to_report_only"] = function()
    local execute = run()
    local f = fixture.new()
    fixture.git(f.plugin, "checkout", f.new)
    local result = execute(f.options)
    assert(result.kind == "check" and #result.lines > 0, vim.inspect(result))
    assert(f.updates == 0 and not vim.uv.fs_stat(f.options.recovery))
  end,
  ["an_unchanged_candidate_closes_recovery_without_an_empty_commit"] = function()
    local execute, f = run(), fixture.new()
    f.options.update = function()
      f.updates = f.updates + 1
      return {}, 0
    end
    local result = execute(f.options)
    assert(result.status == 0 and table.concat(result.lines, "\n"):find("unchanged", 1, true))
    assert(fixture.git(f.repo, "rev-parse", "HEAD") == f.head and f.updates == 1)
    assert(not vim.uv.fs_stat(f.options.recovery))
  end,
  ["an_update_failure_retains_recovery_and_never_reports_completion"] = function()
    local execute = run()
    local f = fixture.new()
    local update = f.options.update
    f.options.update = function()
      update()
      error("owned update failed")
      return {}, 0
    end
    local result = execute(f.options)
    failed(result, f)
    retained(f)
    assert(vim.json.decode(fixture.read(f.options.recovery)).candidate == f.candidate)
    assert(fixture.git(f.repo, "rev-parse", "HEAD") == f.head)
  end,
  ["a_copy_failure_retains_recovery_and_never_reports_completion"] = function()
    local execute = run()
    local f = fixture.new()
    local update = f.options.update
    f.options.update = function()
      update()
      assert(vim.uv.fs_chmod(vim.fn.fnamemodify(f.source, ":h"), 320))
      return {}, 0
    end
    local result = execute(f.options)
    assert(vim.uv.fs_chmod(vim.fn.fnamemodify(f.source, ":h"), 448))
    failed(result, f)
    retained(f)
    assert(vim.json.decode(fixture.read(f.options.recovery)).candidate == f.candidate)
    assert(fixture.git(f.repo, "rev-parse", "HEAD") == f.head)
  end,
  ["a_commit_failure_retains_recovery_and_never_reports_completion"] = function()
    local execute = run()
    local f = fixture.new()
    local update = f.options.update
    f.options.update = function()
      update()
      return {}, 0
    end
    fixture.hook(f, "printf 'owned hook rejected' >&2; exit 7")
    local result = execute(f.options)
    failed(result, f)
    retained(f)
    assert(vim.json.decode(fixture.read(f.options.recovery)).candidate == f.candidate)
    assert(fixture.git(f.repo, "rev-parse", "HEAD") == f.head)
  end,
  ["an_open_recovery_blocks_updates_even_after_auto_commit_is_disabled"] = function()
    local execute, f = run(), fixture.new()
    local update = f.options.update
    f.options.update = function()
      update()
      error("owned interruption")
    end
    failed(execute(f.options), f)
    f.options.auto_commit = false
    failed(execute(f.options), f)
    retained(f)
    assert(f.updates == 1)
  end,
  ["reconciliation_requires_clean_committed_deployed_and_installed_pins_to_agree"] = function()
    local execute, f = run(), fixture.new()
    local update = f.options.update
    f.options.update = function()
      update()
      error("owned interruption")
    end
    failed(execute(f.options), f)
    f.options.auto_commit = false
    fixture.write(f.source, f.candidate)
    failed(execute(f.options), f)
    fixture.git(f.repo, "commit", "--only", "-m", "test: reconcile chosen pins", "--", "dot_config/nvim/lazy-lock.json")
    fixture.write(f.deployed, f.original)
    failed(execute(f.options), f)
    fixture.write(f.deployed, f.candidate)
    fixture.git(f.plugin, "checkout", f.old)
    failed(execute(f.options), f)
    fixture.git(f.plugin, "checkout", f.new)
    local result = execute(f.options)
    assert(result.kind == "check" and not vim.uv.fs_stat(f.options.recovery), vim.inspect(result))
    assert(f.updates == 1)
  end,
  ["a_source_edit_during_update_is_preserved_and_leaves_recovery_open"] = function()
    local execute = run()
    local f = fixture.new()
    local update = f.options.update
    f.options.update = function()
      update()
      fixture.write(f.source, "operator edit\n")
      return {}, 0
    end
    failed(execute(f.options), f)
    retained(f)
    assert(fixture.read(f.source) == "operator edit\n")
  end,
  ["a_branch_change_during_update_is_preserved_and_leaves_recovery_open"] = function()
    local execute = run()
    local f = fixture.new()
    local update = f.options.update
    f.options.update = function()
      update()
      fixture.git(f.repo, "switch", "-c", "operator")
      return {}, 0
    end
    failed(execute(f.options), f)
    retained(f)
    assert(fixture.git(f.repo, "symbolic-ref", "HEAD") == "refs/heads/operator")
  end,
  ["an_index_change_during_update_is_preserved_and_leaves_recovery_open"] = function()
    local execute = run()
    local f = fixture.new()
    local update = f.options.update
    f.options.update = function()
      update()
      fixture.write(f.source, f.candidate)
      fixture.git(f.repo, "add", "dot_config/nvim/lazy-lock.json")
      fixture.write(f.source, f.original)
      return {}, 0
    end
    failed(execute(f.options), f)
    retained(f)
    assert(fixture.git(f.repo, "show", ":dot_config/nvim/lazy-lock.json") .. "\n" == f.candidate)
  end,
  ["a_rejected_hook_preserves_unrelated_staged_paths_and_the_candidate_lock"] = function()
    local execute, f = run(), fixture.new()
    local staged = fixture.staged_other(f)
    fixture.hook(f, "printf 'owned review rejection' >&2; exit 7")
    failed(execute(f.options), f)
    retained(f)
    assert(fixture.git(f.repo, "rev-parse", ":unrelated.txt") == staged)
    assert(fixture.git(f.repo, "show", "HEAD:unrelated.txt") == "original unrelated")
    assert(fixture.read(f.source) == f.candidate and fixture.read(f.deployed) == f.candidate)
    local recovery = vim.json.decode(fixture.read(f.options.recovery))
    assert(recovery.lock == f.original and recovery.candidate == f.candidate)
  end,
  ["a_successful_hook_commits_only_the_lock_and_preserves_other_staged_paths"] = function()
    local execute, f = run(), fixture.new()
    local staged = fixture.staged_other(f)
    fixture.hook(f, "printf 'accepted\\n' >> .git/owned-hook.log")
    local result = execute(f.options)
    assert(result.status == 0 and result.changed, vim.inspect(result))
    assert(fixture.read(f.repo .. "/.git/owned-hook.log") == "accepted\n")
    assert(
      fixture.git(f.repo, "diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD")
        == "dot_config/nvim/lazy-lock.json"
    )
    assert(fixture.git(f.repo, "rev-parse", ":unrelated.txt") == staged)
    assert(fixture.git(f.repo, "show", "HEAD:dot_config/nvim/lazy-lock.json") .. "\n" == f.candidate)
    assert(not vim.uv.fs_stat(f.options.recovery))
  end,
}
