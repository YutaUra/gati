### Requirement: Two-pane layout
The system SHALL display a two-pane layout with the file tree on the left (30% width) and the file viewer on the right (70% width). Panes SHALL be separated by a visible border.

#### Scenario: Application startup
- **WHEN** the application starts
- **THEN** a two-pane layout is rendered with the file tree on the left and viewer on the right
- **AND** the file tree pane is focused by default

#### Scenario: Minimum terminal size
- **WHEN** the terminal width is less than 40 columns or height is less than 10 rows
- **THEN** the application displays an error message requesting a larger terminal and exits

### Requirement: Key hint bar at the bottom
The system SHALL display a key hint bar at the bottom of the screen showing available keybindings for the current context. The hint bar SHALL update based on which pane is focused.

#### Scenario: File tree is focused
- **WHEN** the file tree pane is focused
- **THEN** the hint bar shows relevant keys: j/k navigate, h/l fold/unfold, Enter open, Tab switch pane, q quit

#### Scenario: File viewer is focused
- **WHEN** the file viewer pane is focused
- **THEN** the hint bar shows relevant keys: j/k scroll, Ctrl-d/Ctrl-u page scroll, Tab switch pane, q quit

### Requirement: Pane focus switching
The system SHALL allow switching focus between the file tree and file viewer using the Tab key. The Tab key SHALL toggle focus between the two panes. The focused pane SHALL be visually indicated by a highlighted border (e.g., brighter or different color). Additionally, pressing Enter on a file in the tree SHALL switch focus to the viewer.

#### Scenario: Toggle focus with Tab
- **WHEN** the user presses Tab
- **THEN** focus moves to the other pane and its border is highlighted

#### Scenario: Enter on file switches to viewer
- **WHEN** the file tree is focused and the user presses Enter on a file entry
- **THEN** focus moves to the file viewer pane

### Requirement: Application quit
The system SHALL quit when the user presses q. The terminal SHALL be restored to its original state on exit, including on panic or unexpected errors.

#### Scenario: Quit the application
- **WHEN** the user presses q
- **THEN** the application exits and the terminal is restored to its original state

#### Scenario: Terminal restoration on crash
- **WHEN** the application panics or encounters an unrecoverable error
- **THEN** the terminal is still restored to its original state (via a panic hook)
