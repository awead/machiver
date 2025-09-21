use std::path::{Path, PathBuf};
use std::error::Error;
use async_std::fs as async_fs;
use uuid::Uuid;
use chrono::Datelike;
use crate::date::get_date;
use crate::manifest::{Manifest, is_duplicate};

#[derive(Debug)]
pub struct CopyConfig<'a> {
    pub path: &'a Path,
    pub destination: &'a Path,
    pub recursive: bool,
    pub rename: bool,
    pub manifest: Option<Manifest>,
}

pub async fn process_path<'a>(config: &'a CopyConfig<'a>) -> Result<Vec<PathBuf>, Box<dyn Error>> {
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


pub fn generate_uuid_filename(original: &Path) -> PathBuf {
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

pub async fn copy_file(source: &Path, destination: &Path, rename: bool, manifest: Option<&Manifest>) -> Result<PathBuf, Box<dyn Error>> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use async_std::fs;

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
        let original = fs::read(source).await?;
        let copied = fs::read(&result).await?;
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
        let original = fs::read(source).await?;
        let copied = fs::read(&result).await?;
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
