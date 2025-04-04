use std::path::Path;
use std::fs::metadata;
use std::error::Error;
use exif::{Reader, Tag, In};
use chrono::{NaiveDateTime, DateTime, Local};
use tokio::fs::File;
use tokio::io::AsyncReadExt;

pub async fn get_date(path: &Path) -> Result<NaiveDateTime, Box<dyn Error>> {
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

    // Fallback to file modification time (more reliable across platforms than creation time)
    let metadata = metadata(path)?;
    let modified = metadata.modified()?;
    let datetime: DateTime<Local> = modified.into();
    Ok(datetime.naive_local())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Datelike};

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
    async fn test_file_modified_date() {
        let path = Path::new("fixtures/exifnodate.heif");
        let result = get_date(path).await.unwrap();
        // Since this depends on the file's modification time, we just verify
        // that we get a valid date and don't error
        assert!(result.year() >= 2024);
    }
}
