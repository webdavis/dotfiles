-- Delegate window integration.
local M = {}

-- Check if running inside tmux.
local function in_tmux()
  return vim.env.TMUX ~= nil
end

-- Find or check if delegate window exists.
local function find_delegate_window()
  if not in_tmux() then
    return false
  end

  local result = vim.fn.system("tmux list-windows -F '#{window_name}'")
  for window in result:gmatch("[^\r\n]+") do
    if window == "delegate" then
      return true
    end
  end
  return false
end

-- Toggle delegate window.
local function toggle_delegate()
  if not in_tmux() then
    vim.notify("Not running inside tmux", vim.log.levels.ERROR)
    return
  end

  local window_exists = find_delegate_window()

  if window_exists then
    -- Window exists, switch to it.
    vim.fn.system("tmux select-window -t delegate")
  else
    -- Create new window named "delegate" with `command`.
    vim.fn.system("tmux new-window -n delegate")
  end
end

-- Send text to delegate window.
local function send_to_delegate(text, focus)
  focus = focus == nil and true or focus -- Default to true.

  if not in_tmux() then
    vim.notify("Not running inside tmux", vim.log.levels.ERROR)
    return
  end

  local window_exists = find_delegate_window()

  if not window_exists then
    -- Create window first.
    vim.fn.system("tmux new-window -n delegate")
    -- Wait for window to initialize, then send text.
    vim.defer_fn(function()
      vim.fn.system(string.format("tmux send-keys -t delegate %s Enter", vim.fn.shellescape(text)))
      if focus then
        vim.fn.system("tmux select-window -t delegate")
      end
    end, 1500)
    return
  end

  -- Send text to existing window.
  vim.fn.system(string.format("tmux send-keys -t delegate %s Enter", vim.fn.shellescape(text)))

  -- Focus the window.
  if focus then
    vim.fn.system("tmux select-window -t delegate")
  end
end

-- Send visual selection to delegate with prompt.
local function send_selection_to_delegate(opts)
  opts = opts or {}
  local prompt_separator = opts.prompt_separator or ":\n\n"

  local filepath = vim.fn.expand("%:p")
  if not filepath or filepath == "" then
    vim.notify("No file available for selection reference", vim.log.levels.WARN)
    return
  end

  -- Get visual selection bounds.
  local mode = vim.fn.mode()
  local start_line, end_line

  if mode == "v" or mode == "V" or mode == "\22" then
    local start_pos = vim.fn.getpos("v")
    local end_pos = vim.fn.getpos(".")
    start_line = start_pos[2]
    end_line = end_pos[2]

    if start_line > end_line then
      start_line, end_line = end_line, start_line
    end

    vim.cmd("normal! \\<Esc>")
  else
    local start_pos = vim.fn.getpos("'<")
    local end_pos = vim.fn.getpos("'>")

    if start_pos[2] == 0 or end_pos[2] == 0 then
      vim.notify("No text currently selected", vim.log.levels.WARN)
      return
    end

    start_line = start_pos[2]
    end_line = end_pos[2]
  end

  local line_reference
  if start_line == end_line then
    line_reference = string.format("%s:%d", filepath, start_line)
  else
    line_reference = string.format("%s:%d-%d", filepath, start_line, end_line)
  end

  vim.ui.input({
    prompt = "Delegate prompt (+ selection): ",
    default = "",
  }, function(input)
    if not input or not input:match("%S") then
      return
    end

    local final_text = ("%s%s%s"):format(input, prompt_separator, line_reference)
    send_to_delegate(final_text)
  end)
end

-- Quick prompt to delegate (normal mode).
local function quick_prompt_to_delegate()
  vim.ui.input({
    prompt = "Delegate: ",
    default = "",
  }, function(input)
    if not input or not input:match("%S") then
      return
    end
    send_to_delegate(input)
  end)
end

-- Export functions:
M.toggle = toggle_delegate
M.send_selection = send_selection_to_delegate
M.quick_prompt = quick_prompt_to_delegate

-- Setup keymaps.
function M.setup(opts)
  opts = opts or {}

  -- Default keymaps:
  local keymaps = opts.keymaps
    or {
      { "n", "<leader>dt", M.toggle, "Delegate: toggle" },
      { "n", "<leader>dp", M.quick_prompt, "Delegate: prompt" },
      { "v", "<leader>ds", M.send_selection, "Delegate: send selection" },
    }

  for _, keymap in ipairs(keymaps) do
    local mode, key, func, desc = keymap[1], keymap[2], keymap[3], keymap[4]
    vim.keymap.set(mode, key, func, { desc = desc, silent = true })
  end
end

-- Auto-setup with defaults.
M.setup()

return M
