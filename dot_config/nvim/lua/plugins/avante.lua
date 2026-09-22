-- avante.nvim on its Agent Client Protocol provider, which drives the `claude`
-- executable already on PATH. That is the credential: the Claude Code CLI's own
-- subscription login, reached through `ACP_PATH_TO_CLAUDE_CODE_EXECUTABLE` in
-- the provider's default environment. No API key, no environment variable and
-- no vault entry, and `avante.providers.setup` returns before its key prompt
-- for any provider named in `acp_providers`, so a machine that has never held
-- an Anthropic key still loads this plugin silently.
--
-- The provider spawns `claude-agent-acp`, the npm package of the same name
-- declared on the fnm node lane in
-- `.chezmoidata/system_packages_autoinstall.yaml`. It is needed the first time
-- a chat opens, not at load.
--
-- `build = "make"` compiles four cdylibs (tokenizers, templates, repo-map and
-- html2md) out of the plugin's own cargo workspace, so `cargo` has to be on
-- PATH when lazy.nvim runs the build.
--
-- Avante's own key family is `<leader>a`, which is aerial's group here, so this
-- configuration puts it on `<leader>v`. Every global key avante gets is in the
-- `keys` list below, and `behaviour.auto_set_keymaps = false` is what makes
-- that list the whole surface. The buffer-local keys inside avante's sidebar,
-- diff and input windows stay at their defaults.
return {
  "avante-corp/avante.nvim",
  build = "make",
  dependencies = {
    "nvim-lua/plenary.nvim",
    "MunifTanjim/nui.nvim",
    -- Already in this configuration for noice, aerial and the pickers. Declared
    -- so lazy.nvim loads them ahead of avante, which uses snacks for its input
    -- prompt and devicons in its file selector.
    "folke/snacks.nvim",
    "nvim-tree/nvim-web-devicons",
  },
  cmd = {
    "AvanteAsk",
    "AvanteBuild",
    "AvanteChat",
    "AvanteClear",
    "AvanteSwitchProvider",
    "AvanteToggle",
  },
  keys = {
    -- stylua: ignore start
    { "<leader>va", function() require("avante.api").ask() end,                            mode = { "n", "v" }, desc = "Avante: ask" },
    { "<leader>vn", function() require("avante.api").ask({ new_chat = true }) end,          mode = { "n", "v" }, desc = "Avante: new chat" },
    { "<leader>ve", function() require("avante.api").edit() end,                            mode = "v", desc = "Avante: edit selection" },
    { "<leader>vv", function() require("avante").toggle() end,                              desc = "Avante: toggle sidebar" },
    { "<leader>vf", function() require("avante.api").focus() end,                           desc = "Avante: focus sidebar" },
    { "<leader>vr", function() require("avante.api").refresh() end,                         desc = "Avante: refresh sidebar" },
    { "<leader>vs", function() require("avante.api").stop() end,                            desc = "Avante: stop the request in flight" },
    { "<leader>vh", function() require("avante.api").select_history() end,                  desc = "Avante: select chat history" },
    { "<leader>vm", function() require("avante.api").select_acp_model() end,                desc = "Avante: select agent model" },
    { "<leader>vM", function() require("avante.api").select_acp_mode() end,                 desc = "Avante: select agent mode" },
    { "<leader>vc", function() require("avante.api").add_selected_file(vim.fn.expand("%")) end, desc = "Avante: add current file" },
    { "<leader>vB", function() require("avante.api").add_buffer_files() end,                desc = "Avante: add all buffer files" },
    -- stylua: ignore end
  },
  opts = {
    provider = "claude-code",
    behaviour = {
      auto_set_keymaps = false,
    },
  },
}
