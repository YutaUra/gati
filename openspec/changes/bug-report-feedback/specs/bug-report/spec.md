## ADDED Requirements

### Requirement: Pre-filled GitHub issue URL generation
The system SHALL generate a GitHub issue URL pre-filled with environment information (gati version, OS, architecture) and an optional error description. The URL SHALL target `https://github.com/YutaUra/gati/issues/new` with query parameters for title, body, and labels.

#### Scenario: URL contains environment info
- **WHEN** a bug report URL is generated
- **THEN** the URL body MUST include gati version, OS name, and CPU architecture

#### Scenario: URL is valid and under length limit
- **WHEN** a bug report URL is generated with a long panic message
- **THEN** the total URL length MUST NOT exceed 2000 characters, truncating the panic body if necessary

### Requirement: Panic hook prints bug report URL
The system SHALL print a pre-filled GitHub issue URL to stderr after a panic occurs, following terminal restoration. The panic location and message SHALL be included in the URL body.

#### Scenario: Panic triggers bug report URL output
- **WHEN** the application panics
- **THEN** the terminal MUST be restored AND a bug report URL MUST be printed to stderr

#### Scenario: Panic message is included in URL
- **WHEN** a panic occurs with the message "index out of bounds"
- **THEN** the generated URL body MUST contain "index out of bounds"

### Requirement: CLI bug-report flag
The system SHALL provide a `--bug-report` CLI flag that prints a pre-filled GitHub issue URL and exits without entering the TUI.

#### Scenario: Running with --bug-report flag
- **WHEN** the user runs `gati --bug-report`
- **THEN** the system MUST print a bug report URL to stdout and exit with code 0

#### Scenario: --bug-report does not start TUI
- **WHEN** the user runs `gati --bug-report`
- **THEN** the system MUST NOT enter alternate screen or enable raw mode

### Requirement: Browser open with fallback
The system SHALL attempt to open the bug report URL in the default browser. If opening fails, the system SHALL print the URL to stderr instead.

#### Scenario: Browser opens successfully
- **WHEN** the `open` crate successfully opens the URL
- **THEN** no URL MUST be printed to stderr

#### Scenario: Browser open fails
- **WHEN** the `open` crate fails to open the URL (e.g., headless environment)
- **THEN** the URL MUST be printed to stderr

### Requirement: Bug report entry in help dialog
The help dialog (`?`) SHALL include a "B report bug / feedback" entry. Pressing `B` (Shift+B) in Normal mode SHALL trigger bug report URL generation and open/print.

#### Scenario: Help dialog shows bug report entry
- **WHEN** the user opens the help dialog
- **THEN** the dialog MUST contain an entry for "B" with description "report bug / feedback"

#### Scenario: Pressing B opens bug report
- **WHEN** the user presses Shift+B in Normal mode
- **THEN** the system MUST generate a bug report URL and attempt to open it in the default browser
