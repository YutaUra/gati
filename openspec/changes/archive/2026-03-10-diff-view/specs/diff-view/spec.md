### Requirement: Full file mode shows change gutter markers

The system SHALL display gutter markers in normal (full file) mode to indicate which lines have been changed relative to HEAD. This allows the user to see changes in context without switching to diff mode.

#### Scenario: Changed lines show gutter indicator

- **WHEN** viewing a file in normal mode that has modified lines relative to HEAD
- **THEN** changed lines display a gutter indicator (e.g., `▎` in yellow) in the line number gutter

#### Scenario: Added lines show gutter indicator

- **WHEN** viewing a file in normal mode that has newly added lines relative to HEAD
- **THEN** added lines display a gutter indicator (e.g., `▎` in green) in the line number gutter

#### Scenario: No gutter markers outside git

- **WHEN** the application is running outside a git repository
- **THEN** no gutter markers are displayed in normal mode

### Requirement: File viewer supports diff mode

The system SHALL provide a diff mode in the file viewer that shows a unified diff of the selected file's changes. The diff compares the working tree version against HEAD. The user SHALL be able to toggle between normal (full file) mode and diff mode.

#### Scenario: Toggle diff mode with d key

- **WHEN** the file viewer is focused and the user presses `d`
- **THEN** the viewer switches to unified diff mode showing the file's changes relative to HEAD
- **AND** pressing `d` again returns to normal (full file) mode

#### Scenario: Diff mode shows added lines

- **WHEN** viewing a file in diff mode that has new lines added
- **THEN** added lines are displayed with a `+` prefix and highlighted in green

#### Scenario: Diff mode shows removed lines

- **WHEN** viewing a file in diff mode that has lines removed
- **THEN** removed lines are displayed with a `-` prefix and highlighted in red

#### Scenario: Diff mode shows context lines

- **WHEN** viewing a file in diff mode
- **THEN** unchanged context lines surrounding changes are displayed (3 lines of context by default)
- **AND** context lines have no prefix highlight

#### Scenario: Diff mode shows hunk headers

- **WHEN** viewing a file in diff mode with multiple change hunks
- **THEN** each hunk is preceded by a header line showing the line range (e.g., `@@ -10,5 +10,7 @@`)
- **AND** the hunk header is displayed in a distinct style (e.g., cyan)

#### Scenario: File with no changes shows message in diff mode

- **WHEN** the user toggles to diff mode on a file that has no changes relative to HEAD
- **THEN** the viewer displays a message such as "No changes"

#### Scenario: Untracked file in diff mode

- **WHEN** the user toggles to diff mode on an untracked file (not yet committed)
- **THEN** the entire file is displayed as added lines (all lines prefixed with `+`)

#### Scenario: Diff mode is not available outside git

- **WHEN** the application is running outside a git repository and the user presses `d`
- **THEN** nothing happens (diff mode is unavailable) or a message indicates git is not available

#### Scenario: Diff mode scrolling

- **WHEN** the viewer is in diff mode
- **THEN** all existing scrolling keybindings (j/k, Ctrl-d/Ctrl-u) work within the diff content

### Requirement: Diff mode indicator in viewer title

The system SHALL indicate the current viewing mode in the viewer pane's title bar.

#### Scenario: Normal mode title

- **WHEN** the viewer is in normal (full file) mode
- **THEN** the pane title displays "Preview"

#### Scenario: Diff mode title

- **WHEN** the viewer is in diff mode
- **THEN** the pane title displays "Diff" or "Diff: <filename>"

### Requirement: Hint bar updates for diff mode

The system SHALL update the key hint bar to reflect diff-related keybindings when relevant.

#### Scenario: Viewer focused with diff available

- **WHEN** the file viewer is focused and inside a git repository
- **THEN** the hint bar includes `d diff` among the available keys

#### Scenario: Viewer focused outside git

- **WHEN** the file viewer is focused and outside a git repository
- **THEN** the hint bar does not show `d diff`
