-- MemoryWhale for Neovim: terminal memory without leaving the editor.
--
-- Install: copy this file into your config and require it, e.g.
--   mkdir -p ~/.config/nvim/lua
--   cp integrations/neovim/memorywhale.lua ~/.config/nvim/lua/memorywhale.lua
-- then in init.lua:
--   require("memorywhale").setup()
--
-- Commands:
--   :MwAsk [question]     package the last failure (+ history + lessons) for
--                         ChatGPT/Claude — clipboard filled, browser opened
--   :MwGitFix [id]        diagnose the last failed git command
--   :MwSearch {text}      search commands, output, notes, and transcripts
--   :MwRemember {text}    save a lesson; with a visual selection and no args,
--                         remembers the selected lines instead
--
-- Requires the `mw` binary on $PATH (the standard MemoryWhale install).

local M = {}

local function have_mw()
  if vim.fn.executable("mw") == 0 then
    vim.notify("MemoryWhale: `mw` not found on $PATH — is it installed?", vim.log.levels.ERROR)
    return false
  end
  return true
end

-- Run an mw subcommand in a bottom terminal split (for commands with output
-- worth reading: ask/git-fix/search).
local function term_run(args, height)
  vim.cmd("botright new")
  vim.cmd("resize " .. (height or 20))
  vim.fn.termopen(args)
  vim.cmd("startinsert")
end

function M.setup(opts)
  opts = opts or {}
  local height = opts.height or 20

  vim.api.nvim_create_user_command("MwAsk", function(c)
    if not have_mw() then return end
    local args = { "mw", "ask" }
    for _, w in ipairs(vim.split(c.args, "%s+", { trimempty = true })) do
      table.insert(args, w)
    end
    term_run(args, height)
  end, { nargs = "*", desc = "MemoryWhale: package the last failure for ChatGPT/Claude" })

  vim.api.nvim_create_user_command("MwGitFix", function(c)
    if not have_mw() then return end
    if c.args ~= "" and not c.args:match("^%d+$") then
      vim.notify("MwGitFix: id must be a number", vim.log.levels.ERROR)
      return
    end
    local args = { "mw", "git-fix" }
    if c.args ~= "" then table.insert(args, c.args) end
    term_run(args, height)
  end, { nargs = "?", desc = "MemoryWhale: diagnose the last failed git command" })

  vim.api.nvim_create_user_command("MwSearch", function(c)
    if not have_mw() then return end
    if c.args == "" then
      vim.notify("MwSearch: give a search term", vim.log.levels.ERROR)
      return
    end
    term_run({ "mw", "search", c.args }, height)
  end, { nargs = "+", desc = "MemoryWhale: search terminal memory" })

  vim.api.nvim_create_user_command("MwRemember", function(c)
    if not have_mw() then return end
    local text = c.args
    if text == "" and c.range > 0 then
      -- No args but a visual range: remember the selected lines.
      local lines = vim.api.nvim_buf_get_lines(0, c.line1 - 1, c.line2, false)
      text = table.concat(lines, " ")
    end
    if text == "" then
      vim.notify("MwRemember: give text or select lines first", vim.log.levels.ERROR)
      return
    end
    local out = vim.fn.system({ "mw", "remember", text })
    if vim.v.shell_error ~= 0 then
      vim.notify("MwRemember failed: " .. out, vim.log.levels.ERROR)
    else
      vim.notify(vim.trim(out))
    end
  end, {
    nargs = "*",
    range = true,
    desc = "MemoryWhale: save a lesson (args, or the visual selection)",
  })
end

return M
