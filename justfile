default:
    just --list

[private]
build-android-lib:
    cargo ndk -t arm64-v8a -o android/app/src/main/jniLibs build -p scriptorum-android --release

# Run the unit tests and the integration tests
test:
    cargo test --workspace
    cargo test -p scriptorum-core --features client

# Lint and format the rust code
check:
    cargo clippy --workspace -- -D warnings && cargo fmt --check

# Build the android app (resulting in an apk file)
build-apk: build-android-lib
    cd android && ./gradlew assembleRelease -Pandroid.aapt2FromMavenOverride="$ANDROID_HOME/build-tools/34.0.0/aapt2"

# Install the android app on a real device (Supernote)
device-install: build-apk
    adb install -r android/app/build/outputs/apk/release/app-release.apk

# Create the android emulation environment (run this once)
emulator-create:
    avdmanager create avd -n scriptorum -k "system-images;android-30;google_apis;x86_64" -d pixel_4 --force

# Generate all certificates needed for the emulator workflow (CA, server, client).
# Certs are written to ./emulator-certs/ and used by Caddyfile.testserver.
emulator-gen-certs:
    ./certificates/gen-ca.sh --out ./emulator-certs
    ./certificates/gen-server-cert.sh --out ./emulator-certs --ca-dir ./emulator-certs 10.0.2.2
    ./certificates/gen-client-cert.sh --out ./emulator-certs --ca-dir ./emulator-certs

# Build and install the app on the emulator, push certs and seed test notes
emulator-install: build-apk
    @test -f emulator-certs/ca.pem || (echo "error: emulator-certs/ not found — run 'just emulator-gen-certs' first" && exit 1)
    adb install -r android/app/build/outputs/apk/release/app-release.apk
    adb shell mkdir -p /sdcard/Scriptorum
    adb push emulator-certs/ca.pem /sdcard/Scriptorum/ca.pem
    adb push emulator-certs/client.pem /sdcard/Scriptorum/client.pem
    adb push emulator-certs/client-key.pem /sdcard/Scriptorum/client-key.pem
    echo '{"server_url": "https://10.0.2.2"}' | adb shell 'cat > /sdcard/Scriptorum/config.json'
    adb shell mkdir -p /sdcard/Note
    adb push testfiles/. /sdcard/Note/

# Start emulator in background (logs in logs/)
emulator-start:
    mkdir -p logs
    DISPLAY="${DISPLAY:-:0}" XAUTHORITY="${XAUTHORITY:-$HOME/.Xauthority}" emulator -avd scriptorum -gpu swiftshader_indirect > logs/emulator.log 2>&1 & echo $! > logs/emulator.pid
    @echo "Emulator PID: $(cat logs/emulator.pid)"
    @echo "Logs: logs/emulator.log"

# Stop background emulator
emulator-stop:
    -kill $(cat logs/emulator.pid 2>/dev/null) 2>/dev/null; rm -f logs/emulator.pid
    @echo "Emulator stopped."

# Restart emulator
emulator-restart:
    just emulator-stop
    just emulator-start

# Start server and caddy in background (logs in logs/)
testserver-start:
    @test -f emulator-certs/ca.pem || (echo "error: emulator-certs/ not found — run 'just emulator-gen-certs' first" && exit 1)
    mkdir -p logs
    cargo build -p scriptorum-server
    sudo sysctl -q net.ipv4.ip_unprivileged_port_start=443
    ./target/debug/scriptorum-server --storage ./testserver-files > logs/server.log 2>&1 & echo $! > logs/server.pid
    caddy run --config Caddyfile.testserver > logs/caddy.log 2>&1 & echo $! > logs/caddy.pid
    @echo "Server PID: $(cat logs/server.pid), Caddy PID: $(cat logs/caddy.pid)"
    @echo "Logs: logs/server.log, logs/caddy.log"

# Stop background server and caddy
testserver-stop:
    -kill $(cat logs/server.pid 2>/dev/null) 2>/dev/null; rm -f logs/server.pid
    -kill $(cat logs/caddy.pid 2>/dev/null) 2>/dev/null; rm -f logs/caddy.pid
    @echo "Server and Caddy stopped."

# Restart server and caddy
testserver-restart:
    just testserver-stop
    just testserver-start
