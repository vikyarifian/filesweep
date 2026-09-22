use std::fs;
use std::path::Path;
use std::time::SystemTime;
use crate::cli::ScanConfig;

/// Represents metadata of an identified file (file path, file size in bytes,
/// last modified timestamp, and optional SHA-256 checksum hash).
#[derive(Debug, Clone)]
pub struct FileDetail {
    pub path: String,
    pub size: u64,
    pub modified: u64,
    pub hash: Option<String>,
}

/// Sanitizes UNC and standard paths for Windows/SMB compatibility.
pub fn sanitize_unc_path(path_str: &str) -> String {
    let mut cleaned = path_str.replace("/", "\\");
    
    // Workaround for std::fs::canonicalize limitation where it prepends the verbatim UNC prefix "\\?\"
    // or "\\?\UNC\" to the path. This Windows-specific prefix confuses legacy ERP macros
    // and Pak Budi's automatic Excel templates that consume our generated reports.
    if cleaned.starts_with("\\\\?\\UNC\\") {
        cleaned = format!("\\\\{}", &cleaned[8..]);
    } else if cleaned.starts_with("\\\\?\\") {
        cleaned = cleaned[4..].to_string();
    }
    
    cleaned
}

/// Crawls directories recursively, filtering by age (e.g., older than 30 days)
/// and specific file types like export sheets (.xlsx, .csv) and scanned attachments (.pdf, .jpg).
/// Includes UNC path sanitization for SMB.
pub fn scan_directory(config: &ScanConfig) -> Result<Vec<FileDetail>, std::io::Error> {
    let mut files = Vec::new();
    let start_path = Path::new(&config.target_path);
    
    if !start_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Target path does not exist: {}", config.target_path),
        ));
    }

    visit_dirs(start_path, config, &mut files)?;
    Ok(files)
}

fn visit_dirs(dir: &Path, config: &ScanConfig, files: &mut Vec<FileDetail>) -> std::io::Result<()> {
    // Read the directory contents, handling permission errors gracefully so the scan doesn't abort
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(()), 
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();
        
        if path.is_dir() {
            // Recursively scan, ignoring errors in sub-folders to keep scanning stable
            let _ = visit_dirs(&path, config, files);
        } else {
            if let Ok(metadata) = entry.metadata() {
                if !metadata.is_file() {
                    continue;
                }

                // Check file extension filter
                let extension = path.extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.to_lowercase());

                let match_extension = if config.extensions.is_empty() {
                    true
                } else {
                    match &extension {
                        Some(ext) => config.extensions.iter().any(|e| e.to_lowercase() == *ext),
                        None => false,
                    }
                };

                if !match_extension {
                    continue;
                }

                // Check age threshold (older than X days)
                let modified_time = match metadata.modified() {
                    Ok(t) => t,
                    Err(_) => continue,
                };

                let age_days = match SystemTime::now().duration_since(modified_time) {
                    Ok(duration) => duration.as_secs() / 86400,
                    Err(_) => 0,
                };

                if age_days >= config.age_threshold_days {
                    let abs_path = match path.canonicalize() {
                        Ok(p) => p.to_string_lossy().into_owned(),
                        Err(_) => path.to_string_lossy().into_owned(),
                    };

                    let sanitized_path = sanitize_unc_path(&abs_path);
                    let modified_epoch = match modified_time.duration_since(SystemTime::UNIX_EPOCH) {
                        Ok(dur) => dur.as_secs(),
                        Err(_) => 0,
                    };

                    files.push(FileDetail {
                        path: sanitized_path,
                        size: metadata.len(),
                        modified: modified_epoch,
                        hash: None,
                    });
                }
            }
        }
    }

    Ok(())
}
