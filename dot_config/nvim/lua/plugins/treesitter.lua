-- Credit: https://github.com/ThorstenRhau/neovim/blob/main/lua/optional/treesitter.lua
return {
  "nvim-treesitter/nvim-treesitter",
  dependencies = {
    {
      "WhoIsSethDaniel/mason-tool-installer.nvim",
    },
    {
      "nvim-treesitter/nvim-treesitter-context",
      opts = {
        max_lines = 4,
        multiline_threshold = 2,
      },
      config = function(_, opts)
        local treesitter_context = require("treesitter-context")
        treesitter_context.setup(opts)

        map({
          mode = "n",
          lhs = "[r",
          rhs = function()
            treesitter_context.go_to_context(vim.v.count1)
          end,
          silent = true,
          desc = "nvim-treesitter-context: jump up to context",
        })
      end,
    },
    {
      "nvim-treesitter/nvim-treesitter-textobjects",
      branch = "main",
      init = function()
        -- Disable entire built-in ftplugin mappings to avoid conflicts.
        -- See https://github.com/neovim/neovim/tree/master/runtime/ftplugin for built-in ftplugins.
        vim.g.no_plugin_maps = true

        -- Or, disable per filetype (add as you like)
        -- vim.g.no_python_maps = true
        -- vim.g.no_ruby_maps = true
        -- vim.g.no_rust_maps = true
        -- vim.g.no_go_maps = true
      end,
      config = function()
        -- configuration
        require("nvim-treesitter-textobjects").setup({
          select = {
            -- Automatically jump forward to textobj, similar to targets.vim
            lookahead = true,
            -- You can choose the select mode (default is charwise 'v')
            --
            -- Can also be a function which gets passed a table with the keys
            -- * query_string: eg '@function.inner'
            -- * method: eg 'v' or 'o'
            -- and should return the mode ('v', 'V', or '<c-v>') or a table
            -- mapping query_strings to modes.
            selection_modes = {
              ["@parameter.outer"] = "v", -- charwise
              ["@function.outer"] = "V", -- linewise
              -- ['@class.outer'] = '<c-v>', -- blockwise
            },
            -- If you set this to `true` (default is `false`) then any textobject is
            -- extended to include preceding or succeeding whitespace. Succeeding
            -- whitespace has priority in order to act similarly to eg the built-in
            -- `ap`.
            --
            -- Can also be a function which gets passed a table with the keys
            -- * query_string: eg '@function.inner'
            -- * selection_mode: eg 'v'
            -- and should return true of false
            include_surrounding_whitespace = false,
          },
          move = {
            -- whether to set jumps in the jumplist
            set_jumps = true,
          },
        })

        -- You can use the capture groups defined in `textobjects.scm`
        --------------------------------------------------------------
        -- Select (inside/around):
        local select = require("nvim-treesitter-textobjects.select")

        -- stylua: ignore start
        map({ mode = { "x", "o" }, lhs = "ac", rhs = function() select.select_textobject("@class.outer", "textobjects") end, desc = "outer class" })
        map({ mode = { "x", "o" }, lhs = "ic", rhs = function() select.select_textobject("@class.inner", "textobjects") end, desc = "inner class" })

        map({ mode = { "x", "o" }, lhs = "am", rhs = function() select.select_textobject("@function.outer", "textobjects") end, desc = "outer function" })
        map({ mode = { "x", "o" }, lhs = "im", rhs = function() select.select_textobject("@function.inner", "textobjects") end, desc = "inner function" })

        map({ mode = { "x", "o" }, lhs = "al", rhs = function() select.select_textobject("@loop.outer", "textobjects") end, desc = "outer loop" })
        map({ mode = { "x", "o" }, lhs = "il", rhs = function() select.select_textobject("@loop.inner", "textobjects") end, desc = "inner loop" })

        map({ mode = { "x", "o" }, lhs = "ak", rhs = function() select.select_textobject("@conditional.outer", "textobjects") end, desc = "outer conditional" })
        map({ mode = { "x", "o" }, lhs = "ik", rhs = function() select.select_textobject("@conditional.inner", "textobjects") end, desc = "inner conditional" })

        -- You can also use captures from other query groups like `locals.scm`.
        map({ mode = { "x", "o" }, lhs = "as", rhs = function() select.select_textobject("@local.scope", "locals") end, desc = "local scope" })

        -- Move (next/previous)
        ------------------------
        local move = require("nvim-treesitter-textobjects.move")

        -- Classes:
        map({ mode = { "n", "x", "o" }, lhs = "[c", rhs = function() move.goto_previous_start({ "@class.outer", "@class.inner" }, "textobjects") end, desc = "previous class start" })
        map({ mode = { "n", "x", "o" }, lhs = "]c", rhs = function() move.goto_next_start({ "@class.outer", "@class.inner" }, "textobjects") end, desc = "next class start" })
        map({ mode = { "n", "x", "o" }, lhs = "[C", rhs = function() move.goto_previous_end({ "@class.outer", "@class.inner" }, "textobjects") end, desc = "previous class end" })
        map({ mode = { "n", "x", "o" }, lhs = "]C", rhs = function() move.goto_next_end({ "@class.outer", "@class.inner" }, "textobjects") end, desc = "next class end"})

        -- Functions / Methods:
        map({ mode = { "n", "x", "o" }, lhs = "[m", rhs = function() move.goto_previous_start({ "@function.outer", "@function.inner" }, "textobjects") end, desc = "previous function start" })
        map({ mode = { "n", "x", "o" }, lhs = "]m", rhs = function() move.goto_next_start({ "@function.outer", "@function.inner" }, "textobjects") end, desc = "next function start" })
        map({ mode = { "n", "x", "o" }, lhs = "[M", rhs = function() move.goto_previous_end({ "@function.outer", "@function.inner" }, "textobjects") end, desc = "previous function end" })
        map({ mode = { "n", "x", "o" }, lhs = "]M", rhs = function() move.goto_next_end({ "@function.outer", "@function.inner" }, "textobjects") end, desc = "next function end" })

        -- Loops:
        map({ mode = { "n", "x", "o" }, lhs = "[l", rhs = function() move.goto_previous_start({ "@loop.outer", "@loop.inner" }, "textobjects") end, desc = "previous loop start" })
        map({ mode = { "n", "x", "o" }, lhs = "]l", rhs = function() move.goto_next_start({ "@loop.outer", "@loop.inner" }, "textobjects") end, desc = "next loop start" })
        map({ mode = { "n", "x", "o" }, lhs = "[L", rhs = function() move.goto_previous_end({ "@loop.outer", "@loop.inner" }, "textobjects") end, desc = "previous loop end" })
        map({ mode = { "n", "x", "o" }, lhs = "]L", rhs = function() move.goto_next_end({ "@loop.outer", "@loop.inner" }, "textobjects") end, desc = "next loop end" })

        -- Locals:
        map({ mode = { "n", "x", "o" }, lhs = "]s", rhs = function() move.goto_next("@local.scope", "locals") end, desc = "next local scope" })
        map({ mode = { "n", "x", "o" }, lhs = "[s", rhs = function() move.goto_previous("@local.scope", "locals") end, desc = "previous local scope" })

        -- Folds:
        map({ mode = { "n", "x", "o" }, lhs = "]z", rhs = function() move.goto_next("@fold", "folds") end, desc = "next fold" })
        map({ mode = { "n", "x", "o" }, lhs = "[z", rhs = function() move.goto_previous("@fold", "folds") end, desc = "previous fold" })

        map({ mode = { "n", "x", "o" }, lhs = "]k", rhs = function() move.goto_next({ "@conditional.outer", "@conditional.inner" }, "textobjects") end, desc = "next conditional" })
        map({ mode = { "n", "x", "o" }, lhs = "[k", rhs = function() move.goto_previous({ "@conditional.outer", "@conditional.inner" }, "textobjects") end, desc = "previous conditional" })

        -- Repeat movements
        -------------------
        local ts_repeat_move = require("nvim-treesitter-textobjects.repeatable_move")

        -- Repeat movement with ; and , (This ensures ; goes forward and , goes backward regardless of the last direction.)
        -- map({ mode = { "n", "x", "o" }, lhs = ";", rhs = ts_repeat_move.repeat_last_move_next, desc = "" })
        -- map({ mode = { "n", "x", "o" }, lhs = ",", rhs = ts_repeat_move.repeat_last_move_previous, desc = "" })

        -- vim way: ; goes to the direction you were moving.
        map({ mode = { "n", "x", "o" }, lhs = ";", rhs = ts_repeat_move.repeat_last_move, desc = "treesitter-textobjects: repeat last move" })
        map({ mode = { "n", "x", "o" }, lhs = ",", rhs = ts_repeat_move.repeat_last_move_opposite, desc = "treesitter-textobjects: repeat last move (opposite)" })

        -- Optionally, make builtin f, F, t, T also repeatable with ; and ,
        map({ mode = { "n", "x", "o" }, lhs = "f", rhs = ts_repeat_move.builtin_f_expr, expr = true, desc = "treesitter-textobjects: f (with repeat)" })
        map({ mode = { "n", "x", "o" }, lhs = "F", rhs = ts_repeat_move.builtin_F_expr, expr = true, desc = "treesitter-textobjects: F (with repeat)" })
        map({ mode = { "n", "x", "o" }, lhs = "t", rhs = ts_repeat_move.builtin_t_expr, expr = true, desc = "treesitter-textobjects: t (with repeat)" })
        map({ mode = { "n", "x", "o" }, lhs = "T", rhs = ts_repeat_move.builtin_T_expr, expr = true, desc = "treesitter-textobjects: T (with repeat)" })
      end,
    },
  },
  lazy = false,
  branch = "main",
  build = ":TSUpdate",
  config = function()
    local treesitter = require("nvim-treesitter")

    -- Track buffers waiting for parser installation: { lang = { [buf] = true, ... } }.
    local waiting_buffers = {}

    -- Track languages currently being installed to avoid duplicate install tasks.
    local installing_languages = {}

    local treesitter_setup_group = vim.api.nvim_create_augroup("TreesitterSetup", { clear = true })

    -- Enable treesitter for a buffer.
    local function enable_treesitter(buffer, language)
      if not vim.api.nvim_buf_is_valid(buffer) then
        return false
      end

      local ok = pcall(vim.treesitter.start, buffer, language)
      if ok then
        -- Set treesitter indentation (buffer-local).
        vim.api.nvim_set_option_value("indentexpr", "v:lua.require'nvim-treesitter'.indentexpr()", { buf = buffer })

        -- Set treesitter folding for all windows displaying this buffer (window-local):
        for _, win in ipairs(vim.api.nvim_list_wins()) do
          if vim.api.nvim_win_get_buf(win) == buffer and vim.api.nvim_win_is_valid(win) then
            vim.api.nvim_set_option_value("foldmethod", "expr", { win = win })
            vim.api.nvim_set_option_value("foldexpr", "v:lua.vim.treesitter.foldexpr()", { win = win })
          end
        end
      end
      return ok
    end

    -- Install core parsers after lazy.nvim finishes loading all plugins.
    vim.api.nvim_create_autocmd("User", {
      group = treesitter_setup_group,
      pattern = "LazyDone",
      once = true,
      desc = "Install core treesitter parsers",
      callback = function()
        treesitter.install({
          "bash",
          "c",
          "clojure",
          "cpp",
          "css",
          "diff",
          "dockerfile",
          "ecma",
          "eex",
          "elixir",
          "git_config",
          "git_rebase",
          "gitattributes",
          "gitcommit",
          "gitignore",
          "gpg",
          "graphql",
          "haskell",
          "hcl",
          "heex",
          "html",
          "html_tags",
          "http",
          "ini",
          "javascript",
          "jq",
          "json",
          "jsx",
          "just",
          "latex",
          "lua",
          "markdown",
          "markdown_inline",
          "ninja",
          "nix",
          "nu",
          "python",
          "query",
          "regex",
          "ron",
          "rst",
          "rust",
          "swift",
          "terraform",
          "toml",
          "typescript",
          "typst",
          "vim",
          "vimdoc",
          "xml",
          "yaml",
          "zig",
        })
      end,
    })

    local ignore_filetypes = {
      checkhealth = true,
      lazy = true,
      mason = true,
      notify = true,
      noice = true,
      qf = true,
      toggleterm = true,
    }

    -- Auto-install parsers and enable highlighting on FileType.
    vim.api.nvim_create_autocmd("FileType", {
      group = treesitter_setup_group,
      desc = "Enable treesitter highlighting and indentation",
      callback = function(event)
        local filetype = event.match

        if ignore_filetypes[filetype] then
          return
        end

        local language = vim.treesitter.language.get_lang(filetype) or filetype
        local buffer = event.buf

        if not enable_treesitter(buffer, language) then
          -- Parser not available, queue buffer (set handles duplicates).
          waiting_buffers[language] = waiting_buffers[language] or {}
          waiting_buffers[language][buffer] = true

          -- Only start install if not already in progress.
          if not installing_languages[language] then
            installing_languages[language] = true
            treesitter.install({ language })

            -- Poll for parser availability.
            local attempts = 0
            local max_attempts = 60 -- 30 seconds max (60 * 500ms).
            local function poll_parser()
              attempts = attempts + 1
              local ok, result = pcall(vim.treesitter.language.add, language)

              if ok and result then
                installing_languages[language] = nil
                local buffers = waiting_buffers[language]
                if buffers then
                  for b in pairs(buffers) do
                    enable_treesitter(b, language)
                  end
                  waiting_buffers[language] = nil
                end
              elseif attempts < max_attempts then
                vim.defer_fn(poll_parser, 500)
              else
                -- Timeout, clean up
                installing_languages[language] = nil
                waiting_buffers[language] = nil
              end
            end

            vim.defer_fn(poll_parser, 1000)
          end
        end
      end,
    })

    -- Clean up waiting buffers when buffer is deleted.
    vim.api.nvim_create_autocmd("BufDelete", {
      group = treesitter_setup_group,
      desc = "Clean up treesitter waiting buffers",
      callback = function(event)
        for lang, buffers in pairs(waiting_buffers) do
          buffers[event.buf] = nil
          if next(buffers) == nil then
            waiting_buffers[lang] = nil
          end
        end
      end,
    })
  end,
}
