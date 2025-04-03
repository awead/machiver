use std::fs::{self, File, metadata};
use std::path::{Path, PathBuf};
use exif::{Reader, Tag, In};
use std::error::Error;
use chrono::{NaiveDateTime, DateTime, Local, Datelike};
use clap::{Parser, Subcommand};
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "machiver")]
#[command(about = "A tool for archiving files into BagIt bags")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug)]
struct Config<'a> {
    path: &'a Path,
    destination: &'a Path,
    recursive: bool,
    rename: bool,
    manifest: Option<Vec<PathBuf>>,
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

fn get_date(path: &Path) -> Result<NaiveDateTime, Box<dyn Error>> {
    // Try to get EXIF date first
    let exif_date = File::open(path)
        .ok()
        .and_then(|file| Reader::new().read_from_container(&mut std::io::BufReader::new(file)).ok())
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

fn process_path(config: &Config) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut copied_files = Vec::new();

    if config.path.is_file() {
        copied_files.push(copy_file(config.path, config.destination, config.rename)?);
    } else if config.path.is_dir() && config.recursive {
        for entry in fs::read_dir(config.path)? {
            let entry = entry?;
            let path = entry.path();
            let nested_config = Config {
                path: &path,
                destination: config.destination,
                recursive: config.recursive,
                rename: config.rename,
                manifest: config.manifest.clone(),
            };
            copied_files.extend(process_path(&nested_config)?);
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

fn copy_file(source: &Path, destination: &Path, rename: bool) -> Result<PathBuf, Box<dyn Error>> {
    let date = get_date(source)?;

    // Create the date-based directory structure
    let date_path = PathBuf::from(format!("{}/{:02}/{:02}",
        date.year(),
        date.month(),
        date.day()
    ));

    // Combine with destination path
    let target_dir = destination.join(&date_path);
    fs::create_dir_all(&target_dir)?;

    // Get the target filename
    let file_name = if rename {
        generate_uuid_filename(source)
    } else {
        PathBuf::from(source.file_name().ok_or("Source file has no name")?)
    };

    // Create the full destination path
    let target_path = target_dir.join(file_name);

    // Copy the file
    fs::copy(source, &target_path)?;

    Ok(target_path)
}

fn parse_manifest(path: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let content = fs::read_to_string(path)?;
    let paths: Vec<PathBuf> = content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            PathBuf::from(parts[0])
        })
        .collect();

    if paths.is_empty() {
        return Err("Manifest file is empty".into());
    }

    Ok(paths)
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Date { file } => {
            match get_date(&file) {
                Ok(datetime) => println!("{}", datetime),
                Err(e) => println!("{}", e),
            }
        },
        Commands::Copy { source, destination, recursive, rename, manifest } => {
            let config = Config {
                path: &source,
                destination: &destination,
                recursive,
                rename,
                manifest: manifest.as_ref().and_then(|m| parse_manifest(m).ok()),
            };
            match process_path(&config) {
                Ok(copied_files) => {
                    for path in copied_files {
                        println!("Copied to {}", path.display());
                    }
                },
                Err(e) => println!("Error: {}", e),
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Datelike};
    use tempfile::TempDir;
    use std::collections::HashSet;
    use std::fs;

    #[test]
    fn test_exif_date() {
        let path = Path::new("fixtures/exifdate.jpeg");
        let result = get_date(path).unwrap();
        assert_eq!(
            result.date(),
            NaiveDate::from_ymd_opt(2020, 12, 26).unwrap()
        );
    }

    #[test]
    fn test_file_creation_date() {
        let path = Path::new("fixtures/exifnodate.heif");
        let result = get_date(path).unwrap();
        // Since this depends on the file's creation time, we just verify
        // that we get a valid date and don't error
        assert!(result.year() >= 2024);
    }

    #[test]
    fn test_copy_file_with_exif() -> Result<(), Box<dyn Error>> {
        // Create a temporary directory for our test
        let temp_dir = TempDir::new()?;

        // Copy a file with EXIF data
        let source = Path::new("fixtures/exifdate.jpeg");
        let result = copy_file(source, temp_dir.path(), false)?;

        // Verify the directory structure and file
        assert!(result.exists());
        assert_eq!(
            result.parent().unwrap().strip_prefix(temp_dir.path())?,
            Path::new("2020/12/26")
        );
        assert_eq!(result.file_name().unwrap(), source.file_name().unwrap());

        // Verify file contents are the same
        let original = fs::read(source)?;
        let copied = fs::read(&result)?;
        assert_eq!(original, copied);

        Ok(())
    }

    #[test]
    fn test_copy_file_with_creation_date() -> Result<(), Box<dyn Error>> {
        // Create a temporary directory for our test
        let temp_dir = TempDir::new()?;

        // Copy a file without EXIF data
        let source = Path::new("fixtures/exifnodate.heif");
        let result = copy_file(source, temp_dir.path(), false)?;

        // Verify the file exists and has correct name
        assert!(result.exists());
        assert_eq!(result.file_name().unwrap(), source.file_name().unwrap());

        // Verify the directory structure follows YYYY/MM/DD pattern
        let relative_path = result.parent().unwrap().strip_prefix(temp_dir.path())?;
        let path_str = relative_path.to_str().unwrap();
        assert!(path_str.matches('/').count() == 2); // Should have two slashes for YYYY/MM/DD

        // Verify file contents are the same
        let original = fs::read(source)?;
        let copied = fs::read(&result)?;
        assert_eq!(original, copied);

        Ok(())
    }

    #[test]
    fn test_process_path_single_file() -> Result<(), Box<dyn Error>> {
        let temp_dir = TempDir::new()?;
        let source = Path::new("fixtures/exifdate.jpeg");

        let config = Config {
            path: source,
            destination: temp_dir.path(),
            recursive: false,
            rename: false,
            manifest: None,
        };
        let results = process_path(&config)?;

        assert_eq!(results.len(), 1);
        assert!(results[0].exists());
        assert_eq!(results[0].file_name().unwrap(), source.file_name().unwrap());

        Ok(())
    }

    #[test]
    fn test_process_path_directory_without_recursive() {
        let temp_dir = TempDir::new().unwrap();
        let source = Path::new("fixtures");

        let config = Config {
            path: source,
            destination: temp_dir.path(),
            recursive: false,
            rename: false,
            manifest: None,
        };
        let result = process_path(&config);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Use --recursive"));
    }

    #[test]
    fn test_process_path_recursive() -> Result<(), Box<dyn Error>> {
        let temp_dir = TempDir::new()?;
        let source = Path::new("fixtures");

        let config = Config {
            path: source,
            destination: temp_dir.path(),
            recursive: true,
            rename: false,
            manifest: None,
        };
        let results = process_path(&config)?;

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

    #[test]
    fn test_parse_manifest() -> Result<(), Box<dyn Error>> {
        let manifest_path = Path::new("fixtures/good-bag/manifest-md5.txt");
        let paths = parse_manifest(manifest_path)?;

        // Convert to a set of strings for easier comparison
        let path_strings: HashSet<String> = paths
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();

        // Expected hashes from manifest-md5.txt
        let expected_hashes: HashSet<String> = vec![
            "3b5d5c3712955042212316173ccf37be".to_string(),
            "60b725f10c9c85c70d97880dfe8191b3".to_string(),
        ].into_iter().collect();

        assert_eq!(path_strings, expected_hashes);
        Ok(())
    }

    #[test]
    fn test_generate_uuid_filename() {
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
