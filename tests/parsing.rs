mod support;

use linuprint::backend::avahi::parse_avahi;
use linuprint::backend::diagnostics::nsswitch_has_mdns;
use linuprint::backend::drivers::{recommend_print_package, recommend_scanner_package};
use linuprint::backend::printers::{
    friendly_name_from_uri, parse_jobs, parse_lpinfo, parse_lpinfo_models, parse_lpstat,
    select_model, suggest_queue_name, uri_supports_driverless, validate_queue_name,
};
use linuprint::backend::scanners::parse_scanimage;

#[test]
fn parses_lpinfo_devices_and_ignores_classes() {
    let output = "\
network beh
direct hp
network ipps://HP%20OfficeJet%20Pro:631/ipp/print
network dnssd://Brother%20HL-L2350DW._ipp._tcp.local/?uuid=abc
direct usb://HP/LaserJet%20Pro?serial=123
";
    let printers = parse_lpinfo(output);
    assert_eq!(printers.len(), 3);
    assert_eq!(printers[0].queue_name, "hp-officejet-pro");
    assert!(printers[1].queue_name.starts_with("brother-hl-l2350dw"));
    assert_eq!(printers[2].transport as u8, 0);
    assert!(printers[2].name.contains("LaserJet"));
    assert!(printers[0].driverless);
    assert!(printers[1].driverless);
    assert!(!printers[2].driverless);
    assert!(printers.iter().all(|printer| !printer.uri.is_empty()));
}

#[test]
fn parses_avahi_with_escapes_and_prefers_ipps() {
    let output = concat!(
        "=;wlan0;IPv4;HP_OfficeJet_Pro_9010;_ipp._tcp;local;HP-OfficeJet.local;192.168.1.20;631;\"txtvers=1\" \"rp=ipp/print\" \"ty=HP OfficeJet Pro 9010\"\n",
        "=;wlan0;IPv4;HP_OfficeJet_Pro_9010;_ipps._tcp;local;HP-OfficeJet.local;192.168.1.20;631;\"rp=ipp/print\" \"ty=HP OfficeJet Pro 9010\"\n",
        "=;wlan0;IPv4;Brother\\032HL-L2350DW;_pdl-datastream._tcp;local;BRW.local;192.168.1.42;9100;\"ty=Brother HL-L2350DW\"\n"
    );

    let printers = parse_avahi(output);
    assert_eq!(printers.len(), 2);
    assert!(printers
        .iter()
        .any(|printer| printer.uri == "ipps://192.168.1.20:631/ipp/print"));
    assert!(printers
        .iter()
        .any(|printer| printer.uri == "socket://192.168.1.42:9100"));
    assert!(printers
        .iter()
        .any(|printer| printer.name == "Brother HL-L2350DW"));
}

#[test]
fn parses_print_jobs_and_ignores_duplicates() {
    let output = "\
hp-laserjet-1320-42 azizi 1024 Fri 23 Oct 2026 01:22:33 AM CET
office-printer-7 guest 2048 Fri 23 Oct 2026 02:00:00 AM CET
hp-laserjet-1320-42 azizi 1024 Fri 23 Oct 2026 01:22:33 AM CET
";
    let jobs = parse_jobs(output);
    assert_eq!(jobs.len(), 2);
    assert_eq!(jobs[0].queue, "hp-laserjet-1320");
    assert_eq!(jobs[0].submitted, "Fri 23 Oct 2026 01:22:33 AM CET");
    assert_eq!(jobs[1].queue, "office-printer");
    assert_eq!(jobs[1].user, "guest");
}

#[test]
fn parses_scanimage_devices() {
    let output = "\
device `escl:https://192.168.1.20:443` is a ESCL EPSON ET-2750 Series flatbed scanner
device `hpaio:/usb/OfficeJet_Pro_6970?serial=CN123` is a Hewlett-Packard OfficeJet_Pro_6970 all-in-one
";
    let scanners = parse_scanimage(output);
    assert_eq!(scanners.len(), 2);
    assert_eq!(scanners[0].device, "escl:https://192.168.1.20:443");
    assert!(scanners[0].network);
    assert!(!scanners[1].network);
    assert!(scanners[1].description.contains("Hewlett-Packard"));
}

#[test]
fn parses_lpstat_queues_and_default() {
    let output = "\
printer HP_LaserJet is idle.  enabled since Thu 01 Jan 2026
system default destination: HP_LaserJet
printer Office is disabled since Thu 01 Jan 2026
";
    let queues = parse_lpstat(output);
    assert_eq!(queues.len(), 2);
    let laser = queues
        .iter()
        .find(|queue| queue.name == "HP_LaserJet")
        .unwrap();
    assert!(laser.is_default);
    assert!(laser.state.starts_with("idle."));
    let office = queues.iter().find(|queue| queue.name == "Office").unwrap();
    assert!(!office.is_default);
}

#[test]
fn builds_safe_queue_names() {
    assert_eq!(
        suggest_queue_name("usb://HP/OfficeJet%20Pro%209010?serial=1"),
        "hp-officejet-pro-9010"
    );
    assert_eq!(
        friendly_name_from_uri("ipps://Printer.local:631/ipp/print"),
        "Printer.local"
    );
    assert_eq!(
        suggest_queue_name("dnssd://Brother%20HL-L2350DW._ipp._tcp.local/"),
        "brother-hl-l2350dw"
    );
    assert!(validate_queue_name("hp-9010").is_ok());
    assert!(validate_queue_name("hp.9010").is_ok());
    assert!(validate_queue_name("-bad").is_err());
    assert!(validate_queue_name("bad/name").is_err());
    assert!(validate_queue_name("").is_err());
}

#[test]
fn selects_exact_model_from_lpinfo_catalog() {
    let output = "\
everywhere IPP Everywhere
raw Raw Queue
drv:///hp/hpcups.drv/hp-laserjet_1300_series.ppd HP LaserJet 1300 Series, hpcups 3.26.6
drv:///hp/hpcups.drv/hp-laserjet_1320_series-pcl3.ppd HP LaserJet 1320 Series pcl3, hpcups 3.26.6
drv:///hp/hpcups.drv/hp-laserjet_1320n.ppd HP LaserJet 1320n, hpcups 3.26.6
drv:///hp/hpcups.drv/hp-laserjet_1320.ppd HP LaserJet 1320, hpcups 3.26.6
lsb/usr/HP/hp-laserjet_1320_series-ps.ppd.gz HP LaserJet 1320 series Postscript
";
    let models = parse_lpinfo_models(output);
    assert_eq!(models.len(), 5);
    let selected = select_model(
        &models,
        "HP LaserJet 1320 series",
        "usb://HP/LaserJet%201320%20series?serial=00CNGW53YHLB",
    )
    .unwrap();
    assert_eq!(selected.model, "drv:///hp/hpcups.drv/hp-laserjet_1320.ppd");
    assert!(uri_supports_driverless("ipps://printer.local/ipp/print"));
    assert!(uri_supports_driverless("dnssd://Printer._ipp._tcp.local/"));
    assert!(!uri_supports_driverless(
        "usb://HP/LaserJet%201320%20series"
    ));
}

#[test]
fn detects_mdns_in_nsswitch() {
    assert!(nsswitch_has_mdns(
        "hosts: files mymachines mdns_minimal [NOTFOUND=return] dns\n"
    ));
    assert!(!nsswitch_has_mdns(
        "# hosts: mdns_minimal\nhosts: files dns\n"
    ));
    assert!(!nsswitch_has_mdns("hosts: files mymachines dns\n"));
}

#[test]
fn recommends_drivers_by_vendor() {
    assert_eq!(
        recommend_print_package("HP LaserJet 1020").unwrap().package,
        "hplip"
    );
    assert_eq!(
        recommend_print_package("Brother HL-L2350DW")
            .unwrap()
            .package,
        "brlaser"
    );
    assert!(recommend_print_package("Generic Esc/POS").is_none());

    let network = recommend_scanner_package(
        "ESCL EPSON ET-2750 Series flatbed scanner",
        "escl:https://192.168.1.20:443",
    )
    .unwrap();
    assert_eq!(network.package, "sane-airscan");
    assert!(network.official);

    let hp = recommend_scanner_package(
        "Hewlett-Packard OfficeJet all-in-one",
        "hpaio:/usb/OfficeJet?serial=1",
    )
    .unwrap();
    assert_eq!(hp.package, "hplip");

    let brother = recommend_scanner_package("Brother DCP-L2520DW", "airscan:e0:Brother").unwrap();
    assert_eq!(brother.package, "sane-airscan");
    assert!(
        !recommend_scanner_package("Brother DCP-L2520DW", "brother4:bus2;dev1")
            .unwrap()
            .official
    );
}
