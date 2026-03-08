# Scriptorum

Supernote note sync system: Android app syncs `/Note` files over mTLS to a Rust server.

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
just gen-certs             # generate CA, server, and client certs in ./certs
                           # Optional: SERVER_HOSTNAME=your.host just gen-certs
just install-certs         # copy client certs to Android assets for APK bundling
just gen-placeholder-certs # regenerate non-functional placeholder certs in assets/
```

**Placeholder certs**: `android/app/src/main/assets/certs/` contains non-functional
placeholder PEMs committed to the repo so `just apk` works without setup.
They will not authenticate against any real server. Replace with real certs by running
`just gen-certs && just install-certs`.

## Emulator workflow

```
just avd-create            # create AVD (once)
just emulator              # launch emulator (separate terminal)
just gen-certs             # generate certs (once)
just install-certs         # bundle client certs into APK assets (once, or after cert rotation)
just emulator-seed-notes   # push testfiles/ to /sdcard/Note
just emulator-install      # build + install Scriptorum APK
just server                # run server (separate terminal)
just caddy                 # run Caddy mTLS proxy (separate terminal)
```

## APK personalisation (for distributed builds)

```
just inject-certs -- \
  --ca certs/ca.pem --cert certs/client.pem --key certs/client-key.pem \
  --url https://your.server \
  input.apk output.apk
```

This removes the placeholder certs/config from an APK, injects real ones, re-aligns,
and re-signs. Requires `zip`, `zipalign`, `apksigner` (all in the Nix dev shell).

## NixOS

- The flake exposes `packages.scriptorum-server` (Rust binary) and
  `nixosModules.default` (systemd service with `services.scriptorum.*` options).
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
- The `client` feature flag on scriptorum-core enables the HTTP sync client (pulls in ureq, rustls, rustls-pemfile)
