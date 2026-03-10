### Requirement: File tree supports changed-files-only filter

The system SHALL allow the user to toggle the file tree between "all files" mode and "changed files only" mode. In changed-files-only mode, only files with git status changes (and their parent directories) are shown.

#### Scenario: Toggle changed-files filter

- **WHEN** the file tree is focused and the user presses `g`
- **THEN** the file tree switches to showing only changed files and their ancestor directories
- **AND** pressing `g` again returns to showing all files

#### Scenario: Changed-files mode shows only modified, added, deleted, and renamed files

- **WHEN** the tree is in changed-files-only mode
- **THEN** only files with a git status marker (`[M]`, `[A]`, `[D]`, `[R]`, `[?]`) are displayed
- **AND** parent directories of changed files are shown to preserve tree structure

#### Scenario: Changed-files mode with no changes

- **WHEN** the user toggles to changed-files-only mode but no files have changes
- **THEN** the tree displays a message such as "No changed files" or shows an empty tree

#### Scenario: Selection preserved across filter toggle

- **WHEN** the user toggles the filter and the currently selected file is still visible
- **THEN** the selection remains on that file

#### Scenario: Selection reset when current file is hidden

- **WHEN** the user toggles to changed-files-only mode and the currently selected file has no changes
- **THEN** the selection moves to the first visible entry

#### Scenario: Filter unavailable outside git

- **WHEN** the application is running outside a git repository and the user presses the filter key
- **THEN** nothing happens (filter is unavailable)

### Requirement: Filter indicator in tree title

The system SHALL indicate the active filter mode in the file tree pane's title.

#### Scenario: All files mode

- **WHEN** the tree is showing all files
- **THEN** the pane title displays "Files" (or the current default)

#### Scenario: Changed files mode

- **WHEN** the tree is showing only changed files
- **THEN** the pane title displays "Changed Files" or includes a filter indicator

### Requirement: Hint bar updates for filter

The system SHALL update the key hint bar to include the filter toggle keybinding when inside a git repository.

#### Scenario: Tree focused inside git repository

- **WHEN** the file tree is focused and inside a git repository
- **THEN** the hint bar includes `g changed` (or similar) among the available keys
