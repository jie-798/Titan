use std::net::{IpAddr, Ipv4Addr};

use crate::resolver::{parse_nameserver_for_test, DnsResolver};

#[test]
fn parses_nameserver_without_port() {
    let addr = parse_nameserver_for_test("1.1.1.1").unwrap();
    assert_eq!(addr, "1.1.1.1:53".parse().unwrap());
}

#[test]
fn parses_a_record_response() {
    let response = [
        0x12, 0x34, 0x81, 0x80, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x07, b'e',
        b'x', b'a', b'm', b'p', b'l', b'e', 0x03, b'c', b'o', b'm', 0x00, 0x00, 0x01, 0x00,
        0x01, 0xC0, 0x0C, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x3C, 0x00, 0x04, 93,
        184, 216, 34,
    ];

    let ips = crate::resolver::parse_response_for_test(&response, 0x1234, 1).unwrap();
    assert_eq!(ips, vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))]);
}

#[test]
fn resolver_accepts_valid_nameserver_config() {
    let config = titan_config::DnsConfig {
        nameserver: vec!["1.1.1.1".to_string()],
        ..Default::default()
    };

    let _ = DnsResolver::new(&config).unwrap();
}
