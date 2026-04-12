{
  description = "cmux — Claude Code session manager";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {inherit system;};
      cmux = pkgs.rustPlatform.buildRustPackage {
        pname = "cmux";
        version = "0.1.0";
        src = ./.;
        cargoLock.lockFile = ./Cargo.lock;
      };
    in {
      packages.default = cmux;
      packages.cmux = cmux;

      apps.default = {
        type = "app";
        program = "${cmux}/bin/cmux";
      };

      devShells.default = pkgs.mkShell {
        packages = with pkgs; [
          rustc
          cargo
          rust-analyzer
          clippy
          rustfmt
          tmux
        ];
      };
    });
}
