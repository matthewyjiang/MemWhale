# Neovim plugin

`memorywhale.lua` brings MemoryWhale into the editor — four commands, no
plugin manager required.

## Install

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

## Commands

```
:MwAsk [question]      package the last failure (exact error + similar past
                       failures + saved lessons) into a debugging prompt —
                       clipboard filled, chatgpt.com opened; you paste.
:MwGitFix [id]         diagnose the last failed git command (push rejected,
                       merge conflict, …) with the fix and your history.
:MwSearch {text}       full-text search commands, output, notes, transcripts.
:MwRemember {text}     save a lesson. With a visual selection and no args,
                       remembers the selected lines — select an error or a
                       fix in any buffer and save it in one keystroke.
```

`:MwAsk`, `:MwGitFix`, and `:MwSearch` open in a terminal split; `:MwRemember`
runs inline and notifies the result.

Requires `mw` on `$PATH` (the standard MemoryWhale install). Every command
reports clearly if it isn't found.

> `mw-git-fix.lua` (the earlier single-command module) still works, but
> `memorywhale.lua` includes `:MwGitFix` and supersedes it — install one, not
> both.
