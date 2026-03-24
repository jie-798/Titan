# Titan Desktop GUI Design

## Goal

This document defines the first desktop GUI for Titan. The GUI should make the current proxy workflow easier to start, observe, and adjust without replacing the core CLI or runtime architecture.

The GUI must fit the current Titan capabilities:

- Mixed inbound proxy on one local port
- Windows system proxy toggle
- Runtime status via local API
- Subscription import and config loading
- Manual node selection
- GEOIP and GEOSITE update workflows

The product goal is not to mimic a generic proxy dashboard. The interface should feel deliberate, compact, and operational.

## Product Principles

- Make the current state obvious at a glance.
- Keep the main flow to three actions: connect, inspect, adjust.
- Prefer direct action over hidden configuration.
- Show routing decisions in plain language.
- Treat background state as first-class information, not an advanced panel.
- Keep Windows as the first-class platform.

## Primary Users

- A user who wants a browser to work through Titan with minimal setup.
- A power user who needs to inspect nodes, groups, and runtime health.
- A user who periodically refreshes subscription, GEOIP, and GEOSITE data.

## Information Architecture

The application should use a left navigation rail with a persistent top status bar.

Top-level sections:

- Overview
- Proxies
- Rules
- Network
- Updates
- Settings

The home screen should be Overview, because that is where the user decides whether Titan is healthy and connected.

## Key Screens

### Overview

Purpose:

- Show whether Titan is running
- Show whether system proxy is enabled
- Show current mode, active inbound port, and current runtime-selected node
- Surface health issues and recent routing events

Core content:

- Large connection state card
- System proxy toggle card
- Runtime summary strip
- Recent sessions table
- Health and warning feed

The overview should answer, in under five seconds, “Is Titan ready?”

### Proxies

Purpose:

- Show all proxy nodes and groups
- Let the user select a node for a `select` group
- Show which nodes are healthy, unhealthy, or skipped

Core content:

- Left-side group list
- Main group detail panel
- Node table with type, server, port, runtime state, and action menu
- One-click select action for `select` groups

The selection UI should make it clear when a group is manual, automatic, or fallback-driven.

### Rules

Purpose:

- Show rule order and rule types
- Explain why a request matched a route

Core content:

- Rule list with type badges
- Rule detail panel
- Match preview panel for hostname, IP, or port

This screen should help a user understand routing without needing to read config files.

### Network

Purpose:

- Show sessions, traffic, and runtime API state

Core content:

- Session list
- Upload/download totals
- Active connection count
- Runtime-selected proxies
- Unhealthy proxy list

### Updates

Purpose:

- Import subscriptions
- Refresh GEOIP and GEOSITE data
- Show last update status

Core content:

- Subscription import form
- GEOIP update control
- GEOSITE update control
- Last updated timestamps

### Settings

Purpose:

- Configure local bind addresses and API port
- Set startup behavior
- Control Windows system proxy behavior

Core content:

- Mixed inbound bind and port
- API bind and port
- Auto-enable system proxy on launch
- Default config path

## User Flows

### First Launch

1. User opens the app.
2. Overview shows current state and a primary action to import or load config.
3. User imports subscription or opens an existing config.
4. Titan resolves groups and shows runtime status.
5. User enables system proxy and starts browsing.

### Manual Node Selection

1. User opens Proxies.
2. User selects a proxy group.
3. User picks a node from the node table.
4. UI confirms the active selection and runtime state.

### Update Workflow

1. User opens Updates.
2. User runs subscription import, GEOIP refresh, or GEOSITE refresh.
3. UI shows progress and completion state.
4. Runtime state updates without requiring a restart when possible.

### Recovery Workflow

1. User notices a site fails to load.
2. User checks Overview or Network.
3. UI highlights unhealthy nodes and recent failures.
4. User changes node selection or disables system proxy to test behavior.

## Component Model

The GUI should be composed of small reusable blocks:

- Status chip
- Summary card
- Action button
- Group selector
- Node table
- Session table
- Event timeline
- Progress row
- Empty state panel
- Error banner

Each component should have a stable visual language:

- Green for healthy or connected
- Amber for degraded or pending
- Red for blocked or failed
- Slate for neutral background and structure

## Status and Monitoring UX

The monitoring surface should be calm but specific.

### Tray Icon and System Tray

The application should minimize to system tray with context menu:

- Show/Hide main window
- Current connection status indicator
- Quick toggle system proxy
- Quick access to frequently used nodes
- Exit application

Tray tooltip should show:

- Titan running status
- Current active node
- System proxy status

### Notifications

The application should provide native Windows notifications for:

- Engine start/stop events
- System proxy state changes
- Subscription update completion
- GEOIP/GEOSITE update completion
- Error conditions requiring attention

### Keyboard Shortcuts

Global shortcuts (when app is in background):

- Toggle system proxy: Ctrl+Shift+P (configurable)
- Quick node switch: Ctrl+Shift+[1-9] for favorite nodes

In-app shortcuts:

- Open Overview: Ctrl+1
- Open Proxies: Ctrl+2
- Open Network: Ctrl+3
- Open Settings: Ctrl+,
- Toggle engine: Ctrl+R
- Search/Filter: Ctrl+F

## Data Visualization

### Traffic Chart

- Real-time line chart showing upload/download speeds
- Time window: last 60 seconds (scrollable)
- Y-axis: auto-scaling bandwidth (KB/s, MB/s)
- Color coding: green for download, blue for upload
- Hover tooltip showing exact values

### Latency Chart

- Bar or line chart showing node response times
- Color coding by latency range:
  - Green: < 100ms
  - Yellow: 100-300ms
  - Red: > 300ms or timeout
- Sortable by latency in node table

### Session Table

- Virtual scrolling for large session lists
- Columns: Source, Destination, Proxy, Rule, Upload, Download, Duration, Status
- Row color coding by status
- Expandable rows for connection details

## Proxy Protocol Support Display

The UI should clearly identify proxy protocol types with badges:

| Protocol | Badge Color | Icon |
|----------|-------------|------|
| VLESS | #4EC9B0 (Teal) | V |
| VMess | #569CD6 (Blue) | M |
| Trojan | #CE9178 (Orange) | T |
| Shadowsocks | #B5CEA8 (Light Green) | S |
| SOCKS5 | #DCDCAA (Yellow) | S5 |
| HTTP | #9CDCFE (Light Blue) | H |
| Hysteria2 | #C586C0 (Purple) | H2 |
| AnyTLS | #808080 (Gray) | A |

## Error Handling and Edge Cases

### Empty States

- No config loaded: Show import/configure prompt
- No proxies: Show add subscription prompt
- No sessions: Show "No active connections" message
- Update failed: Show error with retry option

### Error Recovery

- Engine crash: Show error notification, offer restart
- Node timeout: Mark node as error, show in Overview
- Config parse error: Show validation errors, prevent apply
- Network error: Show offline indicator, queue actions

### Confirmation Dialogs

- Delete subscription: Confirm with name
- Reset config: Confirm with warning
- Exit while connected: Warn about losing proxy access

## Accessibility

- All interactive elements should be keyboard accessible
- Tab order should follow logical flow
- Focus indicators should be visible
- Screen reader support for key information
- High contrast mode support
- Minimum touch target size: 44x44 pixels

## Node Selection UX

Selection should be quick and unambiguous.

Rules:

- Only groups that support manual choice should expose a direct `Select` action.
- `select` groups should show the current chosen node.
- `url-test` and `fallback` groups should show the currently resolved runtime node and why it won.
- Unhealthy nodes should be visibly marked but still readable.

The user should not need to guess whether a node is actually active.

### Node Health Testing

- Manual test button per node
- Batch test all nodes in a group
- Auto-test on configurable interval for url-test groups
- Visual feedback during test (spinner/progress)
- Test result history

## Subscription and Config UX

The config experience should support three modes:

- Load a local config file
- Import a subscription URL
- Edit current runtime settings through safe controls

The GUI should avoid raw YAML editing in the first version. Editing should happen through structured forms where possible.

Recommended safeguards:

- Show the active config path
- Warn before overwriting a config
- Preserve a backup when importing a new subscription
- Clearly separate subscription data from local runtime settings

### Config File Management

- Config file browser/selector
- Recent configs list
- Import config from clipboard (paste URL or paste YAML)
- Export current config
- Diff view when comparing configs

## Windows-First Visual Direction

The visual direction should feel like a control room, not a consumer app.

Suggested style:

- Dense but readable layout
- Strong section headers
- Clear dividers and layered panels
- Muted base surface with a single strong accent color
- Subtle motion only for state changes, loading, and transitions

The layout should work well on Windows with standard title bar, keyboard navigation, and sensible minimum window sizing.

Avoid:

- Generic rounded-card dashboard styling
- Overuse of glassmorphism
- Excessive animation
- Hidden controls behind hover only interactions

### Color Palette

| Role | Color | Usage |
|------|-------|-------|
| Background | #1E1E2E (Dark slate) | Main background |
| Surface | #2A2A3E (Lighter slate) | Cards, panels |
| Border | #3A3A4E (Muted border) | Dividers, outlines |
| Text Primary | #E4E4E7 (Off-white) | Main text |
| Text Secondary | #A1A1AA (Gray) | Secondary text |
| Accent | #6366F1 (Indigo) | Primary actions, highlights |
| Success | #22C55E (Green) | Connected, healthy |
| Warning | #F59E0B (Amber) | Degraded, pending |
| Error | #EF4444 (Red) | Failed, blocked |

## Implementation Order

1. Build the app shell, navigation rail, and top status bar.
2. Implement Overview with live runtime state and system proxy toggle.
3. Implement Proxies with group selection and node status.
4. Add Updates for subscription, GEOIP, and GEOSITE operations.
5. Add Network and Rules inspection screens.
6. Add Settings for bind addresses, API, and startup behavior.
7. Add system tray integration and notifications.
8. Add keyboard shortcuts and global hotkeys.

## Technical Notes

### Frontend State Management

Recommended approach:

- Global app state in zustand store
- Screen-specific local state with React hooks
- Server-Sent Events (SSE) or WebSocket for real-time updates from backend
- Optimistic UI updates for user actions

### Backend Communication

Use Tauri's command/invoke system:

```
Frontend (React)
  -> tauri.invoke("method_name", payload)
  -> Rust AppService
  -> Core crates
```

Event flow (backend -> frontend):

```
Core crates emit events
  -> AppService publishes
  -> Tauri emit to frontend
  -> Frontend updates state
```

### Key Tauri Commands to Implement

| Command | Purpose |
|---------|---------|
| `get_status` | Get engine and system proxy status |
| `start_engine` | Start the proxy engine |
| `stop_engine` | Stop the proxy engine |
| `get_proxies` | List all proxy groups and nodes |
| `select_proxy` | Select a node in a select group |
| `test_proxy` | Test latency for a specific node |
| `get_rules` | List all rules |
| `get_sessions` | List active sessions |
| `get_traffic` | Get traffic statistics |
| `import_subscription` | Import from subscription URL |
| `update_geoip` | Update GEOIP database |
| `update_geosite` | Update GEOSITE database |
| `get_config` | Get current config |
| `set_config` | Update config settings |
| `enable_system_proxy` | Enable Windows system proxy |
| `disable_system_proxy` | Disable Windows system proxy |
