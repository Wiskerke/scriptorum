# Scriptorum

A personal sync system for the [Supernote](https://supernote.com/) e-ink tablet.
An Android app running on the Supernote syncs `.note` files over mutual TLS to a
self-hosted Rust server.

## Why?

I wanted a personal minimal cloud solution for my supernote usage. It was also a possibility to experiment using AI for development, and to experiment with slightly different workflows.
I also want something that integrates well with [SilverBullet](https://silverbullet.md/). I imagine SilverBullet as the basic UI to view notes on the server.

Some alternatives:

- [Supernote private cloud](https://support.supernote.com/setting-up-your-own-supernote-private-cloud-beta) is a bit too big with multiple dockers and databases, and requiring an email server.
- I was already invested in my own solution, when I found out about [supernote knowledge hub](https://allenporter.github.io/supernote/supernote.html). It does seem interesting to me.

## How it works

The basic idea is that there is a side-loaded android app on the Supernote which will communicate with the server. It will upload all notebooks that are not on the server, and download files that are only on the server.

The communication uses mTLS (via HTTPS): The app identifies itself to the server via a client certificate. The server only listens to clients that have a valid certificate. The client only communicates if the server has a valid certificate. 
Caddy is used as reverse proxy to handle HTTPS and certificate validation. Which is a well regarded application, which should make this safe enough to expose as a personal server to the internet.

The certificates on the Supernote are stored on the device filesystem (`/sdcard/Scriptorum/`) and read by the app at sync time. The APK itself is generic — you push your own certs to the device via `adb`.

## Architecture

**Monorepo** with a Rust workspace and an Android Gradle project:

| Component | Description |
|-----------|-------------|
| `crates/scriptorum-core` | Shared library: checksums, scanning, sync protocol types, diff logic, HTTP client |
| `crates/scriptorum-server` | Axum HTTP server with file storage and manifest tracking |
| `crates/scriptorum-android` | JNI bridge exposing the Rust core to Kotlin |
| `android/` | Kotlin app: sync button, log view, WiFi control |

The Kotlin shell handles Android system APIs. All sync logic runs in Rust via JNI.

## Self-hosting quickstart

### 1. Add the NixOS module

In your `flake.nix`:

```nix
inputs.scriptorum.url = "github:Wiskerke/scriptorum";
```

In your NixOS configuration:

```nix
{ inputs, ... }: {
  imports = [ inputs.scriptorum.nixosModules.default ];

  services.scriptorum = {
    enable = true;
    storageDir = "/var/lib/scriptorum/notes";
    bindAddress = "127.0.0.1:3742";
    # openFirewall = false;  # keep false if Caddy is in front
  };
}
```

### 2. Generate certificates

```bash
just gen-certs your.server.example.com
```

This creates `certs/ca.pem`, `certs/server.pem`, `certs/client.pem`, and their keys.

### 3. Configure Caddy for mTLS

Caddy handles TLS termination and client cert verification.
Add to your Caddyfile (adjust paths to your generated certs):

```
your.server.example.com {
    tls /path/to/certs/server.pem /path/to/certs/server-key.pem

    @mtls {
        tls client_auth {
            mode require_and_verify
            trusted_ca_certs_pem_file /path/to/certs/ca.pem
        }
    }

    handle @mtls {
        reverse_proxy 127.0.0.1:3742
    }

    handle {
        respond "Unauthorized" 401
    }
}
```

### 4. Push certs to your device

Sideload the APK onto your Supernote, then push your certs and server URL:

```bash
just install-device-certs https://your.server.example.com
```

Or manually:

```bash
adb shell mkdir -p /sdcard/Scriptorum
adb push certs/ca.pem /sdcard/Scriptorum/ca.pem
adb push certs/client.pem /sdcard/Scriptorum/client.pem
adb push certs/client-key.pem /sdcard/Scriptorum/client-key.pem
echo '{"server_url": "https://your.server.example.com"}' | adb shell 'cat > /sdcard/Scriptorum/config.json'
```

The app reads certs from `/sdcard/Scriptorum/` at sync time and shows setup instructions if they are missing.

## Development setup

### Prerequisites

- [Nix](https://nixos.org/) with flakes enabled
- [direnv](https://direnv.net/) (optional, for automatic shell activation)

```bash
direnv allow        # or: nix develop
```

### Emulator workflow

```bash
just avd-create                                    # create AVD (once)
just emulator                                      # launch emulator (separate terminal)
just gen-certs                                     # generate certs (once)
just emulator-install                              # build + install Scriptorum APK
just install-device-certs https://10.0.2.2         # push certs to emulator
just emulator-seed-notes                           # push testfiles/ to /sdcard/Note
just server                                        # run server (separate terminal)
just caddy                                         # run Caddy mTLS proxy (separate terminal)
```

### Building from source

```bash
just test                  # run all Rust tests (unit + e2e)
just check                 # clippy + fmt check
just server                # run the server on 0.0.0.0:3742
just build-android-lib     # cross-compile JNI lib for arm64
just apk                   # build the Android APK
```

### NixOS notes

- AGP downloads a dynamically linked `aapt2` that won't run on NixOS. `just apk`
  passes `-Pandroid.aapt2FromMavenOverride` to use the Nix-provided one.
- `ANDROID_NDK_ROOT` points to `ndk/26.1.10909125` (not `ndk-bundle`).
