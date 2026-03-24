# Titan GUI Architecture

## Goal

Build a Windows-first desktop GUI for Titan that can:

- import and manage subscriptions
- start and stop the local proxy engine
- enable and disable system proxy
- inspect runtime status, sessions, and traffic
- choose nodes and proxy groups
- update GEOIP and GEOSITE data
- reload configuration without forcing users into the CLI

The GUI should feel like a real desktop control plane for the existing Rust runtime, not a thin wrapper around terminal commands.

## Recommended Stack

### Desktop shell

Use `Tauri v2` for the desktop application shell.

Why this is the best fit for Titan now:

- the project is already Rust-first
- system proxy control is already implemented in Rust
- the runtime engine is already in-process Rust code
- Tauri lets us keep one native backend and a separate UI layer
- the frontend can move faster without forcing the runtime to become web-shaped

### Frontend

Use a small SPA frontend inside Tauri.

Recommended frontend shape:

- `React + TypeScript`
- `Vite`
- state management kept simple at first with `zustand` or plain React state
- charts only where useful, not everywhere

This keeps the UI work separate enough for interface-focused iteration while still being practical for a Rust-heavy project.

## High-Level Architecture

The GUI should not call CLI commands like `run`, `select`, or `subscribe` through shell execution.

That would create avoidable problems:

- parsing terminal output instead of typed results
- hard-to-control process lifecycle
- duplicated logic between CLI and GUI
- weaker error handling
- more fragile Windows integration

Instead, the architecture should be:

```text
Tauri Frontend
    ->
Tauri Commands / Event Bridge
    ->
Titan App Service Layer
    ->
Existing Core Crates
```

## Proposed Workspace Layout

Add two new parts:

- `crates/app`
- `apps/desktop`

Suggested responsibility split:

- `crates/app`
  - GUI-facing application service layer
  - process lifecycle and shared app state
  - typed operations for config, runtime, subscription, updates, and system proxy
- `apps/desktop`
  - Tauri app
  - frontend assets
  - windowing, menus, tray, notifications, settings persistence for GUI-only preferences

## Backend Responsibilities

### 1. Application service layer

Create a new crate, for example `crates/app`, with an `AppService` that wraps current capabilities into stable typed methods.

Core responsibilities:

- load current config from `data/config.yaml`
- import subscription into a target file
- start the engine
- stop the engine
- reload config
- expose runtime snapshots
- change selected node in a `select` group
- run GEOIP and GEOSITE update tasks
- enable, disable, and inspect system proxy

This layer becomes the single backend contract for both GUI and future non-CLI integrations.

### 2. Runtime controller

The GUI needs stronger lifecycle control than the CLI currently exposes.

Add a runtime controller object that owns:

- the current `ProxyEngine`
- optional API server task if still needed
- inbound server task
- runtime status
- last error
- system proxy state applied by the GUI

Suggested shape:

```text
AppService
  - config_store
  - runtime_controller
  - system_proxy_controller
  - update_manager
```

### 3. Structured snapshots for the GUI

The frontend should not build itself from raw internal structs spread across many crates.

Expose GUI-specific DTOs such as:

- `AppStatus`
- `RuntimeSnapshot`
- `ProxyGroupView`
- `ProxyNodeView`
- `SessionView`
- `TrafficSnapshot`
- `SystemProxyStatus`
- `UpdateTaskStatus`

This gives the UI a stable contract even if internals evolve.

### 4. Event stream

The GUI should receive runtime changes without requiring manual refresh for every action.

For MVP, polling is acceptable for some views:

- stats every 1 second
- sessions every 2 seconds
- proxies every 2 seconds

But the backend should still prepare for event-driven updates.

Recommended event categories:

- runtime started
- runtime stopped
- config reloaded
- proxy group selection changed
- system proxy changed
- update task progress
- runtime error

## Existing Crates and How the GUI Should Use Them

### Reuse directly

- `titan-core`
- `titan-config`
- `titan-rules`
- `titan-dns`
- `titan-system-proxy`

### Reuse with adaptation

- `titan-api`

The current HTTP API is useful for external inspection, but the GUI should not depend on localhost HTTP to talk to its own backend. The GUI backend can call Rust code directly and optionally still expose the HTTP API for power users.

### Refactor away from CLI-only flow

The GUI should not depend on:

- terminal printing
- shelling out to `titan.exe`
- parsing human-readable CLI output

The CLI can later become a thin layer over the same `crates/app` service methods.

## Process Model

### Recommended MVP

Single desktop process with in-process runtime.

That means:

- Tauri host process owns `AppService`
- `AppService` owns the engine and controllers
- frontend communicates via Tauri commands and events

Why this is good for MVP:

- simpler deployment
- easy reuse of current Rust runtime
- no extra local daemon management
- easier system proxy cleanup on app exit

### Future-ready direction

If Titan later needs background persistence or multi-window control, split into:

- background service process
- separate GUI process

That is not necessary yet.

## Configuration Strategy

The GUI should treat `data/config.yaml` as the source of truth for runtime config, but avoid destructive rewrite patterns where possible.

Short-term:

- continue using the current config model
- save explicit changes through structured operations
- warn users that formatting/comments may change

Medium-term:

- split user state from imported subscription material
- keep manual selections and app preferences in a separate local state file

Suggested GUI-local state file:

- `data/gui-state.json`

Good candidates for that file:

- window state
- last opened tab
- recent subscription URLs if user opts in
- current GUI theme
- last selected group in the dashboard

## Runtime UX Requirements That Affect Backend Design

The backend must support these GUI interactions cleanly:

- start proxy
- stop proxy
- restart proxy
- enable system proxy together with start
- disable system proxy independently
- switch a selected node and optionally hot reload
- inspect which group is currently active at runtime
- see unhealthy nodes and recent failover results
- display active sessions and traffic totals

That means several existing runtime values should become explicit service methods rather than only being available through logs.

## Proposed Backend API Surface for GUI

The desktop layer should expose typed commands roughly like:

- `app.load_config()`
- `app.import_subscription(url, output_path)`
- `app.start_runtime(options)`
- `app.stop_runtime()`
- `app.restart_runtime()`
- `app.reload_runtime()`
- `app.get_runtime_snapshot()`
- `app.get_proxy_overview()`
- `app.select_group_proxy(group, proxy)`
- `app.get_sessions()`
- `app.get_stats()`
- `app.get_system_proxy_status()`
- `app.set_system_proxy(enabled)`
- `app.run_geoip_update(options)`
- `app.run_geosite_update(options)`

## Security and Safety

The GUI should make the safe path the default.

Important safeguards:

- never display full subscription tokens unless explicitly revealed
- mask sensitive URLs in recent-history UI
- confirm before overwriting config files
- on app exit, restore system proxy if Titan enabled it in this session
- clearly separate runtime errors from user mistakes

## Windows-First Requirements

The first desktop target should be Windows because:

- system proxy support already exists there
- the current user workflow is Windows-based
- it reduces scope while the runtime is still maturing

Recommended Windows-first features:

- tray integration
- start minimized option later
- explicit system proxy indicator
- clear status line for `Running`, `Stopped`, `System Proxy On`, `System Proxy Off`

## Implementation Order

### Phase 1

- add `crates/app`
- move reusable runtime control out of CLI commands
- define GUI-facing DTOs
- define Tauri command list

### Phase 2

- create `apps/desktop`
- build shell window and navigation
- connect dashboard, proxy groups, and runtime controls

### Phase 3

- add subscription import, GEOIP update, GEOSITE update, and settings pages
- add tray integration and better lifecycle handling

### Phase 4

- add richer live events, error history, and background behavior

## Recommended First Development Milestone

The first usable GUI milestone should be intentionally narrow:

- one main window
- start and stop Titan
- show current runtime status
- show proxy groups and selected node
- enable and disable system proxy
- import subscription
- reload config

If those pieces are stable, the rest of the GUI can grow on a solid base instead of becoming a polished shell over fragile internals.
