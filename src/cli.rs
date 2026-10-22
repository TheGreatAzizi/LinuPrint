use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::backend::diagnostics::{format_report, run_diagnostics, CheckState};
use crate::backend::drivers::{recommend_print_package, recommend_scanner_package};
use crate::backend::packages::{enable_services, install_packages};
use crate::backend::printers::{
    add_printer, discover_printers, list_queues, remove_printer, test_print, PrinterOrigin,
    PrinterTransport,
};
use crate::backend::scanners::{discover_scanners, run_test_scan};
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
    /// افزودن چاپگر
    Add {
        /// آدرس دستگاه، مثلاً ipp://192.168.1.20/ipp/print یا usb://HP/...
        #[arg(long)]
        uri: String,
        /// نام صف در CUPS
        #[arg(long)]
        name: String,
        /// مدل درایور؛ خالی یعنی driverless (everywhere)
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
}

#[derive(Debug, Subcommand)]
enum ScannerCommands {
    /// فهرست اسکنرهای شناسایی‌شده
    List,
    /// اسکن آزمایشی
    Scan {
        /// شناسه دستگاه از خروجی scanners list
        #[arg(long)]
        device: String,
        /// مسیر فایل PNG خروجی
        #[arg(long)]
        output: PathBuf,
        /// رزولوشن اسکن
        #[arg(long, default_value_t = 150)]
        resolution: u32,
    },
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
                let log = add_printer(&runner, &name, &uri, model.as_deref(), !no_default, false)?;
                println!("چاپگر «{name}» اضافه شد.");
                for line in log {
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
            } => {
                println!("{}", run_test_scan(&runner, &device, &output, resolution)?);
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
