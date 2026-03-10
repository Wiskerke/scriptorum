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
just build-apk             # build the Android APK
just device-install        # build + install APK on a real device
just testserver-start      # start test server + Caddy in background
```

## Certificate setup

Emulator certs are managed from the main justfile:

```
just emulator-gen-certs                          # generate CA + server + client into ./emulator-certs/
```

For production or custom deployments, use the standalone tools in `certificates/`:

```
cd certificates
just gen-ca                                      # generate CA only (run once)
just gen-server-cert example.com api.example.com # server cert with one or more SANs
just gen-client-cert                             # client cert: client.pem + client.p12
just gen-client-cert --name johan               # client cert with custom CN (johan.pem + johan.p12)
```

- Server cert always includes `localhost` / `127.0.0.1` in SAN; extra hostnames/IPs are added as arguments.
- Client cert is exported as both PEM and a PKCS#12 (`.p12`) bundle for browser/OS import (no password).
- Scripts: `certificates/gen-ca.sh`, `certificates/gen-server-cert.sh`, `certificates/gen-client-cert.sh`

Certs are stored on the device at `/sdcard/Scriptorum/` and read at sync time.

## Emulator workflow

```
just emulator-create                             # create AVD (once)
just emulator-start                              # launch emulator (separate terminal)
just emulator-gen-certs                          # generate certs (once)
just emulator-install                            # build + install APK, push certs, seed notes
just testserver-start                            # run server + Caddy mTLS proxy (separate terminal)
```

## NixOS

- The flake exposes `packages.scriptorum-server` (Rust binary) and
  `nixosModules.default` (systemd service with `services.scriptorum.*` options).
- AGP downloads a dynamically linked aapt2 that won't run on NixOS. The `just build-apk` command passes `-Pandroid.aapt2FromMavenOverride` to use the Nix-provided aapt2 from build-tools.
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
