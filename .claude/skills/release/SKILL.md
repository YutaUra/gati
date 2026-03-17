---
name: release
description: >
  Release a new version of gati. Use when the user says "release", "tag", "version bump",
  "cut a release", or wants to publish a new version. Handles git cleanliness check,
  version bump, changelog update, commit, tag, push, and post-release hash updates.
---

# Release Workflow

## Step 1: Ensure clean working tree

Run `git status`. If there are uncommitted changes:
1. Show the changes to the user
2. Ask if they should be committed before proceeding
3. If yes, commit them. If no, abort the release.

## Step 2: Determine version bump

Read the current version from `Cargo.toml`.
Gather commits since the last tag with `git log --oneline $(git describe --tags --abbrev=0)..HEAD`.

Use AskUserQuestion to ask which version bump to apply. Analyze the commits and recommend
the appropriate option:
- **Patch** (bug fixes, minor improvements, documentation) — recommend if all changes are fixes or small additions
- **Minor** (new features, non-breaking changes) — recommend if there are new user-facing features
- **Major** (breaking changes) — recommend if there are breaking API or behavioral changes

Mark the recommended option with "(Recommended)" in the label.

## Step 3: Update version in all locations

All four files must be updated to the new version:
- `Cargo.toml` — `version = "X.Y.Z"`
- `flake.nix` — `version = "X.Y.Z";`
- `nix/package.nix` — `version = "X.Y.Z";`
- `CHANGELOG.md` — add new section (see below)

Run `cargo check` after updating `Cargo.toml` to regenerate `Cargo.lock`.

For `nix/package.nix`, set `hash` to `""` (empty string) temporarily — it will be
updated in step 6 after the tag is published.

## Step 4: Update CHANGELOG.md

Add a new version section at the top (below the header), following the existing
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format:

```markdown
## [X.Y.Z] - YYYY-MM-DD

### Added
- ...

### Changed
- ...

### Fixed
- ...
```

Categorize commits into Added/Changed/Fixed/Removed sections. Omit empty sections.
Write entries from the user's perspective, not implementation details.

## Step 5: Commit, tag, and push

```
git add Cargo.toml Cargo.lock flake.nix nix/package.nix CHANGELOG.md
git commit -m "Bump version to X.Y.Z"
git tag vX.Y.Z
git push && git push origin vX.Y.Z
```

## Step 6: Wait for Release CI and update nix hash

Watch the release workflow with `gh run watch`. Once it succeeds:

1. Compute the new source hash:
   ```
   nix-prefetch-url --unpack "https://github.com/YutaUra/gati/archive/refs/tags/vX.Y.Z.tar.gz" 2>&1 | tail -1 | xargs nix hash convert --hash-algo sha256 --to sri
   ```
2. Update `nix/package.nix` with the real hash
3. Commit and push:
   ```
   git add nix/package.nix
   git commit -m "Update nix/package.nix hashes for vX.Y.Z"
   git push
   ```

## Step 7: Update nixpkgs PR (if open)

Check if there is an open nixpkgs PR for gati:

```
gh pr list --repo NixOS/nixpkgs --author YutaUra --state open --search "gati"
```

If found, update the PR to the new version:

1. Fetch and checkout the PR branch in the local nixpkgs clone (`~/work/github.com/yutaura/nixpkgs`):
   ```
   DIRENV_LOG_FORMAT="" git -C ~/work/github.com/yutaura/nixpkgs fetch origin <branch> --depth=1
   DIRENV_LOG_FORMAT="" git -C ~/work/github.com/yutaura/nixpkgs checkout <branch>
   ```
   If the branch doesn't exist locally, create it from FETCH_HEAD:
   ```
   DIRENV_LOG_FORMAT="" git -C ~/work/github.com/yutaura/nixpkgs checkout -b <branch> FETCH_HEAD
   ```

2. Update `pkgs/by-name/ga/gati/package.nix` with the new version and hashes
   (use the same source hash and cargoHash from step 6).

3. Commit and push:
   ```
   DIRENV_LOG_FORMAT="" git -C ~/work/github.com/yutaura/nixpkgs add pkgs/by-name/ga/gati/package.nix
   DIRENV_LOG_FORMAT="" git -C ~/work/github.com/yutaura/nixpkgs commit -m "gati: X.Y.Z -> A.B.C"
   DIRENV_LOG_FORMAT="" git -C ~/work/github.com/yutaura/nixpkgs push origin <branch>
   ```

4. Update the PR title if needed:
   ```
   gh pr edit <PR_NUMBER> --repo NixOS/nixpkgs --title "gati: init at A.B.C"
   ```
