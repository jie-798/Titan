use std::net::IpAddr;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crate::geoip::{GeoIpDatabase, GeoIpMatcher, DEFAULT_GEOIP_DB_PATH};

#[test]
fn default_geoip_path_looks_stable() {
    assert_eq!(DEFAULT_GEOIP_DB_PATH, "data/geoip-apnic.raw");
}

#[test]
fn parses_apnic_style_data() {
    let sample = "\
apnic|CN|ipv4|1.0.1.0|256|20110414|allocated
apnic|CN|ipv6|2400:3200::|32|20110414|allocated
apnic|JP|ipv4|1.0.16.0|4096|20110414|allocated
";

    let db = GeoIpDatabase::from_str(sample).unwrap();
    assert_eq!(db.country_count(), 2);
    assert!(db.contains_country(&"1.0.1.5".parse::<IpAddr>().unwrap(), "CN"));
    assert!(db.contains_country(&"2400:3200::1".parse::<IpAddr>().unwrap(), "CN"));
    assert!(!db.contains_country(&"1.0.16.1".parse::<IpAddr>().unwrap(), "CN"));
    assert!(db.contains_country(&"1.0.16.1".parse::<IpAddr>().unwrap(), "JP"));
}

#[test]
fn reloads_when_file_changes() {
    let path = temp_geoip_path();
    std::fs::write(&path, "apnic|CN|ipv4|1.0.1.0|256|20110414|allocated\n").unwrap();

    let matcher = GeoIpMatcher::from_path(&path).unwrap();
    let ip = "1.0.1.5".parse::<IpAddr>().unwrap();
    assert!(matcher.contains_country(&ip, "CN"));
    assert!(!matcher.contains_country(&ip, "JP"));

    thread::sleep(Duration::from_secs(5) + Duration::from_millis(50));
    std::fs::write(&path, "apnic|JP|ipv4|1.0.1.0|256|20110414|allocated\n").unwrap();
    thread::sleep(Duration::from_millis(50));

    assert!(!matcher.contains_country(&ip, "CN"));
    assert!(matcher.contains_country(&ip, "JP"));

    let _ = std::fs::remove_file(path);
}

fn temp_geoip_path() -> PathBuf {
    let mut path = std::env::temp_dir();
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    path.push(format!(
        "titan-geoip-test-{}-{}.txt",
        std::process::id(),
        unique
    ));
    path
}
