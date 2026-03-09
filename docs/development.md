# Development Environment

## Prerequisites

- [Nix](https://nixos.org/) with flakes enabled
- [direnv](https://direnv.net/) with shell hook configured

## Setup

```bash
git clone https://github.com/yutaura/gati.git
cd gati
direnv allow
npm install
```

`direnv allow` activates the Nix dev shell automatically when you enter the directory. This provides:

| Tool | Purpose |
|------|---------|
| Rust (stable) + rust-analyzer | Application development |
| Node.js | OpenSpec CLI runtime |
| pkg-config, cmake | Native dependency build tools |

## Available Commands

```bash
# Rust
rustc --version
cargo build

# OpenSpec (spec-driven development)
npx openspec --version
npx openspec init
npx openspec status
```

## Project Structure

```
gati/
├── docs/
│   ├── development.md          # This file
│   └── plans/                  # Design documents
├── flake.nix                   # Nix dev environment
├── flake.lock                  # Nix dependency lock
├── package.json                # Node.js dependencies (OpenSpec)
├── package-lock.json           # Node.js dependency lock
└── .envrc                      # direnv configuration
```
