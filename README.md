# Titan

Titan is a Rust proxy toolkit written in Rust. It currently focuses on a practical local proxy workflow: subscription import, mixed inbound proxying, rule-based routing, runtime inspection, and Windows system proxy integration.

## Current Status

As of 2026-03-22, the project can:

- Fetch and parse Clash-style subscriptions
- Save configs to `data/config.yaml` by default
- Save subscriptions to a custom path with `subscribe --output`
- Download and refresh local GEOIP source data
- Download and refresh local GEOSITE source data
- Start one mixed inbound port for both `SOCKS5` and `HTTP/CONNECT`
- Enable or disable the Windows system proxy from the CLI
- Optionally expose a local HTTP API for runtime inspection
- Route traffic through the implemented `AnyTLS`, `HTTP`, `SOCKS5`, `VLESS`, and `VMess` outbound
- Resolve `select`, `url-test`, and `fallback` groups at startup
- Prefer `AnyTLS` candidates in auto-selection groups when they are present
- Persist manual selections for `select` groups
- Match `DOMAIN`, `DOMAIN-SUFFIX`, `DOMAIN-KEYWORD`, `DST-PORT`, `PROCESS-NAME`, `IP-CIDR`, `IP-CIDR6`, `GEOIP`, and `GEOSITE`

Current limitations:

- `AnyTLS`, `HTTP`, `SOCKS5`, `VLESS`, and `VMess` are implemented for outbound traffic
- `VLESS` and `VMess` currently support TCP transport only
- `Trojan`, `Shadowsocks`, and `Hysteria2` outbound are still skipped at runtime
- Rule matching can use direct IP targets and can also resolve a hostname when an IP-based rule requires it
- Failed outbounds are temporarily marked unhealthy and inbound connection setup retries the next usable candidate once
- DNS resolution now uses configured `nameserver` entries instead of always relying on the system resolver

## Subscription

Use your own Clash-style subscription URL. Do not commit live tokens or private links.

## Build

Build a local binary:

```bash
cargo build
```

Build a release binary:

```bash
cargo build --release
```

## Import Subscription

Save to the default path:

```bash
cargo run -- subscribe --url "<your-subscription-url>"
```

Save to a custom file:

```bash
cargo run -- subscribe --url "<your-subscription-url>" --output "data/riolu.yaml"
```

Inspect the parsed config:

```bash
cargo run -- info -c data/config.yaml
```

Update the local GEOIP file once:

```bash
cargo run -- geoip-update --output data/geoip-apnic.raw
```

Keep refreshing the GEOIP file every 24 hours:

```bash
cargo run -- geoip-update --output data/geoip-apnic.raw --interval-hours 24
```

Note: the running rule engine now checks the GEOIP file for changes and reloads it automatically. By default, file changes are picked up within a few seconds.

Update GEOSITE categories referenced by the current config:

```bash
cargo run -- geosite-update -c data/config.yaml
```

Update one specific GEOSITE category:

```bash
cargo run -- geosite-update --category google
```

Keep refreshing GEOSITE data every 24 hours:

```bash
cargo run -- geosite-update -c data/config.yaml --interval-hours 24
```

The running rule engine also checks local GEOSITE files for changes and reloads them automatically within a few seconds.

## Run Proxy

Start the local mixed proxy:

```bash
cargo run -- run -c data/config.yaml -b 127.0.0.1 -p 7890
```

Start the local proxy and automatically set the Windows system proxy:

```bash
cargo run -- run -c data/config.yaml -b 127.0.0.1 -p 7890 --set-system-proxy
```

Then point your client to one of these:

- `SOCKS5 127.0.0.1:7890`
- `HTTP 127.0.0.1:7890`

Manage the Windows system proxy directly:

```bash
cargo run -- system-proxy set --host 127.0.0.1 --port 7890
cargo run -- system-proxy status
cargo run -- system-proxy unset
```

Notes:

- The current system proxy integration is Windows-only
- System proxy just sends browser and system HTTP or HTTPS traffic into Titan
- The actual routing decision still follows your current `mode` and rules inside `data/config.yaml`
- This is not the same as TUN or transparent proxy, so some applications may still bypass Titan

Start the proxy with the local API enabled:

```bash
cargo run -- run -c data/config.yaml -b 127.0.0.1 -p 7890 --api-port 9090
```

Available API endpoints:

- `GET /health`
- `GET /stats`
- `GET /sessions`
- `GET /config`
- `GET /proxies`
- `POST /reload`

`POST /reload` reloads the current config file from disk and rebuilds the rule engine, DNS resolver, and outbound manager without restarting the process.

`GET /proxies` now includes:

- Configured proxies and proxy groups
- Each group's runtime-selected member as `runtime_selected`
- Currently unhealthy proxies as `unhealthy_proxies`

## Select Node

List current groups and selections:

```bash
cargo run -- info -c data/config.yaml
```

Select a node in a `select` group:

```bash
cargo run -- select -c data/config.yaml -g "<group-name>" -p "<proxy-name>"
```

The command rewrites the YAML through `serde_yaml`, so comments and original formatting may change.

## Binary Usage

If you prefer running the compiled binary directly:

```bash
target/debug/titan.exe run -c data/config.yaml -b 127.0.0.1 -p 7890 --set-system-proxy
target/debug/titan.exe system-proxy status
```

Or after a release build:

```bash
target/release/titan.exe run -c data/config.yaml -b 127.0.0.1 -p 7890 --set-system-proxy
```

## Verified Behavior

The current end-to-end path has been verified with real requests through the local proxy:

- `https://www.youtube.com/` -> `200`
- `https://www.google.com/` -> `200`

That verification was done through the `AnyTLS` outbound path. `HTTP`, `SOCKS5`, `VLESS`, and `VMess` outbound now have local handshake coverage in unit tests.

Recent local verification also covered:

- Full workspace test pass with `cargo test`
- Core failover behavior when a selected proxy becomes unhealthy
- API reporting of runtime-selected groups and unhealthy proxies
- DNS packet parsing and custom nameserver handling

## Data Layout

- Default config path: `data/config.yaml`
- Default GEOIP path: `data/geoip-apnic.raw`
- Default GEOSITE dir: `data/geosite`
- Example alternate output: `data/riolu.yaml`
- The `data/` directory is intended for local runtime files and is ignored by Git by default
