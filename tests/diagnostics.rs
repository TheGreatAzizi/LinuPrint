mod support;

use linuprint::backend::diagnostics::{run_diagnostics, CheckState};
use support::{failure, FakeRunner};

fn healthy_runner() -> FakeRunner {
    let mut runner = FakeRunner::with_tools(&[
        "pacman",
        "systemctl",
        "lpadmin",
        "lpinfo",
        "lpstat",
        "scanimage",
        "avahi-browse",
        "pkexec",
        "id",
        "paru",
    ]);
    runner
        .with_file(
            "/etc/nsswitch.conf",
            "hosts: mymachines mdns_minimal [NOTFOUND=return] resolve files myhostname dns\n",
        )
        .on(
            "systemctl is-active cups.socket",
            support::success("active\n"),
        )
        .on(
            "systemctl is-active avahi-daemon.service",
            support::success("active\n"),
        )
        .on("id -u", support::success("1000\n"))
        .on("id -nG", support::success("wheel lp scanner\n"));
    runner
}

#[test]
fn healthy_system_has_no_errors_or_warnings() {
    let checks = run_diagnostics(&healthy_runner());
    let errors: Vec<_> = checks
        .iter()
        .filter(|check| check.state == CheckState::Error)
        .collect();
    let warnings: Vec<_> = checks
        .iter()
        .filter(|check| check.state == CheckState::Warning)
        .collect();
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
}

#[test]
fn missing_cups_is_reported_as_error() {
    let mut runner = healthy_runner();
    runner.on(
        "pacman -Qq cups",
        failure(1, "error: package 'cups' was not found\n"),
    );
    let checks = run_diagnostics(&runner);
    let cups = checks.iter().find(|check| check.id == "pkg-cups").unwrap();
    assert_eq!(cups.state, CheckState::Error);
    assert!(cups.fix.as_ref().unwrap().contains("pacman -S cups"));
}

#[test]
fn inactive_services_are_reported() {
    let mut runner = healthy_runner();
    runner
        .on("systemctl is-active cups.socket", failure(3, ""))
        .on("systemctl is-active cups.service", failure(3, ""))
        .on("systemctl is-active avahi-daemon.service", failure(3, ""));
    let checks = run_diagnostics(&runner);
    assert_eq!(
        checks
            .iter()
            .find(|check| check.id == "svc-cups")
            .unwrap()
            .state,
        CheckState::Error
    );
    assert_eq!(
        checks
            .iter()
            .find(|check| check.id == "svc-avahi")
            .unwrap()
            .state,
        CheckState::Warning
    );
}

#[test]
fn missing_mdns_configuration_is_reported() {
    let mut runner = healthy_runner();
    runner.with_file("/etc/nsswitch.conf", "hosts: files dns\n");
    let checks = run_diagnostics(&runner);
    let nsswitch = checks.iter().find(|check| check.id == "nsswitch").unwrap();
    assert_eq!(nsswitch.state, CheckState::Warning);
    assert!(nsswitch.detail.contains("mdns_minimal"));
}

#[test]
fn missing_privilege_tools_are_reported() {
    let mut runner = FakeRunner::with_tools(&["pacman", "systemctl"]);
    runner.with_file("/etc/nsswitch.conf", "hosts: files dns\n");
    let checks = run_diagnostics(&runner);
    assert_eq!(
        checks
            .iter()
            .find(|check| check.id == "privilege")
            .unwrap()
            .state,
        CheckState::Warning
    );
    assert_eq!(
        checks
            .iter()
            .find(|check| check.id == "tool-lpadmin")
            .unwrap()
            .state,
        CheckState::Error
    );
}
