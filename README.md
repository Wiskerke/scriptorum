# Scriptorum

Scriptorum provides a basic workflow to synchronize between a Supernote e-ink table and a personal server.

It is a work-in-progress, and I cannot recommend it for usage by anyone. I am not even yet using it.

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

Future plans:
 - With AI integration, the structure will change. I expect something like this: A) The client will request a sync diff, and only receives the list of files to upload. B) The client provides all changed files. C) The client tells the server to process them. The processing might result in commands to the AI which could result in changes in the notes or new notes. The processing can result in info messages and a final list of files to download. D) The client downloads all the files. 
 - I would want a mechanism to backup all files on the server which have been received from the client. The risk of conflicts seems minimal, but it would be good to have an option to recover older notes. 

## Architecture

**Monorepo** with a Rust workspace and an Android Gradle project:

| Component | Description |
|-----------|-------------|
| `crates/scriptorum-core` | Shared library: checksums, file scanning, sync protocol types, diff logic, HTTP client |
| `crates/scriptorum-server` | Axum HTTP server with file storage and manifest tracking |
| `crates/scriptorum-android` | JNI bridge exposing the Rust core to Kotlin |
| `android/` | Kotlin app: sync button, log view, WiFi control |

The Kotlin shell handles Android system APIs (WiFi panel, UI). All sync logic runs in Rust via JNI.

## Setup

### Prerequisites

- [Nix](https://nixos.org/) with flakes enabled
- [direnv](https://direnv.net/) (optional, for automatic shell activation)

### Getting started

I would recommend not to use this project in the current state. 
But if you are insterested, `direnv allow` and reading through the `justfile` are probably a good starting point.
