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
    let
      # NixOS module (system-independent)
      nixosModule = { config, lib, pkgs, ... }:
        let
          cfg = config.services.scriptorum;
          inherit (lib) mkEnableOption mkOption types mkIf;
        in
        {
          options.services.scriptorum = {
            enable = mkEnableOption "Scriptorum note sync server";

            storageDir = mkOption {
              default = "/var/lib/scriptorum/notes";
              type = types.str;
              description = "Directory where synced note files are stored.";
            };

            bindAddress = mkOption {
              default = "127.0.0.1:3742";
              type = types.str;
              description = "Address and port for the HTTP server to listen on.";
            };

            openFirewall = mkOption {
              default = false;
              type = types.bool;
              description = "Open the bind port in the firewall. Only useful when binding to a public interface.";
            };

            user = mkOption {
              default = "scriptorum";
              type = types.str;
              description = "User to run scriptorum-server as.";
            };

            group = mkOption {
              default = "scriptorum";
              type = types.str;
              description = "Group to run scriptorum-server as.";
            };
          };

          config = mkIf cfg.enable {
            users.users.${cfg.user} = {
              isSystemUser = true;
              group = cfg.group;
              description = "Scriptorum sync server";
            };
            users.groups.${cfg.group} = { };

            systemd.tmpfiles.rules = [
              "d '${cfg.storageDir}' 0750 ${cfg.user} ${cfg.group} - -"
            ];

            systemd.services.scriptorum = {
              description = "Scriptorum note sync server";
              wantedBy = [ "multi-user.target" ];
              after = [ "network.target" ];

              serviceConfig = {
                ExecStart = "${self.packages.${pkgs.system}.scriptorum-server}/bin/scriptorum-server --bind ${cfg.bindAddress} --storage ${cfg.storageDir}";
                User = cfg.user;
                Group = cfg.group;
                Restart = "on-failure";
                RestartSec = "5s";
                # Hardening
                NoNewPrivileges = true;
                ProtectSystem = "strict";
                ProtectHome = true;
                ReadWritePaths = [ cfg.storageDir ];
                PrivateTmp = true;
              };
            };

            networking.firewall = mkIf cfg.openFirewall {
              allowedTCPPorts =
                let
                  port = lib.toInt (lib.last (lib.splitString ":" cfg.bindAddress));
                in
                [ port ];
            };
          };
        };
    in
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
        packages.scriptorum-server = pkgs.rustPlatform.buildRustPackage {
          pname = "scriptorum-server";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [ "-p" "scriptorum-server" ];
        };

        packages.default = self.packages.${system}.scriptorum-server;

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
            # For inject-certs.sh
            zip
            unzip
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
    ) // {
      nixosModules.default = nixosModule;
    };
}
