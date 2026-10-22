#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriverRecommendation {
    pub package: &'static str,
    pub official: bool,
    pub reason: &'static str,
}

pub fn recommend_print_package(model: &str) -> Option<DriverRecommendation> {
    let text = model.to_lowercase();
    if text.contains("hp") || text.contains("hewlett") {
        Some(DriverRecommendation {
            package: "hplip",
            official: true,
            reason: "درایور و ابزارهای HP",
        })
    } else if text.contains("epson") {
        Some(DriverRecommendation {
            package: "epson-inkjet-printer-escpr",
            official: true,
            reason: "درایور ESC/P-R اپسون",
        })
    } else if text.contains("brother") {
        Some(DriverRecommendation {
            package: "brlaser",
            official: true,
            reason: "درایور Brother Laser",
        })
    } else if text.contains("canon") {
        Some(DriverRecommendation {
            package: "gutenprint",
            official: true,
            reason: "درایورهای عمومی Canon و سایر برندها",
        })
    } else {
        None
    }
}

pub fn recommend_scanner_package(description: &str, device: &str) -> Option<DriverRecommendation> {
    let text = format!("{description} {device}").to_lowercase();
    if text.contains("hpaio") {
        return Some(DriverRecommendation {
            package: "hplip",
            official: true,
            reason: "بک‌اند اسکنر HP",
        });
    }
    let network_discovery = text.contains("escl:")
        || text.contains("airscan:")
        || text.contains(" ip=")
        || text.contains("http://")
        || text.contains("https://");
    if network_discovery {
        return Some(DriverRecommendation {
            package: "sane-airscan",
            official: true,
            reason: "پشتیبانی از اسکنرهای شبکه eSCL/AirScan",
        });
    }
    if text.contains("hp") || text.contains("hewlett") {
        Some(DriverRecommendation {
            package: "hplip",
            official: true,
            reason: "بک‌اند اسکنر HP",
        })
    } else if text.contains("brother") {
        Some(DriverRecommendation {
            package: "brscan5",
            official: false,
            reason: "درایور اسکنر Brother (AUR؛ برای مدل‌های قدیمی‌تر brscan4)",
        })
    } else if text.contains("epson") {
        Some(DriverRecommendation {
            package: "epsonscan2",
            official: false,
            reason: "درایور اسکنر اپسون (AUR)",
        })
    } else if text.contains("canon") {
        Some(DriverRecommendation {
            package: "scangearmp2",
            official: false,
            reason: "درایور اسکنر کانن (AUR)",
        })
    } else {
        None
    }
}
