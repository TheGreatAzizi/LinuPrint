use crate::backend::avahi::parse_avahi;
use crate::backend::shell::{describe_failure, run_privileged, Cmd, Runner};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrinterTransport {
    Usb,
    Network,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrinterOrigin {
    CupsDevices,
    Avahi,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredPrinter {
    pub name: String,
    pub uri: String,
    pub transport: PrinterTransport,
    pub origin: PrinterOrigin,
    pub driverless: bool,
    pub detail: String,
    pub queue_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrinterDiscovery {
    pub printers: Vec<DiscoveredPrinter>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledPrinter {
    pub name: String,
    pub state: String,
    pub is_default: bool,
}

pub fn parse_lpinfo(output: &str) -> Vec<DiscoveredPrinter> {
    let mut printers: Vec<DiscoveredPrinter> = Vec::new();
    for line in output.lines() {
        let mut parts = line.split_whitespace();
        let Some(kind) = parts.next() else {
            continue;
        };
        let Some(uri) = parts.next() else {
            continue;
        };
        if !uri.contains("://") && !uri.starts_with("hp:/") {
            continue;
        }
        if printers.iter().any(|printer| printer.uri == uri) {
            continue;
        }
        let transport = if kind == "direct" {
            if uri.starts_with("usb://") || uri.starts_with("hp:/usb") {
                PrinterTransport::Usb
            } else {
                PrinterTransport::Other
            }
        } else if kind == "network" {
            PrinterTransport::Network
        } else {
            PrinterTransport::Other
        };
        let name = friendly_name_from_uri(uri);
        let driverless =
            uri.starts_with("ipp://") || uri.starts_with("ipps://") || uri.starts_with("dnssd://");
        let queue_name = suggest_queue_name_from_label(&name);
        printers.push(DiscoveredPrinter {
            name,
            uri: uri.to_string(),
            transport,
            origin: PrinterOrigin::CupsDevices,
            driverless,
            detail: format!("{kind} — {uri}"),
            queue_name,
        });
    }
    printers
}

pub fn friendly_name_from_uri(uri: &str) -> String {
    let decoded = percent_decode(uri);
    if let Some(rest) = decoded.strip_prefix("usb://") {
        return clean_label(rest.split('?').next().unwrap_or(rest).replace('/', " "));
    }
    if let Some(rest) = decoded.strip_prefix("hp:/") {
        let rest = rest.strip_prefix("usb/").unwrap_or(rest);
        return clean_label(rest.split('?').next().unwrap_or(rest).replace('/', " "));
    }
    let rest = decoded
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(decoded.as_str());
    let without_query = rest.split('?').next().unwrap_or(rest);
    let host_part = without_query.split('/').next().unwrap_or(without_query);
    let host = host_part.rsplit('@').next().unwrap_or(host_part);
    let host = host.split("._").next().unwrap_or(host);
    let host = strip_port(host);
    clean_label(host.to_string())
}

fn strip_port(host: &str) -> &str {
    if host.starts_with('[') {
        return host;
    }
    match host.rsplit_once(':') {
        Some((name, port))
            if !name.contains(':')
                && !port.is_empty()
                && port.chars().all(|c| c.is_ascii_digit()) =>
        {
            name
        }
        _ => host,
    }
}

fn clean_label(value: String) -> String {
    let trimmed = value.trim_matches(['/', ' ']).to_string();
    let collapsed = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        "چاپگر".to_string()
    } else {
        collapsed
    }
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
            {
                output.push(high * 16 + low);
                index += 3;
                continue;
            }
        }
        output.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub fn suggest_queue_name_from_label(label: &str) -> String {
    let mut output = String::new();
    let mut last_was_dash = false;
    for character in label.chars() {
        if character.is_ascii_alphanumeric() {
            output.push(character.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash && !output.is_empty() {
            output.push('-');
            last_was_dash = true;
        }
    }
    while output.ends_with('-') {
        output.pop();
    }
    let truncated: String = output.chars().take(50).collect();
    let truncated = truncated.trim_end_matches('-').to_string();
    if truncated.is_empty() {
        "printer".to_string()
    } else {
        truncated
    }
}

pub fn suggest_queue_name(uri: &str) -> String {
    suggest_queue_name_from_label(&friendly_name_from_uri(uri))
}

pub fn validate_queue_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("نام صف خالی است.".into());
    }
    if name.len() > 127 {
        return Err("نام صف نباید بیشتر از ۱۲۷ نویسه باشد.".into());
    }
    if name.starts_with('-') {
        return Err("نام صف نمی‌تواند با خط تیره شروع شود.".into());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return Err(
            "نام صف فقط می‌تواند شامل حروف انگلیسی، عدد، خط تیره، زیرخط و نقطه باشد.".into(),
        );
    }
    Ok(())
}

pub fn discover_printers(runner: &dyn Runner) -> PrinterDiscovery {
    let mut notes = Vec::new();
    let mut printers = Vec::new();

    if runner.exists("lpinfo") {
        let cmd = Cmd::new("lpinfo").arg("-v");
        match runner.run(&cmd) {
            Ok(output) => {
                if !output.success() && output.stdout.trim().is_empty() {
                    notes.push(describe_failure(&cmd, &output));
                }
                printers.extend(parse_lpinfo(&output.stdout));
            }
            Err(error) => notes.push(format!("اجرای lpinfo ناموفق بود: {error}")),
        }
    } else {
        notes.push("lpinfo پیدا نشد؛ بسته cups را نصب کنید.".into());
    }

    if runner.exists("avahi-browse") {
        for service in ["_ipps._tcp", "_ipp._tcp", "_pdl-datastream._tcp"] {
            let cmd = Cmd::new("avahi-browse").args(["-rtp", service]);
            match runner.run(&cmd) {
                Ok(output) => {
                    printers = merge_printers(printers, parse_avahi(&output.stdout));
                }
                Err(error) => {
                    notes.push(format!("جستجوی Avahi برای {service} ناموفق بود: {error}"));
                    break;
                }
            }
        }
    } else if printers.is_empty() {
        notes.push("avahi-browse پیدا نشد؛ برای کشف چاپگرهای شبکه avahi را نصب کنید.".into());
    }

    printers = dedup_printers(printers);
    if printers.is_empty() {
        notes.push("روشن بودن چاپگر، کابل USB و اتصال شبکه را بررسی کنید.".into());
    }

    PrinterDiscovery { printers, notes }
}

fn merge_printers(
    mut existing: Vec<DiscoveredPrinter>,
    candidates: Vec<DiscoveredPrinter>,
) -> Vec<DiscoveredPrinter> {
    for candidate in candidates {
        if let Some(index) = existing
            .iter()
            .position(|printer| same_printer(printer, &candidate))
        {
            if should_replace(&existing[index], &candidate) {
                existing[index] = candidate;
            }
        } else {
            existing.push(candidate);
        }
    }
    existing
}

fn dedup_printers(printers: Vec<DiscoveredPrinter>) -> Vec<DiscoveredPrinter> {
    let mut deduped: Vec<DiscoveredPrinter> = Vec::new();
    for printer in printers {
        if let Some(index) = deduped
            .iter()
            .position(|existing| existing.uri == printer.uri || same_printer(existing, &printer))
        {
            if should_replace(&deduped[index], &printer) {
                deduped[index] = printer;
            }
        } else {
            deduped.push(printer);
        }
    }
    deduped
}

fn should_replace(existing: &DiscoveredPrinter, candidate: &DiscoveredPrinter) -> bool {
    let better_uri = candidate.uri.starts_with("ipps://") && existing.uri.starts_with("ipp://");
    let better_driverless = candidate.driverless && !existing.driverless;
    let better_source =
        candidate.origin == PrinterOrigin::CupsDevices && existing.origin == PrinterOrigin::Avahi;
    better_uri || better_driverless || better_source
}

fn same_printer(left: &DiscoveredPrinter, right: &DiscoveredPrinter) -> bool {
    normalized_name(&left.name) == normalized_name(&right.name) && left.transport == right.transport
}

fn normalized_name(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .filter(|character| character.is_alphanumeric())
        .collect()
}

pub fn parse_lpstat(output: &str) -> Vec<InstalledPrinter> {
    let mut printers: Vec<InstalledPrinter> = Vec::new();
    let mut default_name: Option<String> = None;

    for line in output.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("system default destination:") {
            default_name = Some(rest.trim().to_string());
            continue;
        }
        if let Some(rest) = line.strip_prefix("printer ") {
            let mut parts = rest.splitn(2, ' ');
            let Some(name) = parts.next() else {
                continue;
            };
            let state_raw = parts.next().unwrap_or("").trim();
            let state = state_raw
                .strip_prefix("is ")
                .unwrap_or(state_raw)
                .trim()
                .to_string();
            printers.push(InstalledPrinter {
                name: name.to_string(),
                state,
                is_default: false,
            });
        }
    }

    if let Some(default) = default_name {
        for printer in &mut printers {
            if printer.name == default {
                printer.is_default = true;
            }
        }
    }
    printers
}

pub fn list_queues(runner: &dyn Runner) -> Result<Vec<InstalledPrinter>, String> {
    if !runner.exists("lpstat") {
        return Err("lpstat پیدا نشد؛ بسته cups را نصب کنید.".into());
    }
    let cmd = Cmd::new("lpstat").args(["-p", "-d"]);
    let output = runner
        .run(&cmd)
        .map_err(|error| format!("اجرای lpstat ناموفق بود: {error}"))?;
    let queues = parse_lpstat(&output.stdout);
    if !output.success() && queues.is_empty() {
        let stderr = output.stderr.to_lowercase();
        if stderr.contains("no destinations") {
            return Ok(Vec::new());
        }
        return Err(describe_failure(&cmd, &output));
    }
    Ok(queues)
}

pub fn add_printer_command(
    queue: &str,
    uri: &str,
    model: Option<&str>,
    make_default: bool,
) -> Result<Vec<Cmd>, String> {
    validate_queue_name(queue)?;
    let uri = uri.trim();
    if uri.is_empty() || uri.starts_with('-') {
        return Err("آدرس چاپگر نامعتبر است.".into());
    }
    let model = model
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .unwrap_or("everywhere");
    if model.starts_with('-') {
        return Err("مدل درایور نامعتبر است.".into());
    }

    let mut commands = vec![Cmd::new("lpadmin").args(["-p", queue, "-E", "-v", uri, "-m", model])];
    if make_default {
        commands.push(Cmd::new("lpadmin").args(["-d", queue]));
    }
    commands.push(Cmd::new("cupsenable").arg(queue));
    commands.push(Cmd::new("cupsaccept").arg(queue));
    Ok(commands)
}

pub fn add_printer(
    runner: &dyn Runner,
    queue: &str,
    uri: &str,
    model: Option<&str>,
    make_default: bool,
    prefer_pkexec: bool,
) -> Result<Vec<String>, String> {
    let commands = add_printer_command(queue, uri, model, make_default)?;
    let mut log = Vec::new();
    for command in commands {
        let output = run_privileged(runner, command.clone(), prefer_pkexec)?;
        if !output.success() {
            return Err(describe_failure(&command, &output));
        }
        let summary = output.summary();
        if !summary.is_empty() {
            log.push(summary);
        }
    }
    Ok(log)
}

pub fn remove_printer(
    runner: &dyn Runner,
    queue: &str,
    prefer_pkexec: bool,
) -> Result<String, String> {
    validate_queue_name(queue)?;
    let cmd = Cmd::new("lpadmin").args(["-x", queue]);
    let output = run_privileged(runner, cmd.clone(), prefer_pkexec)?;
    if !output.success() {
        return Err(describe_failure(&cmd, &output));
    }
    Ok(format!("چاپگر «{queue}» حذف شد."))
}

pub fn test_print(runner: &dyn Runner, queue: &str) -> Result<String, String> {
    validate_queue_name(queue)?;
    let path = std::env::temp_dir().join(format!("linuprint-test-{}.txt", std::process::id()));
    std::fs::write(&path, "LinuPrint\nTest page\n")
        .map_err(|error| format!("ساخت فایل آزمایشی ناموفق بود: {error}"))?;
    let cmd = Cmd::new("lp")
        .args(["-d", queue])
        .arg(path.to_string_lossy().into_owned());
    let result = runner.run(&cmd);
    let _ = std::fs::remove_file(&path);
    let output = result.map_err(|error| format!("اجرای lp ناموفق بود: {error}"))?;
    if !output.success() {
        return Err(describe_failure(&cmd, &output));
    }
    let message = output.stdout.trim();
    Ok(if message.is_empty() {
        format!("کار چاپ برای «{queue}» ارسال شد.")
    } else {
        message.to_string()
    })
}
