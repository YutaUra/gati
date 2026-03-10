### Requirement: File tree displays git status markers

The system SHALL display git status markers next to each file entry in the file tree when inside a git repository. Status markers indicate how each file has changed in the working tree or index relative to HEAD. Directories SHALL display a generic change indicator (e.g., `[●]`) when any descendant has changes, rather than aggregating specific status types.

#### Scenario: Modified file shows marker

- **WHEN** a file has been modified (unstaged or staged changes) relative to HEAD
- **THEN** the file tree displays `[M]` next to the file name in a distinct color (e.g., yellow)

#### Scenario: Added (untracked) file shows marker

- **WHEN** a file is untracked (new file not yet committed)
- **THEN** the file tree displays `[?]` next to the file name in a distinct color (e.g., green)

#### Scenario: Staged new file shows marker

- **WHEN** a file has been staged as a new addition (`git add` on an untracked file)
- **THEN** the file tree displays `[A]` next to the file name in a distinct color (e.g., green)

#### Scenario: Deleted file shows marker

- **WHEN** a file has been deleted (tracked file removed from working tree)
- **THEN** the file tree displays `[D]` next to the file name in a distinct color (e.g., red)

#### Scenario: Renamed file shows marker

- **WHEN** a file has been renamed (detected by git)
- **THEN** the file tree displays `[R]` next to the file name in a distinct color (e.g., blue)

#### Scenario: File with both staged and unstaged changes

- **WHEN** a file has been partially staged (some changes staged, others not)
- **THEN** the file tree displays `[M]` (the working tree status takes visual priority)

#### Scenario: Unchanged file has no marker

- **WHEN** a file has no changes relative to HEAD
- **THEN** no status marker is displayed next to the file name

#### Scenario: Directory indicates changed descendants

- **WHEN** a directory contains one or more files with git status changes
- **THEN** the directory entry displays `[●]` in a distinct color to indicate it contains changes

#### Scenario: Non-git directory shows no markers

- **WHEN** the target directory is not inside a git repository
- **THEN** no status markers are displayed and the file tree renders normally without git information

### Requirement: Git status is computed on startup and file load

The system SHALL compute git status once when the application starts (or when the target directory changes). The status data SHALL be used to annotate the file tree without blocking the UI.

#### Scenario: Application starts inside a git repository

- **WHEN** the application starts in a directory that is part of a git repository
- **THEN** git status is computed and markers appear in the file tree

#### Scenario: Application starts outside a git repository

- **WHEN** the application starts in a directory that is not part of any git repository
- **THEN** the application functions normally without git status markers
