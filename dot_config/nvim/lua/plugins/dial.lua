return {
  "monaqa/dial.nvim",
  lazy = false,
  config = function()
    local augend = require("dial.augend")

    -- Steps `0X` hex numbers and keeps the case of the rightmost letter, as native <C-a> does.
    -- A number with no letter keeps the case of its prefix.
    local function uppercase_prefix_hex()
      local upper = augend.integer.new({ radix = 16, prefix = "0X", natural = true, case = "upper" })
      local lower = augend.integer.new({ radix = 16, prefix = "0X", natural = true, case = "lower" })

      return augend.user.new({
        find = function(line, cursor)
          return upper:find(line, cursor)
        end,
        add = function(text, addend, cursor)
          local last_letter = text:sub(3):match(".*(%a)")
          local keep_upper = last_letter == nil or last_letter:upper() == last_letter

          return (keep_upper and upper or lower):add(text, addend, cursor)
        end,
      })
    end

    -- Each loop also cycles its Capitalized and UPPER forms.
    local cased = {
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

    -- These cycle only in the case written here.
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
      { "started", "stopped" },

      { "present", "absent" },
      { "changed", "unchanged" },

      { "let", "var" },
      { "public", "private" },
      { "internal", "fileprivate" },
      { "class", "struct" },

      { "case", "esac" },
      { "readable", "unreadable" },
      { "writable", "readonly" },
      { "set", "unset" },
    }

    -- Matches dial's default group without its Japanese weekdays.
    -- Decimals are signed, so -1 steps to 0 as native <C-a> does.
    local group = {
      augend.integer.alias.decimal_int,
      augend.integer.alias.hex,

      -- Adds binary numbers and uppercase prefixes, which native <C-a> steps and dial's default group skips.
      augend.integer.alias.binary,
      augend.integer.new({ radix = 2, prefix = "0B", natural = true }),
      uppercase_prefix_hex(),
      augend.date.new({ pattern = "%Y/%m/%d", default_kind = "day" }),
      augend.date.new({ pattern = "%Y-%m-%d", default_kind = "day" }),
      augend.date.new({ pattern = "%m/%d", default_kind = "day", only_valid = true }),
      augend.date.new({ pattern = "%H:%M", default_kind = "day", only_valid = true }),
    }
    -- One exact cycle per spelling, because `preserve_case` also matches mixed case such as `tRuE`.
    local spellings = {
      function(word)
        return word
      end,
      function(word)
        return word:sub(1, 1):upper() .. word:sub(2)
      end,
      string.upper,
    }
    for _, elements in ipairs(cased) do
      for _, spelling in ipairs(spellings) do
        table.insert(
          group,
          augend.constant.new({ elements = vim.tbl_map(spelling, elements), word = true, cyclic = true })
        )
      end
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
