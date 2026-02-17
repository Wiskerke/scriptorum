# Scriptorum

Scriptorum provides a basic workflow to synchronize between a Supernote e-ink table and a personal server.

It supports syncing of all the notes in the `/Note` over WireGuard to a server you control. (Or at least once we get a little bit farther in the project.)

## Why?

This project basically fulfills a personal desire.

- I want a personal cloud solution for my supernote device, without depending on external parties.
- I want to have the possibility to add custom processing on the server after receiving notes. Something that takes the notes, translates them to markdown, and then adds them to my personal documentation structure. Or maybe I can hook in an AI account and perform requests to it via a note, and receive an answer back via notes. Not sure yet, but it is something I would like to try and experiment with.
- I tried the supernote private cloud, but it was not the thing I wanted. It was using a lot of resources, multiple dockers, databases, and features like requiring an email smtp server. I think it is great that supernote provides this option, but it was not what I was looking for.

Some other aspects:

- I like using WireGuard to protect the communication between client and server, which seems like a good way to ensure my device is the only one that can connect with the server. Maybe I'll look into using https with a client certificate at some point. 
- It would have been nice if I could have integrated with the nice sync button in the supernote UI, but I didn't really want to start reverse engineering the private cloud. That just seemed a bit like too much effort and trouble. And starting a side-loaded app is pretty userfriendly as well, once you've moved it up in the list. 


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
just avd-create                  # create Android Virtual Device (once)
just emulator                    # launch emulator (run this in a separate shell or in the background)
just emulator-install-wireguard  # push WireGuard.apk to the emulator
just emulator-seed-notes         # push test notes to /sdcard/Note
just emulator-install            # build and install the app
just server                      # run server (in a separate terminal)
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

Please run `just -l` to check the commands and what they do
