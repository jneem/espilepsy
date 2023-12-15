{
  description = "Devshell for esp32c3 dev";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix.url = "github:nix-community/fenix";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, fenix, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ fenix.overlays.default ];
        };
        rust-toolchain = pkgs.fenix.complete.withComponents [
          "rustc" "cargo" "rust-src" "clippy" "rustfmt" "rust-analyzer"
        ];
      in
      {
        devShell = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            rust-toolchain
            rust-analyzer
            cargo-espflash
            cargo-expand
            cargo-outdated
            taplo
          ];
        };
      }
    );
}
