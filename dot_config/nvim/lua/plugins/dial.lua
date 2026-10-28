-- dial.nvim owns <C-a>/<C-x>: numbers and dates as it ships them, plus the word cycles boole.nvim
-- used to toggle (dropped in the same commit). Loaded eagerly like the bare spec it replaces: no
-- startup trigger moves before PR 30a.
return {
  "monaqa/dial.nvim",
  lazy = false,
  config = function()
    local augend = require("dial.augend")

    -- Hexadecimal with an uppercase `0X` prefix, writing its digits back in the
    -- case they were already in. dial's integer augend fixes the output case, so
    -- an uppercase-prefix augend turned `0X1a` into `0X1B` where native <C-a>
    -- gives `0X1b`. Native follows the RIGHTMOST letter already in the number,
    -- and falls back to the prefix when the number has no letter at all. The two
    -- fixed-case augends do the arithmetic; this only decides which answers.
    -- ponytail: dial's shipped lowercase `0x` alias has the same forced case
    -- (`0x1A` steps to `0x1b`); left alone because that is upstream behavior this
    -- config has always had, not something the `0X` augend widened.
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

    -- boole's built-in loops, which also matched the Capitalized and UPPER forms of each word.
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
      -- and `0X` keeps whatever case its digits were already written in.
      augend.integer.alias.binary,
      augend.integer.new({ radix = 2, prefix = "0B", natural = true }),
      uppercase_prefix_hex(),
      augend.date.new({ pattern = "%Y/%m/%d", default_kind = "day" }),
      augend.date.new({ pattern = "%Y-%m-%d", default_kind = "day" }),
      augend.date.new({ pattern = "%m/%d", default_kind = "day", only_valid = true }),
      augend.date.new({ pattern = "%H:%M", default_kind = "day", only_valid = true }),
    }
    -- Each cased loop is registered three times, once per spelling, rather than once with
    -- `preserve_case`. That option matches case-INSENSITIVELY, which accepts spellings boole never
    -- did: on `tRuE 1` boole left the word alone and stepped the number to `2`, while dial flipped
    -- `tRuE` to `false` and left the number where it was. Three exact cycles restore boole's three
    -- spellings and no more.
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
