-- The message history in a snacks picker, for ui2: every message `:messages`
-- keeps, with the time it arrived, its level and its kind, laid out like the
-- snacks notifications picker.
--
-- `:messages` only gives text, so `record()` wraps ui2's `msg_show`, which ui2
-- looks up on every message (`$VIMRUNTIME/lua/vim/_core/ui2.lua`, `ui_callback`).
-- ui2 is private: when that handler is missing, nothing is wrapped and the
-- picker falls back to the plain `:messages` text.
local M = {}

local entries = {} ---@type { added: integer, kind: string, msg: string, level: string }[]

local LEVELS = {
  emsg = "error",
  echoerr = "error",
  lua_error = "error",
  rpc_error = "error",
  wmsg = "warn",
}

function M.record()
  local ok, messages = pcall(require, "vim._core.ui2.messages")
  if not ok or type(messages.msg_show) ~= "function" then
    return
  end
  local msg_show = messages.msg_show
  local function add(kind, content, history)
    if history then
      local text = {}
      for _, chunk in ipairs(content) do
        text[#text + 1] = chunk[2]
      end
      entries[#entries + 1] = {
        added = os.time(),
        kind = kind ~= "" and kind or "echo",
        msg = table.concat(text),
        level = LEVELS[kind] or "info",
      }
    end
  end
  messages.msg_show = function(kind, content, replace_last, history, ...)
    -- A ui2 that changed the arguments' shape loses a history row, never the message.
    pcall(add, kind, content, history)
    return msg_show(kind, content, replace_last, history, ...)
  end
end

function M.pick()
  local list = entries
  if #list == 0 then
    list = {}
    for _, line in ipairs(vim.split(vim.api.nvim_exec2("messages", { output = true }).output, "\n")) do
      if line ~= "" then
        list[#list + 1] = { kind = "", msg = line, level = "info" }
      end
    end
  end

  local items = {}
  for i = #list, 1, -1 do
    local entry = list[i]
    items[#items + 1] = {
      text = ("%s %s %s"):format(entry.level, entry.kind, entry.msg),
      item = { added = entry.added, title = entry.kind, msg = entry.msg },
      severity = entry.level,
      preview = { text = entry.msg, ft = "text" },
    }
  end

  Snacks.picker({
    title = "Messages",
    items = items,
    format = "notification",
    preview = "preview",
    formatters = { severity = { level = true } },
    confirm = "close",
  })
end

return M
