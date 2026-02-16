{
  description = "Flake for bencher";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    rust-overlay,
    flake-utils,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        overlays = [(import rust-overlay)];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.minimal.override {
          extensions = [
            "rust-src"
            "clippy"
            "rust-analyzer"
          ];
        };

        # Build inputs for the Rust project
        buildInputs = with pkgs; [
          rustToolchain
          rust-bin.nightly.latest.rustfmt # Get nightly formatter.
          clang
          mold
          pkg-config
          fontconfig
          binutils
        ];
        rust_tools = with pkgs; [
          cargo-nextest
        ];
        nix_tools = with pkgs; [
          alejandra # Nix code formatter
          deadnix # Nix dead code checker
          statix # Nix static code checker.
        ];
      in {
        # Build package with `nix build`
        packages = rec {
          default = bencher;
          bencher = pkgs.rustPlatform.buildRustPackage {
            name = "bencher";
            src = ./.;
            cargoBuildFlags = [ "--bin" "bencher"];
            cargoLock.lockFile = ./Cargo.lock;
            doCheck = false;
            inherit buildInputs;
            nativeBuildInputs = buildInputs;
            LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath buildInputs}";
          };
        };
        # Enter reproducible development shell with `nix develop`
        devShells = {
          default = pkgs.mkShell {
            buildInputs = buildInputs ++ rust_tools ++ nix_tools;
          };
        };

        # Run an app with `nix run` or more specifically `nix run .#bencher`
        apps = rec {
          default = bencher;
          bencher = flake-utils.lib.mkApp {
            drv = self.packages.${system}.bencher;
          };
        };
      }
    );
}
