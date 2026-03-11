## MODIFIED Requirements

### Requirement: Users can leave inline comments on file lines

The system SHALL allow users to create inline comments on any line (or range of lines) in the file viewer. Comments are displayed inline between code lines, similar to GitHub PR review comments. When a comment is being created or edited, the text input SHALL appear inline directly below the target line (or range end line) in the file viewer, rather than in a separate input area.

#### Scenario: Add a comment on a single line

- **WHEN** the file viewer is focused and the user presses `c` on a line
- **THEN** an inline text input widget appears directly below that line in the file viewer
- **AND** the widget uses a visually distinct style (e.g., cyan text on black background) with a visible text cursor
- **AND** the viewport scrolls if necessary to keep the input widget visible

#### Scenario: Add a comment on a line range

- **WHEN** the user visually selects a range of lines (e.g., via `V` for line select mode) and presses `c`
- **THEN** an inline text input widget appears directly below the last line of the selected range
- **AND** the comment is associated with the entire line range

#### Scenario: Display inline comments

- **WHEN** a file has comments attached to it
- **THEN** comments are rendered inline between code lines in a visually distinct block (e.g., bordered box with different background)
- **AND** the comment block shows the line range it applies to

#### Scenario: Edit an existing comment

- **WHEN** the cursor is on a line that has an existing comment and the user presses `c`
- **THEN** the inline text input widget appears below the line pre-filled with the existing comment text

#### Scenario: Delete a comment

- **WHEN** the cursor is on a line with a comment and the user presses a delete key (e.g., `dc` or a designated key combination)
- **THEN** the comment is removed

#### Scenario: Comments do not affect line numbering

- **WHEN** comments or the inline editor are displayed between code lines
- **THEN** the original file line numbers remain correct and are not shifted by comment blocks or the editor

#### Scenario: Scroll through file with comments

- **WHEN** scrolling through a file that has inline comments
- **THEN** comments scroll with the code lines they are attached to

### Requirement: Hint bar updates for comment mode

The system SHALL update the key hint bar to reflect comment-related keybindings.

#### Scenario: Viewer focused with comment capabilities

- **WHEN** the file viewer is focused
- **THEN** the hint bar includes `c comment` among the available keys

#### Scenario: Comment input active

- **WHEN** the user is typing a comment via the inline editor
- **THEN** the hint bar shows: `Editing comment on L{n}  Enter save  Esc cancel` where {n} is the target line number
