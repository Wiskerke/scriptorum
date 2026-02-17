server:
    cargo run -p scriptorum-server

build-android-lib:
    cargo ndk -t arm64-v8a -o android/app/src/main/jniLibs build -p scriptorum-android --release

test:
    cargo test --workspace

check:
    cargo clippy --workspace -- -D warnings && cargo fmt --check

apk:
    cd android && ./gradlew assembleRelease -Pandroid.aapt2FromMavenOverride="$ANDROID_HOME/build-tools/34.0.0/aapt2"

# Android emulator

avd-create:
    avdmanager create avd -n scriptorum -k "system-images;android-30;google_apis;x86_64" -d pixel_4 --force

emulator:
    emulator -avd scriptorum -gpu swiftshader_indirect

emulator-install: build-android-lib apk
    adb install -r android/app/build/outputs/apk/release/app-release.apk

# Install on a real device (Supernote)
device-install: build-android-lib apk
    adb install -r android/app/build/outputs/apk/release/app-release.apk

emulator-seed-notes:
    adb shell mkdir -p /sdcard/Note
    adb push testfiles/. /sdcard/Note/

emulator-install-wireguard:
    adb install -r ~/experiments/wireguard-server/com.wireguard.android-1.0.20260102.apk
