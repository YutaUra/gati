### Requirement: File viewer displays selected file contents
The system SHALL display the full contents of the selected file in the right pane with syntax highlighting and line numbers. Each line SHALL be prefixed with its line number, right-aligned. The line number gutter width SHALL adjust based on the total number of lines in the file. File contents SHALL be syntax-highlighted based on the file's language, detected from file extension or first-line content (e.g., shebang). The file content area SHALL be reduced by 2 columns on the right to accommodate the minimap column, unless the viewer's inner width is less than 30 columns.

#### Scenario: Select a file in the tree
- **WHEN** the user selects a file in the file tree
- **THEN** the file viewer displays the file's contents with line numbers and syntax highlighting appropriate for the file's language
- **AND** the rightmost 2 columns of the viewer are reserved for the minimap

#### Scenario: No file selected
- **WHEN** no file has been selected yet (e.g., application just started and cursor is on a directory)
- **THEN** the file viewer displays a placeholder message (e.g., "Select a file to preview")

#### Scenario: File with recognized language
- **WHEN** the user selects a file with a known extension (e.g., `.rs`, `.py`, `.js`, `.md`)
- **THEN** the file viewer renders the contents with language-specific syntax highlighting

#### Scenario: File with unrecognized language
- **WHEN** the user selects a file with an unknown extension or no extension
- **THEN** the file viewer renders the contents as plain text without highlighting

#### Scenario: File with shebang line
- **WHEN** the user selects a file with no recognizable extension but a shebang line (e.g., `#!/bin/bash`)
- **THEN** the file viewer detects the language from the first line and applies syntax highlighting

### Requirement: File viewer supports scrolling
The system SHALL allow vertical scrolling through the file contents using j/k or Up/Down arrows (line by line) and Ctrl-d/Ctrl-u (half page) when the viewer pane is focused. The file viewer SHALL also support mouse wheel scrolling when the mouse cursor is over the viewer pane, scrolling by 5 lines per tick.

#### Scenario: Scroll down line by line
- **WHEN** the viewer is focused and the user presses j or Down arrow
- **THEN** the view scrolls down by one line

#### Scenario: Scroll up line by line
- **WHEN** the viewer is focused and the user presses k or Up arrow
- **THEN** the view scrolls up by one line

#### Scenario: Scroll down half page
- **WHEN** the viewer is focused and the user presses Ctrl-d
- **THEN** the view scrolls down by half the pane height

#### Scenario: Scroll up half page
- **WHEN** the viewer is focused and the user presses Ctrl-u
- **THEN** the view scrolls up by half the pane height

#### Scenario: Vertical scroll clamped at end of file
- **WHEN** the view is scrolled to the bottom of the file
- **THEN** scrolling stops when the last line of the file reaches the bottom of the viewport, plus 1 line of padding

#### Scenario: Mouse wheel scroll down
- **WHEN** the mouse cursor is over the file viewer pane and the user scrolls the mouse wheel down
- **THEN** the viewer content scrolls down by 5 lines

#### Scenario: Mouse wheel scroll up
- **WHEN** the mouse cursor is over the file viewer pane and the user scrolls the mouse wheel up
- **THEN** the viewer content scrolls up by 5 lines

#### Scenario: Mouse wheel over tree pane does not scroll viewer
- **WHEN** the mouse cursor is over the file tree pane and the user scrolls the mouse wheel
- **THEN** the viewer content does not scroll (the tree scrolls instead)

#### Scenario: Scroll clamped at beginning of file
- **WHEN** the view is at the beginning of the file and the user presses k or Ctrl-u
- **THEN** the view does not scroll further

### Requirement: File viewer supports horizontal scrolling
The file viewer SHALL support horizontal scrolling to view content that extends beyond the viewport width. Horizontal scrolling SHALL be available via keyboard (H/L or Left/Right arrows) and mouse (Shift+wheel or native horizontal scroll). The line number gutter SHALL remain fixed and not be affected by horizontal scrolling. Horizontal scroll offset SHALL reset to 0 when a new file is loaded.

#### Scenario: Scroll right with keyboard
- **WHEN** the user presses L or Right arrow in the file viewer
- **THEN** the viewport shifts right by 4 columns, revealing more content on the right side

#### Scenario: Scroll left with keyboard
- **WHEN** the user presses H or Left arrow in the file viewer
- **THEN** the viewport shifts left by 4 columns (minimum 0)

#### Scenario: Horizontal scroll resets on file change
- **WHEN** a new file is loaded in the viewer
- **THEN** the horizontal scroll offset resets to 0

#### Scenario: Mouse horizontal scroll
- **WHEN** the mouse cursor is over the viewer pane and the user scrolls horizontally (Shift+wheel or native horizontal scroll)
- **THEN** the viewport shifts left or right by 4 columns per tick

#### Scenario: Horizontal offset applied to rendering
- **WHEN** the viewer has a horizontal scroll offset greater than 0
- **THEN** each line of content is rendered starting from that character offset
- **AND** the line number gutter remains fixed (not scrolled)

#### Scenario: Horizontal scroll clamped at end of longest line
- **WHEN** the view is scrolled horizontally to the right
- **THEN** scrolling stops when the end of the longest line reaches the right edge of the viewport, plus 2 columns of padding

#### Scenario: Horizontal scroll clamped when content fits viewport
- **WHEN** all lines in the file are shorter than the viewport width
- **THEN** horizontal scrolling has no effect

### Requirement: Binary files are detected and not displayed as text
The system SHALL detect binary files by checking for null bytes in the first 512 bytes. Binary files SHALL display a placeholder message instead of their raw contents.

#### Scenario: Open a binary file
- **WHEN** the user selects a binary file (e.g., a compiled executable)
- **THEN** the viewer displays the message "Binary file — cannot display"

### Requirement: Empty files display a message
The system SHALL display a message when the selected file is empty.

#### Scenario: Open an empty file
- **WHEN** the user selects an empty file
- **THEN** the viewer displays the message "Empty file"

### Requirement: Permission errors are handled gracefully
The system SHALL display an error message when a file cannot be read due to permission errors, rather than crashing.

#### Scenario: Open an unreadable file
- **WHEN** the user selects a file that cannot be read (e.g., insufficient permissions)
- **THEN** the viewer displays a message indicating the file cannot be read
