# Scriptorum

A personal sync system for the [Supernote](https://supernote.com/) e-ink tablet.
An Android app running on the Supernote syncs `.note` files over mutual TLS to a
self-hosted Rust server.

## Why?

Mostly to to scratch an itch and make the supernote more useful for me.

Some things I would like to have:

- A personal cloud solution for my supernote device, without depending on public cloud servers.
- The possibility to add custom processing on the server after receiving notes. Something that takes the notes, translates them to markdown, and then adds them to my personal documentation structure. Or maybe hook in some AI and perform requests to it via a note, and receive an answer back as a note. Not sure where to go there, but it sounds like fun.
- I tried the supernote private cloud, but it was not for me. It was using a lot of resources, multiple dockers, databases, and features like requiring an email smtp server. Things you need when you want to support multiple users. I think it is great that ratta provides this option, but it did not spark joy for me.

Some other aspects:

- I currently am planning to use a mTLS configuration with a personal CA certificate, and require the client to identify itself via the HTTPS channel. This seems like a good compromise of secure communication, while being fairly standard.
- It would have been nice if I could have integrated with the nice sync button in the supernote UI, but I didn't want to start reverse engineering the private cloud interfaces. That just seemed a bit too much efforts and trouble. And starting a side-loaded app is pretty userfriendly, and allows me a bit more custom options if needed.


## How it works

```
Supernote (Android app)          Your Server (Rust/Axum)
        |                                  |
        |-- POST /api/v1/sync/diff ------->|  send local file manifest
        |<------- SyncDiff (JSON) ---------|  server says what to upload/download
        |                                  |
        |-- PUT  /api/v1/files/{path} ---->|  upload new/changed files
        |<-- GET /api/v1/files/{path} -----|  download new/changed files
```

Conflict resolution is last-write-wins by modification time.

Future plans:
 - With AI integration, the structure will change. I expect something like this: A) The client will request a sync diff, and only receives the list of files to upload. B) The client provides all changed files. C) The client tells the server to process them. The processing might result in commands to the AI which could result in changes in the notes or new notes. The processing can result in info messages and a final list of files to download. D) The client downloads all the files. 
 - I would want a mechanism to backup all files on the server which have been received from the client. The risk of conflicts seems minimal, but it would be good to have an option to recover older notes. 

## Architecture

**Monorepo** with a Rust workspace and an Android Gradle project:

| Component | Description |
|-----------|-------------|
| `crates/scriptorum-core` | Shared library: checksums, scanning, sync protocol types, diff logic, HTTP client |
| `crates/scriptorum-server` | Axum HTTP server with file storage and manifest tracking |
| `crates/scriptorum-android` | JNI bridge exposing the Rust core to Kotlin |
| `android/` | Kotlin app: sync button, log view, WiFi control |

The Kotlin shell handles Android system APIs. All sync logic runs in Rust via JNI.

---

## Self-hosting quickstart

### 1. Add the NixOS module

In your `flake.nix`:

```nix
inputs.scriptorum.url = "github:YOUR_USERNAME/scriptorum";
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
SERVER_HOSTNAME=your.server.example.com just gen-certs
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

### 4. Personalise the APK

Download the latest release APK, then inject your certs and server URL:

```bash
just inject-certs -- \
  --ca certs/ca.pem \
  --cert certs/client.pem \
  --key certs/client-key.pem \
  --url https://your.server.example.com \
  scriptorum-release.apk \
  scriptorum-personal.apk
```

Sideload `scriptorum-personal.apk` onto your Supernote.

---

## Development setup

### Prerequisites

- [Nix](https://nixos.org/) with flakes enabled
- [direnv](https://direnv.net/) (optional, for automatic shell activation)

```bash
direnv allow        # or: nix develop
```

### Emulator workflow

```bash
just avd-create            # create AVD (once)
just emulator              # launch emulator (separate terminal)
just gen-certs             # generate certs (once)
just install-certs         # bundle certs into APK assets
just emulator-seed-notes   # push testfiles/ to /sdcard/Note
just emulator-install      # build + install Scriptorum APK
just server                # run server (separate terminal)
just caddy                 # run Caddy mTLS proxy (separate terminal)
```

### Building from source

```bash
just test                  # run all Rust tests (unit + e2e)
just check                 # clippy + fmt check
just server                # run the server on 0.0.0.0:3742
just build-android-lib     # cross-compile JNI lib for arm64
just apk                   # build the Android APK
```

**Note:** The repo ships placeholder certs in `android/app/src/main/assets/certs/`
so `just apk` works out of the box.  They are non-functional for real servers.
Replace them with `just gen-certs && just install-certs`.

### NixOS notes

- AGP downloads a dynamically linked `aapt2` that won't run on NixOS. `just apk`
  passes `-Pandroid.aapt2FromMavenOverride` to use the Nix-provided one.
- `ANDROID_NDK_ROOT` points to `ndk/26.1.10909125` (not `ndk-bundle`).

## License

BSD 3-Clause — see [LICENSE](LICENSE).
