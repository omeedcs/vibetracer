# Contributing to vibetracer

Thanks for your interest in contributing. Here's how to get started.

## Development Setup

```bash
# Clone the repo
git clone https://github.com/omeedcs/vibetracer.git
cd vibetracer

# Build
cargo build

# Run tests
cargo test

# Run with live watching
cargo run -- /path/to/your/project

# Run with embedded terminal
cargo run -- --embed /path/to/your/project
```

**Requirements:**
- Rust 1.85+ (edition 2024)
- macOS or Linux

## How to Contribute

### Bug Reports

Open an issue with:
- What you expected to happen
- What actually happened
- Steps to reproduce
- Your OS and terminal emulator

### Feature Requests

Open an issue describing the feature and why it would be useful. Vibetracer is opinionated — not every feature fits, but good ideas are always welcome.

### Pull Requests

1. Fork the repo and create a branch from `main`
2. Write tests for any new functionality
3. Run `cargo test`, `cargo clippy`, and `cargo fmt` before submitting
4. Keep PRs focused — one feature or fix per PR
5. Write a clear description of what changed and why

### Code Style

- Follow existing patterns in the codebase
- No emojis in code, comments, or UI
- Use the muted color palette for any TUI changes (see `src/tui/widgets/` for reference)
- Keep files focused — one responsibility per module
- Write tests. TDD is encouraged.

### Architecture

```
src/
  main.rs          # CLI entry point
  config.rs        # TOML config parsing
  event.rs         # Edit event types
  session.rs       # Session management
  splash.rs        # Launch animation
  snapshot/        # Content-addressed file storage
  watcher/         # Filesystem watching + diffing
  hook/            # Claude Code hook integration
  rewind/          # File restoration
  import/          # Claude Code session import
  pty/             # Embedded terminal (PTY)
  analysis/        # Blast radius, sentinels, watchdog, refactor tracker, schema diff
  equation/        # LaTeX detection + rendering
  tui/             # ratatui-based UI
    widgets/       # Individual UI components
```

### Testing

```bash
# Run all tests
cargo test

# Run a specific test
cargo test test_name

# Run with output
cargo test -- --nocapture
```

Integration tests are in `tests/integration/`. Unit tests are inline in their modules.

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
