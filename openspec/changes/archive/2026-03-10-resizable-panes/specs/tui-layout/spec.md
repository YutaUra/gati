## MODIFIED Requirements

### Requirement: Two-pane layout
The system SHALL display a two-pane layout with the file tree on the left and the file viewer on the right. The default split SHALL be 30% tree / 70% viewer. The split ratio SHALL be adjustable by mouse-dragging the pane border. Each pane SHALL have a minimum width of max(10% of terminal width, 10 columns). The tree pane SHALL NOT exceed 70% of the terminal width.

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

## ADDED Requirements

### Requirement: Mouse capture enabled
The system SHALL enable mouse capture on startup and disable it on exit. Mouse events SHALL be processed in the event loop alongside keyboard events.

#### Scenario: Mouse events are captured
- **WHEN** the application starts
- **THEN** mouse events (click, drag, release) are captured by the application

#### Scenario: Mouse capture disabled on exit
- **WHEN** the application exits (normally or via panic hook)
- **THEN** mouse capture is disabled and the terminal is restored to its original state
