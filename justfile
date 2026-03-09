# Build and run the backend server
server:
    cargo run -p scriptorum-server

# Build the android libraries
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
apk: build-android-lib
    cd android && ./gradlew assembleRelease -Pandroid.aapt2FromMavenOverride="$ANDROID_HOME/build-tools/34.0.0/aapt2"

# Create the android emulation environment (run this once)
avd-create:
    avdmanager create avd -n scriptorum -k "system-images;android-30;google_apis;x86_64" -d pixel_4 --force

# Run the android emulation environment
emulator:
    emulator -avd scriptorum -gpu swiftshader_indirect

# Install the android app on the emulator
emulator-install: apk
    adb install -r android/app/build/outputs/apk/release/app-release.apk

# Install the android app on a real device (Supernote)
device-install: apk
    adb install -r android/app/build/outputs/apk/release/app-release.apk

# Copy some test notes to the emulator
emulator-seed-notes:
    adb shell mkdir -p /sdcard/Note
    adb push testfiles/. /sdcard/Note/

# Generate mTLS certificates (CA, server, client)
# Optional: just gen-certs your.server.example.com
gen-certs hostname="":
    ./scripts/gen-certs.sh {{ if hostname != "" { "--hostname " + hostname } else { "" } }}

# Push certs and server config to the connected device or emulator
# Usage: just install-device-certs https://your.server.example.com
install-device-certs URL:
    adb shell mkdir -p /sdcard/Scriptorum
    adb push certs/ca.pem /sdcard/Scriptorum/ca.pem
    adb push certs/client.pem /sdcard/Scriptorum/client.pem
    adb push certs/client-key.pem /sdcard/Scriptorum/client-key.pem
    echo '{"server_url": "{{URL}}"}' | adb shell 'cat > /sdcard/Scriptorum/config.json'

# Run Caddy as mTLS reverse proxy in front of the server
caddy:
    sudo sysctl -q net.ipv4.ip_unprivileged_port_start=443
    caddy run --config Caddyfile

# Start server and caddy in background (logs in logs/)
start-server:
    mkdir -p logs
    cargo build -p scriptorum-server
    sudo sysctl -q net.ipv4.ip_unprivileged_port_start=443
    ./target/debug/scriptorum-server > logs/server.log 2>&1 & echo $! > logs/server.pid
    caddy run --config Caddyfile > logs/caddy.log 2>&1 & echo $! > logs/caddy.pid
    @echo "Server PID: $(cat logs/server.pid), Caddy PID: $(cat logs/caddy.pid)"
    @echo "Logs: logs/server.log, logs/caddy.log"

# Stop background server and caddy
stop-server:
    -kill $(cat logs/server.pid 2>/dev/null) 2>/dev/null; rm -f logs/server.pid
    -kill $(cat logs/caddy.pid 2>/dev/null) 2>/dev/null; rm -f logs/caddy.pid
    @echo "Server and Caddy stopped."

# Restart server and caddy
restart-server:
    just stop-server
    just start-server

# Start emulator in background (logs in logs/)
start-emulator:
    mkdir -p logs
    DISPLAY="${DISPLAY:-:0}" XAUTHORITY="${XAUTHORITY:-$HOME/.Xauthority}" emulator -avd scriptorum -gpu swiftshader_indirect > logs/emulator.log 2>&1 & echo $! > logs/emulator.pid
    @echo "Emulator PID: $(cat logs/emulator.pid)"
    @echo "Logs: logs/emulator.log"

# Stop background emulator
stop-emulator:
    -kill $(cat logs/emulator.pid 2>/dev/null) 2>/dev/null; rm -f logs/emulator.pid
    @echo "Emulator stopped."

# Restart emulator
restart-emulator:
    just stop-emulator
    just start-emulator
