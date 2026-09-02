return {
  "nat-418/boole.nvim",
  opts = {
    mappings = {
      increment = "<C-a>",
      decrement = "<C-x>",
    },
    -- User defined loops
    -- active
    additions = {
      { "foo", "bar" },
      { "increment", "decrement" },
      { "allow", "deny" },
      { "show", "hide" },
      { "open", "closed" },
      { "start", "end" },
      { "up", "down" },
      { "left", "right" },
      { "high", "low" },
      { "active", "inactive" },
      { "null", "nil" },
      { "add", "remove" },
      { "push", "pop" },
      { "lock", "unlock" },
      { "mount", "unmount" },
      { "connect", "disconnect" },
      { "light", "dark" },
      { "visible", "hidden" },
      { "full", "empty" },
      { "expanded", "collapsed" },
      { "checked", "unchecked" },
      { "started", "stopped" }, -- for services

      -- Ansible:
      { "present", "absent" }, -- for package or file states
      { "changed", "unchanged" }, -- for checking task results

      -- Swift lang:
      { "let", "var" }, -- immutability toggle
      { "public", "private" }, -- access control
      { "internal", "fileprivate" }, -- access control
      { "class", "struct" }, -- type toggle

      -- Bash:
      { "$HOME", "~" },
      { "case", "esac" },
      { "readable", "unreadable" }, -- file permissions
      { "writable", "readonly" }, -- file permissions
      { "set", "unset" }, -- shell variables
    },
    allow_caps_additions = {
      { "true", "false" },
      { "enable", "disable" },
      -- For example:
      -- enable → disable
      -- Enable → Disable
      -- ENABLE → DISABLE
    },
  },
}
