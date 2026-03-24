use std::path::PathBuf;
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::hash::{Hash, Hasher};
use std::time::Instant;

use serde_yaml::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::JoinHandle;

use crate::commands::path::{
    default_tun_config_path,
    default_tun_state_path,
    display_path,
    resolve_workspace_str,
};

pub fn tun_init(config_path: &str, force: bool) -> anyhow::Result<()> {
    let config_path = resolve_config_path(Some(config_path));
    if config_path.exists() && !force {
        anyhow::bail!(
            "TUN config already exists at {} (use --force to overwrite)",
            display_path(&config_path)
        );
    }

    let config = titan_tun::TunConfig::starter();
    config.save_to_path(&config_path)?;

    println!("\n========== Titan TUN ==========\n");
    println!("Starter config written to {}", display_path(&config_path));
    println!("Adapter: {}", config.adapter_name);
    println!("Stack: {:?}", config.stack);
    println!("Addresses: {}", config.addresses.len());
    println!("DNS servers: {}", config.dns_servers.len());
    println!("Route all traffic: {}", config.route_all_traffic);
    println!("Next step: titan tun up --preview");
    println!("Tip: you can also copy this into data/config.yaml under a top-level `tun:` section.");
    Ok(())
}

pub fn tun_status(state_path: &str, config_path: Option<&str>) -> anyhow::Result<()> {
    let state_path = resolve_state_path(Some(state_path));
    let manager = titan_tun::TunManager::new(&state_path);
    let status = manager.status();
    let wintun = titan_tun::probe_wintun();

    println!("\n========== Titan TUN ==========\n");
    println!("State file: {}", display_path(&state_path));
    println!("State: {}", status.describe());
    println!("Wintun DLL: {}", display_path(&wintun.dll_path));
    println!("Wintun present: {}", wintun.dll_exists);
    println!("Wintun loadable: {}", wintun.loadable);
    println!(
        "Wintun driver version: {}",
        status
            .driver_version
            .or(wintun.driver_version)
            .map(|version| version.to_string())
            .unwrap_or_else(|| "-".to_string())
    );
    println!(
        "Config path: {}",
        status
            .config_path
            .as_ref()
            .map(display_path)
            .unwrap_or_else(|| "-".to_string())
    );
    println!(
        "Adapter: {}",
        status
            .config
            .as_ref()
            .map(|config| config.adapter_name.as_str())
            .unwrap_or("-")
    );

    if let Some(config_path) = config_path {
        let config_path = resolve_config_path(Some(config_path));
        if let Ok((config, source)) = load_effective_tun_config(&config_path, Some(&default_proxy_config_path())) {
            println!("Preview config: {}", display_path(&config_path));
            println!("Preview source: {}", source);
            println!("Preview hint: {}", config.plan_hint());
        }
    }

    if let Some(config) = &status.config {
        println!("Interface: {}", config.interface_name.as_deref().unwrap_or("-"));
        println!("Stack: {:?}", config.stack);
        println!("Addresses: {}", config.addresses.len());
        println!("DNS servers: {}", config.dns_servers.len());
        println!("DNS hijack: {}", config.dns_hijack.join(", "));
        println!("Excluded routes: {}", config.exclude_routes.len());
        println!("Routes: {}", config.routes.len());
        println!("MTU: {}", config.mtu.map_or_else(|| "-".to_string(), |mtu| mtu.to_string()));
        println!("Route all traffic: {}", config.route_all_traffic);
        println!("Strict route: {}", config.strict_route);
        println!("Adapter ready: {}", status.adapter_ready);
    }

    if let Some(operation) = &status.last_operation {
        println!("\n--- Last Operation ---");
        println!("State: {:?}", operation.state);
        println!("Requested at: {}", operation.requested_at_unix);
        println!("Message: {}", operation.message);
        println!("Plan: {}", operation.config.adapter_name.as_str());
        println!("Applied: {}", operation.applied);
        if !operation.commands.is_empty() {
            println!("Commands:");
            for command in &operation.commands {
                println!("  {}", command);
            }
        }
    }

    if let Some(err) = &status.last_error {
        println!("\nLast error: {}", err);
    }
    if let Some(err) = &wintun.error {
        println!("Wintun error: {}", err);
    }

    Ok(())
}

pub fn tun_prepare(config_path: &str, state_path: &str) -> anyhow::Result<()> {
    let config_path = resolve_config_path(Some(config_path));
    let state_path = resolve_state_path(Some(state_path));
    let proxy_config_path = default_proxy_config_path();
    let (config, source) = load_effective_tun_config(&config_path, Some(&proxy_config_path))?;
    let manager = titan_tun::TunManager::new(&state_path);
    let plan = manager.plan(&config)?;

    println!("\n========== Titan TUN Preview ==========\n");
    println!("Config file: {}", display_path(&config_path));
    println!("Config source: {}", source);
    println!("State file: {}", display_path(&state_path));
    println!("Plan: {}", plan.describe());
    println!("Summary: {} address(es), {} DNS server(s), {} route(s)", plan.addresses.len(), plan.dns_servers.len(), plan.routes.len());
    let commands = plan.command_preview();
    if !commands.is_empty() {
        println!("Apply commands:");
        for command in &commands {
            println!("  {}", command);
        }
    }
    let rollback_commands = plan.rollback_preview();
    if !rollback_commands.is_empty() {
        println!("Rollback commands:");
        for command in &rollback_commands {
            println!("  {}", command);
        }
    }
    println!("Note: this only prepares a Windows TUN plan; it does not change routing.");
    Ok(())
}

pub fn tun_up(config_path: &str, state_path: &str) -> anyhow::Result<()> {
    let config_path = resolve_config_path(Some(config_path));
    let state_path = resolve_state_path(Some(state_path));
    let proxy_config_path = default_proxy_config_path();
    let (config, source) = load_effective_tun_config(&config_path, Some(&proxy_config_path))?;
    let manager = titan_tun::TunManager::new(&state_path);
    let status = manager.up(Some(config_path.clone()), config)?;

    println!("\n========== Titan TUN ==========\n");
    println!("Config file: {}", display_path(&config_path));
    println!("Config source: {}", source);
    println!("State file: {}", display_path(&state_path));
    println!("State: {}", status.describe());
    if let Some(operation) = status.last_operation {
        println!("Plan: {}", operation.message);
    }
    println!("Note: this is a safe configuration stage only; no route takeover was attempted.");
    Ok(())
}

pub async fn tun_hold(config_path: &str, state_path: &str) -> anyhow::Result<()> {
    let config_path = resolve_config_path(Some(config_path));
    let state_path = resolve_state_path(Some(state_path));
    let proxy_config_path = default_proxy_config_path();
    let (config, source) = load_effective_tun_config(&config_path, Some(&proxy_config_path))?;
    let manager = titan_tun::TunManager::new(&state_path);
    let _active = manager.activate(&config).map_err(|err| {
        anyhow::anyhow!(
            "{err}. Creating or configuring a live Wintun adapter usually requires an elevated terminal."
        )
    })?;
    let status = manager.up_live(Some(config_path.clone()), config.clone(), &_active)?;

    println!("\n========== Titan TUN ==========\n");
    println!("Config file: {}", display_path(&config_path));
    println!("Config source: {}", source);
    println!("State file: {}", display_path(&state_path));
    println!("State: {}", status.describe());
    println!("Adapter: {}", config.adapter_name);
    println!("Interface alias: {}", _active.interface_name());
    println!(
        "Driver version: {}",
        _active
            .driver_version()
            .map(|version| version.to_string())
            .unwrap_or_else(|| "-".to_string())
    );
    println!("Created adapter: {}", _active.created_adapter());
    println!("Network settings were applied to the live Wintun adapter.");
    println!("This process is holding the adapter/session open.");
    println!("Press Ctrl+C to release the adapter.");

    tokio::signal::ctrl_c().await?;

    let shutdown_result = _active.shutdown();
    let status = manager.down()?;
    println!("\nTUN released.");
    println!("State: {}", status.describe());
    shutdown_result?;
    Ok(())
}

pub async fn tun_inspect(
    config_path: &str,
    proxy_config_path: &str,
    state_path: &str,
    limit: usize,
) -> anyhow::Result<()> {
    tun_live(config_path, proxy_config_path, state_path, Some(limit), true).await
}

pub async fn tun_run(
    config_path: &str,
    proxy_config_path: &str,
    state_path: &str,
) -> anyhow::Result<()> {
    tun_live(config_path, proxy_config_path, state_path, None, false).await
}

async fn tun_live(
    config_path: &str,
    proxy_config_path: &str,
    state_path: &str,
    limit: Option<usize>,
    verbose: bool,
) -> anyhow::Result<()> {
    let config_path = resolve_config_path(Some(config_path));
    let proxy_config_path = resolve_workspace_str(proxy_config_path);
    let state_path = resolve_state_path(Some(state_path));
    ensure_config_exists(&proxy_config_path)?;

    let (config, source) = load_effective_tun_config(&config_path, Some(&proxy_config_path))?;
    let proxy_config = titan_config::load_config(&proxy_config_path)?;
    let engine = Arc::new(
        titan_core::ProxyEngine::new_with_path(
            proxy_config.clone(),
            Some(display_path(&proxy_config_path)),
        )
        .await?,
    );
    engine.start().await?;

    let manager = titan_tun::TunManager::new(&state_path);
    let mut active = manager.activate(&config).map_err(|err| {
        anyhow::anyhow!(
            "{err}. Creating or configuring a live Wintun adapter usually requires an elevated terminal."
        )
    })?;
    apply_runtime_bypass_routes(&mut active, &config, &proxy_config, verbose).await?;
    let (mut system_proxy, inbound_task) =
        maybe_enable_mixed_fallback(&config, engine.clone()).await?;
    let status = manager.up_live(Some(config_path.clone()), config.clone(), &active)?;

    println!("\n========== Titan TUN Inspect ==========\n");
    println!("TUN config: {}", display_path(&config_path));
    println!("TUN source: {}", source);
    println!("Proxy config: {}", display_path(&proxy_config_path));
    println!("State file: {}", display_path(&state_path));
    println!("State: {}", status.describe());
    println!("Adapter: {}", config.adapter_name);
    println!("Interface alias: {}", active.interface_name());
    println!("Stack: {:?}", config.stack);
    if let Some(limit) = limit {
        println!("Limit: {}", limit.clamp(1, 10_000));
    } else {
        println!("Mode: run");
    }
    if matches!(
        config.stack,
        titan_tun::TunStack::Mixed | titan_tun::TunStack::System
    ) {
        println!(
            "Proxy fallback: enabled -> {}:{}",
            config.proxy_fallback_bind, config.proxy_fallback_port
        );
    }
    println!("Press Ctrl+C to stop TUN processing.\n");

    let mut seen_flows = std::collections::HashSet::new();
    let mut active_tcp = std::collections::HashMap::new();
    let mut active_udp = std::collections::HashMap::new();
    let mut dns_cache = std::collections::HashMap::new();
    let limit = limit.map(|value| value.clamp(1, 10_000));

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                break;
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(20)) => {
                while let Some(bytes) = active.receive_packet()? {
                    let packet = match titan_tun::parse_ip_packet(&bytes) {
                        Ok(packet) => packet,
                        Err(_) => continue,
                    };

                    let flow_key = match (packet.src_port, packet.dst_port) {
                        (Some(src), Some(dst)) => format!(
                            "{} {}:{} -> {}:{}",
                            packet.protocol.label(),
                            packet.src_ip,
                            src,
                            packet.dst_ip,
                            dst
                        ),
                        _ => format!(
                            "{} {} -> {}",
                            packet.protocol.label(),
                            packet.src_ip,
                            packet.dst_ip
                        ),
                    };
                    let is_new_flow = seen_flows.insert(flow_key.clone());
                    if seen_flows.len() > 4096 {
                        seen_flows.clear();
                    }

                    let target_host =
                        resolve_cached_host(&mut dns_cache, packet.dst_ip).unwrap_or_else(|| {
                            packet.dst_ip.to_string()
                        });
                    let target =
                        titan_core::Target::new(target_host, packet.dst_port.unwrap_or(0));
                    let route = engine
                        .select_route(&target)
                        .await
                        .unwrap_or_else(|| titan_core::engine::RouteDecision {
                            policy: "DIRECT".to_string(),
                            rule: "MATCH,DIRECT".to_string(),
                            resolved_ip: None,
                        });

                    let connect_note = if matches!(packet.protocol, titan_tun::TransportProtocol::Tcp)
                        && target.port != 0
                    {
                        let source = match packet.src_port {
                            Some(port) => format!("{}:{}", packet.src_ip, port),
                            None => packet.src_ip.to_string(),
                        };
                        let note = update_tcp_bridge(
                            &mut active_tcp,
                            &active,
                            engine.clone(),
                            &packet,
                            &flow_key,
                            &source,
                            &route.policy,
                            &route.rule,
                            &target,
                            packet.tcp_syn,
                            packet.tcp_fin || packet.tcp_rst,
                        ).await;
                        format!(" connect={note}")
                    } else if matches!(packet.protocol, titan_tun::TransportProtocol::Udp)
                    {
                        let note = handle_udp_packet(
                            engine.clone(),
                            &mut active_udp,
                            &active,
                            &config,
                            &packet,
                            &bytes,
                            &mut dns_cache,
                            &flow_key,
                            &route.policy,
                        )
                        .await;
                        format!(" connect={note}")
                    } else {
                        String::new()
                    };

                    if is_new_flow {
                        let rule_note = if verbose || route.policy == "DIRECT" {
                            format!(" rule={}", route.rule)
                        } else {
                            String::new()
                        };
                        if verbose {
                            println!(
                                "[{}] {} -> policy {}{}{}",
                                packet.protocol.label(),
                                flow_key,
                                route.policy,
                                rule_note,
                                connect_note
                            );
                        } else {
                            println!(
                                "[{}] {} -> {}{}{}",
                                packet.protocol.label(),
                                flow_key,
                                route.policy,
                                rule_note,
                                connect_note
                            );
                        }
                    }

                    if limit.is_some_and(|max| seen_flows.len() >= max) {
                        break;
                    }
                }
            }
        }

        if limit.is_some_and(|max| seen_flows.len() >= max) {
            break;
        }

        pump_tcp_bridge(&mut active_tcp, &active, engine.clone()).await;
        pump_udp_bridge(&mut active_udp, &active).await;
    }

    for (_, held) in active_tcp.drain() {
        engine.session_manager().close(&held.session_id);
    }
    if let Some(task) = inbound_task {
        task.abort();
    }
    if let Some(proxy) = system_proxy.as_mut() {
        proxy.restore()?;
    }
    let shutdown_result = active.shutdown();
    let status = manager.down()?;
    engine.stop().await?;
    println!("\nTUN processing stopped.");
    println!("State: {}", status.describe());
    shutdown_result?;
    Ok(())
}

struct HeldTcpConnection {
    stream: titan_protocols::BoxedStream,
    session_id: String,
    outbound_name: String,
    downloaded: u64,
    syn_seq: u32,
    server_seq: u32,
    client_next_seq: u32,
    uploaded: u64,
    client_ip: std::net::IpAddr,
    server_ip: std::net::IpAddr,
    client_port: u16,
    server_port: u16,
    state: TcpState,
    last_active: Instant,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TcpState {
    SynAckSent,
    Established,
    FinSent,
}

struct HeldUdpConnection {
    socket: titan_protocols::BoxedDatagram,
    outbound_name: String,
    client_ip: std::net::IpAddr,
    server_ip: std::net::IpAddr,
    client_port: u16,
    server_port: u16,
    last_active: Instant,
}

struct DnsCacheEntry {
    host: String,
    expires_at: Instant,
}

async fn update_tcp_bridge(
    active_tcp: &mut std::collections::HashMap<String, HeldTcpConnection>,
    active: &titan_tun::ActiveWindowsTun,
    engine: Arc<titan_core::ProxyEngine>,
    packet: &titan_tun::IpPacketSummary,
    flow_key: &str,
    source: &str,
    policy: &str,
    matched_rule: &str,
    target: &titan_core::Target,
    is_syn: bool,
    is_fin_or_rst: bool,
) -> String {
    if is_fin_or_rst {
        if let Some(held) = active_tcp.remove(flow_key) {
            engine.session_manager().close(&held.session_id);
            return if packet.tcp_rst || held.state == TcpState::FinSent {
                format!("closed {}", held.outbound_name)
            } else {
                let fin_note = match build_tcp_fin_packet_for_held(&held) {
                    Some(reply) => match active.send_packet(&reply) {
                        Ok(()) => " fin=sent".to_string(),
                        Err(send_err) => format!(" fin=fail:{send_err}"),
                    },
                    None => String::new(),
                };
                format!("closed {}{}", held.outbound_name, fin_note)
            };
        }
        return "close-ignored".to_string();
    }

    if let Some(held) = active_tcp.get_mut(flow_key) {
        held.last_active = Instant::now();

        if held.state == TcpState::SynAckSent && packet.tcp_syn && !packet.tcp_ack {
            let note = match build_tcp_syn_ack_for_summary(packet, held.syn_seq) {
                Some(reply) => match active.send_packet(&reply) {
                    Ok(()) => " syn-ack=resent".to_string(),
                    Err(send_err) => format!(" syn-ack-resend-fail:{send_err}"),
                },
                None => String::new(),
            };
            return format!("held via {}{}", held.outbound_name, note);
        }

        if held.state == TcpState::SynAckSent && packet.tcp_ack {
            held.state = TcpState::Established;
        }

        if held.state == TcpState::FinSent && packet.tcp_ack && packet.payload.is_empty() {
            let session_id = held.session_id.clone();
            let outbound_name = held.outbound_name.clone();
            active_tcp.remove(flow_key);
            engine.session_manager().close(&session_id);
            return format!("closed {}", outbound_name);
        }

        if !packet.payload.is_empty() {
            match held.stream.write_all(&packet.payload).await {
                Ok(()) => {
                    held.uploaded += packet.payload.len() as u64;
                    held.client_next_seq = packet
                        .tcp_seq
                        .unwrap_or(held.client_next_seq)
                        .wrapping_add(packet.payload.len() as u32)
                        .wrapping_add(u32::from(packet.tcp_fin))
                        .wrapping_add(u32::from(packet.tcp_syn));
                    engine
                        .session_manager()
                        .update_traffic(&held.session_id, packet.payload.len() as u64, 0);
                    let ack_note = match build_tcp_ack_packet_for_held(held) {
                        Some(reply) => match active.send_packet(&reply) {
                            Ok(()) => " ack=sent".to_string(),
                            Err(send_err) => format!(" ack=fail:{send_err}"),
                        },
                        None => String::new(),
                    };
                    return format!(
                        "held via {} up={} down={} seq={}{}",
                        held.outbound_name, held.uploaded, held.downloaded, held.server_seq, ack_note
                    );
                }
                Err(err) => {
                    let session_id = held.session_id.clone();
                    let outbound_name = held.outbound_name.clone();
                    engine
                        .mark_outbound_unhealthy_with_reason(
                            &outbound_name,
                            policy,
                            target,
                            std::time::Duration::from_secs(60),
                            err.to_string(),
                        )
                        .await;
                    active_tcp.remove(flow_key);
                    engine.session_manager().close(&session_id);
                    return format!("write-fail via {}: {}", outbound_name, err);
                }
            }
        }

        if let Some(seq) = packet.tcp_seq {
            held.client_next_seq = seq
                .wrapping_add(packet.payload.len() as u32)
                .wrapping_add(u32::from(packet.tcp_fin))
                .wrapping_add(u32::from(packet.tcp_syn));
        }

        return format!(
            "held via {} up={} down={} seq={}",
            held.outbound_name, held.uploaded, held.downloaded, held.server_seq
        );
    }

    if !is_syn {
        return "await-syn".to_string();
    }

        match engine.connect_outbound_with_failover(policy, target, 4).await {
        Ok((resolved_name, stream)) => {
            let session = engine
                .session_manager()
                .create(
                    source.to_string(),
                    target.to_string(),
                    resolved_name.clone(),
                    Some(policy.to_string()),
                    Some(matched_rule.to_string()),
                );
            let server_seq = derive_server_seq(flow_key);
            let syn_ack_note = match build_tcp_syn_ack_for_summary(packet, server_seq) {
                Some(reply) => match active.send_packet(&reply) {
                    Ok(()) => " syn-ack=sent".to_string(),
                    Err(send_err) => format!(" syn-ack=fail:{send_err}"),
                },
                None => String::new(),
            };
            active_tcp.insert(
                flow_key.to_string(),
                HeldTcpConnection {
                    stream,
                    session_id: session.id.clone(),
                    outbound_name: resolved_name.clone(),
                    downloaded: 0,
                    syn_seq: server_seq,
                    server_seq: server_seq.wrapping_add(1),
                    client_next_seq: packet.tcp_seq.unwrap_or(0).wrapping_add(1),
                    uploaded: 0,
                    client_ip: packet.src_ip,
                    server_ip: packet.dst_ip,
                    client_port: packet.src_port.unwrap_or(0),
                    server_port: packet.dst_port.unwrap_or(0),
                    state: TcpState::SynAckSent,
                    last_active: Instant::now(),
                },
            );
            format!("opened via {resolved_name}{syn_ack_note}")
        }
        Err(err) => {
            let reset_note = match build_tcp_reset_for_summary(packet) {
                Some(reply) => match active.send_packet(&reply) {
                    Ok(()) => " rst=sent".to_string(),
                    Err(send_err) => format!(" rst=fail:{send_err}"),
                },
                None => String::new(),
            };
            format!("fail: {err}{reset_note}")
        }
    }
}

async fn pump_tcp_bridge(
    active_tcp: &mut std::collections::HashMap<String, HeldTcpConnection>,
    active: &titan_tun::ActiveWindowsTun,
    engine: Arc<titan_core::ProxyEngine>,
) {
    let keys: Vec<String> = active_tcp.keys().cloned().collect();
    let mut to_remove = Vec::new();

    for key in keys {
        let Some(held) = active_tcp.get_mut(&key) else {
            continue;
        };

        let idle_timeout = match held.state {
            TcpState::SynAckSent => std::time::Duration::from_secs(10),
            TcpState::Established => std::time::Duration::from_secs(120),
            TcpState::FinSent => std::time::Duration::from_secs(5),
        };
        if held.last_active.elapsed() > idle_timeout {
            to_remove.push((key, held.session_id.clone()));
            continue;
        }

        let mut buffer = [0_u8; 4096];
        match tokio::time::timeout(
            std::time::Duration::from_millis(1),
            held.stream.read(&mut buffer),
        )
        .await
        {
            Ok(Ok(0)) => {
                if let Some(packet) = build_tcp_fin_packet_for_held(held) {
                    let _ = active.send_packet(&packet);
                    held.server_seq = held.server_seq.wrapping_add(1);
                }
                held.state = TcpState::FinSent;
                held.last_active = Instant::now();
            }
            Ok(Ok(read)) => {
                let data = &buffer[..read];
                if let Some(packet) = build_tcp_data_packet_for_held(held, data) {
                    let _ = active.send_packet(&packet);
                }
                held.server_seq = held.server_seq.wrapping_add(read as u32);
                held.downloaded += read as u64;
                 held.last_active = Instant::now();
                if held.state == TcpState::SynAckSent {
                    held.state = TcpState::Established;
                }
                engine
                    .session_manager()
                    .update_traffic(&held.session_id, 0, read as u64);
            }
            Ok(Err(_)) => {
                to_remove.push((key, held.session_id.clone()));
            }
            Err(_) => {}
        }
    }

    for (key, session_id) in to_remove {
        active_tcp.remove(&key);
        engine.session_manager().close(&session_id);
    }
}

async fn update_udp_bridge(
    engine: Arc<titan_core::ProxyEngine>,
    active_udp: &mut std::collections::HashMap<String, HeldUdpConnection>,
    packet: &titan_tun::IpPacketSummary,
    flow_key: &str,
    policy: &str,
) -> String {
    let Some(dst_port) = packet.dst_port else {
        return "udp-invalid".to_string();
    };

    if let Some(held) = active_udp.get_mut(flow_key) {
        held.last_active = Instant::now();
        return match held.socket.send(&packet.payload).await {
            Ok(sent) => format!("udp-{}:{}b", held.outbound_name, sent),
            Err(err) => format!("udp-send-fail:{err}"),
        };
    }

    let target = titan_core::Target::new(packet.dst_ip.to_string(), dst_port);
    let (outbound_name, mut socket) = match engine.connect_outbound_udp_with_failover(policy, &target, 4).await {
        Ok(result) => result,
        Err(err) => return format!("udp-connect-fail:{err}"),
    };

    let sent = match socket.send(&packet.payload).await {
        Ok(sent) => sent,
        Err(err) => return format!("udp-send-fail:{err}"),
    };

    active_udp.insert(
        flow_key.to_string(),
        HeldUdpConnection {
            socket,
            outbound_name: outbound_name.clone(),
            client_ip: packet.src_ip,
            server_ip: packet.dst_ip,
            client_port: packet.src_port.unwrap_or(0),
            server_port: dst_port,
            last_active: Instant::now(),
        },
    );

    format!("udp-{}:{}b", outbound_name, sent)
}

async fn handle_udp_packet(
    engine: Arc<titan_core::ProxyEngine>,
    active_udp: &mut std::collections::HashMap<String, HeldUdpConnection>,
    active: &titan_tun::ActiveWindowsTun,
    config: &titan_tun::TunConfig,
    packet: &titan_tun::IpPacketSummary,
    raw_packet: &[u8],
    dns_cache: &mut std::collections::HashMap<std::net::IpAddr, DnsCacheEntry>,
    flow_key: &str,
    policy: &str,
) -> String {
    if packet.payload.is_empty() {
        return "udp-empty".to_string();
    }

    if !matches!(packet.version, 4 | 6) {
        return "udp-ip-version-unhandled".to_string();
    }

    if should_hijack_dns(config, packet) {
        return handle_dns_udp(active, packet, dns_cache).await;
    }

    if is_local_multicast(packet) {
        return "udp-local-multicast".to_string();
    }

    if policy.eq_ignore_ascii_case("DIRECT") {
        return update_udp_bridge(engine, active_udp, packet, flow_key, policy).await;
    }

    let note = update_udp_bridge(engine, active_udp, packet, flow_key, policy).await;
    if note.starts_with("udp-connect-fail:no UDP-capable outbound remained")
        || note.starts_with("udp-connect-fail:UDP is not supported")
    {
        let icmp_note = match build_icmp_port_unreachable(raw_packet, packet.version) {
            Some(reply) => match active.send_packet(&reply) {
                Ok(()) => " icmp=sent".to_string(),
                Err(err) => format!(" icmp=fail:{err}"),
            },
            None => String::new(),
        };
        return format!("{note}{icmp_note}");
    }

    note
}

async fn pump_udp_bridge(
    active_udp: &mut std::collections::HashMap<String, HeldUdpConnection>,
    active: &titan_tun::ActiveWindowsTun,
) {
    let keys: Vec<String> = active_udp.keys().cloned().collect();
    let mut to_remove = Vec::new();

    for key in keys {
        let Some(held) = active_udp.get_mut(&key) else {
            continue;
        };

        if held.last_active.elapsed() > std::time::Duration::from_secs(30) {
            to_remove.push(key);
            continue;
        }

        let mut buffer = [0_u8; 2048];
        match tokio::time::timeout(
            std::time::Duration::from_millis(1),
            held.socket.recv(&mut buffer),
        )
        .await
        {
            Ok(Ok(read)) if read > 0 => {
                if let Some(packet) = build_udp_packet_for_held(held, &buffer[..read]) {
                    let _ = active.send_packet(&packet);
                }
                held.last_active = Instant::now();
            }
            Ok(Ok(_)) => {}
            Ok(Err(_)) => {
                to_remove.push(key);
            }
            Err(_) => {}
        }
    }

    for key in to_remove {
        active_udp.remove(&key);
    }
}

fn build_tcp_data_packet_for_held(
    held: &HeldTcpConnection,
    data: &[u8],
) -> Option<Vec<u8>> {
    let summary = held_summary(held);
    build_tcp_data_for_summary(&summary, held.server_seq, held.client_next_seq, data)
}

fn build_tcp_ack_packet_for_held(held: &HeldTcpConnection) -> Option<Vec<u8>> {
    let summary = held_summary(held);
    build_tcp_ack_for_summary(&summary, held.server_seq, held.client_next_seq)
}

fn build_tcp_fin_packet_for_held(held: &HeldTcpConnection) -> Option<Vec<u8>> {
    let summary = held_summary(held);
    build_tcp_fin_for_summary(&summary, held.server_seq, held.client_next_seq)
}

fn build_udp_packet_for_held(
    held: &HeldUdpConnection,
    data: &[u8],
) -> Option<Vec<u8>> {
    let summary = held_udp_summary(held);
    build_udp_response_for_summary(&summary, data)
}

fn build_tcp_syn_ack_for_summary(
    summary: &titan_tun::IpPacketSummary,
    server_seq: u32,
) -> Option<Vec<u8>> {
    match summary.version {
        4 => titan_tun::build_ipv4_tcp_syn_ack(summary, server_seq),
        6 => titan_tun::build_ipv6_tcp_syn_ack(summary, server_seq),
        _ => None,
    }
}

fn build_tcp_reset_for_summary(summary: &titan_tun::IpPacketSummary) -> Option<Vec<u8>> {
    match summary.version {
        4 => titan_tun::build_ipv4_tcp_reset(summary),
        6 => titan_tun::build_ipv6_tcp_reset(summary),
        _ => None,
    }
}

fn build_tcp_data_for_summary(
    summary: &titan_tun::IpPacketSummary,
    server_seq: u32,
    client_ack: u32,
    payload: &[u8],
) -> Option<Vec<u8>> {
    match summary.version {
        4 => titan_tun::build_ipv4_tcp_data(summary, server_seq, client_ack, payload),
        6 => titan_tun::build_ipv6_tcp_data(summary, server_seq, client_ack, payload),
        _ => None,
    }
}

fn build_tcp_ack_for_summary(
    summary: &titan_tun::IpPacketSummary,
    server_seq: u32,
    client_ack: u32,
) -> Option<Vec<u8>> {
    match summary.version {
        4 => titan_tun::build_ipv4_tcp_ack(summary, server_seq, client_ack),
        6 => titan_tun::build_ipv6_tcp_ack(summary, server_seq, client_ack),
        _ => None,
    }
}

fn build_tcp_fin_for_summary(
    summary: &titan_tun::IpPacketSummary,
    server_seq: u32,
    client_ack: u32,
) -> Option<Vec<u8>> {
    match summary.version {
        4 => titan_tun::build_ipv4_tcp_fin_ack(summary, server_seq, client_ack),
        6 => titan_tun::build_ipv6_tcp_fin_ack(summary, server_seq, client_ack),
        _ => None,
    }
}

fn build_udp_response_for_summary(
    summary: &titan_tun::IpPacketSummary,
    payload: &[u8],
) -> Option<Vec<u8>> {
    match summary.version {
        4 => titan_tun::build_ipv4_udp_response(summary, payload),
        6 => titan_tun::build_ipv6_udp_response(summary, payload),
        _ => None,
    }
}

fn build_icmp_port_unreachable(
    original_packet: &[u8],
    version: u8,
) -> Option<Vec<u8>> {
    match version {
        4 => titan_tun::build_ipv4_icmp_port_unreachable(original_packet),
        6 => titan_tun::build_ipv6_icmp_port_unreachable(original_packet),
        _ => None,
    }
}

fn held_summary(held: &HeldTcpConnection) -> titan_tun::IpPacketSummary {
    let version = match held.client_ip {
        std::net::IpAddr::V4(_) => 4,
        std::net::IpAddr::V6(_) => 6,
    };
    titan_tun::IpPacketSummary {
        version,
        protocol: titan_tun::TransportProtocol::Tcp,
        src_ip: held.client_ip,
        dst_ip: held.server_ip,
        src_port: Some(held.client_port),
        dst_port: Some(held.server_port),
        payload_len: 0,
        payload: Vec::new(),
        tcp_syn: false,
        tcp_fin: false,
        tcp_rst: false,
        tcp_ack: true,
        tcp_seq: Some(held.client_next_seq),
        tcp_ack_seq: Some(held.server_seq),
    }
}

fn held_udp_summary(held: &HeldUdpConnection) -> titan_tun::IpPacketSummary {
    let version = match held.client_ip {
        std::net::IpAddr::V4(_) => 4,
        std::net::IpAddr::V6(_) => 6,
    };
    titan_tun::IpPacketSummary {
        version,
        protocol: titan_tun::TransportProtocol::Udp,
        src_ip: held.client_ip,
        dst_ip: held.server_ip,
        src_port: Some(held.client_port),
        dst_port: Some(held.server_port),
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

fn derive_server_seq(flow_key: &str) -> u32 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    flow_key.hash(&mut hasher);
    (hasher.finish() & 0xffff_ffff) as u32
}

fn is_local_multicast(packet: &titan_tun::IpPacketSummary) -> bool {
    match packet.dst_ip {
        std::net::IpAddr::V4(ip) => {
            ip.is_multicast() || ip == std::net::Ipv4Addr::BROADCAST
        }
        std::net::IpAddr::V6(ip) => ip.is_multicast(),
    }
}

pub fn tun_down(state_path: &str) -> anyhow::Result<()> {
    let state_path = resolve_state_path(Some(state_path));
    let manager = titan_tun::TunManager::new(&state_path);
    let status = manager.down()?;

    println!("\n========== Titan TUN ==========\n");
    println!("State file: {}", display_path(&state_path));
    println!("State: {}", status.describe());
    if let Some(operation) = status.last_operation {
        println!("Message: {}", operation.message);
    }
    Ok(())
}

fn resolve_config_path(path: Option<&str>) -> PathBuf {
    path.map(resolve_workspace_str)
        .unwrap_or_else(default_tun_config_path)
}

fn resolve_state_path(path: Option<&str>) -> PathBuf {
    path.map(resolve_workspace_str)
        .unwrap_or_else(default_tun_state_path)
}

fn default_proxy_config_path() -> PathBuf {
    resolve_workspace_str(crate::commands::DEFAULT_CONFIG_PATH)
}

fn ensure_config_exists(path: &PathBuf) -> anyhow::Result<()> {
    if path.exists() {
        Ok(())
    } else {
        anyhow::bail!("TUN config file not found: {}", display_path(path));
    }
}

fn load_effective_tun_config(
    tun_config_path: &PathBuf,
    proxy_config_path: Option<&PathBuf>,
) -> anyhow::Result<(titan_tun::TunConfig, String)> {
    if tun_config_path.exists() {
        return Ok((
            titan_tun::TunConfig::load_from_path(tun_config_path)?,
            format!("file {}", display_path(tun_config_path)),
        ));
    }

    if let Some(proxy_config_path) = proxy_config_path {
        if let Some(config) = extract_tun_config_from_proxy(proxy_config_path)? {
            return Ok((
                config,
                format!("main config {}", display_path(proxy_config_path)),
            ));
        }
    }

    anyhow::bail!(
        "TUN config not found in {} and no `tun` section was found in {}",
        display_path(tun_config_path),
        proxy_config_path
            .map(display_path)
            .unwrap_or_else(|| "-".to_string())
    )
}

fn extract_tun_config_from_proxy(proxy_config_path: &PathBuf) -> anyhow::Result<Option<titan_tun::TunConfig>> {
    let content = std::fs::read_to_string(proxy_config_path)?;
    let value: Value = serde_yaml::from_str(&content)?;
    let Some(tun_value) = value.get("tun") else {
        return Ok(None);
    };
    Ok(Some(serde_yaml::from_value(tun_value.clone())?))
}

async fn apply_runtime_bypass_routes(
    active: &mut titan_tun::ActiveWindowsTun,
    tun_config: &titan_tun::TunConfig,
    proxy_config: &titan_config::Config,
    verbose: bool,
) -> anyhow::Result<()> {
    let gateway = titan_tun::detect_default_gateway_ipv4()?;
    let mut bypass_ips = resolve_proxy_server_ips(proxy_config).await;
    bypass_ips.extend(
        tun_config
            .dns_servers
            .iter()
            .filter_map(|dns| match dns.address {
                std::net::IpAddr::V4(ip) => Some(ip),
                std::net::IpAddr::V6(_) => None,
            }),
    );
    bypass_ips.sort_unstable();
    bypass_ips.dedup();

    if bypass_ips.is_empty() && tun_config.exclude_routes.is_empty() {
        return Ok(());
    }

    if !bypass_ips.is_empty() {
        let (apply, rollback) = titan_tun::build_proxy_bypass_commands(gateway, &bypass_ips);
        active.apply_runtime_commands(&apply, &rollback)?;
    }

    if !tun_config.exclude_routes.is_empty() {
        let (apply, rollback) =
            titan_tun::build_exclude_route_commands(gateway, &tun_config.exclude_routes);
        active.apply_runtime_commands(&apply, &rollback)?;
    }

    if verbose {
        println!(
            "Applied {} bypass route(s) via gateway {}",
            bypass_ips.len(),
            gateway
        );
        for ip in bypass_ips.iter().take(8) {
            println!("  bypass {}", ip);
        }
        if bypass_ips.len() > 8 {
            println!("  ... and {} more", bypass_ips.len() - 8);
        }
        if !tun_config.exclude_routes.is_empty() {
            println!(
                "Applied {} excluded route(s) via gateway {}",
                tun_config.exclude_routes.len(),
                gateway
            );
        }
    }

    Ok(())
}

async fn maybe_enable_mixed_fallback(
    config: &titan_tun::TunConfig,
    engine: Arc<titan_core::ProxyEngine>,
) -> anyhow::Result<(Option<titan_system_proxy::SystemProxy>, Option<JoinHandle<()>>)> {
    if !matches!(
        config.stack,
        titan_tun::TunStack::Mixed | titan_tun::TunStack::System
    ) {
        return Ok((None, None));
    }

    let bind_addr: std::net::SocketAddr =
        format!("{}:{}", config.proxy_fallback_bind, config.proxy_fallback_port).parse()?;
    let probe_listener = std::net::TcpListener::bind(bind_addr).map_err(|err| {
        anyhow::anyhow!(
            "failed to bind mixed proxy fallback on {}: {}",
            bind_addr,
            err
        )
    })?;
    drop(probe_listener);

    let inbound = titan_core::inbound::InboundServer::new(bind_addr, engine);
    let task = tokio::spawn(async move {
        if let Err(err) = inbound.start().await {
            tracing::warn!("mixed proxy fallback inbound stopped: {}", err);
        }
    });

    let mut system_proxy = titan_system_proxy::SystemProxy::new();
    system_proxy.set(&titan_system_proxy::ProxyConfig::new(
        &config.proxy_fallback_bind,
        config.proxy_fallback_port,
    ))?;

    Ok((Some(system_proxy), Some(task)))
}

async fn resolve_proxy_server_ips(config: &titan_config::Config) -> Vec<Ipv4Addr> {
    let mut ips = std::collections::BTreeSet::new();

    for proxy in &config.proxies {
        if let Ok(ip) = proxy.server.parse::<Ipv4Addr>() {
            ips.insert(ip);
            continue;
        }

        let lookup_target = format!("{}:{}", proxy.server, proxy.port.max(1));
        if let Ok(addrs) = tokio::net::lookup_host(lookup_target.as_str()).await {
            for addr in addrs {
                if let std::net::IpAddr::V4(ip) = addr.ip() {
                    ips.insert(ip);
                }
            }
        };
    }

    ips.into_iter().collect()
}

async fn handle_dns_udp(
    active: &titan_tun::ActiveWindowsTun,
    packet: &titan_tun::IpPacketSummary,
    dns_cache: &mut std::collections::HashMap<std::net::IpAddr, DnsCacheEntry>,
) -> String {
    let Some(dst_port) = packet.dst_port else {
        return "dns-invalid".to_string();
    };
    let dns_server = std::net::SocketAddr::new(packet.dst_ip, dst_port);
    let bind_addr = match packet.dst_ip {
        std::net::IpAddr::V4(_) => "0.0.0.0:0",
        std::net::IpAddr::V6(_) => "[::]:0",
    };
    let socket = match tokio::net::UdpSocket::bind(bind_addr).await {
        Ok(socket) => socket,
        Err(err) => return format!("dns-bind-fail:{err}"),
    };

    if let Err(err) = socket.send_to(&packet.payload, dns_server).await {
        return format!("dns-send-fail:{err}");
    }

    let mut buf = [0_u8; 2048];
    let response = match tokio::time::timeout(
        std::time::Duration::from_secs(3),
        socket.recv_from(&mut buf),
    )
    .await
    {
        Ok(Ok((read, _))) => &buf[..read],
        Ok(Err(err)) => return format!("dns-recv-fail:{err}"),
        Err(_) => return "dns-timeout".to_string(),
    };

    let reply = match build_udp_response_for_summary(packet, response) {
        Some(reply) => reply,
        None => return "dns-reply-build-fail".to_string(),
    };

    if let Some((host, addresses, ttl)) = extract_dns_mapping(response) {
        let expires_at = Instant::now()
            + std::time::Duration::from_secs(u64::from(ttl.max(30)));
        for address in addresses {
            dns_cache.insert(
                address,
                DnsCacheEntry {
                    host: host.clone(),
                    expires_at,
                },
            );
        }
    }

    match active.send_packet(&reply) {
        Ok(()) => format!("dns-ok:{}b", response.len()),
        Err(err) => format!("dns-reply-send-fail:{err}"),
    }
}

fn should_hijack_dns(
    config: &titan_tun::TunConfig,
    packet: &titan_tun::IpPacketSummary,
) -> bool {
    let Some(dst_port) = packet.dst_port else {
        return false;
    };
    if dst_port != 53 {
        return false;
    }

    config.dns_hijack.iter().any(|rule| {
        let normalized = rule.trim().to_ascii_lowercase();
        if normalized == "any:53" || normalized == "53" {
            return true;
        }
        normalized == format!("{}:53", packet.dst_ip)
    })
}

fn resolve_cached_host(
    dns_cache: &mut std::collections::HashMap<std::net::IpAddr, DnsCacheEntry>,
    ip: std::net::IpAddr,
) -> Option<String> {
    let now = Instant::now();
    match dns_cache.get(&ip) {
        Some(entry) if entry.expires_at > now => Some(entry.host.clone()),
        Some(_) => {
            dns_cache.remove(&ip);
            None
        }
        None => None,
    }
}

fn extract_dns_mapping(message: &[u8]) -> Option<(String, Vec<std::net::IpAddr>, u32)> {
    if message.len() < 12 {
        return None;
    }

    let qdcount = u16::from_be_bytes([message[4], message[5]]) as usize;
    let ancount = u16::from_be_bytes([message[6], message[7]]) as usize;
    if qdcount == 0 || ancount == 0 {
        return None;
    }

    let mut offset = 12;
    let host = parse_dns_name(message, &mut offset, 0)?;
    if offset + 4 > message.len() {
        return None;
    }
    offset += 4;

    let mut addresses = Vec::new();
    let mut ttl = u32::MAX;
    for _ in 0..ancount {
        let _ = parse_dns_name(message, &mut offset, 0)?;
        if offset + 10 > message.len() {
            return None;
        }
        let record_type = u16::from_be_bytes([message[offset], message[offset + 1]]);
        let class = u16::from_be_bytes([message[offset + 2], message[offset + 3]]);
        let record_ttl = u32::from_be_bytes([
            message[offset + 4],
            message[offset + 5],
            message[offset + 6],
            message[offset + 7],
        ]);
        let rdlen = u16::from_be_bytes([message[offset + 8], message[offset + 9]]) as usize;
        offset += 10;
        if offset + rdlen > message.len() {
            return None;
        }

        if class == 1 {
            match (record_type, rdlen) {
                (1, 4) => {
                    addresses.push(std::net::IpAddr::V4(std::net::Ipv4Addr::new(
                        message[offset],
                        message[offset + 1],
                        message[offset + 2],
                        message[offset + 3],
                    )));
                    ttl = ttl.min(record_ttl);
                }
                (28, 16) => {
                    let mut octets = [0_u8; 16];
                    octets.copy_from_slice(&message[offset..offset + 16]);
                    addresses.push(std::net::IpAddr::V6(std::net::Ipv6Addr::from(octets)));
                    ttl = ttl.min(record_ttl);
                }
                _ => {}
            }
        }

        offset += rdlen;
    }

    if addresses.is_empty() {
        None
    } else {
        Some((host, addresses, if ttl == u32::MAX { 300 } else { ttl }))
    }
}

fn parse_dns_name(message: &[u8], offset: &mut usize, depth: u8) -> Option<String> {
    if depth > 8 || *offset >= message.len() {
        return None;
    }

    let mut labels = Vec::new();
    let mut cursor = *offset;
    let mut jumped = false;

    loop {
        let len = *message.get(cursor)?;
        if len & 0xC0 == 0xC0 {
            let next = *message.get(cursor + 1)?;
            let pointer = usize::from(u16::from_be_bytes([len & 0x3F, next]));
            let mut nested = pointer;
            let suffix = parse_dns_name(message, &mut nested, depth + 1)?;
            if !suffix.is_empty() {
                labels.push(suffix);
            }
            cursor += 2;
            jumped = true;
            break;
        }
        if len == 0 {
            cursor += 1;
            break;
        }
        let label_len = len as usize;
        let start = cursor + 1;
        let end = start + label_len;
        let label = std::str::from_utf8(message.get(start..end)?).ok()?.to_string();
        labels.push(label);
        cursor = end;
    }

    if !jumped {
        *offset = cursor;
    } else {
        *offset = cursor;
    }

    Some(labels.join("."))
}
