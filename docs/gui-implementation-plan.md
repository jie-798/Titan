# Titan GUI Implementation Plan

## Goal

Turn the current desktop shell into a practical daily-use control surface for Titan.

The current GUI already has these pages:

- `Overview`
- `Proxies`
- `Connections`
- `Tests`
- `Rules`
- `Updates`
- `Settings`

This plan now focuses on polishing and filling the remaining gaps instead of initial scaffolding.

## Current State

Implemented today:

- Tauri desktop backend under `apps/desktop/src-tauri`
- React + TypeScript frontend under `apps/desktop`
- Shared GUI-facing service layer in `crates/app`
- Runtime start and stop
- Windows system proxy toggle
- Proxy group inspection and manual selection
- Session inspection with active and closed history
- Batch proxy testing surface
- Subscription usage and refresh controls

Still incomplete:

- stronger visual polish across all pages
- richer test presets and result caching
- deeper rule explanation tooling
- more complete TUN controls in GUI
- stronger tray and background lifecycle behavior

## Technical Direction

- Desktop shell: `Tauri v2`
- Frontend: `React + TypeScript + Vite`
- Desktop communication: `Tauri invoke`
- Shared backend integration: `crates/app`
- Runtime config source of truth: `data/config.yaml`
- Desktop-local state file: `data/desktop-config.json`

## Work Areas

### Backend

Primary files:

- `crates/app/**`
- `apps/desktop/src-tauri/**`
- `crates/api/**`

Current backend responsibilities:

- expose GUI-facing DTOs
- start, stop, and reload runtime
- bridge desktop actions to shared runtime services
- bridge desktop actions to system proxy control
- expose session history, proxy overview, and subscription refresh

### Frontend

Primary files:

- `apps/desktop/src/**`

Current frontend responsibilities:

- page composition and layout
- runtime status display
- session and traffic presentation
- proxy selection UX
- test report display
- settings and update flows

## Near-Term Priorities

### 1. Visual polish

- improve spacing, typography, and information density
- add clearer iconography and state emphasis
- make the layout feel closer to a polished desktop client

### 2. Connections page refinement

- add stronger filtering controls
- improve row density and sorting
- add clearer separation between active and closed sessions

### 3. Tests page refinement

- add reusable presets
- make single-node, group, and broader batch testing clearer
- cache the latest results more intentionally

### 4. Rules and updates polish

- improve rule summary readability
- surface more subscription metadata cleanly
- make refresh outcomes more obvious

### 5. TUN settings integration

- expose more of the current TUN config safely
- avoid presenting unsupported combinations as if they are production ready

## Definition of Done for the Next GUI Milestone

The next milestone is done when a Windows user can:

- start Titan from the GUI
- toggle the Windows system proxy
- inspect and switch proxy groups
- inspect active and closed sessions
- run batch proxy tests and review results
- inspect subscription usage and refresh the subscription
- use the GUI comfortably without needing the CLI for normal daily tasks
