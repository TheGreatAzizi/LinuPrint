use crate::backend::packages::{aur_helper, package_installed};
use crate::backend::shell::{detect_privilege, Cmd, Runner};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckState {
    Ok,
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub id: &'static str,
    pub title: String,
    pub state: CheckState,
    pub detail: String,
    pub fix: Option<String>,
}

fn ok(id: &'static str, title: impl Into<String>, detail: impl Into<String>) -> Check {
    Check {
        id,
        title: title.into(),
        state: CheckState::Ok,
        detail: detail.into(),
        fix: None,
    }
}

fn info(id: &'static str, title: impl Into<String>, detail: impl Into<String>) -> Check {
    Check {
        id,
        title: title.into(),
        state: CheckState::Info,
        detail: detail.into(),
        fix: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn package_check(
    runner: &dyn Runner,
    id: &'static str,
    title: &str,
    package: &str,
    missing_state: CheckState,
    missing_detail: &str,
    fix_command: &str,
) -> Check {
    match package_installed(runner, package) {
        Ok(true) => ok(id, title, format!("بسته {package} نصب است.")),
        Ok(false) => Check {
            id,
            title: title.to_string(),
            state: missing_state,
            detail: format!("بسته {package} نصب نیست. {missing_detail}"),
            fix: Some(fix_command.to_string()),
        },
        Err(error) => Check {
            id,
            title: title.to_string(),
            state: CheckState::Warning,
            detail: error,
            fix: None,
        },
    }
}

pub fn nsswitch_has_mdns(contents: &str) -> bool {
    contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .find_map(|line| {
            let mut parts = line.split_whitespace();
            if parts.next().map(|database| database.trim_end_matches(':')) == Some("hosts") {
                let has_mdns = parts.any(|library| library.starts_with("mdns"));
                if has_mdns {
                    Some(())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .is_some()
}

pub fn run_diagnostics(runner: &dyn Runner) -> Vec<Check> {
    let mut checks = Vec::new();
    let has_pacman = runner.exists("pacman");

    if has_pacman {
        checks.push(package_check(
            runner,
            "pkg-cups",
            "بسته CUPS",
            "cups",
            CheckState::Error,
            "بدون CUPS امکان افزودن چاپگر وجود ندارد.",
            "sudo pacman -S cups",
        ));
        checks.push(package_check(
            runner,
            "pkg-cups-filters",
            "فیلترهای CUPS",
            "cups-filters",
            CheckState::Warning,
            "برای برخی چاپگرهای USB به فیلترها نیاز است.",
            "sudo pacman -S cups-filters",
        ));
        checks.push(package_check(
            runner,
            "pkg-avahi",
            "بسته Avahi",
            "avahi",
            CheckState::Warning,
            "برای کشف خودکار چاپگرهای شبکه لازم است.",
            "sudo pacman -S avahi",
        ));
        checks.push(package_check(
            runner,
            "pkg-nss-mdns",
            "بسته nss-mdns",
            "nss-mdns",
            CheckState::Warning,
            "برای حل نام‌های mDNS مثل printer.local لازم است.",
            "sudo pacman -S nss-mdns",
        ));
        checks.push(package_check(
            runner,
            "pkg-sane",
            "بسته SANE",
            "sane",
            CheckState::Warning,
            "بدون SANE اسکنر پشتیبانی نمی‌شود.",
            "sudo pacman -S sane",
        ));
        checks.push(package_check(
            runner,
            "pkg-sane-airscan",
            "اسکنر شبکه (sane-airscan)",
            "sane-airscan",
            CheckState::Info,
            "برای اسکنرهای شبکه eSCL/AirScan پیشنهاد می‌شود.",
            "sudo pacman -S sane-airscan",
        ));

        let nsswitch = runner
            .read_to_string("/etc/nsswitch.conf")
            .unwrap_or_default();
        if nsswitch.is_empty() {
            checks.push(Check {
                id: "nsswitch",
                title: "پیکربندی mDNS در nsswitch".into(),
                state: CheckState::Warning,
                detail: "فایل /etc/nsswitch.conf خوانده نشد؛ برای حل نام‌های .local خط mdns_minimal را به hosts اضافه کنید.".into(),
                fix: Some(
                    "hosts: mymachines mdns_minimal [NOTFOUND=return] resolve [!UNAVAIL=return] files myhostname dns"
                        .into(),
                ),
            });
        } else if nsswitch_has_mdns(&nsswitch) {
            checks.push(ok(
                "nsswitch",
                "پیکربندی mDNS در nsswitch",
                "mdns_minimal در خط hosts تنظیم شده است.",
            ));
        } else {
            checks.push(Check {
                id: "nsswitch",
                title: "پیکربندی mDNS در nsswitch".into(),
                state: CheckState::Warning,
                detail: "mdns_minimal در /etc/nsswitch.conf نیست؛ ممکن است چاپگرها با نام .local پیدا نشوند. استفاده از IP نیز کار می‌کند.".into(),
                fix: Some(
                    "خط hosts را به mymachines mdns_minimal [NOTFOUND=return] resolve [!UNAVAIL=return] files myhostname dns تغییر دهید."
                        .into(),
                ),
            });
        }
    } else {
        checks.push(info(
            "pacman",
            "مدیر بسته pacman",
            "pacman پیدا نشد؛ بررسی بسته‌ها انجام نشد. این ابزار برای آرچ لینوکس طراحی شده است.",
        ));
    }

    let tools = [
        (
            "tool-lpadmin",
            "lpadmin",
            "ابزار lpadmin",
            CheckState::Error,
            "فرمان مدیریت چاپگر پیدا نشد؛ بسته cups را نصب کنید.",
        ),
        (
            "tool-lpinfo",
            "lpinfo",
            "ابزار lpinfo",
            CheckState::Warning,
            "کشف دستگاه‌های چاپ با lpinfo انجام می‌شود.",
        ),
        (
            "tool-lpstat",
            "lpstat",
            "ابزار lpstat",
            CheckState::Warning,
            "فهرست چاپگرها با lpstat خوانده می‌شود.",
        ),
        (
            "tool-scanimage",
            "scanimage",
            "ابزار scanimage",
            CheckState::Warning,
            "برای اسکن باید بسته sane نصب باشد.",
        ),
        (
            "tool-avahi-browse",
            "avahi-browse",
            "ابزار avahi-browse",
            CheckState::Info,
            "کشف چاپگرهای شبکه با Avahi انجام می‌شود.",
        ),
    ];

    for (id, program, title, missing_state, detail) in tools {
        if runner.exists(program) {
            checks.push(ok(id, title, "در PATH موجود است."));
        } else {
            checks.push(Check {
                id,
                title: title.to_string(),
                state: missing_state,
                detail: detail.to_string(),
                fix: None,
            });
        }
    }

    if runner.exists("systemctl") {
        let cups_active = ["cups.socket", "cups.service"].iter().any(|service| {
            runner
                .run(&Cmd::new("systemctl").args(["is-active", service]))
                .map(|output| output.success())
                .unwrap_or(false)
        });
        checks.push(if cups_active {
            ok("svc-cups", "سرویس CUPS", "CUPS فعال است.")
        } else {
            Check {
                id: "svc-cups",
                title: "سرویس CUPS".into(),
                state: CheckState::Error,
                detail: "CUPS فعال نیست؛ بدون آن چاپگر اضافه نمی‌شود.".into(),
                fix: Some("systemctl enable --now cups.socket".into()),
            }
        });

        let avahi_active = runner
            .run(&Cmd::new("systemctl").args(["is-active", "avahi-daemon.service"]))
            .map(|output| output.success())
            .unwrap_or(false);
        checks.push(if avahi_active {
            ok("svc-avahi", "سرویس Avahi", "Avahi فعال است.")
        } else {
            Check {
                id: "svc-avahi",
                title: "سرویس Avahi".into(),
                state: CheckState::Warning,
                detail: "Avahi غیرفعال است؛ کشف خودکار چاپگرهای شبکه کار نمی‌کند.".into(),
                fix: Some("systemctl enable --now avahi-daemon.service".into()),
            }
        });
    } else {
        checks.push(info(
            "svc-cups",
            "سرویس‌های systemd",
            "systemctl پیدا نشد؛ وضعیت سرویس‌ها بررسی نشد.",
        ));
    }

    let privilege = detect_privilege(runner);
    if privilege.is_root {
        checks.push(ok(
            "privilege",
            "دسترسی مدیریتی",
            "برنامه با دسترسی root اجرا می‌شود.",
        ));
    } else if privilege.has_pkexec || privilege.has_sudo {
        let detail = if privilege.has_pkexec {
            "pkexec برای نمایش پنجره احراز هویت موجود است."
        } else {
            "sudo برای عملیات مدیریتی موجود است."
        };
        checks.push(ok("privilege", "دسترسی مدیریتی", detail));
    } else {
        checks.push(Check {
            id: "privilege",
            title: "دسترسی مدیریتی".into(),
            state: CheckState::Warning,
            detail: "pkexec یا sudo پیدا نشد؛ برای افزودن چاپگر به دسترسی root نیاز است.".into(),
            fix: Some("sudo pacman -S polkit".into()),
        });
    }

    let groups = runner
        .run(&Cmd::new("id").arg("-nG"))
        .ok()
        .map(|output| output.stdout)
        .unwrap_or_default();
    let has_print_groups = groups
        .split_whitespace()
        .any(|group| group == "scanner" || group == "lp");
    if has_print_groups {
        checks.push(info(
            "groups",
            "گروه‌های کاربر",
            "عضو گروه scanner یا lp هستید.",
        ));
    } else {
        checks.push(Check {
            id: "groups",
            title: "گروه‌های کاربر".into(),
            state: CheckState::Info,
            detail: "عضو scanner/lp نیستید؛ بیشتر اسکنرهای USB با ACL کار می‌کنند، اما برای برخی دستگاه‌های شبکه ممکن است لازم باشد.".into(),
            fix: Some("sudo usermod -aG scanner,lp $USER و سپس خروج و ورود مجدد".into()),
        });
    }

    match aur_helper(runner) {
        Some(helper) => checks.push(ok(
            "aur-helper",
            "کمک‌کننده AUR",
            format!("{helper} موجود است."),
        )),
        None => checks.push(info(
            "aur-helper",
            "کمک‌کننده AUR",
            "paru یا yay پیدا نشد؛ برخی درایورها فقط در AUR هستند.",
        )),
    }

    checks
}

pub fn format_report(checks: &[Check]) -> String {
    let mut output = String::new();
    output.push_str("گزارش وضعیت چاپ و اسکن\n");
    output.push_str("========================\n");
    for check in checks {
        let marker = match check.state {
            CheckState::Ok => "[OK]",
            CheckState::Info => "[i] ",
            CheckState::Warning => "[!] ",
            CheckState::Error => "[X] ",
        };
        output.push_str(&format!("{marker} {}: {}\n", check.title, check.detail));
        if let Some(fix) = &check.fix {
            output.push_str(&format!("       راه‌حل: {fix}\n"));
        }
    }
    let errors = checks
        .iter()
        .filter(|check| check.state == CheckState::Error)
        .count();
    let warnings = checks
        .iter()
        .filter(|check| check.state == CheckState::Warning)
        .count();
    output.push_str(&format!("\n{errors} خطا، {warnings} هشدار\n"));
    output
}
