use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::atomic::{AtomicU16, Ordering};

use tokio::net::UdpSocket;

use titan_config::DnsConfig;

pub struct DnsResolver {
    nameservers: Vec<SocketAddr>,
    next_id: AtomicU16,
}

impl DnsResolver {
    pub fn new(config: &DnsConfig) -> anyhow::Result<Self> {
        let nameserver_values = if config.nameserver.is_empty() {
            vec!["8.8.8.8".to_string(), "8.8.4.4".to_string()]
        } else {
            config.nameserver.clone()
        };

        let nameservers = nameserver_values
            .into_iter()
            .map(parse_nameserver)
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(Self {
            nameservers,
            next_id: AtomicU16::new(1),
        })
    }

    pub async fn resolve(&self, domain: &str) -> anyhow::Result<Vec<IpAddr>> {
        let mut ips = Vec::new();

        for record_type in [RecordType::A, RecordType::Aaaa] {
            for nameserver in &self.nameservers {
                match self.query_nameserver(domain, record_type, *nameserver).await {
                    Ok(mut resolved) if !resolved.is_empty() => {
                        ips.append(&mut resolved);
                        break;
                    }
                    Ok(_) => continue,
                    Err(err) => {
                        tracing::debug!(
                            "dns query for {} via {} failed: {}",
                            domain,
                            nameserver,
                            err
                        );
                    }
                }
            }
        }

        if ips.is_empty() {
            anyhow::bail!("DNS resolution failed: {}", domain);
        }

        Ok(ips)
    }

    pub async fn resolve_one(&self, domain: &str) -> anyhow::Result<IpAddr> {
        let ips = self.resolve(domain).await?;
        ips.into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("DNS resolution failed: {}", domain))
    }

    async fn query_nameserver(
        &self,
        domain: &str,
        record_type: RecordType,
        nameserver: SocketAddr,
    ) -> anyhow::Result<Vec<IpAddr>> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let request = build_query(domain, record_type, id)?;
        let bind_addr = match nameserver {
            SocketAddr::V4(_) => "0.0.0.0:0",
            SocketAddr::V6(_) => "[::]:0",
        };
        let socket = UdpSocket::bind(bind_addr).await?;
        socket.send_to(&request, nameserver).await?;

        let mut response = [0_u8; 2048];
        let recv = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            socket.recv_from(&mut response),
        )
        .await??;

        parse_response(&response[..recv.0], id, record_type)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RecordType {
    A,
    Aaaa,
}

impl RecordType {
    fn as_u16(self) -> u16 {
        match self {
            Self::A => 1,
            Self::Aaaa => 28,
        }
    }
}

fn parse_nameserver(value: String) -> anyhow::Result<SocketAddr> {
    if let Ok(addr) = value.parse::<SocketAddr>() {
        return Ok(addr);
    }

    if let Ok(ip) = value.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, 53));
    }

    anyhow::bail!("invalid nameserver: {}", value)
}

fn build_query(domain: &str, record_type: RecordType, id: u16) -> anyhow::Result<Vec<u8>> {
    let mut query = Vec::with_capacity(512);
    query.extend_from_slice(&id.to_be_bytes());
    query.extend_from_slice(&0x0100u16.to_be_bytes());
    query.extend_from_slice(&1u16.to_be_bytes());
    query.extend_from_slice(&0u16.to_be_bytes());
    query.extend_from_slice(&0u16.to_be_bytes());
    query.extend_from_slice(&0u16.to_be_bytes());

    for label in domain.trim_end_matches('.').split('.') {
        if label.is_empty() || label.len() > 63 {
            anyhow::bail!("invalid domain label in {}", domain);
        }
        query.push(label.len() as u8);
        query.extend_from_slice(label.as_bytes());
    }
    query.push(0);
    query.extend_from_slice(&record_type.as_u16().to_be_bytes());
    query.extend_from_slice(&1u16.to_be_bytes());
    Ok(query)
}

fn parse_response(response: &[u8], id: u16, record_type: RecordType) -> anyhow::Result<Vec<IpAddr>> {
    if response.len() < 12 {
        anyhow::bail!("short DNS response");
    }

    let response_id = u16::from_be_bytes([response[0], response[1]]);
    if response_id != id {
        anyhow::bail!("mismatched DNS response id");
    }

    let flags = u16::from_be_bytes([response[2], response[3]]);
    if (flags & 0x8000) == 0 {
        anyhow::bail!("not a DNS response");
    }
    let rcode = flags & 0x000f;
    if rcode != 0 {
        anyhow::bail!("dns server returned error code {}", rcode);
    }

    let qdcount = u16::from_be_bytes([response[4], response[5]]) as usize;
    let ancount = u16::from_be_bytes([response[6], response[7]]) as usize;
    let mut cursor = 12;

    for _ in 0..qdcount {
        skip_name(response, &mut cursor)?;
        if cursor + 4 > response.len() {
            anyhow::bail!("truncated DNS question");
        }
        cursor += 4;
    }

    let mut ips = Vec::new();
    for _ in 0..ancount {
        skip_name(response, &mut cursor)?;
        if cursor + 10 > response.len() {
            anyhow::bail!("truncated DNS answer");
        }
        let answer_type = u16::from_be_bytes([response[cursor], response[cursor + 1]]);
        let answer_class = u16::from_be_bytes([response[cursor + 2], response[cursor + 3]]);
        let rdlen = u16::from_be_bytes([response[cursor + 8], response[cursor + 9]]) as usize;
        cursor += 10;
        if cursor + rdlen > response.len() {
            anyhow::bail!("truncated DNS answer data");
        }

        if answer_class == 1 && answer_type == record_type.as_u16() {
            match record_type {
                RecordType::A if rdlen == 4 => {
                    ips.push(IpAddr::V4(Ipv4Addr::new(
                        response[cursor],
                        response[cursor + 1],
                        response[cursor + 2],
                        response[cursor + 3],
                    )));
                }
                RecordType::Aaaa if rdlen == 16 => {
                    let mut octets = [0_u8; 16];
                    octets.copy_from_slice(&response[cursor..cursor + 16]);
                    ips.push(IpAddr::V6(Ipv6Addr::from(octets)));
                }
                _ => {}
            }
        }

        cursor += rdlen;
    }

    Ok(ips)
}

#[cfg(test)]
pub(crate) fn parse_nameserver_for_test(value: &str) -> anyhow::Result<SocketAddr> {
    parse_nameserver(value.to_string())
}

#[cfg(test)]
pub(crate) fn parse_response_for_test(
    response: &[u8],
    id: u16,
    record_type: u16,
) -> anyhow::Result<Vec<IpAddr>> {
    let record_type = match record_type {
        1 => RecordType::A,
        28 => RecordType::Aaaa,
        _ => anyhow::bail!("unsupported test record type: {}", record_type),
    };

    parse_response(response, id, record_type)
}

fn skip_name(response: &[u8], cursor: &mut usize) -> anyhow::Result<()> {
    let mut jumped = false;
    let mut offset = *cursor;
    let mut jumps = 0;

    loop {
        if offset >= response.len() {
            anyhow::bail!("truncated DNS name");
        }
        let len = response[offset];
        if (len & 0xC0) == 0xC0 {
            if offset + 1 >= response.len() {
                anyhow::bail!("truncated DNS compression pointer");
            }
            if !jumped {
                *cursor = offset + 2;
            }
            let pointer = (((len as usize) & 0x3F) << 8) | response[offset + 1] as usize;
            offset = pointer;
            jumped = true;
            jumps += 1;
            if jumps > 16 {
                anyhow::bail!("too many DNS compression jumps");
            }
            continue;
        }

        if len == 0 {
            if !jumped {
                *cursor = offset + 1;
            }
            return Ok(());
        }

        offset += 1 + len as usize;
    }
}
