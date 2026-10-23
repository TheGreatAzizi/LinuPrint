use std::collections::BTreeSet;
use std::path::Path;

use crate::backend::avahi::parse_avahi;
use crate::backend::drivers::recommend_print_package;
use crate::backend::packages::{install_packages, package_installed};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrinterModel {
    pub model: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionResult {
    pub model: String,
    pub installed_packages: Vec<String>,
    pub log: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrintJob {
    pub id: String,
    pub queue: String,
    pub user: String,
    pub size: String,
    pub submitted: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrintOptions {
    pub copies: u32,
    pub pages: Option<String>,
    pub duplex: bool,
    pub fit_to_page: bool,
    pub media: Option<String>,
}

impl Default for PrintOptions {
    fn default() -> Self {
        Self {
            copies: 1,
            pages: None,
            duplex: false,
            fit_to_page: false,
            media: None,
        }
    }
}

pub fn uri_supports_driverless(uri: &str) -> bool {
    let uri = uri.trim().to_ascii_lowercase();
    uri.starts_with("ipp://") || uri.starts_with("ipps://") || uri.starts_with("dnssd://")
}

pub fn parse_lpinfo_models(output: &str) -> Vec<PrinterModel> {
    let mut models: Vec<PrinterModel> = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (model, description) =
            match line.split_once(|character: char| character.is_whitespace()) {
                Some((model, description)) => (model.trim(), description.trim()),
                None => (line, ""),
            };
        if model.is_empty() || model == "everywhere" || model == "raw" {
            continue;
        }
        if models.iter().any(|entry| entry.model == model) {
            continue;
        }
        models.push(PrinterModel {
            model: model.to_string(),
            description: description.to_string(),
        });
    }
    models
}

const GENERIC_TOKENS: &[&str] = &[
    "hp",
    "hewlett",
    "packard",
    "printer",
    "printers",
    "series",
    "driver",
    "drivers",
    "cups",
    "hpcups",
    "hplip",
    "drv",
    "ppd",
    "lsb",
    "usr",
    "share",
    "everywhere",
    "raw",
    "pdf",
    "postscript",
    "ps",
    "pcl",
    "pcl5",
    "pcl6",
    "zjs",
    "escpr",
    "brlaser",
    "gutenprint",
    "foomatic",
    "db",
    "the",
    "for",
    "and",
    "with",
    "inc",
    "ltd",
    "co",
    "corp",
    "corporation",
];

fn normalized_tokens(text: &str) -> BTreeSet<String> {
    text.split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| token.len() >= 2)
        .map(str::to_ascii_lowercase)
        .filter(|token| !GENERIC_TOKENS.contains(&token.as_str()))
        .collect()
}

fn number_tokens(text: &str) -> BTreeSet<String> {
    let mut numbers = BTreeSet::new();
    let mut current = String::new();
    for character in text.chars() {
        if character.is_ascii_digit() {
            current.push(character);
        } else {
            if current.len() >= 3 {
                numbers.insert(current.clone());
            }
            current.clear();
        }
    }
    if current.len() >= 3 {
        numbers.insert(current);
    }
    numbers
}

fn vendor_from_text(text: &str) -> Option<&'static str> {
    for token in text
        .to_ascii_lowercase()
        .split(|character: char| !character.is_ascii_alphanumeric())
    {
        let vendor = match token {
            "hp" | "hewlett" | "packard" => "hp",
            "epson" => "epson",
            "brother" => "brother",
            "canon" => "canon",
            "lexmark" => "lexmark",
            "xerox" => "xerox",
            "samsung" => "samsung",
            "ricoh" => "ricoh",
            "kyocera" => "kyocera",
            "dell" => "dell",
            _ => continue,
        };
        return Some(vendor);
    }
    None
}

fn candidate_matches_vendor(vendor: &str, text: &str) -> bool {
    let text = text.to_ascii_lowercase();
    match vendor {
        "hp" => {
            text.contains("hp")
                || text.contains("hewlett")
                || text.contains("hpcups")
                || text.contains("hplip")
        }
        _ => text.contains(vendor),
    }
}

pub fn select_model(
    models: &[PrinterModel],
    printer_name: &str,
    uri: &str,
) -> Option<PrinterModel> {
    let printer_text = format!("{printer_name} {}", percent_decode(uri));
    let printer_tokens = normalized_tokens(&printer_text);
    let printer_numbers = number_tokens(printer_name);
    let vendor = vendor_from_text(&printer_text);
    let mut best: Option<(i32, PrinterModel)> = None;

    for model in models {
        let candidate_text = format!("{} {}", model.model, model.description);
        if let Some(vendor) = vendor {
            if !candidate_matches_vendor(vendor, &candidate_text) {
                continue;
            }
        }

        let mut candidate_numbers = number_tokens(&model.model);
        let description_head = model.description.split(',').next().unwrap_or("");
        candidate_numbers.extend(number_tokens(description_head));

        if !printer_numbers.is_empty()
            && !candidate_numbers.is_empty()
            && printer_numbers.is_disjoint(&candidate_numbers)
        {
            continue;
        }

        let candidate_tokens = normalized_tokens(&candidate_text);
        let mut score = 0;
        for token in &printer_tokens {
            if candidate_tokens.contains(token) {
                score += 10;
            }
        }
        for number in &printer_numbers {
            if candidate_numbers.contains(number) {
                score += 100;
            }
        }
        if score == 0 {
            continue;
        }

        let description_tokens = normalized_tokens(description_head);
        let extra_tokens = description_tokens
            .iter()
            .filter(|token| !printer_tokens.contains(*token))
            .count() as i32;
        score -= extra_tokens * 5;

        let lower = candidate_text.to_ascii_lowercase();
        if lower.contains("hpcups")
            || lower.contains("hplip")
            || lower.contains("escpr")
            || lower.contains("brlaser")
            || lower.contains("gutenprint")
        {
            score += 5;
        }
        if model.model.starts_with("drv:///") {
            score += 2;
        }

        let better = match &best {
            Some((best_score, _)) => score > *best_score,
            None => true,
        };
        if better {
            best = Some((score, model.clone()));
        }
    }

    best.map(|(_, model)| model)
}

pub fn provision_printer(
    runner: &dyn Runner,
    queue: &str,
    uri: &str,
    printer_name: &str,
    explicit_model: Option<&str>,
    make_default: bool,
    prefer_pkexec: bool,
) -> Result<ProvisionResult, String> {
    validate_queue_name(queue)?;

    if let Some(model) = explicit_model
        .map(str::trim)
        .filter(|model| !model.is_empty())
    {
        if model.starts_with('-') {
            return Err("مدل درایور نامعتبر است.".into());
        }
        let log = add_printer(runner, queue, uri, Some(model), make_default, prefer_pkexec)?;
        return Ok(ProvisionResult {
            model: model.to_string(),
            installed_packages: Vec::new(),
            log,
        });
    }

    let mut log = Vec::new();
    if uri_supports_driverless(uri) {
        match add_printer(runner, queue, uri, None, make_default, prefer_pkexec) {
            Ok(mut lines) => {
                log.append(&mut lines);
                return Ok(ProvisionResult {
                    model: "everywhere".into(),
                    installed_packages: Vec::new(),
                    log,
                });
            }
            Err(error) => log.push(format!("راه‌اندازی driverless ناموفق بود: {error}")),
        }
    }

    let printer_text = format!("{printer_name} {}", percent_decode(uri));
    let mut installed_packages = Vec::new();
    let mut package_error = None;

    if let Some(recommendation) = recommend_print_package(&printer_text) {
        if recommendation.official {
            match package_installed(runner, recommendation.package) {
                Ok(true) => {}
                Ok(false) => {
                    log.push(format!(
                        "بسته درایور «{}» نصب می‌شود؛ ممکن است چند دقیقه طول بکشد.",
                        recommendation.package
                    ));
                    match install_packages(
                        runner,
                        &[recommendation.package.to_string()],
                        prefer_pkexec,
                    ) {
                        Ok(message) => {
                            log.push(message);
                            installed_packages.push(recommendation.package.to_string());
                        }
                        Err(error) => package_error = Some(error),
                    }
                }
                Err(error) => package_error = Some(error),
            }
        } else {
            package_error = Some(format!(
                "بسته درایور «{}» در AUR است و به‌صورت خودکار نصب نمی‌شود.",
                recommendation.package
            ));
        }
    }

    if !runner.exists("lpinfo") {
        return Err("lpinfo پیدا نشد؛ بسته cups را نصب کنید.".into());
    }
    let cmd = Cmd::new("lpinfo").arg("-m");
    let output = runner
        .run(&cmd)
        .map_err(|error| format!("اجرای lpinfo -m ناموفق بود: {error}"))?;
    if !output.success() && output.stdout.trim().is_empty() {
        let mut message = describe_failure(&cmd, &output);
        if let Some(error) = package_error {
            message.push_str(&format!(" نصب درایور نیز ناموفق بود: {error}"));
        }
        return Err(message);
    }

    let models = parse_lpinfo_models(&output.stdout);
    let Some(selected) = select_model(&models, printer_name, uri) else {
        let mut message =
            "مدل درایور مناسبی برای این چاپگر پیدا نشد. مدل را دستی از خروجی lpinfo -m وارد کنید."
                .to_string();
        if let Some(error) = package_error {
            message.push_str(&format!(" نصب درایور ناموفق بود: {error}"));
        }
        return Err(message);
    };

    if let Some(error) = package_error {
        log.push(format!("هشدار نصب درایور: {error}"));
    }
    let mut added = add_printer(
        runner,
        queue,
        uri,
        Some(&selected.model),
        make_default,
        prefer_pkexec,
    )?;
    log.append(&mut added);
    log.push(format!(
        "درایور خودکار انتخاب‌شده: {} ({})",
        selected.model, selected.description
    ));

    Ok(ProvisionResult {
        model: selected.model,
        installed_packages,
        log,
    })
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
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let path = std::env::temp_dir().join(format!(
        "linuprint-test-{}-{timestamp}.pdf",
        std::process::id()
    ));
    std::fs::write(&path, test_page_pdf(queue))
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
        format!("صفحه آزمایشی برای «{queue}» ارسال شد.")
    } else {
        message.to_string()
    })
}

pub fn test_page_pdf(queue: &str) -> Vec<u8> {
    let queue = escape_pdf_text(queue);
    let version = escape_pdf_text(env!("CARGO_PKG_VERSION"));
    let content = format!(
        "\
0.08 0.24 0.45 rg
40 790 515 12 re f
0.08 0.24 0.45 RG
1.5 w
40 40 515 762 re S
72 735 m 523 735 l S
BT
/F2 26 Tf
0.08 0.24 0.45 rg
72 755 Td
(LinuPrint Test Page) Tj
0 -30 Td
/F1 13 Tf
0 0 0 rg
(Printer and scanner setup for Arch Linux) Tj
0 -22 Td
/F2 12 Tf
(LinuPrint {version}) Tj
0 -18 Td
/F1 11 Tf
(Queue: {queue}) Tj
0 -18 Td
(This page was generated automatically by LinuPrint.) Tj
0 -38 Td
/F2 15 Tf
(If you can read this page, printing is working.) Tj
0 -34 Td
/F1 12 Tf
(LinuPrint helps you:) Tj
0 -22 Td
(\\267 Set up CUPS printers and choose the right driver automatically) Tj
0 -19 Td
(\\267 Print files and manage pending jobs) Tj
0 -19 Td
(\\267 Scan to PNG, JPEG, or PDF with SANE) Tj
0 -19 Td
(\\267 Diagnose CUPS, Avahi, SANE, and permissions) Tj
0 -38 Td
/F2 12 Tf
(Project: github.com/TheGreatAzizi/LinuPrint) Tj
0 -18 Td
(Issues: github.com/TheGreatAzizi/LinuPrint/issues) Tj
ET
0.45 0.45 0.45 rg
BT
/F1 9 Tf
72 62 Td
(A4 test page - generated by LinuPrint) Tj
ET
"
    );
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 5 0 R /F2 6 0 R >> >> /Contents 4 0 R >>".to_string(),
        format!("<< /Length {} >>\nstream\n{}endstream", content.len(), content),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold >>".to_string(),
    ];

    let mut pdf = String::from("%PDF-1.4\n%LinuPrint\n");
    let mut offsets = Vec::with_capacity(objects.len());
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.push_str(&format!("{} 0 obj\n{}\nendobj\n", index + 1, object));
    }
    let xref_offset = pdf.len();
    pdf.push_str(&format!(
        "xref\n0 {}\n0000000000 65535 f \n",
        objects.len() + 1
    ));
    for offset in offsets {
        pdf.push_str(&format!("{offset:010} 00000 n \n"));
    }
    pdf.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
        objects.len() + 1
    ));
    pdf.into_bytes()
}

fn escape_pdf_text(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '(' => escaped.push_str("\\("),
            ')' => escaped.push_str("\\)"),
            '\n' | '\r' => escaped.push(' '),
            character if character.is_ascii() => escaped.push(character),
            _ => escaped.push('?'),
        }
    }
    escaped
}

pub fn print_file_command(queue: &str, file: &Path, options: &PrintOptions) -> Result<Cmd, String> {
    validate_queue_name(queue)?;
    if file.as_os_str().is_empty() {
        return Err("مسیر فایل خالی است.".into());
    }
    if file.to_string_lossy().starts_with('-') {
        return Err("مسیر فایل نامعتبر است.".into());
    }
    if options.copies == 0 || options.copies > 99 {
        return Err("تعداد نسخه‌ها باید بین ۱ تا ۹۹ باشد.".into());
    }

    let mut cmd = Cmd::new("lp").args(["-d", queue]);
    if options.copies > 1 {
        let copies = options.copies.to_string();
        cmd = cmd.args(["-n", copies.as_str()]);
    }
    if options.duplex {
        cmd = cmd.args(["-o", "sides=two-sided-long-edge"]);
    }
    if options.fit_to_page {
        cmd = cmd.args(["-o", "fit-to-page"]);
    }
    if let Some(pages) = options.pages.as_deref() {
        let pages = normalize_page_ranges(pages)?;
        let option = format!("page-ranges={pages}");
        cmd = cmd.args(["-o", option.as_str()]);
    }
    if let Some(media) = options.media.as_deref() {
        validate_media(media)?;
        let option = format!("media={media}");
        cmd = cmd.args(["-o", option.as_str()]);
    }
    Ok(cmd.arg(file.to_string_lossy().into_owned()))
}

pub fn print_file(
    runner: &dyn Runner,
    queue: &str,
    file: &Path,
    options: &PrintOptions,
) -> Result<String, String> {
    if !file.is_file() {
        return Err(format!("فایل «{}» پیدا نشد.", file.display()));
    }
    let cmd = print_file_command(queue, file, options)?;
    let output = runner
        .run(&cmd)
        .map_err(|error| format!("اجرای lp ناموفق بود: {error}"))?;
    if !output.success() {
        return Err(describe_failure(&cmd, &output));
    }
    let message = output.stdout.trim();
    Ok(if message.is_empty() {
        format!("فایل برای چاپ در «{queue}» ارسال شد.")
    } else {
        format!("فایل ارسال شد: {message}")
    })
}

pub fn parse_jobs(output: &str) -> Vec<PrintJob> {
    let mut jobs: Vec<PrintJob> = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(id) = parts.next() else {
            continue;
        };
        if jobs.iter().any(|job| job.id == id) {
            continue;
        }
        let queue = id
            .rsplit_once('-')
            .map(|(queue, _)| queue)
            .unwrap_or(id)
            .to_string();
        let user = parts.next().unwrap_or("").to_string();
        let size = parts.next().unwrap_or("").to_string();
        let submitted = parts.collect::<Vec<_>>().join(" ");
        jobs.push(PrintJob {
            id: id.to_string(),
            queue,
            user,
            size,
            submitted,
        });
    }
    jobs
}

pub fn list_jobs(runner: &dyn Runner, queue: Option<&str>) -> Result<Vec<PrintJob>, String> {
    if !runner.exists("lpstat") {
        return Err("lpstat پیدا نشد؛ بسته cups را نصب کنید.".into());
    }
    let cmd = if let Some(queue) = queue {
        validate_queue_name(queue)?;
        Cmd::new("lpstat").args(["-o", queue])
    } else {
        Cmd::new("lpstat").arg("-o")
    };
    let output = runner
        .run(&cmd)
        .map_err(|error| format!("اجرای lpstat ناموفق بود: {error}"))?;
    if !output.success() && output.stdout.trim().is_empty() {
        let stderr = output.stderr.to_lowercase();
        if stderr.contains("no entries")
            || stderr.contains("no jobs")
            || stderr.contains("no destinations")
        {
            return Ok(Vec::new());
        }
        return Err(describe_failure(&cmd, &output));
    }
    Ok(parse_jobs(&output.stdout))
}

pub fn cancel_job(runner: &dyn Runner, job_id: &str) -> Result<String, String> {
    validate_job_id(job_id)?;
    if !runner.exists("cancel") {
        return Err("cancel پیدا نشد؛ بسته cups را نصب کنید.".into());
    }
    let cmd = Cmd::new("cancel").arg(job_id.trim());
    let output = runner
        .run(&cmd)
        .map_err(|error| format!("اجرای cancel ناموفق بود: {error}"))?;
    if !output.success() {
        return Err(describe_failure(&cmd, &output));
    }
    Ok(format!("کار «{}» لغو شد.", job_id.trim()))
}

pub fn cancel_all_jobs(
    runner: &dyn Runner,
    queue: &str,
    prefer_pkexec: bool,
) -> Result<String, String> {
    validate_queue_name(queue)?;
    let cmd = Cmd::new("cancel").args(["-a", queue]);
    let output = run_privileged(runner, cmd.clone(), prefer_pkexec)?;
    if !output.success() {
        return Err(describe_failure(&cmd, &output));
    }
    Ok(format!("همه کارهای صف «{queue}» لغو شدند."))
}

pub fn set_default_printer(
    runner: &dyn Runner,
    queue: &str,
    prefer_pkexec: bool,
) -> Result<String, String> {
    validate_queue_name(queue)?;
    let cmd = Cmd::new("lpadmin").args(["-d", queue]);
    let output = run_privileged(runner, cmd.clone(), prefer_pkexec)?;
    if !output.success() {
        return Err(describe_failure(&cmd, &output));
    }
    Ok(format!("چاپگر «{queue}» پیش‌فرض شد."))
}

pub fn set_queue_enabled(
    runner: &dyn Runner,
    queue: &str,
    enabled: bool,
    prefer_pkexec: bool,
) -> Result<String, String> {
    validate_queue_name(queue)?;
    let program = if enabled { "cupsenable" } else { "cupsdisable" };
    let cmd = Cmd::new(program).arg(queue);
    let output = run_privileged(runner, cmd.clone(), prefer_pkexec)?;
    if !output.success() {
        return Err(describe_failure(&cmd, &output));
    }
    Ok(if enabled {
        format!("صف «{queue}» فعال شد.")
    } else {
        format!("صف «{queue}» متوقف شد.")
    })
}

pub fn is_queue_disabled(state: &str) -> bool {
    state.trim().to_lowercase().starts_with("disabled")
}

fn validate_job_id(job_id: &str) -> Result<(), String> {
    let job_id = job_id.trim();
    if job_id.is_empty() {
        return Err("شناسه کار خالی است.".into());
    }
    if job_id.starts_with('-') {
        return Err("شناسه کار نامعتبر است.".into());
    }
    if !job_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '@'))
    {
        return Err("شناسه کار نامعتبر است.".into());
    }
    Ok(())
}

fn normalize_page_ranges(value: &str) -> Result<String, String> {
    let mut ranges = Vec::new();
    for part in value.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err("محدوده صفحات نامعتبر است.".into());
        }
        if let Some((start, end)) = part.split_once('-') {
            let start = parse_page_number(start)?;
            let end = parse_page_number(end)?;
            if start > end {
                return Err("محدوده صفحات نامعتبر است.".into());
            }
        } else {
            parse_page_number(part)?;
        }
        ranges.push(part);
    }
    Ok(ranges.join(","))
}

fn parse_page_number(value: &str) -> Result<u32, String> {
    let value = value.trim();
    if value.is_empty() || !value.chars().all(|c| c.is_ascii_digit()) {
        return Err("شماره صفحه نامعتبر است.".into());
    }
    value
        .parse::<u32>()
        .map_err(|_| "شماره صفحه نامعتبر است.".into())
}

fn validate_media(media: &str) -> Result<(), String> {
    let media = media.trim();
    if media.is_empty()
        || media.starts_with('-')
        || !media
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return Err("اندازه کاغذ نامعتبر است.".into());
    }
    Ok(())
}
