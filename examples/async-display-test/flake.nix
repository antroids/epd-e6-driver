{
  description = "Flake configuration for my systems";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { nixpkgs, utils, rust-overlay, ... }:
    utils.lib.eachDefaultSystem (system:
      let pkgs = import nixpkgs { inherit system; overlays = [ (import rust-overlay) ]; };
      in {
        devShells.default = pkgs.mkShell {
          propagatedBuildInputs = [ pkgs.probe-rs ];

          buildInputs = [
            (pkgs.rust-bin.stable.latest.default.override {
              extensions = [
                "rust-src"
                "clippy"
                "rust-analyzer"
              ];
              targets = [
                "thumbv8m.main-none-eabihf"
              ];
            })
          ];
        };
      });
}
