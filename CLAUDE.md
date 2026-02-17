# Scriptorum

Supernote note sync system: Android app syncs `/Note` files over WireGuard to a Rust server.

## Architecture

- **scriptorum-core**: shared Rust library (checksums, scanning, sync protocol types, diff logic, HTTP sync client)
- **scriptorum-server**: Axum HTTP server storing files and computing sync diffs
- **scriptorum-android**: JNI bridge (cdylib) exposing core to Kotlin
- **android/**: Kotlin shell app (WiFi panel, WireGuard intents, UI)

## Build

Requires Nix with flakes. Enter the dev shell with `direnv allow` or `nix develop`.

```
just test                  # run all Rust tests (unit + e2e)
just check                 # clippy + fmt check
just server                # run the server on 0.0.0.0:3742
just build-android-lib     # cross-compile JNI lib for arm64
just apk                   # build the Android APK
```

## Emulator workflow

```
just avd-create            # create AVD (once)
just emulator              # launch emulator (separate terminal)
just emulator-install-wireguard  # sideload WireGuard APK
just emulator-seed-notes   # push testfiles/ to /sdcard/Note
just emulator-install      # build + install Scriptorum APK
just server                # run server (separate terminal, reachable at 10.0.2.2:3742)
```

## NixOS notes

- AGP downloads a dynamically linked aapt2 that won't run on NixOS. The `just apk` command passes `-Pandroid.aapt2FromMavenOverride` to use the Nix-provided aapt2 from build-tools.
- The `ANDROID_NDK_ROOT` points to `ndk/26.1.10909125` (not `ndk-bundle`).

## Sync Protocol

```
POST /api/v1/sync/diff    — client sends Manifest, server returns SyncDiff
PUT  /api/v1/files/{path} — upload file (X-SHA256 header)
GET  /api/v1/files/{path} — download file
GET  /api/v1/health       — health check
```

Conflict resolution: last-write-wins by mtime.

## Conventions

- Paths in FileEntry are relative to the note root (e.g. "Daily/2026-02-17.note")
- SHA256 as lowercase hex strings
- Unix timestamps in seconds for modified times
- The `client` feature flag on scriptorum-core enables the HTTP sync client (pulls in ureq)
