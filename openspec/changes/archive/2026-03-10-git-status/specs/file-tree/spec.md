## MODIFIED Requirements

### Requirement: File tree displays directory contents

The system SHALL display the contents of the target directory as a tree structure with indentation representing depth. Directories SHALL be visually distinguishable from files (e.g., directory icon/indicator vs file icon/indicator). When inside a git repository, each file entry SHALL display a git status marker next to its name indicating its change status. Directories SHALL display a change indicator when any descendant has changes.

#### Scenario: Launch with default directory

- **WHEN** the user runs `gati` without arguments
- **THEN** the file tree displays the contents of the current working directory
- **AND** the first entry is selected by default

#### Scenario: Launch with specified directory

- **WHEN** the user runs `gati src/`
- **THEN** the file tree displays the contents of the `src/` directory

#### Scenario: Launch with a file path

- **WHEN** the user runs `gati src/main.rs`
- **THEN** the file tree displays the contents of `src/` (the parent directory)
- **AND** the file `main.rs` is selected in the tree
- **AND** the file viewer displays the contents of `main.rs`

#### Scenario: Launch with non-existent path

- **WHEN** the user runs `gati /nonexistent`
- **THEN** the application exits with a descriptive error message indicating the path does not exist

#### Scenario: Launch with a path the user cannot read

- **WHEN** the user runs `gati` with a path that exists but is not readable
- **THEN** the application exits with an error message indicating insufficient permissions

#### Scenario: Modified file shows marker

- **WHEN** a file has been modified relative to HEAD
- **THEN** the file tree displays `[M]` next to the file name in yellow

#### Scenario: Added file shows marker

- **WHEN** a new file has been staged for commit
- **THEN** the file tree displays `[A]` next to the file name in green

#### Scenario: Untracked file shows marker

- **WHEN** a file is untracked
- **THEN** the file tree displays `[?]` next to the file name in green

#### Scenario: Deleted file shows marker

- **WHEN** a tracked file has been deleted from the working tree
- **THEN** the file tree displays `[D]` next to the file name in red

#### Scenario: Renamed file shows marker

- **WHEN** a file has been renamed
- **THEN** the file tree displays `[R]` next to the file name in blue

#### Scenario: Unchanged file has no marker

- **WHEN** a file has no changes relative to HEAD
- **THEN** no status marker is displayed next to the file name

#### Scenario: Directory indicates changed descendants

- **WHEN** a directory contains one or more files with git status changes
- **THEN** the directory entry displays `[●]` in yellow to indicate it contains changes

#### Scenario: Non-git directory shows no markers

- **WHEN** the target directory is not inside a git repository
- **THEN** no status markers are displayed and the file tree renders normally
