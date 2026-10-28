local overseer_title = { title = "Overseer" }
local overseer_watch_run_desc = "Overseer: watch-run"

local function toggle_runner(window)
  local overseer = require("overseer")
  if vim.bo.buftype == "terminal" then
    vim.cmd("close")
    return
  end

  local task_list = require("overseer.task_list")
  local tasks = overseer.list_tasks({
    status = {
      overseer.STATUS.RUNNING,
      overseer.STATUS.SUCCESS,
      overseer.STATUS.FAILURE,
      overseer.STATUS.CANCELED,
    },
    sort = task_list.sort_finished_recently,
  })

  if vim.tbl_isempty(tasks) then
    vim.notify("No tasks found", vim.log.levels.WARN, overseer_title)
  else
    local most_recent = tasks[1]
    overseer.run_action(most_recent, "open " .. window)
  end
end

-- A split that always shows the newest task's output, following every task started later.
local function output_view()
  local overseer = require("overseer")
  vim.cmd("botright split")
  overseer.create_task_output_view(vim.api.nvim_get_current_win(), {
    list_task_opts = {
      filter = function(task)
        return task.time_start ~= nil
      end,
    },
    -- Always the newest task, so the view never stays on the task last hovered in the sidebar.
    select = function(_, tasks)
      -- By start sequence, because `time_start` has one-second resolution and ties.
      -- `time_start` breaks ties for tasks started without a sequence.
      table.sort(tasks, function(a, b)
        local sa = a.metadata and a.metadata.start_sequence or 0
        local sb = b.metadata and b.metadata.start_sequence or 0
        if sa ~= sb then
          return sa > sb
        end
        return (a.time_start or 0) > (b.time_start or 0)
      end)
      return tasks[1]
    end,
  })
end

-- Catches what a provider's cache_key misses, such as a recipe added to a justfile further up the tree.
local function refresh_templates()
  local overseer = require("overseer")
  local dir = vim.uv.cwd()
  overseer.clear_task_cache({ dir = dir })
  overseer.preload_task_cache({ dir = dir }, function()
    vim.notify("Task templates refreshed", vim.log.levels.INFO, overseer_title)
  end)
end

local function view(window)
  return function()
    toggle_runner(window)
  end
end

return {
  "stevearc/overseer.nvim",
  dependencies = { "webdavis/pns.nvim" },
  opts = {},
  -- Every command overseer and `config` define, so any of them loads the plugin.
  -- `OverseerShell` gets its own loader below.
  cmd = {
    "OverseerClose",
    "OverseerDeleteBundle",
    "OverseerLoadBundle",
    "OverseerOpen",
    "OverseerRestartLast",
    "OverseerRun",
    "OverseerSaveBundle",
    "OverseerTaskAction",
    "OverseerToggle",
    "OverseerWatchRun",
  },
  -- lazy's placeholder rebuilds `args` by joining `fargs` with single spaces, which breaks the raw
  -- string `OverseerShell` runs. This proxy replays the untouched `event.args` instead, and deletes
  -- itself first so a failed load ends in E492 rather than recursion.
  init = function()
    vim.api.nvim_create_user_command("OverseerShell", function(event)
      vim.api.nvim_del_user_command("OverseerShell")
      require("lazy").load({ plugins = { "overseer.nvim" } })
      vim.cmd({
        cmd = "OverseerShell",
        -- `nvim_cmd` rejects an empty argument, and a bare `:OverseerShell` prompts for the command.
        args = event.args ~= "" and { event.args } or {},
        bang = event.bang or nil,
        mods = event.smods,
        -- `shellcmdline` completion already expanded `%`, `#` and `<cword>`, so expanding again
        -- would rewrite text the user escaped.
        magic = { file = false },
      })
    end, {
      -- Matches overseer's own declaration, which takes no range or count.
      bang = true,
      nargs = "*",
      -- A built-in completion type, so completion works before overseer loads.
      complete = "shellcmdline",
      desc = "Run a shell command as an overseer task. With `!` the task is created but not started",
    })
  end,
  -- The four `:`-prefixed rows leave the cmdline open for an argument, so they carry no `silent`.
  keys = {
    {
      "<leader>or",
      "<cmd>OverseerOpen!<cr><cmd>OverseerRun<cr>",
      desc = "Overseer: run (and open list)",
      silent = true,
    },
    { "<leader>oR", "<cmd>OverseerRun<cr>", desc = "Overseer: run", silent = true },
    { "<leader>ol", "<cmd>OverseerRestartLast<cr>", desc = "Overseer: run last task", silent = true },
    { "<leader>oo", "<cmd>OverseerOpen<cr>", desc = "Overseer: open (and focus)", silent = true },
    { "<leader>oO", "<cmd>OverseerOpen!<cr>", desc = "Overseer: open (without focus)", silent = true },
    { "<leader>oc", "<cmd>OverseerClose<cr>", desc = "Overseer: close", silent = true },
    { "<leader>ot", "<cmd>OverseerToggle<cr>", desc = "Overseer: toggle (and focus)", silent = true },
    { "<leader>oT", "<cmd>OverseerToggle!<cr>", desc = "Overseer: toggle (without focus)", silent = true },
    { "<leader>ov", output_view, desc = "Overseer: open a live view of the newest task's output" },
    { "<M-'>", output_view, desc = "Overseer: open a live view of the newest task's output" },
    { "<leader>oC", refresh_templates, desc = "Overseer: refresh the task templates", silent = true },
    { "<leader>ob", ":OverseerSaveBundle ", desc = "Overseer: save the task list as a bundle" },
    { "<leader>oB", ":OverseerLoadBundle ", desc = "Overseer: load a task bundle" },
    { "<leader>oX", ":OverseerDeleteBundle ", desc = "Overseer: delete a task bundle" },
    { "<leader>os", ":OverseerShell ", desc = "Overseer: run a shell command as a task" },
    { "<leader>oa", "<cmd>OverseerTaskAction<cr>", desc = "Overseer: run an action on a task", silent = true },
    { '<leader>o"', view("hsplit"), desc = "Overseer: open task in hsplit" },
    { "<M-7>", view("hsplit"), desc = "Overseer: open task in hsplit" },
    { "<leader>o%", view("vsplit"), desc = "Overseer: open task in vsplit" },
    { "<M-8>", view("vsplit"), desc = "Overseer: open task in vsplit" },
    { "<M-;>", view("float"), desc = "Overseer: open task in floating window" },
    -- `<M-[>` starts a terminal escape sequence, so watch-run lives in the `<leader>o` group.
    { "<leader>ow", "<cmd>OverseerWatchRun<cr>", desc = overseer_watch_run_desc, silent = true },
  },
  config = function()
    local overseer = require("overseer")

    -- Parses this repo's Neovim test runner output, `FAIL <spec>: <case>: <file>:<line>: <message>`.
    -- `%*[^:]` stops at the first colon, so a location inside the message is never picked instead.
    local quickfix_errorformat = "FAIL %*[^:]: %*[^:]: %f:%l: %m," .. vim.o.errorformat

    -- `dispose all finished` clears tasks by hand, since tasks are kept past overseer's dispose timeout.
    -- `copy command` shows what a template actually ran.
    local task_actions = {
      ["dispose all finished"] = {
        desc = "Dispose every finished task, not only this one",
        run = function()
          local finished = overseer.list_tasks({
            status = { overseer.STATUS.SUCCESS, overseer.STATUS.FAILURE, overseer.STATUS.CANCELED },
          })
          for _, task in ipairs(finished) do
            task:dispose(true)
          end
          vim.notify(("Disposed %d finished task(s)"):format(#finished), vim.log.levels.INFO, { title = "Overseer" })
        end,
      },
      ["copy command"] = {
        desc = "Copy the task's command to the system clipboard",
        run = function(task)
          local cmd = type(task.cmd) == "table" and table.concat(task.cmd, " ") or tostring(task.cmd)
          vim.fn.setreg("+", cmd)
          vim.notify("Copied: " .. cmd, vim.log.levels.INFO, { title = "Overseer" })
        end,
      },
    }

    overseer.setup({
      -- nvim-dap is present, so preLaunchTask and postDebugTask run as real tasks.
      dap = true,
      -- WARN keeps template-provider errors visible without narrating every task.
      log_level = vim.log.levels.WARN,
      output = {
        -- A terminal buffer, so a task that draws progress bars or colors renders.
        use_terminal = true,
        -- Clear on restart, so a watch task shows only the current run.
        preserve_output = false,
      },
      task_list = {
        direction = "right",
        max_width = { 100, 0.2 },
        min_width = { 40, 0.1 },
        max_height = { 20, 0.1 },
        min_height = 8,
        separator = "────────────────────────────────────────",
        child_indent = { "┃ ", "┣━", "┗━" },
        render = function(task)
          return require("overseer.render").format_standard(task)
        end,
        sort = function(a, b)
          return require("overseer.task_list").default_sort(a, b)
        end,
        -- Merged over the defaults case-insensitively, so only additions belong here.
        keymaps = {
          ["<C-r>"] = { "keymap.run_action", opts = { action = "restart" }, desc = "Restart task" },
          ["s"] = { "keymap.run_action", opts = { action = "start" }, desc = "Start task" },
          ["x"] = { "keymap.run_action", opts = { action = "stop" }, desc = "Stop task" },
          ["r"] = { "keymap.run_action", opts = { action = "retain" }, desc = "Retain task" },
          ["e"] = { "keymap.run_action", opts = { action = "ensure" }, desc = "Ensure task is running" },
          ["<localleader>w"] = { "keymap.run_action", opts = { action = "watch" }, desc = "Watch task" },
          ["<localleader>W"] = { "keymap.run_action", opts = { action = "unwatch" }, desc = "Unwatch task" },
          ["<localleader>q"] = {
            "keymap.run_action",
            opts = { action = "set quickfix diagnostics" },
            desc = "Send task diagnostics to the quickfix",
          },
          ["<localleader>l"] = {
            "keymap.run_action",
            opts = { action = "set loclist diagnostics" },
            desc = "Send task diagnostics to the loclist",
          },
          ["<localleader>y"] = {
            "keymap.run_action",
            opts = { action = "copy command" },
            desc = "Copy the task command",
          },
          ["<localleader>D"] = {
            "keymap.run_action",
            opts = { action = "dispose all finished" },
            desc = "Dispose every finished task",
          },
          [";"] = "keymap.toggle_preview",
        },
      },
      actions = task_actions,
      form = {
        border = "rounded",
        zindex = 40,
        min_width = 80,
        max_width = 0.9,
        min_height = 10,
        max_height = 0.9,
        win_opts = {
          winblend = 10,
        },
      },
      task_win = {
        padding = 2,
        border = "rounded",
        zindex = 40,
        win_opts = {
          winblend = 10,
        },
      },
      component_aliases = {
        -- Leaves out on_complete_dispose, because OverseerRestartLast, the openers and the bundle
        -- commands need finished tasks. The "dispose all finished" action clears them by hand.
        default = {
          "on_exit_set_status",
          "pns.report",
          { "on_complete_notify", system = "unfocused" },
          {
            "on_output_quickfix",
            -- Keeps only lines that parse as a location instead of replacing the quickfix with all output.
            items_only = true,
            set_diagnostics = true,
            open_on_match = true,
          },
          -- Fires only when the errorformat matched, so a clean run leaves no diagnostics.
          { "on_result_diagnostics", remove_on_restart = true },
          -- Stamps each task's start order, which the live output view sorts by.
          "user.start_sequence",
          -- `on_start = "always"`, because its default never fires when on_output_quickfix is present.
          -- A horizontal split, because a docked window needs the task list open.
          { "open_output", on_start = "always", direction = "horizontal", focus = false },
        },
        -- Tasks read out of a .vscode/tasks.json.
        default_vscode = {
          "default",
          "on_result_diagnostics",
          -- A VS Code task's diagnostics come from its problem matcher, not this config's errorformat.
          { "on_result_diagnostics_quickfix", open = true },
        },
        -- Tasks the vim.system and jobstart wrappers create. They are other plugins' background jobs,
        -- so they clean up after themselves.
        default_builtin = {
          "on_exit_set_status",
          "on_complete_dispose",
          { "unique", soft = true },
        },
        -- neotest runs, when neotest's overseer consumer is registered.
        default_neotest = {
          "on_exit_set_status",
          { "on_complete_notify", system = "unfocused" },
          { "on_output_quickfix", items_only = true, set_diagnostics = true },
          { "on_result_diagnostics", remove_on_restart = true },
          "user.start_sequence",
        },
        -- Routes a task's diagnostics to trouble.nvim instead of the quickfix.
        trouble = {
          "default",
          { "on_result_diagnostics_trouble", close = true },
        },
        -- A watch task re-runs on every write, so it notifies only when the verdict changes and
        -- cancels a run that hangs.
        watched = {
          "on_exit_set_status",
          { "on_complete_notify", on_change = true, system = "unfocused" },
          { "on_output_quickfix", items_only = true, set_diagnostics = true },
          { "on_result_diagnostics", remove_on_restart = true },
          { "on_result_notify", on_change = true, system = "unfocused" },
          { "timeout", timeout = 600 },
          "user.start_sequence",
        },
        -- Kept out of `default`, where restarting on every failure would loop.
        retry = {
          "default",
          { "on_complete_restart", statuses = { "FAILURE" }, delay = 2000 },
        },
        -- Shows a live output summary in the notification.
        live_notify = {
          "on_exit_set_status",
          { "on_output_notify", output_on_complete = true, max_lines = 3 },
          { "on_output_quickfix", items_only = true, set_diagnostics = true },
          { "on_result_diagnostics", remove_on_restart = true },
          "user.start_sequence",
        },
      },
      -- This config's templates live under the default `lua/overseer/template/**` glob. Project
      -- tasks come from the `.overseer/` provider and `.vscode/tasks.json`.
      template_dirs = {},
      -- This key only removes providers, and every builtin is wanted.
      disable_template_modules = {},
      template_timeout_ms = 3000,
      template_cache_threshold_ms = 200,
      -- Every vim.system and jobstart call becomes a task, so a plugin's background job is
      -- inspectable with `g.` in the task list.
      --
      -- A table command whose binary is missing goes to the builtin, which raises as documented.
      -- overseer's wrapper would return a nil handle instead. A string `jobstart` always spawns the shell.
      experimental_wrap_builtins = {
        enabled = true,
        condition = function(cmd)
          if type(cmd) == "string" then
            return true
          end
          local binary = type(cmd) == "table" and cmd[1] or nil
          return type(binary) == "string" and vim.fn.executable(binary) == 1
        end,
      },
    })

    -- The generic errorformat is set as a task default, so a template's own format still wins.
    overseer.add_template_hook(nil, function(task_defn)
      -- `components` is the shared alias table, and initialization writes into it, so each task
      -- gets its own copy.
      task_defn.components = vim.deepcopy(task_defn.components)

      local defaults = task_defn.default_component_params or {}
      if defaults.errorformat == nil then
        defaults.errorformat = quickfix_errorformat
        task_defn.default_component_params = defaults
      end
    end)

    -- `unique` makes re-running a `just` recipe replace its last run instead of stacking a copy.
    overseer.add_template_hook({ module = "^just$" }, function(task_defn, util)
      -- `unique` compares names, so the name carries the directory to keep worktrees apart, and the
      -- command when the `just` builder gives no name.
      if task_defn.cwd then
        -- JSON keeps argument boundaries, so {"a b", "c"} and {"a", "b c"} get different names.
        local name = task_defn.name
          or (type(task_defn.cmd) == "table" and vim.json.encode(task_defn.cmd))
          or tostring(task_defn.cmd)
        task_defn.name = ("%s (%s)"):format(name, vim.fn.fnamemodify(task_defn.cwd, ":~"))
      end
      util.add_component(task_defn, { "unique", replace = true })
    end)

    -- Warms the template cache, because the first :OverseerRun runs `just --dump` for each justfile
    -- above the cwd.
    vim.api.nvim_create_autocmd({ "VimEnter", "DirChanged" }, {
      group = vim.api.nvim_create_augroup("OverseerPreloadTemplates", { clear = true }),
      desc = "Overseer: warm the task template cache for this directory",
      callback = function()
        overseer.preload_task_cache({ dir = vim.v.cwd ~= "" and vim.v.cwd or vim.uv.cwd() })
      end,
    })

    vim.api.nvim_create_user_command("OverseerRestartLast", function()
      local task_list = require("overseer.task_list")
      local tasks = overseer.list_tasks({
        status = {
          overseer.STATUS.SUCCESS,
          overseer.STATUS.FAILURE,
          overseer.STATUS.CANCELED,
        },
        sort = task_list.sort_finished_recently,
      })
      if vim.tbl_isempty(tasks) then
        vim.notify("No tasks found", vim.log.levels.WARN, overseer_title)
      else
        local most_recent = tasks[1]
        overseer.run_action(most_recent, "restart")
      end
    end, {})

    -- Task bundles are tasks saved with `Task:serialize()` and restored with `new_task()`.
    -- Loading a bundle adds to the task list, so run "dispose all finished" first for a clean list.
    local bundle_dir = vim.fs.joinpath(vim.fn.stdpath("state"), "overseer", "bundles")

    local function bundle_path(name)
      return vim.fs.joinpath(bundle_dir, name .. ".json")
    end

    local function bundle_names()
      local names = {}
      for name, kind in vim.fs.dir(bundle_dir) do
        if kind == "file" and name:match("%.json$") then
          table.insert(names, (name:gsub("%.json$", "")))
        end
      end
      table.sort(names)
      return names
    end

    -- A bundle with no name is this project's bundle.
    local function default_bundle_name()
      return vim.fs.basename(vim.uv.cwd() or "overseer")
    end

    local function complete_bundle(arglead)
      return vim.tbl_filter(function(name)
        return vim.startswith(name, arglead)
      end, bundle_names())
    end

    vim.api.nvim_create_user_command("OverseerSaveBundle", function(args)
      local name = args.args ~= "" and args.args or default_bundle_name()
      local tasks = overseer.list_tasks({})
      if vim.tbl_isempty(tasks) then
        vim.notify("No tasks to save", vim.log.levels.WARN, overseer_title)
        return
      end
      local serialized = vim.tbl_map(function(task)
        local defn = task:serialize()
        -- `Task:serialize()` drops `default_component_params`, and the quickfix action reads them.
        defn.default_component_params = task.default_component_params
        return defn
      end, tasks)
      vim.fn.mkdir(bundle_dir, "p")
      -- writefile reports failure as a non-zero return.
      if vim.fn.writefile({ vim.json.encode(serialized) }, bundle_path(name)) ~= 0 then
        vim.notify("Could not write bundle " .. name, vim.log.levels.ERROR, overseer_title)
        return
      end
      vim.notify(("Saved %d task(s) to bundle %s"):format(#serialized, name), vim.log.levels.INFO, overseer_title)
    end, { nargs = "?", complete = complete_bundle, desc = "Overseer: save the task list as a bundle" })

    vim.api.nvim_create_user_command("OverseerLoadBundle", function(args)
      local name = args.args ~= "" and args.args or default_bundle_name()
      local path = bundle_path(name)
      if vim.fn.filereadable(path) ~= 1 then
        vim.notify("No bundle named " .. name, vim.log.levels.ERROR, overseer_title)
        return
      end
      local ok, decoded = pcall(vim.json.decode, table.concat(vim.fn.readfile(path), "\n"))
      if not ok or type(decoded) ~= "table" then
        vim.notify("Bundle " .. name .. " is not readable JSON", vim.log.levels.ERROR, overseer_title)
        return
      end
      for _, params in ipairs(decoded) do
        -- Restores the saved definition rather than rebuilding it from its template.
        params.from_template = nil
        local task = overseer.new_task(params)
        if not args.bang then
          task:start()
        end
      end
      vim.notify(("Loaded %d task(s) from bundle %s"):format(#decoded, name), vim.log.levels.INFO, overseer_title)
    end, {
      nargs = "?",
      bang = true,
      complete = complete_bundle,
      desc = "Overseer: load a task bundle (with ! do not start them)",
    })

    vim.api.nvim_create_user_command("OverseerDeleteBundle", function(args)
      local name = args.args ~= "" and args.args or default_bundle_name()
      if vim.fn.delete(bundle_path(name)) ~= 0 then
        vim.notify("No bundle named " .. name, vim.log.levels.ERROR, overseer_title)
        return
      end
      vim.notify("Deleted bundle " .. name, vim.log.levels.INFO, overseer_title)
    end, { nargs = "?", complete = complete_bundle, desc = "Overseer: delete a task bundle" })

    vim.api.nvim_create_user_command("OverseerWatchRun", function()
      -- `autostart = false`, so the components attached below see `on_start`.
      overseer.run_task({ name = "run script", autostart = false }, function(task)
        if task then
          -- `watched` notifies only when the verdict changes and cancels a hung run.
          task:set_components({ "watched" })
          -- The split below opens the output, so this drops the template's own opener.
          task:remove_component("open_output")
          task:add_component({ "restart_on_save", paths = { vim.fn.expand("%:p") } })
          task:start()
          local main_win = vim.api.nvim_get_current_win()
          overseer.run_action(task, "open hsplit")
          vim.api.nvim_set_current_win(main_win)
        else
          vim.notify(
            "OverseerWatchRun not supported for filetype " .. vim.bo.filetype,
            vim.log.levels.ERROR,
            overseer_title
          )
        end
      end)
    end, { desc = overseer_watch_run_desc })
  end,
}
