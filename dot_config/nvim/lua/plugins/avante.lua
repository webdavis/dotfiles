-- Uses the `claude` CLI's own login through the ACP provider, so no API key is needed.
-- `build = "make"` needs `cargo` on PATH.
-- `auto_set_keymaps = false` leaves `keys` as the only global mappings.
-- They sit on `<leader>v`, because `<leader>a` belongs to herdr.
return {
  "avante-corp/avante.nvim",
  build = "make",
  dependencies = {
    "nvim-lua/plenary.nvim",
    "MunifTanjim/nui.nvim",
    -- avante uses snacks for its input prompt and devicons in its file selector.
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
