use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportProtocol {
    Tcp,
    Udp,
    Icmp,
    Icmpv6,
    Other(u8),
}

impl TransportProtocol {
    pub fn label(&self) -> String {
        match self {
            Self::Tcp => "tcp".to_string(),
            Self::Udp => "udp".to_string(),
            Self::Icmp => "icmp".to_string(),
            Self::Icmpv6 => "icmpv6".to_string(),
            Self::Other(value) => format!("proto-{value}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpPacketSummary {
    pub version: u8,
    pub protocol: TransportProtocol,
    pub src_ip: IpAddr,
    pub dst_ip: IpAddr,
    pub src_port: Option<u16>,
    pub dst_port: Option<u16>,
    pub payload_len: usize,
    pub payload: Vec<u8>,
    pub tcp_syn: bool,
    pub tcp_fin: bool,
    pub tcp_rst: bool,
    pub tcp_ack: bool,
    pub tcp_seq: Option<u32>,
    pub tcp_ack_seq: Option<u32>,
}

pub fn parse_ip_packet(packet: &[u8]) -> anyhow::Result<IpPacketSummary> {
    let Some(first) = packet.first() else {
        anyhow::bail!("empty packet");
    };

    match first >> 4 {
        4 => parse_ipv4(packet),
        6 => parse_ipv6(packet),
        version => anyhow::bail!("unsupported IP version: {version}"),
    }
}

fn parse_ipv4(packet: &[u8]) -> anyhow::Result<IpPacketSummary> {
    if packet.len() < 20 {
        anyhow::bail!("IPv4 packet is too short");
    }

    let ihl = ((packet[0] & 0x0f) as usize) * 4;
    if ihl < 20 || packet.len() < ihl {
        anyhow::bail!("invalid IPv4 header length");
    }

    let protocol = packet[9];
    let src_ip = IpAddr::V4(Ipv4Addr::new(packet[12], packet[13], packet[14], packet[15]));
    let dst_ip = IpAddr::V4(Ipv4Addr::new(packet[16], packet[17], packet[18], packet[19]));
    let payload = &packet[ihl..];
    let (
        protocol,
        src_port,
        dst_port,
        payload_len,
        transport_payload,
        tcp_syn,
        tcp_fin,
        tcp_rst,
        tcp_ack,
        tcp_seq,
        tcp_ack_seq,
    ) =
        parse_transport(protocol, payload);

    Ok(IpPacketSummary {
        version: 4,
        protocol,
        src_ip,
        dst_ip,
        src_port,
        dst_port,
        payload_len,
        payload: transport_payload,
        tcp_syn,
        tcp_fin,
        tcp_rst,
        tcp_ack,
        tcp_seq,
        tcp_ack_seq,
    })
}

fn parse_ipv6(packet: &[u8]) -> anyhow::Result<IpPacketSummary> {
    if packet.len() < 40 {
        anyhow::bail!("IPv6 packet is too short");
    }

    let next_header = packet[6];
    let src_ip = IpAddr::V6(Ipv6Addr::from(<[u8; 16]>::try_from(&packet[8..24])?));
    let dst_ip = IpAddr::V6(Ipv6Addr::from(<[u8; 16]>::try_from(&packet[24..40])?));
    let payload = &packet[40..];
    let (
        protocol,
        src_port,
        dst_port,
        payload_len,
        transport_payload,
        tcp_syn,
        tcp_fin,
        tcp_rst,
        tcp_ack,
        tcp_seq,
        tcp_ack_seq,
    ) =
        parse_transport(next_header, payload);

    Ok(IpPacketSummary {
        version: 6,
        protocol,
        src_ip,
        dst_ip,
        src_port,
        dst_port,
        payload_len,
        payload: transport_payload,
        tcp_syn,
        tcp_fin,
        tcp_rst,
        tcp_ack,
        tcp_seq,
        tcp_ack_seq,
    })
}

fn parse_transport(
    protocol: u8,
    payload: &[u8],
) -> (
    TransportProtocol,
    Option<u16>,
    Option<u16>,
    usize,
    Vec<u8>,
    bool,
    bool,
    bool,
    bool,
    Option<u32>,
    Option<u32>,
) {
    match protocol {
        6 if payload.len() >= 20 => {
            let flags = payload[13];
            let header_len = usize::from(payload[12] >> 4) * 4;
            let data = if header_len <= payload.len() {
                payload[header_len..].to_vec()
            } else {
                Vec::new()
            };
            (
                TransportProtocol::Tcp,
                Some(u16::from_be_bytes([payload[0], payload[1]])),
                Some(u16::from_be_bytes([payload[2], payload[3]])),
                data.len(),
                data,
                flags & 0x02 != 0,
                flags & 0x01 != 0,
                flags & 0x04 != 0,
                flags & 0x10 != 0,
                payload
                    .get(4..8)
                    .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
                    .map(u32::from_be_bytes),
                payload
                    .get(8..12)
                    .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
                    .map(u32::from_be_bytes),
            )
        }
        6 if payload.len() >= 4 => (
            TransportProtocol::Tcp,
            Some(u16::from_be_bytes([payload[0], payload[1]])),
            Some(u16::from_be_bytes([payload[2], payload[3]])),
            0,
            Vec::new(),
            false,
            false,
            false,
            false,
            None,
            None,
        ),
        17 if payload.len() >= 4 => (
            TransportProtocol::Udp,
            Some(u16::from_be_bytes([payload[0], payload[1]])),
            Some(u16::from_be_bytes([payload[2], payload[3]])),
            payload.len().saturating_sub(8),
            payload.get(8..).unwrap_or_default().to_vec(),
            false,
            false,
            false,
            false,
            None,
            None,
        ),
        1 => (TransportProtocol::Icmp, None, None, payload.len(), payload.to_vec(), false, false, false, false, None, None),
        58 => (TransportProtocol::Icmpv6, None, None, payload.len(), payload.to_vec(), false, false, false, false, None, None),
        other => (TransportProtocol::Other(other), None, None, payload.len(), payload.to_vec(), false, false, false, false, None, None),
    }
}

pub fn build_ipv4_tcp_reset(summary: &IpPacketSummary) -> Option<Vec<u8>> {
    if summary.version != 4 || !matches!(summary.protocol, TransportProtocol::Tcp) {
        return None;
    }

    let src_ip = match summary.dst_ip {
        IpAddr::V4(ip) => ip.octets(),
        IpAddr::V6(_) => return None,
    };
    let dst_ip = match summary.src_ip {
        IpAddr::V4(ip) => ip.octets(),
        IpAddr::V6(_) => return None,
    };
    let src_port = summary.dst_port?;
    let dst_port = summary.src_port?;
    let seq = if summary.tcp_ack {
        summary.tcp_ack_seq.unwrap_or(0)
    } else {
        0
    };
    let ack = if summary.tcp_ack {
        0
    } else {
        summary
            .tcp_seq
            .unwrap_or(0)
            .wrapping_add(summary.payload_len as u32)
            .wrapping_add(u32::from(summary.tcp_syn))
            .wrapping_add(u32::from(summary.tcp_fin))
    };
    let flags = if summary.tcp_ack { 0x04 } else { 0x14 };

    Some(build_ipv4_tcp_packet(
        src_ip, dst_ip, src_port, dst_port, seq, ack, flags, &[],
    ))
}

pub fn build_ipv4_tcp_syn_ack(summary: &IpPacketSummary, server_seq: u32) -> Option<Vec<u8>> {
    if summary.version != 4 || !matches!(summary.protocol, TransportProtocol::Tcp) || !summary.tcp_syn {
        return None;
    }

    let src_ip = match summary.dst_ip {
        IpAddr::V4(ip) => ip.octets(),
        IpAddr::V6(_) => return None,
    };
    let dst_ip = match summary.src_ip {
        IpAddr::V4(ip) => ip.octets(),
        IpAddr::V6(_) => return None,
    };
    let src_port = summary.dst_port?;
    let dst_port = summary.src_port?;
    let ack = summary.tcp_seq.unwrap_or(0).wrapping_add(1);

    Some(build_ipv4_tcp_packet(
        src_ip, dst_ip, src_port, dst_port, server_seq, ack, 0x12, &[],
    ))
}

pub fn build_ipv4_tcp_data(
    summary: &IpPacketSummary,
    server_seq: u32,
    client_ack: u32,
    payload: &[u8],
) -> Option<Vec<u8>> {
    if summary.version != 4 || !matches!(summary.protocol, TransportProtocol::Tcp) {
        return None;
    }
    let src_ip = match summary.dst_ip {
        IpAddr::V4(ip) => ip.octets(),
        IpAddr::V6(_) => return None,
    };
    let dst_ip = match summary.src_ip {
        IpAddr::V4(ip) => ip.octets(),
        IpAddr::V6(_) => return None,
    };
    let src_port = summary.dst_port?;
    let dst_port = summary.src_port?;
    Some(build_ipv4_tcp_packet(
        src_ip, dst_ip, src_port, dst_port, server_seq, client_ack, 0x18, payload,
    ))
}

pub fn build_ipv4_tcp_ack(
    summary: &IpPacketSummary,
    server_seq: u32,
    client_ack: u32,
) -> Option<Vec<u8>> {
    if summary.version != 4 || !matches!(summary.protocol, TransportProtocol::Tcp) {
        return None;
    }
    let src_ip = match summary.dst_ip {
        IpAddr::V4(ip) => ip.octets(),
        IpAddr::V6(_) => return None,
    };
    let dst_ip = match summary.src_ip {
        IpAddr::V4(ip) => ip.octets(),
        IpAddr::V6(_) => return None,
    };
    let src_port = summary.dst_port?;
    let dst_port = summary.src_port?;
    Some(build_ipv4_tcp_packet(
        src_ip, dst_ip, src_port, dst_port, server_seq, client_ack, 0x10, &[],
    ))
}

pub fn build_ipv4_tcp_fin_ack(
    summary: &IpPacketSummary,
    server_seq: u32,
    client_ack: u32,
) -> Option<Vec<u8>> {
    if summary.version != 4 || !matches!(summary.protocol, TransportProtocol::Tcp) {
        return None;
    }
    let src_ip = match summary.dst_ip {
        IpAddr::V4(ip) => ip.octets(),
        IpAddr::V6(_) => return None,
    };
    let dst_ip = match summary.src_ip {
        IpAddr::V4(ip) => ip.octets(),
        IpAddr::V6(_) => return None,
    };
    let src_port = summary.dst_port?;
    let dst_port = summary.src_port?;
    Some(build_ipv4_tcp_packet(
        src_ip, dst_ip, src_port, dst_port, server_seq, client_ack, 0x11, &[],
    ))
}

pub fn build_ipv4_udp_response(
    summary: &IpPacketSummary,
    payload: &[u8],
) -> Option<Vec<u8>> {
    if summary.version != 4 || !matches!(summary.protocol, TransportProtocol::Udp) {
        return None;
    }

    let src_ip = match summary.dst_ip {
        IpAddr::V4(ip) => ip.octets(),
        IpAddr::V6(_) => return None,
    };
    let dst_ip = match summary.src_ip {
        IpAddr::V4(ip) => ip.octets(),
        IpAddr::V6(_) => return None,
    };
    let src_port = summary.dst_port?;
    let dst_port = summary.src_port?;

    Some(build_ipv4_udp_packet(
        src_ip, dst_ip, src_port, dst_port, payload,
    ))
}

pub fn build_ipv6_tcp_reset(summary: &IpPacketSummary) -> Option<Vec<u8>> {
    if summary.version != 6 || !matches!(summary.protocol, TransportProtocol::Tcp) {
        return None;
    }

    let src_ip = match summary.dst_ip {
        IpAddr::V6(ip) => ip.octets(),
        IpAddr::V4(_) => return None,
    };
    let dst_ip = match summary.src_ip {
        IpAddr::V6(ip) => ip.octets(),
        IpAddr::V4(_) => return None,
    };
    let src_port = summary.dst_port?;
    let dst_port = summary.src_port?;
    let seq = if summary.tcp_ack {
        summary.tcp_ack_seq.unwrap_or(0)
    } else {
        0
    };
    let ack = if summary.tcp_ack {
        0
    } else {
        summary
            .tcp_seq
            .unwrap_or(0)
            .wrapping_add(summary.payload_len as u32)
            .wrapping_add(u32::from(summary.tcp_syn))
            .wrapping_add(u32::from(summary.tcp_fin))
    };
    let flags = if summary.tcp_ack { 0x04 } else { 0x14 };

    Some(build_ipv6_tcp_packet(
        src_ip, dst_ip, src_port, dst_port, seq, ack, flags, &[],
    ))
}

pub fn build_ipv6_tcp_syn_ack(summary: &IpPacketSummary, server_seq: u32) -> Option<Vec<u8>> {
    if summary.version != 6 || !matches!(summary.protocol, TransportProtocol::Tcp) || !summary.tcp_syn {
        return None;
    }

    let src_ip = match summary.dst_ip {
        IpAddr::V6(ip) => ip.octets(),
        IpAddr::V4(_) => return None,
    };
    let dst_ip = match summary.src_ip {
        IpAddr::V6(ip) => ip.octets(),
        IpAddr::V4(_) => return None,
    };
    let src_port = summary.dst_port?;
    let dst_port = summary.src_port?;
    let ack = summary.tcp_seq.unwrap_or(0).wrapping_add(1);

    Some(build_ipv6_tcp_packet(
        src_ip, dst_ip, src_port, dst_port, server_seq, ack, 0x12, &[],
    ))
}

pub fn build_ipv6_tcp_data(
    summary: &IpPacketSummary,
    server_seq: u32,
    client_ack: u32,
    payload: &[u8],
) -> Option<Vec<u8>> {
    if summary.version != 6 || !matches!(summary.protocol, TransportProtocol::Tcp) {
        return None;
    }
    let src_ip = match summary.dst_ip {
        IpAddr::V6(ip) => ip.octets(),
        IpAddr::V4(_) => return None,
    };
    let dst_ip = match summary.src_ip {
        IpAddr::V6(ip) => ip.octets(),
        IpAddr::V4(_) => return None,
    };
    let src_port = summary.dst_port?;
    let dst_port = summary.src_port?;
    Some(build_ipv6_tcp_packet(
        src_ip, dst_ip, src_port, dst_port, server_seq, client_ack, 0x18, payload,
    ))
}

pub fn build_ipv6_tcp_ack(
    summary: &IpPacketSummary,
    server_seq: u32,
    client_ack: u32,
) -> Option<Vec<u8>> {
    if summary.version != 6 || !matches!(summary.protocol, TransportProtocol::Tcp) {
        return None;
    }
    let src_ip = match summary.dst_ip {
        IpAddr::V6(ip) => ip.octets(),
        IpAddr::V4(_) => return None,
    };
    let dst_ip = match summary.src_ip {
        IpAddr::V6(ip) => ip.octets(),
        IpAddr::V4(_) => return None,
    };
    let src_port = summary.dst_port?;
    let dst_port = summary.src_port?;
    Some(build_ipv6_tcp_packet(
        src_ip, dst_ip, src_port, dst_port, server_seq, client_ack, 0x10, &[],
    ))
}

pub fn build_ipv6_tcp_fin_ack(
    summary: &IpPacketSummary,
    server_seq: u32,
    client_ack: u32,
) -> Option<Vec<u8>> {
    if summary.version != 6 || !matches!(summary.protocol, TransportProtocol::Tcp) {
        return None;
    }
    let src_ip = match summary.dst_ip {
        IpAddr::V6(ip) => ip.octets(),
        IpAddr::V4(_) => return None,
    };
    let dst_ip = match summary.src_ip {
        IpAddr::V6(ip) => ip.octets(),
        IpAddr::V4(_) => return None,
    };
    let src_port = summary.dst_port?;
    let dst_port = summary.src_port?;
    Some(build_ipv6_tcp_packet(
        src_ip, dst_ip, src_port, dst_port, server_seq, client_ack, 0x11, &[],
    ))
}

pub fn build_ipv6_udp_response(
    summary: &IpPacketSummary,
    payload: &[u8],
) -> Option<Vec<u8>> {
    if summary.version != 6 || !matches!(summary.protocol, TransportProtocol::Udp) {
        return None;
    }

    let src_ip = match summary.dst_ip {
        IpAddr::V6(ip) => ip.octets(),
        IpAddr::V4(_) => return None,
    };
    let dst_ip = match summary.src_ip {
        IpAddr::V6(ip) => ip.octets(),
        IpAddr::V4(_) => return None,
    };
    let src_port = summary.dst_port?;
    let dst_port = summary.src_port?;

    Some(build_ipv6_udp_packet(
        src_ip, dst_ip, src_port, dst_port, payload,
    ))
}

pub fn build_ipv6_icmp_port_unreachable(original_packet: &[u8]) -> Option<Vec<u8>> {
    if original_packet.len() < 40 || (original_packet[0] >> 4) != 6 {
        return None;
    }

    let payload_offset = 40;
    let quoted_len = original_packet.len().min(payload_offset + 8);
    let icmp_len = 8 + quoted_len;
    let total_len = 40 + icmp_len;
    let mut packet = vec![0_u8; total_len];

    packet[0] = 0x60;
    packet[4..6].copy_from_slice(&(icmp_len as u16).to_be_bytes());
    packet[6] = 58;
    packet[7] = 64;
    packet[8..24].copy_from_slice(&original_packet[24..40]);
    packet[24..40].copy_from_slice(&original_packet[8..24]);

    packet[40] = 1;
    packet[41] = 4;
    packet[42..44].copy_from_slice(&0_u16.to_be_bytes());
    packet[44..48].copy_from_slice(&0_u32.to_be_bytes());
    packet[48..48 + quoted_len].copy_from_slice(&original_packet[..quoted_len]);
    let src = <[u8; 16]>::try_from(&packet[8..24]).ok()?;
    let dst = <[u8; 16]>::try_from(&packet[24..40]).ok()?;
    let checksum = icmpv6_checksum(&packet[40..], &src, &dst);
    packet[42..44].copy_from_slice(&checksum.to_be_bytes());
    Some(packet)
}

pub fn build_ipv4_icmp_port_unreachable(original_packet: &[u8]) -> Option<Vec<u8>> {
    if original_packet.len() < 20 || (original_packet[0] >> 4) != 4 {
        return None;
    }

    let ihl = usize::from(original_packet[0] & 0x0f) * 4;
    if ihl < 20 || original_packet.len() < ihl {
        return None;
    }

    let src_ip = <[u8; 4]>::try_from(&original_packet[16..20]).ok()?;
    let dst_ip = <[u8; 4]>::try_from(&original_packet[12..16]).ok()?;
    let quoted_len = original_packet.len().min(ihl + 8);
    let icmp_len = 8 + quoted_len;
    let total_len = 20 + icmp_len;
    let mut packet = vec![0_u8; total_len];

    packet[0] = 0x45;
    packet[1] = 0;
    packet[2..4].copy_from_slice(&(total_len as u16).to_be_bytes());
    packet[4..6].copy_from_slice(&0_u16.to_be_bytes());
    packet[6..8].copy_from_slice(&0_u16.to_be_bytes());
    packet[8] = 64;
    packet[9] = 1;
    packet[12..16].copy_from_slice(&src_ip);
    packet[16..20].copy_from_slice(&dst_ip);
    let ip_checksum = checksum(&packet[..20]);
    packet[10..12].copy_from_slice(&ip_checksum.to_be_bytes());

    packet[20] = 3;
    packet[21] = 3;
    packet[22..24].copy_from_slice(&0_u16.to_be_bytes());
    packet[24..28].copy_from_slice(&0_u32.to_be_bytes());
    packet[28..28 + quoted_len].copy_from_slice(&original_packet[..quoted_len]);
    let icmp_checksum = checksum(&packet[20..]);
    packet[22..24].copy_from_slice(&icmp_checksum.to_be_bytes());
    Some(packet)
}

fn tcp_ipv4_checksum(segment: &[u8], src_ip: &[u8; 4], dst_ip: &[u8; 4]) -> u16 {
    let mut pseudo = Vec::with_capacity(12 + segment.len());
    pseudo.extend_from_slice(src_ip);
    pseudo.extend_from_slice(dst_ip);
    pseudo.push(0);
    pseudo.push(6);
    pseudo.extend_from_slice(&(segment.len() as u16).to_be_bytes());
    pseudo.extend_from_slice(segment);
    checksum(&pseudo)
}

fn tcp_ipv6_checksum(segment: &[u8], src_ip: &[u8; 16], dst_ip: &[u8; 16]) -> u16 {
    let mut pseudo = Vec::with_capacity(40 + segment.len());
    pseudo.extend_from_slice(src_ip);
    pseudo.extend_from_slice(dst_ip);
    pseudo.extend_from_slice(&(segment.len() as u32).to_be_bytes());
    pseudo.extend_from_slice(&[0, 0, 0]);
    pseudo.push(6);
    pseudo.extend_from_slice(segment);
    checksum(&pseudo)
}

fn checksum(bytes: &[u8]) -> u16 {
    let mut sum = 0_u32;
    let mut chunks = bytes.chunks_exact(2);
    for chunk in &mut chunks {
        sum += u16::from_be_bytes([chunk[0], chunk[1]]) as u32;
    }
    if let Some(&last) = chunks.remainder().first() {
        sum += u16::from_be_bytes([last, 0]) as u32;
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

fn udp_ipv4_checksum(segment: &[u8], src_ip: &[u8; 4], dst_ip: &[u8; 4]) -> u16 {
    let mut pseudo = Vec::with_capacity(12 + segment.len());
    pseudo.extend_from_slice(src_ip);
    pseudo.extend_from_slice(dst_ip);
    pseudo.push(0);
    pseudo.push(17);
    pseudo.extend_from_slice(&(segment.len() as u16).to_be_bytes());
    pseudo.extend_from_slice(segment);
    checksum(&pseudo)
}

fn udp_ipv6_checksum(segment: &[u8], src_ip: &[u8; 16], dst_ip: &[u8; 16]) -> u16 {
    let mut pseudo = Vec::with_capacity(40 + segment.len());
    pseudo.extend_from_slice(src_ip);
    pseudo.extend_from_slice(dst_ip);
    pseudo.extend_from_slice(&(segment.len() as u32).to_be_bytes());
    pseudo.extend_from_slice(&[0, 0, 0]);
    pseudo.push(17);
    pseudo.extend_from_slice(segment);
    checksum(&pseudo)
}

fn icmpv6_checksum(message: &[u8], src_ip: &[u8; 16], dst_ip: &[u8; 16]) -> u16 {
    let mut pseudo = Vec::with_capacity(40 + message.len());
    pseudo.extend_from_slice(src_ip);
    pseudo.extend_from_slice(dst_ip);
    pseudo.extend_from_slice(&(message.len() as u32).to_be_bytes());
    pseudo.extend_from_slice(&[0, 0, 0]);
    pseudo.push(58);
    pseudo.extend_from_slice(message);
    checksum(&pseudo)
}

fn build_ipv4_tcp_packet(
    src_ip: [u8; 4],
    dst_ip: [u8; 4],
    src_port: u16,
    dst_port: u16,
    seq: u32,
    ack: u32,
    flags: u8,
    payload: &[u8],
) -> Vec<u8> {
    let total_len = 20 + 20 + payload.len();
    let mut packet = vec![0_u8; total_len];
    packet[0] = 0x45;
    packet[1] = 0;
    packet[2..4].copy_from_slice(&(total_len as u16).to_be_bytes());
    packet[4..6].copy_from_slice(&0_u16.to_be_bytes());
    packet[6..8].copy_from_slice(&0_u16.to_be_bytes());
    packet[8] = 64;
    packet[9] = 6;
    packet[10..12].copy_from_slice(&0_u16.to_be_bytes());
    packet[12..16].copy_from_slice(&src_ip);
    packet[16..20].copy_from_slice(&dst_ip);
    let ip_checksum = checksum(&packet[..20]);
    packet[10..12].copy_from_slice(&ip_checksum.to_be_bytes());

    packet[20..22].copy_from_slice(&src_port.to_be_bytes());
    packet[22..24].copy_from_slice(&dst_port.to_be_bytes());
    packet[24..28].copy_from_slice(&seq.to_be_bytes());
    packet[28..32].copy_from_slice(&ack.to_be_bytes());
    packet[32] = 5 << 4;
    packet[33] = flags;
    packet[34..36].copy_from_slice(&64240_u16.to_be_bytes());
    packet[36..38].copy_from_slice(&0_u16.to_be_bytes());
    packet[38..40].copy_from_slice(&0_u16.to_be_bytes());
    packet[40..].copy_from_slice(payload);
    let tcp_checksum = tcp_ipv4_checksum(&packet[20..], &src_ip, &dst_ip);
    packet[36..38].copy_from_slice(&tcp_checksum.to_be_bytes());
    packet
}

fn build_ipv6_tcp_packet(
    src_ip: [u8; 16],
    dst_ip: [u8; 16],
    src_port: u16,
    dst_port: u16,
    seq: u32,
    ack: u32,
    flags: u8,
    payload: &[u8],
) -> Vec<u8> {
    let total_len = 40 + 20 + payload.len();
    let mut packet = vec![0_u8; total_len];
    packet[0] = 0x60;
    packet[4..6].copy_from_slice(&((20 + payload.len()) as u16).to_be_bytes());
    packet[6] = 6;
    packet[7] = 64;
    packet[8..24].copy_from_slice(&src_ip);
    packet[24..40].copy_from_slice(&dst_ip);

    packet[40..42].copy_from_slice(&src_port.to_be_bytes());
    packet[42..44].copy_from_slice(&dst_port.to_be_bytes());
    packet[44..48].copy_from_slice(&seq.to_be_bytes());
    packet[48..52].copy_from_slice(&ack.to_be_bytes());
    packet[52] = 5 << 4;
    packet[53] = flags;
    packet[54..56].copy_from_slice(&64240_u16.to_be_bytes());
    packet[56..58].copy_from_slice(&0_u16.to_be_bytes());
    packet[58..60].copy_from_slice(&0_u16.to_be_bytes());
    packet[60..].copy_from_slice(payload);
    let tcp_checksum = tcp_ipv6_checksum(&packet[40..], &src_ip, &dst_ip);
    packet[56..58].copy_from_slice(&tcp_checksum.to_be_bytes());
    packet
}

fn build_ipv4_udp_packet(
    src_ip: [u8; 4],
    dst_ip: [u8; 4],
    src_port: u16,
    dst_port: u16,
    payload: &[u8],
) -> Vec<u8> {
    let udp_len = 8 + payload.len();
    let total_len = 20 + udp_len;
    let mut packet = vec![0_u8; total_len];
    packet[0] = 0x45;
    packet[1] = 0;
    packet[2..4].copy_from_slice(&(total_len as u16).to_be_bytes());
    packet[4..6].copy_from_slice(&0_u16.to_be_bytes());
    packet[6..8].copy_from_slice(&0_u16.to_be_bytes());
    packet[8] = 64;
    packet[9] = 17;
    packet[10..12].copy_from_slice(&0_u16.to_be_bytes());
    packet[12..16].copy_from_slice(&src_ip);
    packet[16..20].copy_from_slice(&dst_ip);
    let ip_checksum = checksum(&packet[..20]);
    packet[10..12].copy_from_slice(&ip_checksum.to_be_bytes());

    packet[20..22].copy_from_slice(&src_port.to_be_bytes());
    packet[22..24].copy_from_slice(&dst_port.to_be_bytes());
    packet[24..26].copy_from_slice(&(udp_len as u16).to_be_bytes());
    packet[26..28].copy_from_slice(&0_u16.to_be_bytes());
    packet[28..].copy_from_slice(payload);

    let checksum = udp_ipv4_checksum(&packet[20..], &src_ip, &dst_ip);
    packet[26..28].copy_from_slice(&checksum.to_be_bytes());
    packet
}

fn build_ipv6_udp_packet(
    src_ip: [u8; 16],
    dst_ip: [u8; 16],
    src_port: u16,
    dst_port: u16,
    payload: &[u8],
) -> Vec<u8> {
    let udp_len = 8 + payload.len();
    let total_len = 40 + udp_len;
    let mut packet = vec![0_u8; total_len];
    packet[0] = 0x60;
    packet[4..6].copy_from_slice(&(udp_len as u16).to_be_bytes());
    packet[6] = 17;
    packet[7] = 64;
    packet[8..24].copy_from_slice(&src_ip);
    packet[24..40].copy_from_slice(&dst_ip);

    packet[40..42].copy_from_slice(&src_port.to_be_bytes());
    packet[42..44].copy_from_slice(&dst_port.to_be_bytes());
    packet[44..46].copy_from_slice(&(udp_len as u16).to_be_bytes());
    packet[46..48].copy_from_slice(&0_u16.to_be_bytes());
    packet[48..].copy_from_slice(payload);

    let checksum = udp_ipv6_checksum(&packet[40..], &src_ip, &dst_ip);
    packet[46..48].copy_from_slice(&checksum.to_be_bytes());
    packet
}
