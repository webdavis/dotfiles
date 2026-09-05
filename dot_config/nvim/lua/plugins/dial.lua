-- dial.nvim owns <C-a>/<C-x>: numbers and dates as it ships them, plus the word cycles boole.nvim
-- used to toggle (dropped in the same commit). `preserve_case` reproduces boole's caps handling
-- (true -> false, True -> False, TRUE -> FALSE) for the loops boole allowed it on. Loaded eagerly
-- like the bare spec it replaces: no startup trigger moves before PR 30a.
return {
  "monaqa/dial.nvim",
  config = function()
    local augend = require("dial.augend")

    -- boole's built-in loops that also matched Capitalized and UPPER forms.
    local case_preserving = {
      { "true", "false" },
      { "yes", "no" },
      { "on", "off" },
      { "enable", "disable" },
      { "enabled", "disabled" },
      { "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday" },
      { "mon", "tue", "wed", "thu", "fri", "sat", "sun" },
      {
        "january",
        "february",
        "march",
        "april",
        "may",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
      },
    }

    -- The operator's own pairs from the boole spec, exact case only, in its order.
    local exact = {
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
      { "case", "esac" },
      { "readable", "unreadable" }, -- file permissions
      { "writable", "readonly" }, -- file permissions
      { "set", "unset" }, -- shell variables
    }

    -- dial's shipped default group minus its Japanese weekday loop: signed decimal (so -1 still
    -- steps to 0 the way Vim's own <C-a> does), hex, and its four date formats.
    local group = {
      augend.integer.alias.decimal_int,
      augend.integer.alias.hex,

      -- Vim's own `nrformats` defaults to "bin,hex", and dial's shipped group covers neither binary
      -- nor an uppercase prefix, so taking over <C-a> silently lost both: only the leading `0`
      -- matched, and `0b101` stepped to `1b101` where native gives `0b110`, `0X1F` to `1X1F` where
      -- native gives `0X20`. dial ships the lowercase `0b`; the two uppercase prefixes are ours,
      -- and `0X` renders its digits uppercase to match the prefix it was written with.
      augend.integer.alias.binary,
      augend.integer.new({ radix = 2, prefix = "0B", natural = true }),
      augend.integer.new({ radix = 16, prefix = "0X", natural = true, case = "upper" }),
      augend.date.new({ pattern = "%Y/%m/%d", default_kind = "day" }),
      augend.date.new({ pattern = "%Y-%m-%d", default_kind = "day" }),
      augend.date.new({ pattern = "%m/%d", default_kind = "day", only_valid = true }),
      augend.date.new({ pattern = "%H:%M", default_kind = "day", only_valid = true }),
    }
    for _, elements in ipairs(case_preserving) do
      table.insert(
        group,
        augend.constant.new({ elements = elements, word = true, cyclic = true, preserve_case = true })
      )
    end
    for _, elements in ipairs(exact) do
      table.insert(group, augend.constant.new({ elements = elements, word = true, cyclic = true }))
    end
    require("dial.config").augends:register_group({ default = group })

    local map = require("custom_api.keymap").map
    local dial = require("dial.map")
    map({
      mode = "n",
      lhs = "<C-a>",
      rhs = function()
        dial.manipulate("increment", "normal")
      end,
      desc = "Dial: increment",
    })
    map({
      mode = "n",
      lhs = "<C-x>",
      rhs = function()
        dial.manipulate("decrement", "normal")
      end,
      desc = "Dial: decrement",
    })
    map({
      mode = "v",
      lhs = "<C-a>",
      rhs = function()
        dial.manipulate("increment", "visual")
      end,
      desc = "Dial: increment",
    })
    map({
      mode = "v",
      lhs = "<C-x>",
      rhs = function()
        dial.manipulate("decrement", "visual")
      end,
      desc = "Dial: decrement",
    })
  end,
}
