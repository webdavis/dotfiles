return {
  "stevearc/overseer.nvim",
  opts = {},
  config = function()
    local overseer = require("overseer")
    -- Only what differs from the plugin's own defaults. Copying the upstream
    -- default table wholesale is how twelve keys the plugin no longer has
    -- survived here as fields nothing read (7d8e81ff); a short table cannot rot
    -- the same way, because every line in it is a decision.
    overseer.setup({
      -- nvim-dap is present (xcodebuild.nvim depends on it), so this patches in
      -- real preLaunchTask and postDebugTask support rather than a theoretical one.
      dap = true,
      task_list = {
        direction = "right",
        max_height = { 20, 0.1 },
        -- Light box-drawing rather than the heavy default.
        separator = "────────────────────────────────────────",
        -- Merged over the defaults case-insensitively, so only the additions
        -- belong here. All four run actions that exist at the pinned commit.
        keymaps = {
          ["<C-r>"] = { "keymap.run_action", opts = { action = "restart" }, desc = "Restart task" },
          ["<localleader>w"] = { "keymap.run_action", opts = { action = "watch" }, desc = "Watch task" },
          ["<localleader>W"] = { "keymap.run_action", opts = { action = "unwatch" }, desc = "Unwatch task" },
          [";"] = "keymap.toggle_preview",
        },
      },
      form = {
        border = "rounded",
        win_opts = {
          winblend = 10,
        },
      },
      task_win = {
        border = "rounded",
        win_opts = {
          winblend = 10,
        },
      },
      component_aliases = {
        -- on_complete_dispose is deliberately absent, and is the one place this
        -- alias must stay spelled out: OverseerRestartLast and the
        -- <M-7>/<M-8>/<M-;> openers all look up a FINISHED task, so tasks have
        -- to outlive the upstream five-minute dispose timeout.
        --
        -- The pns on_complete reporter (plan task 26) belongs in this list.
        default = {
          "on_exit_set_status",
          "on_complete_notify",
          {
            "on_output_quickfix",
            -- Without this every task replaces the quickfix with its entire
            -- output as unnavigable text, clobbering whatever was there. Keep
            -- only the lines that parse as a location.
            items_only = true,
            -- Neovim's default errorformat already parses luacheck and every
            -- other `file:line:col: message` tool this repo runs. It does not
            -- parse this repo's own Neovim test runner, which prints
            -- `FAIL <spec>: <case>: <file>:<line>: <message>`: `%f` swallows the
            -- whole prefix into the filename, so the entry jumps nowhere. One
            -- leading pattern fixes that; the rest is the stock list.
            errorformat = "FAIL %.%#: %f:%l: %m," .. vim.o.errorformat,
            set_diagnostics = true,
          },
          -- Paired with set_diagnostics above. Fires only when the errorformat
          -- matched something, so a clean run leaves no diagnostics behind.
          { "on_result_diagnostics", remove_on_restart = true },
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
      overseer.run_task({ name = "run script" }, function(task)
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
        vim.cmd("OverseerWatchRun")
      end,
      desc = overseer_watch_run_desc,
    })
  end,
}
