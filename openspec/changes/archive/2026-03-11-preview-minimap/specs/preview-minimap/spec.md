## ADDED Requirements

### Requirement: Minimap displays file overview with viewport indicator
The file viewer SHALL display a minimap column on its right edge, exactly 2 terminal columns wide. The minimap SHALL show a viewport indicator highlighting which portion of the file is currently visible. The minimap height SHALL equal the viewer's inner height (excluding borders). Each minimap row SHALL represent a proportional segment of the file. When the file has fewer lines than the minimap height, each row maps to at most one line with empty rows below.

#### Scenario: Minimap visible when file is loaded
- **WHEN** a file is loaded in the file viewer
- **THEN** a 2-column minimap appears on the right edge of the viewer pane
- **AND** the viewport indicator highlights the rows corresponding to the currently visible lines

#### Scenario: Viewport indicator updates on scroll
- **WHEN** the user scrolls the file viewer (by any method: keyboard, mouse wheel)
- **THEN** the viewport indicator position in the minimap updates to reflect the new visible range

#### Scenario: Short file minimap
- **WHEN** a file has fewer lines than the minimap height
- **THEN** the minimap shows markers only for the rows that correspond to actual file lines
- **AND** remaining rows below are empty (background only)

#### Scenario: Minimap hidden on narrow viewer
- **WHEN** the file viewer's inner width is less than 30 columns
- **THEN** the minimap SHALL NOT be displayed to preserve content readability

### Requirement: Minimap shows diff markers
The minimap SHALL display colored markers at positions corresponding to changed lines in the file. Added lines SHALL be marked with green, modified lines with yellow, and removed lines with red. Markers SHALL be shown in both normal mode (using line diff data) and diff mode (using unified diff data).

#### Scenario: File with added lines
- **WHEN** a file has lines that were added (compared to the git index)
- **THEN** the minimap rows corresponding to those lines display a green marker

#### Scenario: File with modified lines
- **WHEN** a file has lines that were modified
- **THEN** the minimap rows corresponding to those lines display a yellow marker

#### Scenario: Diff mode markers
- **WHEN** the viewer is in diff mode
- **THEN** the minimap shows green markers for added lines and red markers for removed lines at their respective positions in the unified diff

#### Scenario: Unmodified file
- **WHEN** a file has no changes
- **THEN** the minimap displays no diff markers (only the viewport indicator)

### Requirement: Minimap shows comment indicators
The minimap SHALL display cyan markers at positions corresponding to lines that have inline comments.

#### Scenario: File with comments
- **WHEN** a file has inline comments on specific lines
- **THEN** the minimap rows corresponding to those lines display a cyan marker

#### Scenario: Comment added during session
- **WHEN** the user adds a new inline comment
- **THEN** the minimap immediately shows a cyan marker at the corresponding position

### Requirement: Minimap supports click-to-scroll
The minimap SHALL respond to mouse clicks by scrolling the file viewer to the corresponding file position. Clicking a minimap row SHALL center the corresponding file line in the viewport.

#### Scenario: Click on minimap row
- **WHEN** the user clicks on a row in the minimap
- **THEN** the file viewer scrolls so that the corresponding file line is centered in the viewport

#### Scenario: Click on minimap in diff mode
- **WHEN** the viewer is in diff mode and the user clicks on a minimap row
- **THEN** the diff view scrolls to the corresponding position in the unified diff

### Requirement: Minimap works in placeholder states
The minimap SHALL NOT be displayed when the file viewer is in a non-file state (placeholder, binary, empty, or error).

#### Scenario: No file selected
- **WHEN** no file is loaded (placeholder message displayed)
- **THEN** the minimap is not shown

#### Scenario: Binary file selected
- **WHEN** a binary file is selected
- **THEN** the minimap is not shown
