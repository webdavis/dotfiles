local overseer_title = { title = "Overseer" }
local overseer_watch_run_desc = "Overseer: watch-run"

local function toggle_runner(window)
  local overseer = require("overseer")
  if vim.bo.buftype == "terminal" then
    vim.cmd("close")
    return
  end

  -- Check for Overseer window.
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

-- A window that always shows the newest task's output, rather than one task's
-- output frozen at the moment it was opened. Overseer builds it from any window;
-- this opens a split and hands that window over, and the view then follows every
-- task started afterwards.
local function output_view()
  local overseer = require("overseer")
  vim.cmd("botright split")
  overseer.create_task_output_view(vim.api.nvim_get_current_win(), {
    list_task_opts = {
      filter = function(task)
        return task.time_start ~= nil
      end,
    },
    -- The newest task, always. Overseer keeps whichever task the cursor was last
    -- over in the sidebar, and preferring that pinned the view to it: hover an
    -- older task, close the sidebar, and every task started afterwards was
    -- ignored while the view sat on the old one.
    select = function(_, tasks)
      -- By start sequence, not `time_start`: that is `os.time()`, so two starts
      -- in one second compare equal and the earlier task stayed displayed.
      -- time_start is only the tiebreak for a task carrying no sequence, which
      -- means it was started with components of its own.
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

-- The other half of the template preload in `config`. A provider's cache_key
-- invalidates on its own file, so this is for the cases that miss: a recipe added
-- to a justfile further up the tree, or a template edited in this config.
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
  opts = {},
  -- Every command overseer registers itself (overseer/init.lua `commands`) plus
  -- the five this file's `config` creates. A command defined only inside
  -- `config` cannot trigger the load unless its name is here, so it would simply
  -- not exist until something else loaded the plugin.
  cmd = {
    "OverseerClose",
    "OverseerDeleteBundle",
    "OverseerLoadBundle",
    "OverseerOpen",
    "OverseerRestartLast",
    "OverseerRun",
    "OverseerSaveBundle",
    "OverseerShell",
    "OverseerTaskAction",
    "OverseerToggle",
    "OverseerWatchRun",
  },
  -- Each row IS the mapping: lazy sets a placeholder at startup and installs this
  -- same rhs when the plugin loads, so there is no second copy in `config`. The
  -- four `:`-prefixed rows leave the cmdline open for an argument, so they carry
  -- no `silent`.
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
    { "<M-[>", "<cmd>OverseerWatchRun<cr>", desc = overseer_watch_run_desc },
  },
  config = function()
    local overseer = require("overseer")

    -- Neovim's default errorformat already parses luacheck and every other
    -- `file:line:col: message` tool this repo runs. It does not parse this repo's
    -- own Neovim test runner, which prints
    -- `FAIL <spec>: <case>: <file>:<line>: <message>`: `%f` swallows the whole
    -- prefix into the filename, so the entry jumps nowhere.
    --
    -- The two skipped fields are `%*[^:]`, not `%.%#`. A greedy `.*` runs to the
    -- LAST `file:line` on the line, so an assertion message that mentions another
    -- location sent the entry there instead of to the failure. Bounding both
    -- fields to colon-free runs stops the prefix at the assertion's own location.
    -- The cost is that a case name containing a colon does not match and falls
    -- back to the stock format; the runner's own names avoid one for that reason.
    local quickfix_errorformat = "FAIL %*[^:]: %*[^:]: %f:%l: %m," .. vim.o.errorformat

    -- Two actions overseer does not ship.
    --
    -- `dispose all finished` is the manual sweep that stands in for the
    -- `on_complete_dispose` component this config deliberately omits: tasks have
    -- to outlive the upstream timeout, so something has to clear them by hand.
    --
    -- `copy command` answers "what did that actually run", which matters because
    -- a template builds the argv rather than the operator typing it.
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

    -- Every option overseer exposes at the pinned commit is set here, on purpose:
    -- the operator asked for the plugin fully featured. Where an option has a
    -- preference rather than an obvious answer, the comment beside it says which
    -- way it went and why. Nothing is left at a default silently.
    overseer.setup({
      -- nvim-dap is present (xcodebuild.nvim depends on it), so preLaunchTask and
      -- postDebugTask in a debug configuration run as real tasks.
      dap = true,
      -- WARN keeps template-provider errors visible without narrating every task.
      log_level = vim.log.levels.WARN,
      output = {
        -- A terminal buffer, so a task that draws progress bars or colors renders.
        use_terminal = true,
        -- Clear on restart: a watch task's output should show this run, not a pile.
        preserve_output = false,
      },
      task_list = {
        direction = "right",
        max_width = { 100, 0.2 },
        min_width = { 40, 0.1 },
        max_height = { 20, 0.1 },
        min_height = 8,
        -- Light box-drawing rather than the heavy default.
        separator = "────────────────────────────────────────",
        child_indent = { "┃ ", "┣━", "┗━" },
        -- format_standard over format_compact and format_verbose: it is what this
        -- config has actually been rendering, so the familiar view is kept.
        render = function(task)
          return require("overseer.render").format_standard(task)
        end,
        sort = function(a, b)
          return require("overseer.task_list").default_sort(a, b)
        end,
        -- Merged over the defaults case-insensitively, so only additions belong
        -- here. Every action with no key of its own gets one, which is what makes
        -- the list a full surface rather than a viewer.
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
      -- Two actions this config needs that overseer does not ship. See below.
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
        -- on_complete_dispose is deliberately absent, and is the one place this
        -- alias must stay spelled out: OverseerRestartLast, the
        -- <M-7>/<M-8>/<M-;> openers and the bundle commands all look up a
        -- FINISHED task, so tasks have to outlive the upstream five-minute
        -- dispose timeout. The "dispose all finished" action is the manual sweep
        -- that replaces it.
        --
        -- The pns on_complete reporter (plan task 26) belongs in this list.
        default = {
          "on_exit_set_status",
          { "on_complete_notify", system = "unfocused" },
          {
            "on_output_quickfix",
            -- Without this every task replaces the quickfix with its entire
            -- output as unnavigable text, clobbering whatever was there. Keep
            -- only the lines that parse as a location.
            items_only = true,
            set_diagnostics = true,
            open_on_match = true,
          },
          -- Paired with set_diagnostics above. Fires only when the errorformat
          -- matched something, so a clean run leaves no diagnostics behind.
          { "on_result_diagnostics", remove_on_restart = true },
          -- Orders the live output view; see the component for why
          -- time_start cannot.
          "user.start_sequence",
          -- The live equivalent of the dead `open_on_start = true` this config
          -- used to pass. `on_start` has to be "always": its own default,
          -- "if_no_on_output_quickfix", means it would never fire here, because
          -- every task in this alias carries on_output_quickfix. Docked rather
          -- "horizontal" rather than the component's own "dock" default:
          -- docking binds the window to the task list, and with the list closed,
          -- which is the normal state here, starting a task never returned. A
          -- split opens in 1.2 s, measured, and `focus = false` keeps the cursor
          -- in the buffer being worked in.
          { "open_output", on_start = "always", direction = "horizontal", focus = false },
        },
        -- Tasks read out of a .vscode/tasks.json.
        default_vscode = {
          "default",
          "on_result_diagnostics",
          -- A VS Code task's diagnostics come from its problem matcher rather
          -- than this config's errorformat, so on_output_quickfix in `default`
          -- never sees them. This is the route that does.
          { "on_result_diagnostics_quickfix", open = true },
        },
        -- Tasks the vim.system and jobstart wrappers create. These are other
        -- plugins' background jobs, not the operator's, so they clean up after
        -- themselves and never stack.
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
        -- trouble.nvim is installed, so a task can route its diagnostics there
        -- instead of the quickfix. Opt in per task with the task editor.
        trouble = {
          "default",
          { "on_result_diagnostics_trouble", close = true },
        },
        -- A watch task re-runs on every write, so it notifies only when the
        -- verdict CHANGES and cancels itself if a run wedges.
        watched = {
          "on_exit_set_status",
          { "on_complete_notify", on_change = true, system = "unfocused" },
          { "on_output_quickfix", items_only = true, set_diagnostics = true },
          { "on_result_diagnostics", remove_on_restart = true },
          -- The component written for exactly this: a long-running task that
          -- produces new results periodically, notifying on the verdict rather
          -- than on every completion.
          { "on_result_notify", on_change = true, system = "unfocused" },
          { "timeout", timeout = 600 },
          "user.start_sequence",
        },
        -- A flaky task worth another go on its own. Not in `default`, where
        -- restarting on every failure is a loop rather than a retry.
        retry = {
          "default",
          { "on_complete_restart", statuses = { "FAILURE" }, delay = 2000 },
        },
        -- nvim-notify is installed, so a long task can show a live output
        -- summary in the notification instead of only its final status.
        live_notify = {
          "on_exit_set_status",
          { "on_output_notify", output_on_complete = true, max_lines = 3 },
          { "on_output_quickfix", items_only = true, set_diagnostics = true },
          { "on_result_diagnostics", remove_on_restart = true },
          "user.start_sequence",
        },
      },
      -- Empty on purpose. Our own templates already live under the default
      -- `lua/overseer/template/**` glob, and this key is runtimepath-relative, so
      -- it cannot reach a project directory. Project-local tasks come from the
      -- `.overseer/` provider and from `.vscode/tasks.json` instead.
      template_dirs = {},
      -- Empty on purpose: this key only ever REMOVES providers, and every one of
      -- the fourteen builtins is wanted. Measured in this repo, resolving all of
      -- them costs 150 ms cold and 19 ms warm, so there is nothing to buy back.
      disable_template_modules = {},
      template_timeout_ms = 3000,
      template_cache_threshold_ms = 200,
      -- Every vim.system and jobstart call becomes a task, so a plugin's
      -- background job is inspectable instead of invisible. Safe to leave on:
      -- list_tasks excludes wrapped tasks unless asked for them, so they stay out
      -- of the task list until `g.` reveals them.
      experimental_wrap_builtins = {
        enabled = true,
        condition = function()
          return true
        end,
      },
    })

    -- The generic errorformat is a task DEFAULT, not a component parameter.
    -- `on_output_quickfix.errorformat` carries `default_from_task`, which fills
    -- in only when the component does not set it, so pinning it in the alias
    -- overrode every template that ships one of its own: a Cargo error resolved
    -- to the nonexistent `--> src/lib.rs` buffer instead of to `src/lib.rs:2:3`.
    -- Setting it here leaves a template's own format in place and supplies ours
    -- to the templates with none.
    overseer.add_template_hook(nil, function(task_defn)
      -- Overseer expands aliases before calling a hook, so `components` IS the
      -- shared alias table. Component initialization fills `default_from_task`
      -- into those params IN PLACE, so the first task built wrote its resolved
      -- errorformat into the table every later task shares: an ordinary task
      -- first made Cargo inherit the generic format despite shipping its own,
      -- and Cargo first contaminated ordinary tasks. Each task gets its own copy.
      task_defn.components = vim.deepcopy(task_defn.components)

      local defaults = task_defn.default_component_params or {}
      if defaults.errorformat == nil then
        defaults.errorformat = quickfix_errorformat
        task_defn.default_component_params = defaults
      end
    end)

    -- Re-running a gate should replace the last run of it, not stack a second
    -- copy beside it. `unique` does that, and a template hook is how a component
    -- reaches templates this config does not own. Scoped to the `just` provider
    -- because that is where the repeat-a-gate habit lives; ad-hoc shell commands
    -- and VS Code tasks are left alone, since two of those side by side is often
    -- the point.
    overseer.add_template_hook({ module = "^just$" }, function(task_defn, util)
      -- `unique` compares task NAMES, and every worktree of this repository
      -- offers the same recipes, so `just test` started in one stopped and
      -- disposed the run in another. Putting the directory in the name is what
      -- makes that comparison see two different tasks.
      -- The pinned `just` builder returns `cmd` and `cwd` and NO name, so
      -- interpolating `task_defn.name` gave every recipe the same `nil (<cwd>)`
      -- and `unique` disposed whichever was already running. The command is what
      -- distinguishes them when the builder names nothing.
      if task_defn.cwd then
        -- json over a space join: joining argv threw the argument boundaries
        -- away, so a two-parameter recipe called with {"a b", "c"} and with
        -- {"a", "b c"} produced one name and `unique` disposed the running one.
        local name = task_defn.name
          or (type(task_defn.cmd) == "table" and vim.json.encode(task_defn.cmd))
          or tostring(task_defn.cmd)
        task_defn.name = ("%s (%s)"):format(name, vim.fn.fnamemodify(task_defn.cwd, ":~"))
      end
      util.add_component(task_defn, { "unique", replace = true })
    end)

    -- Every template provider runs on the first :OverseerRun in a directory, and
    -- the `just` one shells out to `just --dump` for each justfile above the
    -- cwd. Measured in this repo that is 150 ms cold against 19 ms warm, so this
    -- moves the wait off the first keystroke. DirChanged as well as VimEnter,
    -- because the templates a directory offers are the thing that changed.
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

    -- Task bundles.
    --
    -- Overseer shipped OverseerSaveBundle and OverseerLoadBundle until they were
    -- removed; `bundles` is not a config key at the pinned commit and `bundle`
    -- appears nowhere in the plugin's source. What survived is the pair the
    -- feature was built on, `Task:serialize()` and `new_task()`, which is exactly
    -- what the plugin's own resession extension uses. These three commands are
    -- that same pair with a file behind it, so the capability is back without
    -- taking on a session manager to get it.
    --
    -- Loading APPENDS; it does not replace the task list. Loading the same
    -- bundle three times leaves three copies of each task. An ordinary load
    -- starts them, so any task carrying a persisted `unique` component replaces
    -- its previous instance as it starts and the list settles; a bang load
    -- starts nothing, so its duplicates simply sit there pending. Clearing
    -- first is the "dispose all finished" action, deliberately a separate
    -- keystroke rather than something a load does on the operator's behalf.
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

    -- A bundle with no name is this project's bundle, which is the common case.
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
        -- `Task:serialize()` drops `default_component_params`, and the built-in
        -- "open output in quickfix" action reads those task defaults directly
        -- rather than the component's resolved copy. Without them a restored
        -- task's action fell back to the stock errorformat and targeted
        -- `FAIL util_spec: assertion: actual.lua:42` instead of `actual.lua`.
        defn.default_component_params = task.default_component_params
        return defn
      end, tasks)
      vim.fn.mkdir(bundle_dir, "p")
      -- writefile over io.open: it writes the file in one call and reports failure
      -- as a non-zero return rather than leaving a half-written bundle behind.
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
        -- `Task:serialize()` keeps `from_template`, and `new_task` treats that
        -- as "rebuild me from the template", which threw away everything the
        -- bundle had saved: an edited command came back as the template's, with
        -- the environment and any hand-added component gone, while the restore
        -- still reported success. The saved definition IS the thing to restore.
        params.from_template = nil
        local task = overseer.new_task(params)
        -- Bang means restore the tasks without running them.
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
      -- `autostart = false` is load-bearing. `run_task` starts the task BEFORE
      -- this callback runs, so components attached here miss their `on_start`:
      -- the timeout never armed its timer on the first run, and only a restart
      -- gave the watch the cancellation it promises. Attach first, then start.
      overseer.run_task({ name = "run script", autostart = false }, function(task)
        if task then
          -- A watch task re-runs on every write, so it wants the `watched` set
          -- rather than the everyday one: notify only when the verdict CHANGES,
          -- and cancel a run that wedges instead of leaving it to sit. This
          -- upserts by component name, so the template's own components stay and
          -- only the ones `watched` names are re-parameterised.
          task:set_components({ "watched" })
          -- The template's `default` brings `open_output` with it, which would
          -- open the output on start and leave the explicit split below opening
          -- a second window on the same task.
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
