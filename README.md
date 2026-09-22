# filesweep

`filesweep` is a lightweight CLI tool written in Rust for scanning shared network directories (such as `Z:\FinanceShared` SMB mounts and `lampiran_temp` folders), identifying abandoned temporary files, detecting duplicate file attachments, and automating storage maintenance.

![filesweep CLI output](docs/screenshot.png)

## Features

- **Recursive Directory Scan**: Filters files by extension (`.tmp`, `.bak`, `.xlsx`, `.pdf`) and retention age.
- **Duplicate Detection**: Group files by file size and verify identity using content checksum comparison.
- **CSV Reporting**: Export disk usage analysis and cleanup summaries (`laporan_cleanup.csv`).
- **Dry-run Mode**: Safely preview deleted file lists and estimated reclaimed megabytes prior to execution.

## Target Environment & Build

- **Target**: Windows Server `x86_64-pc-windows-msvc` (Rust 1.98)
- **Deployment**: Compiled binary deployed on internal hypervisor VM and scheduled via Windows Task Scheduler.

### Building from Source

```powershell
cargo build --release --target x86_64-pc-windows-msvc
```

## Usage Example

### Manual Dry-Run Scan
```powershell
.\filesweep.exe scan --path "Z:\FinanceShared" --days 60 --ext "tmp,bak,csv" --dry-run
```

### Scheduled Purge Execution
Configured in Windows Task Scheduler to run weekly on Sundays at **23:00 WIB** (Asia/Jakarta):

```powershell
.\filesweep.exe purge --path "Z:\FinanceShared" --days 90 --ext "tmp,bak" --output "C:\Logs\filesweep\laporan_cleanup.csv"
```

> **Note**: Standard `std::fs::canonicalize` on Windows prepends the `\\?\` UNC prefix which breaks SMB drive mapping path resolution on mapped Windows Server drives like `Z:\`. The application uses custom string normalization as a workaround for this standard library limitation.