-- MemoryWhale git-fix, from Neovim: diagnose the last failed git command
-- (or a specific one, `mw git-fix <id>`) without leaving the editor.
--
-- Install: copy this file into your config and require it, e.g.
--   mkdir -p ~/.config/nvim/lua
--   cp integrations/neovim/mw-git-fix.lua ~/.config/nvim/lua/mw-git-fix.lua
-- then in init.lua:
--   require("mw-git-fix").setup()
--
-- Usage: :MwGitFix          (diagnoses the most recent failed git command)
--        :MwGitFix 42       (diagnoses command_run #42 specifically)
--
-- Requires `mw` on $PATH (part of the standard MemoryWhale install).

local M = {}

function M.setup(opts)
  opts = opts or {}
  local height = opts.height or 20

  vim.api.nvim_create_user_command("MwGitFix", function(cmd_opts)
    local id = cmd_opts.args
    if id ~= "" and not id:match("^%d+$") then
      vim.notify("MwGitFix: id must be a number, e.g. :MwGitFix 42", vim.log.levels.ERROR)
      return
    end
    if vim.fn.executable("mw") == 0 then
      vim.notify("MwGitFix: `mw` not found on $PATH — is MemoryWhale installed?", vim.log.levels.ERROR)
      return
    end

    local args = { "mw", "git-fix" }
    if id ~= "" then
      table.insert(args, id)
    end

    vim.cmd("botright new")
    vim.cmd("resize " .. height)
    vim.fn.termopen(args)
    vim.cmd("startinsert")
  end, {
    nargs = "?",
    desc = "MemoryWhale: diagnose the last failed git command (or one by id)",
  })
end

return M
