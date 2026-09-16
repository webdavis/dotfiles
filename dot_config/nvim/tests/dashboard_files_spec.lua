-- custom_api.dashboard_files: the dashboard's file list ranks uncommitted work
-- above merely recent files, and reports each path exactly as git gave it.
--
-- Real git repositories in real temporary directories, because the behavior
-- under test IS the parsing of `git status --porcelain` output, and a fixture
-- string would answer for a format this module never actually meets. Each case
-- builds its own repository, so nothing depends on the checkout the suite runs
-- in.

-- Required per case rather than once at the top of the file, so a missing
-- module fails every case by name instead of aborting the run before the first.
local function dashboard_files()
  return require("custom_api.dashboard_files")
end

local function run(command, cwd)
  local result = vim.system(command, { cwd = cwd, text = true }):wait()
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
  -- Resolved, because on macOS the temp tree lives under a symlink
  -- (`/var` to `/private/var`) and git reports the resolved path. Comparing an
  -- unresolved expectation against it fails on the link rather than the logic.
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

---`changed_files` reads the working directory, so a case has to be standing in
---the repository it built.
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
    -- THE REGRESSION THIS FILE EXISTS FOR. `git status --porcelain` writes an
    -- unstaged change as " M path", with a LEADING SPACE, and the first version
    -- of this module trimmed the command's whole output before splitting it.
    -- That strips the first line's leading space only, so the first path came
    -- back one character short: `dot_config/...` arrived as `ot_config/...`.
    -- Every following line was correct, which is what made it easy to miss.
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
    -- The sibling case, and the reason the bug survived a first glance: `??`
    -- fills both status columns, so an untracked path has no leading space to
    -- lose and reads correctly even under the broken trim.
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
    -- `R  old -> new` carries both names. The new one is the one worth opening,
    -- and the old one no longer exists, so a row pointing at it would fail.
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
    -- The whole premise of the module: a dirty file is the strongest available
    -- signal for why the editor was opened, so it takes the first row even when
    -- another file was opened more recently.
    local module = dashboard_files()
    local root = repository()
    write(root .. "/committed.txt", "one\ntwo\n")
    -- Outside the repository on purpose: a file written inside it would be
    -- untracked, which makes it a CHANGED file and not the merely-recent one
    -- this case is about.
    local elsewhere = vim.fn.resolve(vim.fn.tempname())
    assert(vim.fn.mkdir(elsewhere, "p") == 1, "could not create " .. elsewhere)
    local recent = elsewhere .. "/elsewhere.txt"
    write(recent, "unrelated\n")

    inside(root, function()
      -- `recent_files` reads `vim.v.oldfiles`, which a `--clean` run leaves
      -- empty, so it is replaced for the length of this case.
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
      -- The changed row is the one that survives, because it is the row that
      -- carries the marker telling the operator there is unfinished work.
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
    -- A dashboard that errors is worse than a dashboard missing a section, so
    -- no repository means no changed files rather than a raise.
    local module = dashboard_files()
    local outside = vim.fn.tempname()
    assert(vim.fn.mkdir(outside, "p") == 1, "could not create " .. outside)

    inside(outside, function()
      assert(#module.changed_files() == 0, "found changed files with no repository")
    end)
  end,

  ["a path is shown as its filename and one parent directory"] = function()
    -- Neovim's own abbreviation renders the acceptance drill's path as
    -- `~/w/I/w/d/d/a/nvim-acceptance-drills.md`, which the operator named as the
    -- dashboard's worst legibility problem. One parent is enough to tell two
    -- files of the same name apart.
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
    -- The module first cut with `sub`, which counts bytes, so a name of
    -- multi-byte characters came back with half a character at its end and a
    -- row wider than the limit allowed.
    local module = dashboard_files()
    local _, directory = module.split_for_display("/a/" .. string.rep("é", 40) .. "/file.lua")
    local body = directory:gsub("/$", "")
    assert(vim.fn.strchars(body) <= 18, "the directory was " .. vim.fn.strchars(body) .. " characters")
    -- A byte-wise cut leaves an invalid sequence, which `strchars` counts but
    -- which is not the character that was there. Round-tripping catches it.
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

  ["every row carries a key and an action that opens its own file"] = function()
    local module = dashboard_files()
    local root = repository()
    write(root .. "/committed.txt", "one\ntwo\n")

    inside(root, function()
      local items = module.section(9, 60)
      assert(#items == 1, "expected one item, got " .. #items)
      -- The first row is the resume row: Enter, not a digit, because continuing
      -- is the most likely reason the dashboard is on screen at all.
      assert(items[1].key == "<CR>", "the first row answers to " .. tostring(items[1].key))
      assert(
        items[1].action:find("committed.txt", 1, true),
        "the action does not name its own file: " .. items[1].action
      )
      assert(items[1].file == root .. "/committed.txt", "the item's file is " .. tostring(items[1].file))
    end)
  end,
}
