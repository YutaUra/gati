{
  description = "gati - a terminal tool for reviewing code, not writing it";

  nixConfig = {
    extra-substituters = ["https://yutaura.cachix.org"];
    extra-trusted-public-keys = ["yutaura.cachix.org-1:uoMGhQXiri/CBTK1IByqBipk42mkEfWhYo2q9ENseJ8="];
  };

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
          targets = [
            "aarch64-apple-darwin"
            "x86_64-apple-darwin"
            "x86_64-unknown-linux-gnu"
            "aarch64-unknown-linux-gnu"
          ];
        };
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "gati";
          version = "0.7.0";
          src = self;

          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = [ pkgs.pkg-config pkgs.cmake ];

          buildInputs = [ pkgs.oniguruma ];

          RUSTONIG_SYSTEM_LIBONIG = 1;

          # cli_clipboard requires a display server, unavailable in the sandbox
          checkFlags = [ "--skip=app::tests::export_sets_flash_message_on_success" ];

          meta = with pkgs.lib; {
            description = "A terminal tool for reviewing code, not writing it";
            homepage = "https://github.com/YutaUra/gati";
            license = licenses.mit;
            maintainers = [ ];
            mainProgram = "gati";
            platforms = platforms.unix;
          };
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [
            # Rust
            rustToolchain
            pkgs.pkg-config
            pkgs.cmake

            # Cross-compilation
            pkgs.zig
            pkgs.cargo-zigbuild

            # Node.js (for OpenSpec, requires >=20.19.0)
            pkgs.nodejs
          ];
        };
      }
    );
}
