{
  description = "ocx - a secure Docker wrapper for OpenCode";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { nixpkgs, fenix, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        rustToolchain = fenix.packages.${system}.stable.toolchain;
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "ocx";
          version = "0.1.0";
          src = pkgs.lib.cleanSource ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = [ rustToolchain ];

          buildInputs = [];

          meta = with pkgs.lib; {
            description = "ocx - a secure Docker wrapper for OpenCode";
            homepage = "https://github.com/palekiwi-labs/ocx-rs";
            license = licenses.mit;
            maintainers = [ ];
          };
        };

        devShells.default = pkgs.mkShell
          {
            name = "ocx";
            buildInputs = [
              rustToolchain
              pkgs.rust-analyzer
              pkgs.cargo-expand
              pkgs.cargo-watch
              pkgs.cargo-edit
            ];

            shellHook = ''
              echo "Rust development environment ready!"
              echo "Rust version: $(rustc --version)"
            '';
          };
      });
}
