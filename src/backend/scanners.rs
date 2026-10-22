use std::path::{Path, PathBuf};

use crate::backend::shell::{describe_failure, Cmd, Runner};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scanner {
    pub device: String,
    pub description: String,
    pub network: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannerDiscovery {
    pub scanners: Vec<Scanner>,
    pub notes: Vec<String>,
    pub usb_findings: Vec<String>,
}

pub fn parse_scanimage(output: &str) -> Vec<Scanner> {
    let mut scanners = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        let Some(first_tick) = line.find('`') else {
            continue;
        };
        let Some(second_offset) = line[first_tick + 1..].find('`') else {
            continue;
        };
        let device = line[first_tick + 1..first_tick + 1 + second_offset]
            .trim()
            .to_string();
        let description = line[first_tick + 1 + second_offset + 1..]
            .trim()
            .strip_prefix("is a ")
            .unwrap_or("")
            .trim()
            .to_string();
        if device.is_empty() || description.is_empty() {
            continue;
        }
        let network = device.starts_with("escl:")
            || device.starts_with("airscan:")
            || device.contains(":/net")
            || device.starts_with("http")
            || description.to_lowercase().contains("ip=");
        scanners.push(Scanner {
            device,
            description,
            network,
        });
    }
    scanners
}

pub fn parse_find_scanner(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| line.contains("found USB scanner") || line.contains("found SCSI scanner"))
        .map(str::to_string)
        .collect()
}

pub fn discover_scanners(runner: &dyn Runner) -> ScannerDiscovery {
    let mut scanners = Vec::new();
    let mut notes = Vec::new();
    let mut usb_findings = Vec::new();

    if runner.exists("scanimage") {
        let cmd = Cmd::new("scanimage").arg("-L");
        match runner.run(&cmd) {
            Ok(output) => {
                scanners = parse_scanimage(&output.stdout);
                if !output.success() && !output.stderr.trim().is_empty() {
                    notes.push(output.stderr.trim().to_string());
                }
            }
            Err(error) => notes.push(format!("اجرای scanimage -L ناموفق بود: {error}")),
        }
    } else {
        notes.push("scanimage پیدا نشد؛ بسته sane را نصب کنید.".into());
    }

    if scanners.is_empty() && runner.exists("sane-find-scanner") {
        match runner.run(&Cmd::new("sane-find-scanner")) {
            Ok(output) => {
                usb_findings = parse_find_scanner(&output.stdout);
                if !usb_findings.is_empty() {
                    notes.push(
                        "دستگاه USB پیدا شد اما SANE آن را به‌عنوان اسکنر معرفی نکرد؛ احتمالاً درایور یا دسترسی udev لازم است."
                            .into(),
                    );
                }
            }
            Err(error) => notes.push(format!("اجرای sane-find-scanner ناموفق بود: {error}")),
        }
    }

    ScannerDiscovery {
        scanners,
        notes,
        usb_findings,
    }
}

pub fn test_scan_command(device: &str, output: &Path, resolution: u32) -> Result<Cmd, String> {
    if device.trim().is_empty() || device.starts_with('-') {
        return Err("شناسه اسکنر نامعتبر است.".into());
    }
    if !(50..=1200).contains(&resolution) {
        return Err("رزولوشن باید بین ۵۰ و ۱۲۰۰ باشد.".into());
    }
    if output.as_os_str().is_empty() {
        return Err("مسیر خروجی اسکن نامعتبر است.".into());
    }
    Ok(Cmd::new("scanimage").args([
        "-d",
        device,
        "--format=png",
        &format!("--resolution={resolution}"),
        "-o",
        &output.to_string_lossy(),
    ]))
}

pub fn run_test_scan(
    runner: &dyn Runner,
    device: &str,
    output: &Path,
    resolution: u32,
) -> Result<String, String> {
    let cmd = test_scan_command(device, output, resolution)?;
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!("ساخت پوشه خروجی «{}» ناموفق بود: {error}", parent.display())
            })?;
        }
    }
    let process = runner
        .run(&cmd)
        .map_err(|error| format!("اجرای scanimage ناموفق بود: {error}. آیا بسته sane نصب است؟"))?;
    if !process.success() {
        return Err(describe_failure(&cmd, &process));
    }
    match std::fs::metadata(output) {
        Ok(metadata) if metadata.len() > 0 => Ok(format!(
            "اسکن آزمایشی با موفقیت ساخته شد ({} بایت).",
            metadata.len()
        )),
        Ok(_) => Err("فایل اسکن ساخته شد اما خالی است.".into()),
        Err(error) => Err(format!(
            "فایل خروجی «{}» ساخته نشد: {error}",
            output.display()
        )),
    }
}

pub fn default_scan_path() -> PathBuf {
    std::env::temp_dir().join(format!("linuprint-scan-{}.png", std::process::id()))
}
