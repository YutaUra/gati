# VHS tape: Inline comments & export
# Run: vhs demo/comments.tape
# Requires: gati built (cargo build --release)

Output demo/comments.gif

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

# Navigate to src/ (3 down) and expand
Type "j"
Sleep 300ms
Type "j"
Sleep 300ms
Type "j"
Sleep 300ms
Type "l"
Sleep 500ms

# Navigate to app.rs and open
Type "j"
Sleep 300ms
Enter
Sleep 2s

# Navigate down a few lines in the viewer
Type "j"
Sleep 200ms
Type "j"
Sleep 200ms
Type "j"
Sleep 200ms
Type "j"
Sleep 200ms
Type "j"
Sleep 500ms

# Add a comment on current line
Type "c"
Sleep 500ms
Type "TODO: Consider extracting this logic"
Enter
Sleep 2s

# Move down a few lines
Type "j"
Sleep 200ms
Type "j"
Sleep 200ms
Type "j"
Sleep 500ms

# Add another comment
Type "c"
Sleep 500ms
Type "This could use better error handling"
Enter
Sleep 2s

# Export comments to clipboard
Type "e"
Sleep 2s

# Quit
Type "q"
Sleep 500ms
