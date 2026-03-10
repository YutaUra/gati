## MODIFIED Requirements

### Requirement: File viewer scrolling
The file viewer SHALL support mouse wheel scrolling in addition to keyboard scrolling. Scrolling SHALL only occur when the mouse cursor is over the viewer pane.

#### Scenario: Mouse wheel scroll down
- **WHEN** the mouse cursor is over the file viewer pane and the user scrolls the mouse wheel down
- **THEN** the viewer content scrolls down by 5 lines

#### Scenario: Mouse wheel scroll up
- **WHEN** the mouse cursor is over the file viewer pane and the user scrolls the mouse wheel up
- **THEN** the viewer content scrolls up by 5 lines

#### Scenario: Mouse wheel over tree pane scrolls tree
- **WHEN** the mouse cursor is over the file tree pane and the user scrolls the mouse wheel
- **THEN** the tree viewport scrolls (not the viewer)

## ADDED Requirements

_(none)_
