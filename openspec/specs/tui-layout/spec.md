### Requirement: Two-pane layout
The system SHALL display a two-pane layout with the file tree on the left and the file viewer on the right. The default split SHALL be 30% tree / 70% viewer. The split ratio SHALL be adjustable by mouse-dragging the pane border. Each pane SHALL have a minimum width of max(10% of terminal width, 10 columns). The tree pane SHALL NOT exceed 70% of the terminal width. The system SHALL support a focus mode where the tree pane is hidden and the viewer occupies the full width.

#### Scenario: Application startup
- **WHEN** the application starts
- **THEN** a two-pane layout is rendered with the file tree on the left (30%) and viewer on the right (70%)
- **AND** the file tree pane is focused by default

#### Scenario: Minimum terminal size
- **WHEN** the terminal width is less than 40 columns or height is less than 10 rows
- **THEN** the application displays an error message requesting a larger terminal and exits

#### Scenario: Resize panes by dragging the border
- **WHEN** the user clicks and drags the vertical border between the tree and viewer panes
- **THEN** the pane widths adjust to follow the mouse position
- **AND** the layout is re-rendered with the new ratio

#### Scenario: Minimum pane width enforced during resize
- **WHEN** the user drags the border such that either pane would be narrower than max(10% of terminal width, 10 columns)
- **THEN** the border stops at the minimum width boundary and does not move further

#### Scenario: Maximum tree pane width enforced during resize
- **WHEN** the user drags the border such that the tree pane would exceed 70% of the terminal width
- **THEN** the border stops at 70% and does not move further

#### Scenario: Resize ratio persists during session
- **WHEN** the user resizes the panes and continues using the application
- **THEN** the adjusted ratio is maintained across focus switches, file navigation, and other interactions

#### Scenario: Toggle focus mode with keyboard shortcut
- **WHEN** the user presses b
- **THEN** the tree pane is hidden and the viewer fills the full terminal width (focus mode)
- **AND** focus moves to the viewer pane

#### Scenario: Exit focus mode with keyboard shortcut
- **WHEN** focus mode is active and the user presses b
- **THEN** the tree pane reappears at its previous width
- **AND** the two-pane layout is restored

#### Scenario: Enter focus mode by dragging border below minimum
- **WHEN** the user drags the pane border to the left past the minimum pane width
- **THEN** the tree pane collapses and focus mode is activated
- **AND** the previous tree width is saved for later restoration

#### Scenario: Exit focus mode by dragging border outward
- **WHEN** focus mode is active and the user clicks and drags the left edge outward (to the right)
- **THEN** focus mode is deactivated and the tree pane reappears following the mouse position
- **AND** the pane width is clamped to normal min/max bounds

#### Scenario: Focus mode preserves previous tree width
- **WHEN** the user toggles focus mode off via keyboard shortcut
- **THEN** the tree pane restores to its width before focus mode was activated

### Requirement: Key hint bar at the bottom
The system SHALL display a key hint bar at the bottom of the screen showing available keybindings for the current context. The hint bar SHALL update based on which pane is focused.

#### Scenario: File tree is focused
- **WHEN** the file tree pane is focused
- **THEN** the hint bar shows relevant keys: j/k navigate, h/l fold/unfold, Enter open, Tab switch pane, q quit

#### Scenario: File viewer is focused
- **WHEN** the file viewer pane is focused
- **THEN** the hint bar shows relevant keys: j/k scroll, Ctrl-d/Ctrl-u page scroll, Tab switch pane, q quit

### Requirement: Pane focus switching
The system SHALL allow switching focus between the file tree and file viewer using the Tab key or by clicking inside a pane. The Tab key SHALL toggle focus between the two panes. The focused pane SHALL be visually indicated by a highlighted border (e.g., brighter or different color). Additionally, pressing Enter on a file in the tree SHALL switch focus to the viewer.

#### Scenario: Toggle focus with Tab
- **WHEN** the user presses Tab
- **THEN** focus moves to the other pane and its border is highlighted

#### Scenario: Enter on file switches to viewer
- **WHEN** the file tree is focused and the user presses Enter on a file entry
- **THEN** focus moves to the file viewer pane

#### Scenario: Click on tree pane switches focus to tree
- **WHEN** the file viewer is focused and the user clicks inside the tree pane
- **THEN** focus moves to the file tree pane

#### Scenario: Click on viewer pane switches focus to viewer
- **WHEN** the file tree is focused and the user clicks inside the viewer pane
- **THEN** focus moves to the file viewer pane

#### Scenario: Click on pane border does not change focus
- **WHEN** the user clicks on the pane border
- **THEN** focus does not change (the border click initiates resize)

### Requirement: Mouse capture enabled
The system SHALL enable mouse capture on startup and disable it on exit. Mouse events SHALL be processed in the event loop alongside keyboard events.

#### Scenario: Mouse events are captured
- **WHEN** the application starts
- **THEN** mouse events (click, drag, release) are captured by the application

#### Scenario: Mouse capture disabled on exit
- **WHEN** the application exits (normally or via panic hook)
- **THEN** mouse capture is disabled and the terminal is restored to its original state

### Requirement: Application quit
The system SHALL quit when the user presses q. The terminal SHALL be restored to its original state on exit, including on panic or unexpected errors.

#### Scenario: Quit the application
- **WHEN** the user presses q
- **THEN** the application exits and the terminal is restored to its original state

#### Scenario: Terminal restoration on crash
- **WHEN** the application panics or encounters an unrecoverable error
- **THEN** the terminal is still restored to its original state (via a panic hook)
