use std::net::{IpAddr, Ipv6Addr};

use crate::{
    build_ipv6_tcp_ack,
    build_ipv6_tcp_fin_ack,
    build_ipv6_tcp_reset,
    build_ipv6_tcp_syn_ack,
    build_ipv6_udp_response,
    IpPacketSummary,
    TransportProtocol,
};

fn tcp_summary() -> IpPacketSummary {
    IpPacketSummary {
        version: 6,
        protocol: TransportProtocol::Tcp,
        src_ip: IpAddr::V6(Ipv6Addr::LOCALHOST),
        dst_ip: IpAddr::V6(Ipv6Addr::LOCALHOST),
        src_port: Some(12345),
        dst_port: Some(443),
        payload_len: 0,
        payload: Vec::new(),
        tcp_syn: true,
        tcp_fin: false,
        tcp_rst: false,
        tcp_ack: false,
        tcp_seq: Some(100),
        tcp_ack_seq: None,
    }
}

fn udp_summary() -> IpPacketSummary {
    IpPacketSummary {
        version: 6,
        protocol: TransportProtocol::Udp,
        src_ip: IpAddr::V6(Ipv6Addr::LOCALHOST),
        dst_ip: IpAddr::V6(Ipv6Addr::LOCALHOST),
        src_port: Some(5353),
        dst_port: Some(53),
        payload_len: 0,
        payload: Vec::new(),
        tcp_syn: false,
        tcp_fin: false,
        tcp_rst: false,
        tcp_ack: false,
        tcp_seq: None,
        tcp_ack_seq: None,
    }
}

#[test]
fn builds_ipv6_tcp_syn_ack() {
    let packet = build_ipv6_tcp_syn_ack(&tcp_summary(), 42).expect("packet");
    assert_eq!(packet[0] >> 4, 6);
    assert_eq!(packet[6], 6);
    assert_eq!(u16::from_be_bytes([packet[4], packet[5]]) as usize, 20);
    assert_eq!(packet[53], 0x12);
}

#[test]
fn builds_ipv6_tcp_reset() {
    let mut summary = tcp_summary();
    summary.tcp_ack = true;
    summary.tcp_ack_seq = Some(200);
    let packet = build_ipv6_tcp_reset(&summary).expect("packet");
    assert_eq!(packet[0] >> 4, 6);
    assert_eq!(packet[53], 0x04);
}

#[test]
fn builds_ipv6_tcp_ack() {
    let mut summary = tcp_summary();
    summary.tcp_syn = false;
    summary.tcp_ack = true;
    summary.tcp_seq = Some(101);
    summary.tcp_ack_seq = Some(201);
    let packet = build_ipv6_tcp_ack(&summary, 42, 201).expect("packet");
    assert_eq!(packet[0] >> 4, 6);
    assert_eq!(packet[53], 0x10);
}

#[test]
fn builds_ipv6_tcp_fin_ack() {
    let mut summary = tcp_summary();
    summary.tcp_syn = false;
    summary.tcp_fin = true;
    summary.tcp_ack = true;
    summary.tcp_seq = Some(101);
    summary.tcp_ack_seq = Some(201);
    let packet = build_ipv6_tcp_fin_ack(&summary, 42, 201).expect("packet");
    assert_eq!(packet[0] >> 4, 6);
    assert_eq!(packet[53], 0x11);
}

#[test]
fn builds_ipv6_udp_response() {
    let packet = build_ipv6_udp_response(&udp_summary(), b"abc").expect("packet");
    assert_eq!(packet[0] >> 4, 6);
    assert_eq!(packet[6], 17);
    assert_eq!(u16::from_be_bytes([packet[44], packet[45]]) as usize, 11);
    assert_eq!(&packet[48..], b"abc");
}
