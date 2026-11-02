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

        default = inputs.self.packages.${system}.rust-toolchain;
      }) inputs.nixpkgs.legacyPackages;

      devShells = forAllSystems (system: pkgs: {
        default = pkgs.mkShell {
          packages = [ inputs.self.packages.${system}.rust-toolchain ];
        };
      }) inputs.nixpkgs.legacyPackages;

      checks = forAllSystems (system: pkgs: {
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
