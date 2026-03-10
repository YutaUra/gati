### Requirement: File tree supports file name search

The system SHALL provide an incremental search mode that filters the file tree to show only entries matching the search query. The search is activated by pressing `/` when the file tree is focused.

#### Scenario: Activate search mode

- **WHEN** the file tree is focused and the user presses `/`
- **THEN** a search input field appears (e.g., at the bottom of the tree pane or in the hint bar area)
- **AND** the user can type a search query

#### Scenario: Incremental filtering as user types

- **WHEN** the user types characters in the search input
- **THEN** the file tree incrementally filters to show only files (and their ancestor directories) whose names contain the query substring
- **AND** the matching portion of file names is visually highlighted

#### Scenario: Case-insensitive matching

- **WHEN** the user types a lowercase query
- **THEN** the search matches file names case-insensitively

#### Scenario: Navigate search results

- **WHEN** search results are displayed
- **THEN** the user can navigate among matching entries with j/k or Up/Down arrows

#### Scenario: Confirm search selection with Enter

- **WHEN** search results are displayed and the user presses Enter
- **THEN** the search mode exits, the selected file remains selected in the tree, and the viewer shows its contents

#### Scenario: Cancel search with Escape

- **WHEN** the user is in search mode and presses Escape
- **THEN** search mode exits, the filter is cleared, and the tree returns to its previous state (original selection restored)

#### Scenario: Empty query shows all files

- **WHEN** the search input is empty (user pressed `/` but hasn't typed anything yet)
- **THEN** all files are shown (no filtering applied)

#### Scenario: No matches found

- **WHEN** the user types a query that matches no files
- **THEN** the tree shows an empty state or a message such as "No matches"
- **AND** the search input remains active for the user to modify the query

#### Scenario: Search matches across nested directories

- **WHEN** the user searches for a file name that exists in a nested directory
- **THEN** the file appears in the results with its ancestor directories expanded to show the path

### Requirement: Search mode indicator

The system SHALL visually indicate when search mode is active.

#### Scenario: Search mode active

- **WHEN** the user is in search mode
- **THEN** the search input is visible and the tree title or border indicates search is active (e.g., "Files [/search]")

### Requirement: Hint bar updates for search

The system SHALL update the hint bar to show search-related keybindings when in search mode.

#### Scenario: Search mode hint bar

- **WHEN** the user is in search mode
- **THEN** the hint bar shows: `Enter confirm  Esc cancel  ↑/↓ navigate`
