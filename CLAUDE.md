# Scriptorum

Supernote sync system for notes: Android app syncs `/Note` files over mTLS to a Rust server.

## Architecture

- **scriptorum-core**: shared Rust library (checksums, scanning, sync protocol types, diff logic, HTTP sync client with mTLS)
- **scriptorum-server**: Axum HTTP server storing files and computing sync diffs
- **scriptorum-android**: JNI bridge (cdylib) exposing core to Kotlin
- **android/**: Kotlin shell app (WiFi panel, cert extraction, UI)
- **Caddy**: reverse proxy handling TLS termination and client cert verification

## Build

Requires Nix with flakes. Enter the dev shell with `direnv allow` or `nix develop`.

```
just test                  # run all Rust tests (unit + e2e)
just check                 # clippy + fmt check
just server                # run the server on 0.0.0.0:3742
just build-android-lib     # cross-compile JNI lib for arm64
just apk                   # build the Android APK
```

## Certificate setup

```
just gen-certs                                   # generate CA, server, and client certs in ./certs
just gen-certs your.host                         # also add hostname/IP to server cert SAN
                                                 # script: gen-certs.sh --hostname <name> --out <dir>
just install-device-certs https://your.server    # push certs + config to connected device/emulator
```

Certs are stored on the device at `/sdcard/Scriptorum/` and read at sync time.

## Emulator workflow

```
just avd-create                                  # create AVD (once)
just emulator                                    # launch emulator (separate terminal)
just gen-certs                                   # generate certs (once)
just emulator-install                            # build + install Scriptorum APK
just install-device-certs https://10.0.2.2       # push certs to emulator
just emulator-seed-notes                         # push testfiles/ to /sdcard/Note
just server                                      # run server (separate terminal)
just caddy                                       # run Caddy mTLS proxy (separate terminal)
```

## NixOS

- The flake exposes `packages.scriptorum-server` (Rust binary) and
  `nixosModules.default` (systemd service with `services.scriptorum.*` options).
- AGP downloads a dynamically linked aapt2 that won't run on NixOS. The `just apk` command passes `-Pandroid.aapt2FromMavenOverride` to use the Nix-provided aapt2 from build-tools.
- The `ANDROID_NDK_ROOT` points to `ndk/26.1.10909125` (not `ndk-bundle`).
- No assets are committed to the repo; `android/app/src/main/assets/` is gitignored.

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
- The `client` feature flag on scriptorum-core enables the HTTP sync client (pulls in ureq, rustls, rustls-pemfile)
