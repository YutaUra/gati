## MODIFIED Requirements

### Requirement: Two-pane layout
The system SHALL display a two-pane layout with the file tree on the left and the file viewer on the right. The default split SHALL be 30% tree / 70% viewer. The split ratio SHALL be adjustable by mouse-dragging the pane border. Each pane SHALL have a minimum width of max(10% of terminal width, 10 columns). The tree pane SHALL NOT exceed 70% of the terminal width. The system SHALL support a focus mode where the tree pane is hidden and the viewer occupies the full width.

#### Scenario: Toggle focus mode with keyboard shortcut
- **WHEN** the user presses Ctrl+Shift+B
- **THEN** the tree pane is hidden and the viewer fills the full terminal width (focus mode)
- **AND** focus moves to the viewer pane

#### Scenario: Exit focus mode with keyboard shortcut
- **WHEN** focus mode is active and the user presses Ctrl+Shift+B
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

## ADDED Requirements

_(none)_
