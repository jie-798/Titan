use std::net::{IpAddr, Ipv4Addr};
use std::process::Command;

use crate::config::{DnsServer, TunAddress, TunConfig, TunRoute, TunStack, TunSummary};
use crate::{WintunLibrary, WintunResult};

#[derive(Debug, Clone)]
pub struct WindowsTunPlan {
    pub stack: TunStack,
    pub adapter_name: String,
    pub interface_name: Option<String>,
    pub addresses: Vec<TunAddress>,
    pub gateway: Option<IpAddr>,
    pub dns_servers: Vec<DnsServer>,
    pub mtu: Option<u32>,
    pub route_all_traffic: bool,
    pub strict_route: bool,
    pub dns_hijack: Vec<String>,
    pub exclude_routes: Vec<TunAddress>,
    pub routes: Vec<TunRoute>,
}

pub struct ActiveWindowsTun {
    _library: WintunLibrary,
    _adapter: crate::Adapter,
    _session: crate::Session,
    interface_name: String,
    created_adapter: bool,
    driver_version: Option<u32>,
    commands: Vec<String>,
    rollback_commands: Vec<String>,
}

impl WindowsTunPlan {
    pub fn from_config(config: &TunConfig) -> Self {
        Self {
            stack: config.stack.clone(),
            adapter_name: config.adapter_name.clone(),
            interface_name: config.interface_name.clone(),
            addresses: config.addresses.clone(),
            gateway: config.gateway,
            dns_servers: config.dns_servers.clone(),
            mtu: config.mtu,
            route_all_traffic: config.route_all_traffic,
            strict_route: config.strict_route,
            dns_hijack: config.dns_hijack.clone(),
            exclude_routes: config.exclude_routes.clone(),
            routes: config.routes.clone(),
        }
    }

    pub fn summary(&self) -> TunSummary {
        TunSummary {
            stack: self.stack.clone(),
            adapter_name: self.adapter_name.clone(),
            interface_name: self.interface_name.clone(),
            address_count: self.addresses.len(),
            dns_count: self.dns_servers.len(),
            dns_hijack_count: self.dns_hijack.len(),
            exclude_route_count: self.exclude_routes.len(),
            route_count: self.routes.len(),
            route_all_traffic: self.route_all_traffic,
            strict_route: self.strict_route,
            mtu: self.mtu,
        }
    }

    pub fn describe(&self) -> String {
        let mut details = vec![
            format!("{:?} stack", self.stack),
            format!("adapter {}", self.adapter_name),
            format!("{} address(es)", self.addresses.len()),
            format!("{} DNS server(s)", self.dns_servers.len()),
            format!("{} DNS hijack rule(s)", self.dns_hijack.len()),
            format!("{} excluded route(s)", self.exclude_routes.len()),
            format!("{} route(s)", self.routes.len()),
        ];
        if let Some(interface_name) = &self.interface_name {
            details.push(format!("interface {}", interface_name));
        }
        if let Some(gateway) = self.gateway {
            details.push(format!("gateway {}", gateway));
        }
        if let Some(mtu) = self.mtu {
            details.push(format!("MTU {}", mtu));
        }
        details.push(format!(
            "route_all_traffic={}",
            if self.route_all_traffic { "true" } else { "false" }
        ));
        details.push(format!(
            "strict_route={}",
            if self.strict_route { "true" } else { "false" }
        ));
        details.join(", ")
    }

    pub fn interface_alias(&self) -> &str {
        self.adapter_name.as_str()
    }

    pub fn command_preview(&self) -> Vec<String> {
        let alias = self.interface_alias();
        let mut commands = Vec::new();

        if let Some(mtu) = self.mtu {
            commands.push(format!(
                "netsh interface ipv4 set subinterface name=\"{alias}\" mtu={mtu} store=active"
            ));
        }

        for address in &self.addresses {
            match address.address {
                IpAddr::V4(ip) => commands.push(format!(
                    "netsh interface ipv4 set address name=\"{alias}\" static {ip} {}",
                    ipv4_mask(address.prefix_len)
                )),
                IpAddr::V6(ip) => commands.push(format!(
                    "netsh interface ipv6 add address interface=\"{alias}\" address={ip}/{}",
                    address.prefix_len
                )),
            }
        }

        if let Some(primary_dns) = self.dns_servers.first() {
            commands.push(dns_command(alias, primary_dns, true));
            for dns in self.dns_servers.iter().skip(1) {
                commands.push(dns_command(alias, dns, false));
            }
        }

        commands.extend(self.route_commands());

        commands
    }

    pub fn rollback_preview(&self) -> Vec<String> {
        self.rollback_commands()
    }

    fn route_commands(&self) -> Vec<String> {
        let alias = self.interface_alias();
        let mut commands = Vec::new();

        if self.route_all_traffic
            && self
                .addresses
                .iter()
                .any(|address| matches!(address.address, IpAddr::V4(_)))
        {
            commands.push(format!(
                "netsh interface ipv4 add route prefix=0.0.0.0/0 interface=\"{alias}\" nexthop=0.0.0.0 metric=6 store=active"
            ));
        }

        for route in &self.routes {
            if let IpAddr::V4(ip) = route.destination.address {
                commands.push(format!(
                    "netsh interface ipv4 add route prefix={}/{} interface=\"{alias}\" nexthop=0.0.0.0 metric=6 store=active",
                    ip,
                    route.destination.prefix_len
                ));
            }
        }

        commands
    }

    fn rollback_commands(&self) -> Vec<String> {
        let alias = self.interface_alias();
        let mut commands = Vec::new();

        for route in self.routes.iter().rev() {
            if let IpAddr::V4(ip) = route.destination.address {
                commands.push(format!(
                    "netsh interface ipv4 delete route prefix={}/{} interface=\"{alias}\" store=active",
                    ip,
                    route.destination.prefix_len
                ));
            }
        }

        if self.route_all_traffic
            && self
                .addresses
                .iter()
                .any(|address| matches!(address.address, IpAddr::V4(_)))
        {
            commands.push(format!(
                "netsh interface ipv4 delete route prefix=0.0.0.0/0 interface=\"{alias}\" store=active"
            ));
        }

        if self
            .dns_servers
            .iter()
            .any(|dns| matches!(dns.address, IpAddr::V4(_)))
        {
            commands.push(format!(
                "netsh interface ipv4 set dnsservers name=\"{alias}\" source=dhcp"
            ));
        }

        if self
            .dns_servers
            .iter()
            .any(|dns| matches!(dns.address, IpAddr::V6(_)))
        {
            commands.push(format!(
                "netsh interface ipv6 set dnsservers interface=\"{alias}\" source=dhcp"
            ));
        }

        commands.push("ipconfig /flushdns".to_string());

        commands
    }
}

pub fn build_plan(config: &TunConfig) -> anyhow::Result<WindowsTunPlan> {
    config.validate()?;
    Ok(WindowsTunPlan::from_config(config))
}

pub fn open_or_create_adapter(plan: &WindowsTunPlan) -> WintunResult<crate::Adapter> {
    let library = WintunLibrary::load_default()?;
    match library.open_adapter(&plan.adapter_name) {
        Ok(adapter) => Ok(adapter),
        Err(_) => library.create_adapter(&plan.adapter_name, "Titan", None),
    }
}

pub fn activate(plan: &WindowsTunPlan) -> anyhow::Result<ActiveWindowsTun> {
    if !cfg!(windows) {
        anyhow::bail!("Windows TUN activation is only available on Windows");
    }

    let library = WintunLibrary::load_default()
        .map_err(|err| anyhow::anyhow!("failed to load Wintun: {err}"))?;
    let mut created_adapter = false;
    let adapter = match library.open_adapter(&plan.adapter_name) {
        Ok(adapter) => adapter,
        Err(_) => {
            created_adapter = true;
            library
                .create_adapter(&plan.adapter_name, "Titan", None)
                .map_err(|err| anyhow::anyhow!("failed to create Wintun adapter: {err}"))?
        }
    };
    let session = adapter
        .start_session(0x400000)
        .map_err(|err| anyhow::anyhow!("failed to start Wintun session: {err}"))?;
    let commands = plan.command_preview();
    let rollback_commands = plan.rollback_preview();
    apply_network_settings(&commands)?;

    Ok(ActiveWindowsTun {
        driver_version: library.running_driver_version(),
        _library: library,
        _adapter: adapter,
        _session: session,
        interface_name: plan.interface_alias().to_string(),
        created_adapter,
        commands,
        rollback_commands,
    })
}

impl ActiveWindowsTun {
    pub fn interface_name(&self) -> &str {
        &self.interface_name
    }

    pub fn created_adapter(&self) -> bool {
        self.created_adapter
    }

    pub fn driver_version(&self) -> Option<u32> {
        self.driver_version
    }

    pub fn commands(&self) -> &[String] {
        &self.commands
    }

    pub fn rollback_commands(&self) -> &[String] {
        &self.rollback_commands
    }

    pub fn receive_packet(&self) -> anyhow::Result<Option<Vec<u8>>> {
        match self._session.receive_packet() {
            Ok(Some(packet)) => Ok(Some(packet.as_slice().to_vec())),
            Ok(None) => Ok(None),
            Err(err) => Err(anyhow::anyhow!("failed to receive packet from Wintun: {err}")),
        }
    }

    pub fn send_packet(&self, packet: &[u8]) -> anyhow::Result<()> {
        let mut send_packet = self
            ._session
            .allocate_send_packet(packet.len())
            .map_err(|err| anyhow::anyhow!("failed to allocate Wintun send packet: {err}"))?;
        send_packet.as_mut_slice().copy_from_slice(packet);
        send_packet.commit();
        Ok(())
    }

    pub fn shutdown(self) -> anyhow::Result<()> {
        let mut commands = self.rollback_commands.clone();
        commands.sort_by_key(|command| rollback_priority(command));

        let mut failures = Vec::new();
        for command in &commands {
            if let Err(err) = run_netsh_command(command) {
                failures.push(err.to_string());
            }
        }

        if failures.is_empty() {
            Ok(())
        } else {
            anyhow::bail!("TUN rollback completed with errors: {}", failures.join(" | "))
        }
    }

    pub fn apply_runtime_commands(
        &mut self,
        commands: &[String],
        rollback_commands: &[String],
    ) -> anyhow::Result<()> {
        apply_network_settings(commands)?;
        self.commands.extend(commands.iter().cloned());
        for command in rollback_commands.iter().rev() {
            self.rollback_commands.insert(0, command.clone());
        }
        Ok(())
    }
}

fn apply_network_settings(commands: &[String]) -> anyhow::Result<()> {
    if !cfg!(windows) {
        anyhow::bail!("Windows network configuration is only available on Windows");
    }

    for command in commands {
        run_netsh_command(&command)?;
    }

    Ok(())
}

fn run_netsh_command(command: &str) -> anyhow::Result<()> {
    let output = Command::new("cmd")
        .args(["/C", command])
        .output()
        .map_err(|err| anyhow::anyhow!("failed to execute `{command}`: {err}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        anyhow::bail!("`{command}` failed: {}", detail)
    }
}

pub fn detect_default_gateway_ipv4() -> anyhow::Result<Ipv4Addr> {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "(Get-NetRoute -AddressFamily IPv4 -DestinationPrefix '0.0.0.0/0' | Sort-Object RouteMetric, InterfaceMetric | Select-Object -First 1 -ExpandProperty NextHop)",
        ])
        .output()
        .map_err(|err| anyhow::anyhow!("failed to query default gateway: {err}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        anyhow::bail!("failed to query default gateway: {}", detail);
    }

    let gateway = String::from_utf8_lossy(&output.stdout).trim().to_string();
    gateway
        .parse::<Ipv4Addr>()
        .map_err(|err| anyhow::anyhow!("invalid default gateway `{gateway}`: {err}"))
}

pub fn build_proxy_bypass_commands(
    gateway: Ipv4Addr,
    ips: &[Ipv4Addr],
) -> (Vec<String>, Vec<String>) {
    let mut apply = Vec::new();
    let mut rollback = Vec::new();

    for ip in ips {
        apply.push(format!(
            "route ADD {} MASK 255.255.255.255 {} METRIC 3",
            ip, gateway
        ));
        rollback.push(format!("route DELETE {}", ip));
    }

    (apply, rollback)
}

pub fn build_exclude_route_commands(
    gateway: Ipv4Addr,
    routes: &[TunAddress],
) -> (Vec<String>, Vec<String>) {
    let mut apply = Vec::new();
    let mut rollback = Vec::new();

    for route in routes {
        let IpAddr::V4(ip) = route.address else {
            continue;
        };
        let mask = ipv4_mask(route.prefix_len);
        apply.push(format!(
            "route ADD {} MASK {} {} METRIC 3",
            ip, mask, gateway
        ));
        rollback.push(format!(
            "route DELETE {} MASK {} {}",
            ip, mask, gateway
        ));
    }

    (apply, rollback)
}

fn dns_command(alias: &str, dns: &DnsServer, primary: bool) -> String {
    match dns.address {
        IpAddr::V4(ip) => {
            if primary {
                format!(
                    "netsh interface ipv4 set dnsservers name=\"{alias}\" static {ip} primary validate=no"
                )
            } else {
                format!(
                    "netsh interface ipv4 add dnsservers name=\"{alias}\" address={ip} validate=no"
                )
            }
        }
        IpAddr::V6(ip) => {
            if primary {
                format!(
                    "netsh interface ipv6 set dnsservers interface=\"{alias}\" static {ip} validate=no"
                )
            } else {
                format!(
                    "netsh interface ipv6 add dnsservers interface=\"{alias}\" address={ip} validate=no"
                )
            }
        }
    }
}

fn ipv4_mask(prefix_len: u8) -> String {
    let mask = if prefix_len == 0 {
        0
    } else {
        u32::MAX << (32 - prefix_len)
    };
    format!(
        "{}.{}.{}.{}",
        (mask >> 24) & 0xff,
        (mask >> 16) & 0xff,
        (mask >> 8) & 0xff,
        mask & 0xff
    )
}

fn rollback_priority(command: &str) -> u8 {
    if command.contains(" set dnsservers ") && command.contains(" source=dhcp") {
        0
    } else if command.eq_ignore_ascii_case("ipconfig /flushdns") {
        2
    } else {
        1
    }
}
