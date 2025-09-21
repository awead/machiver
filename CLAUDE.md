# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Machiver is a Rust CLI tool for archiving files, primarily designed to organize files into BagIt bag structures based on their dates. The tool extracts dates from EXIF metadata when available, or falls back to file modification times.

## Core Architecture

The project is structured as a Rust binary crate with four main modules:

- **main.rs**: CLI argument parsing using `clap` with subcommands for `date` and `copy`
- **date.rs**: Date extraction logic that tries EXIF data first, then falls back to file modification time
- **copy.rs**: File copying and organization logic with async processing and UUID renaming
- **manifest.rs**: BagIt manifest parsing and duplicate detection with support for multiple hash algorithms

Key architectural decisions:
- Uses async/await with `tokio` for file operations
- Supports both EXIF date extraction (`kamadak-exif`) and filesystem metadata
- Implements duplicate detection via multiple hash algorithms (MD5, SHA-256, SHA-512) against BagIt manifests
- Algorithm detection based on manifest filename (e.g., `manifest-sha256.txt`)
- Creates ISO8601 date-based directory structure (YYYY/MM/DD)
- Clean separation of concerns with dedicated manifest module

## Development Commands

### Building and Testing
```bash
# Build the project
cargo build

# Build for release
cargo build --release

# Run tests
cargo test

# Run tests with verbose output
cargo test --verbose

# Run specific test
cargo test test_name

# Install locally for development
cargo install --path .
```

### Linting and Code Quality
```bash
# Run clippy linter (strict mode matching CI)
cargo clippy -- -D warnings

# Format code
cargo fmt

# Check formatting without modifying files
cargo fmt --check
```

### Cross-compilation for aarch64 (Synology NAS)
```bash
# Add target
rustup target add aarch64-unknown-linux-gnu

# Build for target (requires cross-compiler setup)
cargo build --release --target aarch64-unknown-linux-gnu
```

## Testing

The test suite uses:
- `tokio::test` for async tests
- `tempfile` crate for temporary directories in tests
- Test fixtures in `fixtures/` directory including sample images and BagIt bags

Tests cover:
- EXIF date extraction
- File modification date fallback
- Duplicate detection with multiple hash algorithms (MD5, SHA-256, SHA-512)
- BagIt manifest parsing with algorithm detection
- Hash algorithm detection from manifest filenames
- Recursive directory processing
- UUID filename generation
- Unsupported algorithm warning behavior

## CI/CD

GitHub Actions workflow (`.github/workflows/ci.yml`) runs:
- `cargo test --verbose` on Ubuntu
- `cargo clippy -- -D warnings` for strict linting

## Key Dependencies

- **tokio**: Async runtime and file I/O
- **async-std**: Additional async utilities
- **kamadak-exif**: EXIF metadata extraction
- **chrono**: Date/time handling
- **clap**: CLI argument parsing
- **uuid**: UUID generation for file renaming
- **md5**: MD5 checksum calculation for duplicate detection
- **sha2**: SHA-256 and SHA-512 checksum calculation

## File Structure Patterns

When copying files, the tool creates directories following ISO8601 format:
```
destination/
  2024/
    01/
      15/
        file.jpg
```

Files can optionally be renamed using UUIDs while preserving extensions.