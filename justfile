# Build and run the backend server
server:
    cargo run -p scriptorum-server

# Build the android libraries
build-android-lib:
    cargo ndk -t arm64-v8a -o android/app/src/main/jniLibs build -p scriptorum-android --release

# Run the unit tests and the integration tests
test:
    cargo test --workspace

# Lint and format the rust code
check:
    cargo clippy --workspace -- -D warnings && cargo fmt --check

# Build the android app (resulting in an apk file)
apk:
    cd android && ./gradlew assembleRelease -Pandroid.aapt2FromMavenOverride="$ANDROID_HOME/build-tools/34.0.0/aapt2"

# Create the android emulation environment (run this once)
avd-create:
    avdmanager create avd -n scriptorum -k "system-images;android-30;google_apis;x86_64" -d pixel_4 --force

# Run the android emulation environment
emulator:
    emulator -avd scriptorum -gpu swiftshader_indirect

# Install the android app on the emulator
emulator-install: build-android-lib apk
    adb install -r android/app/build/outputs/apk/release/app-release.apk

# Install the android app on a real device (Supernote)
device-install: build-android-lib apk
    adb install -r android/app/build/outputs/apk/release/app-release.apk

# Copy some test notes to the emulator
emulator-seed-notes:
    adb shell mkdir -p /sdcard/Note
    adb push testfiles/. /sdcard/Note/

# Generate mTLS certificates (CA, server, client)
# Optional: SERVER_HOSTNAME=your.server.example.com just gen-certs
gen-certs:
    ./scripts/gen-certs.sh

# Copy client certs to Android assets for APK bundling (overwrites placeholder certs)
install-certs:
    mkdir -p android/app/src/main/assets/certs
    cp certs/ca.pem certs/client.pem certs/client-key.pem android/app/src/main/assets/certs/

# Generate placeholder (non-functional) certs and commit them to the assets directory
# Only needed when bootstrapping the repo. Real users run gen-certs + install-certs.
gen-placeholder-certs:
    ./scripts/gen-placeholder-certs.sh

# Inject real certs (and optionally server URL) into a distributed APK and re-sign it
# Usage: just inject-certs -- --ca ca.pem --cert client.pem --key client-key.pem --url https://your.server input.apk output.apk
inject-certs *ARGS:
    ./scripts/inject-certs.sh {{ARGS}}

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
