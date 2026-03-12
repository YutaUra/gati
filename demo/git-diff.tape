# VHS tape: Git diff view & changed files filter
# Run: vhs demo/git-diff.tape
# Requires: gati built (cargo build --release)
# NOTE: Best demonstrated when the repo has uncommitted changes.
#       The tape navigates to src/diff.rs which should have [M] marker.

Output demo/git-diff.gif

Set Shell zsh
Set FontSize 14
Set Width 1200
Set Height 800
Set Padding 20
Set Theme "Dracula"

# Start gati
Type "./target/release/gati"
Enter
Sleep 2s

# Filter to changed files only (shows only modified files)
Type "g"
Sleep 2s

# Navigate: demo [●] is first, move to src [●]
Type "j"
Sleep 500ms

# Expand src directory
Type "l"
Sleep 500ms

# Move to diff.rs [M]
Type "j"
Sleep 500ms

# Open diff.rs
Enter
Sleep 2s

# Toggle diff view
Type "d"
Sleep 2s

# Scroll down to the actual changes (~line 157)
Ctrl+d
Sleep 500ms
Ctrl+d
Sleep 500ms
Ctrl+d
Sleep 500ms
Ctrl+d
Sleep 500ms
Ctrl+d
Sleep 500ms
Ctrl+d
Sleep 500ms
Ctrl+d
Sleep 500ms
Ctrl+d
Sleep 2s

# Toggle diff off
Type "d"
Sleep 1s

# Switch back to file tree
Tab
Sleep 1s

# Turn off changed files filter
Type "g"
Sleep 1s

# Quit
Type "q"
Sleep 500ms
