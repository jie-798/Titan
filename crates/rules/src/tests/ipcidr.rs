use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use crate::ipcidr::match_cidr;

#[test]
fn matches_ipv4_cidr() {
    let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10));
    assert!(match_cidr(&ip, "192.168.0.0/16"));
    assert!(match_cidr(&ip, "192.168.1.10/32"));
    assert!(!match_cidr(&ip, "10.0.0.0/8"));
}

#[test]
fn matches_ipv6_cidr() {
    let ip = IpAddr::V6("2001:db8::1".parse::<Ipv6Addr>().unwrap());
    assert!(match_cidr(&ip, "2001:db8::/32"));
    assert!(match_cidr(&ip, "2001:db8::1/128"));
    assert!(!match_cidr(&ip, "2001:dead::/32"));
}

#[test]
fn rejects_invalid_cidr() {
    let ip = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1));
    assert!(!match_cidr(&ip, "not-a-cidr"));
    assert!(!match_cidr(&ip, "1.1.1.0/33"));
}
