use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

mod cli;
mod scanner;

struct AbsensiRecord {
    nik: String,
    nama: String,
    tanggal: String,
    jam_masuk: String,
    jam_keluar: String,
    tarif_harian: u64,
}

// Parse standard HH:MM time format into total minutes from start of day
fn parse_time_to_minutes(time_str: &str) -> Option<i32> {
    let parts: Vec<&str> = time_str.trim().split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let hours: i32 = parts[0].parse().ok()?;
    let minutes: i32 = parts[1].parse().ok()?;
    Some(hours * 60 + minutes)
}

// Calculate overtime based on Indonesian labor regulations
fn hitung_lembur(menit_kerja: i32, tarif_harian: u64) -> (u64, i32) {
    let jam_kerja = menit_kerja / 60;
    if jam_kerja <= 8 {
        return (0, 0);
    }
    let jam_lembur = jam_kerja - 8;
    let tarif_per_jam = tarif_harian / 8;
    
    let mut total_lembur = 0;
    if jam_lembur > 0 {
        // First hour of overtime is paid 1.5x hourly wage
        total_lembur += (1.5 * tarif_per_jam as f64) as u64;
    }
    if jam_lembur > 1 {
        // Subsequent hours are paid 2x hourly wage
        total_lembur += ((jam_lembur - 1) as f64 * 2.0 * tarif_per_jam as f64) as u64;
    }
    (total_lembur, jam_lembur)
}

// Calculate simplified daily worker income tax (PPh 21)
fn hitung_pph21(total_penghasilan: u64) -> u64 {
    // Daily non-taxable limit (PTKP harian) is Rp 450.000
    if total_penghasilan <= 450_000 { 
        0
    } else {
        let kena_pajak = total_penghasilan - 450_000;
        // Progressive standard daily rate of 5% for the taxable excess
        (kena_pajak as f64 * 0.05) as u64
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    // Check if we are running the SMB folder scanner instead of payroll calculations
    let is_scan_mode = args.iter().any(|arg| arg.starts_with('-'));

    if is_scan_mode {
        match cli::parse_args() {
            Ok(config) => {
                println!("Starting directory scan on target path: {}", config.target_path);
                match scanner::scan_directory(&config) {
                    Ok(files) => {
                        println!("Found {} matching files.", files.len());
                        let mut out_file = File::create(&config.csv_output_path)?;
                        writeln!(out_file, "path,size_bytes,modified_epoch")?;
                        for f in files {
                            writeln!(out_file, "{},{},{}", f.path, f.size, f.modified)?;
                        }
                        println!("Scan results written to {}", config.csv_output_path);
                    }
                    Err(e) => {
                        eprintln!("Scan error: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error parsing scan arguments: {}", e);
            }
        }
        return Ok(());
    }

    let input_path = args.get(1).map(|s| s.as_str()).unwrap_or("absensi.csv");
    let output_path = args.get(2).map(|s| s.as_str()).unwrap_or("rekap_gaji.csv");

    println!("Reading attendance data from: {}", input_path);

    if !Path::new(input_path).exists() { 
        println!("File {} not found. Creating a default template...", input_path);
        let default_csv = "\
nik,nama,tanggal,jam_masuk,jam_keluar,tarif_harian
1001,Budi Santoso,2026-03-02,08:00,17:00,350000
1002,Siti Aminah,2026-03-02,08:00,19:30,400000
1003,Eko Prasetyo,2026-03-02,08:30,17:30,500000
1004,Dewi Lestari,2026-03-02,08:00,16:00,300000
";
        std::fs::write(input_path, default_csv)?;
    }

    // Workaround for std::fs::read_to_string limitation where it strictly validates UTF-8 and fails on Windows-1252 encoded CSVs exported from Excel. We have to read raw bytes and do lossy conversion instead.
    let bytes = std::fs::read(input_path)?;
    let content = String::from_utf8_lossy(&bytes);

    let mut records = Vec::new();
    let lines = content.lines();

    let mut header_skipped = false;
    for (line_num, line) in lines.enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !header_skipped {
            header_skipped = true;
            continue;
        }

        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() < 6 {
            eprintln!("Warning: Row {} ignored due to incomplete fields: {}", line_num + 1, line);
            continue;
        }

        let nik = fields[0].trim().to_string();
        let nama = fields[1].trim().to_string();
        let tanggal = fields[2].trim().to_string();
        let jam_masuk = fields[3].trim().to_string();
        let jam_keluar = fields[4].trim().to_string();
        let tarif_harian: u64 = match fields[5].trim().parse() {
            Ok(val) => val,
            Err(_) => {
                eprintln!("Warning: Row {} ignored due to invalid daily tariff", line_num + 1);
                continue;
            }
        };

        records.push(AbsensiRecord {
            nik,
            nama,
            tanggal,
            jam_masuk,
            jam_keluar,
            tarif_harian,
        });
    }

    let mut out_file = File::create(output_path)?;
    writeln!(
        out_file,
        "nik,nama,tanggal,jam_kerja_menit,lembur_jam,gaji_pokok,uang_lembur,pph21,gaji_bersih"
    )?;

    println!("Processing {} employee records...", records.len());

    for rec in records {
        let masuk_min = match parse_time_to_minutes(&rec.jam_masuk) {
            Some(m) => m,
            None => {
                eprintln!("Failed parsing jam_masuk for NIK {}", rec.nik);
                continue;
            }
        };
        let keluar_min = match parse_time_to_minutes(&rec.jam_keluar) {
            Some(m) => m,
            None => {
                eprintln!("Failed parsing jam_keluar for NIK {}", rec.nik);
                continue;
            }
        };

        let mut menit_kerja = keluar_min - masuk_min;
        if menit_kerja < 0 {
            // Workaround for overnight shift crossing midnight
            menit_kerja += 24 * 60;
        }

        // Under Indonesian labor regulations, a shift longer than 4 hours must include a rest break of at least 1 hour.
        if menit_kerja > 240 {
            menit_kerja -= 60;
        }

        let (uang_lembur, jam_lembur) = hitung_lembur(menit_kerja, rec.tarif_harian);
        let gaji_kotor = rec.tarif_harian + uang_lembur;
        let pph21_potongan = hitung_pph21(gaji_kotor);
        let gaji_bersih = gaji_kotor - pph21_potongan;

        writeln!(
            out_file,
            "{},{},{},{},{},{},{},{},{}",
            rec.nik,
            rec.nama,
            rec.tanggal,
            menit_kerja,
            jam_lembur,
            rec.tarif_harian,
            uang_lembur,
            pph21_potongan,
            gaji_bersih
        )?;
    }

    println!("Finished! Saved to output: {}", output_path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test] 
    fn test_hitung_pph21_di_bawah_ptkp() {
        // Daily income below PTKP limit of Rp 450.000 should yield 0 tax
        assert_eq!(hitung_pph21(350_000), 0);
    }
}
