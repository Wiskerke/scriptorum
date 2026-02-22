{
  description = "Scriptorum - Supernote sync system";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    android-nixpkgs = {
      url = "github:tadfisher/android-nixpkgs";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, android-nixpkgs, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          config.allowUnfree = true;
          overlays = [ rust-overlay.overlays.default ];
        };

        androidSdk = android-nixpkgs.sdk.${system} (sdkPkgs: with sdkPkgs; [
          cmdline-tools-latest
          build-tools-30-0-3
          build-tools-34-0-0
          platform-tools
          platforms-android-30
          platforms-android-34
          ndk-26-1-10909125
          emulator
          system-images-android-30-google-apis-x86-64
        ]);

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
          targets = [ "aarch64-linux-android" ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            cargo-ndk
            androidSdk
            jdk17
            just
            ripgrep
            caddy
            openssl
          ];

          ANDROID_HOME = "${androidSdk}/share/android-sdk";
          ANDROID_SDK_ROOT = "${androidSdk}/share/android-sdk";
          ANDROID_NDK_ROOT = "${androidSdk}/share/android-sdk/ndk/26.1.10909125";
          JAVA_HOME = "${pkgs.jdk17}";
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";

          shellHook = ''
            echo "Scriptorum dev environment loaded"
            echo "  Rust: $(rustc --version)"
            echo "  Android SDK: $ANDROID_HOME"
          '';
        };
      }
    );
}
