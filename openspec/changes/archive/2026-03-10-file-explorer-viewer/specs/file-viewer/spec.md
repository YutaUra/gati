## ADDED Requirements

### Requirement: File viewer displays selected file contents
The system SHALL display the full contents of the selected file in the right pane with line numbers. Each line SHALL be prefixed with its line number, right-aligned. The line number gutter width SHALL adjust based on the total number of lines in the file.

#### Scenario: Select a file in the tree
- **WHEN** the user selects a file in the file tree
- **THEN** the file viewer displays the file's contents with line numbers

#### Scenario: No file selected
- **WHEN** no file has been selected yet (e.g., application just started and cursor is on a directory)
- **THEN** the file viewer displays a placeholder message (e.g., "Select a file to preview")

### Requirement: File viewer supports scrolling
The system SHALL allow vertical scrolling through the file contents using j/k or Up/Down arrows (line by line) and Ctrl-d/Ctrl-u (half page) when the viewer pane is focused.

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

#### Scenario: Scroll clamped at end of file
- **WHEN** the view is at the end of the file and the user presses j or Ctrl-d
- **THEN** the view does not scroll further

#### Scenario: Scroll clamped at beginning of file
- **WHEN** the view is at the beginning of the file and the user presses k or Ctrl-u
- **THEN** the view does not scroll further

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
