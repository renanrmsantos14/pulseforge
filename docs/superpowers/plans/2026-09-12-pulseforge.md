# PulseForge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Build a lightweight Linux Mint equalizer with configurable 10/15/31-band profiles, automatic output matching, animated native UI, and autostart.

**Architecture:** GTK4/libadwaita presents state owned by a Rust application service. A dedicated audio service manages a PipeWire filter node and applies immutable profile snapshots; configuration and output rules persist independently from audio processing.

**Tech Stack:** Rust stable, GTK4, libadwaita, PipeWire SPA/WirePlumber, serde JSON, Cargo, Debian packaging.

**Spec:** `implementation_plan.md`

## Global Constraints

- Linux Mint target; PipeWire primary, PulseAudio compatibility through PipeWire.
- Audio thread performs no heap allocation and never blocks on UI/config I/O.
- UI uses semantic theme tokens, visible focus, keyboard operation, and reduced-motion support.
- Profile matching priority: persistent sink id, sink name, sink type, global fallback.
- Required checks: `cargo test`, `cargo clippy -- -D warnings`, release build, `.deb` build.

### Task 1: Bootstrap native application

**Files:** Create `Cargo.toml`, `src/main.rs`, `src/app.rs`, `src/config/mod.rs`, `src/config/model.rs`, `tests/config_model.rs`.

- [ ] Install Rust stable and GTK4/libadwaita development packages on target machine.
- [ ] Define crate dependencies: `gtk4`, `libadwaita`, `pipewire`, `serde`, `serde_json`, `directories`, `anyhow`, `thiserror`.
- [ ] Add `Profile`, `Band`, `OutputRule`, and `AppConfig` with `schema_version`.
- [ ] Add tests for default 31-band profile and JSON round-trip.
- [ ] Run `cargo test` and `cargo clippy -- -D warnings`.

### Task 2: Implement DSP core

**Files:** Create `src/audio/mod.rs`, `src/audio/dsp.rs`, `tests/dsp.rs`.

- [ ] Implement `BiquadCoefficients` and `BiquadState` for stereo processing.
- [ ] Implement `EqualizerChain::from_profile`, `process_in_place`, `set_snapshot`, and `bypass`.
- [ ] Add tests for bypass identity, flat profile stability, and gain clamp.
- [ ] Benchmark one stereo buffer and verify no per-buffer allocation.

### Task 3: Add PipeWire backend

**Files:** Create `src/audio/backend.rs`, `src/audio/pipewire.rs`, `src/audio/events.rs`, `tests/backend_contract.rs`.

- [ ] Define `AudioBackend` methods `list_outputs`, `default_output`, `attach_filter`, `detach_filter`, and `apply_snapshot`.
- [ ] Enumerate sinks through PipeWire registry and subscribe to node/default metadata changes.
- [ ] Route audio through filter node with stable reconnect behavior.
- [ ] Return actionable diagnostics when PipeWire or filter creation fails.
- [ ] Test backend contract with a mock and run integration test when PipeWire is available.

### Task 4: Persist profiles and automatic matching

**Files:** Create `src/config/store.rs`, `src/profiles.rs`, `src/output_rules.rs`, `tests/profile_matching.rs`.

- [ ] Save atomically under XDG config and create a timestamped backup before migration.
- [ ] Implement profile CRUD and import/export validation.
- [ ] Implement `match_profile(output, rules, fallback)` using the stated priority.
- [ ] Add tests for exact id, name, type, fallback, disabled rule, and disconnected output.

### Task 5: Build GTK interface

**Files:** Create `src/ui/mod.rs`, `src/ui/dashboard.rs`, `src/ui/equalizer.rs`, `src/ui/profiles.rs`, `src/ui/rules.rs`, `src/ui/diagnostics.rs`, `data/style.css`.

- [ ] Build dashboard with output selector, profile selector, bypass, preamp, status, and diagnostics.
- [ ] Build 10/15/31-band editor with accessible labels and keyboard adjustment.
- [ ] Build profile and output-rule views with explicit empty/error states.
- [ ] Render spectrum at bounded frame rate and keep it independent from DSP thread.
- [ ] Add dark instrument-panel theme: graphite surfaces, electric cyan action, amber warning.
- [ ] Add 150–300 ms state transitions and disable decorative animation under reduced motion.

### Task 6: Autostart and packaging

**Files:** Create `src/autostart.rs`, `data/pulseforge.desktop`, `data/icons/pulseforge.svg`, `README.md`, `packaging/pulseforge.install`.

- [ ] Toggle XDG autostart file from settings with enabled/disabled state reflected in UI.
- [ ] Support start minimized and tray when available; keep window-minimized fallback.
- [ ] Build release binary and `.deb` with clean install/uninstall paths.
- [ ] Document PipeWire diagnostics, reset config, and common Mint permission issues.

### Task 7: Verification and performance gate

**Files:** Create `tests/smoke.rs`, `docs/verification.md`.

- [ ] Run unit, integration, clippy, and release build checks.
- [ ] Verify profile switch after output change within 500 ms.
- [ ] Verify no audible glitch during rapid slider changes.
- [ ] Measure startup under 1 s and resident memory target under ~80 MB, excluding shared libraries.
- [ ] Verify keyboard navigation, screen-reader labels, dark contrast, and reduced-motion behavior.

## Execution order

Tasks 1–4 form the functional core; Task 5 depends on their interfaces; Tasks 6–7 finish delivery. Do not start UI polish before Task 3 proves filter-node creation on the target Mint environment.
