# Neovim + MemoryWhale

`memorywhale.lua` brings MemoryWhale into the editor with four commands. It
calls the `mw` CLI. It is not an MCP client.

## Status

This plugin is the Lua module in this repository. There is no third-party
Neovim MCP adapter documented here. Commands require `mw` on `$PATH`.

## Requirements

- MemoryWhale installed with `mw` on `$PATH`.
- Neovim with Lua `init.lua`.
- A checkout of this repository to copy `memorywhale.lua`.

## Setup

```bash
mkdir -p ~/.config/nvim/lua
cp integrations/neovim/memorywhale.lua ~/.config/nvim/lua/memorywhale.lua
```

Then in `init.lua`:

```lua
require("memorywhale").setup()
-- optional keymaps
vim.keymap.set("n", "<leader>ma", ":MwAsk<CR>",    { desc = "MemoryWhale: ask ChatGPT about the last failure" })
vim.keymap.set("n", "<leader>mg", ":MwGitFix<CR>", { desc = "MemoryWhale: diagnose last git failure" })
vim.keymap.set("v", "<leader>mr", ":MwRemember<CR>", { desc = "MemoryWhale: remember selection" })
```

`mw-git-fix.lua` (the earlier single-command module) still works, but
`memorywhale.lua` includes `:MwGitFix` and supersedes it. Install one, not
both.

## Verify

```bash
command -v mw
mw doctor
```

In Neovim, run `:MwSearch test` or `:MwAsk`. Each command reports clearly if
`mw` is not found. `:MwAsk`, `:MwGitFix`, and `:MwSearch` open in a terminal
split; `:MwRemember` runs inline and notifies the result.

## Available capabilities

| Capability | Available |
| --- | --- |
| MCP memory access | No; uses the CLI directly |
| Automatic execution capture | No |
| Memory-use guidance | Commands only |

```text
:MwAsk [question]      package the last failure (exact error + similar past
                       failures + saved lessons) into a debugging prompt.
                       Clipboard filled, chatgpt.com opened; you paste.
:MwGitFix [id]         diagnose the last failed git command (push rejected,
                       merge conflict, …) with the fix and your history.
:MwSearch {text}       full-text search commands, output, notes, transcripts.
:MwRemember {text}     save a lesson. With a visual selection and no args,
                       remembers the selected lines. Select an error or a
                       fix in any buffer and save it in one keystroke.
```

The plugin does not register `mw-mcp` and does not record Neovim terminal jobs
automatically.

## Example prompt

Not applicable. Neovim is not an agent host here. Use `:MwAsk` to package the
last failure for an external chat, or `:MwSearch` / `:MwRemember` in the
editor.

## Troubleshooting

- Run `command -v mw` from the same environment as Neovim. GUI Neovim often
  has a shorter `PATH` than your shell; fix that or wrap `mw` with an absolute
  path in your own config.
- Confirm `require("memorywhale").setup()` runs from `init.lua`.
- Run `mw doctor` if search or remember return empty or missing-database
  errors.

## Uninstall

Remove `require("memorywhale").setup()` and any keymaps from `init.lua`, then
delete `~/.config/nvim/lua/memorywhale.lua`. This does not delete the
MemoryWhale database.
