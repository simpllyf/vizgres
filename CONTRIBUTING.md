# Contributing to vizgres

## Development Setup

### Prerequisites

- [just](https://github.com/casey/just) (command runner)
- [mise](https://mise.jdx.dev/) (tool version manager) — or install manually:
  - Rust 1.93+
  - Docker and Docker Compose (for integration tests)

### Running Tests

All commands use `just` (see `justfile` for the full list):

```sh
just test              # Unit + doc tests
just lint              # Format check + clippy
just db-up             # Start test PostgreSQL
just test-integration  # Run integration tests
just db-down           # Stop test PostgreSQL
```

If using mise, ensure it's activated in your shell or prefix with `mise exec --`.

## Project Structure

```
src/
├── main.rs                # Entry point, terminal setup, event loop
├── app/                   # Application state (split module)
│   ├── mod.rs             # App struct, types, constructors, helpers
│   ├── event_handler.rs   # Event dispatch (key, query results, etc.)
│   ├── actions.rs         # Key action execution
│   ├── sql_utils.rs       # SQL analysis (destructive/write detection, meta-commands)
│   └── tests.rs           # Unit tests
├── keymap.rs              # Data-driven keybinding config
├── history.rs             # Query history ring buffer
├── connection_manager.rs  # Per-tab connection management with auto-reconnect
├── export.rs              # CSV/JSON export
├── error.rs               # Error hierarchy
├── config/                # Connection URL parsing, SSL, settings
├── commands/              # Command bar parsing
├── db/                    # Database layer (PostgreSQL)
└── ui/                    # TUI components
    ├── render.rs          # Top-level render orchestrator
    ├── tree.rs            # Schema tree browser
    ├── editor.rs          # Query editor
    ├── results.rs         # Results viewer
    ├── explain.rs         # EXPLAIN tree viewer
    ├── theme.rs           # Color themes
    └── ...
```

## Making Changes

1. Create a branch from `main`
2. Follow the conventions in [CONVENTIONS.md](CONVENTIONS.md)
3. Run `just lint` and `just test` before pushing
4. Keep changes focused — one logical change per PR

## Pull Requests

- Write clear commit messages using [conventional commits](https://www.conventionalcommits.org/) (lowercase)
- All commits must be signed — see [GitHub's guide on commit signing](https://docs.github.com/en/authentication/managing-commit-signature-verification/signing-commits)
- CI must pass before merge

## Releases

Releases are managed by the maintainer via `just release <version>`. See the justfile for details.

## Questions?

Open an issue for bugs or feature requests.
