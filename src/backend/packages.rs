use crate::backend::shell::{describe_failure, run_privileged, Cmd, Runner};

pub const BASE_PACKAGES: &[&str] = &[
    "cups",
    "cups-filters",
    "avahi",
    "nss-mdns",
    "sane",
    "sane-airscan",
    "polkit",
    "xdg-utils",
];

pub fn package_installed(runner: &dyn Runner, package: &str) -> Result<bool, String> {
    if !runner.exists("pacman") {
        return Err("pacman پیدا نشد؛ این ابزار برای سیستم‌های آرچ‌بیس طراحی شده است.".into());
    }
    let cmd = Cmd::new("pacman").args(["-Qq", package]);
    let output = runner
        .run(&cmd)
        .map_err(|error| format!("بررسی بسته «{package}» ناموفق بود: {error}"))?;
    Ok(output.success())
}

pub fn missing_base_packages(runner: &dyn Runner) -> Result<Vec<&'static str>, String> {
    let mut missing = Vec::new();
    for package in BASE_PACKAGES {
        if !package_installed(runner, package)? {
            missing.push(*package);
        }
    }
    Ok(missing)
}

fn valid_package_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '@' | '.' | '_' | '+' | '-'))
}

pub fn install_packages(
    runner: &dyn Runner,
    packages: &[String],
    prefer_pkexec: bool,
) -> Result<String, String> {
    if packages.is_empty() {
        return Err("هیچ بسته‌ای برای نصب مشخص نشده است.".into());
    }
    for package in packages {
        if !valid_package_name(package) {
            return Err(format!("نام بسته نامعتبر است: {package}"));
        }
    }
    if !runner.exists("pacman") {
        return Err("pacman پیدا نشد.".into());
    }
    let cmd = Cmd::new("pacman")
        .args(["-S", "--needed", "--noconfirm"])
        .args(packages.iter().cloned());
    let output = run_privileged(runner, cmd.clone(), prefer_pkexec)?;
    if !output.success() {
        return Err(describe_failure(&cmd, &output));
    }
    Ok(format!("نصب انجام شد: {}", packages.join(", ")))
}

pub fn install_base_packages(runner: &dyn Runner, prefer_pkexec: bool) -> Result<String, String> {
    let missing = missing_base_packages(runner)?;
    if missing.is_empty() {
        return Ok("همه بسته‌های پایه از قبل نصب هستند.".into());
    }
    let packages: Vec<String> = missing
        .iter()
        .map(|package| (*package).to_string())
        .collect();
    install_packages(runner, &packages, prefer_pkexec)
}

pub fn enable_services(runner: &dyn Runner, prefer_pkexec: bool) -> Result<String, String> {
    if !runner.exists("systemctl") {
        return Err("systemctl پیدا نشد؛ این ابزار به systemd نیاز دارد.".into());
    }
    let cmd =
        Cmd::new("systemctl").args(["enable", "--now", "cups.socket", "avahi-daemon.service"]);
    let output = run_privileged(runner, cmd.clone(), prefer_pkexec)?;
    if !output.success() {
        return Err(describe_failure(&cmd, &output));
    }
    Ok("سرویس‌های cups.socket و avahi-daemon فعال شدند.".into())
}

pub fn aur_helper(runner: &dyn Runner) -> Option<&'static str> {
    if runner.exists("paru") {
        Some("paru")
    } else if runner.exists("yay") {
        Some("yay")
    } else {
        None
    }
}
