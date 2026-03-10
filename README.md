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

The `certificates/` folder contains scripts to generate the CA and certificates needed for mTLS. 

Tip: You can copy the `certificates` folder to your home folder or somewhere else outside the repo. That way it is easier to keep track of the certificates you create and to add more in the future. The examples below assume you are working from inside the repo's `certificates/` folder; adjust the path if you copied it elsewhere.

All scripts require `openssl` and output to `./certs/` by default (`--out` overrides this). Scripts that sign a certificate read the CA from `./certs/` by default (`--ca-dir` overrides this).

**`gen-ca`** — generates the CA certificate (`ca.pem`, `ca-key.pem`) used to sign all other certificates. Run once. `--name` sets the CN only; the output filenames are always `ca.pem` and `ca-key.pem`.

**`gen-server-cert`** — generates a server certificate (`<name>.pem`, `<name>-key.pem`) signed by the CA. `--name` sets both the CN and the output filename prefix (default: `server`). Add the hostname or IP that clients will use to connect to the server as argument(s).

**`gen-client-cert`** — generates a client certificate (`<name>.pem`, `<name>-key.pem`, `<name>.p12`) signed by the CA. `--name` sets both the CN and the output filename prefix (default: `client`). The `.p12` bundle is for importing into a browser or OS certificate store; the Supernote only needs the `.pem` files.

Use a distinct `--name` for each certificate so browsers and OS certificate stores can tell them apart.

```bash
cd certificates
just gen-ca --name my_ca
just gen-server-cert --name my_server your.server.example.com
just gen-client-cert --name my_nomad
just gen-client-cert --name my_laptop
```

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

### 4. Install the app

Build the APK and install it on your Supernote via adb. If you have the emulator running, stop it first — adb treats it as a connected device and may install there instead.

```bash
just device-install
```

### 5. Push certs to your device

Push your certs and server URL via adb. Adjust the local paths to match wherever your `certs/` folder lives. The app expects the files to be named exactly `ca.pem`, `client.pem`, and `client-key.pem` on the device, regardless of what `--name` you used when generating them:

```bash
adb shell mkdir -p /sdcard/Scriptorum
adb push /path/to/certs/ca.pem /sdcard/Scriptorum/ca.pem
adb push /path/to/certs/my_nomad.pem /sdcard/Scriptorum/client.pem
adb push /path/to/certs/my_nomad-key.pem /sdcard/Scriptorum/client-key.pem
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

If you have a real device connected, disconnect it first — adb treats it as a connected device and `emulator-install` may install there instead.

```bash
just emulator-create     # create AVD (once)
just emulator-start      # launch emulator (separate terminal)
just emulator-gen-certs  # generate certs (once)
just emulator-install    # build + install APK, push certs, seed notes
just testserver-start    # run server + Caddy mTLS proxy (separate terminal)
```

### Building from source

```bash
just test            # run all Rust tests (unit + e2e)
just check           # clippy + fmt check
just build-apk       # build the Android APK
just device-install  # build + install APK on a real device
```

### NixOS notes

- AGP downloads a dynamically linked `aapt2` that won't run on NixOS. `just build-apk`
  passes `-Pandroid.aapt2FromMavenOverride` to use the Nix-provided one.
- `ANDROID_NDK_ROOT` points to `ndk/26.1.10909125` (not `ndk-bundle`).
