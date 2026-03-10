## MODIFIED Requirements

### Requirement: File viewer horizontal scrolling
The file viewer SHALL support horizontal scrolling to view content that extends beyond the viewport width.

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

## ADDED Requirements

_(none)_
