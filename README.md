# Image Metadata Extractor

A simple Rust program that extracts EXIF metadata from image files using the `kamadak-exif` crate.

## Features

- Extracts common EXIF metadata including:
  - Date/Time
  - Camera Make and Model
  - Exposure Time
  - F-Number
  - ISO Speed

## Usage

```bash
cargo run -- <path_to_image>
```

For example:
```bash
cargo run -- path/to/your/photo.jpg
```

## Requirements

- Rust (2021 edition or later)
- The program works with image files that contain EXIF metadata (typically JPEG and TIFF files)
