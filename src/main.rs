use std::fs::{File, metadata};
use std::path::{Path, PathBuf};
use exif::{Reader, Tag, In};
use std::error::Error;
use chrono::{NaiveDateTime, DateTime, Local};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "machiver")]
#[command(about = "A tool for archiving files into BagIt bags")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
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
        /// Source image file
        source: PathBuf,
        /// Destination directory
        destination: PathBuf,
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

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Date { file } => {
            match get_date(&file) {
                Ok(datetime) => println!("{}", datetime),
                Err(e) => println!("{}", e),
            }
        },
        Commands::Copy { source, destination } => {
            match get_date(&source) {
                Ok(datetime) => {
                    println!("Would copy {} to {} using date {}",
                        source.display(),
                        destination.display(),
                        datetime
                    );
                },
                Err(e) => println!("{}", e),
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Datelike};

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
}
