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
    flake-utils.lib.eachSystem ["x86_64-linux" "aarch64-linux" "aarch64-darwin"] (
      system: let
        overlays = [(import rust-overlay)];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        buildToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        rustToolchain = buildToolchain.override {
          extensions = [
            "rust-src"
            "clippy"
            "rust-analyzer"
          ];
        };
        rustPlatform = pkgs.makeRustPlatform {
          cargo = buildToolchain;
          rustc = buildToolchain;
        };

        # Build inputs for the Rust project
        packageInputs = with pkgs; [
          clang
          mold
          pkg-config
          binutils
        ];
        buildInputs = [rustToolchain] ++ packageInputs;
        rust_tools = with pkgs; [
          cargo-nextest
        ];
        nix_tools = with pkgs; [
          alejandra # Nix code formatter
          deadnix # Nix dead code checker
          statix # Nix static code checker.
        ];
        mkPackage = pname:
          rustPlatform.buildRustPackage {
            name = pname;
            src = ./.;
            cargoBuildFlags = ["--bin" "${pname}"];
            cargoLock.lockFile = ./Cargo.lock;
            doCheck = false;
            buildInputs = packageInputs;
            nativeBuildInputs = packageInputs;
            LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath packageInputs}";
            disallowedReferences = [buildToolchain rustToolchain];
          };
      in {
        # Build package with `nix build` or more specifically for example `nix run .#bencher`
        packages = rec {
          default = bencher;
          bencher = mkPackage "bencher";
        };
        # Enter reproducible development shell with `nix develop`
        devShells = {
          default = pkgs.mkShell {
            buildInputs = buildInputs ++ rust_tools ++ nix_tools;
          };
          # A more lightweight dev shell with only packages relevant for CI in it.
          ci = pkgs.mkShell {
            buildInputs = nix_tools;
          };
        };

        # Run an app with `nix run` or more specifically e.g.: `nix run .#bencher`
        apps = rec {
          default = bencher;
          # nix run .#bencher
          bencher = flake-utils.lib.mkApp {
            drv = self.packages.${system}.bencher;
          };
        };
      }
    );
}
