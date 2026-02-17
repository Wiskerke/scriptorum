# Scriptorum

A custom sync system for Supernote e-ink tablets. Replaces the built-in Supernote private cloud with a lightweight setup: an Android app that syncs `/Note` files over WireGuard to a Rust server you control.

## Why

The Supernote private cloud is resource-heavy, has no CLI access to notes, and requires their infrastructure. Scriptorum gives you full control: your notes sync to your own server over an encrypted WireGuard tunnel.

## How it works

```
Supernote                                Your Server
(Android app)                            (Rust/Axum)
    |                                        |
    |-- POST /api/v1/sync/diff ------------->|  send local file manifest
    |<------------- SyncDiff (JSON) ---------|  server says what to upload/download
    |                                        |
    |-- PUT /api/v1/files/{path} ----------->|  upload new/changed files
    |<- GET /api/v1/files/{path} ------------|  download new/changed files
```

Conflict resolution is last-write-wins by modification time.

## Architecture

**Monorepo** with a Rust workspace and an Android Gradle project:

| Component | Description |
|-----------|-------------|
| `crates/scriptorum-core` | Shared library: checksums, file scanning, sync protocol types, diff logic, HTTP client |
| `crates/scriptorum-server` | Axum HTTP server with file storage and manifest tracking |
| `crates/scriptorum-android` | JNI bridge exposing the Rust core to Kotlin |
| `android/` | Kotlin app: sync button, log view, WireGuard/WiFi control |

The Kotlin shell handles Android system APIs (WiFi panel, WireGuard broadcast intents, UI). All sync logic runs in Rust via JNI.

## Setup

### Prerequisites

- [Nix](https://nixos.org/) with flakes enabled
- [direnv](https://direnv.net/) (optional, for automatic shell activation)

### Getting started

```sh
direnv allow  # or: nix develop

# Run the server
just server

# Run all tests
just test
```

### Emulator testing

```sh
just avd-create              # create Android Virtual Device (once)
just emulator                # launch emulator (in a separate terminal)
just emulator-install-wireguard  # sideload WireGuard into emulator
just emulator-seed-notes     # push test notes to /sdcard/Note
just emulator-install        # build and install the app
just server                  # run server (in a separate terminal)
```

The emulator reaches the host at `10.0.2.2:3742`.

### Deploying to Supernote

```sh
just device-install          # build and install via adb
```

Configure a WireGuard tunnel on the Supernote pointing to your server. The app's sync flow:

1. Opens WiFi settings panel (user enables WiFi)
2. Brings WireGuard tunnel up
3. Syncs notes via Rust HTTP client
4. Brings WireGuard tunnel down
5. Opens WiFi settings panel (user disables WiFi)

## Commands

| Command | Description |
|---------|-------------|
| `just test` | Run all Rust tests (unit + integration + e2e) |
| `just check` | Clippy lint + format check |
| `just server` | Start the sync server |
| `just apk` | Build the Android APK |
| `just build-android-lib` | Cross-compile Rust JNI library for arm64 |
| `just emulator` | Launch the Android emulator |
| `just emulator-install` | Build everything and install on emulator |
| `just emulator-seed-notes` | Push testfiles/ to emulator's /sdcard/Note |
