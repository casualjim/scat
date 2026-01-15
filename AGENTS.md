# Agent Coding Guidelines

> **Important:** Prefer the `mise` tasks for installs, builds, tests, and formatting. Only use raw toolchain commands when no `mise` wrapper exists, and call that out explicitly.
>
> **CRITICAL: Use the Rust LSP for all code navigation!** Do NOT use grep/find/rg or manual file browsing - the Rust LSP provides accurate, fast, type-aware navigation. See the "Code Navigation (Use Rust LSP!)" section for detailed commands.


## Build, Test, and Development Commands
Always default to the `mise` tasks below; only run direct toolchain commands if no `mise` wrapper exists and note the deviation.

**For code navigation and understanding, use the Rust LSP!** See the "Code Navigation (Use Rust LSP!)" section above for detailed commands.

- `mise install`: Install pinned Rust, Bun, Wrangler, etc.
- `mise build:debug`: Build Rust 
- `mise test`: All tests (Rust nextest + Workers via bun test).

## Release Process (PR-Driven Bumps)
Releases are automated after CI passes on `main`. The bump level is controlled by tokens in the PR title/body (commit message fallback).

- `bump:major`
- `bump:minor`
- `bump:patch`

If no token is present, the release defaults to `bump:patch`. If multiple tokens are present, the highest wins (major > minor > patch).

When using `gh` to create a PR, the agent must include the bump token in the PR body.

## Code Navigation (Use Rust LSP!)

**IMPORTANT: Always use the Rust LSP for code navigation!** The Rust LSP should be your primary tool for:
- Finding symbols and definitions
- Navigating to references
- Getting function signatures and documentation
- Understanding code structure
- Finding implementations and usages

**Prefer Rust LSP over:** grep/find/rg, manual file browsing, or any other navigation method!

### Rust LSP Commands Available

Use these `mcp__rust-lsp__*` tools for navigation:

```bash
# Get file structure and symbols
mcp__rust-lsp__outline <file_path>

# Search for symbols across the codebase
mcp__rust-lsp__search <query>

# Find all references to a symbol
mcp__rust-lsp__references <file_path> <line> <character>

# Get detailed info about a symbol at cursor position
mcp__rust-lsp__inspect <file_path> <line> <character>

# Get code completions at a position
mcp__rust-lsp__completion <file_path> <line> <character>

# Rename a symbol across the codebase
mcp__rust-lsp__rename <file_path> <line> <character> <new_name>

# Get diagnostics (errors/warnings) for a file
mcp__rust-lsp__diagnostics <file_path>
```

### Navigation Examples

```bash
# Find all search-related services
mcp__rust-lsp__search "SearchService"

# Explore the main application structure
mcp__rust-lsp__outline "crates/slipstreamd/src/lib.rs"

# Find all references to AppState
mcp__rust-lsp__references "crates/slipstreamd/src/app.rs" 16 1

# Inspect a function to get its documentation
mcp__rust-lsp__inspect "crates/embedding/src/lib.rs" 127 1

# Get completions for method calls
mcp__rust-lsp__completion "crates/slipstreamd/src/routes.rs" 42 20
```

### Why Use Rust LSP?

- **Accurate**: Understands Rust's type system and module resolution
- **Fast**: Instant navigation without scanning files
- **Context-aware**: Knows about imports, traits, generics
- **Complete**: Shows parameters, return types, documentation
- **IDE-quality**: Same experience as modern IDEs

**Remember: When you need to understand or navigate code, reach for the Rust LSP first!**

## Code Style & Formatting
- Rust:
  - Use `eyre::Result` for error handling, `thiserror` for domain errors
  - No `unwrap()` or `expect()` in public APIs
  - Async streaming first - avoid `collect()` patterns
  - Imports: Group std/core, external crates, and internal modules separately
  - Formatting: run `mise format`; never invoke `cargo fmt` directly
  - Strict error handling - fail spectacularly, don't swallow errors
- TypeScript:
  - Strict mode with no `any` or `unknown`
  - Bun package manager
  - Double quotes for strings
- General:
  - 2-space indentation (except Python which uses 4)
  - LF line endings with final newline
  - Trim trailing whitespace
  - UTF-8 encoding

## Naming Conventions
- Rust: snake_case for variables/functions, PascalCase for types
- TypeScript: camelCase for variables/functions, PascalCase for types
- Files: snake_case for Rust, camelCase for TypeScript

## Error Handling
- Rust: Use `eyre::Result` for function returns, `thiserror` for domain-specific errors
- TypeScript: Proper error catching and handling without swallowing
- Never ignore errors - propagate or handle explicitly

## Commit Messages
- Commit messages follow Conventional Commits (examples: `feat: ...`, `fix: ...`, `chore: ...`, `refactor: ...`, `test: ...`, `docs: ...`).
- Keep the first line a clear summary (50-72 chars recommended).
- Use the body for detailed explanation if needed.

Good examples:
- "feat: add automatic theme detection"
- "fix: handle binary stdin safely"
- "chore: update dependencies"

Bad examples:
- "Update stuff"
- "WIP"
- "feat add thing"
