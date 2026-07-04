# Neovim integration

`mw-git-fix.lua` adds a `:MwGitFix` command that runs [`mw git-fix`](../../docs/CLI.md)
in a terminal split, so you can diagnose the last failed git command — push
rejected, merge conflict, dirty working tree, diverged branches, unrelated
histories, SSH auth failures — without leaving the editor.

## Install

```bash
mkdir -p ~/.config/nvim/lua
cp integrations/neovim/mw-git-fix.lua ~/.config/nvim/lua/mw-git-fix.lua
```

Then in `init.lua`:

```lua
require("mw-git-fix").setup()
-- optional: a keymap
vim.keymap.set("n", "<leader>gf", ":MwGitFix<CR>", { desc = "MemoryWhale: diagnose last git failure" })
```

## Usage

```
:MwGitFix       diagnose the most recent failed git command
:MwGitFix 42    diagnose a specific command_run id (from `mw list`/`mw search`)
```

Opens a terminal split running `mw git-fix`, which explains what the error
means, prints the fix, and checks whether this exact failure — or a lesson
you already saved with `mw remember` — has come up before.

Requires `mw` on `$PATH` (the standard MemoryWhale install). If it isn't
found, `:MwGitFix` reports that instead of failing silently.
