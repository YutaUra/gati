# VHS tape: Basic navigation & file preview
# Run: vhs demo/basic.tape
# Requires: gati built (cargo build --release)

Output demo/basic.gif

Set Shell zsh
Set FontSize 14
Set Width 1200
Set Height 800
Set Padding 20
Set Theme "Dracula"

# Start gati in the project directory
Type "./target/release/gati"
Enter
Sleep 2s

# Tree: demo, docs, openspec, src, Cargo.lock, ...
# Navigate to src/ (3 down from demo)
Type "j"
Sleep 400ms
Type "j"
Sleep 400ms
Type "j"
Sleep 400ms

# Expand src/ directory
Type "l"
Sleep 500ms

# Navigate to app.rs (first file in src/)
Type "j"
Sleep 400ms

# Open app.rs in preview
Enter
Sleep 2s

# Scroll down in the viewer
Type "j"
Sleep 200ms
Type "j"
Sleep 200ms
Type "j"
Sleep 200ms
Type "j"
Sleep 200ms
Type "j"
Sleep 1s

# Half-page scroll
Ctrl+d
Sleep 1s
Ctrl+d
Sleep 1s

# Switch pane back to file tree
Tab
Sleep 1s

# Use search to find main.rs
Type "/"
Sleep 500ms
Type "main"
Sleep 1s
Enter
Sleep 2s

# Quit
Type "q"
Sleep 500ms
