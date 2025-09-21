use std::path::{Path, PathBuf};
use std::error::Error;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use async_std::fs as async_fs;
use sha2::{Sha256, Sha512, Digest};

#[derive(Debug, Clone)]
pub enum HashAlgorithm {
    MD5,
    SHA256,
    SHA512,
}

impl HashAlgorithm {
    pub fn from_filename(path: &Path) -> Self {
        let filename = path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");

        if filename.contains("md5") {
            HashAlgorithm::MD5
        } else if filename.contains("sha256") {
            HashAlgorithm::SHA256
        } else if filename.contains("sha512") {
            HashAlgorithm::SHA512
        } else {
            eprintln!("Warning: Unsupported hash algorithm in '{}', defaulting to SHA256", filename);
            HashAlgorithm::SHA256
        }
    }

    pub async fn calculate_hash(&self, data: &[u8]) -> String {
        match self {
            HashAlgorithm::MD5 => {
                format!("{:x}", md5::compute(data))
            },
            HashAlgorithm::SHA256 => {
                let mut hasher = Sha256::new();
                hasher.update(data);
                format!("{:x}", hasher.finalize())
            },
            HashAlgorithm::SHA512 => {
                let mut hasher = Sha512::new();
                hasher.update(data);
                format!("{:x}", hasher.finalize())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Manifest {
    pub checksums: Vec<String>,
    pub algorithm: HashAlgorithm,
}

pub async fn parse_manifest(path: &Path) -> Result<Manifest, Box<dyn Error>> {
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

    let algorithm = HashAlgorithm::from_filename(path);

    Ok(Manifest { checksums, algorithm })
}

pub async fn is_duplicate(source: &Path, manifest: Option<&Manifest>) -> Result<Option<PathBuf>, Box<dyn Error>> {
    let Some(manifest) = manifest else { return Ok(None) };

    let mut file = File::open(source).await?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).await?;

    let digest = manifest.algorithm.calculate_hash(&buffer).await;

    // Check if this hash exists in the manifest
    for checksum in &manifest.checksums {
        if digest == *checksum {
            return Ok(Some(source.to_path_buf()));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use async_std::fs;
    use std::collections::HashSet;

    #[tokio::test]
    async fn test_hash_algorithm_detection() {
        assert!(matches!(
            HashAlgorithm::from_filename(Path::new("manifest-md5.txt")),
            HashAlgorithm::MD5
        ));
        assert!(matches!(
            HashAlgorithm::from_filename(Path::new("manifest-sha256.txt")),
            HashAlgorithm::SHA256
        ));
        assert!(matches!(
            HashAlgorithm::from_filename(Path::new("manifest-sha512.txt")),
            HashAlgorithm::SHA512
        ));
        assert!(matches!(
            HashAlgorithm::from_filename(Path::new("manifest-xyz123.txt")),
            HashAlgorithm::SHA256
        ));
    }

    #[tokio::test]
    async fn test_hash_calculations() -> Result<(), Box<dyn Error>> {
        let test_data = b"test content";

        // Test MD5
        let md5_hash = HashAlgorithm::MD5.calculate_hash(test_data).await;
        assert_eq!(md5_hash, "9473fdd0d880a43c21b7778d34872157");

        // Test SHA256
        let sha256_hash = HashAlgorithm::SHA256.calculate_hash(test_data).await;
        assert_eq!(sha256_hash, "6ae8a75555209fd6c44157c0aed8016e763ff435a19cf186f76863140143ff72");

        // Test SHA512
        let sha512_hash = HashAlgorithm::SHA512.calculate_hash(test_data).await;
        assert_eq!(sha512_hash, "0cbf4caef38047bba9a24e621a961484e5d2a92176a859e7eb27df343dd34eb98d538a6c5f4da1ce302ec250b821cc001e46cc97a704988297185a4df7e99602");

        Ok(())
    }

    #[tokio::test]
    async fn test_is_duplicate_with_different_algorithms() -> Result<(), Box<dyn Error>> {
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, b"test content").await?;

        // Test with MD5
        let md5_manifest = Manifest {
            checksums: vec!["9473fdd0d880a43c21b7778d34872157".to_string()],
            algorithm: HashAlgorithm::MD5,
        };
        let result = is_duplicate(&file_path, Some(&md5_manifest)).await?;
        assert!(result.is_some());

        // Test with SHA256
        let sha256_manifest = Manifest {
            checksums: vec!["6ae8a75555209fd6c44157c0aed8016e763ff435a19cf186f76863140143ff72".to_string()],
            algorithm: HashAlgorithm::SHA256,
        };
        let result = is_duplicate(&file_path, Some(&sha256_manifest)).await?;
        assert!(result.is_some());

        // Test with SHA512
        let sha512_manifest = Manifest {
            checksums: vec!["0cbf4caef38047bba9a24e621a961484e5d2a92176a859e7eb27df343dd34eb98d538a6c5f4da1ce302ec250b821cc001e46cc97a704988297185a4df7e99602".to_string()],
            algorithm: HashAlgorithm::SHA512,
        };
        let result = is_duplicate(&file_path, Some(&sha512_manifest)).await?;
        assert!(result.is_some());

        // Test with wrong hash
        let wrong_manifest = Manifest {
            checksums: vec!["wrong_hash".to_string()],
            algorithm: HashAlgorithm::MD5,
        };
        let result = is_duplicate(&file_path, Some(&wrong_manifest)).await?;
        assert!(result.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_parse_manifest() -> Result<(), Box<dyn Error>> {
        let manifest_path = Path::new("fixtures/good-bag/manifest-md5.txt");
        let manifest = parse_manifest(manifest_path).await?;

        // Should detect MD5 algorithm
        assert!(matches!(manifest.algorithm, HashAlgorithm::MD5));

        // Convert to a set for easier comparison
        let checksum_set: HashSet<String> = manifest.checksums.into_iter().collect();

        // Expected hashes from manifest-md5.txt
        let expected_hashes: HashSet<String> = vec![
            "3b5d5c3712955042212316173ccf37be".to_string(),
            "60b725f10c9c85c70d97880dfe8191b3".to_string(),
        ].into_iter().collect();

        assert_eq!(checksum_set, expected_hashes);
        Ok(())
    }

    #[tokio::test]
    async fn test_parse_manifest_sha256() -> Result<(), Box<dyn Error>> {
        let temp_dir = TempDir::new()?;
        let manifest_path = temp_dir.path().join("manifest-sha256.txt");

        // Create a test manifest file
        fs::write(&manifest_path, "abc123def456789 data/file1.txt\n987654321fedcba data/file2.txt\n").await?;

        let manifest = parse_manifest(&manifest_path).await?;

        // Should detect SHA256 algorithm
        assert!(matches!(manifest.algorithm, HashAlgorithm::SHA256));
        assert_eq!(manifest.checksums.len(), 2);
        assert_eq!(manifest.checksums[0], "abc123def456789");
        assert_eq!(manifest.checksums[1], "987654321fedcba");

        Ok(())
    }
}