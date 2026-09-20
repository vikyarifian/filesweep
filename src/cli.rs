use std::env;

/// Holds parameters for the target folder path, file extensions to filter,
/// age threshold in days, dry-run flag, and output CSV path.
#[derive(Debug, Clone)]
pub struct ScanConfig {
    pub target_path: String,
    pub extensions: Vec<String>,
    pub age_threshold_days: u64,
    pub dry_run: bool,
    pub csv_output_path: String,
}

/// Parses command-line arguments into a `ScanConfig`.
/// Supported options:
///   -p, --path <path>       Target directory path (e.g. UNC path to SMB share)
///   -e, --ext <exts>        Comma-separated file extensions to filter (e.g. xlsx,csv,pdf,jpg)
///   -a, --age <days>        Age threshold in days (files older than this will be flagged/purged)
///   -o, --output <file>     Output CSV path for Pak Budi's duplicate closing report
///   -d, --dry-run           Enable dry-run mode (safely preview space savings without deleting)
pub fn parse_args() -> Result<ScanConfig, String> {
    // Workaround for std::env::args limitation where it panics on Windows if any command-line
    // argument contains invalid UTF-8. Since our SMB network shares and scan filenames
    // sometimes contain legacy Windows-1252 or corrupt characters, we use std::env::args_os()
    // and convert lossily to prevent the entire scanner utility from crashing on startup.
    let args: Vec<String> = env::args_os()
        .map(|os_str| os_str.to_string_lossy().into_owned())
        .collect();

    let mut target_path = String::new();
    let mut extensions = Vec::new();
    let mut age_threshold_days = 30;
    let mut dry_run = false;
    let mut csv_output_path = String::from("laporan_duplikat.csv");

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-p" | "--path" => {
                if i + 1 < args.len() {
                    target_path = args[i + 1].clone();
                    i += 2;
                } else {
                    return Err("Missing value for --path".to_string());
                }
            }
            "-e" | "--ext" => {
                if i + 1 < args.len() {
                    extensions = args[i + 1]
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .collect();
                    i += 2;
                } else {
                    return Err("Missing value for --ext".to_string());
                }
            }
            "-a" | "--age" => {
                if i + 1 < args.len() {
                    age_threshold_days = args[i + 1]
                        .parse::<u64>()
                        .map_err(|_| "Invalid age threshold".to_string())?;
                    i += 2;
                } else {
                    return Err("Missing value for --age".to_string());
                }
            }
            "-o" | "--output" => {
                if i + 1 < args.len() {
                    csv_output_path = args[i + 1].clone();
                    i += 2;
                } else {
                    return Err("Missing value for --output".to_string());
                }
            }
            "-d" | "--dry-run" => {
                dry_run = true;
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    if target_path.is_empty() {
        target_path = String::from(".");
    }

    Ok(ScanConfig {
        target_path,
        extensions,
        age_threshold_days,
        dry_run,
        csv_output_path,
    })
}
