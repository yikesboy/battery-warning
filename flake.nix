{
  description = "My custom battery notification util";

  inputs = {
    nixpkgs.url = "https://channels.nixos.org/nixpkgs-unstable/nixexprs.tar.zst";
  };

  outputs = inputs:
    let
      forAllSystems = builtins.mapAttrs;
    in
    {
      packages = forAllSystems (system: pkgs: {
        rust-toolchain = pkgs.buildEnv {
          name = "rust-toolchain";
          paths = with pkgs; [ rustc cargo rustfmt clippy ];
        };

        default = pkgs.rustPlatform.buildRustPackage {
          pname = "battery-warning";
          version = "0.1.0";
          src = inputs.self;
          cargoLock.lockFile = ./Cargo.lock;
          nativeCheckInputs = with pkgs; [ dbus bash coreutils ];
          BATTERY_TEST_DBUS_CONFIG = "${pkgs.dbus}/share/dbus-1/session.conf";
          doCheck = true;
          checkPhase = ''
            runHook preCheck
            cargo test --offline --locked
            runHook postCheck
          '';
        };
      }) inputs.nixpkgs.legacyPackages;

      devShells = forAllSystems (system: pkgs: {
        default = pkgs.mkShell {
          packages = [ inputs.self.packages.${system}.rust-toolchain pkgs.dbus ];
        };
      }) inputs.nixpkgs.legacyPackages;

      checks = forAllSystems (system: pkgs: {
        package = inputs.self.packages.${system}.default;
        lint = inputs.self.packages.${system}.default.overrideAttrs (old: {
          pname = "battery-warning-lint";
          nativeCheckInputs = old.nativeCheckInputs ++ [ pkgs.rustfmt pkgs.clippy ];
          # Clippy checks all targets; do not also compile a release binary here.
          dontBuild = true;
          checkPhase = ''
            runHook preCheck
            cargo fmt --check
            cargo clippy --offline --locked --all-targets -- -D warnings
            runHook postCheck
          '';
          installPhase = ''
            mkdir -p "$out"
          '';
        });
        rust-toolchain = pkgs.runCommand "rust-toolchain-check" {
          nativeBuildInputs = [ inputs.self.packages.${system}.rust-toolchain ];
        } ''
          rustc --version
          cargo --version
          cargo fmt --version
          cargo clippy --version
          touch "$out"
        '';
      }) inputs.nixpkgs.legacyPackages;
    };
}
