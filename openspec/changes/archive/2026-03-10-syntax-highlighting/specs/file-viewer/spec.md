## MODIFIED Requirements

### Requirement: File viewer displays selected file contents
The system SHALL display the full contents of the selected file in the right pane with syntax highlighting and line numbers. Each line SHALL be prefixed with its line number, right-aligned. The line number gutter width SHALL adjust based on the total number of lines in the file. File contents SHALL be syntax-highlighted based on the file's language, detected from file extension or first-line content (e.g., shebang).

#### Scenario: Select a file in the tree
- **WHEN** the user selects a file in the file tree
- **THEN** the file viewer displays the file's contents with line numbers and syntax highlighting appropriate for the file's language

#### Scenario: No file selected
- **WHEN** no file has been selected yet (e.g., application just started and cursor is on a directory)
- **THEN** the file viewer displays a placeholder message (e.g., "Select a file to preview")

#### Scenario: File with recognized language
- **WHEN** the user selects a file with a known extension (e.g., `.rs`, `.py`, `.js`, `.md`)
- **THEN** the file viewer renders the contents with language-specific syntax highlighting

#### Scenario: File with unrecognized language
- **WHEN** the user selects a file with an unknown extension or no extension
- **THEN** the file viewer renders the contents as plain text without highlighting

#### Scenario: File with shebang line
- **WHEN** the user selects a file with no recognizable extension but a shebang line (e.g., `#!/bin/bash`)
- **THEN** the file viewer detects the language from the first line and applies syntax highlighting
