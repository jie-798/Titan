use std::net::IpAddr;

/// Returns true when the given IP is contained by the CIDR range.
pub fn match_cidr(ip: &IpAddr, cidr: &str) -> bool {
    let Some((network, prefix_len)) = cidr.split_once('/') else {
        return false;
    };

    let Ok(prefix_len) = prefix_len.parse::<u8>() else {
        return false;
    };

    let Ok(network_ip) = network.parse::<IpAddr>() else {
        return false;
    };

    match (ip, network_ip) {
        (IpAddr::V4(ip), IpAddr::V4(network)) => {
            if prefix_len > 32 {
                return false;
            }

            let mask = if prefix_len == 0 {
                0
            } else {
                u32::MAX << (32 - prefix_len)
            };

            (u32::from(*ip) & mask) == (u32::from(network) & mask)
        }
        (IpAddr::V6(ip), IpAddr::V6(network)) => {
            if prefix_len > 128 {
                return false;
            }

            let mask = if prefix_len == 0 {
                0
            } else {
                u128::MAX << (128 - prefix_len)
            };

            (u128::from_be_bytes(ip.octets()) & mask)
                == (u128::from_be_bytes(network.octets()) & mask)
        }
        _ => false,
    }
}
