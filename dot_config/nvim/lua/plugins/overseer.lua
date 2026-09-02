return {
  "stevearc/overseer.nvim",
  opts = {},
  config = function()
    local overseer = require("overseer")
    overseer.setup({
      -- Default task strategy
      output = {
        -- Use a terminal buffer to display output. If false, a normal buffer is used
        use_terminal = true,
        -- If true, don't clear the buffer when a task restarts
        preserve_output = false,
      },
      open_on_start = true,
      -- Template modules to load
      templates = { "builtin", "user.run_script" },
      -- When true, tries to detect a green color from your colorscheme to use for success highlight
      auto_detect_success_color = true,
      -- Patch nvim-dap to support preLaunchTask and postDebugTask
      dap = true,
      -- Configure the task list
      task_list = {
        -- Default direction. Can be "left", "right", or "bottom"
        direction = "right",
        -- Default detail level for tasks. Can be 1-3.
        default_detail = 1,
        -- Width dimensions can be integers or a float between 0 and 1 (e.g. 0.4 for 40%)
        -- min_width and max_width can be a single value or a list of mixed integer/float types.
        -- max_width = {100, 0.2} means "the lesser of 100 columns or 20% of total"
        max_width = { 100, 0.2 },
        -- min_width = {40, 0.1} means "the greater of 40 columns or 10% of total"
        min_width = { 40, 0.1 },
        -- optionally define an integer/float for the exact width of the task list
        width = nil,
        max_height = { 20, 0.1 },
        min_height = 8,
        height = nil,
        -- String that separates tasks
        separator = "────────────────────────────────────────",
        -- Indentation for child tasks
        child_indent = { "┃ ", "┣━", "┗━" },
        -- Function that renders tasks. See lua/overseer/render.lua for built-in options
        -- and for useful functions if you want to build your own.
        render = function(task)
          return require("overseer.render").format_standard(task)
        end,
        -- The sort function for tasks
        sort = function(a, b)
          return require("overseer.task_list").default_sort(a, b)
        end,
        -- Set keymap to false to remove default behavior
        -- You can add custom keymaps here as well (anything vim.keymap.set accepts)
        keymaps = {
          ["?"] = "keymap.show_help",
          ["g?"] = "keymap.show_help",
          ["<CR>"] = "keymap.run_action",
          ["<C-r>"] = { "keymap.run_action", opts = { action = "restart" }, desc = "Restart task" },
          ["<localleader>w"] = { "keymap.run_action", opts = { action = "watch" }, desc = "Watch task" },
          ["<localleader>W"] = { "keymap.run_action", opts = { action = "unwatch" }, desc = "Unwatch task" },
          ["dd"] = { "keymap.run_action", opts = { action = "dispose" }, desc = "Dispose task" },
          ["<C-e>"] = { "keymap.run_action", opts = { action = "edit" }, desc = "Edit task" },
          ["o"] = "keymap.open",
          ["<C-v>"] = { "keymap.open", opts = { dir = "vsplit" }, desc = "Open task output in vsplit" },
          ["<C-s>"] = { "keymap.open", opts = { dir = "split" }, desc = "Open task output in split" },
          ["<C-t>"] = { "keymap.open", opts = { dir = "tab" }, desc = "Open task output in tab" },
          ["<C-f>"] = { "keymap.open", opts = { dir = "float" }, desc = "Open task output in float" },
          ["<C-q>"] = {
            "keymap.run_action",
            opts = { action = "open output in quickfix" },
            desc = "Open task output in the quickfix",
          },
          ["p"] = "keymap.toggle_preview",
          [";"] = "keymap.toggle_preview",
          ["{"] = "keymap.prev_task",
          ["}"] = "keymap.next_task",
          ["<C-k>"] = "keymap.scroll_output_up",
          ["<C-j>"] = "keymap.scroll_output_down",
          ["g."] = "keymap.toggle_show_wrapped",
          ["q"] = { "<CMD>close<CR>", desc = "Close task list" },
        },
      },
      -- See :help overseer-actions
      actions = {},
      -- Configure the floating window used for task templates that require input
      -- and the floating window used for editing tasks
      form = {
        border = "rounded",
        zindex = 40,
        -- Dimensions can be integers or a float between 0 and 1 (e.g. 0.4 for 40%)
        -- min_X and max_X can be a single value or a list of mixed integer/float types.
        min_width = 80,
        max_width = 0.9,
        width = nil,
        min_height = 10,
        max_height = 0.9,
        height = nil,
        -- Set any window options here (e.g. winhighlight)
        win_opts = {
          winblend = 10,
        },
      },
      task_launcher = {
        -- Set keymap to false to remove default behavior
        -- You can add custom keymaps here as well (anything vim.keymap.set accepts)
        bindings = {
          i = {
            ["<C-s>"] = "Submit",
            ["<C-c>"] = "Cancel",
          },
          n = {
            ["<CR>"] = "Submit",
            ["<C-s>"] = "Submit",
            ["q"] = "Cancel",
            ["?"] = "ShowHelp",
          },
        },
      },
      task_editor = {
        -- Set keymap to false to remove default behavior
        -- You can add custom keymaps here as well (anything vim.keymap.set accepts)
        bindings = {
          i = {
            ["<CR>"] = "NextOrSubmit",
            ["<C-s>"] = "Submit",
            ["<Tab>"] = "Next",
            ["<S-Tab>"] = "Prev",
            ["<C-c>"] = "Cancel",
          },
          n = {
            ["<CR>"] = "NextOrSubmit",
            ["<C-s>"] = "Submit",
            ["<Tab>"] = "Next",
            ["<S-Tab>"] = "Prev",
            ["q"] = "Cancel",
            ["?"] = "ShowHelp",
          },
        },
      },
      -- Configure the floating window used for confirmation prompts
      confirm = {
        border = "rounded",
        zindex = 40,
        -- Dimensions can be integers or a float between 0 and 1 (e.g. 0.4 for 40%)
        -- min_X and max_X can be a single value or a list of mixed integer/float types.
        min_width = 20,
        max_width = 0.5,
        width = nil,
        min_height = 6,
        max_height = 0.9,
        height = nil,
        -- Set any window options here (e.g. winhighlight)
        win_opts = {
          winblend = 10,
        },
      },
      -- Configuration for task floating windows
      task_win = {
        -- How much space to leave around the floating window
        padding = 2,
        border = "rounded",
        -- Set any window options here (e.g. winhighlight)
        win_opts = {
          winblend = 10,
        },
      },
      -- Configuration for mapping help floating windows
      help_win = {
        border = "rounded",
        win_opts = {},
      },
      -- Aliases for bundles of components. Redefine the builtins, or create your own.
      component_aliases = {
        -- Most tasks are initialized with the default components
        default = {
          "on_exit_set_status",
          "on_complete_notify",
          "on_output_quickfix",
          -- { "on_complete_dispose", require_view = { "SUCCESS", "FAILURE" } },
        },
        -- Tasks from tasks.json use these components
        default_vscode = {
          "default",
          "on_result_diagnostics",
        },
        -- Tasks created from experimental_wrap_builtins
        default_builtin = {
          "on_exit_set_status",
          "on_complete_dispose",
          { "unique", soft = true },
        },
      },
      bundles = {
        -- When saving a bundle with OverseerSaveBundle or save_task_bundle(), filter the tasks with
        -- these options (passed to list_tasks())
        save_task_opts = {
          bundleable = true,
        },
        -- Autostart tasks when they are loaded from a bundle
        autostart_on_load = true,
      },
      -- A list of components to preload on setup.
      -- Only matters if you want them to show up in the task editor.
      preload_components = {},
      -- Controls when the parameter prompt is shown when running a template
      --   always    Show when template has any params
      --   missing   Show when template has any params not explicitly passed in
      --   allow     Only show when a required param is missing
      --   avoid     Only show when a required param with no default value is missing
      --   never     Never show prompt (error if required param missing)
      default_template_prompt = "allow",
      -- For template providers, how long to wait (in ms) before timing out.
      -- Set to 0 to disable timeouts.
      template_timeout = 3000,
      -- Cache template provider results if the provider takes longer than this to run.
      -- Time is in ms. Set to 0 to disable caching.
      template_cache_threshold = 100,
      -- Configure where the logs go and what level to use
      -- Types are "echo", "notify", and "file"
      log = {
        {
          type = "echo",
          level = vim.log.levels.WARN,
        },
        {
          type = "file",
          filename = "overseer.log",
          level = vim.log.levels.WARN,
        },
      },
    })

    local overseer_title = { title = "Overseer" }

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

    local overseer_watch_run_desc = "Overseer: watch-run"
    vim.api.nvim_create_user_command("OverseerWatchRun", function()
      overseer.run_template({ name = "run script" }, function(task)
        if task then
          task:add_component({ "restart_on_save", paths = { vim.fn.expand("%:p") } })
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

    local function toggle_runner(window)
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

    map({
      mode = "n",
      lhs = "<leader>or",
      rhs = function()
        vim.cmd("OverseerOpen!")
        vim.cmd("OverseerRun")
      end,
      remap = false,
      silent = true,
      desc = "Overseer: run (and open list)",
    })

    map({
      mode = "n",
      lhs = "<leader>oR",
      rhs = function()
        vim.cmd("OverseerRun")
      end,
      remap = false,
      silent = true,
      desc = "Overseer: run",
    })

    map({
      mode = "n",
      lhs = "<leader>ol",
      rhs = "OverseerRestartLast",
      remap = false,
      silent = true,
      desc = "Overseer: run last task",
    })

    map({
      mode = "n",
      lhs = "<leader>oo",
      rhs = function()
        vim.cmd("OverseerOpen")
      end,
      remap = false,
      silent = true,
      desc = "Overseer: open (and focus)",
    })

    map({
      mode = "n",
      lhs = "<leader>oO",
      rhs = function()
        vim.cmd("OverseerOpen!")
      end,
      remap = false,
      silent = true,
      desc = "Overseer: open (without focus)",
    })

    map({
      mode = "n",
      lhs = "<leader>oc",
      rhs = function()
        vim.cmd("OverseerClose")
      end,
      remap = false,
      silent = true,
      desc = "Overseer: close",
    })

    map({
      mode = "n",
      lhs = "<leader>ot",
      rhs = function()
        vim.cmd("OverseerToggle")
      end,
      remap = false,
      silent = true,
      desc = "Overseer: toggle (and focus)",
    })

    map({
      mode = "n",
      lhs = "<leader>oT",
      rhs = function()
        vim.cmd("OverseerToggle!")
      end,
      remap = false,
      silent = true,
      desc = "Overseer: toggle (without focus)",
    })

    map({
      mode = "n",
      lhs = { '<leader>o"', "<M-7>" },
      rhs = function()
        toggle_runner("hsplit")
      end,
      desc = "Overseer: open task in hsplit",
    })

    map({
      mode = "n",
      lhs = { "<leader>o%", "<M-8>" },
      rhs = function()
        toggle_runner("vsplit")
      end,
      desc = "Overseer: open task in vsplit",
    })

    map({
      mode = "n",
      lhs = "<M-;>",
      rhs = function()
        toggle_runner("float")
      end,
      desc = "Overseer: open task in floating window",
    })

    map({
      mode = "n",
      lhs = "<M-[>",
      rhs = function()
        toggle_runner("OverseerWatchRun")
      end,
      desc = overseer_watch_run_desc,
    })
  end,
}
