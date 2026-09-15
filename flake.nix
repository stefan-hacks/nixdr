{
  description = "nixdr — Nix error diagnosis and human-readable reporting tool";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs @ { self, nixpkgs, flake-parts, rust-overlay, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [];
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];

      perSystem = { config, self', inputs', pkgs, system, ... }:
        let
          rustToolchain = pkgs.rustPlatform;
        in
        {
          # Package definition
          packages.default = rustToolchain.buildRustPackage {
            pname = "nixdr";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            meta = with pkgs.lib; {
              description = "Nix error diagnosis and human-readable reporting tool";
              license = licenses.mit;
              maintainers = [ ];
              mainProgram = "nixdr";
            };
          };

          # App definition for `nix run`
          apps.default = {
            type = "app";
            program = "${self'.packages.default}/bin/nixdr";
          };

          # Development shell
          devShells.default = pkgs.mkShell {
            name = "nixdr-dev";
            inputsFrom = [ self'.packages.default ];
            nativeBuildInputs = with pkgs; [
              rustc
              cargo
              rustfmt
              clippy
              rust-analyzer
            ];
          };
        };
    };
}