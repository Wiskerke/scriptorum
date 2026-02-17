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

# Copy the wireguard android app to the emulator
emulator-install-wireguard:
    adb install -r ~/experiments/wireguard-server/com.wireguard.android-1.0.20260102.apk
