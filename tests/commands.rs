mod support;

use std::path::Path;

use linuprint::backend::printers::{add_printer_command, provision_printer};
use linuprint::backend::scanners::test_scan_command;
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
