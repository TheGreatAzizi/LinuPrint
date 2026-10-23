mod support;

use std::path::Path;

use linuprint::backend::printers::{
    add_printer_command, list_jobs, print_file_command, provision_printer, test_page_pdf,
    PrintOptions,
};
use linuprint::backend::scanners::{
    scan_command, test_scan_command, ScanColorMode, ScanFormat, ScanOptions,
};
use linuprint::backend::shell::{wrap_privileged, Cmd, PrivilegeTools};

use support::{failure, success, FakeRunner};

const HP_1320_CATALOG: &str = "\
everywhere IPP Everywhere
raw Raw Queue
drv:///hp/hpcups.drv/hp-laserjet_1300_series.ppd HP LaserJet 1300 Series, hpcups 3.26.6
drv:///hp/hpcups.drv/hp-laserjet_1320_series-pcl3.ppd HP LaserJet 1320 Series pcl3, hpcups 3.26.6
drv:///hp/hpcups.drv/hp-laserjet_1320n.ppd HP LaserJet 1320n, hpcups 3.26.6
drv:///hp/hpcups.drv/hp-laserjet_1320.ppd HP LaserJet 1320, hpcups 3.26.6
";

#[test]
fn wraps_privileged_commands_for_gui_and_cli() {
    let cmd = Cmd::new("lpadmin").args(["-p", "hp", "-E"]);
    let tools = PrivilegeTools {
        is_root: false,
        has_pkexec: true,
        has_sudo: true,
    };

    let gui = wrap_privileged(cmd.clone(), tools, true).unwrap();
    assert_eq!(gui.program, "pkexec");
    assert_eq!(
        gui.args.iter().map(String::as_str).collect::<Vec<_>>(),
        vec!["lpadmin", "-p", "hp", "-E"]
    );

    let cli = wrap_privileged(cmd.clone(), tools, false).unwrap();
    assert_eq!(cli.program, "sudo");

    let root = wrap_privileged(
        cmd.clone(),
        PrivilegeTools {
            is_root: true,
            has_pkexec: false,
            has_sudo: false,
        },
        true,
    )
    .unwrap();
    assert_eq!(root.program, "lpadmin");

    assert!(wrap_privileged(
        cmd,
        PrivilegeTools {
            is_root: false,
            has_pkexec: false,
            has_sudo: false,
        },
        true,
    )
    .is_err());
}

#[test]
fn builds_lpadmin_commands_with_driverless_default() {
    let commands = add_printer_command(
        "hp-officejet",
        "ipp://192.168.1.20:631/ipp/print",
        None,
        true,
    )
    .unwrap();
    assert_eq!(commands.len(), 4);
    assert_eq!(commands[0].program, "lpadmin");
    assert_eq!(
        commands[0]
            .args
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec![
            "-p",
            "hp-officejet",
            "-E",
            "-v",
            "ipp://192.168.1.20:631/ipp/print",
            "-m",
            "everywhere"
        ]
    );
    assert_eq!(
        commands[1]
            .args
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["-d", "hp-officejet"]
    );
    assert_eq!(commands[2].program, "cupsenable");
    assert_eq!(commands[3].program, "cupsaccept");
}

#[test]
fn builds_lpadmin_commands_with_manual_model() {
    let commands = add_printer_command(
        "laser",
        "usb://HP/LaserJet",
        Some("drv:///hp/hpcups.drv/hp-laserjet.drv"),
        false,
    )
    .unwrap();
    assert_eq!(commands.len(), 3);
    assert_eq!(commands[0].args[6], "drv:///hp/hpcups.drv/hp-laserjet.drv");
}

#[test]
fn rejects_invalid_printer_arguments() {
    assert!(add_printer_command("bad/name", "ipp://host/ipp/print", None, false).is_err());
    assert!(add_printer_command("ok", "-bad", None, false).is_err());
    assert!(add_printer_command("ok", "ipp://host", Some("-bad"), false).is_err());
    assert!(add_printer_command("", "ipp://host", None, false).is_err());
}

#[test]
fn provisions_usb_printer_with_auto_driver_install() {
    let mut runner = FakeRunner::with_tools(&[
        "id",
        "pkexec",
        "pacman",
        "lpinfo",
        "lpadmin",
        "cupsenable",
        "cupsaccept",
    ]);
    runner.on("pacman -Qq hplip", failure(1, "package not found"));
    runner.on("lpinfo -m", success(HP_1320_CATALOG));

    let result = provision_printer(
        &runner,
        "hp-laserjet-1320-series",
        "usb://HP/LaserJet%201320%20series?serial=00CNGW53YHLB",
        "HP LaserJet 1320 series",
        None,
        true,
        true,
    )
    .unwrap();

    assert_eq!(result.model, "drv:///hp/hpcups.drv/hp-laserjet_1320.ppd");
    assert_eq!(result.installed_packages, vec!["hplip"]);
    assert!(result.log.iter().any(|line| line.contains("hplip")));
}

#[test]
fn skips_package_install_when_driver_is_present() {
    let mut runner = FakeRunner::with_tools(&[
        "id",
        "pkexec",
        "pacman",
        "lpinfo",
        "lpadmin",
        "cupsenable",
        "cupsaccept",
    ]);
    runner.on("pacman -Qq hplip", success("hplip\n"));
    runner.on("lpinfo -m", success(HP_1320_CATALOG));

    let result = provision_printer(
        &runner,
        "hp-laserjet-1320-series",
        "usb://HP/LaserJet%201320%20series?serial=00CNGW53YHLB",
        "HP LaserJet 1320 series",
        None,
        true,
        true,
    )
    .unwrap();

    assert!(result.installed_packages.is_empty());
}

#[test]
fn uses_driverless_for_ipp_without_installing_packages() {
    let runner = FakeRunner::with_tools(&["id", "pkexec", "lpadmin", "cupsenable", "cupsaccept"]);

    let result = provision_printer(
        &runner,
        "officejet",
        "ipps://printer.local/ipp/print",
        "OfficeJet Pro",
        None,
        true,
        true,
    )
    .unwrap();

    assert_eq!(result.model, "everywhere");
    assert!(result.installed_packages.is_empty());
}

#[test]
fn builds_scanimage_command() {
    let cmd = test_scan_command(
        "hpaio:/usb/OfficeJet?serial=1",
        Path::new("/tmp/scan.png"),
        150,
    )
    .unwrap();
    assert_eq!(cmd.program, "scanimage");
    assert_eq!(
        cmd.args.iter().map(String::as_str).collect::<Vec<_>>(),
        vec![
            "-d",
            "hpaio:/usb/OfficeJet?serial=1",
            "--format=png",
            "--resolution=150",
            "-o",
            "/tmp/scan.png"
        ]
    );
    assert!(test_scan_command("-bad", Path::new("/tmp/scan.png"), 150).is_err());
    assert!(test_scan_command("dev", Path::new("/tmp/scan.png"), 10).is_err());
    assert!(test_scan_command("dev", Path::new("/tmp/scan.png"), 5000).is_err());
}

#[test]
fn builds_print_file_command_with_options() {
    let options = PrintOptions {
        copies: 2,
        pages: Some("1-3,5".to_string()),
        duplex: true,
        fit_to_page: true,
        media: Some("A4".to_string()),
    };
    let cmd =
        print_file_command("hp-laserjet-1320", Path::new("/tmp/report.pdf"), &options).unwrap();

    assert_eq!(cmd.program, "lp");
    assert_eq!(
        cmd.args.iter().map(String::as_str).collect::<Vec<_>>(),
        vec![
            "-d",
            "hp-laserjet-1320",
            "-n",
            "2",
            "-o",
            "sides=two-sided-long-edge",
            "-o",
            "fit-to-page",
            "-o",
            "page-ranges=1-3,5",
            "-o",
            "media=A4",
            "/tmp/report.pdf"
        ]
    );
}

#[test]
fn rejects_invalid_print_options() {
    let base = PrintOptions::default();
    assert!(print_file_command(
        "hp",
        Path::new("/tmp/report.pdf"),
        &PrintOptions {
            copies: 0,
            ..base.clone()
        }
    )
    .is_err());
    assert!(print_file_command(
        "hp",
        Path::new("/tmp/report.pdf"),
        &PrintOptions {
            pages: Some("3-1".to_string()),
            ..base.clone()
        }
    )
    .is_err());
    assert!(print_file_command(
        "hp",
        Path::new("/tmp/report.pdf"),
        &PrintOptions {
            pages: Some("1,,2".to_string()),
            ..base.clone()
        }
    )
    .is_err());
    assert!(print_file_command(
        "hp",
        Path::new("/tmp/report.pdf"),
        &PrintOptions {
            media: Some("-bad".to_string()),
            ..base
        }
    )
    .is_err());
}

#[test]
fn builds_colored_jpeg_scanimage_command() {
    let options = ScanOptions {
        resolution: 300,
        format: ScanFormat::Jpeg,
        color_mode: ScanColorMode::Color,
    };
    let cmd = scan_command(
        "escl:http://scanner.local",
        Path::new("/tmp/scan.jpg"),
        &options,
    )
    .unwrap();

    assert_eq!(
        cmd.args.iter().map(String::as_str).collect::<Vec<_>>(),
        vec![
            "-d",
            "escl:http://scanner.local",
            "--format=jpeg",
            "--resolution=300",
            "--mode=Color",
            "-o",
            "/tmp/scan.jpg"
        ]
    );
}

#[test]
fn builds_valid_test_page_pdf() {
    let bytes = test_page_pdf("hp-laserjet-1320");
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert!(text.starts_with("%PDF-1.4"));
    assert!(text.contains("LinuPrint Test Page"));
    assert!(text.contains("Queue: hp-laserjet-1320"));
    assert!(text.contains("If you can read this page, printing is working."));
    assert!(text.ends_with("%%EOF\n"));

    let startxref_marker = "startxref\n";
    let start = text.rfind(startxref_marker).unwrap() + startxref_marker.len();
    let end = text[start..].find('\n').unwrap() + start;
    let offset: usize = text[start..end].parse().unwrap();
    assert!(bytes[offset..].starts_with(b"xref\n"));
}

#[test]
fn lists_jobs_for_selected_queue() {
    let mut runner = FakeRunner::with_tools(&["lpstat"]);
    runner.on(
        "lpstat -o hp-laserjet-1320",
        success("hp-laserjet-1320-42 azizi 1024 Fri 23 Oct 2026 01:22:33 AM CET\n"),
    );

    let jobs = list_jobs(&runner, Some("hp-laserjet-1320")).unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].id, "hp-laserjet-1320-42");
    assert_eq!(jobs[0].queue, "hp-laserjet-1320");
    assert_eq!(jobs[0].user, "azizi");
    assert_eq!(jobs[0].size, "1024");
}
