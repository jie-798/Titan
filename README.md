# Titan

Titan is a Rust proxy toolkit written in Rust. It currently focuses on a practical local proxy workflow: subscription import, mixed inbound proxying, rule-based routing, runtime inspection, and Windows system proxy integration.

## Current Status

As of 2026-03-24, the project can:

- Fetch and parse Clash-style subscriptions
- Save configs to `data/config.yaml` by default
- Save subscriptions to a custom path with `subscribe --output`
- Download and refresh local GEOIP source data
- Download and refresh local GEOSITE source data
- Start one mixed inbound port for both `SOCKS5` and `HTTP/CONNECT`
- Enable or disable the Windows system proxy from the CLI
- Optionally expose a local HTTP API for runtime inspection
- Expose session history and batch proxy testing through both CLI and GUI
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
- The desktop GUI now includes `Overview`, `Proxies`, `Connections`, `Tests`, `Rules`, `Updates`, and `Settings`

## Subscription

Use your own Clash-style subscription URL. Do not commit live tokens or private links.

## Build

Build a release binary:

```bash
cargo build --release
```

## Import Subscription

Save to the default path:

```bash
target/release/titan.exe subscribe --url "<your-subscription-url>"
```

Save to a custom file:

```bash
target/release/titan.exe subscribe --url "<your-subscription-url>" --output "data/riolu.yaml"
```

Inspect the parsed config:

```bash
target/release/titan.exe info -c data/config.yaml
```

Update the local GEOIP file once:

```bash
target/release/titan.exe geoip-update --output data/geoip-apnic.raw
```

Keep refreshing the GEOIP file every 24 hours:

```bash
target/release/titan.exe geoip-update --output data/geoip-apnic.raw --interval-hours 24
```

Note: the running rule engine now checks the GEOIP file for changes and reloads it automatically. By default, file changes are picked up within a few seconds.

Update GEOSITE categories referenced by the current config:

```bash
target/release/titan.exe geosite-update -c data/config.yaml
```

Update one specific GEOSITE category:

```bash
target/release/titan.exe geosite-update --category google
```

Keep refreshing GEOSITE data every 24 hours:

```bash
target/release/titan.exe geosite-update -c data/config.yaml --interval-hours 24
```

The running rule engine also checks local GEOSITE files for changes and reloads them automatically within a few seconds.

## Run Proxy

Start the local mixed proxy:

```bash
target/release/titan.exe run -c data/config.yaml -b 127.0.0.1 -p 7890
```

Start the local proxy and automatically set the Windows system proxy:

```bash
target/release/titan.exe run -c data/config.yaml -b 127.0.0.1 -p 7890 --set-system-proxy
```

Then point your client to one of these:

- `SOCKS5 127.0.0.1:7890`
- `HTTP 127.0.0.1:7890`

Manage the Windows system proxy directly:

```bash
target/release/titan.exe system-proxy set --host 127.0.0.1 --port 7890
target/release/titan.exe system-proxy status
target/release/titan.exe system-proxy unset
```

Notes:

- The current system proxy integration is Windows-only
- System proxy just sends browser and system HTTP or HTTPS traffic into Titan
- The actual routing decision still follows your current `mode` and rules inside `data/config.yaml`
- This is not the same as TUN or transparent proxy, so some applications may still bypass Titan

Start the proxy with the local API enabled:

```bash
target/release/titan.exe run -c data/config.yaml -b 127.0.0.1 -p 7890 --api-port 9090
```

Available API endpoints:

- `GET /health`
- `GET /stats`
- `GET /sessions`
- `POST /sessions/close-all`
- `POST /sessions/clear-history`
- `GET /config`
- `GET /proxies`
- `POST /reload`

`POST /reload` reloads the current config file from disk and rebuilds the rule engine, DNS resolver, and outbound manager without restarting the process.

`GET /proxies` now includes:

- Configured proxies and proxy groups
- Each group's runtime-selected member as `runtime_selected`
- Currently unhealthy proxies as `unhealthy_proxies`

`GET /sessions` supports:

- `?state=active`
- `?state=closed`
- `?state=all`
- `?limit=<n>`

## Runtime Inspection

Inspect runtime state from the local API:

```bash
target/release/titan.exe runtime --api http://127.0.0.1:9090
```

Inspect active or closed sessions:

```bash
target/release/titan.exe sessions --api http://127.0.0.1:9090 --state active
target/release/titan.exe sessions --api http://127.0.0.1:9090 --state closed --limit 50
```

Close all active sessions or clear closed-session history:

```bash
target/release/titan.exe sessions --api http://127.0.0.1:9090 --close-all
target/release/titan.exe sessions --api http://127.0.0.1:9090 --clear-history
```

Note: `sessions` and `runtime` require the local API to be enabled, for example:

```bash
target/release/titan.exe run -c data/config.yaml -b 127.0.0.1 -p 7890 --api-port 9090
```

## Select Node

List current groups and selections:

```bash
target/release/titan.exe info -c data/config.yaml
```

Select a node in a `select` group:

```bash
target/release/titan.exe select -c data/config.yaml -g "<group-name>" -p "<proxy-name>"
```

The command rewrites the YAML through `serde_yaml`, so comments and original formatting may change.

## Binary Usage

Use the compiled release binary directly:

```bash
target/release/titan.exe run -c data/config.yaml -b 127.0.0.1 -p 7890 --set-system-proxy
target/release/titan.exe system-proxy status
```

## Desktop GUI

Build the desktop frontend and backend:

```bash
cargo build -p titan-desktop
cd apps/desktop
npm run build
```

For normal use, build the release desktop binary:

```bash
cargo build -p titan-desktop --release
```

Run the current desktop binary:

```bash
target/release/titan-desktop.exe
```

Current desktop pages:

- `Overview`
- `Proxies`
- `Connections`
- `Tests`
- `Rules`
- `Updates`
- `Settings`

Current desktop capabilities:

- Start and stop the runtime
- Toggle the Windows system proxy
- Inspect proxy groups and select nodes
- View active and recently closed sessions
- Clear session history
- Run batch proxy tests and inspect the latest test report
- Inspect subscription usage and refresh the subscription

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
- Default TUN config path: `data/tun.yaml`
- Default TUN state path: `data/tun-state.yaml`
- Example alternate output: `data/riolu.yaml`
- The `data/` directory is intended for local runtime files and is ignored by Git by default
