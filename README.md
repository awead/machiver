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

Builds a directory of files based on their creation date and time. Exif data is preferred, but if none is available, the file's creation time is used instead.

## Usage

### Date

Returns the date the file was created:

```bash
machiver date <path_to_image>
```

### Copy

Copies files to a new location using the date extracted from the file's metadata, with the option to rename files using a randomly generated UUID.

```bash
machiver copy <source> <destination> --recursive --rename
```
