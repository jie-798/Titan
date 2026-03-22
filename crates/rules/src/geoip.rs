use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::{Duration, Instant, SystemTime};

pub const DEFAULT_GEOIP_DB_PATH: &str = "data/geoip-apnic.raw";
const GEOIP_REFRESH_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
enum IpRange {
    V4 { network: u32, mask: u32 },
    V6 { network: u128, mask: u128 },
}

impl IpRange {
    fn contains(&self, ip: &IpAddr) -> bool {
        match (self, ip) {
            (Self::V4 { network, mask }, IpAddr::V4(ip)) => (u32::from(*ip) & mask) == *network,
            (Self::V6 { network, mask }, IpAddr::V6(ip)) => {
                (u128::from_be_bytes(ip.octets()) & mask) == *network
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct GeoIpDatabase {
    ranges: HashMap<String, Vec<IpRange>>,
}

impl GeoIpDatabase {
    pub fn from_path<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_str(&content)
    }

    pub fn from_str(content: &str) -> anyhow::Result<Self> {
        let mut ranges: HashMap<String, Vec<IpRange>> = HashMap::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() < 7 {
                continue;
            }

            let country = parts[1].trim();
            let family = parts[2].trim();
            let start = parts[3].trim();
            let value = parts[4].trim();
            let status = parts[6].trim();

            if country.is_empty() || country == "*" {
                continue;
            }
            if status != "allocated" && status != "assigned" {
                continue;
            }

            let entry = ranges.entry(country.to_ascii_uppercase()).or_default();
            match family {
                "ipv4" => {
                    let start_ip: Ipv4Addr = match start.parse() {
                        Ok(ip) => ip,
                        Err(_) => continue,
                    };
                    let count: u32 = match value.parse() {
                        Ok(count) => count,
                        Err(_) => continue,
                    };
                    entry.extend(ipv4_ranges(start_ip, count));
                }
                "ipv6" => {
                    let start_ip: Ipv6Addr = match start.parse() {
                        Ok(ip) => ip,
                        Err(_) => continue,
                    };
                    let prefix_len: u8 = match value.parse() {
                        Ok(prefix_len) if prefix_len <= 128 => prefix_len,
                        _ => continue,
                    };
                    let mask = if prefix_len == 0 {
                        0
                    } else {
                        u128::MAX << (128 - prefix_len)
                    };
                    entry.push(IpRange::V6 {
                        network: u128::from_be_bytes(start_ip.octets()) & mask,
                        mask,
                    });
                }
                _ => {}
            }
        }

        Ok(Self { ranges })
    }

    pub fn contains_country(&self, ip: &IpAddr, country: &str) -> bool {
        self.ranges
            .get(&country.to_ascii_uppercase())
            .is_some_and(|ranges| ranges.iter().any(|range| range.contains(ip)))
    }

    pub fn country_count(&self) -> usize {
        self.ranges.len()
    }
}

#[derive(Debug)]
struct GeoIpState {
    db: Option<GeoIpDatabase>,
    modified: Option<SystemTime>,
    last_checked: Option<Instant>,
}

#[derive(Debug)]
pub struct GeoIpMatcher {
    path: PathBuf,
    state: RwLock<GeoIpState>,
}

impl GeoIpMatcher {
    pub fn from_path<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let db = GeoIpDatabase::from_path(&path)?;
        let modified = file_modified(&path)?;

        Ok(Self {
            path,
            state: RwLock::new(GeoIpState {
                db: Some(db),
                modified,
                last_checked: None,
            }),
        })
    }

    pub fn contains_country(&self, ip: &IpAddr, country: &str) -> bool {
        self.refresh_if_needed();

        self.state
            .read()
            .ok()
            .and_then(|state| state.db.as_ref().map(|db| db.contains_country(ip, country)))
            .unwrap_or(false)
    }

    fn refresh_if_needed(&self) {
        let should_check = self.state.read().ok().is_none_or(|state| {
            state
                .last_checked
                .is_none_or(|last_checked| last_checked.elapsed() >= GEOIP_REFRESH_INTERVAL)
        });

        if !should_check {
            return;
        }

        let modified = match file_modified(&self.path) {
            Ok(modified) => modified,
            Err(err) => {
                tracing::warn!("failed to stat GEOIP file {}: {}", self.path.display(), err);
                if let Ok(mut state) = self.state.write() {
                    state.last_checked = Some(Instant::now());
                }
                return;
            }
        };

        let reload_needed = self
            .state
            .read()
            .ok()
            .is_none_or(|state| state.modified != modified);

        if let Ok(mut state) = self.state.write() {
            state.last_checked = Some(Instant::now());
        }

        if !reload_needed {
            return;
        }

        match GeoIpDatabase::from_path(&self.path) {
            Ok(db) => {
                tracing::info!(
                    "reloaded GEOIP database from {} ({} countries)",
                    self.path.display(),
                    db.country_count()
                );
                if let Ok(mut state) = self.state.write() {
                    state.db = Some(db);
                    state.modified = modified;
                }
            }
            Err(err) => {
                tracing::warn!(
                    "failed to reload GEOIP database from {}: {}",
                    self.path.display(),
                    err
                );
            }
        }
    }
}

fn file_modified(path: &Path) -> anyhow::Result<Option<SystemTime>> {
    Ok(std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok()))
}

fn ipv4_ranges(start_ip: Ipv4Addr, count: u32) -> Vec<IpRange> {
    let mut result = Vec::new();
    let mut current = u32::from(start_ip) as u64;
    let mut remaining = count as u64;

    while remaining > 0 {
        let max_align_prefix = if current == 0 {
            0
        } else {
            32 - (current.trailing_zeros() as u8)
        };

        let mut prefix = max_align_prefix;
        while prefix < 32 && block_size(prefix) > remaining {
            prefix += 1;
        }

        let size = block_size(prefix);
        let mask = if prefix == 0 {
            0
        } else {
            u32::MAX << (32 - prefix)
        };

        result.push(IpRange::V4 {
            network: (current as u32) & mask,
            mask,
        });

        current += size;
        remaining -= size;
    }

    result
}

fn block_size(prefix: u8) -> u64 {
    if prefix >= 32 {
        1
    } else {
        1_u64 << (32 - prefix)
    }
}
