# Development Guide

## Overview

`umber` is a modern replacement for `cat` with syntax highlighting and automatic language detection. This document covers local development and the automated release process.

## Requirements

- Rust stable toolchain
- Git
- `mise` (recommended for installs, builds, tests, and formatting)

## Build and Run

- Install toolchains: `mise install`
- Build (debug): `mise build:debug`
- Run locally: `cargo run -- <args>`

Example:

```sh
cargo run -- -h
```

## Formatting, Linting, and Tests

- Format + lint: `mise format`
- Test: `mise test`

## Release Process

Releases are fully automated after CI passes on `main`. The release bump level is controlled by tokens in the PR title/body.

### Bump Tokens

Add one of the following to the PR title or body:

- `bump:major`
- `bump:minor`
- `bump:patch`

If no token is present, the release defaults to `bump:patch`. If multiple tokens are present, the highest wins (major > minor > patch).

### What Happens in CI

1. The `CI` workflow runs on `main`.
2. The `Cut Release` workflow runs after `CI` succeeds.
3. `Cut Release` determines the bump level from the PR (or commit message fallback).
4. It computes the next version from `Cargo.toml`, updates `CHANGELOG.md` via `git-cliff`, and runs `cargo-release` to create the release tag.
5. The tag push triggers the `Release` workflow, which builds and publishes artifacts with `cargo-dist`.

### Manual Release

You can also run the `Cut Release` workflow via `workflow_dispatch` in GitHub Actions. The bump token rules still apply (PR title/body or commit message fallback).
