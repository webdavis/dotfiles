local Git = require("uu.writeback_git")
local State = require("uu.writeback_state")
local Report = require("uu.report")
local M = {}

local function check(reason)
  return { kind = "check", lines = reason and { "plugin write-back: " .. reason .. "; checking only" } or {} }
end

local function agreement(repo, config, plugins)
  local identity = Git.identity(repo)
  local allowed, why = Report.commit_allowed(identity.porcelain, identity.branch)
  assert(allowed, why)
  local source = State.read(repo .. "/" .. Git.LOCK)
  assert(source == identity.committed and source == identity.index, "source or index lock disagrees with HEAD")
  assert(source == State.read(config .. "/lazy-lock.json"), "deployed lock disagrees with the source lock")
  Git.installed_agree(source, plugins)
  return identity, source
end

local function execute(options)
  local open = State.load(options.recovery)
  if open then
    assert(open.config == options.config, "recovery belongs to a different config")
    agreement(open.repo, open.config, options.plugins)
    State.close(options.recovery)
  end
  if not options.auto_commit then
    return check()
  end

  local allowed, starting, lock = pcall(agreement, options.repo, options.config, options.plugins)
  if not allowed then
    return check(tostring(starting))
  end
  local record = {
    version = 1,
    repo = options.repo,
    config = options.config,
    branch = starting.branch,
    head = starting.head,
    lock = lock,
    index = starting.index,
  }
  State.save(options.recovery, record)
  local updated, lines, status = pcall(options.update)
  record.candidate = State.read(options.config .. "/lazy-lock.json")
  State.save(options.recovery, record)
  assert(updated, "plugin update failed: " .. tostring(lines))
  assert(status == Report.OK, "plugin update failed: " .. table.concat(lines, "; "))
  local current = Git.identity(options.repo)
  assert(current.branch == record.branch and current.head == record.head, "branch or HEAD changed during update")
  assert(
    State.read(options.repo .. "/" .. Git.LOCK) == record.lock and current.index == record.index,
    "source or index lock changed during update"
  )
  Git.installed_agree(record.candidate, options.plugins)
  if record.candidate == record.lock then
    agreement(options.repo, options.config, options.plugins)
    State.close(options.recovery)
    return {
      kind = "finished",
      status = Report.OK,
      lines = { "plugin pins unchanged; no commit needed" },
      changed = false,
    }
  end
  State.write(options.repo .. "/" .. Git.LOCK, record.candidate)
  local commit = Git.commit(options.repo)
  local committed, source = agreement(options.repo, options.config, options.plugins)
  assert(committed.head == commit and source == record.candidate, "committed lock differs from the update candidate")
  State.close(options.recovery)
  lines[#lines + 1] = "plugin pins committed: " .. commit
  return { kind = "finished", status = Report.OK, lines = lines, changed = true }
end

function M.run(options)
  local ok, result = pcall(execute, options)
  if ok then
    return result
  end
  return {
    kind = "finished",
    status = Report.FAILED,
    lines = { "plugin write-back failed: " .. tostring(result) .. "; recovery: " .. options.recovery },
  }
end

return M
