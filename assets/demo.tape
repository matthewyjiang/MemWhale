# MemoryWhale README demo. Regenerate with:  vhs assets/demo.tape  (from repo root)
# Prereqs:  cargo build --release -p mw-cli   (binaries land in target/release)
#           a throwaway crate at ~/auv/camera-driver whose src/main.rs has a
#           type error, e.g.  let fps: i32 = "30";  so `cargo build` really fails.
Output assets/demo.gif

Set Shell "bash"
Set FontSize 20
Set Width 1240
Set Height 620
Set Padding 26
Set Theme "Catppuccin Mocha"
Set TypingSpeed 35ms

# ---- silent setup ----------------------------------------------------------
Hide
Type `export PATH="$PWD/target/release:$PATH"`
Enter
Type `export MEMORYWHALE_DATA_DIR="$(mktemp -d)"`
Enter
Type `export PS1='\[\e[38;5;39m\]$\[\e[0m\] '`
Enter
# seed one older failure so the recall shows real history
Type `mw-remember --exit-code 1 --cwd ~/auv --stderr 'fatal: unable to access /dev/video0: Permission denied' --notes 'project:auv flash' -- v4l2-ctl --list-devices >/dev/null`
Enter
Type `cd ~/auv/camera-driver && cargo clean`
Enter
Type "clear"
Enter
Show

# ---- the demo --------------------------------------------------------------
Type "# capture a build the moment it fails:"
Enter
Sleep 500ms
Type "mw-run --notes 'project:auv camera-driver' -- cargo build"
Enter
Sleep 4500ms

Type "# weeks later, another machine. what was that error?"
Enter
Sleep 900ms
Type "mw context --last-error"
Enter
Sleep 4500ms

Type "# or let your agent read it: claude mcp add memorywhale -- mw-mcp"
Enter
Sleep 3s
