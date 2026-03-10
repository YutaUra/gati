### Requirement: Users can leave inline comments on file lines

The system SHALL allow users to create inline comments on any line (or range of lines) in the file viewer. Comments are displayed inline between code lines, similar to GitHub PR review comments.

#### Scenario: Add a comment on a single line

- **WHEN** the file viewer is focused and the user presses `c` on a line
- **THEN** a text input area opens below the line for the user to type a comment
- **AND** pressing Enter (or a designated confirm key) saves the comment

#### Scenario: Add a comment on a line range

- **WHEN** the user visually selects a range of lines (e.g., via `V` for line select mode) and presses `c`
- **THEN** a text input area opens below the selected range for the user to type a comment
- **AND** the comment is associated with the entire line range

#### Scenario: Display inline comments

- **WHEN** a file has comments attached to it
- **THEN** comments are rendered inline between code lines in a visually distinct block (e.g., bordered box with different background)
- **AND** the comment block shows the line range it applies to

#### Scenario: Edit an existing comment

- **WHEN** the cursor is on a line that has an existing comment and the user presses `c`
- **THEN** the existing comment opens for editing

#### Scenario: Delete a comment

- **WHEN** the cursor is on a line with a comment and the user presses a delete key (e.g., `dc` or a designated key combination)
- **THEN** the comment is removed

#### Scenario: Comments do not affect line numbering

- **WHEN** comments are displayed inline between code lines
- **THEN** the original file line numbers remain correct and are not shifted by comment blocks

#### Scenario: Scroll through file with comments

- **WHEN** scrolling through a file that has inline comments
- **THEN** comments scroll with the code lines they are attached to

### Requirement: Comments can be exported as plain text

The system SHALL support exporting all comments as structured plain text for sharing with AI tools, GitHub issues, or other communication channels.

#### Scenario: Export all comments

- **WHEN** the user triggers the export command (e.g., `:export` or a keybinding)
- **THEN** all comments are exported to a structured plain text format grouped by file

#### Scenario: Export format

- **WHEN** comments are exported
- **THEN** the output follows this format:
  ```
  ## <file-path>

  L<line>: <comment text>

  L<start>-<end>: <comment text>
  ```

#### Scenario: Export to clipboard

- **WHEN** the user exports comments
- **THEN** the exported text is copied to the system clipboard
- **AND** a confirmation message is displayed

#### Scenario: Export with no comments

- **WHEN** the user triggers export but no comments exist
- **THEN** a message indicates there are no comments to export

### Requirement: Comments persist during session

The system SHALL maintain all comments in memory for the duration of the session. Comments are associated with file paths and line numbers.

#### Scenario: Navigate away and back

- **WHEN** the user navigates to a different file and then returns to a file with comments
- **THEN** the comments are still displayed

#### Scenario: Comments are lost on exit

- **WHEN** the user quits the application
- **THEN** all comments are discarded (no persistent storage in initial implementation)

### Requirement: Hint bar updates for comment mode

The system SHALL update the key hint bar to reflect comment-related keybindings.

#### Scenario: Viewer focused with comment capabilities

- **WHEN** the file viewer is focused
- **THEN** the hint bar includes `c comment` among the available keys

#### Scenario: Comment input active

- **WHEN** the user is typing a comment
- **THEN** the hint bar shows: `Enter save  Esc cancel`
