use std::collections::HashMap;

use crate::backend::printers::{
    suggest_queue_name_from_label, DiscoveredPrinter, PrinterOrigin, PrinterTransport,
};

pub fn parse_avahi(output: &str) -> Vec<DiscoveredPrinter> {
    let mut printers: Vec<DiscoveredPrinter> = Vec::new();
    for line in output.lines() {
        let Some(printer) = parse_line(line) else {
            continue;
        };
        if let Some(existing) = printers
            .iter_mut()
            .find(|existing| same_service(existing, &printer))
        {
            let better_uri =
                printer.uri.starts_with("ipps://") && existing.uri.starts_with("ipp://");
            if better_uri || (printer.driverless && !existing.driverless) {
                *existing = printer;
            }
            continue;
        }
        printers.push(printer);
    }
    printers
}

fn same_service(left: &DiscoveredPrinter, right: &DiscoveredPrinter) -> bool {
    normalized_name(&left.name) == normalized_name(&right.name)
}

fn normalized_name(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .filter(|character| character.is_alphanumeric())
        .collect()
}

fn parse_line(line: &str) -> Option<DiscoveredPrinter> {
    let fields = split_escaped(line, ';');
    if fields.len() < 10 || fields[0] != "=" {
        return None;
    }
    let name = unescape(&fields[3]);
    let service = fields[4].to_lowercase();
    let address = unescape(&fields[7]);
    let host = unescape(&fields[6]);
    let port: u16 = fields[8].parse().ok()?;
    let txt = parse_txt(&fields[9]);

    let target = if address.trim().is_empty() {
        host.trim_end_matches('.').to_string()
    } else {
        address
    };
    if target.is_empty() {
        return None;
    }
    let target = if target.contains(':') && !target.starts_with('[') {
        format!("[{target}]")
    } else {
        target
    };

    let rp = txt
        .get("rp")
        .map(|path| path.trim_matches('/').to_string())
        .filter(|path| !path.is_empty());

    let (uri, driverless) = match service.as_str() {
        "_ipps._tcp" => (
            format!(
                "ipps://{target}:{port}/{}",
                rp.unwrap_or_else(|| "ipp/print".into())
            ),
            true,
        ),
        "_ipp._tcp" => (
            format!(
                "ipp://{target}:{port}/{}",
                rp.unwrap_or_else(|| "ipp/print".into())
            ),
            true,
        ),
        "_pdl-datastream._tcp" => (format!("socket://{target}:{port}"), false),
        "_printer._tcp" => {
            let queue = rp.unwrap_or_else(|| "print".into());
            (format!("lpd://{target}/{queue}"), false)
        }
        _ => return None,
    };

    let model = txt
        .get("ty")
        .cloned()
        .filter(|model| !model.trim().is_empty())
        .unwrap_or_else(|| name.clone());
    let display_name = if name.trim().is_empty() { model } else { name };
    let queue_name = suggest_queue_name_from_label(&display_name);
    let detail = format!("{display_name} — {target}:{port}");

    Some(DiscoveredPrinter {
        name: display_name,
        uri,
        transport: PrinterTransport::Network,
        origin: PrinterOrigin::Avahi,
        driverless,
        detail,
        queue_name,
    })
}

fn split_escaped(input: &str, separator: char) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut escaped = false;
    for character in input.chars() {
        if escaped {
            current.push('\\');
            current.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == separator {
            fields.push(std::mem::take(&mut current));
        } else {
            current.push(character);
        }
    }
    if escaped {
        current.push('\\');
    }
    fields.push(current);
    fields
}

fn unescape(input: &str) -> String {
    let mut output = String::new();
    let mut characters = input.chars().peekable();
    while let Some(character) = characters.next() {
        if character != '\\' {
            output.push(character);
            continue;
        }
        match characters.peek().copied() {
            Some(first) if first.is_ascii_digit() => {
                let mut digits = String::new();
                for _ in 0..3 {
                    match characters.peek().copied() {
                        Some(digit) if digit.is_ascii_digit() => {
                            digits.push(digit);
                            characters.next();
                        }
                        _ => break,
                    }
                }
                if let Ok(byte) = digits.parse::<u8>() {
                    output.push(char::from(byte));
                } else {
                    output.push_str(&digits);
                }
            }
            Some(other) => {
                characters.next();
                output.push(other);
            }
            None => output.push('\\'),
        }
    }
    output
}

fn parse_txt(field: &str) -> HashMap<String, String> {
    let mut values = HashMap::new();
    let mut quoted = false;
    let mut escaped = false;
    let mut current = String::new();

    for character in field.chars() {
        if escaped {
            current.push('\\');
            current.push(character);
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            '"' => {
                if quoted {
                    if let Some((key, value)) = current.split_once('=') {
                        values.insert(key.to_lowercase(), unescape(value));
                    }
                    current.clear();
                }
                quoted = !quoted;
            }
            _ if quoted => current.push(character),
            _ => {}
        }
    }
    values
}
