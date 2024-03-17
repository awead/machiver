use std::env;
use std::fs::{File, metadata};
use std::path::Path;
use exif::{Reader, Tag, In};
use std::error::Error;
use chrono::{NaiveDateTime, DateTime, Local};

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
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: {} <image_file>", args[0]);
        return;
    }

    let path = Path::new(&args[1]);

    match get_date(path) {
        Ok(datetime) => println!("{}", datetime),
        Err(e) => println!("{}", e),
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
