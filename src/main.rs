use std::fs::metadata;
use std::path::{Path, PathBuf};
use exif::{Reader, Tag, In};
use std::error::Error;
use chrono::{NaiveDateTime, DateTime, Local, Datelike};
use clap::{Parser, Subcommand};
use uuid::Uuid;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use async_std::fs as async_fs;

#[derive(Parser)]
#[command(name = "machiver")]
#[command(about = "A tool for archiving files into BagIt bags")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug)]
struct CopyConfig<'a> {
    path: &'a Path,
    destination: &'a Path,
    recursive: bool,
    rename: bool,
    manifest: Option<Vec<String>>,
}

#[derive(Subcommand)]
enum Commands {

    /// Returns the date the file was created.
    ///
    /// If the file contains EXIF data, the date will be extracted from the EXIF data.
    /// Otherwise, the file's creation time will be returned.
    Date {
        /// Path to the image file
        file: PathBuf,
    },

    /// Copies files to a new location using the date extracted from the file's metadata. (see Date subcommand)
    ///
    /// Directories will be created relative to the destination directory following the ISO8601 format.
    /// If no destination is specified, the current directory will be used.
    Copy {
        /// Source file or directory
        source: PathBuf,
        /// Destination directory (defaults to current directory)
        #[arg(default_value = ".")]
        destination: PathBuf,
        /// Recursively process directories
        #[arg(short, long)]
        recursive: bool,
        /// Rename files using a randomly generated UUID
        #[arg(short = 'm', long)]
        rename: bool,
        /// Path to a manifest file
        #[arg(short = 'c', long)]
        manifest: Option<PathBuf>,
    },
}

async fn get_date(path: &Path) -> Result<NaiveDateTime, Box<dyn Error>> {
    // Try to get EXIF date first
    let mut file = File::open(path).await?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).await?;
    let exif_date = Reader::new()
        .read_from_container(&mut std::io::Cursor::new(buffer))
        .ok()
        .and_then(|exif| {
            exif.get_field(Tag::DateTime, In::PRIMARY)
                .map(|field| field.display_value().to_string())
        })
        .and_then(|date_str| NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%d %H:%M:%S").ok());

    if let Some(date) = exif_date {
        return Ok(date);
    }

    // Fallback to file creation time
    let metadata = metadata(path)?;
    let created = metadata.created()?;
    let datetime: DateTime<Local> = created.into();
    Ok(datetime.naive_local())
}

async fn process_path<'a>(config: &'a CopyConfig<'a>) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    Box::pin(_process_path(config)).await
}

async fn _process_path<'a>(config: &'a CopyConfig<'a>) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut copied_files = Vec::new();

    if config.path.is_file() {
        copied_files.push(copy_file(config.path, config.destination, config.rename, config.manifest.as_ref()).await?);
    } else if config.path.is_dir() && config.recursive {
        let mut entries = tokio::fs::read_dir(config.path).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let nested_config = CopyConfig {
                path: &path,
                destination: config.destination,
                recursive: config.recursive,
                rename: config.rename,
                manifest: config.manifest.clone(),
            };
            let nested_results = Box::pin(_process_path(&nested_config)).await?;
            copied_files.extend(nested_results);
        }
    } else if config.path.is_dir() {
        return Err(format!("'{}' is a directory. Use --recursive to process directories",
            config.path.display()).into());
    }

    Ok(copied_files)
}

fn generate_uuid_filename(original: &Path) -> PathBuf {
    let extension = original.extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");

    let uuid = Uuid::new_v4().to_string().to_lowercase();

    if extension.is_empty() {
        PathBuf::from(uuid)
    } else {
        PathBuf::from(format!("{}.{}", uuid, extension))
    }
}

async fn is_duplicate(source: &Path, manifest: Option<&Vec<String>>) -> Result<Option<PathBuf>, Box<dyn Error>> {
    let Some(manifest_paths) = manifest else { return Ok(None) };

    let mut file = File::open(source).await?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).await?;
    let mut context = md5::Context::new();
    context.consume(&buffer);
    let digest = format!("{:x}", context.compute());

    // Check if this MD5 exists in the manifest
    for checksum in manifest_paths {
        if digest == *checksum {
            return Ok(Some(source.to_path_buf()));
        }
    }
    Ok(None)
}

async fn copy_file(source: &Path, destination: &Path, rename: bool, manifest: Option<&Vec<String>>) -> Result<PathBuf, Box<dyn Error>> {
    print!("Copying {}\t\t", source.file_name().unwrap_or_default().to_string_lossy());

    // Check for duplicates if manifest is provided
    if let Some(duplicate_path) = is_duplicate(source, manifest).await? {
        println!("(duplicate)");
        return Ok(duplicate_path);
    }

    let date = get_date(source).await?;

    // Create the date-based directory structure
    let date_path = PathBuf::from(format!("{}/{:02}/{:02}",
        date.year(),
        date.month(),
        date.day()
    ));

    // Combine with destination path
    let target_dir = destination.join(&date_path);
    async_fs::create_dir_all(&target_dir).await?;

    // Get the target filename
    let file_name = if rename {
        generate_uuid_filename(source)
    } else {
        PathBuf::from(source.file_name().ok_or("Source file has no name")?)
    };

    // Create the full destination path
    let target_path = target_dir.join(file_name);

    // Copy the file
    async_fs::copy(source, &target_path).await?;
    println!("OK!");

    Ok(target_path)
}

async fn parse_manifest(path: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let content = async_fs::read_to_string(path).await?;
    let checksums: Vec<String> = content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            parts[0].to_string()
        })
        .collect();

    if checksums.is_empty() {
        return Err("Manifest file is empty".into());
    }

    Ok(checksums)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Date { file } => {
            match get_date(&file).await {
                Ok(datetime) => println!("{}", datetime),
                Err(e) => println!("{}", e),
            }
        },
        Commands::Copy { source, destination, recursive, rename, manifest } => {
            let config = CopyConfig {
                path: &source,
                destination: &destination,
                recursive,
                rename,
                manifest: if let Some(m) = manifest.as_ref() {
                    Some(parse_manifest(m).await?)
                } else {
                    None
                },
            };
            match process_path(&config).await {
                Ok(copied_files) => println!("Finished! Files processed: {}", copied_files.len()),
                Err(e) => println!("Error: {}", e),
            }
        },
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Datelike};
    use tempfile::TempDir;
    use std::collections::HashSet;
    use std::fs;

    #[tokio::test]
    async fn test_is_duplicate() -> Result<(), Box<dyn Error>> {
        // Create a temporary directory
        let temp_dir = TempDir::new()?;

        // Create two identical files
        let file1_path = temp_dir.path().join("file1.txt");
        let file2_path = temp_dir.path().join("file2.txt");
        let file3_path = temp_dir.path().join("file3.txt");

        fs::write(&file1_path, b"test content")?;
        fs::write(&file2_path, b"test content")?;  // Same content as file1
        fs::write(&file3_path, b"different content")?;

        // Test with no manifest (should return None)
        assert!(is_duplicate(&file1_path, None).await?.is_none());

        // Calculate MD5 of file1
        let mut file = File::open(&file1_path).await?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).await?;
        let mut context = md5::Context::new();
        context.consume(&buffer);
        let file1_md5 = format!("{:x}", context.compute());

        // Test with manifest containing no duplicates
        let manifest = vec!["different_md5_hash".to_string()];
        assert!(is_duplicate(&file1_path, Some(&manifest)).await?.is_none());

        // Test with manifest containing a duplicate
        let manifest = vec![file1_md5];
        let result = is_duplicate(&file1_path, Some(&manifest)).await?;
        assert!(result.is_some());
        assert_eq!(result.unwrap(), file1_path);

        Ok(())
    }

    #[tokio::test]
    async fn test_exif_date() {
        let path = Path::new("fixtures/exifdate.jpeg");
        let result = get_date(path).await.unwrap();
        assert_eq!(
            result.date(),
            NaiveDate::from_ymd_opt(2020, 12, 26).unwrap()
        );
    }

    #[tokio::test]
    async fn test_file_creation_date() {
        let path = Path::new("fixtures/exifnodate.heif");
        let result = get_date(path).await.unwrap();
        // Since this depends on the file's creation time, we just verify
        // that we get a valid date and don't error
        assert!(result.year() >= 2024);
    }

    #[tokio::test]
    async fn test_copy_file_with_duplicates() -> Result<(), Box<dyn Error>> {
        let temp_dir = TempDir::new()?;
        let source = Path::new("fixtures/exifdate.jpeg");

        // First, calculate the MD5 of the source file
        let mut file = File::open(source).await?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).await?;
        let mut context = md5::Context::new();
        context.consume(&buffer);
        let source_md5 = format!("{:x}", context.compute());

        // Create a manifest with the source file's MD5
        let manifest = vec![source_md5];

        // Try to copy the file - it should be skipped
        let result = copy_file(source, temp_dir.path(), false, Some(&manifest)).await?;

        // Verify the file wasn't actually copied
        assert!(!result.starts_with(temp_dir.path()), "File should not have been copied to temp dir");
        assert_eq!(result, source, "Should return the source path for duplicates");

        // Now try with a different MD5 in the manifest
        let manifest = vec!["different_md5_hash".to_string()];
        let result = copy_file(source, temp_dir.path(), false, Some(&manifest)).await?;

        // Verify the file was copied this time
        assert!(result.starts_with(temp_dir.path()), "File should have been copied to temp dir");
        assert!(result.exists(), "Copied file should exist");

        Ok(())
    }

    #[tokio::test]
    async fn test_copy_file_with_exif() -> Result<(), Box<dyn Error>> {
        // Create a temporary directory for our test
        let temp_dir = TempDir::new()?;

        // Copy a file with EXIF data
        let source = Path::new("fixtures/exifdate.jpeg");
        let result = copy_file(source, temp_dir.path(), false, None).await?;

        // Verify the directory structure and file
        assert!(result.exists());
        assert_eq!(
            result.parent().unwrap().strip_prefix(temp_dir.path())?,
            Path::new("2020/12/26")
        );
        assert_eq!(result.file_name().unwrap(), source.file_name().unwrap());

        // Verify file contents are the same
        let original = async_fs::read(source).await?;
        let copied = async_fs::read(&result).await?;
        assert_eq!(original, copied);

        Ok(())
    }

    #[tokio::test]
    async fn test_copy_file_with_creation_date() -> Result<(), Box<dyn Error>> {
        // Create a temporary directory for our test
        let temp_dir = TempDir::new()?;

        // Copy a file without EXIF data
        let source = Path::new("fixtures/exifnodate.heif");
        let result = copy_file(source, temp_dir.path(), false, None).await?;

        // Verify the file exists and has correct name
        assert!(result.exists());
        assert_eq!(result.file_name().unwrap(), source.file_name().unwrap());

        // Verify the directory structure follows YYYY/MM/DD pattern
        let relative_path = result.parent().unwrap().strip_prefix(temp_dir.path())?;
        let path_str = relative_path.to_str().unwrap();
        assert!(path_str.matches('/').count() == 2); // Should have two slashes for YYYY/MM/DD

        // Verify file contents are the same
        let original = async_fs::read(source).await?;
        let copied = async_fs::read(&result).await?;
        assert_eq!(original, copied);

        Ok(())
    }

    #[tokio::test]
    async fn test_process_path_single_file() -> Result<(), Box<dyn Error>> {
        let temp_dir = TempDir::new()?;
        let source = Path::new("fixtures/exifdate.jpeg");

        let config = CopyConfig {
            path: source,
            destination: temp_dir.path(),
            recursive: false,
            rename: false,
            manifest: None,
        };
        let results = process_path(&config).await?;

        assert_eq!(results.len(), 1);
        assert!(results[0].exists());
        assert_eq!(results[0].file_name().unwrap(), source.file_name().unwrap());

        Ok(())
    }

    #[tokio::test]
    async fn test_process_path_directory_without_recursive() {
        let temp_dir = TempDir::new().unwrap();
        let source = Path::new("fixtures");

        let config = CopyConfig {
            path: source,
            destination: temp_dir.path(),
            recursive: false,
            rename: false,
            manifest: None,
        };
        let result = process_path(&config).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Use --recursive"));
    }

    #[tokio::test]
    async fn test_process_path_recursive() -> Result<(), Box<dyn Error>> {
        let temp_dir = TempDir::new()?;
        let source = Path::new("fixtures");

        let config = CopyConfig {
            path: source,
            destination: temp_dir.path(),
            recursive: true,
            rename: false,
            manifest: None,
        };
        let results = process_path(&config).await?;

        // Verify we copied some files
        assert!(!results.is_empty());

        // Verify each copied file exists and has correct structure
        for path in &results {
            assert!(path.exists());
            assert!(path.is_file());

            // Verify directory structure
            let relative = path.parent().unwrap().strip_prefix(temp_dir.path())?;
            let path_str = relative.to_str().unwrap();
            assert_eq!(path_str.matches('/').count(), 2); // YYYY/MM/DD structure
        }

        // Verify we copied both our test files
        let file_names: Vec<_> = results.iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();
        assert!(file_names.contains(&"exifdate.jpeg"));
        assert!(file_names.contains(&"exifnodate.heif"));

        Ok(())
    }

    #[tokio::test]
    async fn test_parse_manifest() -> Result<(), Box<dyn Error>> {
        let manifest_path = Path::new("fixtures/good-bag/manifest-md5.txt");
        let checksums = parse_manifest(manifest_path).await?;

        // Convert to a set for easier comparison
        let checksum_set: HashSet<String> = checksums.into_iter().collect();

        // Expected hashes from manifest-md5.txt
        let expected_hashes: HashSet<String> = vec![
            "3b5d5c3712955042212316173ccf37be".to_string(),
            "60b725f10c9c85c70d97880dfe8191b3".to_string(),
        ].into_iter().collect();

        assert_eq!(checksum_set, expected_hashes);
        Ok(())
    }

    #[tokio::test]
    async fn test_generate_uuid_filename() {
        // Test with extension
        let path = Path::new("test.jpg");
        let result = generate_uuid_filename(path);
        let result_str = result.to_str().unwrap();

        assert!(result_str.ends_with(".jpg"));
        assert_eq!(result_str.len(), 40); // 36 chars for UUID + 4 for '.jpg'
        assert!(result_str.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')); // Verify lowercase, numbers, and dashes

        // Test without extension
        let path = Path::new("test");
        let result = generate_uuid_filename(path);
        let result_str = result.to_str().unwrap();

        assert_eq!(result_str.len(), 36); // Just UUID
        assert!(!result_str.contains("."));
        assert!(result_str.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')); // Verify lowercase, numbers, and dashes
    }
}
