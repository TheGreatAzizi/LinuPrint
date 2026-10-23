use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};

use crate::backend::diagnostics::{format_report, run_diagnostics, CheckState};
use crate::backend::drivers::{recommend_print_package, recommend_scanner_package};
use crate::backend::packages::{enable_services, install_packages};
use crate::backend::printers::{
    cancel_all_jobs, cancel_job, discover_printers, friendly_name_from_uri, list_jobs, list_queues,
    print_file, provision_printer, remove_printer, set_default_printer, set_queue_enabled,
    test_print, PrintOptions, PrinterOrigin, PrinterTransport,
};
use crate::backend::scanners::{
    default_scan_path, discover_scanners, run_scan, ScanColorMode, ScanFormat, ScanOptions,
};
use crate::backend::shell::SystemRunner;

#[derive(Debug, Parser)]
#[command(
    name = "linuprint",
    version,
    about = "ویزارد چاپگر و اسکنر برای آرچ لینوکس"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// بررسی وضعیت CUPS، Avahi، SANE و دسترسی‌ها
    Diagnose {
        /// فعال‌سازی سرویس‌های لازم
        #[arg(long)]
        fix: bool,
    },
    /// مدیریت چاپگرها
    Printers {
        #[command(subcommand)]
        command: PrinterCommands,
    },
    /// مدیریت اسکنرها
    Scanners {
        #[command(subcommand)]
        command: ScannerCommands,
    },
    /// فعال‌سازی cups.socket و avahi-daemon
    Services,
    /// نصب بسته‌های رسمی با pacman
    Install {
        #[arg(required = true, num_args = 1..)]
        packages: Vec<String>,
    },
    /// اجرای ویزارد گرافیکی (نیازمند ساخت با ویژگی gui)
    Gui,
}

#[derive(Debug, Subcommand)]
enum PrinterCommands {
    /// فهرست چاپگرهای کشف‌شده و نصب‌شده
    List,
    /// افزودن چاپگر با تشخیص خودکار روش راه‌اندازی و درایور
    Add {
        /// آدرس دستگاه، مثلاً ipp://192.168.1.20/ipp/print یا usb://HP/...
        #[arg(long)]
        uri: String,
        /// نام صف در CUPS
        #[arg(long)]
        name: String,
        /// مدل درایور؛ خالی یعنی انتخاب خودکار (driverless یا نصب درایور سازنده)
        #[arg(long)]
        model: Option<String>,
        /// چاپگر پیش‌فرض نشود
        #[arg(long)]
        no_default: bool,
    },
    /// ارسال صفحه آزمایشی
    Test { queue: String },
    /// حذف صف چاپگر
    Remove { queue: String },
    /// تنظیم چاپگر پیش‌فرض
    Default { queue: String },
    /// توقف موقت صف
    Pause { queue: String },
    /// فعال‌سازی دوباره صف
    Resume { queue: String },
    /// چاپ یک فایل
    Print {
        /// نام صف چاپ
        queue: String,
        /// مسیر فایل برای چاپ
        file: PathBuf,
        /// تعداد نسخه
        #[arg(long, default_value_t = 1)]
        copies: u32,
        /// محدوده صفحات، مثلاً 1-3,5
        #[arg(long)]
        pages: Option<String>,
        /// چاپ دوطرفه
        #[arg(long)]
        duplex: bool,
        /// تناسب محتوا با اندازه صفحه
        #[arg(long)]
        fit: bool,
        /// اندازه کاغذ، مثلاً A4 یا Letter
        #[arg(long)]
        media: Option<String>,
    },
    /// فهرست کارهای در صف
    Jobs {
        #[arg(long)]
        queue: Option<String>,
    },
    /// لغو یک کار با شناسه
    Cancel { job: String },
    /// لغو همه کارهای یک صف
    CancelAll { queue: String },
}

#[derive(Debug, Subcommand)]
enum ScannerCommands {
    /// فهرست اسکنرهای شناسایی‌شده
    List,
    /// اسکن و ذخیره فایل
    Scan {
        /// شناسه دستگاه از خروجی scanners list
        #[arg(long)]
        device: String,
        /// مسیر فایل خروجی؛ پیش‌فرض در پوشه Documents
        #[arg(long)]
        output: Option<PathBuf>,
        /// رزولوشن اسکن
        #[arg(long, default_value_t = 150)]
        resolution: u32,
        /// قالب فایل خروجی
        #[arg(long, value_enum, default_value = "png")]
        format: ScanFormatArg,
        /// حالت رنگ
        #[arg(long, value_enum, default_value = "auto")]
        mode: ScanModeArg,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ScanFormatArg {
    Png,
    Jpeg,
    Pdf,
}

impl From<ScanFormatArg> for ScanFormat {
    fn from(value: ScanFormatArg) -> Self {
        match value {
            ScanFormatArg::Png => Self::Png,
            ScanFormatArg::Jpeg => Self::Jpeg,
            ScanFormatArg::Pdf => Self::Pdf,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ScanModeArg {
    Auto,
    Color,
    Gray,
    Lineart,
}

impl From<ScanModeArg> for ScanColorMode {
    fn from(value: ScanModeArg) -> Self {
        match value {
            ScanModeArg::Auto => Self::Auto,
            ScanModeArg::Color => Self::Color,
            ScanModeArg::Gray => Self::Gray,
            ScanModeArg::Lineart => Self::Lineart,
        }
    }
}

pub fn run() -> ExitCode {
    let cli = Cli::parse();
    match dispatch(cli) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("خطا: {error}");
            ExitCode::FAILURE
        }
    }
}

fn dispatch(cli: Cli) -> Result<u8, String> {
    let runner = SystemRunner;
    match cli.command {
        Commands::Diagnose { fix } => {
            let mut checks = run_diagnostics(&runner);
            if fix {
                match enable_services(&runner, false) {
                    Ok(message) => println!("{message}"),
                    Err(error) => eprintln!("{error}"),
                }
                checks = run_diagnostics(&runner);
            }
            println!("{}", format_report(&checks));
            let errors = checks
                .iter()
                .filter(|check| check.state == CheckState::Error)
                .count();
            Ok(if errors == 0 { 0 } else { 1 })
        }
        Commands::Printers { command } => match command {
            PrinterCommands::List => {
                print_discovered_printers();
                print_installed_printers();
                Ok(0)
            }
            PrinterCommands::Add {
                uri,
                name,
                model,
                no_default,
            } => {
                let printer_name = if uri.starts_with("usb://") || uri.starts_with("hp:/") {
                    friendly_name_from_uri(&uri)
                } else {
                    name.clone()
                };
                let result = provision_printer(
                    &runner,
                    &name,
                    &uri,
                    &printer_name,
                    model.as_deref(),
                    !no_default,
                    false,
                )?;
                println!("چاپگر «{name}» اضافه شد (درایور: {}).", result.model);
                for line in result.log {
                    println!("{line}");
                }
                println!("برای آزمایش: linuprint printers test {name}");
                Ok(0)
            }
            PrinterCommands::Test { queue } => {
                println!("{}", test_print(&runner, &queue)?);
                Ok(0)
            }
            PrinterCommands::Remove { queue } => {
                println!("{}", remove_printer(&runner, &queue, false)?);
                Ok(0)
            }
            PrinterCommands::Default { queue } => {
                println!("{}", set_default_printer(&runner, &queue, false)?);
                Ok(0)
            }
            PrinterCommands::Pause { queue } => {
                println!("{}", set_queue_enabled(&runner, &queue, false, false)?);
                Ok(0)
            }
            PrinterCommands::Resume { queue } => {
                println!("{}", set_queue_enabled(&runner, &queue, true, false)?);
                Ok(0)
            }
            PrinterCommands::Print {
                queue,
                file,
                copies,
                pages,
                duplex,
                fit,
                media,
            } => {
                let options = PrintOptions {
                    copies,
                    pages,
                    duplex,
                    fit_to_page: fit,
                    media,
                };
                println!("{}", print_file(&runner, &queue, &file, &options)?);
                Ok(0)
            }
            PrinterCommands::Jobs { queue } => {
                print_jobs(queue.as_deref())?;
                Ok(0)
            }
            PrinterCommands::Cancel { job } => {
                println!("{}", cancel_job(&runner, &job)?);
                Ok(0)
            }
            PrinterCommands::CancelAll { queue } => {
                println!("{}", cancel_all_jobs(&runner, &queue, false)?);
                Ok(0)
            }
        },
        Commands::Scanners { command } => match command {
            ScannerCommands::List => {
                print_scanners();
                Ok(0)
            }
            ScannerCommands::Scan {
                device,
                output,
                resolution,
                format,
                mode,
            } => {
                let format = ScanFormat::from(format);
                let output = output.unwrap_or_else(|| default_scan_path(format));
                let options = ScanOptions {
                    resolution,
                    format,
                    color_mode: mode.into(),
                };
                println!("{}", run_scan(&runner, &device, &output, &options)?);
                println!("فایل: {}", output.display());
                Ok(0)
            }
        },
        Commands::Services => {
            println!("{}", enable_services(&runner, false)?);
            Ok(0)
        }
        Commands::Install { packages } => {
            println!("{}", install_packages(&runner, &packages, false)?);
            Ok(0)
        }
        Commands::Gui => run_gui(),
    }
}

fn print_discovered_printers() {
    let discovery = discover_printers(&SystemRunner);
    println!("چاپگرهای کشف‌شده:");
    if discovery.printers.is_empty() {
        println!("  (هیچ چاپگری پیدا نشد)");
    }
    for printer in &discovery.printers {
        let transport = match printer.transport {
            PrinterTransport::Usb => "USB",
            PrinterTransport::Network => "شبکه",
            PrinterTransport::Other => "سایر",
        };
        let origin = match printer.origin {
            PrinterOrigin::CupsDevices => "CUPS",
            PrinterOrigin::Avahi => "Avahi",
        };
        let mode = if printer.driverless {
            "driverless"
        } else {
            "نیازمند درایور"
        };
        println!(
            "  - {} [{} / {} / {}]\n    {}\n    نام پیشنهادی صف: {}",
            printer.name, transport, origin, mode, printer.uri, printer.queue_name
        );
        if !printer.driverless {
            if let Some(recommendation) = recommend_print_package(&printer.detail) {
                println!(
                    "    بسته پیشنهادی: {} ({})",
                    recommendation.package, recommendation.reason
                );
            }
        }
    }
    for note in &discovery.notes {
        println!("  نکته: {note}");
    }
}

fn print_installed_printers() {
    println!("\nچاپگرهای نصب‌شده:");
    match list_queues(&SystemRunner) {
        Ok(queues) if queues.is_empty() => println!("  (هیچ صفی وجود ندارد)"),
        Ok(queues) => {
            for queue in queues {
                let suffix = if queue.is_default {
                    " — پیش‌فرض"
                } else {
                    ""
                };
                println!("  - {}{} ({})", queue.name, suffix, queue.state);
            }
        }
        Err(error) => println!("  {error}"),
    }
}

fn print_jobs(queue: Option<&str>) -> Result<(), String> {
    let jobs = list_jobs(&SystemRunner, queue)?;
    if jobs.is_empty() {
        println!("هیچ کاری در صف نیست.");
        return Ok(());
    }
    println!("کارهای در صف:");
    for job in jobs {
        println!(
            "  - {} | صف: {} | کاربر: {} | اندازه: {} | زمان: {}",
            job.id, job.queue, job.user, job.size, job.submitted
        );
    }
    Ok(())
}

fn print_scanners() {
    let discovery = discover_scanners(&SystemRunner);
    println!("اسکنرهای شناسایی‌شده:");
    if discovery.scanners.is_empty() {
        println!("  (هیچ اسکنری پیدا نشد)");
    }
    for scanner in &discovery.scanners {
        let kind = if scanner.network {
            "شبکه"
        } else {
            "محلی"
        };
        println!(
            "  - {} [{}]\n    {}",
            scanner.description, kind, scanner.device
        );
        if let Some(recommendation) =
            recommend_scanner_package(&scanner.description, &scanner.device)
        {
            let location = if recommendation.official {
                "مخزن رسمی"
            } else {
                "AUR"
            };
            println!(
                "    بسته پیشنهادی: {} — {} ({})",
                recommendation.package, recommendation.reason, location
            );
        }
    }
    for finding in &discovery.usb_findings {
        println!("  USB: {finding}");
    }
    for note in &discovery.notes {
        println!("  نکته: {note}");
    }
}

fn run_gui() -> Result<u8, String> {
    #[cfg(feature = "gui")]
    {
        crate::gui::run();
        Ok(0)
    }
    #[cfg(not(feature = "gui"))]
    {
        Err("این نسخه بدون GTK ساخته شده است. با «cargo build --release --features gui» دوباره بسازید.".to_string())
    }
}
