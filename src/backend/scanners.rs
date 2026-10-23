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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ScanFormat {
    #[default]
    Png,
    Jpeg,
    Pdf,
}

impl ScanFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Pdf => "pdf",
        }
    }

    fn scanimage_value(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Pdf => "pdf",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ScanColorMode {
    #[default]
    Auto,
    Color,
    Gray,
    Lineart,
}

impl ScanColorMode {
    fn scanimage_value(self) -> Option<&'static str> {
        match self {
            Self::Auto => None,
            Self::Color => Some("Color"),
            Self::Gray => Some("Gray"),
            Self::Lineart => Some("Lineart"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanOptions {
    pub resolution: u32,
    pub format: ScanFormat,
    pub color_mode: ScanColorMode,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            resolution: 150,
            format: ScanFormat::Png,
            color_mode: ScanColorMode::Auto,
        }
    }
}

pub fn scan_command(device: &str, output: &Path, options: &ScanOptions) -> Result<Cmd, String> {
    if device.trim().is_empty() || device.starts_with('-') {
        return Err("شناسه اسکنر نامعتبر است.".into());
    }
    if !(50..=1200).contains(&options.resolution) {
        return Err("رزولوشن باید بین ۵۰ و ۱۲۰۰ باشد.".into());
    }
    if output.as_os_str().is_empty() {
        return Err("مسیر خروجی اسکن نامعتبر است.".into());
    }

    let mut cmd = Cmd::new("scanimage").args(["-d", device]);
    let format = format!("--format={}", options.format.scanimage_value());
    cmd = cmd.args([
        format.as_str(),
        &format!("--resolution={}", options.resolution),
    ]);
    if let Some(mode) = options.color_mode.scanimage_value() {
        let mode = format!("--mode={mode}");
        cmd = cmd.arg(mode);
    }
    Ok(cmd.args(["-o", &output.to_string_lossy()]))
}

pub fn test_scan_command(device: &str, output: &Path, resolution: u32) -> Result<Cmd, String> {
    let options = ScanOptions {
        resolution,
        ..ScanOptions::default()
    };
    scan_command(device, output, &options)
}

pub fn run_scan(
    runner: &dyn Runner,
    device: &str,
    output: &Path,
    options: &ScanOptions,
) -> Result<String, String> {
    let cmd = scan_command(device, output, options)?;
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
            "اسکن با موفقیت ذخیره شد ({} بایت).",
            metadata.len()
        )),
        Ok(_) => Err("فایل اسکن ساخته شد اما خالی است.".into()),
        Err(error) => Err(format!(
            "فایل خروجی «{}» ساخته نشد: {error}",
            output.display()
        )),
    }
}

pub fn run_test_scan(
    runner: &dyn Runner,
    device: &str,
    output: &Path,
    resolution: u32,
) -> Result<String, String> {
    let options = ScanOptions {
        resolution,
        ..ScanOptions::default()
    };
    run_scan(runner, device, output, &options)
}

pub fn default_scan_path(format: ScanFormat) -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let documents = home
        .as_ref()
        .map(|home| home.join("Documents"))
        .filter(|path| path.is_dir());
    let directory = documents.or(home).unwrap_or_else(std::env::temp_dir);
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    directory.join(format!(
        "linuprint-scan-{timestamp}-{}.{}",
        std::process::id(),
        format.extension()
    ))
}

pub fn save_scan(source: &Path, destination: &Path) -> Result<String, String> {
    if source == destination {
        return Ok(format!(
            "اسکن از قبل در «{}» ذخیره شده است.",
            source.display()
        ));
    }
    if !source.is_file() {
        return Err(format!("فایل اسکن «{}» پیدا نشد.", source.display()));
    }
    if let Some(parent) = destination.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("ساخت پوشه «{}» ناموفق بود: {error}", parent.display()))?;
        }
    }
    std::fs::copy(source, destination).map_err(|error| {
        format!(
            "ذخیره اسکن در «{}» ناموفق بود: {error}",
            destination.display()
        )
    })?;
    Ok(format!("اسکن در «{}» ذخیره شد.", destination.display()))
}
