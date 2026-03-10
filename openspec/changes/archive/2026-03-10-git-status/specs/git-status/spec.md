## ADDED Requirements

### Requirement: Git repository detection

The system SHALL detect whether the target directory is inside a git repository on startup. If inside a git repository, the system SHALL compute the working tree status. If outside a git repository, the system SHALL function normally without git features.

#### Scenario: Application starts inside a git repository

- **WHEN** the application starts in a directory that is part of a git repository
- **THEN** git status is computed and available for display

#### Scenario: Application starts outside a git repository

- **WHEN** the application starts in a directory that is not part of any git repository
- **THEN** the application functions normally without git status data

### Requirement: Per-file git status computation

The system SHALL compute the git status of each file in the working tree relative to HEAD. The status SHALL distinguish between modified, added (staged new), deleted, renamed, and untracked files.

#### Scenario: Modified file detected

- **WHEN** a tracked file has changes in the working tree or index relative to HEAD
- **THEN** the file status is reported as Modified

#### Scenario: Staged new file detected

- **WHEN** a new file has been staged for commit
- **THEN** the file status is reported as Added

#### Scenario: Deleted file detected

- **WHEN** a tracked file has been removed from the working tree
- **THEN** the file status is reported as Deleted

#### Scenario: Renamed file detected

- **WHEN** a file has been renamed (detected by git)
- **THEN** the file status is reported as Renamed

#### Scenario: Untracked file detected

- **WHEN** a file exists in the working tree but is not tracked by git
- **THEN** the file status is reported as Untracked

#### Scenario: Unchanged file has no status

- **WHEN** a file has no changes relative to HEAD
- **THEN** no status is reported for that file
