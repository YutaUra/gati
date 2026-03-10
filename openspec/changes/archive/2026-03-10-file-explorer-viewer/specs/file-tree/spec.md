## ADDED Requirements

### Requirement: File tree displays directory contents
The system SHALL display the contents of the target directory as a tree structure with indentation representing depth. Directories SHALL be visually distinguishable from files (e.g., directory icon/indicator vs file icon/indicator).

#### Scenario: Launch with default directory
- **WHEN** the user runs `gati` without arguments
- **THEN** the file tree displays the contents of the current working directory
- **AND** the first entry is selected by default

#### Scenario: Launch with specified directory
- **WHEN** the user runs `gati src/`
- **THEN** the file tree displays the contents of the `src/` directory

#### Scenario: Launch with a file path
- **WHEN** the user runs `gati src/main.rs`
- **THEN** the file tree displays the contents of `src/` (the parent directory)
- **AND** the file `main.rs` is selected in the tree
- **AND** the file viewer displays the contents of `main.rs`

#### Scenario: Launch with non-existent path
- **WHEN** the user runs `gati /nonexistent`
- **THEN** the application exits with a descriptive error message indicating the path does not exist

#### Scenario: Launch with a path the user cannot read
- **WHEN** the user runs `gati` with a path that exists but is not readable
- **THEN** the application exits with an error message indicating insufficient permissions

### Requirement: File tree triggers preview on cursor movement
The system SHALL update the file viewer to display the currently selected file whenever the selection cursor moves to a file entry. This provides an instant preview experience similar to yazi.

#### Scenario: Move cursor to a file
- **WHEN** the user moves the selection cursor to a file entry
- **THEN** the file viewer immediately displays that file's contents

#### Scenario: Move cursor to a directory
- **WHEN** the user moves the selection cursor to a directory entry
- **THEN** the file viewer retains the previously displayed file (or shows a placeholder if no file was previously selected)

### Requirement: Directories can be expanded and collapsed
The system SHALL allow users to expand and collapse directories using l/Right arrow to expand and h/Left arrow to collapse, following vim-style spatial navigation (right = go deeper, left = go back). Collapsed directories SHALL hide their children. The root directory SHALL be expanded by default.

#### Scenario: Expand a collapsed directory with l or Right arrow
- **WHEN** a collapsed directory is selected and the user presses l or Right arrow
- **THEN** the directory expands and its children become visible

#### Scenario: Collapse an expanded directory with h or Left arrow
- **WHEN** an expanded directory is selected and the user presses h or Left arrow
- **THEN** the directory collapses and its children are hidden

#### Scenario: h on a child entry collapses parent directory
- **WHEN** a file or collapsed directory inside an expanded parent is selected and the user presses h or Left arrow
- **THEN** the parent directory collapses and the cursor moves to the parent entry

#### Scenario: h on a root-level entry with no parent
- **WHEN** a root-level file or collapsed directory (depth 0) is selected and the user presses h or Left arrow
- **THEN** nothing happens (no parent to collapse)

#### Scenario: l on a file entry
- **WHEN** a file entry is selected and the user presses l or Right arrow
- **THEN** nothing happens (the selection and tree remain unchanged)

#### Scenario: Expand an empty directory
- **WHEN** an empty directory is selected and the user presses l or Right arrow
- **THEN** the directory expands but shows no children (the tree remains unchanged except for the expanded indicator)

#### Scenario: Enter on a directory
- **WHEN** a directory entry is selected and the user presses Enter
- **THEN** nothing happens (Enter is reserved for opening files in the viewer)

### Requirement: Enter key opens file in viewer
The system SHALL switch focus to the file viewer when the user presses Enter on a file entry. This represents an intentional action to "enter" the file for detailed reading, as opposed to the automatic preview on cursor movement.

#### Scenario: Press Enter on a file
- **WHEN** a file entry is selected in the tree and the user presses Enter
- **THEN** focus moves to the file viewer pane
- **AND** the viewer displays the selected file's contents

### Requirement: File tree navigation with keyboard
The system SHALL support j/k keys and arrow keys (Up/Down) for moving the selection cursor down/up through visible entries in the file tree.

#### Scenario: Move selection down with j or Down arrow
- **WHEN** the user presses j or the Down arrow key
- **THEN** the selection moves to the next visible entry

#### Scenario: Move selection up with k or Up arrow
- **WHEN** the user presses k or the Up arrow key
- **THEN** the selection moves to the previous visible entry

#### Scenario: Selection clamped at bottom boundary
- **WHEN** the selection is at the last visible entry and the user presses j or Down
- **THEN** the selection remains at the last entry

#### Scenario: Selection clamped at top boundary
- **WHEN** the selection is at the first visible entry and the user presses k or Up
- **THEN** the selection remains at the first entry

### Requirement: File tree scrolls with selection
The system SHALL scroll the file tree view to keep the selected entry visible when the selection moves beyond the visible area.

#### Scenario: Selection moves below visible area
- **WHEN** the user navigates down past the last visible row in the tree pane
- **THEN** the tree view scrolls down to keep the selected entry visible

#### Scenario: Selection moves above visible area
- **WHEN** the user navigates up past the first visible row in the tree pane
- **THEN** the tree view scrolls up to keep the selected entry visible

### Requirement: File tree respects .gitignore
The system SHALL filter entries matching .gitignore patterns. Hidden files and directories (starting with `.`) SHALL be hidden by default. The `.git` directory SHALL always be hidden.

#### Scenario: Gitignored files are hidden
- **WHEN** a directory contains files matching .gitignore patterns
- **THEN** those files do not appear in the file tree

#### Scenario: Hidden files are not shown
- **WHEN** a directory contains files starting with `.`
- **THEN** those files do not appear in the file tree

### Requirement: File tree entries are sorted
The system SHALL sort entries with directories first, then files. Within each group, entries SHALL be sorted alphabetically (case-insensitive).

#### Scenario: Directory-first sorting
- **WHEN** a directory contains both files and subdirectories
- **THEN** subdirectories appear before files in the tree

#### Scenario: Case-insensitive sorting
- **WHEN** a directory contains files with mixed case names (e.g., "README.md", "api.rs", "Build.rs")
- **THEN** entries are sorted alphabetically ignoring case
