-- Real git repositories in temporary directories, because the test covers parsing real `git status --porcelain` output.

-- Required per case, so a missing module fails each case by name.
local function dashboard_files()
  return require("custom_api.dashboard_files")
end

-- Git exports these into hooks, and GIT_DIR overrides `-C` and `cwd`, so scrub them on every call.
local GIT_ENVIRONMENT_TO_SCRUB = {
  "GIT_DIR",
  "GIT_WORK_TREE",
  "GIT_INDEX_FILE",
  "GIT_OBJECT_DIRECTORY",
  "GIT_COMMON_DIR",
}

local function without_git_environment(command)
  local wrapped = { "env" }
  for _, name in ipairs(GIT_ENVIRONMENT_TO_SCRUB) do
    table.insert(wrapped, "-u")
    table.insert(wrapped, name)
  end
  return vim.list_extend(wrapped, command)
end

local function run(command, cwd)
  local result = vim.system(without_git_environment(command), { cwd = cwd, text = true }):wait()
  assert(result.code == 0, table.concat(command, " ") .. " failed: " .. tostring(result.stderr))
end

local function write(path, text)
  local handle = assert(io.open(path, "w"), "could not write " .. path)
  handle:write(text)
  handle:close()
end

---A repository with one committed file, then the changes each case needs.
---@return string root
local function repository()
  -- Resolved, because macOS keeps the temp tree behind a symlink and git reports the resolved path.
  local root = vim.fn.resolve(vim.fn.tempname())
  assert(vim.fn.mkdir(root, "p") == 1, "could not create " .. root)
  run({ "git", "init", "--quiet" }, root)
  run({ "git", "config", "user.email", "spec@example.invalid" }, root)
  run({ "git", "config", "user.name", "Spec" }, root)
  write(root .. "/committed.txt", "one\n")
  run({ "git", "add", "committed.txt" }, root)
  run({ "git", "commit", "--quiet", "-m", "first" }, root)
  return root
end

---Run `body` inside `root`, because `changed_files` reads the working directory.
local function inside(root, body)
  local previous = vim.fn.getcwd()
  vim.cmd.cd(root)
  local ok, err = pcall(body)
  vim.cmd.cd(previous)
  assert(ok, err)
end

local function paths_of(entries)
  local paths = {}
  for _, entry in ipairs(entries) do
    table.insert(paths, entry.path)
  end
  return paths
end

return {
  ["an unstaged change reports its whole path"] = function()
    -- Porcelain writes an unstaged change as " M path", so trimming the whole output would cut the first path.
    local module = dashboard_files()
    local root = repository()
    write(root .. "/committed.txt", "one\ntwo\n")

    inside(root, function()
      local changed = module.changed_files()
      assert(#changed == 1, "expected one changed file, got " .. #changed)
      assert(
        changed[1].path == root .. "/committed.txt",
        "the path came back as " .. changed[1].path .. " rather than " .. root .. "/committed.txt"
      )
      assert(changed[1].status == "M", "the status came back as " .. changed[1].status)
    end)
  end,

  ["an untracked file reports its whole path too"] = function()
    -- `??` fills both status columns, so an untracked path has no leading space.
    local module = dashboard_files()
    local root = repository()
    write(root .. "/fresh.txt", "new\n")

    inside(root, function()
      local changed = module.changed_files()
      assert(#changed == 1, "expected one changed file, got " .. #changed)
      assert(changed[1].path == root .. "/fresh.txt", "the path came back as " .. changed[1].path)
      assert(changed[1].status == "??", "the status came back as " .. changed[1].status)
    end)
  end,

  ["a renamed file reports the name it now has"] = function()
    -- A rename lists the new name, because the old one no longer exists.
    local module = dashboard_files()
    local root = repository()
    run({ "git", "mv", "committed.txt", "renamed.txt" }, root)

    inside(root, function()
      local changed = module.changed_files()
      assert(#changed == 1, "expected one changed file, got " .. #changed)
      assert(
        changed[1].path == root .. "/renamed.txt",
        "the path came back as " .. changed[1].path .. ", so the row would open a file that is gone"
      )
    end)
  end,

  ["uncommitted work outranks a merely recent file"] = function()
    -- A dirty file takes the first row even when another was opened more recently.
    local module = dashboard_files()
    local root = repository()
    write(root .. "/committed.txt", "one\ntwo\n")
    -- Outside the repository, because a file inside it would count as changed.
    local elsewhere = vim.fn.resolve(vim.fn.tempname())
    assert(vim.fn.mkdir(elsewhere, "p") == 1, "could not create " .. elsewhere)
    local recent = elsewhere .. "/elsewhere.txt"
    write(recent, "unrelated\n")

    inside(root, function()
      -- `--clean` leaves `vim.v.oldfiles` empty, so `recent_files` is replaced for this case.
      local real_recent_files = module.recent_files
      module.recent_files = function()
        return { recent }
      end

      local paths = paths_of(module.entries(9))
      module.recent_files = real_recent_files

      assert(#paths == 2, "expected two rows, got " .. #paths)
      assert(paths[1] == root .. "/committed.txt", "the first row is " .. paths[1])
      assert(paths[2] == recent, "the second row is " .. paths[2])
    end)
  end,

  ["a file that is both dirty and recent takes one row, with its status"] = function()
    local module = dashboard_files()
    local root = repository()
    local both = root .. "/committed.txt"
    write(both, "one\ntwo\n")

    inside(root, function()
      local real_recent_files = module.recent_files
      module.recent_files = function()
        return { both }
      end

      local entries = module.entries(9)
      module.recent_files = real_recent_files

      assert(#entries == 1, "the same file took " .. #entries .. " rows")
      -- The changed row survives, because it carries the status marker.
      assert(entries[1].status == "M", "the surviving row lost its status")
    end)
  end,

  ["the row limit is respected"] = function()
    local module = dashboard_files()
    local root = repository()
    for index = 1, 5 do
      write(root .. "/file" .. index .. ".txt", "body\n")
    end

    inside(root, function()
      assert(#module.entries(3) == 3, "the limit did not hold")
      assert(#module.entries(0) == 0, "a limit of zero still produced rows")
    end)
  end,

  ["outside a repository the list falls back to recent files"] = function()
    -- No repository means no changed files, never an error.
    local module = dashboard_files()
    local outside = vim.fn.tempname()
    assert(vim.fn.mkdir(outside, "p") == 1, "could not create " .. outside)

    inside(outside, function()
      assert(#module.changed_files() == 0, "found changed files with no repository")
    end)
  end,

  ["a path is shown as its filename and one parent directory"] = function()
    -- One parent directory is enough to tell two same-named files apart.
    local module = dashboard_files()
    local name, directory = module.split_for_display("/Users/someone/project/lua/plugins/snacks.lua")
    assert(name == "snacks.lua", "the filename came back as " .. name)
    assert(directory == "plugins/", "the directory came back as " .. directory)
  end,

  ["a long directory name is cut short rather than pushing the row over"] = function()
    local module = dashboard_files()
    local _, directory = module.split_for_display("/a/" .. string.rep("z", 40) .. "/file.lua")
    local columns = vim.fn.strdisplaywidth(directory)
    assert(columns <= 19, "the directory was " .. columns .. " columns wide: " .. directory)
    assert(directory:find("…", 1, true), "a cut directory does not say it was cut: " .. directory)
  end,

  ["a multi-byte directory name is cut on a character boundary"] = function()
    -- Cut by characters, so a multi-byte name stays whole and within the limit.
    local module = dashboard_files()
    local _, directory = module.split_for_display("/a/" .. string.rep("é", 40) .. "/file.lua")
    local body = directory:gsub("/$", "")
    assert(vim.fn.strchars(body) <= 18, "the directory was " .. vim.fn.strchars(body) .. " characters")
    -- Round-tripping catches a broken byte sequence that `strchars` would still count.
    assert(vim.fn.strcharpart(body, 0, 1) == "é", "the first character came back as a broken sequence")
  end,

  ["the rows after the first answer to digits"] = function()
    local module = dashboard_files()
    local root = repository()
    write(root .. "/committed.txt", "one\ntwo\n")
    write(root .. "/second.txt", "body\n")
    write(root .. "/third.txt", "body\n")

    inside(root, function()
      local items = module.section(9, 60)
      assert(#items == 3, "expected three items, got " .. #items)
      assert(items[2].key == "1", "the second row answers to " .. tostring(items[2].key))
      assert(items[3].key == "2", "the third row answers to " .. tostring(items[3].key))
    end)
  end,

  ["an inherited GIT_DIR cannot reach the repository the caller is in"] = function()
    -- GIT_DIR from a hook overrides `-C` and `cwd`, so an unscrubbed module would read and corrupt the hook repository.
    -- The decoy commits a different filename, so a leak changes the result.
    local module = dashboard_files()
    local decoy = repository()
    write(decoy .. "/decoy-only.txt", "decoy\n")
    run({ "git", "add", "decoy-only.txt" }, decoy)
    run({ "git", "commit", "--quiet", "-m", "decoy" }, decoy)

    local subject = repository()
    write(subject .. "/committed.txt", "one\ntwo\n")

    local decoy_config = decoy .. "/.git/config"
    local before = assert(io.open(decoy_config)):read("a")

    local restore = vim.env.GIT_DIR
    vim.env.GIT_DIR = decoy .. "/.git"

    local ok, err = pcall(function()
      inside(subject, function()
        local changed = module.changed_files()
        assert(#changed == 1, "expected the subject's one change, got " .. #changed)
        assert(changed[1].path == subject .. "/committed.txt", "read the wrong repository: " .. changed[1].path)
      end)
    end)

    -- The spec's own git calls run with GIT_DIR on the decoy, so this half checks for corruption.
    local scratch = repository()
    write(scratch .. "/fresh.txt", "x\n")

    vim.env.GIT_DIR = restore

    local after = assert(io.open(decoy_config)):read("a")
    assert(after == before, "the inherited repository's config was rewritten:\n" .. after)
    -- `git init` writes `bare = false` everywhere, so match the corrupted value.
    assert(not after:match("bare%s*=%s*true"), "core.bare = true was written into the inherited repository")
    assert(not after:match("worktree%s*="), "core.worktree was written into the inherited repository")
    assert(vim.fn.isdirectory(scratch .. "/.git") == 1, "the scratch repository was never created")
    assert(ok, err)
  end,

  ["every row carries a key and an action that opens its own file"] = function()
    local module = dashboard_files()
    local root = repository()
    write(root .. "/committed.txt", "one\ntwo\n")

    inside(root, function()
      local items = module.section(9, 60)
      assert(#items == 1, "expected one item, got " .. #items)
      -- The first row is the resume row, on Enter.
      assert(items[1].key == "<CR>", "the first row answers to " .. tostring(items[1].key))
      assert(
        items[1].action:find("committed.txt", 1, true),
        "the action does not name its own file: " .. items[1].action
      )
      assert(items[1].file == root .. "/committed.txt", "the item's file is " .. tostring(items[1].file))
    end)
  end,
}
