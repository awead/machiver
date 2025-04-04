# Machiver

[![CI](https://github.com/awead/machiver/actions/workflows/ci.yml/badge.svg)](https://github.com/awead/machiver/actions/workflows/ci.yml)

A multi-purpose tool for archiving files. It assumes files will be organized into BagIt bags.

## Installation

To install machiver, you'll need Rust installed on your system. Then run:

```bash
cargo install --path .
```

This will compile the binary and install it in your Cargo binary directory (usually `~/.cargo/bin`).

## Features

Builds a directory of files based on their associated date and time. EXIF data is preferred, but if none is available, the file's modification time is used instead.

## Usage

### Date

Returns the date associated with the file:

```bash
machiver date <path_to_image>
```

### Copy

Copies files to a new location using the date extracted from the file's metadata, with the option to rename files using a randomly generated UUID.

```bash
machiver copy <source> <destination> --recursive --rename
```

## Cross-Compilation for Synology NAS

Add the target:

```bash
rustup target add aarch64-unknown-linux-gnu
```

Install the cross-compiler tools for MacOS using brew:
```bash
brew tap messense/macos-cross-toolchains
brew install aarch64-unknown-linux-gnu
```

Create `.cargo/config.toml` with:
```toml
[target.aarch64-unknown-linux-gnu]
linker = "aarch64-linux-gnu-gcc"
rustflags = ["-C", "target-feature=+crt-static"]
```

Build for the target:
```bash
cargo build --release --target aarch64-unknown-linux-gnu
```

The compiled binary will be in `target/aarch64-unknown-linux-gnu/release/machiver`.
