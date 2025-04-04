use std::path::PathBuf;
use std::error::Error;
use clap::{Parser, Subcommand};

pub mod copy;
pub mod date;

use copy::{CopyConfig, process_path, parse_manifest};
use date::get_date;

#[derive(Parser)]
#[command(name = "machiver")]
#[command(about = "A tool for archiving files into BagIt bags")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Returns the date associated with the file.
    ///
    /// If the file contains EXIF data, the date will be extracted from the EXIF data.
    /// Otherwise, the file's modification time will be returned.
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
