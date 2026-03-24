pub fn system_proxy_set(host: &str, port: u16) -> anyhow::Result<()> {
    let mut proxy = titan_system_proxy::SystemProxy::new();
    proxy.set(&titan_system_proxy::ProxyConfig::new(host, port))?;
    std::mem::forget(proxy);
    println!("System proxy enabled: {}:{}", host, port);
    Ok(())
}

pub fn system_proxy_unset() -> anyhow::Result<()> {
    let mut proxy = titan_system_proxy::SystemProxy::new();
    proxy.disable()?;
    println!("System proxy disabled");
    Ok(())
}

pub fn system_proxy_status() -> anyhow::Result<()> {
    let proxy = titan_system_proxy::SystemProxy::new();
    let (enabled, server, bypass) = proxy.current_state()?;
    println!("Enabled: {}", enabled);
    println!("Server: {}", server);
    println!("Bypass: {}", bypass);
    Ok(())
}
